use anyhow::{anyhow, Result};
use axum::body::Bytes;
use axum::http::{HeaderMap, HeaderValue};
use dashmap::DashMap;
use regex::Regex;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tower_http::services::ServeDir;

use crate::cache_optimizer;

pub static REGEX_CACHE: once_cell::sync::Lazy<DashMap<String, Arc<Regex>>> =
    once_cell::sync::Lazy::new(DashMap::new);

pub static CONTENT_TYPE_LIST_CACHE: once_cell::sync::Lazy<DashMap<String, Arc<[String]>>> =
    once_cell::sync::Lazy::new(DashMap::new);

pub static STATIC_DIR_SERVICE_CACHE: once_cell::sync::Lazy<DashMap<String, ServeDir>> =
    once_cell::sync::Lazy::new(DashMap::new);

pub static INDEX_HTML_CACHE: once_cell::sync::Lazy<DashMap<String, (Instant, Bytes)>> =
    once_cell::sync::Lazy::new(DashMap::new);

pub const INDEX_HTML_CACHE_TTL: Duration = Duration::from_secs(2);

pub struct EtagEntry {
    pub mtime: std::time::SystemTime,
    pub etag: String,
}

pub static ETAG_CACHE: once_cell::sync::Lazy<DashMap<String, EtagEntry>> =
    once_cell::sync::Lazy::new(DashMap::new);

#[inline]
pub fn generate_etag(path: &std::path::Path, mtime: std::time::SystemTime, size: u64) -> String {
    use std::hash::{Hash, Hasher};
    use std::time::UNIX_EPOCH;

    let mtime_secs = mtime
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let hash_input = format!("{}-{}-{}", mtime_secs, size, path.display());

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    hash_input.hash(&mut hasher);
    let hash = hasher.finish();

    format!("\"{:x}\"", hash)
}

#[inline]
pub fn get_or_create_etag(path: &std::path::Path) -> Option<String> {
    let metadata = path.metadata().ok()?;
    let mtime = metadata.modified().ok()?;
    let size = metadata.len();

    let key = path.to_string_lossy().to_string();

    if let Some(entry) = ETAG_CACHE.get(&key) {
        let cached_mtime = entry.mtime;
        let cached_etag = entry.etag.clone();
        if cached_mtime == mtime {
            return Some(cached_etag);
        }
    }

    let etag = generate_etag(path, mtime, size);
    ETAG_CACHE.insert(
        key,
        EtagEntry {
            mtime,
            etag: etag.clone(),
        },
    );
    Some(etag)
}

#[inline]
pub fn check_etag_match(request_etag: Option<&str>, file_etag: &str) -> bool {
    match request_etag {
        None => false,
        Some(req_etag) => req_etag
            .split(',')
            .any(|e| e.trim() == file_etag || e.trim() == "*"),
    }
}

#[inline]
pub fn cached_regex(pattern: &str) -> Option<Arc<Regex>> {
    if let Ok(regex) = cache_optimizer::global_cache_manager()
        .regex_cache()
        .get_or_compile(pattern)
    {
        return Some(regex);
    }

    if let Some(entry) = REGEX_CACHE.get(pattern) {
        return Some(entry.clone());
    }
    match Regex::new(pattern) {
        Ok(re) => {
            let arc = Arc::new(re);
            REGEX_CACHE.insert(pattern.to_string(), arc.clone());
            Some(arc)
        }
        Err(_) => None,
    }
}

#[inline]
pub fn cached_content_types(raw: &str) -> Arc<[String]> {
    let key = raw.trim();
    if key.is_empty() {
        return Arc::from(Vec::<String>::new());
    }

    if let Some(entry) = CONTENT_TYPE_LIST_CACHE.get(key) {
        return entry.clone();
    }

    let parsed: Vec<String> = key
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_ascii_lowercase())
        .collect();

    let arc: Arc<[String]> = Arc::from(parsed);
    CONTENT_TYPE_LIST_CACHE.insert(key.to_string(), arc.clone());
    arc
}

#[inline]
pub fn pure_content_type_from_headers<'a>(headers: &'a HeaderMap) -> Option<&'a str> {
    headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v: &HeaderValue| v.to_str().ok())
        .and_then(|s| s.split(';').next())
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

#[inline]
pub fn content_type_allowed(headers: &HeaderMap, configured: &str) -> bool {
    let allowed = cached_content_types(configured);
    if allowed.is_empty() {
        return true;
    }

    let Some(actual) = pure_content_type_from_headers(headers) else {
        return false;
    };

    allowed.iter().any(|ct| ct.eq_ignore_ascii_case(actual))
}

#[allow(dead_code)]
pub async fn resolve_hostname_with_cache(hostname: &str) -> Result<Vec<std::net::IpAddr>> {
    use std::net::IpAddr;

    let dns_cache = cache_optimizer::global_cache_manager().dns_cache();

    if let Some(ips) = dns_cache.get(hostname) {
        tracing::debug!("DNS cache hit: {}", hostname);
        return Ok(ips);
    }

    tracing::debug!("DNS cache miss, resolving: {}", hostname);
    let ips: Vec<IpAddr> = tokio::net::lookup_host(format!("{}:0", hostname))
        .await
        .map_err(|e| anyhow!("DNS lookup failed: {}", e))?
        .map(|addr| addr.ip())
        .collect();

    if !ips.is_empty() {
        dns_cache.put(hostname.to_string(), ips.clone());
    }

    Ok(ips)
}

