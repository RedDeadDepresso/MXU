//! 系统相关命令
//!
//! 提供权限检查、系统信息查询、全局选项设置等功能

use super::types::{SystemInfo, WebView2DirInfo};
use log::info;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::time::sleep;
use std::time::Duration;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 标记是否检测到可能缺少 VC++ 运行库
static VCREDIST_MISSING: AtomicBool = AtomicBool::new(false);

/// 设置 VC++ 运行库缺失标记 (供内部调用)
pub fn set_vcredist_missing(missing: bool) {
    VCREDIST_MISSING.store(missing, Ordering::SeqCst);
}

/// 检查当前进程是否以管理员权限运行
#[tauri::command]
pub fn is_elevated() -> bool {
    #[cfg(windows)]
    {
        use winsafe::co::{TOKEN, TOKEN_INFORMATION_CLASS};
        use winsafe::{TokenInfo, HPROCESS};

        if let Ok(token_handle) = HPROCESS::GetCurrentProcess().OpenProcessToken(TOKEN::QUERY) {
            let result = token_handle.GetTokenInformation(TOKEN_INFORMATION_CLASS::Elevation);
            if let Ok(TokenInfo::Elevation(elevation)) = result {
                elevation.TokenIsElevated()
            } else {
                false
            }
        } else {
            false
        }
    }

    #[cfg(not(windows))]
    {
        unsafe { libc::geteuid() == 0 }
    }
}

/// 以管理员权限重启应用
#[tauri::command]
pub fn restart_as_admin(app_handle: tauri::AppHandle) -> Result<(), String> {
    #[cfg(windows)]
    {
        use winsafe::co::{SEE_MASK, SW};
        use winsafe::{ShellExecuteEx, SHELLEXECUTEINFO};

        let exe_path = std::env::current_exe().map_err(|e| format!("获取程序路径失败: {}", e))?;
        let exe_path_str = exe_path.to_string_lossy().to_string();

        info!("restart_as_admin: restarting with admin privileges");

        let result = ShellExecuteEx(&SHELLEXECUTEINFO {
            file: &exe_path_str,
            verb: Option::from("runas"),
            show: SW::SHOWNORMAL,
            mask: SEE_MASK::NOASYNC | SEE_MASK::FLAG_NO_UI,
            ..Default::default()
        });

        if let Err(e) = result {
            Err(format!("以管理员身份启动失败: 错误码 {}", e.raw()))
        } else {
            info!("restart_as_admin: new process started, exiting current");
            app_handle.exit(0);
            Ok(())
        }
    }

    #[cfg(not(windows))]
    {
        let _ = app_handle;
        Err("此功能仅在 Windows 上可用".to_string())
    }
}

/// 打开文件（使用系统默认程序）
#[tauri::command]
pub async fn open_file(file_path: String) -> Result<(), String> {
    info!("open_file: {}", file_path);

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use std::process::Command;
        Command::new("cmd")
            .args(["/c", "start", "", &file_path])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        Command::new("open")
            .arg(&file_path)
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        Command::new("xdg-open")
            .arg(&file_path)
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
    }

    Ok(())
}

/// 运行程序并等待其退出
#[tauri::command]
pub async fn run_and_wait(file_path: String) -> Result<i32, String> {
    info!("run_and_wait: {}", file_path);

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use std::process::Command;
        let status = Command::new(&file_path)
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .map_err(|e| format!("Failed to run file: {}", e))?;

        let exit_code = status.code().unwrap_or(-1);
        info!("run_and_wait finished with exit code: {}", exit_code);
        Ok(exit_code)
    }

    #[cfg(not(windows))]
    {
        let _ = file_path;
        Err("run_and_wait is only supported on Windows".to_string())
    }
}

