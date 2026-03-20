// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// 使用 mimalloc 作为全局内存分配器（性能优化）
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod app;
mod commands;
mod config;
mod metrics;
mod proxy;
mod ws_proxy;
mod stream_proxy;
mod access_control;
#[cfg(test)]
mod access_control_test;
mod rate_limit;
mod i18n;
mod buffer_pool;
mod network_optimizer;
mod cache_optimizer;
mod hot_reload;
mod test_tools;
mod system_metrics;

use tauri::Manager;

mod tray;
mod update;

fn main() {
    // 初始化日志系统
    // 根据构建模式优化日志配置：
    // - Debug 模式：详细日志，包含目标模块
    // - Release 模式：紧凑格式，仅 info 及以上级别
    use tracing_subscriber::{fmt, EnvFilter, prelude::*};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            if cfg!(debug_assertions) {
                EnvFilter::new("debug")
            } else {
                EnvFilter::new("info")
            }
        });

    let fmt_layer = fmt::layer()
        .with_timer(fmt::time::ChronoLocal::rfc_3339())
        .with_target(cfg!(debug_assertions));  // Release 模式不显示 target

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                window.unminimize().ok();
                window.set_focus().ok();
            }
        }))
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_config,
            commands::get_version,
            commands::check_update,
            commands::start_server,
            commands::stop_server,
            commands::get_status,
            commands::get_logs,
            commands::clear_logs,
            commands::get_metrics,
            commands::get_system_metrics,
            commands::set_system_metrics_subscription,
            commands::query_historical_system_metrics,
            commands::get_listen_addrs,
            commands::query_historical_metrics,
            commands::query_request_logs,
            commands::add_blacklist_entry,
            commands::remove_blacklist_entry,
            commands::get_blacklist_entries,
            commands::refresh_blacklist_cache,
            commands::get_metrics_db_status,
            commands::get_metrics_db_status_detail,
            commands::test_metrics_db_connection,
            commands::open_cert_file_dialog,
            commands::open_key_file_dialog,
            commands::open_directory_dialog,
            commands::open_db_file_dialog,
            commands::open_existing_db_file_dialog,
            commands::save_config_toml_as,
            commands::export_current_config_toml,
            commands::hide_to_tray,
            commands::quit_app,
            commands::get_dashboard_stats,
            commands::set_tray_proxy_state,
            commands::set_route_enabled,
            commands::set_listen_rule_enabled,
            commands::set_locale,
            commands::get_locale,
            commands::get_buffer_pool_stats,
            commands::get_cache_stats,
            commands::clear_all_caches,
            commands::send_http_test,
            commands::test_route_match,
            commands::run_route_test_suite,
            commands::run_performance_test,
            commands::validate_config_tool,
            commands::dns_lookup,
            commands::get_ssl_cert_info,
            commands::generate_self_signed_cert,
            commands::scan_ports,
            commands::encode_decode,
        ])
        .setup(|app| {
            // 初始化应用
            app::init(app.handle())?;

            // 托盘初始化延后执行，避免阻塞首屏显示
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(120)).await;
                if let Err(e) = tray::init_tray(&app_handle) {
                    eprintln!("Tray init failed: {e}");
                }
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                // 点击关闭按钮时不退出，改为隐藏到托盘
                let _ = window.hide();
                return;
            }

            // 窗口销毁：执行清理（停止后台 metrics 推送任务等）
            if let tauri::WindowEvent::Destroyed = event {
                crate::app::cleanup();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
