// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod commands;
mod config;
mod metrics;
mod proxy;
mod ws_proxy;
mod stream_proxy;
mod access_control;

use tauri::Manager;

mod tray;
mod update;

fn main() {
    // 初始化日志
    tracing_subscriber::fmt::init();

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
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_config,
            commands::get_version,
            commands::check_update,
            commands::open_url,
            commands::start_server,
            commands::stop_server,
            commands::get_status,
            commands::get_logs,
            commands::clear_logs,
            commands::get_metrics,
            commands::get_listen_addrs,
            commands::query_historical_metrics,
            commands::query_request_logs,
            commands::add_blacklist_entry,
            commands::remove_blacklist_entry,
            commands::get_blacklist_entries,
            commands::refresh_blacklist_cache,
            commands::get_metrics_db_status,
            commands::test_metrics_db_connection,
            commands::open_cert_file_dialog,
            commands::open_key_file_dialog,
            commands::open_directory_dialog,
            commands::save_config_toml_as,
            commands::export_current_config_toml,
            commands::hide_to_tray,
            commands::quit_app,
            commands::get_dashboard_stats,
            commands::set_tray_proxy_state,
            commands::set_route_enabled,
            commands::set_listen_rule_enabled,
        ])
        .setup(|app| {
            // 初始化应用
            app::init(app.handle())?;

            // 初始化托盘
            tray::init_tray(app.handle())?;

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
