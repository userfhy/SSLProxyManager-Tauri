use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::proxy::ws_proxy;

// 为 Config 派生 PartialEq，用于配置变更检测
impl PartialEq for Config {
    fn eq(&self, other: &Self) -> bool {
        // 比较所有关键字段
        self.rules == other.rules
            && self.ws_proxy_enabled == other.ws_proxy_enabled
            && self.ws_proxy == other.ws_proxy
            && self.stream.enabled == other.stream.enabled
            && self.stream.upstreams == other.stream.upstreams
            && self.stream.servers == other.stream.servers
            && self.http_access_control_enabled == other.http_access_control_enabled
            && self.ws_access_control_enabled == other.ws_access_control_enabled
            && self.stream_access_control_enabled == other.stream_access_control_enabled
            && self.allow_all_lan == other.allow_all_lan
            && self.allow_all_ip == other.allow_all_ip
            && self.whitelist == other.whitelist
            && self.enable_http2 == other.enable_http2
            && self.compression_enabled == other.compression_enabled
            && self.max_body_size == other.max_body_size
            && self.system_metrics_sample_interval_secs == other.system_metrics_sample_interval_secs
            && self.system_metrics_persistence_enabled == other.system_metrics_persistence_enabled
            && self.alerting == other.alerting
    }
}

// 为其他结构体派生 PartialEq
impl PartialEq for ListenRule {
    fn eq(&self, other: &Self) -> bool {
        self.enabled == other.enabled
            && self.listen_addr == other.listen_addr
            && self.listen_addrs == other.listen_addrs
            && self.ssl_enable == other.ssl_enable
            && self.cert_file == other.cert_file
            && self.key_file == other.key_file
            && self.routes == other.routes
    }
}

impl PartialEq for Route {
    fn eq(&self, other: &Self) -> bool {
        self.enabled == other.enabled
            && self.host == other.host
            && self.path == other.path
            && self.upstreams == other.upstreams
    }
}

impl PartialEq for Upstream {
    fn eq(&self, other: &Self) -> bool {
        self.url == other.url && self.weight == other.weight
    }
}

impl PartialEq for WhitelistEntry {
    fn eq(&self, other: &Self) -> bool {
        self.ip == other.ip
    }
}

impl PartialEq for StreamProxyConfig {
    fn eq(&self, other: &Self) -> bool {
        self.enabled == other.enabled
            && self.upstreams == other.upstreams
            && self.servers == other.servers
    }
}

impl PartialEq for StreamUpstream {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.servers == other.servers
    }
}

impl PartialEq for StreamUpstreamServer {
    fn eq(&self, other: &Self) -> bool {
        self.addr == other.addr && self.weight == other.weight
    }
}

impl PartialEq for StreamServer {
    fn eq(&self, other: &Self) -> bool {
        self.enabled == other.enabled
            && self.listen_addr == other.listen_addr
            && self.proxy_pass == other.proxy_pass
    }
}

// 默认 true 帮助函数，供 serde 使用
fn default_true() -> bool {
    true
}

fn default_route_enabled() -> bool {
    true
}

fn default_max_body_size() -> usize {
    10 * 1024 * 1024
}

fn default_enable_http2() -> bool {
    true
}

fn default_ws_proxy_enabled() -> bool {
    true
}

fn default_http_access_control_enabled() -> bool {
    true
}

fn default_ws_access_control_enabled() -> bool {
    true
}

fn default_stream_access_control_enabled() -> bool {
    true
}

// 新增的默认值函数
fn default_upstream_connect_timeout_ms() -> u64 {
    3000 // 从 5000 降低到 3000，更快失败
}
fn default_upstream_read_timeout_ms() -> u64 {
    30000
}
fn default_upstream_pool_max_idle() -> usize {
    200 // 从 100 增加到 200，提升高并发性能
}
fn default_upstream_pool_idle_timeout_sec() -> u64 {
    90 // 从 60 增加到 90，减少连接重建
}
fn default_max_response_body_size() -> usize {
    10 * 1024 * 1024
}

fn default_follow_redirects() -> bool {
    false
}

fn default_compression_enabled() -> bool {
    false
}

fn default_compression_gzip() -> bool {
    true
}

fn default_compression_brotli() -> bool {
    true
}

fn default_compression_min_length() -> usize {
    1024
}

fn default_compression_gzip_level() -> u32 {
    6
}

fn default_compression_brotli_level() -> u32 {
    6
}

fn default_system_metrics_sample_interval_secs() -> i64 {
    10
}

fn default_system_metrics_persistence_enabled() -> bool {
    true
}

