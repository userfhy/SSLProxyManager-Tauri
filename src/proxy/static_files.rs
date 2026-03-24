use axum::body::Body;
use axum::http::{HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use std::net::SocketAddr;
use tower::util::ServiceExt;

use super::AppState;
use super::context::{enqueue_request_log, format_access_log, RequestContext};
use super::helpers::{cached_index_html, cached_serve_dir, check_etag_match, get_or_create_etag, is_asset_path};
use super::logging::push_log_lazy;

pub async fn serve_static_owned(
    state: &AppState,
    ctx: &RequestContext,
    remote: &SocketAddr,
    matched_route_id: &str,
    dir: &str,
    req: axum::http::Request<Body>,
) -> Response {
    let node = &*state.listen_addr;
    let serve_dir = cached_serve_dir(dir);
    let request_etag: Option<String> = req
        .headers()
        .get("if-none-match")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let response = match serve_dir.oneshot(req).await {
        Ok(r) => r,
        Err(never) => match never {},
    };
    let status = response.status();
    let response = response.map(Body::new);

    if status.is_success() || status.is_redirection() {
        let full_path = std::path::Path::new(dir);
        if let Some(etag) = get_or_create_etag(full_path) {
            if check_etag_match(request_etag.as_deref(), &etag) {
                let status = StatusCode::NOT_MODIFIED;
                push_log_lazy(&state.app, || format_access_log(node, ctx, status));
                enqueue_request_log(node, ctx, remote, status, "", matched_route_id);
                return status.into_response();
            }

            let mut resp = response.into_response();
            resp.headers_mut().insert(
                axum::http::header::ETAG,
                HeaderValue::from_str(&etag).unwrap_or_else(|_| HeaderValue::from_static("")),
            );

            push_log_lazy(&state.app, || format_access_log(node, ctx, status));
            enqueue_request_log(node, ctx, remote, status, "", matched_route_id);
            return resp;
        }

        push_log_lazy(&state.app, || format_access_log(node, ctx, status));
        enqueue_request_log(node, ctx, remote, status, "", matched_route_id);
        return response;
    }

    if status == StatusCode::NOT_FOUND
        && (ctx.method == Method::GET || ctx.method == Method::HEAD)
        && !is_asset_path(&ctx.path)
    {
        if let Some(bytes) = cached_index_html(dir).await {
            let mut resp = Response::new(Body::from(bytes.clone()));
            resp.headers_mut().insert(
                axum::http::header::CONTENT_TYPE,
                HeaderValue::from_static("text/html; charset=utf-8"),
            );

            let etag = format!("\"spa-{:x}\"", bytes.len());
            if check_etag_match(request_etag.as_deref(), &etag) {
                let status = StatusCode::NOT_MODIFIED;
                push_log_lazy(&state.app, || format_access_log(node, ctx, status));
                enqueue_request_log(node, ctx, remote, status, "", matched_route_id);
                return status.into_response();
            }
            resp.headers_mut().insert(
                axum::http::header::ETAG,
                HeaderValue::from_str(&etag).unwrap_or_else(|_| HeaderValue::from_static("")),
            );

            let status = StatusCode::OK;
            push_log_lazy(&state.app, || format_access_log(node, ctx, status));
            enqueue_request_log(node, ctx, remote, status, "", matched_route_id);
            return resp;
        }
    }

    response
}
