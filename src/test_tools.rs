use anyhow::{Context, Result};
use dashmap::DashMap;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tokio::time::timeout;

use crate::config;

/// HTTP 测试请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpTestRequest {
    pub method: String,
    pub url: String,
    pub headers: std::collections::HashMap<String, String>,
    pub body: Option<String>,
    pub timeout_ms: Option<u64>,
    pub follow_redirects: Option<bool>,
}

/// HTTP 测试响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpTestResponse {
    pub status: u16,
    pub status_text: String,
    pub headers: std::collections::HashMap<String, String>,
    pub body: String,
    pub elapsed_ms: u64,
    pub error: Option<String>,
}

/// 路由测试请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteTestRequest {
    pub path: String,
    pub method: Option<String>,
    pub host: Option<String>,
    pub headers: Option<std::collections::HashMap<String, String>>,
    pub listen_addr: Option<String>,
}

/// 路由测试结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteTestResult {
    pub matched: bool,
    pub listen_rule_id: Option<String>,
    pub route_id: Option<String>,
    pub listen_addr: Option<String>,
    pub matched_path: Option<String>,
    pub upstream_url: Option<String>,
    pub set_headers: Option<std::collections::HashMap<String, String>>,
    pub remove_headers: Option<Vec<String>>,
    pub static_dir: Option<String>,
    pub proxy_pass_path: Option<String>,
    pub basic_auth_required: bool,
    pub ssl_enabled: bool,
}

/// 路由回归测试用例
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteTestSuiteCase {
    pub name: String,
    pub path: String,
    pub method: Option<String>,
    pub host: Option<String>,
    pub headers: Option<std::collections::HashMap<String, String>>,
    pub listen_addr: Option<String>,
    pub expect_matched: bool,
    pub expect_listen_rule_id: Option<String>,
    pub expect_route_id: Option<String>,
    pub expect_listen_addr: Option<String>,
}

/// 路由回归测试请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteTestSuiteRequest {
    pub cases: Vec<RouteTestSuiteCase>,
    pub stop_on_failure: Option<bool>,
}

/// 路由回归测试单用例结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteTestSuiteCaseResult {
    pub name: String,
    pub passed: bool,
    pub failure_reason: Option<String>,
    pub elapsed_ms: u64,
    pub actual: RouteTestResult,
}

/// 路由回归测试结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteTestSuiteResult {
    pub total_cases: u64,
    pub passed_cases: u64,
    pub failed_cases: u64,
    pub elapsed_ms: u64,
    pub cases: Vec<RouteTestSuiteCaseResult>,
}

/// 性能测试请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceTestRequest {
    pub url: String,
    pub method: String,
    pub headers: Option<std::collections::HashMap<String, String>>,
    pub body: Option<String>,
    pub concurrent: u32,
    pub duration_seconds: u32,
}

/// 性能测试结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceTestResult {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub total_duration_ms: u64,
    pub requests_per_second: f64,
    pub avg_response_time_ms: f64,
    pub min_response_time_ms: u64,
    pub max_response_time_ms: u64,
    pub p50_response_time_ms: u64,
    pub p95_response_time_ms: u64,
    pub p99_response_time_ms: u64,
    pub status_codes: std::collections::HashMap<u16, u64>,
}

/// 配置验证请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigValidationRequest {
    pub check_certificates: bool,
    pub check_upstreams: bool,
    pub check_ports: bool,
}

/// 配置验证结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub certificate_checks: Vec<CertificateCheckResult>,
    pub upstream_checks: Vec<UpstreamCheckResult>,
    pub port_checks: Vec<PortCheckResult>,
}

/// DNS 查询请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsLookupRequest {
    pub domain: String,
}

/// DNS 查询结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsLookupResult {
    pub domain: String,
    pub ipv4_addresses: Vec<String>,
    pub ipv6_addresses: Vec<String>,
    pub error: Option<String>,
}

/// SSL 证书信息请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SslCertInfoRequest {
    pub url: String,
}

/// SSL 证书信息结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SslCertInfoResult {
    pub valid: bool,
    pub subject: String,
    pub issuer: String,
    pub not_before: String,
    pub not_after: String,
    pub days_until_expiry: i64,
    pub error: Option<String>,
}

/// 生成自签名证书请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfSignedCertRequest {
    pub common_name: String,
    pub organization: Option<String>,
    pub organizational_unit: Option<String>,
    pub subject_alt_names: Option<Vec<String>>,
    pub valid_days: Option<u32>,
    pub output_dir: Option<String>,
}

/// 生成自签名证书结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfSignedCertResult {
    pub cert_file: String,
    pub key_file: String,
    pub common_name: String,
    pub subject_alt_names: Vec<String>,
    pub valid_days: u32,
}

