use axum::http::{HeaderMap, Method};

use crate::config;
use crate::proxy;

#[inline]
pub fn normalize_host(host: &str) -> &str {
    let host = host.trim();
    if host.is_empty() {
        return host;
    }

    if let Some(rest) = host.strip_prefix('[') {
        if let Some(end) = rest.find(']') {
            return &rest[..end];
        }
        return host;
    }

    if let Some(idx) = host.rfind(':') {
        if !host[..idx].contains(':')
            && !host[idx + 1..].is_empty()
            && host[idx + 1..].bytes().all(|b| b.is_ascii_digit())
        {
            return &host[..idx];
        }
    }

    host
}

#[inline]
pub fn ends_with_ignore_ascii_case(s: &str, suffix: &str) -> bool {
    if s.len() < suffix.len() {
        return false;
    }
    s[s.len() - suffix.len()..].eq_ignore_ascii_case(suffix)
}

#[inline]
pub fn host_matches(route_host: &str, request_host: &str) -> bool {
    let route_host = normalize_host(route_host).trim_end_matches('.');
    let request_host = normalize_host(request_host).trim_end_matches('.');

    if request_host.is_empty() {
        return route_host.is_empty();
    }

    if route_host.eq_ignore_ascii_case(request_host) {
        return true;
    }

    if let Some(suffix) = route_host.strip_prefix("*.") {
        let suffix = suffix.trim_matches('.');
        if suffix.is_empty() {
            return false;
        }

        if request_host.eq_ignore_ascii_case(suffix) {
            return true;
        }

        if !ends_with_ignore_ascii_case(request_host, suffix) {
            return false;
        }

        let dot_idx = request_host.len().saturating_sub(suffix.len() + 1);
        return request_host
            .as_bytes()
            .get(dot_idx)
            .copied()
            .is_some_and(|b| b == b'.');
    }

    false
}

#[inline]
pub fn match_route<'a>(
    routes: &'a [config::Route],
    request_host: &str,
    path: &str,
    method: &Method,
    headers: &HeaderMap,
) -> (Option<&'a config::Route>, String) {
    let host = normalize_host(request_host);

    let mut best: Option<(&config::Route, bool, usize)> = None;

    for r in routes {
        if !r.enabled {
            continue;
        }

        let p = match r.path.as_deref() {
            Some(v) => v,
            None => continue,
        };
        if !path.starts_with(p) {
            continue;
        }

        let host_ok = match r.host.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            None => true,
            Some(h) => host_matches(h, host),
        };
        if !host_ok {
            continue;
        }

        if let Some(ref methods) = r.methods {
            if !methods.iter().any(|m| m.eq_ignore_ascii_case(method.as_str())) {
                continue;
            }
        }

        if let Some(ref required_headers) = r.headers {
            let mut headers_ok = true;
            for (key, expected) in required_headers {
                let actual = headers
                    .get(key)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");
                if expected.contains('*') {
                    let pattern = expected.replace('*', ".*");
                    if let Some(re) = proxy::cached_regex(&pattern) {
                        if !re.is_match(actual) {
                            headers_ok = false;
                            break;
                        }
                    } else if !actual.contains(expected.replace('*', "").as_str()) {
                        headers_ok = false;
                        break;
                    }
                } else if !actual.eq_ignore_ascii_case(expected.trim()) {
                    headers_ok = false;
                    break;
                }
            }
            if !headers_ok {
                continue;
            }
        }

        let cand = (r, r.host.as_ref().is_some(), p.len());
        best = match best {
            None => Some(cand),
            Some((best_r, best_has_host, best_plen)) => {
                if cand.1 != best_has_host {
                    if cand.1 { Some(cand) } else { Some((best_r, best_has_host, best_plen)) }
                } else if cand.2 > best_plen {
                    Some(cand)
                } else {
                    Some((best_r, best_has_host, best_plen))
                }
            }
        };
    }

    if let Some((r, _, _)) = best {
        (Some(r), r.id.as_deref().unwrap_or("").to_string())
    } else {
        (None, String::new())
    }
}
