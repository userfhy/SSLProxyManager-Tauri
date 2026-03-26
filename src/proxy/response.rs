use super::context::{
    enqueue_request_log, format_access_log, format_headers_for_log, RequestContext,
};
use super::helpers::{content_type_allowed, is_hop_header_fast};
use super::logging::push_log_lazy;
use super::{cached_regex, send_log_with_app, AppState};
use crate::config;
use axum::body::Bytes;
use axum::{
    body::Body,
    http::{HeaderMap, HeaderName, StatusCode},
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
    pub guard_ms: f64,
    pub prepare_ms: f64,
    pub upstream_ms: f64,
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

    enqueue_request_log(
        node,
        ctx,
        meta.remote,
        status,
        meta.target,
        meta.matched_route_id,
        meta.guard_ms,
        meta.prepare_ms,
        meta.upstream_ms,
    );

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
        let total_ms = ctx.elapsed_ms();

        send_log_with_app(
            &state.app,
            format!(
                "Reverse proxy error (IN): {} {} -> {} status={} | inbound_headers=[{}] | phase_ms[guard={:.2},prepare={:.2},upstream={:.2},total={:.2}]",
                ctx.method.as_str(),
                ctx.uri,
                meta.target,
                status.as_u16(),
                inbound_headers_line,
                meta.guard_ms,
                meta.prepare_ms,
                meta.upstream_ms,
                total_ms,
            ),
        );

        send_log_with_app(&state.app, format!(
            "Reverse proxy error (OUT): {} {} -> {} status={} | outbound_headers=[{}] | req_body_size={} | phase_ms[guard={:.2},prepare={:.2},upstream={:.2},total={:.2}]",
            ctx.method.as_str(),
            ctx.uri,
            meta.target,
            status.as_u16(),
            outbound_headers_line,
            meta.req_body_size
                .map(|n| n.to_string())
                .unwrap_or_else(|| "stream".to_string()),
            meta.guard_ms,
            meta.prepare_ms,
            meta.upstream_ms,
            total_ms,
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

#[cfg(test)]
mod tests {
    use super::apply_response_body_replace;
    use crate::config::{BodyReplaceRule, Route, Upstream};
    use axum::body::Bytes;
    use axum::http::{HeaderMap, HeaderValue};

    fn sample_route() -> Route {
        Route {
            id: Some("route".into()),
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
            url_rewrite_rules: None,
            request_body_replace: None,
            response_body_replace: None,
            remove_headers: None,
            methods: None,
            headers: None,
            upstreams: vec![Upstream {
                url: "http://backend".into(),
                weight: 1,
            }],
        }
    }

    #[test]
    fn apply_response_body_replace_applies_plain_and_regex_rules_in_order() {
        let mut route = sample_route();
        route.response_body_replace = Some(vec![
            BodyReplaceRule {
                find: "world".into(),
                replace: "team".into(),
                use_regex: false,
                enabled: true,
                content_types: None,
                compiled_regex: None,
            },
            BodyReplaceRule {
                find: "hello (team)".into(),
                replace: "hi $1".into(),
                use_regex: true,
                enabled: true,
                content_types: None,
                compiled_regex: None,
            },
        ]);

        let headers = HeaderMap::new();
        let body = Bytes::from("hello world");

        let out = apply_response_body_replace(&route, &headers, body);
        assert_eq!(out, Bytes::from("hi team"));
    }

    #[test]
    fn apply_response_body_replace_respects_content_type_filter() {
        let mut route = sample_route();
        route.response_body_replace = Some(vec![BodyReplaceRule {
            find: "secret".into(),
            replace: "public".into(),
            use_regex: false,
            enabled: true,
            content_types: Some("application/json".into()),
            compiled_regex: None,
        }]);

        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        );

        let out = apply_response_body_replace(&route, &headers, Bytes::from("secret"));
        assert_eq!(out, Bytes::from("secret"));
    }

    #[test]
    fn apply_response_body_replace_ignores_invalid_utf8_body() {
        let mut route = sample_route();
        route.response_body_replace = Some(vec![BodyReplaceRule {
            find: "a".into(),
            replace: "b".into(),
            use_regex: false,
            enabled: true,
            content_types: None,
            compiled_regex: None,
        }]);

        let raw = Bytes::from_static(&[0xff, 0xfe, 0xfd]);
        let out = apply_response_body_replace(&route, &HeaderMap::new(), raw.clone());
        assert_eq!(out, raw);
    }
}
