//! 账号认证:多账号 + 密码哈希 + 端到端权限
//!
//! 账号持久化在 ~/Pigeon/accounts.json,密码存 `salt$sha256(salt+password)`。
//! 手机端登录成功后发内存会话 cookie,会话不跨应用重启。

use std::sync::Mutex;

use rand::distr::{Alphanumeric, SampleString};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Serialize, Deserialize, Clone)]
pub struct Account {
    pub username: String,
    pub salt: String,
    pub hash: String,
    /// 下载:浏览文件列表 + 下载文件 + 查看记录
    #[serde(default = "default_true")]
    pub allow_download: bool,
    /// 上传文件
    #[serde(default = "default_true")]
    pub allow_upload_file: bool,
    /// 上传文本
    #[serde(default = "default_true")]
    pub allow_upload_text: bool,
}

fn default_true() -> bool {
    true
}

/// 前端展示用(不含哈希)
#[derive(Serialize, Clone)]
pub struct AccountView {
    pub username: String,
    pub allow_download: bool,
    pub allow_upload_file: bool,
    pub allow_upload_text: bool,
}

pub struct AccountsState {
    accounts: Mutex<Vec<Account>>,
    path: std::path::PathBuf,
}

fn accounts_path() -> std::path::PathBuf {
    crate::settings::pigeon_dir().join("accounts.json")
}

fn hash_password(salt: &str, password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update(password.as_bytes());
    hasher.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

impl AccountsState {
    pub fn load() -> Self {
        let path = accounts_path();
        let accounts = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<Vec<Account>>(&s).ok())
            .unwrap_or_default();
        Self {
            accounts: Mutex::new(accounts),
            path,
        }
    }

    fn save_locked(&self, accounts: &[Account]) -> Result<(), String> {
        let json = serde_json::to_string_pretty(accounts).map_err(|e| e.to_string())?;
        std::fs::write(&self.path, json).map_err(|e| format!("写入账号文件失败: {e}"))
    }

    pub fn list(&self) -> Vec<AccountView> {
        self.accounts
            .lock()
            .unwrap()
            .iter()
            .map(|a| AccountView {
                username: a.username.clone(),
                allow_download: a.allow_download,
                allow_upload_file: a.allow_upload_file,
                allow_upload_text: a.allow_upload_text,
            })
            .collect()
    }

    /// 完整账号(含哈希),服务端校验用
    pub fn list_raw(&self) -> Vec<Account> {
        self.accounts.lock().unwrap().clone()
    }

    /// 新增账号;用户名重复/为空或密码为空时拒绝
    pub fn add(
        &self,
        username: &str,
        password: &str,
        dl: bool,
        uf: bool,
        ut: bool,
    ) -> Result<(), String> {
        let username = username.trim();
        if username.is_empty() {
            return Err("用户名不能为空".into());
        }
        if password.is_empty() {
            return Err("密码不能为空".into());
        }
        let mut accounts = self.accounts.lock().unwrap();
        if accounts.iter().any(|a| a.username == username) {
            return Err(format!("用户 {username} 已存在"));
        }
        let salt = Alphanumeric.sample_string(&mut rand::rng(), 16);
        accounts.push(Account {
            username: username.to_string(),
            hash: hash_password(&salt, password),
            salt,
            allow_download: dl,
            allow_upload_file: uf,
            allow_upload_text: ut,
        });
        self.save_locked(&accounts)
    }

    pub fn remove(&self, username: &str) -> Result<(), String> {
        let mut accounts = self.accounts.lock().unwrap();
        let before = accounts.len();
        accounts.retain(|a| a.username != username);
        if accounts.len() == before {
            return Err(format!("用户 {username} 不存在"));
        }
        self.save_locked(&accounts)
    }

    /// 校验登录;成功返回账号
    pub fn verify(&self, username: &str, password: &str) -> Option<Account> {
        let accounts = self.accounts.lock().unwrap();
        let account = accounts.iter().find(|a| a.username == username)?;
        // 常量时间比较不做也行(局域网工具),但 hash 后比较是基本卫生
        let candidate = hash_password(&account.salt, password);
        if candidate == account.hash {
            Some(account.clone())
        } else {
            None
        }
    }

    pub fn count(&self) -> usize {
        self.accounts.lock().unwrap().len()
    }
}