fn default_quiet_hours_start() -> String {
    "23:00".to_string()
}

fn default_quiet_hours_end() -> String {
    "08:00".to_string()
}

fn default_system_report_interval_minutes() -> u32 {
    60
}

fn default_system_report_weekdays() -> Vec<u8> {
    vec![1, 2, 3, 4, 5, 6, 7]
}

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhitelistEntry {
    pub ip: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Upstream {
    pub url: String,
    pub weight: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    #[serde(default = "default_route_enabled")]
    pub enabled: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_pass_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub set_headers: Option<std::collections::HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub static_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_basic_auth: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub basic_auth_enable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub basic_auth_username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub basic_auth_password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub basic_auth_forward_header: Option<bool>,

    #[serde(default = "default_follow_redirects")]
    pub follow_redirects: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression_gzip: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression_brotli: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression_min_length: Option<usize>,

    // 请求/响应修改配置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url_rewrite_rules: Option<Vec<UrlRewriteRule>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_body_replace: Option<Vec<BodyReplaceRule>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_body_replace: Option<Vec<BodyReplaceRule>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remove_headers: Option<Vec<String>>,

    // 路由匹配增强（兼容 Nginx 风格）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub methods: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::HashMap<String, String>>,

    pub upstreams: Vec<Upstream>,
}

/// URL 重写规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrlRewriteRule {
    /// 正则表达式模式
    pub pattern: String,
    /// 替换字符串（支持 $1, $2 等捕获组）
    pub replacement: String,
    /// 是否启用
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// 请求/响应体替换规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BodyReplaceRule {
    /// 查找的文本（支持正则表达式）
    pub find: String,
    /// 替换的文本（支持 $1, $2 等捕获组）
    pub replace: String,
    /// 是否使用正则表达式
    #[serde(default)]
    pub use_regex: bool,
    /// 是否启用
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// （可选）仅对指定的 Content-Type 生效；支持逗号分隔的多值，例如 "text/html,application/json"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_types: Option<String>,
    /// 预编译正则（运行时字段，不参与配置序列化）
    #[serde(skip)]
    pub compiled_regex: Option<Arc<Regex>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListenRule {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    #[serde(default = "default_true")]
    pub enabled: bool,

    /// 兼容旧字段：单个监听地址
    pub listen_addr: String,
    /// 新字段：多个监听地址（如果为空，则回退到 listen_addr）
    #[serde(default)]
    pub listen_addrs: Vec<String>,
    pub ssl_enable: bool,
    pub cert_file: String,
    pub key_file: String,
    pub basic_auth_enable: bool,
    pub basic_auth_username: String,
    pub basic_auth_password: String,
    pub basic_auth_forward_header: bool,
    pub routes: Vec<Route>,

    // 速率限制配置（可选，每个规则独立配置）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit_requests_per_second: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit_burst_size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit_window_seconds: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit_ban_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsStorage {
    pub enabled: bool,
    pub db_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UpdateConfig {
    pub enabled: bool,
    pub server_url: String,
    pub auto_check: bool,
    pub timeout_ms: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    pub ignore_prerelease: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct AlertRulesConfig {
    #[serde(default = "default_true")]
    pub server_start_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlertWebhookConfig {
    pub enabled: bool,
    pub provider: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
    #[serde(default)]
    pub system_report_enabled: bool,
    #[serde(default)]
    pub quiet_hours_enabled: bool,
    #[serde(default = "default_quiet_hours_start")]
    pub quiet_hours_start: String,
    #[serde(default = "default_quiet_hours_end")]
    pub quiet_hours_end: String,
    #[serde(default = "default_system_report_interval_minutes")]
    pub system_report_interval_minutes: u32,
    #[serde(default = "default_system_report_weekdays")]
    pub system_report_weekdays: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlertingConfig {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhook: Option<AlertWebhookConfig>,
    #[serde(default)]
    pub rules: AlertRulesConfig,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfigSnapshotInfo {
    pub name: String,
    pub created_at_unix_ms: i64,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamUpstreamServer {
    pub addr: String,
    #[serde(default = "default_stream_weight")]
    pub weight: i32,
    #[serde(default = "default_stream_max_fails")]
    pub max_fails: i32,
    #[serde(default = "default_stream_fail_timeout")]
    pub fail_timeout: String,
}

fn default_stream_weight() -> i32 {
    1
}

fn default_stream_max_fails() -> i32 {
    1
}

fn default_stream_fail_timeout() -> String {
    "30s".to_string()
}

fn default_stream_hash_key() -> String {
    "$remote_addr".to_string()
}

fn default_stream_consistent() -> bool {
    true
}

fn default_stream_proxy_connect_timeout() -> String {
    "300s".to_string()
}

fn default_stream_proxy_timeout() -> String {
    "600s".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamUpstream {
    pub name: String,
    #[serde(default = "default_stream_hash_key")]
    pub hash_key: String,
    #[serde(default = "default_stream_consistent")]
    pub consistent: bool,
    pub servers: Vec<StreamUpstreamServer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamServer {
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listen_port: Option<u16>,
    pub proxy_pass: String,

    #[serde(default = "default_stream_proxy_connect_timeout")]
    pub proxy_connect_timeout: String,

    #[serde(default = "default_stream_proxy_timeout")]
    pub proxy_timeout: String,

    #[serde(default)]
    pub udp: bool,

    /// 监听地址，支持 IPv4 和 IPv6。如果未指定，默认使用 127.0.0.1（仅本机回环）
    /// 示例: "127.0.0.1:8080", "0.0.0.0:8080", "[::]:8080"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listen_addr: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StreamProxyConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub upstreams: Vec<StreamUpstream>,
    #[serde(default)]
    pub servers: Vec<StreamServer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub rules: Vec<ListenRule>,

    #[serde(default = "default_ws_proxy_enabled")]
    pub ws_proxy_enabled: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ws_proxy: Option<Vec<ws_proxy::WsListenRule>>,

    #[serde(default)]
    pub stream: StreamProxyConfig,

    #[serde(default = "default_http_access_control_enabled")]
    pub http_access_control_enabled: bool,
    #[serde(default = "default_ws_access_control_enabled")]
    pub ws_access_control_enabled: bool,
    #[serde(default = "default_stream_access_control_enabled")]
    pub stream_access_control_enabled: bool,

    pub allow_all_lan: bool,
    #[serde(default)]
    pub allow_all_ip: bool,
    pub whitelist: Vec<WhitelistEntry>,

    #[serde(default)]
    pub auto_start: bool,

    #[serde(default)]
    pub show_realtime_logs: bool,

    #[serde(default)]
    pub realtime_logs_only_errors: bool,

    #[serde(default = "default_true")]
    pub stream_proxy: bool,

    #[serde(default = "default_max_body_size")]
    pub max_body_size: usize,

    #[serde(default = "default_max_response_body_size")]
    pub max_response_body_size: usize,

    #[serde(default = "default_upstream_connect_timeout_ms")]
    pub upstream_connect_timeout_ms: u64,

    #[serde(default = "default_upstream_read_timeout_ms")]
    pub upstream_read_timeout_ms: u64,

    #[serde(default = "default_upstream_pool_max_idle")]
    pub upstream_pool_max_idle: usize,

    #[serde(default = "default_upstream_pool_idle_timeout_sec")]
    pub upstream_pool_idle_timeout_sec: u64,

    #[serde(default = "default_enable_http2")]
    pub enable_http2: bool,

    #[serde(default = "default_compression_enabled")]
    pub compression_enabled: bool,
    #[serde(default = "default_compression_gzip")]
    pub compression_gzip: bool,
    #[serde(default = "default_compression_brotli")]
    pub compression_brotli: bool,
    #[serde(default = "default_compression_min_length")]
    pub compression_min_length: usize,
    #[serde(default = "default_compression_gzip_level")]
    pub compression_gzip_level: u32,
    #[serde(default = "default_compression_brotli_level")]
    pub compression_brotli_level: u32,
    #[serde(default = "default_system_metrics_sample_interval_secs")]
    pub system_metrics_sample_interval_secs: i64,
    #[serde(default = "default_system_metrics_persistence_enabled")]
    pub system_metrics_persistence_enabled: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics_storage: Option<MetricsStorage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update: Option<UpdateConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alerting: Option<AlertingConfig>,
}

static CONFIG: Lazy<RwLock<Config>> = Lazy::new(|| {
    RwLock::new(Config {
        rules: vec![],
        ws_proxy_enabled: default_ws_proxy_enabled(),
        ws_proxy: None,
        stream: StreamProxyConfig::default(),
        http_access_control_enabled: default_http_access_control_enabled(),
        ws_access_control_enabled: default_ws_access_control_enabled(),
        stream_access_control_enabled: default_stream_access_control_enabled(),
        allow_all_lan: true,
        allow_all_ip: false,
        whitelist: vec![],
        auto_start: false,
        show_realtime_logs: true,
        realtime_logs_only_errors: false,
        stream_proxy: true,
        max_body_size: default_max_body_size(),
        max_response_body_size: default_max_response_body_size(),
        upstream_connect_timeout_ms: default_upstream_connect_timeout_ms(),
        upstream_read_timeout_ms: default_upstream_read_timeout_ms(),
        upstream_pool_max_idle: default_upstream_pool_max_idle(),
        upstream_pool_idle_timeout_sec: default_upstream_pool_idle_timeout_sec(),
        enable_http2: default_enable_http2(),
        compression_enabled: default_compression_enabled(),
        compression_gzip: default_compression_gzip(),
        compression_brotli: default_compression_brotli(),
        compression_min_length: default_compression_min_length(),
        compression_gzip_level: default_compression_gzip_level(),
        compression_brotli_level: default_compression_brotli_level(),
        system_metrics_sample_interval_secs: default_system_metrics_sample_interval_secs(),
        system_metrics_persistence_enabled: default_system_metrics_persistence_enabled(),
        metrics_storage: None,
        update: None,
        alerting: None,
    })
});

fn default_config() -> Config {
    Config {
        rules: vec![],
        ws_proxy_enabled: default_ws_proxy_enabled(),
        ws_proxy: None,
        stream: StreamProxyConfig::default(),
        http_access_control_enabled: default_http_access_control_enabled(),
        ws_access_control_enabled: default_ws_access_control_enabled(),
        stream_access_control_enabled: default_stream_access_control_enabled(),
        allow_all_lan: true,
        allow_all_ip: false,
        whitelist: vec![],
        auto_start: false,
        show_realtime_logs: true,
        realtime_logs_only_errors: false,
        stream_proxy: true,
        max_body_size: default_max_body_size(),
        max_response_body_size: default_max_response_body_size(),
        upstream_connect_timeout_ms: default_upstream_connect_timeout_ms(),
        upstream_read_timeout_ms: default_upstream_read_timeout_ms(),
        upstream_pool_max_idle: default_upstream_pool_max_idle(),
        upstream_pool_idle_timeout_sec: default_upstream_pool_idle_timeout_sec(),
        enable_http2: default_enable_http2(),
        compression_enabled: default_compression_enabled(),
        compression_gzip: default_compression_gzip(),
        compression_brotli: default_compression_brotli(),
        compression_min_length: default_compression_min_length(),
        compression_gzip_level: default_compression_gzip_level(),
        compression_brotli_level: default_compression_brotli_level(),
        system_metrics_sample_interval_secs: default_system_metrics_sample_interval_secs(),
        system_metrics_persistence_enabled: default_system_metrics_persistence_enabled(),
        metrics_storage: None,
        update: None,
        alerting: None,
    }
}

fn snapshots_dir(config_path: &PathBuf) -> Result<PathBuf> {
    let parent = config_path
        .parent()
        .context("配置路径缺少父目录，无法创建快照目录")?;
    Ok(parent.join("config_snapshots"))
}

fn create_config_snapshot_if_exists(config_path: &PathBuf) -> Result<()> {
    if !config_path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(config_path)
        .with_context(|| format!("读取现有配置文件失败: {}", config_path.display()))?;

    let dir = snapshots_dir(config_path)?;
    fs::create_dir_all(&dir).with_context(|| format!("创建快照目录失败: {}", dir.display()))?;

    let now = chrono::Local::now();
    let name = format!("config-{}-{}.toml", now.format("%Y%m%d-%H%M%S"), now.timestamp_millis());
    let snap_path = dir.join(name);
    fs::write(&snap_path, content)
        .with_context(|| format!("写入配置快照失败: {}", snap_path.display()))?;

    prune_old_snapshots(config_path, 20)?;
    Ok(())
}

fn prune_old_snapshots(config_path: &PathBuf, keep: usize) -> Result<()> {
    let dir = snapshots_dir(config_path)?;
    if !dir.exists() {
        return Ok(());
    }

    let mut entries: Vec<_> = fs::read_dir(&dir)
        .with_context(|| format!("读取快照目录失败: {}", dir.display()))?
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            if path.extension().and_then(|v| v.to_str()) != Some("toml") {
                return None;
            }
            let meta = e.metadata().ok()?;
            let modified = meta.modified().ok().unwrap_or(SystemTime::UNIX_EPOCH);
            Some((path, modified))
        })
        .collect();

    entries.sort_by(|a, b| b.1.cmp(&a.1));
    if entries.len() <= keep {
        return Ok(());
    }

    for (path, _) in entries.into_iter().skip(keep) {
        let _ = fs::remove_file(path);
    }
    Ok(())
}

pub(crate) fn get_config_path() -> Result<PathBuf> {
    // 开发模式优先读取当前工作目录下的 config.toml（便于调试时直接改项目根目录配置）
    #[cfg(debug_assertions)]
    {
        let cwd_cfg = PathBuf::from("config.toml");
        if cwd_cfg.exists() {
            return Ok(cwd_cfg);
        }
    }

    // 生产模式：按平台选择默认配置位置

    // Linux: ~/.config/SSLProxyManager/config.toml
    #[cfg(target_os = "linux")]
    {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .context("无法确定配置目录（缺少 XDG_CONFIG_HOME/HOME）")?;
        return Ok(base.join("SSLProxyManager").join("config.toml"));
    }

    // Windows: 优先读取可执行文件同目录（安装目录旁）的 config.toml；
    // 若不存在，则使用用户目录下的配置文件（首次启动会自动生成）。
    #[cfg(target_os = "windows")]
    {
        let exe = std::env::current_exe().context("无法获取当前可执行文件路径")?;
        let dir = exe.parent().context("无法获取可执行文件目录")?;
        let install_cfg = dir.join("config.toml");
        if install_cfg.exists() {
            return Ok(install_cfg);
        }

        // Windows 用户目录：%APPDATA%\SSLProxyManager\config.toml
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("USERPROFILE")
                    .map(|p| PathBuf::from(p).join("AppData").join("Roaming"))
            })
            .context("无法确定配置目录（缺少 APPDATA/USERPROFILE）")?;
        return Ok(base.join("SSLProxyManager").join("config.toml"));
    }

    // macOS: ~/Library/Application Support/SSLProxyManager/config.toml
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .context("无法确定配置目录（缺少 HOME）")?;
        return Ok(home
            .join("Library")
            .join("Application Support")
            .join("SSLProxyManager")
            .join("config.toml"));
    }

    // 其它：暂时沿用可执行文件同目录
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        let exe = std::env::current_exe().context("无法获取当前可执行文件路径")?;
        let dir = exe.parent().context("无法获取可执行文件目录")?;
        return Ok(dir.join("config.toml"));
    }
}

fn ensure_config_file_exists(path: &PathBuf) -> Result<()> {
    if path.exists() {
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("创建配置目录失败: {}", parent.display()))?;
    }

    let cfg = default_config();
    let content = toml::to_string_pretty(&cfg).context("序列化默认配置失败")?;
    fs::write(path, content).with_context(|| format!("写入默认配置失败: {}", path.display()))?;
    Ok(())
}

pub fn load_config() -> Result<()> {
    let path = get_config_path()?;

    // 如果不存在则自动生成
    ensure_config_file_exists(&path)?;

    let content = fs::read_to_string(&path)
        .with_context(|| format!("无法读取配置文件: {}", path.display()))?;

    let mut config: Config = toml::from_str(&content).context("解析配置文件失败")?;

    // 确保所有 ID 都存在（加载时补齐，并写回内存）
    ensure_config_ids(&mut config);
    normalize_alerting_config(&mut config.alerting);

    // 预编译所有正则表达式以提升运行时性能
    precompile_regexes(&mut config);

    *CONFIG.write() = config;
    Ok(())
}

/// 预编译配置中的所有正则表达式，提升运行时性能
fn precompile_regexes(config: &mut Config) {
    for rule in &mut config.rules {
        for route in &mut rule.routes {
            // 预编译 URL 重写规则
            if let Some(rewrite_rules) = &route.url_rewrite_rules {
                for rule in rewrite_rules {
                    let _ = crate::proxy::cached_regex(&rule.pattern);
                }
            }

            // 预编译请求体替换规则
            if let Some(replace_rules) = route.request_body_replace.as_mut() {
                for rule in replace_rules {
                    if rule.enabled {
                        if let Some(content_types) = rule.content_types.as_deref() {
                            let _ = crate::proxy::cached_content_types(content_types);
                        }
                    }

                    rule.compiled_regex = if rule.use_regex && rule.enabled {
                        crate::proxy::cached_regex(&rule.find)
                    } else {
                        None
                    };
                }
            }

            // 预编译响应体替换规则
            if let Some(replace_rules) = route.response_body_replace.as_mut() {
                for rule in replace_rules {
                    if rule.enabled {
                        if let Some(content_types) = rule.content_types.as_deref() {
                            let _ = crate::proxy::cached_content_types(content_types);
                        }
                    }

                    rule.compiled_regex = if rule.use_regex && rule.enabled {
                        crate::proxy::cached_regex(&rule.find)
                    } else {
                        None
                    };
                }
            }

            // header 通配匹配改为轻量通配逻辑，无需预编译 regex
        }
    }
}

pub fn save_config() -> Result<()> {
    let path = get_config_path()?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("创建配置目录失败: {}", parent.display()))?;
    }

    create_config_snapshot_if_exists(&path)?;

    let config = CONFIG.read().clone();
    let content = toml::to_string_pretty(&config).context("序列化配置失败")?;
    fs::write(&path, content).with_context(|| format!("写入配置文件失败: {}", path.display()))?;
    Ok(())
}

