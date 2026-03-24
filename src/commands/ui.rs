use crate::i18n;
use crate::proxy;
use crate::tray;
use base64::Engine as _;
use std::path::PathBuf;
use tauri::Emitter;
use tauri::Manager;
use tauri::{WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub fn get_version() -> Result<String, String> {
    Ok(env!("CARGO_PKG_VERSION").to_string())
}

#[tauri::command]
pub fn get_logs() -> Result<Vec<String>, String> {
    Ok(proxy::get_logs())
}

#[tauri::command]
pub fn clear_logs() -> Result<(), String> {
    proxy::clear_logs();
    Ok(())
}

#[tauri::command]
pub fn hide_to_tray(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn open_cert_file_dialog(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let file = app
        .dialog()
        .file()
        .set_title("Select Certificate File")
        .add_filter("Certificate Files", &["crt", "cer", "pem"])
        .add_filter("All Files", &["*"])
        .blocking_pick_file();

    Ok(file
        .and_then(|f| f.into_path().ok())
        .map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
pub async fn open_key_file_dialog(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let file = app
        .dialog()
        .file()
        .set_title("Select Private Key File")
        .add_filter("Private Key Files", &["key", "pem"])
        .add_filter("All Files", &["*"])
        .blocking_pick_file();

    Ok(file
        .and_then(|f| f.into_path().ok())
        .map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
pub async fn open_directory_dialog(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let dir = app
        .dialog()
        .file()
        .set_title("Select Static Directory")
        .blocking_pick_folder();

    Ok(dir
        .and_then(|d| d.into_path().ok())
        .map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
pub async fn open_db_file_dialog(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let file = app
        .dialog()
        .file()
        .set_title("Select or Create Database File")
        .set_file_name("metrics.db")
        .add_filter("All Files", &["*"])
        .add_filter("SQLite Database", &["db", "sqlite", "sqlite3"])
        .blocking_save_file();

    Ok(file
        .and_then(|f| f.into_path().ok())
        .map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
pub async fn open_existing_db_file_dialog(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let file = app
        .dialog()
        .file()
        .set_title("Load Existing Database File")
        .add_filter("SQLite Database", &["db", "sqlite", "sqlite3"])
        .add_filter("All Files", &["*"])
        .blocking_pick_file();

    Ok(file
        .and_then(|f| f.into_path().ok())
        .map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
pub async fn open_chart_preview_window(
    app: tauri::AppHandle,
    title: String,
    payload_key: String,
    window_key: Option<String>,
) -> Result<(), String> {
    if payload_key.trim().is_empty() {
        return Err("payload_key is empty".to_string());
    }

    let label_suffix = window_key
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            let sanitized = s
                .chars()
                .map(|ch| {
                    if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                        ch
                    } else {
                        '-'
                    }
                })
                .collect::<String>()
                .trim_matches('-')
                .to_string();
            if sanitized.is_empty() {
                chrono::Local::now().timestamp_millis().to_string()
            } else {
                sanitized
            }
        })
        .unwrap_or_else(|| chrono::Local::now().timestamp_millis().to_string());
    let label = format!("chart-preview-{label_suffix}");

    let app_url = format!(
        "/index.html?chart_preview=1&key={}",
        urlencoding::encode(&payload_key)
    );

    if let Some(window) = app.get_webview_window(&label) {
        let _ = window.set_title(&title);
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        let _ = window.emit(
            "chart-preview-opened-existing",
            serde_json::json!({
                "payloadKey": payload_key,
                "targetLabel": label,
            }),
        );
        return Ok(());
    }

    WebviewWindowBuilder::new(&app, label, WebviewUrl::App(app_url.into()))
        .title(title)
        .inner_size(1400.0, 900.0)
        .resizable(true)
        .build()
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn quit_app(app: tauri::AppHandle) -> Result<(), String> {
    proxy::stop_server(app.clone()).ok();
    app.exit(0);
    Ok(())
}

#[tauri::command]
pub async fn save_config_toml_as(app: tauri::AppHandle, content: String) -> Result<Option<String>, String> {
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let default_name = format!("config-{}.toml", ts);

    let file = app
        .dialog()
        .file()
        .set_title("Export Full Configuration")
        .set_file_name(&default_name)
        .add_filter("TOML", &["toml"])
        .add_filter("All Files", &["*"])
        .blocking_save_file();

    let Some(file) = file else {
        return Ok(None);
    };

    let path: PathBuf = file
        .into_path()
        .map_err(|e| format!("Failed to get save path: {e}"))?;

    std::fs::write(&path, content).map_err(|e| format!("Failed to write file: {e}"))?;

    Ok(Some(path.to_string_lossy().to_string()))
}

#[tauri::command]
pub async fn save_chart_png_with_dialog(
    app: tauri::AppHandle,
    default_file_name: String,
    png_data_url: String,
) -> Result<Option<String>, String> {
    let file = app
        .dialog()
        .file()
        .set_title("Save PNG Image")
        .set_file_name(&default_file_name)
        .add_filter("PNG Image", &["png"])
        .add_filter("All Files", &["*"])
        .blocking_save_file();

    let Some(file) = file else {
        return Ok(None);
    };

    let path: PathBuf = file
        .into_path()
        .map_err(|e| format!("Failed to get save path: {e}"))?;

    let b64 = if let Some((_, raw)) = png_data_url.split_once(',') {
        raw
    } else {
        png_data_url.as_str()
    };

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| format!("Invalid PNG base64 data: {e}"))?;

    std::fs::write(&path, bytes).map_err(|e| format!("Failed to write file: {e}"))?;

    Ok(Some(path.to_string_lossy().to_string()))
}

#[tauri::command]
pub async fn export_current_config_toml(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let cfg_path = crate::config::get_config_path().map_err(|e| e.to_string())?;

    let content = std::fs::read_to_string(&cfg_path)
        .map_err(|e| format!("Failed to read current config file ({}): {e}", cfg_path.display()))?;

    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let default_name = format!("config-{}.toml", ts);

    let file = app
        .dialog()
        .file()
        .set_title("Export Current Configuration")
        .set_file_name(&default_name)
        .add_filter("TOML", &["toml"])
        .add_filter("All Files", &["*"])
        .blocking_save_file();

    let Some(file) = file else {
        return Ok(None);
    };

    let path: PathBuf = file
        .into_path()
        .map_err(|e| format!("Failed to get save path: {e}"))?;

    std::fs::write(&path, content).map_err(|e| format!("Failed to write file: {e}"))?;

    Ok(Some(path.to_string_lossy().to_string()))
}

#[tauri::command]
pub fn set_locale(locale: String) -> Result<(), String> {
    i18n::set_locale(locale);
    tray::update_tray_menu_texts();
    Ok(())
}

#[tauri::command]
pub fn get_locale() -> Result<String, String> {
    Ok(i18n::get_locale())
}
