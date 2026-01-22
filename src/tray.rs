use parking_lot::RwLock;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager,
};

struct TrayMenuHandles<R: tauri::Runtime> {
    status: MenuItem<R>,
    toggle: MenuItem<R>,
    restart: MenuItem<R>,
}

static TRAY_HANDLES: RwLock<Option<TrayMenuHandles<tauri::Wry>>> = RwLock::new(None);

fn store_tray_handles(
    status: MenuItem<tauri::Wry>,
    toggle: MenuItem<tauri::Wry>,
    restart: MenuItem<tauri::Wry>,
) {
    *TRAY_HANDLES.write() = Some(TrayMenuHandles {
        status,
        toggle,
        restart,
    });
}

pub fn set_tray_proxy_state(running: bool) {
    let handles = TRAY_HANDLES.read();
    let Some(h) = handles.as_ref() else {
        return;
    };

    if running {
        let _ = h.status.set_text("状态：运行中");
        let _ = h.toggle.set_text("停止代理");
        let _ = h.restart.set_enabled(true);
    } else {
        let _ = h.status.set_text("状态：已停止");
        let _ = h.toggle.set_text("启动代理");
        let _ = h.restart.set_enabled(false);
    }
}

const MENU_ID_STATUS: &str = "status";
const MENU_ID_SHOW: &str = "show";
const MENU_ID_HIDE: &str = "hide";
const MENU_ID_TOGGLE: &str = "toggle";
const MENU_ID_RESTART: &str = "restart";
const MENU_ID_QUIT: &str = "quit";

pub fn init_tray(app: &AppHandle) -> tauri::Result<()> {
    // 由前端驱动托盘状态：这里仅创建菜单项，占位显示。
    let status = MenuItem::with_id(app, MENU_ID_STATUS, "状态：-", false, None::<&str>)?;
    let show = MenuItem::with_id(app, MENU_ID_SHOW, "显示窗口", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, MENU_ID_HIDE, "隐藏窗口", true, None::<&str>)?;

    let toggle = MenuItem::with_id(app, MENU_ID_TOGGLE, "-", true, None::<&str>)?;
    let restart = MenuItem::with_id(app, MENU_ID_RESTART, "重启代理", false, None::<&str>)?;

    let quit = MenuItem::with_id(app, MENU_ID_QUIT, "退出", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &status,
            &PredefinedMenuItem::separator(app)?,
            &show,
            &hide,
            &PredefinedMenuItem::separator(app)?,
            &toggle,
            &restart,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    let Some(icon) = app.default_window_icon().cloned() else {
        eprintln!("Tray init skipped: default_window_icon not found");
        return Ok(());
    };

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

    // 保存句柄（给前端 invoke 的 command 用）
    store_tray_handles(status.clone(), toggle.clone(), restart.clone());

    let builder = builder
        .on_menu_event(move |app, event| match event.id().as_ref() {
            MENU_ID_STATUS => {}
            MENU_ID_SHOW => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.unminimize();
                    let _ = window.show();

                    // Linux 下 request_user_attention + always_on_top 切换可能导致任务栏图标持续闪烁。
                    // 这里仅做必要的 focus。
                    let _ = window.set_focus();

                    #[cfg(not(target_os = "linux"))]
                    {
                        let _ = window.request_user_attention(Some(tauri::UserAttentionType::Critical));
                    let _ = window.set_always_on_top(true);
                    let _ = window.set_always_on_top(false);
                    }
                }
            }
            MENU_ID_HIDE => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }
            MENU_ID_TOGGLE => {
                // 仍允许从托盘直接启动/停止，但不在这里更新托盘文案（由前端 status 事件驱动）
                if crate::proxy::is_effectively_running() {
                    crate::proxy::stop_server(app.clone()).ok();
                } else {
                    crate::proxy::start_server(app.clone()).ok();
                }
            }
            MENU_ID_RESTART => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    crate::proxy::stop_server(app.clone()).ok();
                    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                    crate::proxy::start_server(app).ok();
                });
            }
            MENU_ID_QUIT => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                    let _ =
                        window.request_user_attention(Some(tauri::UserAttentionType::Critical));
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
            if let TrayIconEvent::DoubleClick {
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

                        // Linux 下 request_user_attention + always_on_top 切换可能导致任务栏图标持续闪烁。
                        // 这里仅做必要的 focus。
                        let _ = window.set_focus();

                        #[cfg(not(target_os = "linux"))]
                        {
                        let _ = window
                            .request_user_attention(Some(tauri::UserAttentionType::Critical));
                        let _ = window.set_always_on_top(true);
                        let _ = window.set_always_on_top(false);
                        }
                    }
                }
            }
        });

    if let Err(e) = builder.build(app) {
        eprintln!("Tray build failed: {e}");
    }

    Ok(())
}