/// 检查指定程序是否正在运行（通过完整路径比较）
pub fn check_process_running(program: &str) -> bool {
    use std::path::PathBuf;

    let resolved_path = PathBuf::from(program);
    let canonical_target = resolved_path
        .canonicalize()
        .unwrap_or_else(|_| resolved_path.clone());

    #[cfg(windows)]
    {
        use winsafe::co::{PROCESS, PROCESS_NAME, TH32CS};
        use winsafe::{HPROCESS, HPROCESSLIST};

        let file_name = match resolved_path.file_name() {
            Some(name) => name.to_string_lossy().to_string(),
            None => return false,
        };
        let file_name_lower = file_name.to_lowercase();
        let target_lower = canonical_target.to_string_lossy().to_lowercase();

        let mut snapshot = match HPROCESSLIST::CreateToolhelp32Snapshot(TH32CS::SNAPPROCESS, None) {
            Ok(h) => h,
            Err(_) => return false,
        };
        for process_result in snapshot.iter_processes() {
            if let Ok(entry) = process_result {
                if entry.szExeFile().to_lowercase() == file_name_lower {
                    if let Ok(process) = HPROCESS::OpenProcess(
                        PROCESS::QUERY_LIMITED_INFORMATION,
                        false,
                        entry.th32ProcessID,
                    ) {
                        if let Ok(running_path) =
                            process.QueryFullProcessImageName(PROCESS_NAME::WIN32)
                        {
                            let running_canonical = PathBuf::from(&running_path)
                                .canonicalize()
                                .map(|p| p.to_string_lossy().to_lowercase())
                                .unwrap_or_else(|_| running_path.to_lowercase());
                            if running_canonical == target_lower {
                                return true;
                            }
                        }
                    }
                }
            } else {
                break;
            }
        }
        false
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(proc_dir) = std::fs::read_dir("/proc") {
            for entry in proc_dir.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if !name_str.chars().all(|c| c.is_ascii_digit()) {
                    continue;
                }
                let exe_link = entry.path().join("exe");
                if let Ok(resolved) = std::fs::read_link(&exe_link) {
                    let canonical = resolved.canonicalize().unwrap_or(resolved);
                    if canonical == canonical_target {
                        return true;
                    }
                }
            }
        }
        false
    }

    #[cfg(target_os = "macos")]
    {
        extern "C" {
            fn proc_listallpids(buffer: *mut i32, buffersize: i32) -> i32;
            fn proc_pidpath(pid: i32, buffer: *mut u8, buffersize: u32) -> i32;
        }
        unsafe {
            let mut capacity = 1024usize;
            let num_pids;
            let mut pids;
            loop {
                pids = vec![0i32; capacity];
                let buf_size = (capacity * std::mem::size_of::<i32>()) as i32;
                let actual = proc_listallpids(pids.as_mut_ptr(), buf_size);
                if actual <= 0 { return false; }
                if actual as usize >= capacity { capacity *= 2; continue; }
                num_pids = actual as usize;
                break;
            }
            let mut path_buf = [0u8; 4096];
            for &pid in &pids[..num_pids] {
                if pid == 0 { continue; }
                let ret = proc_pidpath(pid, path_buf.as_mut_ptr(), path_buf.len() as u32);
                if ret <= 0 { continue; }
                if let Ok(path_str) = std::str::from_utf8(&path_buf[..ret as usize]) {
                    let pid_path = PathBuf::from(path_str);
                    let canonical = pid_path.canonicalize().unwrap_or(pid_path);
                    if canonical == canonical_target { return true; }
                }
            }
        }
        false
    }
}

#[tauri::command]
pub fn is_process_running(program: String) -> bool {
    check_process_running(&program)
}

/// 根据窗口句柄获取对应进程的可执行文件路径
#[tauri::command]
pub fn get_process_path_from_hwnd(hwnd: u64) -> Result<String, String> {
    #[cfg(windows)]
    {
        use winsafe::co::{PROCESS, PROCESS_NAME};
        use winsafe::{HPROCESS, HWND};

        if hwnd == 0 {
            return Err("Invalid window handle (null)".to_string());
        }
        let hwnd = unsafe { HWND::from_ptr(hwnd as *mut _) };
        let (_, pid) = hwnd.GetWindowThreadProcessId();
        if pid == 0 {
            return Err("PID is 0".to_string());
        }
        let process = HPROCESS::OpenProcess(PROCESS::QUERY_LIMITED_INFORMATION, false, pid)
            .map_err(|e| format!("OpenProcess failed: {}", e))?;
        let path = process
            .QueryFullProcessImageName(PROCESS_NAME::WIN32)
            .map_err(|e| format!("QueryFullProcessImageName failed: {}", e))?;
        Ok(path)
    }

    #[cfg(not(windows))]
    {
        let _ = hwnd;
        Err("This command is only available on Windows".to_string())
    }
}

