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
    http::{header, HeaderMap, HeaderName, HeaderValue, Method, Request, Response as HttpResponse, StatusCode},
    response::{IntoResponse, Response},
};
use context::{enqueue_request_log, format_access_log, RequestContext};
use dispatch::{resolve_route_and_run_guards, GuardOutcome};
use helpers::{expand_proxy_header_value, is_hop_header_fast};
use request::prepare_proxy_request;
use response::{handle_upstream_response, ProxyResponseMeta};
use static_files::serve_static_owned;
use logging::{push_log_lazy, SKIP_HEADERS};
use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use hyper::upgrade::{on, OnUpgrade};
use hyper_util::rt::tokio::WithTokioIo;
use std::net::SocketAddr;
use tokio_tungstenite::{connect_async, tungstenite::client::IntoClientRequest, tungstenite::protocol::Role, MaybeTlsStream, WebSocketStream};

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

    if is_websocket_upgrade(&method, req.headers()) {
        return handle_websocket_upgrade(
            &state,
            route,
            &ctx,
            &remote,
            &matched_route_id,
            req,
            guard_ms,
        )
        .await;
    }

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

fn is_websocket_upgrade(method: &Method, headers: &HeaderMap) -> bool {
    if method != Method::GET {
        return false;
    }

    let upgrade_header = headers
        .get(header::UPGRADE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false);

    let connection_header = headers
        .get(header::CONNECTION)
        .and_then(|v| v.to_str().ok())
        .map(|v| {
            v.split(',')
                .any(|item| item.trim().eq_ignore_ascii_case("upgrade"))
        })
        .unwrap_or(false);

    let has_ws_key = headers.contains_key("sec-websocket-key");
    let has_ws_version = headers
        .get("sec-websocket-version")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.trim() == "13")
        .unwrap_or(false);

    upgrade_header && connection_header && has_ws_key && has_ws_version
}

fn ws_upstream_url(upstream_url: &str) -> String {
    if let Some(rest) = upstream_url.strip_prefix("https://") {
        format!("wss://{}", rest)
    } else if let Some(rest) = upstream_url.strip_prefix("http://") {
        format!("ws://{}", rest)
    } else {
        upstream_url.to_string()
    }
}

