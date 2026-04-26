//! Tauri 命令模块

pub mod types;
pub mod utils;

pub mod app_config;
pub mod download;
pub mod file_ops;
pub mod kkafio;
pub mod state;
pub mod system;
pub mod tray;
pub mod update;

// 重新导出类型（供 lib.rs 使用）
pub use app_config::AppConfigState;
pub use types::AppState;
pub use kkafio::KkafioState;

// 重新导出辅助函数（供 lib.rs 使用）
pub use update::cleanup_dir_contents;

// 重新导出 Tauri 命令（供 lib.rs 直接调用的函数）
pub use file_ops::get_data_dir;
pub use file_ops::get_exe_dir;
