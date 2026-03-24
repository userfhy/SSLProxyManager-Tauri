use crate::test_tools;

#[tauri::command]
pub async fn send_http_test(req: test_tools::HttpTestRequest) -> Result<test_tools::HttpTestResponse, String> {
    test_tools::send_http_test_request(req)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn test_route_match(req: test_tools::RouteTestRequest) -> Result<test_tools::RouteTestResult, String> {
    test_tools::test_route_matching(req)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn run_route_test_suite(req: test_tools::RouteTestSuiteRequest) -> Result<test_tools::RouteTestSuiteResult, String> {
    test_tools::run_route_test_suite(req)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn run_performance_test(req: test_tools::PerformanceTestRequest) -> Result<test_tools::PerformanceTestResult, String> {
    test_tools::run_performance_test(req)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn validate_config_tool(req: test_tools::ConfigValidationRequest) -> Result<test_tools::ConfigValidationResult, String> {
    test_tools::validate_configuration(req)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn dns_lookup(req: test_tools::DnsLookupRequest) -> Result<test_tools::DnsLookupResult, String> {
    test_tools::dns_lookup(req)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_ssl_cert_info(req: test_tools::SslCertInfoRequest) -> Result<test_tools::SslCertInfoResult, String> {
    test_tools::get_ssl_cert_info(req)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn generate_self_signed_cert(req: test_tools::SelfSignedCertRequest) -> Result<test_tools::SelfSignedCertResult, String> {
    test_tools::generate_self_signed_cert(req)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn scan_ports(req: test_tools::PortScanRequest) -> Result<test_tools::PortScanResult, String> {
    test_tools::scan_ports(req)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn encode_decode(req: test_tools::EncodeDecodeRequest) -> Result<test_tools::EncodeDecodeResult, String> {
    test_tools::encode_decode(req)
        .map_err(|e| e.to_string())
}
