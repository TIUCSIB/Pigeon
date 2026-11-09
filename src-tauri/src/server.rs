use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Multipart, Path as AxumPath, State};
use axum::http::{header, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use rand::distr::{Alphanumeric, SampleString};
use serde_json::json;
use tauri::{AppHandle, Emitter};
use tokio_stream::wrappers::WatchStream;
use tokio_stream::StreamExt;
use tokio_util::io::ReaderStream;

use crate::clips::{Clip, ClipsState};
use crate::files::{file_entries, reveal_in_explorer, FileEntry, FilesState};

const TOKEN_LEN: usize = 12;

/// 程序级共享状态
pub struct AppState {
    pub files: FilesState,
    pub clips: ClipsState,
    pub host: String,
    pub port: u16,
    pub token: String,
}

impl AppState {
    pub fn new(
        files: FilesState,
        clips: ClipsState,
        host: String,
        port: u16,
    ) -> Self {
        Self {
            files,
            clips,
            host,
            port,
            token: Alphanumeric.sample_string(&mut rand::rng(), TOKEN_LEN),
        }
    }

    pub fn url(&self) -> String {
        format!("http://{}/s/{}", self.host_with_port(), self.token)
    }

    fn host_with_port(&self) -> String {
        // 纯 IP,不需要 IPv6 括号处理之外的逻辑
        format!("{}:{}", self.host, self.port)
    }
}

pub type SharedState = Arc<AppState>;

/// HTTP 服务入口(监听 socket 由 main.rs 预先绑定好传入)
pub async fn serve(
    app: AppHandle,
    state: SharedState,
    listener: tokio::net::TcpListener,
) -> Result<(), String> {
    let router = router(app, state);
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .map_err(|e| e.to_string())
}

fn router(app: AppHandle, state: SharedState) -> Router {
    use tower_http::cors::CorsLayer;

    // token 即路由前缀:所有路由只注册在 /s/{token}/ 下,
    // 不知道完整链接的请求自然落到 404,无需额外鉴权中间件
    // (axum 的路由匹配先于 layer 执行,改写 URI 的中间件方案行不通)
    let p = format!("/s/{}", state.token);
    Router::new()
        .route(&p, get(redirect_to_slash))
        .route(&format!("{p}/"), get(mobile_home))
        .route(&format!("{p}/app.js"), get(mobile_js))
        .route(&format!("{p}/style.css"), get(mobile_css))
        .route(&format!("{p}/api/events"), get(sse_events))
        .route(
            &format!("{p}/api/upload"),
            post(upload).layer(DefaultBodyLimit::disable()),
        )
        .route(&format!("{p}/api/files"), get(api_list_files))
        .route(&format!("{p}/api/clips"), get(api_list_clips).post(api_add_clip))
        .route(&format!("{p}/file/{{name}}"), get(download))
        .fallback(not_found)
        .with_state(state)
        // 桌面 UI(dev 时在 localhost:5176)fetch 本服务属于跨域,放行任意来源
        // token 本身就是鉴权,来源无关紧要
        .layer(CorsLayer::permissive())
        // app handle 供 upload 发事件用
        .layer(axum::Extension(app))
}

/// /s/{token} -> /s/{token}/ :补斜杠让页面里的相对路径解析到 token 前缀下
async fn redirect_to_slash(State(state): State<SharedState>) -> Response {
    Response::builder()
        .status(StatusCode::TEMPORARY_REDIRECT)
        .header(header::LOCATION, format!("/s/{}/", state.token))
        .body(Body::empty())
        .expect("redirect response")
}

async fn not_found(State(_): State<SharedState>) -> Response {
    not_found_response()
}

fn not_found_response() -> Response {
    (StatusCode::NOT_FOUND, "not found").into_response()
}

fn static_response(body: &'static str, ctype: &'static str) -> Response {
    (
        [
            (header::CONTENT_TYPE, ctype),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        body,
    )
        .into_response()
}

async fn mobile_home() -> Response {
    static_response(MOBILE_HTML, "text/html; charset=utf-8")
}

async fn mobile_js() -> Response {
    static_response(MOBILE_JS, "application/javascript; charset=utf-8")
}

async fn mobile_css() -> Response {
    static_response(MOBILE_CSS, "text/css; charset=utf-8")
}

/// SSE:文件列表或文字记录任一变动时推送,手机端收到后刷新
async fn sse_events(
    State(state): State<SharedState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let files_v = state.files.version_rx();
    let clips_v = state.clips.version_rx();
    let stream = tokio_stream::StreamExt::merge(
        WatchStream::new(files_v).map(|v| ("files", v)),
        WatchStream::new(clips_v).map(|v| ("clips", v)),
    )
    .map(|(kind, v)| {
        Ok::<_, std::convert::Infallible>(
            Event::default()
                .event("sync")
                .data(json!({ "kind": kind, "v": v }).to_string()),
        )
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn api_list_files(State(state): State<SharedState>) -> Json<Vec<FileEntry>> {
    Json(file_entries(&state.files.dir()))
}

async fn api_list_clips(State(state): State<SharedState>) -> Json<Vec<Clip>> {
    Json(state.clips.list())
}

/// 手机端发送文字:入库 + 推给桌面端 + 写进桌面剪贴板
async fn api_add_clip(
    axum::Extension(app): axum::Extension<AppHandle>,
    State(state): State<SharedState>,
    Json(payload): Json<serde_json::Value>,
) -> Response {
    let Some(content) = payload["content"].as_str() else {
        return (StatusCode::BAD_REQUEST, "missing content").into_response();
    };
    let content = content.trim();
    if content.is_empty() {
        return (StatusCode::BAD_REQUEST, "empty content").into_response();
    }
    if content.len() > 100 * 1024 {
        return (StatusCode::PAYLOAD_TOO_LARGE, "content too large").into_response();
    }
    let clip = state.clips.add(content, "phone");

    // 写桌面剪贴板(剪贴板需要消息循环,从主线程外调)
    let text = clip.content.clone();
    let app_cb = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        use tauri_plugin_clipboard_manager::ClipboardExt;
        if let Err(e) = app_cb.clipboard().write_text(text) {
            eprintln!("[pigeon] 写剪贴板失败: {e}");
        }
    });

    let _ = app.emit("clip-added", &clip);
    Json(&clip).into_response()
}

/// 流式接收 multipart 上传,逐块写盘,不整块进内存
async fn upload(
    axum::Extension(app): axum::Extension<AppHandle>,
    State(state): State<SharedState>,
    mut multipart: Multipart,
) -> Response {
    use tokio::io::AsyncWriteExt;

    let mut saved: Vec<(String, u64)> = Vec::new();
    while let Ok(Some(mut field)) = multipart.next_field().await {
        let filename = match field.file_name() {
            Some(n) if !n.is_empty() => n.to_string(),
            _ => return (StatusCode::BAD_REQUEST, "missing filename").into_response(),
        };
        let filename = crate::files::sanitize(&filename);
        if filename.is_empty() {
            return (StatusCode::BAD_REQUEST, "invalid filename").into_response();
        }
        let dir = state.files.dir();
        let unique = crate::files::unique_name(&dir, &filename);
        let target = dir.join(&unique);

        let mut written: u64 = 0;
        let mut failed: Option<String> = None;
        match tokio::fs::File::create(&target).await {
            Ok(mut file) => {
                loop {
                    let chunk = match field.chunk().await {
                        Ok(c) => c,
                        Err(e) => {
                            failed = Some(format!("read chunk: {e}"));
                            break;
                        }
                    };
                    let Some(chunk) = chunk else { break };
                    if let Err(e) = file.write_all(&chunk).await {
                        failed = Some(format!("write: {e}"));
                        break;
                    }
                    written += chunk.len() as u64;
                }
                if failed.is_none() {
                    if let Err(e) = file.flush().await {
                        failed = Some(format!("flush: {e}"));
                    }
                }
            }
            Err(e) => failed = Some(format!("create: {e}")),
        }
        if let Some(e) = failed {
            let _ = tokio::fs::remove_file(&target).await;
            eprintln!("[pigeon] 保存 {filename} 失败: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to save {filename}: {e}"),
            )
                .into_response();
        }
        saved.push((unique, written));
    }

    if saved.is_empty() {
        return (StatusCode::BAD_REQUEST, "no file received").into_response();
    }

    for (name, size) in &saved {
        let _ = app.emit(
            "file-added",
            json!({ "name": name, "size": size, "source": "phone" }),
        );
    }
    state.files.bump();
    Json(json!({ "ok": true, "saved": saved })).into_response()
}

/// 下载共享目录中的文件,支持 Range 断点续传(HTTP 206)
async fn download(
    State(state): State<SharedState>,
    headers: axum::http::HeaderMap,
    AxumPath(name): AxumPath<String>,
) -> Response {
    const CD_SET: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'.')
        .remove(b'_');

    let safe = crate::files::sanitize(&name);
    if safe.is_empty() {
        return not_found_response();
    }
    let dir = state.files.dir();
    let path = dir.join(&safe);
    let Ok(file) = tokio::fs::File::open(&path).await else {
        return not_found_response();
    };
    let Ok(meta) = file.metadata().await else {
        return not_found_response();
    };
    let total_len = meta.len();
    let mime = mime_guess::from_path(&safe).first_or_octet_stream();
    let cd = format!(
        "attachment; filename*=UTF-8''{}",
        percent_encoding::utf8_percent_encode(&safe, CD_SET)
    );

    // 解析单个 "bytes=start-end" 区间;缺失、多区间或格式不对都按整文件返回
    // end 超出文件长度时收敛到最后一个字节(RFC 7233)
    let (start, end_incl, partial) = match parse_range(
        headers.get(header::RANGE).and_then(|v| v.to_str().ok()),
        total_len,
    ) {
        Some(r) => r,
        None => (0, total_len.saturating_sub(1), false),
    };

    let body = if partial {
        use tokio::io::{AsyncReadExt, AsyncSeekExt};
        let mut f = file;
        if f.seek(std::io::SeekFrom::Start(start)).await.is_err() {
            return not_found_response();
        }
        // 边读边写避免大区间整块进内存
        let take = f.take(end_incl - start + 1);
        Response::builder()
            .status(StatusCode::PARTIAL_CONTENT)
            .header(header::CONTENT_TYPE, mime.as_ref())
            .header(header::CONTENT_DISPOSITION, cd.as_str())
            .header(header::ACCEPT_RANGES, "bytes")
            .header(
                header::CONTENT_RANGE,
                format!("bytes {start}-{end_incl}/{total_len}"),
            )
            .header(header::CONTENT_LENGTH, end_incl - start + 1)
            .body(Body::from_stream(ReaderStream::new(take)))
            .expect("range response")
    } else {
        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime.as_ref())
            .header(header::CONTENT_DISPOSITION, cd.as_str())
            .header(header::ACCEPT_RANGES, "bytes")
            .header(header::CONTENT_LENGTH, total_len)
            .body(Body::from_stream(ReaderStream::new(file)))
            .expect("download response")
    };
    body
}

