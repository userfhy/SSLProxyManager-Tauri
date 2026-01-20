use anyhow::Result;
use parking_lot::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

static METRICS_PUSHER_RUNNING: AtomicBool = AtomicBool::new(false);
static METRICS_PUSHER_HANDLE: once_cell::sync::Lazy<RwLock<Option<tauri::async_runtime::JoinHandle<()>>>> =
    once_cell::sync::Lazy::new(|| RwLock::new(None));

fn start_metrics_pusher(app: AppHandle) {
    // 单例：如果已经启动则跳过
    if METRICS_PUSHER_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }

    let handle = tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(2));
        loop {
            ticker.tick().await;

            // 若被停止则退出
            if !METRICS_PUSHER_RUNNING.load(Ordering::Relaxed) {
                break;
            }

            // 获取 metrics（内部有 500ms 缓存）
            let payload = crate::metrics::get_metrics();

            // 推送到前端：给 main 窗口 emit（前端订阅 EventsOn('metrics')）
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.emit("metrics", payload);
            }
        }

        METRICS_PUSHER_RUNNING.store(false, Ordering::SeqCst);
    });

    *METRICS_PUSHER_HANDLE.write() = Some(handle);
}

fn stop_metrics_pusher() {
    METRICS_PUSHER_RUNNING.store(false, Ordering::SeqCst);

    if let Some(h) = METRICS_PUSHER_HANDLE.write().take() {
        h.abort();
    }
}

pub fn init(app: &AppHandle) -> Result<()> {
    // rustls 0.23 需要显式选择 CryptoProvider（避免运行时 panic）
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    // 初始化配置
    crate::config::load_config()?;

    // 初始化数据库（异步，避免在 runtime 内 block_on 导致崩溃）
    // 以及：启动请求日志异步写入 worker
    if let Some(metrics_storage) = crate::config::get_config().metrics_storage.as_ref() {
        if metrics_storage.enabled {
            let db_path = metrics_storage.db_path.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = crate::metrics::init_db(db_path).await {
                    eprintln!("初始化数据库失败: {e}");
                }
                crate::metrics::init_request_log_writer().await;
            });
        }
    }

    // 启动 metrics 定时推送（应用级别，和 proxy running/stopped 无关）
    start_metrics_pusher(app.clone());

    // 启动后自动检查更新
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        if let Some(update_config) = crate::config::get_config().update.as_ref() {
            if update_config.auto_check {
                if let Ok(result) = crate::update::check_for_updates(
                    env!("CARGO_PKG_VERSION"),
                    update_config.clone(),
                )
                .await
                {
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let _ = window.emit("update-check-result", result);
                    }
                }
            }
        }
    });

    Ok(())
}

pub fn cleanup() {
    stop_metrics_pusher();
}
