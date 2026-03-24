use axum::http::{HeaderMap, HeaderValue};
use axum::response::{IntoResponse, Response};
use axum::{body::Body, http::StatusCode};

use crate::config;

pub async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

#[inline]
pub fn is_basic_auth_ok(
    rule: &config::ListenRule,
    route: Option<&config::Route>,
    headers: &HeaderMap,
) -> bool {
    if let Some(r) = route {
        if r.exclude_basic_auth.unwrap_or(false) {
            return true;
        }
    }

    if !rule.basic_auth_enable {
        return true;
    }

    let Some(auth) = headers.get(axum::http::header::AUTHORIZATION) else {
        return false;
    };
    let Ok(auth) = auth.to_str() else {
        return false;
    };
    let Some(b64) = auth.strip_prefix("Basic ") else {
        return false;
    };

    let Ok(decoded) = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64) else {
        return false;
    };
    let Ok(s) = String::from_utf8(decoded) else {
        return false;
    };

    let expected = format!("{}:{}", rule.basic_auth_username, rule.basic_auth_password);
    s == expected
}

pub fn unauthorized_response() -> Response {
    let mut resp = Response::new(Body::from("Unauthorized"));
    *resp.status_mut() = StatusCode::UNAUTHORIZED;
    resp.headers_mut().insert(
        axum::http::header::WWW_AUTHENTICATE,
        HeaderValue::from_static("Basic realm=\"SSLProxyManager\""),
    );
    resp
}