/// 解析 "bytes=start-end" 头;返回 (start, end_inclusive, 是否区间请求)
/// 起点越界(>=len)返回 None → 交给上层按整文件处理(简化处理,不返 416)
fn parse_range(v: Option<&str>, total: u64) -> Option<(u64, u64, bool)> {
    let v = v?;
    let rest = v.strip_prefix("bytes=")?;
    if rest.contains(',') {
        return None; // 多区间不支持
    }
    let (s, e) = rest.split_once('-')?;
    let (start, end) = match (s.trim(), e.trim()) {
        // "bytes=-500":最后 500 字节
        ("", e) => {
            let n: u64 = e.parse().ok()?;
            if n == 0 {
                return None;
            }
            (total.saturating_sub(n), total.saturating_sub(1))
        }
        // "bytes=100-":从 100 到结尾
        (s, "") => {
            let n: u64 = s.parse().ok()?;
            (n, total.saturating_sub(1))
        }
        (s, e) => {
            let a: u64 = s.parse().ok()?;
            let b: u64 = e.parse().ok()?;
            if b < a {
                return None;
            }
            (a, b.min(total.saturating_sub(1)))
        }
    };
    if start >= total {
        return None;
    }
    Some((start, end.max(start), true))
}

// ---------- Tauri 命令(桌面 UI 调用) ----------