pub fn list_config_snapshots() -> Result<Vec<ConfigSnapshotInfo>> {
    let path = get_config_path()?;
    let dir = snapshots_dir(&path)?;
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut out: Vec<ConfigSnapshotInfo> = fs::read_dir(&dir)
        .with_context(|| format!("读取快照目录失败: {}", dir.display()))?
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            if path.extension().and_then(|v| v.to_str()) != Some("toml") {
                return None;
            }
            let meta = e.metadata().ok()?;
            let name = path.file_name()?.to_string_lossy().to_string();
            let ms = meta
                .modified()
                .ok()
                .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);

            Some(ConfigSnapshotInfo {
                name,
                created_at_unix_ms: ms,
                size_bytes: meta.len(),
            })
        })
        .collect();

    out.sort_by(|a, b| b.created_at_unix_ms.cmp(&a.created_at_unix_ms));
    Ok(out)
}

pub fn load_config_snapshot(name: &str) -> Result<Config> {
    let path = get_config_path()?;
    let dir = snapshots_dir(&path)?;
    let file = dir.join(name);

    let content = fs::read_to_string(&file)
        .with_context(|| format!("读取配置快照失败: {}", file.display()))?;
    let mut config: Config = toml::from_str(&content).context("解析配置快照失败")?;

    ensure_config_ids(&mut config);
    normalize_alerting_config(&mut config.alerting);
    precompile_regexes(&mut config);
    Ok(config)
}

