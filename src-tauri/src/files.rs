use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tokio::sync::watch;

#[derive(Serialize, Clone)]
pub struct FileEntry {
    pub name: String,
    pub size: u64,
    pub modified: u64, // 毫秒时间戳
}

/// 共享目录 + 变更版本号(驱动 SSE)
pub struct FilesState {
    pub dir: std::sync::RwLock<PathBuf>,
    version_tx: watch::Sender<u64>,
}

impl FilesState {
    pub fn new(dir: PathBuf) -> Self {
        let (version_tx, _rx) = watch::channel(0u64);
        Self {
            dir: std::sync::RwLock::new(dir),
            version_tx,
        }
    }

    /// 当前共享目录快照
    pub fn dir(&self) -> PathBuf {
        self.dir.read().unwrap().clone()
    }

    /// 切换共享目录(调用方负责确保目录存在)
    pub fn set_dir(&self, dir: PathBuf) {
        *self.dir.write().unwrap() = dir;
        self.bump();
    }

    pub fn bump(&self) {
        self.version_tx.send_modify(|v| *v += 1);
    }

    pub fn version_rx(&self) -> watch::Receiver<u64> {
        self.version_tx.subscribe()
    }
}

/// 默认收件箱:~/Pigeon/Inbox
pub fn default_inbox_dir() -> PathBuf {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join("Pigeon").join("Inbox")
}

/// 清洗外部传入的文件名:去掉路径部分和 Windows 非法字符,防穿越
pub fn sanitize(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("");
    let cleaned: String = base
        .chars()
        .filter(|c| {
            !matches!(
                c,
                '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' | '\0'
            ) && !c.is_control()
        })
        .collect();
    let cleaned = cleaned.trim().trim_end_matches('.').to_string();
    let mut s: String = cleaned.chars().take(180).collect();
    if s.is_empty() {
        s = "file".into();
    }
    s
}

/// 重名时生成 "name (2).ext",避免覆盖
pub fn unique_name(dir: &Path, name: &str) -> String {
    if !dir.join(name).exists() {
        return name.to_string();
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_string(), Some(e.to_string())),
        _ => (name.to_string(), None),
    };
    for n in 2..1000 {
        let candidate = match &ext {
            Some(e) => format!("{stem} ({n}).{e}"),
            None => format!("{stem} ({n})"),
        };
        if !dir.join(&candidate).exists() {
            return candidate;
        }
    }
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    match ext {
        Some(e) => format!("{stem}-{nanos}.{e}"),
        None => format!("{stem}-{nanos}"),
    }
}

/// 列出共享目录下的文件,按修改时间倒序
pub fn file_entries(dir: &Path) -> Vec<FileEntry> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for entry in rd.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            if !meta.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            let modified = meta
                .modified()
                .ok()
                .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            out.push(FileEntry {
                name,
                size: meta.len(),
                modified,
            });
        }
    }
    out.sort_by(|a, b| b.modified.cmp(&a.modified));
    out
}

pub fn open_in_explorer(path: &Path) -> Result<(), String> {
    std::process::Command::new("explorer")
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

pub fn reveal_in_explorer(path: &Path) -> Result<(), String> {
    std::process::Command::new("explorer")
        .arg(format!("/select,{}", path.display()))
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// 递归把 dir 打包成 zip 写入 out,返回未压缩总字节数(进度用)。
/// 保留相对路径,目录内空文件夹跳过。同步阻塞,调用方放 spawn_blocking。
pub fn zip_dir(dir: &Path, out: &Path) -> Result<u64, String> {
    let file = std::fs::File::create(out).map_err(|e| e.to_string())?;
    let mut w = zip::ZipWriter::new(file);
    let options: zip::write::FileOptions<'_, ()> =
        zip::write::FileOptions::default().unix_permissions(0o644);
    let mut total: u64 = 0;

    let walk = |w: &mut zip::write::ZipWriter<std::fs::File>,
                options: &zip::write::FileOptions<'_, ()>,
                total: &mut u64|
     -> Result<(), String> {
        let mut stack = vec![dir.to_path_buf()];
        while let Some(cur) = stack.pop() {
            let rd = std::fs::read_dir(&cur).map_err(|e| format!("读取 {}: {e}", cur.display()))?;
            let mut entries: Vec<_> = rd.flatten().collect();
            entries.sort_by_key(|e| e.file_name());
            for entry in entries {
                let path = entry.path();
                // zip 内路径相对于 dir
                let rel = path
                    .strip_prefix(dir)
                    .map_err(|_| "路径越界".to_string())?
                    .to_string_lossy()
                    .replace('\\', "/");
                if path.is_dir() {
                    w.add_directory(&rel, options.clone())
                        .map_err(|e| e.to_string())?;
                    stack.push(path);
                } else {
                    w.start_file(&rel, options.clone())
                        .map_err(|e| e.to_string())?;
                    let bytes = std::fs::read(&path).map_err(|e| format!("读取 {rel}: {e}"))?;
                    std::io::copy(&mut bytes.as_slice(), w).map_err(|e| e.to_string())?;
                    *total += bytes.len() as u64;
                }
            }
        }
        Ok(())
    };

    walk(&mut w, &options, &mut total)?;
    w.finish().map_err(|e| e.to_string())?;
    Ok(total)
}