#[tauri::command]
pub fn server_info(state: tauri::State<'_, crate::AppServer>) -> serde_json::Value {
    let s = &state.shared;
    json!({
        "url": s.url(),
        "host": s.host,
        "port": s.port,
        "token": s.token,
        "inbox_dir": s.files.dir().to_string_lossy(),
    })
}

#[tauri::command]
pub fn list_files(state: tauri::State<'_, crate::AppServer>) -> Vec<FileEntry> {
    file_entries(&state.shared.files.dir())
}

#[tauri::command]
pub fn open_folder(state: tauri::State<'_, crate::AppServer>) -> Result<(), String> {
    crate::files::open_in_explorer(&state.shared.files.dir())
}

#[tauri::command]
pub fn reveal_file(state: tauri::State<'_, crate::AppServer>, name: String) -> Result<(), String> {
    // 名称经过 sanitize,只能定位到共享目录内的文件
    let safe = crate::files::sanitize(&name);
    let path = state.shared.files.dir().join(safe);
    reveal_in_explorer(&path)
}

/// 删除共享目录中的文件(桌面 UI 右键/按钮触发)
#[tauri::command]
pub async fn delete_file(
    state: tauri::State<'_, crate::AppServer>,
    name: String,
) -> Result<(), String> {
    let safe = crate::files::sanitize(&name);
    let path = state.shared.files.dir().join(safe);
    tokio::fs::remove_file(&path)
        .await
        .map_err(|e| format!("删除失败: {e}"))?;
    state.shared.files.bump();
    Ok(())
}

