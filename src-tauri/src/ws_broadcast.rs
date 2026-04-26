//! WebSocket 广播基础设施
//!
//! 提供发布-订阅模型，将 KKAFIO 输出、配置变更等事件广播给所有已连接的 WebSocket 客户端。

use serde::Serialize;
use tokio::sync::broadcast;

/// 通过 WebSocket 推送给浏览器客户端的事件类型
#[derive(Clone, Serialize, Debug)]
#[serde(tag = "type", content = "payload")]
pub enum WsEvent {
    /// KKAFIO CLI 子进程输出（对应 Tauri `kkafio-output` 事件）
    #[serde(rename = "kkafio-output")]
    KkafioOutput { stream: String, line: String },

    /// 配置被某个客户端修改，其它客户端需重新拉取
    #[serde(rename = "config-changed")]
    ConfigChanged,
}

pub struct WsBroadcast {
    pub sender: broadcast::Sender<WsEvent>,
}

impl WsBroadcast {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<WsEvent> {
        self.sender.subscribe()
    }

    pub fn send(&self, event: WsEvent) {
        let _ = self.sender.send(event);
    }
}