/// 检查是否检测到 VC++ 运行库缺失（检查后自动清除标记）
#[tauri::command]
pub fn check_vcredist_missing() -> bool {
    let missing = VCREDIST_MISSING.swap(false, Ordering::SeqCst);
    if missing {
        info!("VC++ runtime missing detected, notifying frontend");
    }
    missing
}

/// 检查本次启动是否来自开机自启动
#[tauri::command]
pub fn is_autostart() -> bool {
    std::env::args().any(|arg| arg == "--autostart")
}

/// 检查命令行是否包含 -h/--help 参数
pub fn has_help_flag() -> bool {
    std::env::args()
        .skip(1)
        .any(|arg| arg == "-h" || arg == "--help")
}

/// 生成命令行帮助文本
pub fn get_cli_help_text() -> String {
    let exe_name = std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "mxu".to_string());

    format!(
        "\
MXU 命令行参数

用法:
  {exe_name} [参数]

参数:
  -h, --help
      显示本帮助并退出

  --autostart
      以开机自启动模式运行
      通常由 MXU 创建的系统自启动任务自动传入

  -i, --instance <实例名>
      指定自动执行时使用的实例名
      仅在 --autostart 模式下生效

  -q, --quit-after-run
      当本次启动实际触发自动执行后，在任务完成时自动退出

示例:
  {exe_name} --autostart --instance \"日常任务\"
  {exe_name} --autostart -i \"日常任务\" --quit-after-run
"
    )
}

fn get_cli_arg_value(short: &str, long: &str) -> Option<String> {
    let short_eq = format!("{}=", short);
    let long_eq = format!("{}=", long);
    let args: Vec<String> = std::env::args().collect();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == short || arg == long {
            if let Some(value) = iter.next() {
                if !value.starts_with('-') {
                    return Some(value.clone());
                }
            }
            return None;
        }
        if let Some(value) = arg.strip_prefix(&short_eq) {
            return Some(value.to_string());
        }
        if let Some(value) = arg.strip_prefix(&long_eq) {
            return Some(value.to_string());
        }
    }
    None
}

#[tauri::command]
pub fn get_start_instance() -> Option<String> {
    get_cli_arg_value("-i", "--instance")
}

#[tauri::command]
pub fn has_quit_after_run_flag() -> bool {
    std::env::args().any(|arg| arg == "-q" || arg == "--quit-after-run")
}

#[cfg(windows)]
pub fn migrate_legacy_autostart() {
    if has_legacy_registry_autostart() {
        if create_schtask_autostart().is_ok() {
            remove_legacy_registry_autostart();
        }
    }
    if schtask_autostart_needs_refresh() {
        let _ = create_schtask_autostart();
    }
}

#[cfg(windows)]
fn create_schtask_autostart() -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let exe_path = std::env::current_exe().map_err(|e| format!("获取程序路径失败: {}", e))?;
    let exe = exe_path.to_string_lossy();
    let output = std::process::Command::new("schtasks")
        .args([
            "/create", "/tn", "MXU",
            "/tr", &format!("\"{}\" --autostart", exe),
            "/sc", "onlogon",
            "/delay", "0000:30",
            "/it", "/rl", "highest", "/f",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("执行 schtasks 失败: {}", e))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("创建计划任务失败: {}", stderr));
    }
    Ok(())
}

