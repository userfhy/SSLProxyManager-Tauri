pub mod auth;
pub mod context;
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

use auth::is_basic_auth_ok;
use context::RequestContext;
use early::{handle_access_control, handle_basic_auth_failure, handle_missing_route, handle_rate_limit};
use matching::match_route;
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

#[inline]
#[cfg(test)]
mod tests {
    use super::upstream::build_upstream_url;
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