async fn handle_websocket_upgrade(
    state: &AppState,
    route: &crate::config::Route,
    ctx: &RequestContext,
    remote: &SocketAddr,
    matched_route_id: &str,
    req: Request<Body>,
    guard_ms: f64,
) -> Response {
    let inbound_headers = req.headers().clone();
    let target_upstream = upstream::pick_upstream_smooth(route).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            "No static directory or upstream configured",
        )
            .into_response()
    });

    let target_upstream = match target_upstream {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let final_uri = request::rewrite_uri(route, &ctx.uri);
    let target_upstream = request::select_upstream_url(state, &target_upstream);
    let target_url = match upstream::build_upstream_url(
        &ws_upstream_url(&target_upstream),
        route.path.as_deref(),
        route.proxy_pass_path.as_deref(),
        &final_uri,
    ) {
        Ok(u) => u,
        Err(e) => {
            let status = StatusCode::BAD_GATEWAY;
            push_log_lazy(&state.app, || format_access_log(&*state.listen_addr, ctx, status));
            enqueue_request_log(
                &*state.listen_addr,
                ctx,
                remote,
                status,
                &target_upstream,
                matched_route_id,
                guard_ms,
                0.0,
                0.0,
            );
            return (status, format!("bad upstream url: {e}")).into_response();
        }
    };

    let mut request = match target_url.clone().into_client_request() {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                format!("build websocket upstream request failed: {e}"),
            )
                .into_response();
        }
    };

    let mut final_headers = HeaderMap::with_capacity(inbound_headers.len() + 8);
    for (k, v) in inbound_headers.iter() {
        let name = k.as_str();
        if SKIP_HEADERS.contains(k)
            || (is_hop_header_fast(name)
                && !name.eq_ignore_ascii_case(header::CONNECTION.as_str())
                && !name.eq_ignore_ascii_case(header::UPGRADE.as_str()))
        {
            continue;
        }
        final_headers.append(k.clone(), v.clone());
    }

    let remote_ip = remote.ip().to_string();
    if let Ok(v) = HeaderValue::from_str(&remote_ip) {
        final_headers.insert(HeaderName::from_static("x-real-ip"), v);
    }

    let prior = inbound_headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    let combined = match prior {
        Some(p) => format!("{}, {}", p, remote_ip),
        None => remote_ip,
    };
    if let Ok(v) = HeaderValue::from_str(&combined) {
        final_headers.insert(HeaderName::from_static("x-forwarded-for"), v);
    }

    if let Ok(v) = HeaderValue::from_str(if state.rule.ssl_enable { "https" } else { "http" }) {
        final_headers.insert(HeaderName::from_static("x-forwarded-proto"), v);
    }

    if let Some(map) = route.set_headers.as_ref() {
        for (k, v) in map {
            let key = k.trim();
            if key.is_empty() || is_hop_header_fast(key) {
                continue;
            }

            let expanded = expand_proxy_header_value(v, remote, &inbound_headers, state.rule.ssl_enable);
            let name = match HeaderName::from_bytes(key.as_bytes()) {
                Ok(n) => n,
                Err(_) => continue,
            };

            if expanded.is_empty() {
                final_headers.insert(name, HeaderValue::from_static(""));
                continue;
            }

            let value = match HeaderValue::from_str(&expanded) {
                Ok(v) => v,
                Err(_) => continue,
            };
            final_headers.insert(name, value);
        }
    }

    if state.rule.basic_auth_enable && !state.rule.basic_auth_forward_header {
        final_headers.remove(header::AUTHORIZATION);
    }

    if let Some(headers_to_remove) = route.remove_headers.as_ref() {
        for header_name in headers_to_remove {
            let trimmed = header_name.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(name) = HeaderName::from_bytes(trimmed.as_bytes()) {
                final_headers.remove(name);
            }
        }
    }

    for (k, v) in final_headers.iter() {
        request.headers_mut().insert(k.clone(), v.clone());
    }

    let _outbound_headers_snapshot = request.headers().clone();
    let on_upgrade = on(req);

    let t_prepare = std::time::Instant::now();
    let (upstream_ws, response) = match connect_async(request).await {
        Ok((ws, resp)) => (ws, resp),
        Err(tokio_tungstenite::tungstenite::Error::Http(resp)) => {
            return log_and_build_ws_error_response(
                state,
                ctx,
                remote,
                matched_route_id,
                &target_url,
                guard_ms,
                *resp,
            );
        }
        Err(e) => {
            let status = StatusCode::BAD_GATEWAY;
            push_log_lazy(&state.app, || format_access_log(&*state.listen_addr, ctx, status));
            enqueue_request_log(
                &*state.listen_addr,
                ctx,
                remote,
                status,
                &target_url,
                matched_route_id,
                guard_ms,
                0.0,
                0.0,
            );
            return (status, format!("upstream websocket connect failed: {e}")).into_response();
        }
    };
    let upstream_ms = t_prepare.elapsed().as_secs_f64() * 1000.0;

    let status = StatusCode::from_u16(response.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    push_log_lazy(&state.app, || format_access_log(&*state.listen_addr, ctx, status));
    enqueue_request_log(
        &*state.listen_addr,
        ctx,
        remote,
        status,
        &target_url,
        matched_route_id,
        guard_ms,
        0.0,
        upstream_ms,
    );

    let mut out = Response::new(Body::empty());
    *out.status_mut() = status;
    for (k, v) in response.headers().iter() {
        out.headers_mut().insert(k.clone(), v.clone());
    }

    let app = state.app.clone();
    tokio::spawn(async move {
        if let Err(e) = proxy_websocket_streams(on_upgrade, upstream_ws).await {
            send_log_with_app(&app, format!("WS proxy tunnel error: {e}"));
        }
    });

    out
}

fn log_and_build_ws_error_response<B>(
    state: &AppState,
    ctx: &RequestContext,
    remote: &SocketAddr,
    matched_route_id: &str,
    target_url: &str,
    guard_ms: f64,
    response: HttpResponse<B>,
) -> Response {
    let status = StatusCode::from_u16(response.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    push_log_lazy(&state.app, || format_access_log(&*state.listen_addr, ctx, status));
    enqueue_request_log(
        &*state.listen_addr,
        ctx,
        remote,
        status,
        target_url,
        matched_route_id,
        guard_ms,
        0.0,
        0.0,
    );

    let mut out = Response::new(Body::empty());
    *out.status_mut() = status;
    for (k, v) in response.headers().iter() {
        out.headers_mut().insert(k.clone(), v.clone());
    }
    out
}

async fn proxy_websocket_streams(
    on_upgrade: OnUpgrade,
    upstream_ws: WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
) -> Result<()> {
    let upgraded = on_upgrade
        .await
        .map_err(|e| anyhow!("client websocket upgrade failed: {e}"))?;

    let client_ws = WebSocketStream::from_raw_socket(WithTokioIo::new(upgraded), Role::Server, None).await;

    let (mut c_tx, mut c_rx) = client_ws.split();
    let (mut u_tx, mut u_rx) = upstream_ws.split();

    let c_to_u = async {
        while let Some(msg) = c_rx.next().await {
            let msg = msg.map_err(|e| anyhow!(e))?;
            u_tx.send(msg).await.map_err(|e| anyhow!(e))?;
        }
        Ok::<(), anyhow::Error>(())
    };

    let u_to_c = async {
        while let Some(msg) = u_rx.next().await {
            let msg = msg.map_err(|e| anyhow!(e))?;
            c_tx.send(msg).await.map_err(|e| anyhow!(e))?;
        }
        Ok::<(), anyhow::Error>(())
    };

    tokio::select! {
        r = c_to_u => r?,
        r = u_to_c => r?,
    }

    Ok(())
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
