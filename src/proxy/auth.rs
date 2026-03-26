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

    let Ok(decoded) = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64)
    else {
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

#[cfg(test)]
mod tests {
    use super::{is_basic_auth_ok, unauthorized_response};
    use crate::config::{ListenRule, Route, Upstream};
    use axum::http::{HeaderMap, HeaderValue, StatusCode};
    use base64::Engine;

    fn sample_rule() -> ListenRule {
        ListenRule {
            id: Some("rule".into()),
            enabled: true,
            listen_addr: "127.0.0.1:8080".into(),
            listen_addrs: vec![],
            ssl_enable: false,
            cert_file: String::new(),
            key_file: String::new(),
            basic_auth_enable: true,
            basic_auth_username: "admin".into(),
            basic_auth_password: "secret".into(),
            basic_auth_forward_header: false,
            routes: vec![],
            rate_limit_enabled: None,
            rate_limit_requests_per_second: None,
            rate_limit_burst_size: None,
            rate_limit_window_seconds: None,
            rate_limit_ban_seconds: None,
        }
    }

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

    fn auth_headers(userpass: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        let encoded = base64::engine::general_purpose::STANDARD.encode(userpass);
        headers.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Basic {encoded}")).unwrap(),
        );
        headers
    }

    #[test]
    fn basic_auth_accepts_valid_credentials() {
        let rule = sample_rule();
        let headers = auth_headers("admin:secret");
        assert!(is_basic_auth_ok(&rule, None, &headers));
    }

    #[test]
    fn basic_auth_rejects_invalid_credentials() {
        let rule = sample_rule();
        let headers = auth_headers("admin:wrong");
        assert!(!is_basic_auth_ok(&rule, None, &headers));
    }

    #[test]
    fn basic_auth_bypasses_when_route_excludes_auth() {
        let rule = sample_rule();
        let route = Route {
            exclude_basic_auth: Some(true),
            ..sample_route()
        };
        assert!(is_basic_auth_ok(&rule, Some(&route), &HeaderMap::new()));
    }

    #[test]
    fn basic_auth_bypasses_when_rule_disabled() {
        let mut rule = sample_rule();
        rule.basic_auth_enable = false;
        assert!(is_basic_auth_ok(&rule, None, &HeaderMap::new()));
    }

    #[test]
    fn unauthorized_response_sets_status_and_challenge_header() {
        let resp = unauthorized_response();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            resp.headers()
                .get(axum::http::header::WWW_AUTHENTICATE)
                .unwrap(),
            "Basic realm=\"SSLProxyManager\""
        );
    }
}
