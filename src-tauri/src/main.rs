#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod clips;
mod files;
mod server;
mod settings;
#[cfg(windows)]
mod window_style;

use std::net::TcpListener;
use std::sync::Arc;

use tauri::Manager;

/// Tauri manage 的状态,包一层方便命令取用
pub struct AppServer {
    pub shared: Arc<server::AppState>,
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(tauri::generate_handler![
            server::server_info,
            server::list_files,
            server::open_folder,
            server::reveal_file,
            server::share_files,
            server::send_clip,
            server::list_clips,
            server::save_screenshot,
            server::delete_file,
            server::file_url,
            server::delete_clip,
            server::clear_clips,
            server::clear_files,
            server::toggle_sharing,
            server::freeze_file,
            server::unfreeze_file,
            server::list_frozen,
            server::delete_frozen,
            server::get_settings,
            server::save_settings,
            server::pick_inbox_dir,
            server::set_theme,
        ])
        .setup(|app| {
            // 1. 用户设置(端口、共享目录)
            let settings = settings::Settings::load();

            // 2. 共享目录:设置里的目录优先,否则 ~/Pigeon/Inbox
            let inbox = settings
                .inbox_dir
                .as_ref()
                .map(std::path::PathBuf::from)
                .unwrap_or_else(files::default_inbox_dir);
            std::fs::create_dir_all(&inbox)?;

            // 3. 端口:设置端口(0=自动),被占用则回落随机
            //    先用 std 绑好再移交给 tokio,避免端口被抢的竞态
            let listener = TcpListener::bind(("0.0.0.0", settings.port))
                .or_else(|_| TcpListener::bind(("0.0.0.0", 0)))?;
            let port = listener.local_addr()?.port();

            // 4. 局域网 IP(UDP connect 不实际发包)
            let host = local_lan_ip().unwrap_or_else(|| "127.0.0.1".into());

            // 文字记录等持久化文件放 ~/Pigeon 下
            let pigeon_dir = settings::pigeon_dir();
            let clips = clips::ClipsState::load(pigeon_dir.join("clips.json"));

            let shared = Arc::new(server::AppState::new(
                files::FilesState::new(inbox),
                clips,
                host,
                port,
                settings.sharing,
            ));
            eprintln!("[pigeon] serving at {}", shared.url());
            // 把连接地址落到文件里,方便排查和找回(内容含 token,仅本机使用)
            let _ = std::fs::write(pigeon_dir.join("server-url.txt"), shared.url());
            app.manage(AppServer { shared: shared.clone() });

            // 恢复上次保存的窗口主题(含窗口底色,避免暗色下露白边)
            if settings.dark_mode {
                use tauri::{Manager, Theme};
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.set_theme(Some(Theme::Dark));
                    let _ = w.set_background_color(Some(tauri::window::Color(0x14, 0x16, 0x1a, 255)));
                }
            }

            // Windows:无边框窗口补 DWM 圆角,边框颜色与画布一致
            #[cfg(windows)]
            if let Some(w) = app.get_webview_window("main") {
                if let Ok(h) = w.hwnd() {
                    window_style::apply(h.0 as isize, settings.dark_mode);
                }
            }

            // 5. 后台跑 HTTP 服务(from_std 需要 Tokio 运行时,必须在 spawn 的 async 块里做)
            let handle2 = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let tokio_listener = match listener.set_nonblocking(true)
                    .map_err(|e| e.to_string())
                    .and_then(|_| tokio::net::TcpListener::from_std(listener).map_err(|e| e.to_string()))
                {
                    Ok(l) => l,
                    Err(e) => {
                        eprintln!("[pigeon] 监听 socket 转换失败: {e}");
                        return;
                    }
                };
                if let Err(e) = server::serve(handle2, shared, tokio_listener).await {
                    eprintln!("[pigeon] HTTP 服务异常退出: {e}");
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// 通过 UDP connect 技巧拿本机出口 IP(无需真实发包)
fn local_lan_ip() -> Option<String> {
    let s = std::net::UdpSocket::bind(("0.0.0.0", 0)).ok()?;
    s.connect("8.8.8.8:80").ok()?;
    Some(s.local_addr().ok()?.ip().to_string())
}
