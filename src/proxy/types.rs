use std::sync::Arc;

use crate::config;

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) rule: config::ListenRule,
    pub(crate) client_follow: reqwest::Client,
    pub(crate) client_nofollow: reqwest::Client,
    pub(crate) app: tauri::AppHandle,
    pub(crate) listen_addr: Arc<str>,
    pub(crate) server_port: u16,
    pub(crate) stream_proxy: bool,
    pub(crate) max_body_size: usize,
    pub(crate) max_response_body_size: usize,
    pub(crate) http_access_control_enabled: bool,
    pub(crate) allow_all_lan: bool,
    pub(crate) allow_all_ip: bool,
    pub(crate) whitelist: Arc<[config::WhitelistEntry]>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RuleStartErrorPayload {
    pub listen_addr: String,
    pub error: String,
}
