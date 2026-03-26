use axum::http::{HeaderMap, Method};

use crate::config;

#[inline]
pub fn normalize_host(host: &str) -> &str {
    let host = host.trim();
    if host.is_empty() {
        return host;
    }

    if let Some(rest) = host.strip_prefix('[') {
        if let Some(end) = rest.find(']') {
            return &rest[..end];
        }
        return host;
    }

    if let Some(idx) = host.rfind(':') {
        if !host[..idx].contains(':')
            && !host[idx + 1..].is_empty()
            && host[idx + 1..].bytes().all(|b| b.is_ascii_digit())
        {
            return &host[..idx];
        }
    }

    host
}

#[inline]
pub fn ends_with_ignore_ascii_case(s: &str, suffix: &str) -> bool {
    if s.len() < suffix.len() {
        return false;
    }
    s[s.len() - suffix.len()..].eq_ignore_ascii_case(suffix)
}

#[inline]
pub fn host_matches(route_host: &str, request_host: &str) -> bool {
    let route_host = normalize_host(route_host).trim_end_matches('.');
    let request_host = normalize_host(request_host).trim_end_matches('.');

    if request_host.is_empty() {
        return route_host.is_empty();
    }

    if route_host.eq_ignore_ascii_case(request_host) {
        return true;
    }

    if let Some(suffix) = route_host.strip_prefix("*.") {
        let suffix = suffix.trim_matches('.');
        if suffix.is_empty() {
            return false;
        }

        if request_host.eq_ignore_ascii_case(suffix) {
            return true;
        }

        if !ends_with_ignore_ascii_case(request_host, suffix) {
            return false;
        }

        let dot_idx = request_host.len().saturating_sub(suffix.len() + 1);
        return request_host
            .as_bytes()
            .get(dot_idx)
            .copied()
            .is_some_and(|b| b == b'.');
    }

    false
}

#[inline]
fn wildcard_match_ignore_ascii_case(pattern: &str, value: &str) -> bool {
    let pattern = pattern.trim();
    if pattern.is_empty() {
        return value.is_empty();
    }

    if !pattern.contains('*') {
        return value.eq_ignore_ascii_case(pattern);
    }

    let lower_value = value.to_ascii_lowercase();
    let mut from = 0usize;

    for part in pattern.split('*').filter(|p| !p.is_empty()) {
        let part = part.to_ascii_lowercase();
        if let Some(idx) = lower_value[from..].find(&part) {
            from += idx + part.len();
        } else {
            return false;
        }
    }

    true
}

#[inline]
pub fn match_route<'a>(
    routes: &'a [config::Route],
    request_host: &str,
    path: &str,
    method: &Method,
    headers: &HeaderMap,
) -> (Option<&'a config::Route>, String) {
    let host = normalize_host(request_host);

    let mut best: Option<(&config::Route, bool, usize)> = None;

    for r in routes {
        if !r.enabled {
            continue;
        }

        let p = match r.path.as_deref() {
            Some(v) => v,
            None => continue,
        };
        if !path.starts_with(p) {
            continue;
        }

        let host_ok = match r.host.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            None => true,
            Some(h) => host_matches(h, host),
        };
        if !host_ok {
            continue;
        }

        if let Some(ref methods) = r.methods {
            if !methods
                .iter()
                .any(|m| m.eq_ignore_ascii_case(method.as_str()))
            {
                continue;
            }
        }

        if let Some(ref required_headers) = r.headers {
            let mut headers_ok = true;
            for (key, expected) in required_headers {
                let actual = headers.get(key).and_then(|v| v.to_str().ok()).unwrap_or("");

                if !wildcard_match_ignore_ascii_case(expected, actual) {
                    headers_ok = false;
                    break;
                }
            }
            if !headers_ok {
                continue;
            }
        }

        let cand = (r, r.host.as_ref().is_some(), p.len());
        best = match best {
            None => Some(cand),
            Some((best_r, best_has_host, best_plen)) => {
                if cand.1 != best_has_host {
                    if cand.1 {
                        Some(cand)
                    } else {
                        Some((best_r, best_has_host, best_plen))
                    }
                } else if cand.2 > best_plen {
                    Some(cand)
                } else {
                    Some((best_r, best_has_host, best_plen))
                }
            }
        };
    }

    if let Some((r, _, _)) = best {
        (Some(r), r.id.as_deref().unwrap_or("").to_string())
    } else {
        (None, String::new())
    }
}

#[cfg(test)]
mod tests {
    use super::{host_matches, match_route, normalize_host, wildcard_match_ignore_ascii_case};
    use crate::config::{Route, Upstream};
    use axum::http::{HeaderMap, HeaderValue, Method};
    use std::collections::HashMap;

