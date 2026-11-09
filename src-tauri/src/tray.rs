//! 系统托盘:常驻图标,关窗不退出,方便手机随时连
//!
//! - 左键单击托盘图标:显示/聚焦主窗口
//! - 右键菜单:打开主窗口 / 复制连接地址 / 退出

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager,
};

pub const COPY_URL_EVENT: &str = "tray-copy-url";

/// 托盘图标,32px 与 icon.ico 同源
static TRAY_PNG: &[u8] = include_bytes!("../../public/favicon.png");

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "打开 Pigeon", true, None::<&str>)?;
    let copy = MenuItem::with_id(app, "copy-url", "复制连接地址", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &copy, &quit])?;

    let icon = tauri::image::Image::from_bytes(TRAY_PNG)?;

    TrayIconBuilder::with_id("main-tray")
        .icon(icon)
        .tooltip("Pigeon — 局域网传输")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "copy-url" => {
                // 剪贴板由前端已有逻辑处理(避免引额外插件配置),发事件给前端
                let _ = app.emit(COPY_URL_EVENT, ());
                show_main(app);
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// 显示并聚焦主窗口(从最小化/隐藏状态恢复)
fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}
