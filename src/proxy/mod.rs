pub mod auth;
pub mod context;
pub mod early;
pub mod helpers;
pub mod lifecycle;
pub mod listen;
pub mod logging;
pub mod matching;
pub mod request;
pub mod runtime;
pub mod server;
pub mod static_files;
pub mod upstream;

pub use auth::healthz;
pub use helpers::{cached_content_types, cached_regex};
pub use listen::parse_listen_addr;
pub use logging::{clear_logs, get_logs, send_log_with_app};
pub use runtime::{is_effectively_running, is_running, start_server, stop_server};

use auth::is_basic_auth_ok;
use context::{enqueue_request_log, format_access_log, format_headers_for_log, RequestContext};
use early::{handle_access_control, handle_basic_auth_failure, handle_missing_route, handle_rate_limit};
use helpers::{content_type_allowed, is_hop_header_fast};
use logging::push_log_lazy;
use matching::match_route;
use request::prepare_proxy_request;
use static_files::serve_static_owned;
use crate::config;
use anyhow::Result;
use axum::body::Bytes;
use axum::{
    body::Body,
    extract::{connect_info::ConnectInfo, State},
    http::{HeaderName, Request, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use std::{net::SocketAddr, sync::Arc};

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) rule: config::ListenRule,
    pub(crate) client_follow: reqwest::Client,
    pub(crate) client_nofollow: reqwest::Client,
    pub(crate) app: tauri::AppHandle,
    pub(crate) listen_addr: Arc<str>,
    pub(crate) server_port: u16,
    pub(crate) stream_proxy: bool,
    pub(crate) max_body_size: usize,
    pub(crate) max_response_body_size: usize,
    pub(crate) http_access_control_enabled: bool,
    pub(crate) allow_all_lan: bool,
    pub(crate) allow_all_ip: bool,
    pub(crate) whitelist: Arc<[config::WhitelistEntry]>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RuleStartErrorPayload {
    pub listen_addr: String,
    pub error: String,
}

#[inline]
pub(crate) async fn proxy_handler(
    State(state): State<AppState>,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    req: Request<Body>,
) -> Response {
    let node = &*state.listen_addr;
    let method = req.method().clone();
    let uri = req.uri().clone();

    let ctx = RequestContext::new(remote, req.headers(), &method, &uri);
    let host = ctx.host_header.as_ref();
    let (route, matched_route_id) = match_route(&state.rule.routes, host, &ctx.path, &method, req.headers());

    if let Some(resp) = handle_access_control(&state, &ctx, &remote, req.headers(), &matched_route_id) {
        return resp;
    }

    if let Some(resp) = handle_rate_limit(&state, &ctx, &remote, &matched_route_id) {
        return resp;
    }

    if let Some(resp) = handle_basic_auth_failure(
        &state,
        &ctx,
        req.headers(),
        &remote,
        &matched_route_id,
        is_basic_auth_ok(&state.rule, route, req.headers()),
    ) {
        return resp;
    }

    let Some(route) = route else {
        return handle_missing_route(&state, &ctx, &remote, &matched_route_id);
    };

    if let Some(dir) = route.static_dir.as_ref() {
        if !state.stream_proxy {
            return serve_static_owned(&state, &ctx, &remote, &matched_route_id, dir, req).await;
        }
    }

    if route.upstreams.is_empty() {
        return (StatusCode::NOT_FOUND, "No static directory or upstream configured").into_response();
    }

    let inbound_headers = req.headers().clone();

    let request::PreparedProxyRequest {
        target,
        req_body_size,
        outbound_headers_snapshot,
        upstream_req,
    } = match prepare_proxy_request(&state, route, &ctx, &remote, &matched_route_id, req).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let client = if route.follow_redirects {
        state.client_follow.clone()
    } else {
        state.client_nofollow.clone()
    };
    let resp = match client.execute(upstream_req).await {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                format!("upstream request failed: {e}"),
            )
                .into_response();
        }
    };

    let status = resp.status();
    let response_headers = resp.headers().clone();

    push_log_lazy(&state.app, || {
        format_access_log(
            node,
            &ctx,
            StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY),
        )
    });

    enqueue_request_log(node, &ctx, &remote, status, &target, &matched_route_id);

    let mut out = Response::new(Body::empty());
    *out.status_mut() = StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);

    for (k, v) in response_headers.iter() {
        if is_hop_header_fast(k.as_str()) {
            continue;
        }
        out.headers_mut().insert(k.clone(), v.clone());
    }

    if let Some(headers_to_remove) = route.remove_headers.as_ref() {
        for header_name in headers_to_remove {
            let trimmed = header_name.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(name) = HeaderName::from_bytes(trimmed.as_bytes()) {
                out.headers_mut().remove(name);
            }
        }
    }

    if state.stream_proxy {
        let stream = resp.bytes_stream();
        *out.body_mut() = Body::from_stream(stream);
    } else {
        let bytes = match resp.bytes().await {
            Ok(b) => b,
            Err(e) => {
                return (
                    StatusCode::BAD_GATEWAY,
                    format!("read upstream body failed: {e}"),
                )
                    .into_response();
            }
        };

        if state.max_response_body_size > 0 && bytes.len() > state.max_response_body_size {
            return (
                StatusCode::BAD_GATEWAY,
                format!(
                    "upstream body too large (limit={} bytes)",
                    state.max_response_body_size
                ),
            )
                .into_response();
        }

        let final_bytes = if let Some(rules) = route.response_body_replace.as_ref() {
            match std::str::from_utf8(&bytes) {
                Ok(body_str) => {
                    let mut modified_body = body_str.to_string();
                    for rule in rules {
                        if !rule.enabled {
                            continue;
                        }

                        if let Some(ref content_types) = rule.content_types {
                            if !content_type_allowed(&response_headers, content_types) {
                                continue;
                            }
                        }

                        if rule.use_regex {
                            if let Some(re) = cached_regex(&rule.find) {
                                modified_body = re.replace_all(&modified_body, &rule.replace).to_string();
                            }
                        } else {
                            modified_body = modified_body.replace(&rule.find, &rule.replace);
                        }
                    }
                    Bytes::from(modified_body.into_bytes())
                }
                Err(_) => bytes,
            }
        } else {
            bytes
        };

        *out.body_mut() = Body::from(final_bytes);
    }

    if !status.is_success() {
        let inbound_headers_line = format_headers_for_log(&inbound_headers);
        let outbound_headers_line = format_headers_for_log(&outbound_headers_snapshot);

        send_log_with_app(&state.app, format!(
            "Reverse proxy error (IN): {} {} -> {} status={} | inbound_headers=[{}]",
            ctx.method.as_str(),
            ctx.uri,
            target,
            status.as_u16(),
            inbound_headers_line
        ));

        send_log_with_app(&state.app, format!(
            "Reverse proxy error (OUT): {} {} -> {} status={} | outbound_headers=[{}] | req_body_size={}",
            ctx.method.as_str(),
            ctx.uri,
            target,
            status.as_u16(),
            outbound_headers_line,
            req_body_size
                .map(|n| n.to_string())
                .unwrap_or_else(|| "stream".to_string())
        ));
    }

    out
}

