// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// 使用 mimalloc 作为全局内存分配器（性能优化）
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod access_control;
mod app;
mod buffer_pool;
mod cache_optimizer;
mod commands;
mod config;
mod hot_reload;
mod i18n;
mod metrics;
mod network_optimizer;
mod proxy;
mod rate_limit;
mod single_instance;
mod stream_proxy;
mod system_metrics;
mod test_tools;
mod ws_proxy;

use tauri::Manager;

mod tray;
mod update;

fn main() {
    // 初始化日志系统
    // 根据构建模式优化日志配置：
    // - Debug 模式：详细日志，包含目标模块
    // - Release 模式：紧凑格式，仅 info 及以上级别
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        if cfg!(debug_assertions) {
            EnvFilter::new("debug")
        } else {
            EnvFilter::new("info")
        }
    });

    let fmt_layer = fmt::layer()
        .with_timer(fmt::time::ChronoLocal::rfc_3339())
        .with_target(cfg!(debug_assertions)); // Release 模式不显示 target

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            single_instance::handle_second_instance(&app);
        }))
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_version,
            commands::get_config,
            commands::save_config,
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
            commands::get_dashboard_stats,
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
            commands::save_chart_png_with_dialog,
            commands::export_current_config_toml,
            commands::set_route_enabled,
            commands::set_listen_rule_enabled,
            commands::hide_to_tray,
            commands::quit_app,
            commands::open_chart_preview_window,
            commands::set_locale,
            commands::get_locale,
            commands::set_tray_proxy_state,
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
            commands::get_buffer_pool_stats,
            commands::get_cache_stats,
            commands::clear_all_caches,
        ])
        .setup(|app| {
            app::init(app.handle())?;
            tray::init_tray(app.handle()).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                match event {
                    tauri::WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        if let Some(main_window) = window.app_handle().get_webview_window("main") {
                            let _ = main_window.hide();
                        }
                    }
                    tauri::WindowEvent::Destroyed => {
                        app::cleanup();
                        let _ = proxy::stop_server(window.app_handle().clone());
                    }
                    _ => {}
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
