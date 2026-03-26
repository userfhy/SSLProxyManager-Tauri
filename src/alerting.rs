use crate::config::{self, AlertWebhookConfig};
use crate::system_metrics::{NetworkInterfaceStats, SystemMetricsPoint};
use chrono::Datelike;
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use serde_json::json;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::time::Duration;
use tauri::AppHandle;
use tracing::{info, warn};

static SYSTEM_REPORT_PUSHER_RUNNING: AtomicBool = AtomicBool::new(false);
static SYSTEM_REPORT_LAST_SLOT: AtomicI64 = AtomicI64::new(-1);
static SYSTEM_REPORT_PUSHER_HANDLE: Lazy<RwLock<Option<tauri::async_runtime::JoinHandle<()>>>> =
    Lazy::new(|| RwLock::new(None));

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

fn format_bytes(bytes: i64) -> String {
    let value = bytes.max(0) as f64;
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut size = value;
    let mut idx = 0usize;
    while size >= 1024.0 && idx < units.len() - 1 {
        size /= 1024.0;
        idx += 1;
    }

    if idx == 0 {
        format!("{} {}", size as i64, units[idx])
    } else {
        format!("{size:.2} {}", units[idx])
    }
}

fn format_bps(value: f64) -> String {
    format!("{}/s", format_bytes(value.max(0.0).round() as i64))
}

