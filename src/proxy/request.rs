use anyhow::Result;
use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Request, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use regex::Regex;
use std::net::SocketAddr;

use super::{upstream::build_upstream_url, AppState};
use super::context::{enqueue_request_log, format_access_log, RequestContext};
use super::helpers::{cached_regex, content_type_allowed, expand_proxy_header_value, is_hop_header_fast};
use super::logging::{push_log_lazy, SKIP_HEADERS};

pub(crate) struct PreparedProxyRequest {
    pub target: String,
    pub req_body_size: Option<usize>,
    pub outbound_headers_snapshot: HeaderMap,
    pub upstream_req: reqwest::Request,
}

pub fn rewrite_uri(route: &crate::config::Route, uri: &Uri) -> Uri {
    let mut final_uri = uri.clone();
    if let Some(rules) = route.url_rewrite_rules.as_ref() {
        for rule in rules {
            if !rule.enabled {
                continue;
            }
            if let Some(re) = cached_regex(&rule.pattern) {
                let re: &Regex = &re;
                let original = final_uri.to_string();
                let rewritten = re.replace_all(&original, &rule.replacement);
                if rewritten != original {
                    if let Ok(new_uri) = rewritten.parse::<Uri>() {
                        final_uri = new_uri;
                    }
                }
            }
        }
    }
    final_uri
}

pub fn select_upstream_url(state: &AppState, upstream_url: &str) -> String {
    if upstream_url.contains("$server_port") {
        upstream_url.replace("$server_port", &state.server_port.to_string())
    } else {
        upstream_url.to_string()
    }
}

pub async fn prepare_proxy_request(
    state: &AppState,
    route: &crate::config::Route,
    ctx: &RequestContext,
    remote: &SocketAddr,
    matched_route_id: &str,
    req: Request<Body>,
) -> Result<PreparedProxyRequest, Response> {
    let node = &*state.listen_addr;
    let has_enabled_response_body_replace = route
        .response_body_replace
        .as_ref()
        .map(|rules| rules.iter().any(|r| r.enabled))
        .unwrap_or(false);

    let mut upstream_url = super::upstream::pick_upstream_smooth(route)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "No static directory or upstream configured").into_response())?;

    let final_uri = rewrite_uri(route, &ctx.uri);
    upstream_url = select_upstream_url(state, &upstream_url);

    let target: String = match build_upstream_url(
        &upstream_url,
        route.path.as_deref(),
        route.proxy_pass_path.as_deref(),
        &final_uri,
    ) {
        Ok(u) => u,
        Err(e) => {
            let status = StatusCode::BAD_GATEWAY;
            push_log_lazy(&state.app, || format_access_log(node, ctx, status));
            enqueue_request_log(node, ctx, remote, status, &upstream_url, matched_route_id);
            return Err((status, format!("bad upstream url: {e}")).into_response());
        }
    };

    let client = if route.follow_redirects {
        state.client_follow.clone()
    } else {
        state.client_nofollow.clone()
    };

    let (req_parts, req_body_axum) = req.into_parts();
    let inbound_headers = req_parts.headers;
    let method_up = req_parts.method;

    let (reqwest_body, req_body_size) = if state.stream_proxy {
        let body_stream = req_body_axum.into_data_stream();
        (reqwest::Body::wrap_stream(body_stream), None)
    } else {
        let bytes = match axum::body::to_bytes(req_body_axum, state.max_body_size).await {
            Ok(b) => b,
            Err(e) => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    format!("read request body failed: {e}"),
                )
                    .into_response());
            }
        };

        let final_bytes = if let Some(rules) = route.request_body_replace.as_ref() {
            match std::str::from_utf8(&bytes) {
                Ok(body_str) => {
                    let mut modified_body: Option<String> = None;

                    for rule in rules {
                        if !rule.enabled {
                            continue;
                        }
                        if let Some(ref content_types) = rule.content_types {
                            if !content_type_allowed(&inbound_headers, content_types) {
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
        } else {
            bytes
        };

        let len = final_bytes.len();
        (reqwest::Body::from(final_bytes), Some(len))
    };

    let mut final_headers = HeaderMap::with_capacity(inbound_headers.len() + 8);
    for (k, v) in inbound_headers.iter() {
        if SKIP_HEADERS.contains(k) || is_hop_header_fast(k.as_str()) {
            continue;
        }
        final_headers.append(k.clone(), v.clone());
    }

    if let Some(h) = inbound_headers.get(axum::http::header::HOST) {
        final_headers.insert(axum::http::header::HOST, h.clone());
    }

    {
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
    }

    final_headers.insert(
        HeaderName::from_static("x-forwarded-proto"),
        HeaderValue::from_static(if state.rule.ssl_enable { "https" } else { "http" }),
    );

    if has_enabled_response_body_replace {
        final_headers.insert(
            axum::http::header::ACCEPT_ENCODING,
            HeaderValue::from_static("identity"),
        );
    } else if let Some(v) = inbound_headers.get(axum::http::header::ACCEPT_ENCODING) {
        final_headers.insert(axum::http::header::ACCEPT_ENCODING, v.clone());
    }

    if !final_headers.contains_key(axum::http::header::CONTENT_TYPE) {
        if let Some(ct) = inbound_headers.get(axum::http::header::CONTENT_TYPE) {
            final_headers.insert(axum::http::header::CONTENT_TYPE, ct.clone());
        }
    }

    if let Some(map) = route.set_headers.as_ref() {
        for (k, v) in map {
            let key = k.trim();
            if key.is_empty() || is_hop_header_fast(key) {
                continue;
            }

            let expanded =
                expand_proxy_header_value(v, remote, &inbound_headers, state.rule.ssl_enable);

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
        final_headers.remove(axum::http::header::AUTHORIZATION);
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

    let mut builder = client.request(method_up, target.clone());
    builder = builder.body(reqwest_body);

    let mut upstream_req = match builder.build() {
        Ok(r) => r,
        Err(e) => {
            return Err((
                StatusCode::BAD_GATEWAY,
                format!("build upstream request failed: {e}"),
            )
                .into_response());
        }
    };

    upstream_req.headers_mut().clear();
    upstream_req.headers_mut().extend(final_headers);
    let outbound_headers_snapshot = upstream_req.headers().clone();

    Ok(PreparedProxyRequest {
        target,
        req_body_size,
        outbound_headers_snapshot,
        upstream_req,
    })
}