/// 复制单个文件的下载链接(含 token,发给对方即可下载)
#[tauri::command]
pub fn file_url(state: tauri::State<'_, crate::AppServer>, name: String) -> Result<String, String> {
    let safe = crate::files::sanitize(&name);
    if safe.is_empty() {
        return Err("无效文件名".into());
    }
    Ok(format!("{}/file/{}", state.shared.url(), encode_name(&safe)))
}

/// 路径段百分号编码(与 download 的 CD_SET 保持一致的保留字符)
fn encode_name(name: &str) -> String {
    const SET: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'.')
        .remove(b'_');
    percent_encoding::utf8_percent_encode(name, SET).to_string()
}

/// 分享已有文件:复制进共享目录(文件夹暂不支持)
#[tauri::command]
pub async fn share_files(
    app: AppHandle,
    state: tauri::State<'_, crate::AppServer>,
    paths: Vec<String>,
) -> Result<(), String> {
    let mut copied = 0usize;
    let mut skipped: Vec<String> = Vec::new();
    let dir = state.shared.files.dir();
    for p in &paths {
        let src = std::path::PathBuf::from(p);
        let name = src
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "file".into());
        let name = crate::files::sanitize(&name);
        let meta = match tokio::fs::metadata(&src).await {
            Ok(m) => m,
            Err(e) => {
                skipped.push(format!("{name}({e})"));
                continue;
            }
        };
        // 文件夹:后台打包成 zip 再分享(file-share 风格)
        if meta.is_dir() {
            let stem = name.clone();
            let zip_name = crate::files::unique_name(&dir, &format!("{stem}.zip"));
            let zip_path = dir.join(&zip_name);
            let src_path = src.clone();
            let zipped: Result<u64, String> =
                tauri::async_runtime::spawn_blocking(move || {
                    crate::files::zip_dir(&src_path, &zip_path)
                })
                .await
                .map_err(|e| e.to_string())?;
            match zipped {
                Ok(n) => {
                    let _ = app.emit(
                        "file-added",
                        json!({ "name": zip_name, "size": n, "source": "desktop" }),
                    );
                    copied += 1;
                }
                Err(e) => skipped.push(format!("{zip_name}({e})")),
            }
            continue;
        }
        let unique = crate::files::unique_name(&dir, &name);
        let dst = dir.join(&unique);
        if let Err(e) = tokio::fs::copy(&src, &dst).await {
            skipped.push(format!("{name}({e})"));
            continue;
        }
        let _ = app.emit(
            "file-added",
            json!({ "name": unique, "size": meta.len(), "source": "desktop" }),
        );
        copied += 1;
    }
    state.shared.files.bump();
    if copied == 0 && !skipped.is_empty() {
        return Err(format!("未能分享: {}(文件夹暂不支持)", skipped.join("、")));
    }
    Ok(())
}

/// 桌面端发送文字
#[tauri::command]
pub fn send_clip(app: AppHandle, state: tauri::State<'_, crate::AppServer>, content: String) -> Result<(), String> {
    let content = content.trim().to_string();
    if content.is_empty() {
        return Err("内容为空".into());
    }
    let clip = state.shared.clips.add(&content, "desktop");
    let _ = app.emit("clip-added", &clip);
    Ok(())
}

/// 列出文字记录(桌面 UI)
#[tauri::command]
pub fn list_clips(state: tauri::State<'_, crate::AppServer>) -> Vec<Clip> {
    state.shared.clips.list()
}

/// 读取设置(桌面 UI)
#[tauri::command]
pub fn get_settings() -> crate::settings::Settings {
    crate::settings::Settings::load()
}

/// 保存设置。注意:端口改动需重启 App 生效;目录改动立即生效。
#[tauri::command]
pub async fn save_settings(
    state: tauri::State<'_, crate::AppServer>,
    port: u16,
    inbox_dir: Option<String>,
    dark_mode: Option<bool>,
) -> Result<(), String> {
    // 目录为空字符串时回落默认值
    let inbox_dir = inbox_dir.filter(|s| !s.trim().is_empty());
    let settings = crate::settings::Settings {
        port,
        inbox_dir,
        dark_mode: dark_mode.unwrap_or(false),
    };
    settings.save()?;

    // 目录立即切换;目录不存在则自动创建
    if let Some(d) = &settings.inbox_dir {
        let path = std::path::PathBuf::from(d);
        if path != state.shared.files.dir() {
            tokio::fs::create_dir_all(&path)
                .await
                .map_err(|e| format!("创建目录失败: {e}"))?;
            state.shared.files.set_dir(path);
        }
    }
    Ok(())
}