    fn route(
        id: &str,
        host: Option<&str>,
        path: &str,
        methods: Option<Vec<&str>>,
        headers: Option<Vec<(&str, &str)>>,
    ) -> Route {
        Route {
            id: Some(id.into()),
            enabled: true,
            host: host.map(str::to_string),
            path: Some(path.into()),
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
            methods: methods.map(|v| v.into_iter().map(str::to_string).collect()),
            headers: headers.map(|items| {
                items
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect::<HashMap<_, _>>()
            }),
            upstreams: vec![Upstream {
                url: "http://backend".into(),
                weight: 1,
            }],
        }
    }

    #[test]
    fn wildcard_match_handles_simple_patterns() {
        assert!(wildcard_match_ignore_ascii_case("*", "anything"));
        assert!(wildcard_match_ignore_ascii_case(
            "bearer *",
            "Bearer token-123"
        ));
        assert!(wildcard_match_ignore_ascii_case(
            "*json*",
            "application/json; charset=utf-8"
        ));
        assert!(wildcard_match_ignore_ascii_case("abc*xyz", "xxAbC---XyZyy"));

        assert!(!wildcard_match_ignore_ascii_case("abc*xyz", "abxcyz"));
        assert!(!wildcard_match_ignore_ascii_case("token", "token-1"));
    }

    #[test]
    fn normalize_host_trims_brackets_ports_and_whitespace() {
        assert_eq!(normalize_host(" example.com:443 "), "example.com");
        assert_eq!(normalize_host("[2001:db8::1]:8443"), "2001:db8::1");
        assert_eq!(normalize_host("service.internal."), "service.internal.");
    }

    #[test]
    fn host_matches_handles_trailing_dots_and_wildcards() {
        assert!(host_matches("example.com", "example.com."));
        assert!(host_matches("*.example.com", "api.example.com."));
        assert!(!host_matches("*.example.com", "badexample.com"));
    }

    #[test]
    fn match_route_prefers_longest_path_when_host_specificity_ties() {
        let routes = vec![
            route("short", Some("api.example.com"), "/api", None, None),
            route("long", Some("api.example.com"), "/api/admin", None, None),
        ];

        let (matched, route_id) = match_route(
            &routes,
            "api.example.com",
            "/api/admin/users",
            &Method::GET,
            &HeaderMap::new(),
        );

        assert_eq!(route_id, "long");
        assert_eq!(matched.and_then(|r| r.id.as_deref()), Some("long"));
    }

    #[test]
    fn match_route_prefers_host_specific_route_over_generic_route() {
        let routes = vec![
            route("generic", None, "/api/admin", None, None),
            route("hosted", Some("api.example.com"), "/api", None, None),
        ];

        let (matched, route_id) = match_route(
            &routes,
            "api.example.com",
            "/api/admin/users",
            &Method::GET,
            &HeaderMap::new(),
        );

        assert_eq!(route_id, "hosted");
        assert_eq!(matched.and_then(|r| r.id.as_deref()), Some("hosted"));
    }

    #[test]
    fn match_route_requires_method_and_header_constraints_together() {
        let routes = vec![
            route(
                "strict",
                Some("api.example.com"),
                "/v1",
                Some(vec!["POST"]),
                Some(vec![
                    ("x-env", "prod*"),
                    ("authorization", "Bearer *"),
                ]),
            ),
            route("fallback", Some("api.example.com"), "/v1", None, None),
        ];

        let mut headers = HeaderMap::new();
        headers.insert("x-env", HeaderValue::from_static("production"));
        headers.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_static("Bearer token-123"),
        );

        let (matched, route_id) =
            match_route(&routes, "api.example.com", "/v1/users", &Method::POST, &headers);

        assert_eq!(route_id, "strict");
        assert_eq!(matched.and_then(|r| r.id.as_deref()), Some("strict"));
    }

    #[test]
    fn match_route_falls_back_when_header_constraint_does_not_match() {
        let routes = vec![
            route(
                "strict",
                Some("api.example.com"),
                "/v1",
                Some(vec!["POST"]),
                Some(vec![("x-env", "prod*")]),
            ),
            route("fallback", Some("api.example.com"), "/v1", None, None),
        ];

        let mut headers = HeaderMap::new();
        headers.insert("x-env", HeaderValue::from_static("staging"));

        let (matched, route_id) =
            match_route(&routes, "api.example.com", "/v1/users", &Method::POST, &headers);

        assert_eq!(route_id, "fallback");
        assert_eq!(matched.and_then(|r| r.id.as_deref()), Some("fallback"));
    }

    #[test]
    fn match_route_returns_none_when_method_mismatches_all_candidates() {
        let routes = vec![route(
            "post-only",
            Some("api.example.com"),
            "/v1",
            Some(vec!["POST"]),
            None,
        )];

        let (matched, route_id) =
            match_route(&routes, "api.example.com", "/v1/users", &Method::GET, &HeaderMap::new());

        assert!(matched.is_none());
        assert!(route_id.is_empty());
    }
}
