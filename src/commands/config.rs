use crate::config;
use crate::stream_proxy;
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