pub fn build_upstream_url(
    upstream_base: &str,
    route_path: Option<&str>,
    proxy_pass_path: Option<&str>,
    uri: &Uri,
) -> Result<String> {
    let mut base = upstream_base.trim_end_matches('/').to_string();

    let orig_path = uri.path();
    let route_path = route_path.unwrap_or("/");

    let mut new_path = orig_path.to_string();
    if let Some(pp) = proxy_pass_path {
        let from = if route_path.is_empty() { "/" } else { route_path };
        let to = if pp.trim().is_empty() { "/" } else { pp };

        if new_path.starts_with(from) {
            let suffix = &new_path[from.len()..];

            let mut out_path = to.to_string();
            if out_path.is_empty() {
                out_path = "/".to_string();
            }

            let suffix = suffix.strip_prefix('/').unwrap_or(suffix);
            if out_path.ends_with('/') {
                new_path = if suffix.is_empty() {
                    out_path
                } else {
                    format!("{}{}", out_path, suffix)
                };
            } else {
                new_path = if suffix.is_empty() {
                    out_path
                } else {
                    format!("{}/{}", out_path, suffix)
                };
            }
        }

        if !new_path.starts_with('/') {
            new_path = format!("/{}", new_path);
        }
    }

    base.push_str(&new_path);
    if let Some(q) = uri.query() {
        base.push('?');
        base.push_str(q);
    }
    Ok(base)
}

#[inline]
#[cfg(test)]
mod tests {
    use super::build_upstream_url;
    use crate::proxy::matching::{host_matches, normalize_host};

    #[test]
    fn normalize_host_handles_port_and_ipv6() {
        assert_eq!(normalize_host("example.com:443"), "example.com");
        assert_eq!(normalize_host("[::1]:8443"), "::1");
        assert_eq!(normalize_host("::1"), "::1");
    }

    #[test]
    fn wildcard_host_matches_apex_and_subdomain() {
        assert!(host_matches("*.example.com", "example.com"));
        assert!(host_matches("*.example.com", "api.example.com"));
        assert!(host_matches("*.example.com", "A.B.Example.Com"));
    }

    #[test]
    fn wildcard_host_does_not_partial_match() {
        assert!(!host_matches("*.example.com", "evil-example.com"));
        assert!(!host_matches("*.example.com", "example.com.evil"));
    }
}