/// 端口扫描请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortScanRequest {
    pub host: String,
    pub ports: Vec<u16>,
    pub timeout_ms: u64,
}

/// 端口扫描结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortScanResult {
    pub host: String,
    pub results: Vec<PortStatus>,
    pub total_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortStatus {
    pub port: u16,
    pub open: bool,
    pub service: Option<String>,
}

/// 编码/解码请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncodeDecodeRequest {
    pub operation: String, // "base64_encode", "base64_decode", "url_encode", "url_decode"
    pub input: String,
}

/// 编码/解码结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncodeDecodeResult {
    pub output: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertificateCheckResult {
    pub listen_addr: String,
    pub cert_file: String,
    pub key_file: String,
    pub valid: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamCheckResult {
    pub url: String,
    pub reachable: bool,
    pub response_time_ms: Option<u64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortCheckResult {
    pub listen_addr: String,
    pub available: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct HttpClientCacheKey {
    timeout_ms: u64,
    follow_redirects: bool,
}

static HTTP_TEST_CLIENT_CACHE: Lazy<DashMap<HttpClientCacheKey, reqwest::Client>> =
    Lazy::new(DashMap::new);

fn get_or_build_http_test_client(
    timeout_ms: u64,
    follow_redirects: bool,
) -> Result<reqwest::Client> {
    let key = HttpClientCacheKey {
        timeout_ms,
        follow_redirects,
    };
    if let Some(client) = HTTP_TEST_CLIENT_CACHE.get(&key) {
        return Ok(client.clone());
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .redirect(if follow_redirects {
            reqwest::redirect::Policy::limited(10)
        } else {
            reqwest::redirect::Policy::none()
        })
        .danger_accept_invalid_certs(true) // 用于测试环境
        .build()
        .context("创建 HTTP 客户端失败")?;

    HTTP_TEST_CLIENT_CACHE.insert(key, client.clone());
    Ok(client)
}

/// 发送 HTTP 测试请求
pub async fn send_http_test_request(req: HttpTestRequest) -> Result<HttpTestResponse> {
    let start = Instant::now();
    let timeout_ms = req.timeout_ms.unwrap_or(30000);
    let follow_redirects = req.follow_redirects.unwrap_or(true);
    let client = get_or_build_http_test_client(timeout_ms, follow_redirects)?;

    let method = reqwest::Method::from_bytes(req.method.as_bytes()).context("无效的 HTTP 方法")?;

    let mut request = client.request(method, &req.url);

    // 添加请求头
    for (key, value) in req.headers {
        request = request.header(key, value);
    }

    // 添加请求体
    if let Some(body) = req.body {
        request = request.body(body);
    }

    // 发送请求
    match request.send().await {
        Ok(response) => {
            let status = response.status().as_u16();
            let status_text = response.status().to_string();

            // 提取响应头
            let mut headers = std::collections::HashMap::new();
            for (key, value) in response.headers() {
                if let Ok(value_str) = value.to_str() {
                    headers.insert(key.to_string(), value_str.to_string());
                }
            }

            // 读取响应体
            let body = response
                .text()
                .await
                .unwrap_or_else(|e| format!("读取响应体失败: {}", e));

            let elapsed_ms = start.elapsed().as_millis() as u64;

            Ok(HttpTestResponse {
                status,
                status_text,
                headers,
                body,
                elapsed_ms,
                error: None,
            })
        }
        Err(e) => {
            let elapsed_ms = start.elapsed().as_millis() as u64;
            Ok(HttpTestResponse {
                status: 0,
                status_text: "Error".to_string(),
                headers: std::collections::HashMap::new(),
                body: String::new(),
                elapsed_ms,
                error: Some(e.to_string()),
            })
        }
    }
}

/// 测试路由匹配
pub fn test_route_matching(req: RouteTestRequest) -> Result<RouteTestResult> {
    let config = config::get_config();
    let req_headers = normalize_request_headers(req.headers.as_ref());
    let req_host = req.host.as_deref().unwrap_or("").trim();
    let listen_addr_filter = req
        .listen_addr
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    for rule in &config.rules {
        if !rule.enabled {
            continue;
        }
        if let Some(filter) = listen_addr_filter {
            let in_listen_addrs = rule
                .listen_addrs
                .iter()
                .any(|addr| addr.trim().eq_ignore_ascii_case(filter));
            if !rule.listen_addr.trim().eq_ignore_ascii_case(filter) && !in_listen_addrs {
                continue;
            }
        }

        let route = find_best_matching_route(
            &rule.routes,
            req.path.as_str(),
            req.method.as_deref(),
            req_host,
            &req_headers,
        );

        if let Some(route) = route {
            let upstream_url = route.upstreams.first().map(|u| u.url.clone());
            return Ok(RouteTestResult {
                matched: true,
                listen_rule_id: rule.id.clone(),
                route_id: route.id.clone(),
                listen_addr: Some(rule.listen_addr.clone()),
                matched_path: route.path.clone(),
                upstream_url,
                set_headers: route.set_headers.clone(),
                remove_headers: route.remove_headers.clone(),
                static_dir: route.static_dir.clone(),
                proxy_pass_path: route.proxy_pass_path.clone(),
                basic_auth_required: rule.basic_auth_enable
                    && !route.exclude_basic_auth.unwrap_or(false),
                ssl_enabled: rule.ssl_enable,
            });
        }
    }

    // 没有匹配的路由
    Ok(RouteTestResult {
        matched: false,
        listen_rule_id: None,
        route_id: None,
        listen_addr: None,
        matched_path: None,
        upstream_url: None,
        set_headers: None,
        remove_headers: None,
        static_dir: None,
        proxy_pass_path: None,
        basic_auth_required: false,
        ssl_enabled: false,
    })
}

fn normalize_request_headers(
    headers: Option<&std::collections::HashMap<String, String>>,
) -> std::collections::HashMap<String, String> {
    let mut normalized = std::collections::HashMap::new();
    if let Some(headers) = headers {
        for (k, v) in headers {
            normalized.insert(k.to_ascii_lowercase(), v.clone());
        }
    }
    normalized
}

#[inline]
fn normalize_host(host: &str) -> &str {
    host.split(':').next().unwrap_or(host).trim()
}

#[inline]
fn host_matches(route_host: &str, request_host: &str) -> bool {
    let route_host = normalize_host(route_host);
    let request_host = normalize_host(request_host);

    if request_host.is_empty() {
        return route_host.is_empty();
    }

    if route_host.eq_ignore_ascii_case(request_host) {
        return true;
    }

    if route_host.starts_with("*.") {
        let suffix = &route_host[2..];
        if !suffix.is_empty() && request_host.ends_with(suffix) {
            let prefix_len = request_host.len() - suffix.len();
            if prefix_len > 0 {
                let prefix = &request_host[..prefix_len];
                if !prefix.contains('.') && !prefix.is_empty() {
                    return true;
                }
            }
        }
    }

    false
}

fn headers_match(
    required_headers: &std::collections::HashMap<String, String>,
    req_headers: &std::collections::HashMap<String, String>,
) -> bool {
    for (key, expected) in required_headers {
        let actual = req_headers
            .get(&key.to_ascii_lowercase())
            .map(String::as_str)
            .unwrap_or("");

        if expected.contains('*') {
            let pattern = expected.replace('*', ".*");
            if let Some(re) = crate::proxy::cached_regex(&pattern) {
                if !re.is_match(actual) {
                    return false;
                }
            } else if !actual.contains(expected.replace('*', "").as_str()) {
                return false;
            }
        } else if !actual.eq_ignore_ascii_case(expected.trim()) {
            return false;
        }
    }

    true
}

fn find_best_matching_route<'a>(
    routes: &'a [config::Route],
    path: &str,
    method: Option<&str>,
    request_host: &str,
    req_headers: &std::collections::HashMap<String, String>,
) -> Option<&'a config::Route> {
    let host = normalize_host(request_host);
    let mut best: Option<(&config::Route, bool, usize)> = None;

    for route in routes {
        if !route.enabled {
            continue;
        }

        let route_path = match route.path.as_deref() {
            Some(v) => v,
            None => continue,
        };
        if !path.starts_with(route_path) {
            continue;
        }

        let route_host = route
            .host
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let host_ok = match route_host {
            None => true,
            Some(h) => host_matches(h, host),
        };
        if !host_ok {
            continue;
        }

        if let Some(route_methods) = &route.methods {
            if let Some(req_method) = method {
                if !route_methods
                    .iter()
                    .any(|m| m.eq_ignore_ascii_case(req_method))
                {
                    continue;
                }
            }
        }

        if let Some(required_headers) = &route.headers {
            if !headers_match(required_headers, req_headers) {
                continue;
            }
        }

        let cand = (route, route.host.is_some(), route_path.len());
        best = match best {
            None => Some(cand),
            Some((best_route, best_has_host, best_plen)) => {
                if cand.1 != best_has_host {
                    if cand.1 {
                        Some(cand)
                    } else {
                        Some((best_route, best_has_host, best_plen))
                    }
                } else if cand.2 > best_plen {
                    Some(cand)
                } else {
                    Some((best_route, best_has_host, best_plen))
                }
            }
        };
    }

    best.map(|(route, _, _)| route)
}

/// 执行路由回归测试集（最小版）
pub fn run_route_test_suite(req: RouteTestSuiteRequest) -> Result<RouteTestSuiteResult> {
    use anyhow::anyhow;

    if req.cases.is_empty() {
        return Err(anyhow!("测试集不能为空"));
    }
    if req.cases.len() > 1000 {
        return Err(anyhow!("测试集过大，最多支持 1000 条用例"));
    }

    let started = Instant::now();
    let stop_on_failure = req.stop_on_failure.unwrap_or(false);
    let mut results = Vec::with_capacity(req.cases.len());
    let mut passed_cases = 0u64;
    let mut failed_cases = 0u64;

    for case in req.cases {
        let case_started = Instant::now();
        let actual = test_route_matching(RouteTestRequest {
            path: case.path.clone(),
            method: case.method.clone(),
            host: case.host.clone(),
            headers: case.headers.clone(),
            listen_addr: case.listen_addr.clone(),
        })?;

        let mut failure_reason = None;
        if actual.matched != case.expect_matched {
            failure_reason = Some(format!(
                "匹配状态不符: expect_matched={}, actual_matched={}",
                case.expect_matched, actual.matched
            ));
        }
        if failure_reason.is_none() {
            if let Some(expected) = case.expect_listen_rule_id.as_deref() {
                if actual.listen_rule_id.as_deref() != Some(expected) {
                    failure_reason = Some(format!(
                        "listen_rule_id 不符: expected={}, actual={}",
                        expected,
                        actual.listen_rule_id.as_deref().unwrap_or("")
                    ));
                }
            }
        }
        if failure_reason.is_none() {
            if let Some(expected) = case.expect_route_id.as_deref() {
                if actual.route_id.as_deref() != Some(expected) {
                    failure_reason = Some(format!(
                        "route_id 不符: expected={}, actual={}",
                        expected,
                        actual.route_id.as_deref().unwrap_or("")
                    ));
                }
            }
        }
        if failure_reason.is_none() {
            if let Some(expected) = case.expect_listen_addr.as_deref() {
                if actual.listen_addr.as_deref() != Some(expected) {
                    failure_reason = Some(format!(
                        "listen_addr 不符: expected={}, actual={}",
                        expected,
                        actual.listen_addr.as_deref().unwrap_or("")
                    ));
                }
            }
        }

        let passed = failure_reason.is_none();
        if passed {
            passed_cases += 1;
        } else {
            failed_cases += 1;
        }

        results.push(RouteTestSuiteCaseResult {
            name: case.name,
            passed,
            failure_reason,
            elapsed_ms: case_started.elapsed().as_millis() as u64,
            actual,
        });

        if stop_on_failure && !passed {
            break;
        }
    }

    Ok(RouteTestSuiteResult {
        total_cases: results.len() as u64,
        passed_cases,
        failed_cases,
        elapsed_ms: started.elapsed().as_millis() as u64,
        cases: results,
    })
}

/// 执行性能测试
pub async fn run_performance_test(req: PerformanceTestRequest) -> Result<PerformanceTestResult> {
    use std::collections::HashMap;

    #[derive(Default)]
    struct WorkerStats {
        total_requests: u64,
        successful_requests: u64,
        failed_requests: u64,
        response_times: Vec<u64>,
        status_codes: HashMap<u16, u64>,
    }

    let start = Instant::now();
    let end_time = start + Duration::from_secs(req.duration_seconds as u64);
    let concurrent = req.concurrent.max(1);
    let method = reqwest::Method::from_bytes(req.method.as_bytes()).context("无效的 HTTP 方法")?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .danger_accept_invalid_certs(true)
        .build()
        .context("创建性能测试 HTTP 客户端失败")?;

    let mut handles = Vec::with_capacity(concurrent as usize);

    // 启动并发任务
    for _ in 0..concurrent {
        let url = req.url.clone();
        let method = method.clone();
        let headers = req.headers.clone();
        let body = req.body.clone();
        let client = client.clone();

        let handle = tokio::spawn(async move {
            let mut stats = WorkerStats::default();

            loop {
                if Instant::now() >= end_time {
                    break;
                }

                let req_start = Instant::now();
                let mut request = client.request(method.clone(), &url);

                if let Some(ref headers) = headers {
                    for (key, value) in headers {
                        request = request.header(key, value);
                    }
                }

                if let Some(ref body) = body {
                    request = request.body(body.clone());
                }

                stats.total_requests += 1;
                match request.send().await {
                    Ok(response) => {
                        let elapsed = req_start.elapsed().as_millis() as u64;
                        let status = response.status().as_u16();

                        stats.successful_requests += 1;
                        stats.response_times.push(elapsed);
                        *stats.status_codes.entry(status).or_insert(0) += 1;
                    }
                    Err(_) => {
                        stats.failed_requests += 1;
                    }
                }
            }

            stats
        });

        handles.push(handle);
    }

    let mut total_requests = 0u64;
    let mut successful_requests = 0u64;
    let mut failed_requests = 0u64;
    let mut response_times = Vec::new();
    let mut status_codes: HashMap<u16, u64> = HashMap::new();

    // 等待所有任务完成
    for handle in handles {
        let worker = handle.await.context("性能测试任务执行失败")?;
        total_requests += worker.total_requests;
        successful_requests += worker.successful_requests;
        failed_requests += worker.failed_requests;
        response_times.extend(worker.response_times);
        for (status, count) in worker.status_codes {
            *status_codes.entry(status).or_insert(0) += count;
        }
    }

    let total_duration_ms = start.elapsed().as_millis() as u64;

    // 计算统计数据
    response_times.sort_unstable();

    let requests_per_second = if total_duration_ms > 0 {
        (total_requests as f64 / total_duration_ms as f64) * 1000.0
    } else {
        0.0
    };

    let avg_response_time_ms = if !response_times.is_empty() {
        response_times.iter().sum::<u64>() as f64 / response_times.len() as f64
    } else {
        0.0
    };

    let min_response_time_ms = response_times.first().copied().unwrap_or(0);
    let max_response_time_ms = response_times.last().copied().unwrap_or(0);

    let p50_index = (response_times.len() as f64 * 0.50) as usize;
    let p95_index = (response_times.len() as f64 * 0.95) as usize;
    let p99_index = (response_times.len() as f64 * 0.99) as usize;

    let p50_response_time_ms = response_times.get(p50_index).copied().unwrap_or(0);
    let p95_response_time_ms = response_times.get(p95_index).copied().unwrap_or(0);
    let p99_response_time_ms = response_times.get(p99_index).copied().unwrap_or(0);

    Ok(PerformanceTestResult {
        total_requests,
        successful_requests,
        failed_requests,
        total_duration_ms,
        requests_per_second,
        avg_response_time_ms,
        min_response_time_ms,
        max_response_time_ms,
        p50_response_time_ms,
        p95_response_time_ms,
        p99_response_time_ms,
        status_codes,
    })
}

/// 验证配置
pub async fn validate_configuration(
    req: ConfigValidationRequest,
) -> Result<ConfigValidationResult> {
    use futures_util::stream::{self, StreamExt};

    const VALIDATION_CONCURRENCY: usize = 16;

    let config = config::get_config();
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut certificate_checks = Vec::new();
    let mut upstream_checks = Vec::new();
    let mut port_checks = Vec::new();

    // 检查证书
    if req.check_certificates {
        let cert_targets: Vec<(String, String, String)> = config
            .rules
            .iter()
            .filter(|rule| rule.enabled && rule.ssl_enable)
            .map(|rule| {
                (
                    rule.listen_addr.clone(),
                    rule.cert_file.clone(),
                    rule.key_file.clone(),
                )
            })
            .collect();

        let mut cert_results: Vec<_> = stream::iter(cert_targets.into_iter().enumerate().map(
            |(idx, (listen_addr, cert_file, key_file))| async move {
                let check = match axum_server::tls_rustls::RustlsConfig::from_pem_file(
                    &cert_file, &key_file,
                )
                .await
                {
                    Ok(_) => CertificateCheckResult {
                        listen_addr: listen_addr.clone(),
                        cert_file,
                        key_file,
                        valid: true,
                        error: None,
                    },
                    Err(e) => CertificateCheckResult {
                        listen_addr: listen_addr.clone(),
                        cert_file,
                        key_file,
                        valid: false,
                        error: Some(e.to_string()),
                    },
                };

                (idx, check)
            },
        ))
        .buffer_unordered(VALIDATION_CONCURRENCY)
        .collect()
        .await;

        cert_results.sort_by_key(|(idx, _)| *idx);
        for (_, check) in cert_results {
            if let Some(err) = &check.error {
                errors.push(format!("证书加载失败 ({}): {}", check.listen_addr, err));
            }
            certificate_checks.push(check);
        }
    }

    // 检查上游服务
    if req.check_upstreams {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .danger_accept_invalid_certs(true)
            .build()?;

        let upstream_targets: Vec<String> = config
            .rules
            .iter()
            .filter(|rule| rule.enabled)
            .flat_map(|rule| rule.routes.iter().filter(|route| route.enabled))
            .flat_map(|route| route.upstreams.iter().map(|upstream| upstream.url.clone()))
            .collect();

        let mut upstream_results: Vec<_> =
            stream::iter(upstream_targets.into_iter().enumerate().map(|(idx, url)| {
                let client = client.clone();
                async move {
                    let start = Instant::now();
                    let check = match timeout(Duration::from_secs(5), client.get(&url).send()).await
                    {
                        Ok(Ok(_)) => UpstreamCheckResult {
                            url,
                            reachable: true,
                            response_time_ms: Some(start.elapsed().as_millis() as u64),
                            error: None,
                        },
                        Ok(Err(e)) => UpstreamCheckResult {
                            url,
                            reachable: false,
                            response_time_ms: None,
                            error: Some(e.to_string()),
                        },
                        Err(_) => UpstreamCheckResult {
                            url,
                            reachable: false,
                            response_time_ms: None,
                            error: Some("请求超时".to_string()),
                        },
                    };

                    (idx, check)
                }
            }))
            .buffer_unordered(VALIDATION_CONCURRENCY)
            .collect()
            .await;

        upstream_results.sort_by_key(|(idx, _)| *idx);
        for (_, check) in upstream_results {
            if !check.reachable {
                if let Some(err) = &check.error {
                    if err == "请求超时" {
                        warnings.push(format!("上游服务超时: {}", check.url));
                    } else {
                        warnings.push(format!("上游服务不可达: {} - {}", check.url, err));
                    }
                }
            }
            upstream_checks.push(check);
        }
    }

    // 检查端口可用性
    if req.check_ports {
        // 只有在服务未运行时才检查端口
        let is_running = crate::proxy::is_running();

        if is_running {
            warnings.push("服务正在运行，跳过端口可用性检查".to_string());
        } else {
            let listen_targets: Vec<String> = config
                .rules
                .iter()
                .filter(|rule| rule.enabled)
                .flat_map(|rule| {
                    if rule.listen_addrs.is_empty() {
                        vec![rule.listen_addr.clone()]
                    } else {
                        rule.listen_addrs.clone()
                    }
                })
                .collect();

            let mut port_results: Vec<_> =
                stream::iter(listen_targets.into_iter().enumerate().map(
                    |(idx, listen_addr)| async move {
                        let normalized_addr = if listen_addr.starts_with(':') {
                            format!("0.0.0.0{}", listen_addr)
                        } else {
                            listen_addr.clone()
                        };

                        let check = match tokio::net::TcpListener::bind(&normalized_addr).await {
                            Ok(_) => PortCheckResult {
                                listen_addr,
                                available: true,
                                error: None,
                            },
                            Err(e) => PortCheckResult {
                                listen_addr,
                                available: false,
                                error: Some(e.to_string()),
                            },
                        };

                        (idx, check)
                    },
                ))
                .buffer_unordered(VALIDATION_CONCURRENCY)
                .collect()
                .await;

            port_results.sort_by_key(|(idx, _)| *idx);
            for (_, check) in port_results {
                if let Some(err) = &check.error {
                    errors.push(format!("端口不可用: {} - {}", check.listen_addr, err));
                }
                port_checks.push(check);
            }
        }
    }

    Ok(ConfigValidationResult {
        valid: errors.is_empty(),
        errors,
        warnings,
        certificate_checks,
        upstream_checks,
        port_checks,
    })
}

/// DNS 查询
pub async fn dns_lookup(req: DnsLookupRequest) -> Result<DnsLookupResult> {
    use tokio::net::lookup_host;

    let mut ipv4_addresses = Vec::new();
    let mut ipv6_addresses = Vec::new();

    match lookup_host(format!("{}:0", req.domain)).await {
        Ok(addrs) => {
            for addr in addrs {
                let ip = addr.ip();
                if ip.is_ipv4() {
                    ipv4_addresses.push(ip.to_string());
                } else if ip.is_ipv6() {
                    ipv6_addresses.push(ip.to_string());
                }
            }
            Ok(DnsLookupResult {
                domain: req.domain,
                ipv4_addresses,
                ipv6_addresses,
                error: None,
            })
        }
        Err(e) => Ok(DnsLookupResult {
            domain: req.domain,
            ipv4_addresses: vec![],
            ipv6_addresses: vec![],
            error: Some(e.to_string()),
        }),
    }
}

fn sanitize_filename_component(input: &str) -> String {
    let mut s = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
            s.push(ch);
        } else {
            s.push('_');
        }
    }

    let trimmed = s.trim_matches('_');
    if trimmed.is_empty() {
        "selfsigned".to_string()
    } else {
        trimmed.chars().take(48).collect()
    }
}

