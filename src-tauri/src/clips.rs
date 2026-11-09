use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::sync::watch;

const MAX_CLIPS: usize = 50;
const MAX_CONTENT: usize = 100 * 1024; // 10 万字符,足够长文本

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Clip {
    pub id: u32,
    pub content: String,
    pub created: u64, // 毫秒时间戳
    pub source: String, // "phone" | "desktop"
}

/// 文字记录:内存 + clips.json 持久化,变更经 watch 通道驱动 SSE
pub struct ClipsState {
    clips: Mutex<Vec<Clip>>,
    next_id: AtomicU32,
    version_tx: watch::Sender<u64>,
    path: std::path::PathBuf,
}

impl ClipsState {
    pub fn load(path: std::path::PathBuf) -> Self {
        let clips = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<Vec<Clip>>(&s).ok())
            .unwrap_or_default();
        let next_id = clips.iter().map(|c| c.id).max().unwrap_or(0) + 1;
        let (version_tx, _rx) = watch::channel(0u64);
        Self {
            clips: Mutex::new(clips),
            next_id: AtomicU32::new(next_id),
            version_tx,
            path,
        }
    }

    /// 新增一条记录(最新在前),超量截断,立即落盘
    pub fn add(&self, content: &str, source: &str) -> Clip {
        let clip = Clip {
            id: self.next_id.fetch_add(1, Ordering::SeqCst),
            content: content.chars().take(MAX_CONTENT).collect(),
            created: now_ms(),
            source: source.into(),
        };
        let json = {
            let mut clips = self.clips.lock().unwrap();
            clips.insert(0, clip.clone());
            clips.truncate(MAX_CLIPS);
            serde_json::to_string(&*clips).unwrap_or_default()
        };
        let _ = std::fs::write(&self.path, json);
        self.version_tx.send_modify(|v| *v += 1);
        clip
    }

    pub fn list(&self) -> Vec<Clip> {
        self.clips.lock().unwrap().clone()
    }

    pub fn version_rx(&self) -> watch::Receiver<u64> {
        self.version_tx.subscribe()
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