fn format_uptime(seconds: f64) -> String {
    let total = seconds.max(0.0).round() as i64;
    let days = total / 86_400;
    let hours = (total % 86_400) / 3_600;
    let minutes = (total % 3_600) / 60;

    if days > 0 {
        format!("{days}d {hours}h {minutes}m")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}

fn current_hostname() -> String {
    for key in ["HOSTNAME", "COMPUTERNAME"] {
        if let Ok(value) = std::env::var(key) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(value) = std::fs::read_to_string("/etc/hostname") {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }

    "unknown".to_string()
}

fn current_system_type() -> String {
    format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)
}

fn format_top_interfaces(interfaces: &[NetworkInterfaceStats]) -> String {
    if interfaces.is_empty() {
        return "-".to_string();
    }

    let mut sorted = interfaces.iter().collect::<Vec<_>>();
    sorted.sort_by_key(|iface| std::cmp::Reverse(iface.rx_bytes.saturating_add(iface.tx_bytes)));

    sorted
        .into_iter()
        .take(2)
        .map(|iface| {
            format!(
                "{} 入={} 出={}",
                iface.name,
                format_bytes(iface.rx_bytes),
                format_bytes(iface.tx_bytes)
            )
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

fn build_system_report_body(point: &SystemMetricsPoint, interfaces: &[NetworkInterfaceStats]) -> String {
    [
        format!(
            "时间: {}",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        ),
        format!("主机名: {}", current_hostname()),
        format!("系统类型: {}", current_system_type()),
        format!(
            "CPU: {:.1}% | Load: {:.2}/{:.2}/{:.2}",
            point.cpu_usage_percent, point.load1, point.load5, point.load15
        ),
        format!(
            "内存: {:.1}% ({}/{})",
            point.mem_used_percent,
            format_bytes(point.mem_used_bytes),
            format_bytes(point.mem_total_bytes)
        ),
        format!(
            "Swap: {:.1}% ({}/{})",
            point.swap_used_percent,
            format_bytes(point.swap_used_bytes),
            format_bytes(point.swap_total_bytes)
        ),
        format!(
            "网络: 入 {} | 出 {}",
            format_bps(point.net_rx_bps),
            format_bps(point.net_tx_bps)
        ),
        format!(
            "磁盘: 读 {} | 写 {}",
            format_bps(point.disk_read_bps),
            format_bps(point.disk_write_bps)
        ),
        format!(
            "TCP: EST={} TIME_WAIT={} CLOSE_WAIT={}",
            point.tcp_established, point.tcp_time_wait, point.tcp_close_wait
        ),
        format!(
            "进程/FD: {} | {:.1}% ({}/{})",
            point.process_count, point.fd_usage_percent, point.fd_used, point.fd_max
        ),
        format!("运行时长: {}", format_uptime(point.uptime_seconds)),
        format!("网卡流量 Top2: {}", format_top_interfaces(interfaces)),
    ]
    .join("\n")
}

fn parse_hhmm(value: &str) -> Option<chrono::NaiveTime> {
    chrono::NaiveTime::parse_from_str(value.trim(), "%H:%M").ok()
}

fn is_in_quiet_hours(webhook: &AlertWebhookConfig, now: chrono::DateTime<chrono::Local>) -> bool {
    if !webhook.quiet_hours_enabled {
        return false;
    }

    let Some(start) = parse_hhmm(&webhook.quiet_hours_start) else {
        return false;
    };
    let Some(end) = parse_hhmm(&webhook.quiet_hours_end) else {
        return false;
    };
    let current = now.time();

    if start == end {
        return true;
    }

    if start < end {
        current >= start && current < end
    } else {
        current >= start || current < end
    }
}

fn current_interval_slot(
    now: chrono::DateTime<chrono::Local>,
    interval_minutes: u32,
) -> i64 {
    let offset_seconds = now.offset().local_minus_utc() as i64;
    let local_minutes = (now.timestamp() + offset_seconds) / 60;
    local_minutes / i64::from(interval_minutes.max(1))
}

async fn try_send_system_report() -> Result<(), String> {
    let cfg = config::get_config();
    let Some(alerting) = cfg.alerting else {
        return Ok(());
    };

    if !alerting.enabled {
        return Ok(());
    }

    let Some(webhook) = alerting.webhook else {
        return Ok(());
    };

    if !webhook.enabled || !webhook.system_report_enabled || webhook.url.trim().is_empty() {
        return Ok(());
    }

    let now = chrono::Local::now();
    let slot = current_interval_slot(now, webhook.system_report_interval_minutes);
    let last_slot = SYSTEM_REPORT_LAST_SLOT.load(Ordering::Relaxed);
    if last_slot < 0 {
        SYSTEM_REPORT_LAST_SLOT.store(slot, Ordering::Relaxed);
        return Ok(());
    }
    if slot == last_slot {
        return Ok(());
    }
    SYSTEM_REPORT_LAST_SLOT.store(slot, Ordering::Relaxed);

    let weekday = now.weekday().number_from_monday() as u8;
    if !webhook.system_report_weekdays.contains(&weekday) {
        return Ok(());
    }
    if is_in_quiet_hours(&webhook, now) {
        return Ok(());
    }

    let (point, interfaces) = crate::system_metrics::collect_current_system_metrics()
        .await
        .map_err(|e| e.to_string())?;

    let payload = build_plain_text(
        &webhook.provider,
        "SSLProxyManager / System Report",
        &build_system_report_body(&point, &interfaces),
    );

    send_payload(&webhook, payload).await
}

pub fn start_system_report_pusher(_app: AppHandle) {
    if SYSTEM_REPORT_PUSHER_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }

    SYSTEM_REPORT_LAST_SLOT.store(-1, Ordering::SeqCst);
    let handle = tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(20));
        loop {
            ticker.tick().await;

            if !SYSTEM_REPORT_PUSHER_RUNNING.load(Ordering::Relaxed) {
                break;
            }

            if let Err(err) = try_send_system_report().await {
                warn!("system report webhook failed: {err}");
            }
        }

        SYSTEM_REPORT_PUSHER_RUNNING.store(false, Ordering::SeqCst);
    });

    *SYSTEM_REPORT_PUSHER_HANDLE.write() = Some(handle);
}

pub fn stop_system_report_pusher() {
    SYSTEM_REPORT_PUSHER_RUNNING.store(false, Ordering::SeqCst);
    SYSTEM_REPORT_LAST_SLOT.store(-1, Ordering::SeqCst);

    if let Some(handle) = SYSTEM_REPORT_PUSHER_HANDLE.write().take() {
        handle.abort();
    }
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

    let system_report = match crate::system_metrics::collect_current_system_metrics().await {
        Ok((point, interfaces)) => build_system_report_body(&point, &interfaces),
        Err(e) => format!("## 系统信息\n获取失败：`{}`", e),
    };

    let payload = build_plain_text(
        &webhook.provider,
        "SSLProxyManager / Test Alert",
        &format!(
            "这是一条测试告警消息，说明 Webhook 配置生效。\n\n{}",
            system_report
        ),
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
