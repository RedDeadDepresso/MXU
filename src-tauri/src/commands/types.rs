//! 类型定义
//!
//! 包含 Tauri 命令使用的数据结构

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

// ============================================================================
// 日志缓冲区
// ============================================================================

const DEFAULT_MAX_LOGS: usize = 2000;

/// 单条运行日志条目（前端推送，页面刷新后恢复）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntryDto {
    pub id: String,
    #[serde(rename = "type")]
    pub log_type: String,
    pub message: String,
    pub html: Option<String>,
    pub timestamp: String,
}

/// 运行日志缓冲区（按实例隔离，支持容量限制）
pub struct LogBuffer {
    logs: HashMap<String, VecDeque<LogEntryDto>>,
    max_per_instance: usize,
}

impl Default for LogBuffer {
    fn default() -> Self {
        Self {
            logs: HashMap::new(),
            max_per_instance: DEFAULT_MAX_LOGS,
        }
    }
}

impl LogBuffer {
    pub fn push(&mut self, instance_id: &str, entry: LogEntryDto) {
        let entries = self.logs.entry(instance_id.to_string()).or_default();
        entries.push_back(entry);
        while entries.len() > self.max_per_instance {
            entries.pop_front();
        }
    }

    pub fn get_all(&self) -> &HashMap<String, VecDeque<LogEntryDto>> {
        &self.logs
    }

    pub fn clear_instance(&mut self, instance_id: &str) {
        if let Some(entries) = self.logs.get_mut(instance_id) {
            entries.clear();
        }
    }

    #[allow(dead_code)]
    pub fn set_max(&mut self, max: usize) {
        self.max_per_instance = max.max(100);
    }
}

// ============================================================================
// Application state
// ============================================================================

/// Shared application state (no MaaFramework dependency)
#[derive(Default)]
pub struct AppState {
    /// 运行日志缓冲区（前端推送，页面刷新后恢复）
    pub log_buffer: Mutex<LogBuffer>,
}

// ============================================================================
// System info types
// ============================================================================

/// 系统信息结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub os: String,
    pub os_version: String,
    pub arch: String,
    pub tauri_version: String,
}

/// WebView2 目录信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebView2DirInfo {
    pub path: String,
    pub system: bool,
}

// ============================================================================
// Download / update types
// ============================================================================

/// 下载进度事件数据
#[derive(Clone, Serialize)]
pub struct DownloadProgressEvent {
    pub session_id: u64,
    pub downloaded_size: u64,
    pub total_size: u64,
    pub speed: u64,
    pub progress: f64,
}

/// 下载结果
#[derive(Clone, Serialize)]
pub struct DownloadResult {
    pub session_id: u64,
    pub actual_save_path: String,
    pub detected_filename: Option<String>,
}

/// changes.json 结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangesJson {
    #[serde(default)]
    pub added: Vec<String>,
    #[serde(default)]
    pub deleted: Vec<String>,
    #[serde(default)]
    pub modified: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
    pub name: String,
    pub body: Option<String>,
    pub prerelease: bool,
    pub assets: Vec<GitHubAsset>,
}
