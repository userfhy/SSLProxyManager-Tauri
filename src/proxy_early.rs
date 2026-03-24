use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use std::net::SocketAddr;

use crate::proxy::AppState;
use crate::proxy_auth::unauthorized_response;
use crate::proxy_context::{enqueue_request_log, format_access_log, format_headers_for_log, RequestContext};
use crate::proxy_logging::{push_log_lazy, send_log_with_app};
use crate::{access_control, metrics, rate_limit};

pub fn handle_access_control(
    state: &AppState,
    ctx: &RequestContext,
    remote: &SocketAddr,
    req_headers: &HeaderMap,
    matched_route_id: &str,
) -> Option<Response> {
    if !state.http_access_control_enabled {
        return None;
    }

    let node = &*state.listen_addr;

    if metrics::is_ip_blacklisted(&ctx.client_ip) {
        let status = StatusCode::FORBIDDEN;
        push_log_lazy(&state.app, || format_access_log(node, ctx, status));

        let inbound_headers_line = format_headers_for_log(req_headers);
        send_log_with_app(&state.app, format!(
            "Reverse proxy error (IN): {} {} -> [IP Blacklist] status={} | inbound_headers=[{}]",
            ctx.method.as_str(),
            ctx.uri,
            status.as_u16(),
            inbound_headers_line
        ));

        enqueue_request_log(node, ctx, remote, status, "", matched_route_id);
        return Some((status, "IP Forbidden").into_response());
    }

    let allowed = access_control::is_allowed_remote_ip(
        remote,
        state.allow_all_lan,
        state.allow_all_ip,
        &state.whitelist,
    );

    if !allowed {
        let status = StatusCode::FORBIDDEN;
        tracing::info!(
            "Access denied: client_ip={}, remote_ip={}, allow_all_lan={}, whitelist_len={}",
            ctx.client_ip,
            remote.ip(),
            state.allow_all_lan,
            state.whitelist.len()
        );
        push_log_lazy(&state.app, || format_access_log(node, ctx, status));

        let inbound_headers_line = format_headers_for_log(req_headers);
        send_log_with_app(&state.app, format!(
            "Reverse proxy error (IN): {} {} -> [Access Control Denied] status={} | inbound_headers=[{}] | client_ip={}, remote_ip={}, allow_all_lan={}, allow_all_ip={}, whitelist_len={}",
            ctx.method.as_str(),
            ctx.uri,
            status.as_u16(),
            inbound_headers_line,
            ctx.client_ip,
            remote.ip(),
            state.allow_all_lan,
            state.allow_all_ip,
            state.whitelist.len()
        ));

        enqueue_request_log(node, ctx, remote, status, "", matched_route_id);
        return Some((status, "Forbidden").into_response());
    }

    None
}

pub fn handle_rate_limit(
    state: &AppState,
    ctx: &RequestContext,
    remote: &SocketAddr,
    matched_route_id: &str,
) -> Option<Response> {
    if !state.rule.rate_limit_enabled.unwrap_or(false) {
        return None;
    }

    let node = &*state.listen_addr;
    let Some(limiter) = rate_limit::RATE_LIMITERS.get(node) else {
        return None;
    };

    let (allowed, should_ban) = limiter.read().check(&ctx.client_ip);
    if allowed {
        return None;
    }

    if should_ban {
        let ban_seconds = state.rule.rate_limit_ban_seconds.unwrap_or(0) as i32;
        if ban_seconds > 0 {
            let ip_str: String = ctx.client_ip.as_ref().into();
            let app_clone = state.app.clone();
            tokio::spawn(async move {
                if let Err(e) = metrics::add_blacklist_entry(
                    ip_str.clone(),
                    format!("Rate limit exceeded, auto-ban for {} seconds", ban_seconds),
                    ban_seconds,
                ).await {
                    tracing::warn!("Failed to add IP to blacklist: {} - {}", ip_str, e);
                } else {
                    send_log_with_app(&app_clone, format!(
                        "[Rate Limit] IP {} was banned for {} seconds due to rate limit exceeded",
                        ip_str, ban_seconds
                    ));
                }
            });
        }
    }

    let status = StatusCode::TOO_MANY_REQUESTS;
    push_log_lazy(&state.app, || format_access_log(node, ctx, status));
    enqueue_request_log(node, ctx, remote, status, "", matched_route_id);
    Some((status, "Rate limit exceeded").into_response())
}

pub fn handle_basic_auth_failure(
    state: &AppState,
    ctx: &RequestContext,
    req_headers: &HeaderMap,
    remote: &SocketAddr,
    matched_route_id: &str,
    basic_auth_ok: bool,
) -> Option<Response> {
    if basic_auth_ok {
        return None;
    }

    let node = &*state.listen_addr;
    let status = StatusCode::UNAUTHORIZED;
    push_log_lazy(&state.app, || format_access_log(node, ctx, status));

    let inbound_headers_line = format_headers_for_log(req_headers);
    send_log_with_app(&state.app, format!(
        "Reverse proxy error (IN): {} {} -> [Basic Auth Failed] status={} | inbound_headers=[{}]",
        ctx.method.as_str(),
        ctx.uri,
        status.as_u16(),
        inbound_headers_line
    ));

    enqueue_request_log(node, ctx, remote, status, "", matched_route_id);
    Some(unauthorized_response())
}

pub fn handle_missing_route(
    state: &AppState,
    ctx: &RequestContext,
    remote: &SocketAddr,
    matched_route_id: &str,
) -> Response {
    let node = &*state.listen_addr;
    let status = StatusCode::NOT_FOUND;
    push_log_lazy(&state.app, || format_access_log(node, ctx, status));
    enqueue_request_log(node, ctx, remote, status, "", matched_route_id);
    (status, "No route").into_response()
}