/// 生成自签名证书（PEM）
pub fn generate_self_signed_cert(req: SelfSignedCertRequest) -> Result<SelfSignedCertResult> {
    use anyhow::anyhow;
    use chrono::{Datelike, Duration as ChronoDuration, Local, Utc};
    use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair};
    use std::collections::HashSet;
    use std::path::PathBuf;

    let common_name = req.common_name.trim();
    if common_name.is_empty() {
        return Err(anyhow!("通用名称不能为空"));
    }
    let organization = req
        .organization
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let organizational_unit = req
        .organizational_unit
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let valid_days = req.valid_days.unwrap_or(365).clamp(1, 3650);

    let mut subject_alt_names = vec![common_name.to_string()];
    if let Some(items) = req.subject_alt_names {
        for item in items {
            let item = item.trim();
            if !item.is_empty() {
                subject_alt_names.push(item.to_string());
            }
        }
    }

    // 按字面值去重（不区分大小写）
    let mut seen = HashSet::new();
    subject_alt_names.retain(|s| seen.insert(s.to_ascii_lowercase()));

    let mut params =
        CertificateParams::new(subject_alt_names.clone()).context("无效的备用名称(SAN)")?;
    params.distinguished_name = DistinguishedName::new();
    params
        .distinguished_name
        .push(DnType::CommonName, common_name.to_string());
    if let Some(org) = organization {
        params
            .distinguished_name
            .push(DnType::OrganizationName, org.to_string());
    }
    if let Some(ou) = organizational_unit {
        params
            .distinguished_name
            .push(DnType::OrganizationalUnitName, ou.to_string());
    }

    // 回拨一天容忍轻微时钟偏移
    let not_before = Utc::now() - ChronoDuration::days(1);
    let not_after = Utc::now() + ChronoDuration::days(valid_days as i64);
    params.not_before = rcgen::date_time_ymd(
        not_before.year(),
        not_before.month() as u8,
        not_before.day() as u8,
    );
    params.not_after = rcgen::date_time_ymd(
        not_after.year(),
        not_after.month() as u8,
        not_after.day() as u8,
    );

    let key_pair = KeyPair::generate().context("生成私钥失败")?;
    let cert = params
        .self_signed(&key_pair)
        .context("生成自签名证书失败")?;
    let cert_pem = cert.pem();
    let key_pem = key_pair.serialize_pem();

    let output_dir = if let Some(dir) = req
        .output_dir
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        PathBuf::from(dir)
    } else {
        let cfg_path = config::get_config_path().context("获取配置路径失败")?;
        let base = cfg_path
            .parent()
            .ok_or_else(|| anyhow!("无法确定配置目录"))?;
        base.join("ssl")
    };

    std::fs::create_dir_all(&output_dir)
        .with_context(|| format!("创建输出目录失败: {}", output_dir.display()))?;

    let safe_cn = sanitize_filename_component(common_name);
    let ts = Local::now().format("%Y%m%d-%H%M%S").to_string();
    let uid = uuid::Uuid::new_v4().to_string().replace('-', "");
    let suffix = uid.chars().take(8).collect::<String>();

    let cert_path = output_dir.join(format!("{}-{}-{}.crt", safe_cn, ts, suffix));
    let key_path = output_dir.join(format!("{}-{}-{}.key", safe_cn, ts, suffix));

    std::fs::write(&cert_path, cert_pem)
        .with_context(|| format!("写入证书文件失败: {}", cert_path.display()))?;
    std::fs::write(&key_path, key_pem)
        .with_context(|| format!("写入私钥文件失败: {}", key_path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(&key_path) {
            let mut perm = meta.permissions();
            perm.set_mode(0o600);
            let _ = std::fs::set_permissions(&key_path, perm);
        }
    }

    Ok(SelfSignedCertResult {
        cert_file: cert_path.to_string_lossy().to_string(),
        key_file: key_path.to_string_lossy().to_string(),
        common_name: common_name.to_string(),
        subject_alt_names,
        valid_days,
    })
}