pub fn get_config() -> Config {
    CONFIG.read().clone()
}

/// 仅返回实时日志开关，避免在热路径 clone 整个 Config。
#[inline]
pub fn show_realtime_logs_enabled() -> bool {
    CONFIG.read().show_realtime_logs
}

/// 返回实时日志相关开关，避免在热路径 clone 整个 Config。
#[inline]
pub fn realtime_logs_settings() -> (bool, bool) {
    let cfg = CONFIG.read();
    (cfg.show_realtime_logs, cfg.realtime_logs_only_errors)
}

pub fn set_config(config: Config) {
    *CONFIG.write() = config;
}

pub fn ensure_config_ids_for_save(config: &mut Config) {
    ensure_config_ids(config);
}

pub fn normalize_alerting_config(alerting: &mut Option<AlertingConfig>) {
    let Some(alerting) = alerting.as_mut() else {
        return;
    };

    let Some(webhook) = alerting.webhook.as_mut() else {
        return;
    };

    webhook.provider = webhook.provider.trim().to_string();
    webhook.url = webhook.url.trim().to_string();
    webhook.secret = webhook
        .secret
        .take()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());
    webhook.quiet_hours_start = webhook.quiet_hours_start.trim().to_string();
    webhook.quiet_hours_end = webhook.quiet_hours_end.trim().to_string();

    let weekdays = webhook
        .system_report_weekdays
        .iter()
        .copied()
        .filter(|day| (1..=7).contains(day))
        .collect::<std::collections::BTreeSet<_>>();
    webhook.system_report_weekdays = weekdays.into_iter().collect();
}

