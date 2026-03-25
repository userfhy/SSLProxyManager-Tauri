use crate::config;
use super::{cached_regex, send_log_with_app, AppState};
use super::context::{enqueue_request_log, format_access_log, format_headers_for_log, RequestContext};
use super::helpers::{content_type_allowed, is_hop_header_fast};
use super::logging::push_log_lazy;
use axum::body::Bytes;
use axum::{
    body::Body,
    http::{HeaderName, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use std::net::SocketAddr;

pub(crate) struct ProxyResponseMeta<'a> {
    pub target: &'a str,
    pub req_body_size: Option<usize>,
    pub outbound_headers_snapshot: &'a HeaderMap,
    pub inbound_headers: &'a HeaderMap,
    pub matched_route_id: &'a str,
    pub remote: &'a SocketAddr,
}

pub async fn handle_upstream_response(
    state: &AppState,
    route: &config::Route,
    ctx: &RequestContext,
    resp: reqwest::Response,
    meta: ProxyResponseMeta<'_>,
) -> Response {
    let node = &*state.listen_addr;
    let status = resp.status();
    let response_headers = resp.headers().clone();

    push_log_lazy(&state.app, || {
        format_access_log(
            node,
            ctx,
            StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY),
        )
    });

    enqueue_request_log(node, ctx, meta.remote, status, meta.target, meta.matched_route_id);

    let mut out = Response::new(Body::empty());
    *out.status_mut() = StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);

    for (k, v) in response_headers.iter() {
        if is_hop_header_fast(k.as_str()) {
            continue;
        }
        out.headers_mut().insert(k.clone(), v.clone());
    }

    if let Some(headers_to_remove) = route.remove_headers.as_ref() {
        for header_name in headers_to_remove.iter() {
            let trimmed: &str = header_name.trim();
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

        let final_bytes = apply_response_body_replace(route, &response_headers, bytes);
        *out.body_mut() = Body::from(final_bytes);
    }

    if !status.is_success() {
        let inbound_headers_line = format_headers_for_log(meta.inbound_headers);
        let outbound_headers_line = format_headers_for_log(meta.outbound_headers_snapshot);

        send_log_with_app(&state.app, format!(
            "Reverse proxy error (IN): {} {} -> {} status={} | inbound_headers=[{}]",
            ctx.method.as_str(),
            ctx.uri,
            meta.target,
            status.as_u16(),
            inbound_headers_line
        ));

        send_log_with_app(&state.app, format!(
            "Reverse proxy error (OUT): {} {} -> {} status={} | outbound_headers=[{}] | req_body_size={}",
            ctx.method.as_str(),
            ctx.uri,
            meta.target,
            status.as_u16(),
            outbound_headers_line,
            meta.req_body_size
                .map(|n| n.to_string())
                .unwrap_or_else(|| "stream".to_string())
        ));
    }

    out
}

fn apply_response_body_replace(
    route: &config::Route,
    response_headers: &HeaderMap,
    bytes: Bytes,
) -> Bytes {
    let Some(rules) = route.response_body_replace.as_ref() else {
        return bytes;
    };

    match std::str::from_utf8(&bytes) {
        Ok(body_str) => {
            let mut modified_body: Option<String> = None;

            for rule in rules {
                if !rule.enabled {
                    continue;
                }

                if let Some(content_types) = rule.content_types.as_ref() {
                    if !content_type_allowed(response_headers, content_types) {
                        continue;
                    }
                }

                let source = modified_body.as_deref().unwrap_or(body_str);

                if rule.use_regex {
                    if let Some(re) = rule
                        .compiled_regex
                        .as_ref()
                        .cloned()
                        .or_else(|| cached_regex(&rule.find))
                    {
                        if let std::borrow::Cow::Owned(new_body) =
                            re.replace_all(source, &rule.replace)
                        {
                            modified_body = Some(new_body);
                        }
                    }
                } else if source.contains(&rule.find) {
                    modified_body = Some(source.replace(&rule.find, &rule.replace));
                }
            }

            if let Some(body) = modified_body {
                Bytes::from(body.into_bytes())
            } else {
                bytes
            }
        }
        Err(_) => bytes,
    }
}