/// 切换窗口原生主题(标题栏等跟随),并即时持久化
#[tauri::command]
pub fn set_theme(
    app: AppHandle,
    dark: bool,
) -> Result<(), String> {
    use tauri::{Manager, Theme};
    let window = app
        .get_webview_window("main")
        .ok_or("主窗口不存在")?;
    window
        .set_theme(Some(if dark { Theme::Dark } else { Theme::Light }))
        .map_err(|e| e.to_string())?;
    // 窗口底色同步画布色,避免 WebView 默认白底露出突兀的边
    set_window_bg(&window, dark);

    // 主题是即时生效项,直接更新持久化文件里的 dark_mode
    let mut settings = crate::settings::Settings::load();
    settings.dark_mode = dark;
    settings.save()?;
    Ok(())
}

/// 窗口/WebView 底色跟随主题(画布色:亮 #f1f2f4,暗 #14161a)
fn set_window_bg(window: &tauri::WebviewWindow, dark: bool) {
    let (r, g, b) = if dark { (0x14, 0x16, 0x1a) } else { (0xf1, 0xf2, 0xf4) };
    let _ = window.set_background_color(Some(tauri::window::Color(r, g, b, 255)));
}

/// 选取新的共享目录(系统目录选择框)
#[tauri::command]
pub async fn pick_inbox_dir(app: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    // blocking 对话框不能占主线程,丢进阻塞线程池
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog().file().blocking_pick_folder()
    })
    .await
    .map_err(|e| e.to_string())?;
    // FilePath 枚举转字符串(简化:只处理真实路径,虚拟位置返回 None)
    Ok(picked.and_then(|p| p.as_path().map(|pp| pp.to_string_lossy().to_string())))
}

/// 桌面端粘贴的截图写入共享目录
#[tauri::command]
pub async fn save_screenshot(
    app: AppHandle,
    state: tauri::State<'_, crate::AppServer>,
    data: String, // base64 PNG
) -> Result<String, String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.trim())
        .map_err(|e| format!("base64 解码失败: {e}"))?;
    if bytes.is_empty() {
        return Err("图片数据为空".into());
    }
    // 前缀 PNG 魔数校验,拒绝任意文件
    if bytes.len() < 8 || bytes[..8] != [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A] {
        return Err("不是有效的 PNG 图片".into());
    }
    let ts = chrono_like_ts();
    let dir = state.shared.files.dir();
    let name = crate::files::unique_name(&dir, &format!("截图-{ts}.png"));
    let path = dir.join(&name);
    tokio::fs::write(&path, &bytes)
        .await
        .map_err(|e| format!("写入失败: {e}"))?;
    let _ = app.emit(
        "file-added",
        json!({ "name": name, "size": bytes.len(), "source": "desktop" }),
    );
    state.shared.files.bump();
    Ok(name)
}

/// 本地时间戳字符串(文件名用):20261109-133045
fn chrono_like_ts() -> String {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    // UTC+8 假设太粗糙,但截图文件名够用;直接用本地 API 更好的方案在路线图
    let secs = ms / 1000 + 8 * 3600;
    let (h, m, s) = ((secs / 3600) % 24, (secs / 60) % 60, secs % 60);
    let days = secs / 86400;
    // 1970-01-01 起的天数转 yyyymmdd(民用算法)
    let (y, mo, d) = civil_from_days(days as i64);
    format!("{y:04}{mo:02}{d:02}-{h:02}{m:02}{s:02}")
}

/// Howard Hinnant 的 days->civil 算法
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

// ---------- 手机端页面(内嵌,零外部依赖) ----------

// 注意:内容含 "# 序列(SVG fill="#fff"),必须用 r## 定界
const MOBILE_HTML: &str = r##"<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<title>Pigeon</title>
<link rel="stylesheet" href="style.css">
</head>
<body>
<header>
  <div class="brand">
    <span class="logo"><svg viewBox="0 0 24 24" width="22" height="22"><path fill="#fff" d="M2.01 21L23 12 2.01 3 2 10l15 2-15 2z"/></svg></span>
    <div><h1>Pigeon</h1><p id="host"></p></div>
  </div>
