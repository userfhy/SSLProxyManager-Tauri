use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager,
};

const MENU_ID_SHOW: &str = "show";
const MENU_ID_HIDE: &str = "hide";
const MENU_ID_QUIT: &str = "quit";

pub fn init_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, MENU_ID_SHOW, "显示窗口", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, MENU_ID_HIDE, "隐藏窗口", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, MENU_ID_QUIT, "退出", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[&show, &hide, &PredefinedMenuItem::separator(app)?, &quit],
    )?;

    // 直接把图标编译进二进制，避免 dev 模式下 BaseDirectory::Resource 找不到文件导致崩溃。
    // 注意：include_bytes 路径是相对 crate 根目录（Cargo.toml 所在目录）。
    let icon_bytes = include_bytes!("../icons/64x64.png");
    let icon = Image::from_bytes(icon_bytes)?;

    let _tray = TrayIconBuilder::new()
        .menu(&menu)
        .icon(icon)
        .show_menu_on_left_click(false)
        .tooltip("SSL 代理管理工具")
        .on_menu_event(|app, event| match event.id().as_ref() {
            MENU_ID_SHOW => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                    let _ = window.request_user_attention(Some(tauri::UserAttentionType::Critical));
                    let _ = window.set_always_on_top(true);
                    let _ = window.set_always_on_top(false);
                }
            }
            MENU_ID_HIDE => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }
            MENU_ID_QUIT => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                    let _ = window.request_user_attention(Some(tauri::UserAttentionType::Critical));
                    let _ = window.set_always_on_top(true);
                    let _ = window.set_always_on_top(false);

                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.emit("request-quit", ());
                        }
                    });
                }
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let visible = window.is_visible().unwrap_or(false);
                    if visible {
                        let _ = window.hide();
                    } else {
                        let _ = window.unminimize();
                        let _ = window.show();
                        let _ = window.set_focus();
                        let _ =
                            window.request_user_attention(Some(tauri::UserAttentionType::Critical));
                        let _ = window.set_always_on_top(true);
                        let _ = window.set_always_on_top(false);
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
}