/// 获取 SSL 证书信息
pub async fn get_ssl_cert_info(req: SslCertInfoRequest) -> Result<SslCertInfoResult> {
    use rustls::pki_types::ServerName;
    use std::sync::Arc;

    let url = req.url.parse::<reqwest::Url>().context("无效的 URL")?;
    let host = url.host_str().context("无法解析主机名")?;
    let port = url.port().unwrap_or(443);

    let mut root_store = rustls::RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

    let config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    let connector = tokio_rustls::TlsConnector::from(Arc::new(config));
    let stream = tokio::net::TcpStream::connect((host, port)).await?;
    let server_name = ServerName::try_from(host.to_string())?;

    match connector.connect(server_name, stream).await {
        Ok(tls_stream) => {
            let (_, session) = tls_stream.into_inner();
            if let Some(certs) = session.peer_certificates() {
                if let Some(cert) = certs.first() {
                    use x509_parser::prelude::*;
                    let parsed = X509Certificate::from_der(cert.as_ref())
                        .context("解析证书失败")?
                        .1;

                    let subject = parsed.subject().to_string();
                    let issuer = parsed.issuer().to_string();
                    let not_before = parsed.validity().not_before.to_string();
                    let not_after = parsed.validity().not_after.to_string();

                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)?
                        .as_secs() as i64;
                    let expiry = parsed.validity().not_after.timestamp();
                    let days_until_expiry = (expiry - now) / 86400;

                    return Ok(SslCertInfoResult {
                        valid: true,
                        subject,
                        issuer,
                        not_before,
                        not_after,
                        days_until_expiry,
                        error: None,
                    });
                }
            }
            Ok(SslCertInfoResult {
                valid: false,
                subject: String::new(),
                issuer: String::new(),
                not_before: String::new(),
                not_after: String::new(),
                days_until_expiry: 0,
                error: Some("未找到证书".to_string()),
            })
        }
        Err(e) => Ok(SslCertInfoResult {
            valid: false,
            subject: String::new(),
            issuer: String::new(),
            not_before: String::new(),
            not_after: String::new(),
            days_until_expiry: 0,
            error: Some(e.to_string()),
        }),
    }
}

