use once_cell::sync::Lazy;
use parking_lot::RwLock;
use std::collections::HashMap;

pub type Locale = String;

static CURRENT_LOCALE: Lazy<RwLock<Locale>> = Lazy::new(|| RwLock::new("zh-CN".to_string()));

pub fn set_locale(locale: Locale) {
    *CURRENT_LOCALE.write() = locale;
}

pub fn get_locale() -> Locale {
    CURRENT_LOCALE.read().clone()
}

// 翻译键类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrayText {
    StatusRunning,
    StatusStopped,
    ToggleStart,
    ToggleStop,
    ShowWindow,
    HideWindow,
    RestartProxy,
    Quit,
    Tooltip,
}

// 翻译映射
type Translations = HashMap<TrayText, HashMap<Locale, &'static str>>;

static TRANSLATIONS: Lazy<Translations> = Lazy::new(|| {
    let mut map = HashMap::new();

    // 状态：运行中
    let mut status_running = HashMap::new();
    status_running.insert("zh-CN".to_string(), "状态：运行中");
    status_running.insert("en-US".to_string(), "Status: Running");
    map.insert(TrayText::StatusRunning, status_running);

    // 状态：已停止
    let mut status_stopped = HashMap::new();
    status_stopped.insert("zh-CN".to_string(), "状态：已停止");
    status_stopped.insert("en-US".to_string(), "Status: Stopped");
    map.insert(TrayText::StatusStopped, status_stopped);

    // 启动代理
    let mut toggle_start = HashMap::new();
    toggle_start.insert("zh-CN".to_string(), "启动代理");
    toggle_start.insert("en-US".to_string(), "Start Proxy");
    map.insert(TrayText::ToggleStart, toggle_start);

    // 停止代理
    let mut toggle_stop = HashMap::new();
    toggle_stop.insert("zh-CN".to_string(), "停止代理");
    toggle_stop.insert("en-US".to_string(), "Stop Proxy");
    map.insert(TrayText::ToggleStop, toggle_stop);

    // 显示窗口
    let mut show_window = HashMap::new();
    show_window.insert("zh-CN".to_string(), "显示窗口");
    show_window.insert("en-US".to_string(), "Show Window");
    map.insert(TrayText::ShowWindow, show_window);

    // 隐藏窗口
    let mut hide_window = HashMap::new();
    hide_window.insert("zh-CN".to_string(), "隐藏窗口");
    hide_window.insert("en-US".to_string(), "Hide Window");
    map.insert(TrayText::HideWindow, hide_window);

    // 重启代理
    let mut restart_proxy = HashMap::new();
    restart_proxy.insert("zh-CN".to_string(), "重启代理");
    restart_proxy.insert("en-US".to_string(), "Restart Proxy");
    map.insert(TrayText::RestartProxy, restart_proxy);

    // 退出
    let mut quit = HashMap::new();
    quit.insert("zh-CN".to_string(), "退出");
    quit.insert("en-US".to_string(), "Quit");
    map.insert(TrayText::Quit, quit);

    // 工具提示
    let mut tooltip = HashMap::new();
    tooltip.insert("zh-CN".to_string(), "SSL 代理管理工具");
    tooltip.insert("en-US".to_string(), "SSL Proxy Manager");
    map.insert(TrayText::Tooltip, tooltip);

    map
});

pub fn t(key: TrayText) -> &'static str {
    let locale = get_locale();
    TRANSLATIONS
        .get(&key)
        .and_then(|translations| translations.get(&locale))
        .or_else(|| {
            // 如果当前语言没有翻译，回退到 zh-CN
            TRANSLATIONS
                .get(&key)
                .and_then(|translations| translations.get(&"zh-CN".to_string()))
        })
        .copied()
        .unwrap_or("")
}
