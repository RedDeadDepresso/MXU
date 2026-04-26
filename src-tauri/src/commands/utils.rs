//! 辅助函数
//!
//! 提供路径处理和其他通用工具函数

use crate::ws_broadcast::{WsBroadcast, WsEvent};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

/// 发送配置变更事件（双通道：WS 浏览器客户端 + Tauri WebView）
pub fn emit_config_changed(app: &AppHandle) {
    if let Some(ws) = app.try_state::<Arc<WsBroadcast>>() {
        ws.send(WsEvent::ConfigChanged);
    }
    if let Err(e) = app.emit("config-changed-external", ()) {
        log::error!("Failed to emit config-changed-external: {}", e);
    }
}

/// 获取应用数据目录
/// - Windows: %APPDATA%/KKAFIO
/// - macOS:   ~/Library/Application Support/KKAFIO
/// - Linux:   ~/.config/KKAFIO  (XDG_CONFIG_HOME fallback)
pub fn get_app_data_dir() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var("APPDATA")
            .map_err(|_| "Cannot read APPDATA environment variable".to_string())?;
        let path = PathBuf::from(appdata).join("KKAFIO");
        std::fs::create_dir_all(&path)
            .map_err(|e| format!("Cannot create KKAFIO data dir: {}", e))?;
        Ok(path)
    }

    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME")
            .map_err(|_| "Cannot read HOME environment variable".to_string())?;
        let path = PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("KKAFIO");
        std::fs::create_dir_all(&path)
            .map_err(|e| format!("Cannot create KKAFIO data dir: {}", e))?;
        Ok(path)
    }

    #[cfg(target_os = "linux")]
    {
        // Respect XDG_CONFIG_HOME if set, otherwise fall back to ~/.config
        let config_home = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            format!("{}/.config", home)
        });
        let path = PathBuf::from(config_home).join("KKAFIO");
        std::fs::create_dir_all(&path)
            .map_err(|e| format!("Cannot create KKAFIO data dir: {}", e))?;
        Ok(path)
    }
}

/// 规范化路径：移除冗余的 `.`、处理 `..`、统一分隔符
pub fn normalize_path(path: &str) -> PathBuf {
    use std::path::{Component, Path};

    let path = Path::new(path);
    let mut components = Vec::new();

    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if matches!(components.last(), Some(Component::Normal(_))) {
                    components.pop();
                } else {
                    components.push(component);
                }
            }
            _ => components.push(component),
        }
    }

    components.into_iter().collect()
}

/// 获取日志目录（应用数据目录下的 debug 子目录）
pub fn get_logs_dir() -> PathBuf {
    get_app_data_dir()
        .unwrap_or_else(|_| {
            let exe_path = std::env::current_exe().unwrap_or_default();
            exe_path
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .to_path_buf()
        })
        .join("debug")
}

/// 获取 exe 所在目录路径（内部使用）
pub fn get_exe_directory() -> Result<PathBuf, String> {
    let exe_path = std::env::current_exe().map_err(|e| format!("获取 exe 路径失败: {}", e))?;
    exe_path
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| "无法获取 exe 所在目录".to_string())
}

/// 构建 User-Agent 字符串
pub fn build_user_agent() -> String {
    let version = env!("CARGO_PKG_VERSION");
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let tauri_version = tauri::VERSION;
    format!("MXU/{} ({}; {}) Tauri/{}", version, os, arch, tauri_version)
}

/// 构建启动程序的 Command
pub fn build_launch_command(
    program: &str,
    args: &[String],
    use_cmd: bool,
) -> std::process::Command {
    use std::process::Stdio;

    let mut cmd = if cfg!(target_os = "windows") && use_cmd {
        let mut c = std::process::Command::new("cmd.exe");
        c.arg("/c").arg(program);
        if !args.is_empty() {
            c.args(args);
        }
        c
    } else {
        let mut c = std::process::Command::new(program);
        if !args.is_empty() {
            c.args(args);
        }
        c
    };

    cmd.stdout(Stdio::null()).stderr(Stdio::null());

    #[cfg(target_os = "windows")]
    if use_cmd {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
        cmd.creation_flags(CREATE_NO_WINDOW | CREATE_BREAKAWAY_FROM_JOB);
    }

    cmd
}
