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
    /// 分享服务开关(关闭后手机端所有请求返回 403)
    #[serde(default = "default_true")]
    pub sharing: bool,
    /// URL 携带随机 token(关闭后局域网内任何人可直接访问)
    #[serde(default = "default_true")]
    pub token_enabled: bool,
    /// 账号认证(开启后手机端需账号密码登录)
    #[serde(default)]
    pub auth_enabled: bool,
    /// 上传并发排队上限(重启生效)
    #[serde(default = "default_concurrency")]
    pub max_concurrent_uploads: u32,
    /// 全局上传限速 MB/s(0 = 不限,运行时生效)
    #[serde(default)]
    pub speed_limit_mbps: u32,
}

fn default_true() -> bool {
    true
}

fn default_concurrency() -> u32 {
    2
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            port: 8420,
            inbox_dir: None,
            dark_mode: false,
            sharing: true,
            token_enabled: true,
            auth_enabled: false,
            max_concurrent_uploads: 2,
            speed_limit_mbps: 0,
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

/// 冻结文件暂存目录:取消分享的文件移到这里,源文件保留可解冻
pub fn frozen_dir() -> PathBuf {
    pigeon_dir().join("冻结")
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
