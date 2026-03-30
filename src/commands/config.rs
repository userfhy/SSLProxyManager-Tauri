use crate::config;
use crate::proxy::stream_proxy;
use crate::system_metrics;
use anyhow::Result;

pub async fn validate_config(cfg: &config::Config) -> Result<(), String> {
    for rule in &cfg.rules {
        if !rule.enabled || !rule.ssl_enable {
            continue;
        }
        if rule.cert_file.trim().is_empty() || rule.key_file.trim().is_empty() {
            return Err(format!(
                "Listen rule ({}) has SSL enabled, but certificate or private key path is empty",
                rule.listen_addr
            ));
        }
        axum_server::tls_rustls::RustlsConfig::from_pem_file(
            rule.cert_file.clone(),
            rule.key_file.clone(),
        )
        .await
        .map_err(|e| {
            format!(
                "Failed to load TLS certificate for listen rule ({}): {e}",
                rule.listen_addr
            )
        })?;
    }

    if let Some(ws_rules) = &cfg.ws_proxy {
        for rule in ws_rules {
            if !rule.enabled || !rule.ssl_enable {
                continue;
            }
            if rule.cert_file.trim().is_empty() || rule.key_file.trim().is_empty() {
                return Err(format!(
                    "WS rule ({}) has SSL enabled, but certificate or private key path is empty",
                    rule.listen_addr
                ));
            }
            axum_server::tls_rustls::RustlsConfig::from_pem_file(
                rule.cert_file.clone(),
                rule.key_file.clone(),
            )
            .await
            .map_err(|e| {
                format!(
                    "Failed to load TLS certificate for WS rule ({}): {e}",
                    rule.listen_addr
                )
            })?;
        }
    }

    if cfg.stream.enabled {
        stream_proxy::validate_stream_config(&cfg.stream).map_err(|e| e.to_string())?;
    }

    config::validate_alerting_config(&cfg.alerting)?;

    Ok(())
}

#[tauri::command]
pub fn get_config() -> Result<config::Config, String> {
    Ok(config::get_config())
}

#[tauri::command]
pub fn list_config_snapshots() -> Result<Vec<config::ConfigSnapshotInfo>, String> {
    config::list_config_snapshots().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn restore_config_snapshot(
    app: tauri::AppHandle,
    snapshot_name: String,
) -> Result<config::Config, String> {
    let cfg = config::load_config_snapshot(&snapshot_name).map_err(|e| e.to_string())?;
    validate_config(&cfg).await?;

    let saved_cfg = crate::hot_reload::graceful_reload(app, cfg)
        .await
        .map_err(|e| e.to_string())?;
    system_metrics::refresh_sample_interval_from_config();
    Ok(saved_cfg)
}

#[tauri::command]
pub async fn send_test_alert(cfg: config::AlertingConfig) -> Result<(), String> {
    let mut alerting = Some(cfg);
    config::normalize_alerting_config(&mut alerting);
    config::validate_alerting_config(&alerting)?;
    crate::alerting::send_test_alert(
        alerting.ok_or_else(|| "Webhook config is missing".to_string())?,
    )
    .await
}

#[tauri::command]
pub async fn save_config(
    app: tauri::AppHandle,
    mut cfg: config::Config,
) -> Result<config::Config, String> {
    config::ensure_config_ids_for_save(&mut cfg);
    config::normalize_alerting_config(&mut cfg.alerting);
    validate_config(&cfg).await?;

    if let Some(metrics_storage) = cfg.metrics_storage.as_ref() {
        if metrics_storage.enabled {
            crate::metrics::init_db(metrics_storage.db_path.clone())
                .await
                .map_err(|e| e.to_string())?;
            crate::metrics::init_request_log_writer().await;
        } else {
            crate::metrics::deinit_db();
        }
    } else {
        crate::metrics::deinit_db();
    }

    let saved_cfg = crate::hot_reload::graceful_reload(app, cfg)
        .await
        .map_err(|e| e.to_string())?;
    system_metrics::refresh_sample_interval_from_config();
    Ok(saved_cfg)
}

#[cfg(test)]
mod tests {
    use super::validate_config;
    use crate::config::{
        AlertRulesConfig, AlertWebhookConfig, AlertingConfig, Config, ListenRule, Route,
        StreamProxyConfig, Upstream, WhitelistEntry,
    };
    use crate::proxy::ws_proxy::{WsListenRule, WsRoute};

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

    fn sample_config() -> Config {
        Config {
            rules: vec![ListenRule {
                id: Some("rule".into()),
                enabled: true,
                listen_addr: "127.0.0.1:8080".into(),
                listen_addrs: vec![],
                ssl_enable: false,
                cert_file: String::new(),
                key_file: String::new(),
                basic_auth_enable: false,
                basic_auth_username: String::new(),
                basic_auth_password: String::new(),
                basic_auth_forward_header: false,
                routes: vec![sample_route()],
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
            max_response_body_size: 1024,
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

    #[tokio::test]
    async fn validate_config_accepts_minimal_valid_config() {
        validate_config(&sample_config()).await.unwrap();
    }

    #[tokio::test]
    async fn validate_config_rejects_http_ssl_rule_without_cert_paths() {
        let mut cfg = sample_config();
        cfg.rules[0].ssl_enable = true;

        let err = validate_config(&cfg).await.unwrap_err();
        assert!(err.contains("certificate or private key path is empty"));
    }

    #[tokio::test]
    async fn validate_config_rejects_ws_ssl_rule_without_cert_paths() {
        let mut cfg = sample_config();
        cfg.ws_proxy = Some(vec![WsListenRule {
            enabled: true,
            listen_addr: "127.0.0.1:9000".into(),
            ssl_enable: true,
            cert_file: String::new(),
            key_file: String::new(),
            routes: vec![WsRoute {
                path: "/ws".into(),
                upstream_url: "ws://backend".into(),
            }],
        }]);

        let err = validate_config(&cfg).await.unwrap_err();
        assert!(err.contains("WS rule"));
        assert!(err.contains("certificate or private key path is empty"));
    }

    #[tokio::test]
    async fn validate_config_rejects_invalid_stream_config() {
        let mut cfg = sample_config();
        cfg.stream.enabled = true;
        cfg.stream.servers = vec![crate::config::StreamServer {
            enabled: true,
            listen_port: None,
            proxy_pass: "missing".into(),
            proxy_connect_timeout: "3s".into(),
            proxy_timeout: "30s".into(),
            udp: false,
            listen_addr: Some("127.0.0.1:7000".into()),
        }];

        let err = validate_config(&cfg).await.unwrap_err();
        assert!(err.contains("references missing upstream"));
    }

    #[tokio::test]
    async fn validate_config_rejects_invalid_alerting_config() {
        let mut cfg = sample_config();
        cfg.alerting = Some(AlertingConfig {
            enabled: true,
            webhook: Some(AlertWebhookConfig {
                enabled: true,
                provider: "slack".into(),
                url: String::new(),
                secret: None,
                system_report_enabled: false,
                quiet_hours_enabled: false,
                quiet_hours_start: "23:00".into(),
                quiet_hours_end: "08:00".into(),
                system_report_interval_minutes: 60,
                system_report_weekdays: vec![1],
            }),
            rules: AlertRulesConfig {
                server_start_error: true,
            },
        });

        let err = validate_config(&cfg).await.unwrap_err();
        assert!(err.contains("Webhook URL is empty"));
    }
}
