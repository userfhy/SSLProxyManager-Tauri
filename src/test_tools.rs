use anyhow::{Context, Result};
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

/// 发送 HTTP 测试请求
pub async fn send_http_test_request(req: HttpTestRequest) -> Result<HttpTestResponse> {
    let start = Instant::now();

    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(req.timeout_ms.unwrap_or(30000)))
        .redirect(if req.follow_redirects.unwrap_or(true) {
            reqwest::redirect::Policy::limited(10)
        } else {
            reqwest::redirect::Policy::none()
        })
        .danger_accept_invalid_certs(true) // 用于测试环境
        .build()?;

    let method = reqwest::Method::from_bytes(req.method.as_bytes())
        .context("无效的 HTTP 方法")?;

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
            let body = response.text().await.unwrap_or_else(|e| format!("读取响应体失败: {}", e));

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

    // 遍历所有监听规则
    for rule in &config.rules {
        if !rule.enabled {
            continue;
        }

        // 遍历路由
        for route in &rule.routes {
            if !route.enabled {
                continue;
            }

            // 检查路径匹配
            let path_matches = if let Some(route_path) = &route.path {
                req.path.starts_with(route_path)
            } else {
                true
            };

            if !path_matches {
                continue;
            }

            // 检查 Host 匹配
            let host_matches = if let Some(route_host) = &route.host {
                if let Some(req_host) = &req.host {
                    route_host == req_host
                } else {
                    false
                }
            } else {
                true
            };

            if !host_matches {
                continue;
            }

            // 检查 HTTP 方法匹配
            let method_matches = if let Some(route_methods) = &route.methods {
                if let Some(req_method) = &req.method {
                    route_methods.iter().any(|m| m.eq_ignore_ascii_case(req_method))
                } else {
                    true
                }
            } else {
                true
            };

            if !method_matches {
                continue;
            }

            // 检查 Header 匹配
            let headers_match = if let Some(route_headers) = &route.headers {
                if let Some(req_headers) = &req.headers {
                    route_headers.iter().all(|(key, expected)| {
                        req_headers.get(key).map(|v| v == expected).unwrap_or(false)
                    })
                } else {
                    false
                }
            } else {
                true
            };

            if !headers_match {
                continue;
            }

            // 找到匹配的路由
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
                basic_auth_required: rule.basic_auth_enable && !route.exclude_basic_auth.unwrap_or(false),
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

/// 执行性能测试
pub async fn run_performance_test(req: PerformanceTestRequest) -> Result<PerformanceTestResult> {
    use tokio::sync::Mutex;
    use std::sync::Arc;

    let start = Instant::now();
    let end_time = start + Duration::from_secs(req.duration_seconds as u64);

    let total_requests = Arc::new(Mutex::new(0u64));
    let successful_requests = Arc::new(Mutex::new(0u64));
    let failed_requests = Arc::new(Mutex::new(0u64));
    let response_times = Arc::new(Mutex::new(Vec::new()));
    let status_codes = Arc::new(Mutex::new(std::collections::HashMap::new()));

    let mut handles = vec![];

    // 启动并发任务
    for _ in 0..req.concurrent {
        let url = req.url.clone();
        let method = req.method.clone();
        let headers = req.headers.clone();
        let body = req.body.clone();
        let total_requests = Arc::clone(&total_requests);
        let successful_requests = Arc::clone(&successful_requests);
        let failed_requests = Arc::clone(&failed_requests);
        let response_times = Arc::clone(&response_times);
        let status_codes = Arc::clone(&status_codes);

        let handle = tokio::spawn(async move {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .danger_accept_invalid_certs(true)
                .build()
                .unwrap();

            loop {
                if Instant::now() >= end_time {
                    break;
                }

                let req_start = Instant::now();

                let method = reqwest::Method::from_bytes(method.as_bytes()).unwrap();
                let mut request = client.request(method, &url);

                if let Some(ref headers) = headers {
                    for (key, value) in headers {
                        request = request.header(key, value);
                    }
                }

                if let Some(ref body) = body {
                    request = request.body(body.clone());
                }

                match request.send().await {
                    Ok(response) => {
                        let elapsed = req_start.elapsed().as_millis() as u64;
                        let status = response.status().as_u16();

                        *total_requests.lock().await += 1;
                        *successful_requests.lock().await += 1;
                        response_times.lock().await.push(elapsed);
                        *status_codes.lock().await.entry(status).or_insert(0) += 1;
                    }
                    Err(_) => {
                        *total_requests.lock().await += 1;
                        *failed_requests.lock().await += 1;
                    }
                }
            }
        });

        handles.push(handle);
    }

    // 等待所有任务完成
    for handle in handles {
        handle.await?;
    }

    let total_duration_ms = start.elapsed().as_millis() as u64;
    let total_requests = *total_requests.lock().await;
    let successful_requests = *successful_requests.lock().await;
    let failed_requests = *failed_requests.lock().await;
    let mut response_times = response_times.lock().await.clone();
    let status_codes = status_codes.lock().await.clone();

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
pub async fn validate_configuration(req: ConfigValidationRequest) -> Result<ConfigValidationResult> {
    let config = config::get_config();
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut certificate_checks = Vec::new();
    let mut upstream_checks = Vec::new();
    let mut port_checks = Vec::new();

    // 检查证书
    if req.check_certificates {
        for rule in &config.rules {
            if !rule.enabled || !rule.ssl_enable {
                continue;
            }

            let result = match axum_server::tls_rustls::RustlsConfig::from_pem_file(
                &rule.cert_file,
                &rule.key_file,
            )
            .await
            {
                Ok(_) => CertificateCheckResult {
                    listen_addr: rule.listen_addr.clone(),
                    cert_file: rule.cert_file.clone(),
                    key_file: rule.key_file.clone(),
                    valid: true,
                    error: None,
                },
                Err(e) => {
                    errors.push(format!("证书加载失败 ({}): {}", rule.listen_addr, e));
                    CertificateCheckResult {
                        listen_addr: rule.listen_addr.clone(),
                        cert_file: rule.cert_file.clone(),
                        key_file: rule.key_file.clone(),
                        valid: false,
                        error: Some(e.to_string()),
                    }
                }
            };

            certificate_checks.push(result);
        }
    }

    // 检查上游服务
    if req.check_upstreams {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .danger_accept_invalid_certs(true)
            .build()?;

        for rule in &config.rules {
            if !rule.enabled {
                continue;
            }

            for route in &rule.routes {
                if !route.enabled {
                    continue;
                }

                for upstream in &route.upstreams {
                    let start = Instant::now();
                    match timeout(Duration::from_secs(5), client.get(&upstream.url).send()).await {
                        Ok(Ok(_)) => {
                            let elapsed = start.elapsed().as_millis() as u64;
                            upstream_checks.push(UpstreamCheckResult {
                                url: upstream.url.clone(),
                                reachable: true,
                                response_time_ms: Some(elapsed),
                                error: None,
                            });
                        }
                        Ok(Err(e)) => {
                            warnings.push(format!("上游服务不可达: {} - {}", upstream.url, e));
                            upstream_checks.push(UpstreamCheckResult {
                                url: upstream.url.clone(),
                                reachable: false,
                                response_time_ms: None,
                                error: Some(e.to_string()),
                            });
                        }
                        Err(_) => {
                            warnings.push(format!("上游服务超时: {}", upstream.url));
                            upstream_checks.push(UpstreamCheckResult {
                                url: upstream.url.clone(),
                                reachable: false,
                                response_time_ms: None,
                                error: Some("请求超时".to_string()),
                            });
                        }
                    }
                }
            }
        }
    }

    // 检查端口可用性
    if req.check_ports {
        // 只有在服务未运行时才检查端口
        let is_running = crate::proxy::is_running();

        if is_running {
            warnings.push("服务正在运行，跳过端口可用性检查".to_string());
        } else {
            for rule in &config.rules {
                if !rule.enabled {
                    continue;
                }

                for addr in &rule.listen_addrs {
                    // 规范化地址：如果以 : 开头，补全为 0.0.0.0:
                    let normalized_addr = if addr.starts_with(':') {
                        format!("0.0.0.0{}", addr)
                    } else {
                        addr.clone()
                    };

                    match tokio::net::TcpListener::bind(&normalized_addr).await {
                        Ok(_) => {
                            port_checks.push(PortCheckResult {
                                listen_addr: addr.clone(),
                                available: true,
                                error: None,
                            });
                        }
                        Err(e) => {
                            errors.push(format!("端口不可用: {} - {}", addr, e));
                            port_checks.push(PortCheckResult {
                                listen_addr: addr.clone(),
                                available: false,
                                error: Some(e.to_string()),
                            });
                        }
                    }
                }
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