</header>
<main>
  <button id="pick" class="pick">
    <svg viewBox="0 0 24 24" width="28" height="28"><path fill="currentColor" d="M9 16h6v-6h4l-7-7-7 7h4v6zm-4 2h14v2H5z"/></svg>
    <span>选择文件 / 拍照上传</span>
    <span class="hint">发送到电脑</span>
  </button>
  <input type="file" id="input" multiple>
  <section class="text-card">
    <h2>发送文字</h2>
    <textarea id="text-in" placeholder="输入或粘贴文字,发送到电脑剪贴板" rows="3"></textarea>
    <button id="send-text" class="send-btn" disabled>发送</button>
  </section>
  <section id="progress-section" class="hidden">
    <h2>正在发送</h2>
    <ul id="queue"></ul>
  </section>
  <section id="clips-section" class="hidden">
    <h2>文字记录</h2>
    <ul id="clips"></ul>
  </section>
  <section id="files-section">
    <h2>来自电脑的文件 <span id="count"></span></h2>
    <ul id="files"></ul>
    <p id="empty" class="hidden">电脑还没有分享文件</p>
  </section>
</main>
<div id="toast" class="hidden"></div>
<script src="app.js"></script>
</body>
</html>"##;

const MOBILE_CSS: &str = r##"* { box-sizing: border-box; margin: 0; padding: 0; }
body {
  font-family: system-ui, -apple-system, "PingFang SC", "Microsoft YaHei", sans-serif;
  background: #f4f6fa; color: #17203a;
  min-height: 100vh; min-height: 100dvh;
  padding-bottom: 40px;
  -webkit-tap-highlight-color: transparent;
}
header {
  background: linear-gradient(135deg, #4a7cff, #3b5fe0);
  color: #fff; padding: 22px 20px 26px;
  border-radius: 0 0 22px 22px;
}
.brand { display: flex; align-items: center; gap: 12px; }
.logo {
  width: 42px; height: 42px; border-radius: 12px; background: rgba(255,255,255,.2);
  display: flex; align-items: center; justify-content: center; flex: none;
}
h1 { font-size: 19px; }
#host { font-size: 12px; opacity: .85; font-family: ui-monospace, Consolas, monospace; }
main { padding: 18px 16px; display: flex; flex-direction: column; gap: 16px; }
.pick {
  width: 100%; border: 1px solid #e6e9ef; border-radius: 16px; padding: 22px 16px;
  background: #fff;
  display: flex; flex-direction: column; align-items: center; gap: 6px;
  color: #17203a; font-size: 15px; font-weight: 600; cursor: pointer;
  transition: transform .1s;
}
.pick:active { transform: scale(.98); }
.pick svg { color: #3b6ef3; }
.pick .hint { font-size: 12px; color: #8a93a8; font-weight: 400; }
#input { display: none; }
section { background: #fff; border: 1px solid #e6e9ef; border-radius: 16px; padding: 16px; }
h2 { font-size: 14px; margin-bottom: 10px; }
#count { color: #8a93a8; font-size: 12px; font-weight: 400; }
ul { list-style: none; }
li {
  display: flex; align-items: center; gap: 10px;
  padding: 10px 2px; border-bottom: 1px solid #f0f2f7;
}
li:last-child { border-bottom: 0; }
.ext {
  flex: none; width: 36px; height: 36px; border-radius: 10px;
  background: #eef2ff; color: #3b6ef3;
  font-size: 9px; font-weight: 700; text-transform: uppercase;
  display: flex; align-items: center; justify-content: center; overflow: hidden;
}
.fmain { flex: 1; min-width: 0; }
.fname { font-size: 13px; display: block; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.fmeta { font-size: 11px; color: #8a93a8; }
.fsize { font-size: 12px; color: #5b6478; flex: none; }
li a { color: inherit; text-decoration: none; display: flex; align-items: center; gap: 10px; flex: 1; min-width: 0; }
li .bar { flex: 1; height: 5px; border-radius: 999px; background: #eef2ff; overflow: hidden; }
li .bar i { display: block; height: 100%; background: #3b6ef3; border-radius: 999px; transition: width .15s; width: 0; }
li.done .fmeta { color: #22a06b; }
.hidden { display: none !important; }
#empty { text-align: center; color: #8a93a8; font-size: 13px; padding: 16px 0; }
#toast {
  position: fixed; bottom: 24px; left: 50%; transform: translateX(-50%);
  background: #17203a; color: #fff; font-size: 13px;
  padding: 10px 18px; border-radius: 999px; max-width: 88vw;
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
}
.text-card textarea {
  width: 100%; border: 1.5px solid #e3e8f2; border-radius: 10px;
  padding: 10px 12px; font-size: 14px; font-family: inherit;
  resize: vertical; min-height: 60px; box-sizing: border-box;
}
.text-card textarea:focus { outline: none; border-color: #3b6ef3; }
.send-btn {
  margin-top: 10px; width: 100%; border: 0; border-radius: 10px;
  padding: 11px; background: #3b6ef3; color: #fff;
  font-size: 14px; font-weight: 600; cursor: pointer;
}
.send-btn:disabled { background: #b9c8f5; cursor: default; }
.send-btn:not(:disabled):active { transform: scale(.98); }
#clips li {
  display: block; padding: 10px 2px; border-bottom: 1px solid #f0f2f7;
}
#clips li:last-child { border-bottom: 0; }
.clip-text {
  font-size: 13px; line-height: 1.5; white-space: pre-wrap; word-break: break-all;
  max-height: 5.2em; overflow: hidden;
}
.clip-foot {
  margin-top: 6px; display: flex; justify-content: space-between; align-items: center;
}
.clip-time { font-size: 11px; color: #8a93a8; }
.clip-copy {
  border: 0; background: #eef2ff; color: #3b6ef3; border-radius: 7px;
  font-size: 12px; padding: 5px 12px; cursor: pointer;
}
.clip-copy:active { transform: scale(.96); }
.thumb {
  width: 44px; height: 44px; object-fit: cover; border-radius: 8px; flex: none;
  background: #f0f2f7;
}"##;

const MOBILE_JS: &str = r##"(function () {
  function $(id) { return document.getElementById(id); }
  // 当前页面位于 /s/{token}/,静态资源与接口都在同一前缀下
  var base = location.pathname.replace(/[^/]*$/, '');
  $('host').textContent = location.host;

  function fmtSize(n) {
    if (n < 1024) return n + ' B';
    if (n < 1048576) return (n / 1024).toFixed(1) + ' KB';
    if (n < 1073741824) return (n / 1048576).toFixed(1) + ' MB';
    return (n / 1073741824).toFixed(2) + ' GB';
  }
  function extOf(name) {
    var i = name.lastIndexOf('.');
    return i > 0 ? name.slice(i + 1).toLowerCase().slice(0, 5) : 'file';
  }
  function toast(msg) {
    var t = $('toast');
    t.textContent = msg;
    t.classList.remove('hidden');
    clearTimeout(t._h);
    t._h = setTimeout(function () { t.classList.add('hidden'); }, 2500);
  }

  // ---- 文件列表 ----
  function isImage(name) { return /\.(png|jpe?g|gif|webp|bmp|svg)$/i.test(name); }
  function renderFiles(list) {
    var ul = $('files');
    $('count').textContent = list.length ? '(' + list.length + ')' : '';
    $('empty').classList.toggle('hidden', list.length > 0);
    ul.innerHTML = '';
    list.forEach(function (f) {
      var li = document.createElement('li');
      var a = document.createElement('a');
      a.href = base + 'file/' + encodeURIComponent(f.name);
      a.setAttribute('download', f.name);
      if (isImage(f.name)) {
        var img = document.createElement('img');
        img.className = 'thumb'; img.loading = 'lazy';
        img.src = a.href;
        a.appendChild(img);
      } else {
        var ext = document.createElement('span');
        ext.className = 'ext'; ext.textContent = extOf(f.name);
        a.appendChild(ext);
      }
      var main = document.createElement('span');
      main.className = 'fmain';
      var nm = document.createElement('span');
      nm.className = 'fname'; nm.textContent = f.name;
      var mt = document.createElement('span');
      mt.className = 'fmeta'; mt.textContent = fmtSize(f.size);
      main.appendChild(nm); main.appendChild(mt);
      var sz = document.createElement('span');
      sz.className = 'fsize'; sz.textContent = '下载';
      a.appendChild(main); a.appendChild(sz);
      // 长按(桌面右键)文件行复制直链,可发给同一网络里的其他人
      li.addEventListener('contextmenu', function (ev) {
        ev.preventDefault();
        copyText(location.origin + a.getAttribute('href'));
        toast('链接已复制,可发给其他人');
      });
      li.appendChild(a);
      ul.appendChild(li);
    });
  }

  function loadFiles() {
    fetch(base + 'api/files')
      .then(function (r) { return r.json(); })
      .then(renderFiles)
      .catch(function () {});
  }

  // ---- 文字记录 ----
  function renderClips(list) {
    var ul = $('clips');
    $('clips-section').classList.toggle('hidden', list.length === 0);
    ul.innerHTML = '';
    list.forEach(function (c) {
      var li = document.createElement('li');
      var txt = document.createElement('p');
      txt.className = 'clip-text'; txt.textContent = c.content;
      var foot = document.createElement('div');
      foot.className = 'clip-foot';
      var time = document.createElement('span');
      time.className = 'clip-time';
      time.textContent = (c.source === 'phone' ? '手机 · ' : '电脑 · ') + new Date(c.created).toLocaleString();
      var copy = document.createElement('button');
      copy.className = 'clip-copy'; copy.textContent = '复制';
      copy.addEventListener('click', function () {
        copyText(c.content);
      });
      foot.appendChild(time); foot.appendChild(copy);
      li.appendChild(txt); li.appendChild(foot);
      ul.appendChild(li);
    });
  }

  function copyText(text) {
    function done() { toast('已复制'); }
    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(text).then(done, function () { fallbackCopy(text); done(); });
    } else {
      fallbackCopy(text); done();
    }
  }
  function fallbackCopy(text) {
    var ta = document.createElement('textarea');
    ta.value = text;
    ta.style.position = 'fixed'; ta.style.opacity = '0';
    document.body.appendChild(ta);
    ta.select();
    try { document.execCommand('copy'); } catch (e) {}
    ta.remove();
  }

  function loadClips() {
    fetch(base + 'api/clips')
      .then(function (r) { return r.json(); })
      .then(renderClips)
      .catch(function () {});
  }

  // SSE:桌面共享目录变动时刷新
  try {
    var es = new EventSource(base + 'api/events');
    es.addEventListener('sync', function () { loadFiles(); loadClips(); });
  } catch (e) { /* 老浏览器轮询兜底 */ }

  // ---- 发送文字 ----
  var textIn = $('text-in'), sendBtn = $('send-text');
  textIn.addEventListener('input', function () {
    sendBtn.disabled = !textIn.value.trim();
  });
  function sendText() {
    var content = textIn.value.trim();
    if (!content) return;
    sendBtn.disabled = true;
    sendBtn.textContent = '发送中…';
    fetch(base + 'api/clips', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ content: content })
    }).then(function (r) {
      if (!r.ok) throw new Error('' + r.status);
      textIn.value = '';
      sendBtn.textContent = '已发送到电脑剪贴板 ✓';
      loadClips();
      setTimeout(function () { sendBtn.textContent = '发送'; sendBtn.disabled = !textIn.value.trim(); }, 1500);
    }).catch(function (e) {
      sendBtn.textContent = '发送失败,重试';
      sendBtn.disabled = false;
      toast('发送失败: ' + e.message);
    });
  }
  sendBtn.addEventListener('click', sendText);
  textIn.addEventListener('keydown', function (e) {
    // 手机上 Cmd/Ctrl+Enter 发送,避免误触
    if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') sendText();
  });

  // ---- 上传 ----
  var input = $('input'), queue = $('queue'), ps = $('progress-section');

  $('pick').addEventListener('click', function () { input.click(); });
  input.addEventListener('change', function () {
    sendFiles(Array.prototype.slice.call(input.files));
    input.value = '';
  });

  function sendFiles(files) {
    if (!files.length) return;
    ps.classList.remove('hidden');
    var items = files.map(function (f) {
      var li = document.createElement('li');
      li.innerHTML = '<span class="ext"></span><span class="fmain"><span class="fname"></span><span class="bar"><i></i></span></span><span class="fsize">0%</span>';
      li.querySelector('.ext').textContent = extOf(f.name);
      li.querySelector('.fname').textContent = f.name;
      queue.appendChild(li);
      return { file: f, li: li, bar: li.querySelector('.bar i'), pct: li.querySelector('.fsize') };
    });
    sendNext(items, 0);
  }

  function sendNext(items, i) {
    if (i >= items.length) {
      toast('已发送 ' + items.length + ' 个文件');
      setTimeout(function () {
        ps.classList.add('hidden');
        queue.innerHTML = '';
      }, 1200);
      loadFiles();
      return;
    }
    var it = items[i];
    var xhr = new XMLHttpRequest();
    xhr.open('POST', base + 'api/upload');
    xhr.upload.onprogress = function (e) {
      if (e.lengthComputable) {
        var p = Math.round(e.loaded / e.total * 100);
        it.bar.style.width = p + '%';
        it.pct.textContent = p + '%';
      }
    };
    xhr.onload = function () {
      it.li.classList.add('done');
      it.bar.style.width = '100%';
      it.pct.textContent = xhr.status === 200 ? '完成' : '失败';
      sendNext(items, i + 1);
    };
    xhr.onerror = function () {
      it.pct.textContent = '失败';
      sendNext(items, i + 1);
    };
    var fd = new FormData();
    fd.append('file', it.file, it.file.name);
    xhr.send(fd);
  }

  loadFiles();
  loadClips();
})();
"##;
