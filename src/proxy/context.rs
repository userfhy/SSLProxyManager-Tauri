use axum::http::{HeaderMap, Method, StatusCode, Uri};
use std::net::SocketAddr;
use std::sync::Arc;

use crate::{access_control, metrics};

pub(crate) struct RequestContext {
    pub client_ip: Arc<str>,
    started_at: std::time::Instant,
    pub client_ip_header: Arc<str>,
    pub real_ip_header: Arc<str>,
    pub host_header: Arc<str>,
    pub referer_header: Arc<str>,
    pub user_agent_header: Arc<str>,
    pub method: Method,
    pub uri: Uri,
    pub path: Arc<str>,
}

impl RequestContext {
    pub fn new(remote: SocketAddr, headers: &HeaderMap, method: &Method, uri: &Uri) -> Self {
        let path = uri.path();

        #[inline]
        fn header_to_arc_str(headers: &HeaderMap, key: &'static str) -> Arc<str> {
            headers
                .get(key)
                .and_then(|v| v.to_str().ok())
                .map(|s| Arc::from(s.to_string()))
                .unwrap_or_else(|| Arc::from("-".to_string()))
        }

        let xff = header_to_arc_str(headers, "x-forwarded-for");
        let xri = header_to_arc_str(headers, "x-real-ip");

        let host = headers
            .get("host")
            .or_else(|| headers.get(":authority"))
            .and_then(|v| v.to_str().ok())
            .or_else(|| uri.authority().map(|a| a.as_str()))
            .unwrap_or("")
            .to_string();

        let referer = header_to_arc_str(headers, "referer");
        let ua = header_to_arc_str(headers, "user-agent");

        Self {
            client_ip: Arc::from(access_control::client_ip_from_headers(&remote, headers)),
            started_at: std::time::Instant::now(),
            client_ip_header: xff,
            real_ip_header: xri,
            host_header: Arc::from(host),
            referer_header: referer,
            user_agent_header: ua,
            method: method.clone(),
            uri: uri.clone(),
            path: Arc::from(path),
        }
    }

    #[inline]
    pub fn elapsed_ms(&self) -> f64 {
        self.started_at.elapsed().as_secs_f64() * 1000.0
    }

    #[inline]
    pub fn elapsed_s(&self) -> f64 {
        self.started_at.elapsed().as_secs_f64()
    }
}

#[inline]
pub fn time_local_string() -> String {
    let now = chrono::Local::now();
    now.format("%y.%m.%d %H:%M:%S").to_string()
}

#[inline]
pub fn request_line(method: &Method, uri: &Uri) -> String {
    format!("{} {} HTTP/1.1", method.as_str(), uri)
}

#[inline]
pub fn format_access_log(node: &str, ctx: &RequestContext, status: StatusCode) -> String {
    let ip: &str = if !ctx.client_ip.is_empty() {
        &ctx.client_ip
    } else if !ctx.client_ip_header.is_empty() && ctx.client_ip_header.as_ref() != "-" {
        ctx.client_ip_header.split(',').next().unwrap_or("-").trim()
    } else if !ctx.real_ip_header.is_empty() && ctx.real_ip_header.as_ref() != "-" {
        &ctx.real_ip_header
    } else {
        "-"
    };
    let time_local = time_local_string();
    let req_line = request_line(&ctx.method, &ctx.uri);

    format!(
        "[NODE {}] [-] {} - - [{}] \"{}\" {} - \"{}\" \"{}\" {:.3}s",
        node,
        ip,
        time_local,
        req_line,
        status.as_u16(),
        ctx.referer_header,
        ctx.user_agent_header,
        ctx.elapsed_s()
    )
}

#[inline]
pub fn enqueue_request_log(
    node: &str,
    ctx: &RequestContext,
    remote: &SocketAddr,
    status: StatusCode,
    upstream: &str,
    matched_route_id: &str,
) {
    metrics::try_enqueue_request_log(metrics::RequestLogInsert {
        timestamp: chrono::Utc::now().timestamp(),
        listen_addr: node.to_string(),
        client_ip: ctx.client_ip.as_ref().to_string(),
        remote_ip: remote.ip().to_string(),
        method: ctx.method.as_str().to_string(),
        request_path: ctx.path.as_ref().to_string(),
        request_host: ctx.host_header.as_ref().to_string(),
        status_code: status.as_u16() as i32,
        upstream: upstream.to_string(),
        latency_ms: ctx.elapsed_ms(),
        user_agent: ctx.user_agent_header.as_ref().to_string(),
        referer: ctx.referer_header.as_ref().to_string(),
        matched_route_id: matched_route_id.to_string(),
    });
}

#[inline]
pub fn format_headers_for_log(headers: &HeaderMap) -> String {
    use std::fmt::Write;

    let mut buf = crate::buffer_pool::acquire_buffer();
    for (k, v) in headers.iter() {
        if !buf.is_empty() {
            let _ = write!(buf, " ## ");
        }
        let _ = write!(buf, "{}: {}", k, v.to_str().unwrap_or("[invalid utf8]"));
    }
    std::str::from_utf8(&buf).unwrap_or("").to_string()
}
