use crate::config::{self, AlertWebhookConfig};
use serde_json::json;
use tauri::AppHandle;
use tracing::{info, warn};

fn normalize_provider(provider: &str) -> String {
    provider.trim().to_lowercase()
}

fn build_plain_text(provider: &str, title: &str, body: &str) -> serde_json::Value {
    match normalize_provider(provider).as_str() {
        "wecom" | "wechat_work" => json!({
            "msgtype": "markdown",
            "markdown": {
                "content": format!("**{}**\n{}", title, body)
            }
        }),
        "feishu" | "lark" => json!({
            "msg_type": "text",
            "content": {
                "text": format!("{}\n{}", title, body)
            }
        }),
        _ => json!({
            "msg_type": "text",
            "content": {
                "text": format!("{}\n{}", title, body)
            }
        }),
    }
}

async fn send_payload(webhook: &AlertWebhookConfig, payload: serde_json::Value) -> Result<(), String> {
    let url = webhook.url.trim();
    if url.is_empty() {
        return Err("Webhook URL is empty".to_string());
    }

    let client = reqwest::Client::new();
    let resp = client
        .post(url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Webhook request failed: {e}"))?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();

    if !status.is_success() {
        return Err(format!(
            "Webhook response not success: status={}, body={}",
            status,
            text
        ));
    }

    if normalize_provider(&webhook.provider) == "feishu" {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
            let code = v.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
            if code != 0 {
                return Err(format!("Feishu webhook returned code {}: {}", code, text));
            }
        }
    }

    if normalize_provider(&webhook.provider) == "wecom" {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
            let errcode = v.get("errcode").and_then(|c| c.as_i64()).unwrap_or(0);
            if errcode != 0 {
                return Err(format!("WeCom webhook returned errcode {}: {}", errcode, text));
            }
        }
    }

    Ok(())
}

pub async fn send_test_alert(alerting: config::AlertingConfig) -> Result<(), String> {
    if !alerting.enabled {
        return Err("Alerting is disabled".to_string());
    }
    let webhook = alerting
        .webhook
        .as_ref()
        .ok_or_else(|| "Webhook config is missing".to_string())?;
    if !webhook.enabled {
        return Err("Webhook is disabled".to_string());
    }

    let payload = build_plain_text(
        &webhook.provider,
        "SSLProxyManager 测试告警 / Test Alert",
        "这是一条测试告警消息，说明 Webhook 配置生效。",
    );

    send_payload(webhook, payload).await
}

pub fn notify_server_start_error(_app: &AppHandle, listen_addr: &str, err: &str) {
    let cfg = config::get_config();
    let Some(alerting) = cfg.alerting else {
        return;
    };

    if !alerting.enabled || !alerting.rules.server_start_error {
        return;
    }

    let Some(webhook) = alerting.webhook else {
        return;
    };

    if !webhook.enabled || webhook.url.trim().is_empty() {
        return;
    }

    let provider = webhook.provider.clone();
    let payload = build_plain_text(
        &provider,
        "SSLProxyManager 告警 / Alert",
        &format!(
            "监听地址启动失败\n地址: {}\n错误: {}\n时间: {}",
            listen_addr,
            err,
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        ),
    );

    tauri::async_runtime::spawn(async move {
        match send_payload(&webhook, payload).await {
            Ok(_) => info!("alert webhook sent: provider={}", provider),
            Err(e) => warn!("alert webhook failed: provider={}, err={}", provider, e),
        }
    });
}
