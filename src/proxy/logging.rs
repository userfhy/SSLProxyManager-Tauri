use parking_lot::RwLock;
use std::collections::{HashSet, VecDeque};
use tauri::Emitter;

use crate::config;

pub const LOG_QUEUE_CAPACITY: usize = 10_000;

pub static LOG_TX: once_cell::sync::Lazy<RwLock<Option<tokio::sync::mpsc::Sender<String>>>> =
    once_cell::sync::Lazy::new(|| RwLock::new(None));

pub static LOG_DROPPED: once_cell::sync::Lazy<std::sync::atomic::AtomicU64> =
    once_cell::sync::Lazy::new(|| std::sync::atomic::AtomicU64::new(0));

pub static LOGS: RwLock<VecDeque<String>> = RwLock::new(VecDeque::new());

pub static SKIP_HEADERS: once_cell::sync::Lazy<HashSet<axum::http::HeaderName>> = once_cell::sync::Lazy::new(|| {
    let mut set = HashSet::new();
    set.insert(axum::http::header::HOST);
    set.insert(axum::http::header::CONNECTION);
    set.insert(axum::http::header::ACCEPT_ENCODING);
    set.insert(axum::http::HeaderName::from_static("x-real-ip"));
    set.insert(axum::http::HeaderName::from_static("x-forwarded-for"));
    set.insert(axum::http::HeaderName::from_static("x-forwarded-proto"));
    set
});

pub fn get_logs() -> Vec<String> {
    LOGS.read().iter().cloned().collect()
}

pub fn clear_logs() {
    LOGS.write().clear();
}

#[inline]
pub fn append_log(logs: &mut VecDeque<String>, message: String) {
    const MAX_LOGS: usize = 3000;
    if logs.len() >= MAX_LOGS {
        logs.pop_front();
    }
    logs.push_back(message);
}

pub fn send_log(message: impl Into<String>) {
    append_log(&mut LOGS.write(), message.into());
}

pub fn send_log_with_app(app: &tauri::AppHandle, message: impl Into<String>) {
    let message = message.into();
    {
        append_log(&mut LOGS.write(), message.clone());
    }

    let (show_realtime_logs, realtime_logs_only_errors) = config::realtime_logs_settings();
    if !show_realtime_logs {
        return;
    }

    if realtime_logs_only_errors {
        let lower = message.to_ascii_lowercase();
        if !(lower.contains("error")
            || lower.contains("failed")
            || lower.contains("异常")
            || lower.contains("失败"))
        {
            return;
        }
    }

    let _ = app.emit("log-line", message);
}

pub fn push_log_lazy<F>(_app: &tauri::AppHandle, f: F)
where
    F: FnOnce() -> String,
{
    let line = f();

    if !config::show_realtime_logs_enabled() {
        append_log(&mut LOGS.write(), line);
        return;
    }

    if let Some(tx) = LOG_TX.read().as_ref() {
        if tx.try_send(line).is_err() {
            LOG_DROPPED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    } else {
        append_log(&mut LOGS.write(), line);
    }
}

pub fn init_log_task(app: tauri::AppHandle) {
    if LOG_TX.read().is_some() {
        return;
    }

    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(LOG_QUEUE_CAPACITY);
    *LOG_TX.write() = Some(tx);

    tauri::async_runtime::spawn(async move {
        while let Some(line) = rx.recv().await {
            {
                append_log(&mut LOGS.write(), line.clone());
            }

            let _ = app.emit("log-line", line);
        }
    });
}
