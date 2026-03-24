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