/// 端口扫描
pub async fn scan_ports(req: PortScanRequest) -> Result<PortScanResult> {
    use futures_util::stream::{self, StreamExt};
    use std::time::Duration;
    use tokio::time::timeout;

    let start = Instant::now();
    let timeout_duration = Duration::from_millis(req.timeout_ms);
    let host = req.host.clone();
    let ports = req.ports;
    let port_count = ports.len();
    let concurrency = port_count.clamp(1, 128);

    let mut tasks = stream::iter(ports.into_iter().enumerate().map(|(index, port)| {
        let host = host.clone();
        async move {
            let addr = format!("{}:{}", host, port);
            let is_open = timeout(timeout_duration, tokio::net::TcpStream::connect(&addr))
                .await
                .is_ok();
            let service = if is_open {
                get_service_name(port)
            } else {
                None
            };
            (
                index,
                PortStatus {
                    port,
                    open: is_open,
                    service,
                },
            )
        }
    }))
    .buffer_unordered(concurrency);

    let mut ordered_results = vec![None; port_count];
    while let Some((index, status)) = tasks.next().await {
        ordered_results[index] = Some(status);
    }
    let results = ordered_results.into_iter().flatten().collect();

    Ok(PortScanResult {
        host: req.host,
        results,
        total_time_ms: start.elapsed().as_millis() as u64,
    })
}

