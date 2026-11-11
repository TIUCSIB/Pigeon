use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

const MAX_DEVICES: usize = 50;
const MAX_HISTORY: usize = 200;

/// 连接过的设备:以 IP 为键,记录最近一次的 UA 与登录用户
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DeviceEntry {
    pub ip: String,
    pub ua: String,
    pub username: Option<String>,
    pub first_seen: u64, // 毫秒时间戳
    pub last_seen: u64,
    pub sse: bool, // 是否有活跃的 SSE 连接(视为"在线")
    pub uploads: u32,
    pub downloads: u32,
}

/// 一次传输的记录
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TransferRecord {
    pub id: u32,
    pub time: u64,        // 毫秒时间戳
    pub dir: String,      // "in" = 手机→电脑 | "out" = 电脑→手机
    pub name: String,
    pub size: u64,
    pub ip: String, // 来源/去向 IP
}

/// devices.json 的存储结构
#[derive(Serialize, Deserialize, Default)]
struct Store {
    devices: Vec<DeviceEntry>, // 按最近活跃排序,截断到 MAX_DEVICES
    history: Vec<TransferRecord>,
}

/// 设备与传输历史:内存 + devices.json 持久化(与 clips.rs 同模式)
pub struct DevicesState {
    devices: Mutex<HashMap<String, DeviceEntry>>,
    history: Mutex<Vec<TransferRecord>>,
    next_id: AtomicU32,
    /// 上次落盘时间(毫秒):last_seen 高频更新,节流写盘
    last_save: Mutex<u64>,
    path: std::path::PathBuf,
}

impl DevicesState {
    pub fn load(path: std::path::PathBuf) -> Self {
        let store = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<Store>(&s).ok())
            .unwrap_or_default();
        let next_id = store.history.iter().map(|r| r.id).max().unwrap_or(0) + 1;
        Self {
            devices: Mutex::new(
                store
                    .devices
                    .into_iter()
                    .map(|d| (d.ip.clone(), d))
                    .collect(),
            ),
            history: Mutex::new(store.history),
            next_id: AtomicU32::new(next_id),
            last_save: Mutex::new(0),
            path,
        }
    }

    /// 内存态落到 devices.json;按最近活跃排序、截断设备数
    fn save(&self) {
        let store = Store {
            devices: self.list_devices(),
            history: self.history.lock().unwrap().clone(),
        };
        let _ = std::fs::write(&self.path, serde_json::to_string(&store).unwrap_or_default());
    }

    /// 每个请求都会 touch:更新 last_seen,新设备/身份变化立即落盘,
    /// 单纯的活跃时间更新按 60s 节流,避免每个静态资源请求都写文件
    pub fn touch(&self, ip: &str, ua: &str, username: Option<String>) {
        let mut devices = self.devices.lock().unwrap();
        let now = now_ms();
        let entry = match devices.get_mut(ip) {
            Some(e) => e,
            None => {
                devices.insert(
                    ip.to_string(),
                    DeviceEntry {
                        ip: ip.to_string(),
                        ua: ua.to_string(),
                        username,
                        first_seen: now,
                        last_seen: now,
                        sse: false,
                        uploads: 0,
                        downloads: 0,
                    },
                );
                drop(devices);
                *self.last_save.lock().unwrap() = now;
                self.save();
                return;
            }
        };
        let changed_meta = entry.ua != ua || entry.username != username;
        entry.last_seen = now;
        entry.ua = ua.to_string();
        entry.username = username;
        let mut last_save = self.last_save.lock().unwrap();
        if changed_meta || now.saturating_sub(*last_save) > 60_000 {
            *last_save = now;
            drop(last_save);
            drop(devices);
            self.save();
        }
    }

    /// SSE 连接建立/断开时标记在线状态(内存态,不落盘)
    pub fn set_sse(&self, ip: &str, on: bool) {
        let mut devices = self.devices.lock().unwrap();
        if let Some(e) = devices.get_mut(ip) {
            e.sse = on;
        }
    }

    /// 记录一次传输并落盘;同名同方向 1 分钟内去重(避免断点续传刷屏)
    pub fn record(&self, dir: &str, name: &str, size: u64, ip: &str) {
        let now = now_ms();
        let rec = TransferRecord {
            id: self.next_id.fetch_add(1, Ordering::SeqCst),
            time: now,
            dir: dir.into(),
            name: name.to_string(),
            size,
            ip: ip.into(),
        };
        {
            let mut history = self.history.lock().unwrap();
            if history.iter().any(|r| r.dir == rec.dir && r.name == rec.name && r.ip == rec.ip && now - r.time < 60_000) {
                return;
            }
            history.insert(0, rec);
            history.truncate(MAX_HISTORY);
        }
        // 先更新该设备的传输计数,再统一落盘,避免磁盘计数滞后一次
        {
            let mut devices = self.devices.lock().unwrap();
            if let Some(e) = devices.get_mut(ip) {
                match dir {
                    "in" => e.uploads += 1,
                    "out" => e.downloads += 1,
                    _ => {}
                }
            }
        }
        self.save();
    }

    pub fn list_devices(&self) -> Vec<DeviceEntry> {
        let mut out: Vec<DeviceEntry> = self
            .devices
            .lock()
            .unwrap()
            .values()
            // 回环地址多为本机工具,不作为"连接过的设备"展示
            .filter(|d| d.ip != "127.0.0.1" && d.ip != "::1")
            .cloned()
            .collect();
        out.sort_by(|a, b| b.last_seen.cmp(&a.last_seen));
        out.truncate(MAX_DEVICES);
        out
    }

    pub fn list_history(&self) -> Vec<TransferRecord> {
        self.history.lock().unwrap().clone()
    }

    pub fn clear_history(&self) {
        {
            let mut history = self.history.lock().unwrap();
            history.clear();
        }
        self.save();
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
