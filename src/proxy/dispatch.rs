use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use std::net::SocketAddr;

use super::AppState;
use super::auth::is_basic_auth_ok;
use super::context::RequestContext;
use super::early::{
    handle_access_control, handle_basic_auth_failure, handle_missing_route, handle_rate_limit,
};
use super::matching::match_route;

pub struct GuardOutcome<'a> {
    pub route: &'a crate::config::Route,
    pub matched_route_id: String,
}

pub fn resolve_route_and_run_guards<'a>(
    state: &'a AppState,
    ctx: &RequestContext,
    remote: &SocketAddr,
    method: &axum::http::Method,
    req_headers: &HeaderMap,
) -> Result<GuardOutcome<'a>, Response> {
    let host = ctx.host_header.as_ref();
    let (route, matched_route_id) = match_route(&state.rule.routes, host, &ctx.path, method, req_headers);

    if let Some(resp) = handle_access_control(state, ctx, remote, req_headers, &matched_route_id) {
        return Err(resp);
    }

    if let Some(resp) = handle_rate_limit(state, ctx, remote, &matched_route_id) {
        return Err(resp);
    }

    if let Some(resp) = handle_basic_auth_failure(
        state,
        ctx,
        req_headers,
        remote,
        &matched_route_id,
        is_basic_auth_ok(&state.rule, route, req_headers),
    ) {
        return Err(resp);
    }

    let Some(route) = route else {
        return Err(handle_missing_route(state, ctx, remote, &matched_route_id));
    };

    if route.upstreams.is_empty() && route.static_dir.is_none() {
        return Err((StatusCode::NOT_FOUND, "No static directory or upstream configured").into_response());
    }

    Ok(GuardOutcome {
        route,
        matched_route_id,
    })
}