fn get_service_name(port: u16) -> Option<String> {
    match port {
        21 => Some("FTP".to_string()),
        22 => Some("SSH".to_string()),
        80 => Some("HTTP".to_string()),
        443 => Some("HTTPS".to_string()),
        3306 => Some("MySQL".to_string()),
        5432 => Some("PostgreSQL".to_string()),
        6379 => Some("Redis".to_string()),
        8080 => Some("HTTP-Alt".to_string()),
        27017 => Some("MongoDB".to_string()),
        _ => None,
    }
}

/// 编码/解码
pub fn encode_decode(req: EncodeDecodeRequest) -> Result<EncodeDecodeResult> {
    use base64::Engine;
    let output = match req.operation.as_str() {
        "base64_encode" => base64::engine::general_purpose::STANDARD.encode(&req.input),
        "base64_decode" => match base64::engine::general_purpose::STANDARD.decode(&req.input) {
            Ok(bytes) => String::from_utf8(bytes).unwrap_or_else(|_| "无法解码为UTF-8".to_string()),
            Err(e) => {
                return Ok(EncodeDecodeResult {
                    output: String::new(),
                    error: Some(e.to_string()),
                })
            }
        },
        "url_encode" => urlencoding::encode(&req.input).to_string(),
        "url_decode" => match urlencoding::decode(&req.input) {
            Ok(s) => s.to_string(),
            Err(e) => {
                return Ok(EncodeDecodeResult {
                    output: String::new(),
                    error: Some(e.to_string()),
                })
            }
        },
        "hex_encode" => hex::encode(&req.input),
        "hex_decode" => match hex::decode(&req.input) {
            Ok(bytes) => String::from_utf8(bytes).unwrap_or_else(|_| "无法解码为UTF-8".to_string()),
            Err(e) => {
                return Ok(EncodeDecodeResult {
                    output: String::new(),
                    error: Some(e.to_string()),
                })
            }
        },
        _ => {
            return Ok(EncodeDecodeResult {
                output: String::new(),
                error: Some("不支持的操作".to_string()),
            })
        }
    };
    Ok(EncodeDecodeResult {
        output,
        error: None,
    })
}
