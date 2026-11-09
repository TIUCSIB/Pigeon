use std::path::PathBuf;

use serde::{Deserialize, Serialize};

const SETTINGS_FILE: &str = "settings.json";

/// 用户可调设置,持久化在 ~/Pigeon/settings.json
#[derive(Serialize, Deserialize, Clone)]
pub struct Settings {
    /// HTTP 监听端口;0 表示每次启动自动选择
    pub port: u16,
    /// 共享收件目录
    pub inbox_dir: Option<String>,
    /// 暗黑模式
    #[serde(default)]
    pub dark_mode: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            port: 8420,
            inbox_dir: None,
            dark_mode: false,
        }
    }
}

/// 设置文件所在目录(与 clips.json 同级):~/Pigeon
pub fn pigeon_dir() -> PathBuf {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join("Pigeon")
}

fn settings_path() -> PathBuf {
    pigeon_dir().join(SETTINGS_FILE)
}

impl Settings {
    /// 读取设置;文件缺失或损坏时返回默认值
    pub fn load() -> Self {
        std::fs::read_to_string(settings_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        let dir = pigeon_dir();
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(settings_path(), json).map_err(|e| e.to_string())
    }
}