pub fn validate_alerting_config(
    alerting: &Option<AlertingConfig>,
) -> std::result::Result<(), String> {
    let Some(alerting) = alerting.as_ref() else {
        return Ok(());
    };

    let Some(webhook) = alerting.webhook.as_ref() else {
        return Ok(());
    };

    if webhook.enabled && webhook.url.trim().is_empty() {
        return Err("Webhook URL is empty".to_string());
    }

    if !(1..=10080).contains(&webhook.system_report_interval_minutes) {
        return Err("system_report_interval_minutes must be an integer between 1 and 10080".to_string());
    }

    if webhook.system_report_weekdays.is_empty() {
        return Err("system_report_weekdays must contain at least one weekday".to_string());
    }

    if webhook
        .system_report_weekdays
        .iter()
        .any(|day| !(1..=7).contains(day))
    {
        return Err("system_report_weekdays must only contain integers from 1 to 7".to_string());
    }

    validate_time_hhmm(&webhook.quiet_hours_start, "quiet_hours_start")?;
    validate_time_hhmm(&webhook.quiet_hours_end, "quiet_hours_end")?;

    if webhook.quiet_hours_enabled && webhook.quiet_hours_start == webhook.quiet_hours_end {
        return Err("quiet_hours_start and quiet_hours_end cannot be the same when quiet hours are enabled".to_string());
    }

    Ok(())
}

