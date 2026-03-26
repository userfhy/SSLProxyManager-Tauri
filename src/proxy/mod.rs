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
pub mod stream_proxy;
pub mod types;
pub mod upstream;
pub mod ws_proxy;

pub use auth::healthz;
pub use helpers::{cached_content_types, cached_regex};
pub use listen::parse_listen_addr;
pub use logging::{clear_logs, get_logs, send_log_with_app};
pub use runtime::{is_effectively_running, is_running, start_server, stop_server};
use types::AppState;
pub use types::RuleStartErrorPayload;

use axum::{
    body::Body,
    extract::{connect_info::ConnectInfo, State},
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
};
use context::RequestContext;
use dispatch::{resolve_route_and_run_guards, GuardOutcome};
use request::prepare_proxy_request;
use response::{handle_upstream_response, ProxyResponseMeta};
use static_files::serve_static_owned;
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
    let t_guard = std::time::Instant::now();
    let GuardOutcome {
        route,
        matched_route_id,
    } = match resolve_route_and_run_guards(&state, &ctx, &remote, &method, req.headers()) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let guard_ms = t_guard.elapsed().as_secs_f64() * 1000.0;

    if let Some(dir) = route.static_dir.as_ref() {
        if !state.stream_proxy {
            return serve_static_owned(&state, &ctx, &remote, &matched_route_id, dir, req).await;
        }
    }

    let inbound_headers = req.headers().clone();

    let t_prepare = std::time::Instant::now();
    let request::PreparedProxyRequest {
        target,
        req_body_size,
        outbound_headers_snapshot,
        upstream_req,
    } = match prepare_proxy_request(&state, route, &ctx, &remote, &matched_route_id, req).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let prepare_ms = t_prepare.elapsed().as_secs_f64() * 1000.0;

    let client = if route.follow_redirects {
        state.client_follow.clone()
    } else {
        state.client_nofollow.clone()
    };
    let t_upstream = std::time::Instant::now();
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
    let upstream_ms = t_upstream.elapsed().as_secs_f64() * 1000.0;

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
            guard_ms,
            prepare_ms,
            upstream_ms,
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::helpers::content_type_allowed;
    use super::request::rewrite_uri;
    use super::upstream::build_upstream_url;
    use crate::config::{Route, Upstream, UrlRewriteRule};
    use crate::proxy::matching::{host_matches, match_route, normalize_host};
    use axum::http::{HeaderMap, Method, Uri};

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
        let out =
            build_upstream_url("http://backend:8080/", Some("/api"), Some("/v1/"), &uri).unwrap();

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

    #[test]
    fn content_type_allowed_matches_case_insensitively_and_ignores_charset() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "content-type",
            "Application/JSON; charset=utf-8".parse().unwrap(),
        );

        assert!(content_type_allowed(
            &headers,
            "application/json, text/html"
        ));
        assert!(!content_type_allowed(&headers, "text/plain"));
    }

    #[test]
    fn content_type_allowed_returns_false_when_header_missing_and_rule_non_empty() {
        let headers = HeaderMap::new();
        assert!(!content_type_allowed(&headers, "application/json"));
        assert!(content_type_allowed(&headers, "   "));
    }

    #[test]
    fn match_route_prefers_host_specific_and_longer_path() {
        let routes = vec![
            Route {
                id: Some("generic-api".into()),
                enabled: true,
                host: None,
                path: Some("/api".into()),
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
                url_rewrite_rules: None,
                request_body_replace: None,
                response_body_replace: None,
                remove_headers: None,
                methods: None,
                headers: None,
                upstreams: vec![Upstream {
                    url: "http://a".into(),
                    weight: 1,
                }],
            },
            Route {
                id: Some("host-specific-users".into()),
                enabled: true,
                host: Some("api.example.com".into()),
                path: Some("/api/users".into()),
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
                url_rewrite_rules: None,
                request_body_replace: None,
                response_body_replace: None,
                remove_headers: None,
                methods: None,
                headers: None,
                upstreams: vec![Upstream {
                    url: "http://b".into(),
                    weight: 1,
                }],
            },
        ];

        let headers = HeaderMap::new();
        let (route, route_id) = match_route(
            &routes,
            "api.example.com",
            "/api/users/42",
            &Method::GET,
            &headers,
        );

        assert_eq!(
            route.and_then(|r| r.id.as_deref()),
            Some("host-specific-users")
        );
        assert_eq!(route_id, "host-specific-users");
    }

    #[test]
    fn match_route_checks_method_and_header_filters() {
        let mut required_headers = std::collections::HashMap::new();
        required_headers.insert("x-env".into(), "prod-*".into());

        let routes = vec![Route {
            id: Some("filtered".into()),
            enabled: true,
            host: Some("example.com".into()),
            path: Some("/svc".into()),
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
            url_rewrite_rules: None,
            request_body_replace: None,
            response_body_replace: None,
            remove_headers: None,
            methods: Some(vec!["POST".into()]),
            headers: Some(required_headers),
            upstreams: vec![Upstream {
                url: "http://svc".into(),
                weight: 1,
            }],
        }];

        let mut headers = HeaderMap::new();
        headers.insert("x-env", "prod-cn".parse().unwrap());

        let (matched, _) = match_route(&routes, "example.com", "/svc/run", &Method::POST, &headers);
        assert!(matched.is_some());

        let (wrong_method, _) =
            match_route(&routes, "example.com", "/svc/run", &Method::GET, &headers);
        assert!(wrong_method.is_none());

        headers.insert("x-env", "staging".parse().unwrap());
        let (wrong_header, _) =
            match_route(&routes, "example.com", "/svc/run", &Method::POST, &headers);
        assert!(wrong_header.is_none());
    }
}
