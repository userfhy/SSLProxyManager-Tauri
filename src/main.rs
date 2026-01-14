// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod commands;
mod config;
mod metrics;
mod proxy;
mod single_instance;
mod tray;
mod update;

fn main() {
    // 初始化日志
    tracing_subscriber::fmt::init();

    // 单实例检查
    let instance = single_instance::SingleInstance::new("ssl-proxy-manager").unwrap();
    if !instance.is_single() {
        eprintln!("程序已启动，无法重复启动。");
        std::process::exit(1);
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
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
            commands::hide_to_tray,
            commands::quit_app,
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
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