fn validate_time_hhmm(value: &str, field_name: &str) -> std::result::Result<(), String> {
    let trimmed = value.trim();
    let mut parts = trimmed.split(':');
    let hour = parts
        .next()
        .ok_or_else(|| format!("{field_name} must be in HH:MM format"))?
        .parse::<u32>()
        .map_err(|_| format!("{field_name} must be in HH:MM format"))?;
    let minute = parts
        .next()
        .ok_or_else(|| format!("{field_name} must be in HH:MM format"))?
        .parse::<u32>()
        .map_err(|_| format!("{field_name} must be in HH:MM format"))?;

    if parts.next().is_some() || hour > 23 || minute > 59 {
        return Err(format!("{field_name} must be in HH:MM format"));
    }

    if trimmed.len() != 5 || !trimmed.as_bytes().get(2).is_some_and(|ch| *ch == b':') {
        return Err(format!("{field_name} must be in HH:MM format"));
    }

    Ok(())
}

fn ensure_config_ids(config: &mut Config) {
    use uuid::Uuid;

    let need_new = |v: &Option<String>| match v {
        None => true,
        Some(s) => s.trim().is_empty(),
    };

    for rule in &mut config.rules {
        if need_new(&rule.id) {
            rule.id = Some(Uuid::new_v4().to_string());
        }
        for route in &mut rule.routes {
            if need_new(&route.id) {
                route.id = Some(Uuid::new_v4().to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ensure_config_ids_for_save, normalize_alerting_config, validate_alerting_config,
        AlertRulesConfig, AlertWebhookConfig, AlertingConfig, BodyReplaceRule, Config, ListenRule,
        Route, StreamProxyConfig, Upstream, WhitelistEntry,
    };

    fn sample_route() -> Route {
        Route {
            id: None,
            enabled: true,
            host: Some("example.com".into()),
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
            request_body_replace: Some(vec![BodyReplaceRule {
                find: "foo".into(),
                replace: "bar".into(),
                use_regex: false,
                enabled: true,
                content_types: Some(" text/html , application/json ".into()),
                compiled_regex: None,
            }]),
            response_body_replace: None,
            remove_headers: None,
            methods: None,
            headers: None,
            upstreams: vec![Upstream {
                url: "http://127.0.0.1:8080".into(),
                weight: 1,
            }],
        }
    }

    fn sample_config_for_ids() -> Config {
        Config {
            rules: vec![ListenRule {
                id: Some("  ".into()),
                enabled: true,
                listen_addr: "127.0.0.1:8443".into(),
                listen_addrs: vec![],
                ssl_enable: false,
                cert_file: String::new(),
                key_file: String::new(),
                basic_auth_enable: false,
                basic_auth_username: String::new(),
                basic_auth_password: String::new(),
                basic_auth_forward_header: false,
                routes: vec![
                    sample_route(),
                    Route {
                        id: Some("route-keep".into()),
                        ..sample_route()
                    },
                ],
                rate_limit_enabled: None,
                rate_limit_requests_per_second: None,
                rate_limit_burst_size: None,
                rate_limit_window_seconds: None,
                rate_limit_ban_seconds: None,
            }],
            ws_proxy_enabled: true,
            ws_proxy: None,
            stream: StreamProxyConfig::default(),
            http_access_control_enabled: true,
            ws_access_control_enabled: true,
            stream_access_control_enabled: true,
            allow_all_lan: true,
            allow_all_ip: false,
            whitelist: vec![WhitelistEntry {
                ip: "127.0.0.1".into(),
            }],
            auto_start: false,
            show_realtime_logs: true,
            realtime_logs_only_errors: false,
            stream_proxy: true,
            max_body_size: 1024,
            max_response_body_size: 2048,
            upstream_connect_timeout_ms: 3000,
            upstream_read_timeout_ms: 30000,
            upstream_pool_max_idle: 10,
            upstream_pool_idle_timeout_sec: 30,
            enable_http2: true,
            compression_enabled: false,
            compression_gzip: true,
            compression_brotli: true,
            compression_min_length: 1024,
            compression_gzip_level: 6,
            compression_brotli_level: 6,
            system_metrics_sample_interval_secs: 10,
            system_metrics_persistence_enabled: true,
            metrics_storage: None,
            update: None,
            alerting: None,
        }
    }

    fn sample_alerting() -> Option<AlertingConfig> {
        Some(AlertingConfig {
            enabled: true,
            webhook: Some(AlertWebhookConfig {
                enabled: true,
                provider: "  slack  ".into(),
                url: "  https://example.com/hook  ".into(),
                secret: Some("   secret-token   ".into()),
                system_report_enabled: true,
                quiet_hours_enabled: true,
                quiet_hours_start: " 23:00 ".into(),
                quiet_hours_end: " 08:00 ".into(),
                system_report_interval_minutes: 60,
                system_report_weekdays: vec![7, 1, 3, 3, 0, 8],
            }),
            rules: AlertRulesConfig {
                server_start_error: true,
            },
        })
    }

    #[test]
    fn ensure_config_ids_for_save_fills_missing_or_blank_ids() {
        let mut cfg = sample_config_for_ids();

        ensure_config_ids_for_save(&mut cfg);

        let rule = &cfg.rules[0];
        assert!(rule.id.as_ref().is_some_and(|id| !id.trim().is_empty()));
        assert!(rule.routes[0]
            .id
            .as_ref()
            .is_some_and(|id| !id.trim().is_empty()));
        assert_eq!(rule.routes[1].id.as_deref(), Some("route-keep"));
    }

    #[test]
    fn normalize_alerting_config_trims_secret_and_deduplicates_weekdays() {
        let mut alerting = sample_alerting();

        normalize_alerting_config(&mut alerting);

        let webhook = alerting.unwrap().webhook.unwrap();
        assert_eq!(webhook.provider, "slack");
        assert_eq!(webhook.url, "https://example.com/hook");
        assert_eq!(webhook.secret.as_deref(), Some("secret-token"));
        assert_eq!(webhook.quiet_hours_start, "23:00");
        assert_eq!(webhook.quiet_hours_end, "08:00");
        assert_eq!(webhook.system_report_weekdays, vec![1, 3, 7]);
    }

    #[test]
    fn normalize_alerting_config_drops_empty_secret() {
        let mut alerting = sample_alerting();
        alerting
            .as_mut()
            .unwrap()
            .webhook
            .as_mut()
            .unwrap()
            .secret = Some("   ".into());

        normalize_alerting_config(&mut alerting);

        assert!(alerting.unwrap().webhook.unwrap().secret.is_none());
    }

    #[test]
    fn validate_alerting_config_accepts_normalized_valid_webhook() {
        let mut alerting = sample_alerting();
        normalize_alerting_config(&mut alerting);

        validate_alerting_config(&alerting).unwrap();
    }

    #[test]
    fn validate_alerting_config_rejects_empty_url_when_enabled() {
        let mut alerting = sample_alerting();
        alerting.as_mut().unwrap().webhook.as_mut().unwrap().url = " ".into();
        normalize_alerting_config(&mut alerting);

        let err = validate_alerting_config(&alerting).unwrap_err();
        assert!(err.contains("Webhook URL is empty"));
    }

    #[test]
    fn validate_alerting_config_rejects_invalid_weekday_values() {
        let mut alerting = sample_alerting();
        alerting
            .as_mut()
            .unwrap()
            .webhook
            .as_mut()
            .unwrap()
            .system_report_weekdays = vec![1, 9];

        let err = validate_alerting_config(&alerting).unwrap_err();
        assert!(err.contains("only contain integers from 1 to 7"));
    }

    #[test]
    fn validate_alerting_config_rejects_equal_quiet_hours_when_enabled() {
        let mut alerting = sample_alerting();
        let webhook = alerting.as_mut().unwrap().webhook.as_mut().unwrap();
        webhook.quiet_hours_start = "09:30".into();
        webhook.quiet_hours_end = "09:30".into();
        normalize_alerting_config(&mut alerting);

        let err = validate_alerting_config(&alerting).unwrap_err();
        assert!(err.contains("cannot be the same"));
    }

    #[test]
    fn validate_alerting_config_rejects_invalid_time_format() {
        let mut alerting = sample_alerting();
        alerting
            .as_mut()
            .unwrap()
            .webhook
            .as_mut()
            .unwrap()
            .quiet_hours_start = "9:30".into();
        normalize_alerting_config(&mut alerting);

        let err = validate_alerting_config(&alerting).unwrap_err();
        assert!(err.contains("quiet_hours_start must be in HH:MM format"));
    }
}
