//! 状态查询命令

use std::collections::HashMap;
use std::sync::Arc;

use tauri::State;

use super::types::{AppState, LogEntryDto};

/// 由前端调用，将已格式化的日志行输出到 stdout
#[tauri::command]
pub fn log_to_stdout(message: String) {
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S.%3f");
    for line in message.lines() {
        println!("[{timestamp}] {line}");
    }
}

/// 前端推送一条运行日志到后端缓冲区
#[tauri::command]
pub fn push_log(
    state: State<Arc<AppState>>,
    instance_id: String,
    entry: LogEntryDto,
) -> Result<(), String> {
    let mut buffer = state.log_buffer.lock().map_err(|e| e.to_string())?;
    buffer.push(&instance_id, entry);
    Ok(())
}

/// 获取所有实例的运行日志（用于页面刷新后恢复）
#[tauri::command]
pub fn get_all_logs(
    state: State<Arc<AppState>>,
) -> Result<HashMap<String, Vec<LogEntryDto>>, String> {
    let buffer = state.log_buffer.lock().map_err(|e| e.to_string())?;
    Ok(buffer
        .get_all()
        .iter()
        .map(|(k, v)| (k.clone(), v.iter().cloned().collect()))
        .collect())
}

/// 清空指定实例的运行日志
#[tauri::command]
pub fn clear_instance_logs(
    state: State<Arc<AppState>>,
    instance_id: String,
) -> Result<(), String> {
    let mut buffer = state.log_buffer.lock().map_err(|e| e.to_string())?;
    buffer.clear_instance(&instance_id);
    Ok(())
}
