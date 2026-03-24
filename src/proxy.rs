pub use crate::proxy_auth::healthz;
pub use crate::proxy_helpers::{cached_content_types, cached_regex};
pub use crate::proxy_listen::parse_listen_addr;
pub use crate::proxy_logging::{clear_logs, get_logs, send_log_with_app};
pub use crate::proxy_runtime::{is_effectively_running, is_running, start_server, stop_server};
use crate::proxy_auth::{is_basic_auth_ok, unauthorized_response};
use crate::proxy_context::{enqueue_request_log, format_access_log, format_headers_for_log, RequestContext};
use crate::proxy_helpers::{cached_index_html, cached_serve_dir, check_etag_match, content_type_allowed, expand_proxy_header_value, get_or_create_etag, is_asset_path, is_hop_header_fast};
use crate::proxy_logging::{push_log_lazy, SKIP_HEADERS};
use crate::proxy_matching::match_route;
use crate::proxy_upstream::pick_upstream_smooth;
use crate::{access_control, cache_optimizer, config, metrics, rate_limit};
use anyhow::{anyhow, Result};
use axum::body::Bytes;
use axum::{
    body::Body,
    extract::{connect_info::ConnectInfo, State},
    http::{HeaderMap, HeaderName, HeaderValue, Method, Request, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use std::{net::SocketAddr, sync::Arc};
use tower::util::ServiceExt;
use tracing::info;

#[allow(dead_code)]
async fn resolve_hostname_with_cache(hostname: &str) -> Result<Vec<std::net::IpAddr>> {
    use std::net::IpAddr;

    let dns_cache = cache_optimizer::global_cache_manager().dns_cache();

    // 先查缓存
    if let Some(ips) = dns_cache.get(hostname) {
        tracing::debug!("DNS cache hit: {}", hostname);
        return Ok(ips);
    }

    // 缓存未命中，执行 DNS 查询
    tracing::debug!("DNS cache miss, resolving: {}", hostname);
    let ips: Vec<IpAddr> = tokio::net::lookup_host(format!("{}:0", hostname))
        .await
        .map_err(|e| anyhow!("DNS lookup failed: {}", e))?
        .map(|addr| addr.ip())
        .collect();

    // 存入缓存
    if !ips.is_empty() {
        dns_cache.put(hostname.to_string(), ips.clone());
    }

    Ok(ips)
}

// 优化后的 AppState：缓存常用配置，减少热路径上的配置克隆
#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) rule: config::ListenRule,
    pub(crate) client_follow: reqwest::Client,
    pub(crate) client_nofollow: reqwest::Client,
    pub(crate) app: tauri::AppHandle,
    // 缓存配置字段，避免每次请求都克隆整个 Config
    pub(crate) listen_addr: Arc<str>,
    pub(crate) server_port: u16,
    pub(crate) stream_proxy: bool,
    pub(crate) max_body_size: usize,
    pub(crate) max_response_body_size: usize,
    pub(crate) http_access_control_enabled: bool,
    // 访问控制所需配置快照：避免每请求 get_config() clone 整个 Config
    pub(crate) allow_all_lan: bool,
    pub(crate) allow_all_ip: bool,
    pub(crate) whitelist: Arc<[config::WhitelistEntry]>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RuleStartErrorPayload {
    pub listen_addr: String,
    pub error: String,
}

// 请求上下文：统一管理请求相关数据，减少参数传递
// 使用 Arc<str> 优化频繁 clone 的字段，避免不必要的内存分配
/// 解析监听地址，返回主地址和是否需要同时绑定 IPv4/IPv6
#[inline]
pub(crate) async fn proxy_handler(
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
    req: Request<Body>,
) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let ctx = RequestContext::new(remote, req.headers(), &method, &uri);

    let node = &*state.listen_addr;
    let (route, matched_route_id) = match_route(
        &state.rule.routes,
        &ctx.host_header,
        &ctx.path,
        &ctx.method,
        req.headers()
    );

    // 0. 访问控制
    if state.http_access_control_enabled {
        if metrics::is_ip_blacklisted(&ctx.client_ip) {
            let status = StatusCode::FORBIDDEN;
            push_log_lazy(&state.app, || format_access_log(node, &ctx, status));

            let inbound_headers_line = format_headers_for_log(req.headers());

            send_log_with_app(&state.app, format!(
                "Reverse proxy error (IN): {} {} -> [IP Blacklist] status={} | inbound_headers=[{}]",
                ctx.method.as_str(),
                ctx.uri,
                status.as_u16(),
                inbound_headers_line
            ));

            enqueue_request_log(node, &ctx, &remote, status, "", &matched_route_id);

            return (status, "IP Forbidden").into_response();
        }

        let allowed = access_control::is_allowed_remote_ip(
            &remote,
            state.allow_all_lan,
            state.allow_all_ip,
            &state.whitelist,
        );
        
        if !allowed {
            let status = StatusCode::FORBIDDEN;
            let debug_msg = format!(
                "Access denied: client_ip={}, remote_ip={}, allow_all_lan={}, whitelist_len={}",
                ctx.client_ip,
                remote.ip(),
                state.allow_all_lan,
                state.whitelist.len()
            );
            info!("{}", debug_msg);
            push_log_lazy(&state.app, || format_access_log(node, &ctx, status));

            let inbound_headers_line = format_headers_for_log(req.headers());

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

            enqueue_request_log(node, &ctx, &remote, status, "", &matched_route_id);

            return (status, "Forbidden").into_response();
        }
    }

    // 0.5. 速率限制检查（如果在该规则中启用）
    if state.rule.rate_limit_enabled.unwrap_or(false) {
        if let Some(limiter) = rate_limit::RATE_LIMITERS.get(node) {
            let (allowed, should_ban) = limiter.read().check(&ctx.client_ip);
            
            if !allowed {
                // 如果需要封禁，添加到黑名单
                if should_ban {
                    let ban_seconds = state.rule.rate_limit_ban_seconds.unwrap_or(0) as i32;
                    if ban_seconds > 0 {
                        let ip_str: String = ctx.client_ip.as_ref().into();
                        let app_clone = state.app.clone();
                        // 异步添加到黑名单，不阻塞请求处理
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
                push_log_lazy(&state.app, || format_access_log(node, &ctx, status));

                enqueue_request_log(node, &ctx, &remote, status, "", &matched_route_id);

                return (status, "Rate limit exceeded").into_response();
            }
        }
    }

    // 1. 检查 Basic Auth
    if !is_basic_auth_ok(&state.rule, route, req.headers()) {
        let status = StatusCode::UNAUTHORIZED;
        push_log_lazy(&state.app, || format_access_log(node, &ctx, status));

        let inbound_headers_line = format_headers_for_log(req.headers());

        send_log_with_app(&state.app, format!(
            "Reverse proxy error (IN): {} {} -> [Basic Auth Failed] status={} | inbound_headers=[{}]",
            ctx.method.as_str(),
            ctx.uri,
            status.as_u16(),
            inbound_headers_line
        ));

        enqueue_request_log(node, &ctx, &remote, status, "", &matched_route_id);

        return unauthorized_response();
    }

    let Some(route) = route else {
        let status = StatusCode::NOT_FOUND;
        push_log_lazy(&state.app, || format_access_log(node, &ctx, status));

        enqueue_request_log(node, &ctx, &remote, status, "", &matched_route_id);

        return (status, "No route").into_response();
    };

    // 2. 优先处理静态资源
    if let Some(dir) = route.static_dir.as_ref() {
        let serve_dir = cached_serve_dir(dir);
        // 先提取 If-None-Match header 并转换为 owned String，避免 req 被 move 后无法访问
        let request_etag: Option<String> = req.headers()
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
            // 获取文件路径以生成 ETag
            let full_path = std::path::Path::new(dir);
            if let Some(etag) = get_or_create_etag(full_path) {
                // 检查是否命中缓存（304 Not Modified）
                if check_etag_match(request_etag.as_deref(), &etag) {
                    push_log_lazy(&state.app, || format_access_log(node, &ctx, StatusCode::NOT_MODIFIED));
                    enqueue_request_log(node, &ctx, &remote, StatusCode::NOT_MODIFIED, "", &matched_route_id);
                    return (StatusCode::NOT_MODIFIED).into_response();
                }
                
                // 添加 ETag 头
                let mut resp = response.into_response();
                resp.headers_mut().insert(
                    axum::http::header::ETAG,
                    HeaderValue::from_str(&etag).unwrap_or_else(|_| HeaderValue::from_static("")),
                );
                
                push_log_lazy(&state.app, || format_access_log(node, &ctx, status));
                enqueue_request_log(node, &ctx, &remote, status, "", &matched_route_id);
                return resp;
            }
            
            push_log_lazy(&state.app, || format_access_log(node, &ctx, status));
            enqueue_request_log(node, &ctx, &remote, status, "", &matched_route_id);
            return response;
        }

        // SPA 回退
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
                
                // SPA 也要支持 ETag
                let etag = format!("\"spa-{:x}\"", bytes.len());
                if check_etag_match(request_etag.as_deref(), &etag) {
                    let status = StatusCode::NOT_MODIFIED;
                    push_log_lazy(&state.app, || format_access_log(node, &ctx, status));
                    enqueue_request_log(node, &ctx, &remote, status, "", &matched_route_id);
                    return (status).into_response();
                }
                resp.headers_mut().insert(
                    axum::http::header::ETAG,
                    HeaderValue::from_str(&etag).unwrap_or_else(|_| HeaderValue::from_static("")),
                );

                let status = StatusCode::OK;
                push_log_lazy(&state.app, || format_access_log(node, &ctx, status));
                enqueue_request_log(node, &ctx, &remote, status, "", &matched_route_id);

                return resp;
            }
        }

        return response;
    }

    // 3. 处理反代逻辑
    if let Some(mut upstream_url) = pick_upstream_smooth(route) {
        let has_enabled_response_body_replace = route
            .response_body_replace
            .as_ref()
            .map(|rules| rules.iter().any(|r| r.enabled))
            .unwrap_or(false);

        // 3.1 URL 重写（在构建目标URL之前）
        let mut final_uri = ctx.uri.clone();
        if let Some(rules) = route.url_rewrite_rules.as_ref() {
            for rule in rules {
                if !rule.enabled {
                    continue;
                }
                if let Some(re) = cached_regex(&rule.pattern) {
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

        // 支持在 upstream URL 中使用 $server_port 占位符（例如 http://192.168.1.121:$server_port）
        if upstream_url.contains("$server_port") {
            let port_str = state.server_port.to_string();
            upstream_url = upstream_url.replace("$server_port", &port_str);
        }

        let target = match build_upstream_url(
            &upstream_url,
            route.path.as_deref(),
            route.proxy_pass_path.as_deref(),
            &final_uri,
        ) {
            Ok(u) => u,
            Err(e) => {
                let status = StatusCode::BAD_GATEWAY;
                push_log_lazy(&state.app, || format_access_log(node, &ctx, status));

                enqueue_request_log(node, &ctx, &remote, status, &upstream_url, &matched_route_id);

                return (status, format!("bad upstream url: {e}")).into_response();
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

        // 读取请求体
        let (reqwest_body, req_body_size) = if state.stream_proxy {
            let body_stream = req_body_axum.into_data_stream();
            (reqwest::Body::wrap_stream(body_stream), None)
        } else {
            let bytes = match axum::body::to_bytes(req_body_axum, state.max_body_size).await {
                Ok(b) => b,
                Err(e) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        format!("read request body failed: {e}"),
                    )
                        .into_response();
                }
            };

            // 3.2 请求体修改（如果配置了替换规则）
            let final_bytes = if let Some(rules) = route.request_body_replace.as_ref() {
                // 优化：使用 from_utf8_lossy 避免失败时的 panic，并减少拷贝
                match std::str::from_utf8(&bytes) {
                    Ok(body_str) => {
                        let mut modified_body = body_str.to_string();
                        for rule in rules {
                            if !rule.enabled {
                                continue;
                            }

                            // 检查 Content-Type 过滤
                            if let Some(ref content_types) = rule.content_types {
                                if !content_type_allowed(&inbound_headers, content_types) {
                                    continue; // 不匹配，跳过此规则
                                }
                            }

                            // 执行替换
                            if rule.use_regex {
                                if let Some(re) = cached_regex(&rule.find) {
                                    modified_body = re.replace_all(&modified_body, &rule.replace).to_string();
                                }
                            } else {
                                modified_body = modified_body.replace(&rule.find, &rule.replace);
                            }
                        }
                        Bytes::from(modified_body.into_bytes())
                    }
                    Err(_) => bytes, // 非 UTF-8 内容，直接使用原始 bytes
                }
            } else {
                bytes
            };

            let len = final_bytes.len();
            (reqwest::Body::from(final_bytes), Some(len))
        };

        // 构造最终 headers（使用预计算的 SKIP_HEADERS）
        let mut final_headers = HeaderMap::with_capacity(inbound_headers.len() + 8);

        for (k, v) in inbound_headers.iter() {
            if SKIP_HEADERS.contains(k) || is_hop_header_fast(k.as_str()) {
                continue;
            }
            final_headers.append(k.clone(), v.clone());
        }

        // Host header
        if let Some(h) = inbound_headers.get(axum::http::header::HOST) {
            final_headers.insert(axum::http::header::HOST, h.clone());
        }

        // 转发头
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

        // 只有在需要做响应体替换时才强制 identity，避免拿到压缩体后无法替换；
        // 其他情况尽量透传客户端 Accept-Encoding，减少传输耗时。
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

        // set_headers
        if let Some(map) = route.set_headers.as_ref() {
            for (k, v) in map {
                let key = k.trim();
                if key.is_empty() || is_hop_header_fast(key) {
                    continue;
                }

                let expanded =
                    expand_proxy_header_value(v, &remote, &inbound_headers, state.rule.ssl_enable);

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

        // 移除 Authorization（如需要）
        if state.rule.basic_auth_enable && !state.rule.basic_auth_forward_header {
            final_headers.remove(axum::http::header::AUTHORIZATION);
        }

        // 3.3 移除指定的请求头
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

        // 构造上游请求
        let mut builder = client.request(method_up, target.clone());
        builder = builder.body(reqwest_body);

        let mut upstream_req = match builder.build() {
            Ok(r) => r,
            Err(e) => {
                return (
                    StatusCode::BAD_GATEWAY,
                    format!("build upstream request failed: {e}"),
                )
                    .into_response();
            }
        };

        upstream_req.headers_mut().clear();
        upstream_req.headers_mut().extend(final_headers);

        let outbound_headers_snapshot = upstream_req.headers().clone();

        // 发送请求
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

        let status = resp.status();
        let response_headers = resp.headers().clone(); // 提前 clone headers
        
        push_log_lazy(&state.app, || {
            format_access_log(
                node,
                &ctx,
                StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY),
            )
        });

        enqueue_request_log(node, &ctx, &remote, status, &target, &matched_route_id);

        let mut out = Response::new(Body::empty());
        *out.status_mut() = StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);

        for (k, v) in response_headers.iter() {
            if is_hop_header_fast(k.as_str()) {
                continue;
            }
            out.headers_mut().insert(k.clone(), v.clone());
        }

        // 3.4 移除指定的响应头
        if let Some(headers_to_remove) = route.remove_headers.as_ref() {
            for header_name in headers_to_remove {
                let trimmed = header_name.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if let Ok(name) = HeaderName::from_bytes(trimmed.as_bytes()) {
                    out.headers_mut().remove(name);
                }
            }
        }

        // 响应体处理
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

            // 3.5 响应体修改（如果配置了替换规则）
            let final_bytes = if let Some(rules) = route.response_body_replace.as_ref() {
                // 零拷贝 UTF-8 验证，避免 to_vec() 的内存分配
                match std::str::from_utf8(&bytes) {
                    Ok(body_str) => {
                        let mut modified_body = body_str.to_string();
                        for rule in rules {
                            if !rule.enabled {
                                continue;
                            }

                            // 检查 Content-Type 过滤
                            if let Some(ref content_types) = rule.content_types {
                                if !content_type_allowed(&response_headers, content_types) {
                                    continue; // 不匹配，跳过此规则
                                }
                            }

                            // 执行替换
                            if rule.use_regex {
                                if let Some(re) = cached_regex(&rule.find) {
                                    modified_body = re.replace_all(&modified_body, &rule.replace).to_string();
                                }
                            } else {
                                modified_body = modified_body.replace(&rule.find, &rule.replace);
                            }
                        }
                        Bytes::from(modified_body.into_bytes())
                    }
                    Err(_) => bytes, // 非 UTF-8 内容，直接使用原始字节
                }
            } else {
                bytes
            };

            *out.body_mut() = Body::from(final_bytes);
        }

        // 仅错误时记录详细日志
        if !status.is_success() {
            let inbound_headers_line = format_headers_for_log(&inbound_headers);
            let outbound_headers_line = format_headers_for_log(&outbound_headers_snapshot);

            send_log_with_app(&state.app, format!(
                "Reverse proxy error (IN): {} {} -> {} status={} | inbound_headers=[{}]",
                ctx.method.as_str(),
                ctx.uri,
                target,
                status.as_u16(),
                inbound_headers_line
            ));

            send_log_with_app(&state.app, format!(
                "Reverse proxy error (OUT): {} {} -> {} status={} | outbound_headers=[{}] | req_body_size={}",
                ctx.method.as_str(),
                ctx.uri,
                target,
                status.as_u16(),
                outbound_headers_line,
                req_body_size
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "stream".to_string())
            ));
        }

        return out;
    }

    (
        StatusCode::NOT_FOUND,
        "No static directory or upstream configured",
    )
        .into_response()
}

fn build_upstream_url(
    upstream_base: &str,
    route_path: Option<&str>,
    proxy_pass_path: Option<&str>,
    uri: &Uri,
) -> Result<String> {
    let mut base = upstream_base.trim_end_matches('/').to_string();

    let orig_path = uri.path();
    let route_path = route_path.unwrap_or("/");

    let mut new_path = orig_path.to_string();
    if let Some(pp) = proxy_pass_path {
        let from = if route_path.is_empty() { "/" } else { route_path };
        let to = if pp.trim().is_empty() { "/" } else { pp };

        if new_path.starts_with(from) {
            let suffix = &new_path[from.len()..];

            let mut out_path = to.to_string();
            if out_path.is_empty() {
                out_path = "/".to_string();
            }

            let suffix = suffix.strip_prefix('/').unwrap_or(suffix);
            if out_path.ends_with('/') {
                new_path = if suffix.is_empty() {
                    out_path
                } else {
                    format!("{}{}", out_path, suffix)
                };
            } else {
                new_path = if suffix.is_empty() {
                    out_path
                } else {
                    format!("{}/{}", out_path, suffix)
                };
            }
        }

        if !new_path.starts_with('/') {
            new_path = format!("/{}", new_path);
        }
    }

    base.push_str(&new_path);
    if let Some(q) = uri.query() {
        base.push('?');
        base.push_str(q);
    }
    Ok(base)
}

#[inline]
#[cfg(test)]
mod tests {
    use super::{host_matches, normalize_host};

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