#[cfg(windows)]
fn schtask_autostart_needs_refresh() -> bool {
    use regex::Regex;
    use std::os::windows::process::CommandExt;
    let output = match std::process::Command::new("schtasks")
        .args(["/query", "/tn", "MXU", "/xml"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return false,
    };
    let xml = String::from_utf8_lossy(&output.stdout);
    let tag_equals = |tag: &str, expected: &str| -> bool {
        let pattern = format!(
            r"(?is)<\s*{}\s*>\s*{}\s*<\s*/\s*{}\s*>",
            regex::escape(tag), regex::escape(expected), regex::escape(tag)
        );
        Regex::new(&pattern).map(|re| re.is_match(&xml)).unwrap_or(false)
    };
    if !tag_equals("Enabled", "true") { return false; }
    !(tag_equals("LogonType", "InteractiveToken") && tag_equals("Delay", "PT30S"))
}

#[cfg(windows)]
fn remove_legacy_registry_autostart() {
    use winsafe::co::{KEY, REG_OPTION};
    use winsafe::HKEY;
    let key_result = HKEY::CURRENT_USER.RegOpenKeyEx(
        Some(r"Software\Microsoft\Windows\CurrentVersion\Run"),
        REG_OPTION::NoValue,
        KEY::SET_VALUE | KEY::QUERY_VALUE,
    );
    if let Ok(key) = key_result {
        for name in &["mxu", "MXU"] {
            let _ = key.RegDeleteValue(Some(name));
        }
    }
}

#[cfg(windows)]
fn has_legacy_registry_autostart() -> bool {
    use winsafe::co::{KEY, REG_OPTION};
    use winsafe::HKEY;
    let key_result = HKEY::CURRENT_USER.RegOpenKeyEx(
        Some(r"Software\Microsoft\Windows\CurrentVersion\Run"),
        REG_OPTION::NoValue,
        KEY::QUERY_VALUE,
    );
    if let Ok(key) = key_result {
        ["mxu", "MXU"].iter().any(|name| key.RegQueryValueEx(Some(name)).is_ok())
    } else {
        false
    }
}

#[tauri::command]
pub fn autostart_enable() -> Result<(), String> {
    #[cfg(windows)]
    {
        create_schtask_autostart()?;
        remove_legacy_registry_autostart();
        Ok(())
    }
    #[cfg(not(windows))]
    Err("此功能仅在 Windows 上可用".to_string())
}

#[tauri::command]
pub fn autostart_disable() -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("schtasks")
            .args(["/delete", "/tn", "MXU", "/f"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        remove_legacy_registry_autostart();
        Ok(())
    }
    #[cfg(not(windows))]
    Err("此功能仅在 Windows 上可用".to_string())
}

#[tauri::command]
pub fn autostart_is_enabled() -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let schtask = std::process::Command::new("schtasks")
            .args(["/query", "/tn", "MXU"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        schtask || has_legacy_registry_autostart()
    }
    #[cfg(not(windows))]
    false
}

#[tauri::command]
pub fn get_arch() -> String {
    std::env::consts::ARCH.to_string()
}

#[tauri::command]
pub fn get_os() -> String {
    std::env::consts::OS.to_string()
}

#[tauri::command]
pub fn get_system_info() -> SystemInfo {
    let os = std::env::consts::OS.to_string();
    let info = os_info::get();
    let os_version = format!("{} {}", info.os_type(), info.version());
    let arch = std::env::consts::ARCH.to_string();
    let tauri_version = tauri::VERSION.to_string();
    SystemInfo { os, os_version, arch, tauri_version }
}

/// 获取 Web 服务器实际监听端口
#[tauri::command]
pub async fn get_web_server_port() -> u16 {
    let port = crate::web_server::get_actual_port();
    if port != 0 { return port; }
    for _ in 0..50 {
        sleep(Duration::from_millis(100)).await;
        let port = crate::web_server::get_actual_port();
        if port != 0 { return port; }
    }
    0
}

/// 获取本机局域网 IP
#[tauri::command]
pub fn get_local_lan_ip() -> Option<String> {
    crate::web_server::get_local_ip().map(|s| s.to_string())
}

/// 获取当前使用的 WebView2 目录
#[tauri::command]
pub fn get_webview2_dir() -> WebView2DirInfo {
    if let Ok(folder) = std::env::var("WEBVIEW2_BROWSER_EXECUTABLE_FOLDER") {
        WebView2DirInfo { path: folder, system: false }
    } else {
        WebView2DirInfo { path: String::new(), system: true }
    }
}
