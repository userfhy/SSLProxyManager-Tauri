pub mod auth;
pub mod context;
pub mod dispatch;
pub mod early;
pub mod helpers;
pub mod lifecycle;
pub mod listen;
pub mod logging;
pub mod matching;
pub mod request;
pub mod response;
pub mod runtime;
pub mod server;
pub mod static_files;
pub mod types;
pub mod upstream;

pub use auth::healthz;
pub use helpers::{cached_content_types, cached_regex};
pub use listen::parse_listen_addr;
pub use logging::{clear_logs, get_logs, send_log_with_app};
pub use runtime::{is_effectively_running, is_running, start_server, stop_server};
pub use types::RuleStartErrorPayload;
use types::AppState;

use context::RequestContext;
use dispatch::{resolve_route_and_run_guards, GuardOutcome};
use request::prepare_proxy_request;
use response::{handle_upstream_response, ProxyResponseMeta};
use static_files::serve_static_owned;
use axum::{
    body::Body,
    extract::{connect_info::ConnectInfo, State},
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
};
use std::net::SocketAddr;

#[inline]
pub(crate) async fn proxy_handler(
    State(state): State<AppState>,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    req: Request<Body>,
) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();

    let ctx = RequestContext::new(remote, req.headers(), &method, &uri);
    let GuardOutcome {
        route,
        matched_route_id,
    } = match resolve_route_and_run_guards(&state, &ctx, &remote, &method, req.headers()) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    if let Some(dir) = route.static_dir.as_ref() {
        if !state.stream_proxy {
            return serve_static_owned(&state, &ctx, &remote, &matched_route_id, dir, req).await;
        }
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

    handle_upstream_response(
        &state,
        route,
        &ctx,
        resp,
        ProxyResponseMeta {
            target: &target,
            req_body_size,
            outbound_headers_snapshot: &outbound_headers_snapshot,
            inbound_headers: &inbound_headers,
            matched_route_id: &matched_route_id,
            remote: &remote,
        },
    ).await
}

#[cfg(test)]
mod tests {
    use super::request::rewrite_uri;
    use super::upstream::build_upstream_url;
    use crate::config::{Route, Upstream, UrlRewriteRule};
    use crate::proxy::matching::{host_matches, normalize_host};
    use axum::http::Uri;

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

    #[test]
    fn build_upstream_url_rewrites_prefix_and_keeps_query() {
        let uri: Uri = "/api/users?id=42".parse().unwrap();
        let out = build_upstream_url(
            "http://backend:8080/",
            Some("/api"),
            Some("/v1/"),
            &uri,
        )
        .unwrap();

        assert_eq!(out, "http://backend:8080/v1/users?id=42");
    }

    #[test]
    fn build_upstream_url_preserves_original_path_without_proxy_pass_path() {
        let uri: Uri = "/assets/app.js".parse().unwrap();
        let out = build_upstream_url("http://cdn.local", Some("/assets"), None, &uri).unwrap();

        assert_eq!(out, "http://cdn.local/assets/app.js");
    }

    #[test]
    fn rewrite_uri_applies_enabled_rules_only() {
        let route = Route {
            id: Some("rewrite-test".into()),
            enabled: true,
            host: None,
            path: Some("/".into()),
            proxy_pass_path: None,
            set_headers: None,
            static_dir: None,
            exclude_basic_auth: None,
            basic_auth_enable: None,
            basic_auth_username: None,
            basic_auth_password: None,
            basic_auth_forward_header: None,
            follow_redirects: false,
            compression_enabled: None,
            compression_gzip: None,
            compression_brotli: None,
            compression_min_length: None,
            url_rewrite_rules: Some(vec![
                UrlRewriteRule {
                    pattern: "/old/(.*)".into(),
                    replacement: "/new/$1".into(),
                    enabled: true,
                },
                UrlRewriteRule {
                    pattern: "new".into(),
                    replacement: "ignored".into(),
                    enabled: false,
                },
            ]),
            request_body_replace: None,
            response_body_replace: None,
            remove_headers: None,
            methods: None,
            headers: None,
            upstreams: vec![Upstream {
                url: "http://backend".into(),
                weight: 1,
            }],
        };

        let uri: Uri = "/old/path?q=1".parse().unwrap();
        let out = rewrite_uri(&route, &uri);
        assert_eq!(out.to_string(), "/new/path?q=1");
    }

    #[test]
    fn rewrite_uri_ignores_invalid_rewrite_result() {
        let route = Route {
            id: Some("rewrite-invalid".into()),
            enabled: true,
            host: None,
            path: Some("/".into()),
            proxy_pass_path: None,
            set_headers: None,
            static_dir: None,
            exclude_basic_auth: None,
            basic_auth_enable: None,
            basic_auth_username: None,
            basic_auth_password: None,
            basic_auth_forward_header: None,
            follow_redirects: false,
            compression_enabled: None,
            compression_gzip: None,
            compression_brotli: None,
            compression_min_length: None,
            url_rewrite_rules: Some(vec![UrlRewriteRule {
                pattern: "^/ok$".into(),
                replacement: "http://bad uri".into(),
                enabled: true,
            }]),
            request_body_replace: None,
            response_body_replace: None,
            remove_headers: None,
            methods: None,
            headers: None,
            upstreams: vec![Upstream {
                url: "http://backend".into(),
                weight: 1,
            }],
        };

        let uri: Uri = "/ok".parse().unwrap();
        let out = rewrite_uri(&route, &uri);
        assert_eq!(out, uri);
    }
}