#[inline]
pub fn cached_serve_dir(dir: &str) -> ServeDir {
    if let Some(entry) = STATIC_DIR_SERVICE_CACHE.get(dir) {
        return entry.clone();
    }

    let service = ServeDir::new(dir).precompressed_gzip().precompressed_br();
    STATIC_DIR_SERVICE_CACHE.insert(dir.to_string(), service.clone());
    service
}

pub async fn cached_index_html(dir: &str) -> Option<Bytes> {
    let now = Instant::now();
    if let Some(entry) = INDEX_HTML_CACHE.get(dir) {
        let (cached_at, bytes) = entry.value();
        if now.duration_since(*cached_at) <= INDEX_HTML_CACHE_TTL {
            return Some(bytes.clone());
        }
    }

    let bytes = tokio::fs::read(std::path::Path::new(dir).join("index.html"))
        .await
        .ok()?;
    let bytes = Bytes::from(bytes);
    INDEX_HTML_CACHE.insert(dir.to_string(), (now, bytes.clone()));
    Some(bytes)
}

#[inline]
pub fn is_asset_path(path: &str) -> bool {
    path.contains('.') || path.starts_with("/assets/") || path.starts_with("/static/")
}

#[inline]
pub fn is_hop_header_fast(name: &str) -> bool {
    name.eq_ignore_ascii_case("connection")
        || name.eq_ignore_ascii_case("keep-alive")
        || name.eq_ignore_ascii_case("proxy-authenticate")
        || name.eq_ignore_ascii_case("proxy-authorization")
        || name.eq_ignore_ascii_case("te")
        || name.eq_ignore_ascii_case("trailer")
        || name.eq_ignore_ascii_case("transfer-encoding")
        || name.eq_ignore_ascii_case("upgrade")
}

#[inline]
pub fn expand_proxy_header_value(
    raw: &str,
    remote: &SocketAddr,
    inbound_headers: &HeaderMap,
    is_tls: bool,
) -> String {
    if !(raw.contains('$')) {
        return raw.to_string();
    }

    let remote_ip = remote.ip().to_string();
    let scheme = if is_tls { "https" } else { "http" };
    let host = inbound_headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let proxy_add_xff = if raw.contains("$proxy_add_x_forwarded_for") {
        let prior = inbound_headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty());

        Some(match prior {
            Some(p) => format!("{}, {}", p, remote_ip),
            None => remote_ip.clone(),
        })
    } else {
        None
    };

    let mut out = String::with_capacity(raw.len() + 32);
    let bytes = raw.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'$' {
            let rest = &raw[i..];
            if rest.starts_with("$remote_addr") {
                out.push_str(&remote_ip);
                i += "$remote_addr".len();
                continue;
            }
            if rest.starts_with("$host") {
                out.push_str(&host);
                i += "$host".len();
                continue;
            }
            if rest.starts_with("$scheme") {
                out.push_str(scheme);
                i += "$scheme".len();
                continue;
            }
            if rest.starts_with("$proxy_add_x_forwarded_for") {
                if let Some(ref v) = proxy_add_xff {
                    out.push_str(v);
                }
                i += "$proxy_add_x_forwarded_for".len();
                continue;
            }
        }

        out.push(bytes[i] as char);
        i += 1;
    }

    out
}

#[cfg(test)]
mod tests {
    use super::{
        cached_content_types, check_etag_match, content_type_allowed, expand_proxy_header_value,
        pure_content_type_from_headers,
    };
    use axum::http::{HeaderMap, HeaderValue};
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    #[test]
    fn cached_content_types_trims_and_normalizes_values() {
        let parsed = cached_content_types(" Text/Html , application/json ,, ");
        assert_eq!(&*parsed, &["text/html".to_string(), "application/json".to_string()]);
    }

    #[test]
    fn pure_content_type_from_headers_strips_charset_suffix() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static("application/json; charset=utf-8"),
        );

        assert_eq!(pure_content_type_from_headers(&headers), Some("application/json"));
    }

    #[test]
    fn content_type_allowed_returns_true_for_empty_configuration() {
        let headers = HeaderMap::new();
        assert!(content_type_allowed(&headers, " "));
    }

    #[test]
    fn check_etag_match_supports_wildcard_and_multi_value_headers() {
        assert!(check_etag_match(Some("\"abc\", \"def\""), "\"def\""));
        assert!(check_etag_match(Some("*"), "\"xyz\""));
        assert!(!check_etag_match(Some("\"abc\""), "\"def\""));
    }

    #[test]
    fn expand_proxy_header_value_expands_common_placeholders() {
        let remote = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 1, 2, 3)), 4567);
        let mut headers = HeaderMap::new();
        headers.insert("host", HeaderValue::from_static("example.com"));
        headers.insert("x-forwarded-for", HeaderValue::from_static("1.1.1.1"));

        let expanded = expand_proxy_header_value(
            "$scheme://$host from $remote_addr via $proxy_add_x_forwarded_for",
            &remote,
            &headers,
            true,
        );

        assert_eq!(
            expanded,
            "https://example.com from 10.1.2.3 via 1.1.1.1, 10.1.2.3"
        );
    }

    #[test]
    fn expand_proxy_header_value_keeps_plain_string_unchanged() {
        let remote = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 80);
        let expanded = expand_proxy_header_value("fixed-value", &remote, &HeaderMap::new(), false);
        assert_eq!(expanded, "fixed-value");
    }
}
