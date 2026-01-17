use tauri::{
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

    // 官方推荐：使用默认窗口图标作为托盘图标；拿不到图标则跳过托盘创建，不阻止程序启动
    let Some(icon) = app.default_window_icon().cloned() else {
        eprintln!("Tray init skipped: default_window_icon not found");
        return Ok(());
    };

    // 构造托盘图标，macOS 需要开启模板模式以便系统自动适配深浅色
    #[allow(unused_mut)]
    let mut builder = TrayIconBuilder::new()
        .menu(&menu)
        .icon(icon)
        .show_menu_on_left_click(false)
        .tooltip("SSL 代理管理工具");
    #[cfg(target_os = "macos")]
    {
        builder = builder.icon_as_template(true);
    }

    let builder = builder.on_menu_event(|app, event| match event.id().as_ref() {
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
        });

    // 构建托盘可能会因桌面环境缺少协议而失败；若失败仅记录错误，不阻止程序启动
    if let Err(e) = builder.build(app) {
        eprintln!("Tray build failed: {e}");
    }


    Ok(())
}
