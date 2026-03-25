use super::collect::{collect_one_point, reset_platform_collect_state};
use super::realtime::push_realtime_point;
use super::*;

#[cfg(any(target_os = "linux", target_os = "windows"))]
async fn collect_and_publish_one(app: &AppHandle, persist_enabled: bool, emit_to_frontend: bool) {
    let collected = tauri::async_runtime::spawn_blocking(collect_one_point).await;
    if let Ok(Ok((point, interfaces))) = collected {
        *LAST_INTERFACES.write() = interfaces.clone();
        push_realtime_point(point.clone());
        if persist_enabled {
            try_enqueue_system_metrics(point.clone());
        }

        if emit_to_frontend {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.emit(
                    "system-metrics",
                    SystemMetricsEventPayload { point, interfaces },
                );
            }
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
async fn run_sampler_loop(app: AppHandle) {
    loop {
        if !SAMPLER_RUNNING.load(Ordering::Relaxed) {
            break;
        }

        let has_subscriber = HAS_ACTIVE_SUBSCRIBER.load(Ordering::Relaxed);
        let cfg = crate::config::get_config();
        let wants_persistence = is_system_metrics_persistence_enabled(&cfg);
        if has_subscriber || wants_persistence {
            collect_and_publish_one(&app, wants_persistence, has_subscriber).await;
        }

        let wait_secs = if has_subscriber || wants_persistence {
            effective_sample_interval_secs()
        } else {
            IDLE_PAUSE_INTERVAL_SECS
        } as u64;

        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(wait_secs)) => {}
            _ = SAMPLER_WAKE.notified() => {}
        }
    }

    SAMPLER_RUNNING.store(false, Ordering::SeqCst);
}

pub(super) fn start_system_sampler_inner(app: AppHandle) {
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = app;
        return;
    }

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    {
        if SAMPLER_RUNNING.swap(true, Ordering::SeqCst) {
            return;
        }

        refresh_sample_interval_from_config_inner();
        init_system_metrics_writer();

        let handle = tauri::async_runtime::spawn(run_sampler_loop(app));
        *SAMPLER_HANDLE.write() = Some(handle);
    }
}

pub(super) fn stop_system_sampler_inner() {
    SAMPLER_RUNNING.store(false, Ordering::SeqCst);
    if let Some(h) = SAMPLER_HANDLE.write().take() {
        h.abort();
    }
    reset_platform_collect_state();
}
