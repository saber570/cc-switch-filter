//! SwitchPlus identity and protection against shared CC Switch state.
use std::path::{Path, PathBuf};
use crate::error::AppError;

pub const APP_NAME: &str = "SwitchPlus";
pub const DATA_DIR: &str = ".switchplus";
pub const APP_UPDATES_ENABLED: bool = false;

pub const APP_IDENTIFIER: &str = "com.switchplus.desktop";
pub const DEFAULT_PROXY_PORT: u16 = 15731;

fn legacy_component(path: &Path) -> bool {
    path.components().any(|part| {
        let name = part.as_os_str().to_string_lossy();
        name.eq_ignore_ascii_case(".cc-switch")
            || name.eq_ignore_ascii_case("com.ccswitch.desktop")
    })
}

/// Includes symlink/junction targets and nonexistent children of existing paths.
pub fn is_cc_switch_path(path: &Path) -> bool {
    if legacy_component(path) { return true; }
    let mut parent = path;
    loop {
        if let Ok(resolved) = parent.canonicalize() {
            return legacy_component(&resolved);
        }
        match parent.parent() {
            Some(next) if next != parent => parent = next,
            _ => return false,
        }
    }
}

pub fn data_dir() -> PathBuf { crate::config::get_app_config_dir() }
pub fn device_dir() -> PathBuf { crate::config::get_home_dir().join(".switchplus") }

/// Only examines executable names; never reads CC Switch configuration or credentials.
#[cfg(windows)]
pub fn cc_switch_running() -> Result<bool, AppError> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::ProcessStatus::K32EnumProcesses;
    use windows_sys::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION};
    let mut pids = vec![0u32; 4096];
    let needed = loop {
        let mut needed = 0;
        let size = (pids.len() * std::mem::size_of::<u32>()) as u32;
        if unsafe { K32EnumProcesses(pids.as_mut_ptr(), size, &mut needed) } == 0 {
            return Err(AppError::Config("无法检查 CC Switch 运行状态，请稍后重试".into()));
        }
        if needed < size { break needed as usize / std::mem::size_of::<u32>(); }
        pids.resize(pids.len() * 2, 0);
    };
    for pid in pids.into_iter().take(needed) {
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if process.is_null() { continue; }
        let mut image = vec![0u16; 32768];
        let mut length = image.len() as u32;
        let ok = unsafe { QueryFullProcessImageNameW(process, 0, image.as_mut_ptr(), &mut length) };
        unsafe { CloseHandle(process); }
        if ok == 0 { continue; }
        let image = String::from_utf16_lossy(&image[..length as usize]);
        let name = Path::new(&image).file_name().unwrap_or_default().to_string_lossy();
        if ["cc-switch.exe", "ccswitch.exe", "CC Switch.exe"].iter().any(|expected| name.eq_ignore_ascii_case(expected)) {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(not(windows))]
pub fn cc_switch_running() -> Result<bool, AppError> { Ok(false) }

pub fn ensure_no_cc_switch() -> Result<(), AppError> {
    if cc_switch_running()? {
        Err(AppError::Config("CC Switch 正在运行。请先退出 CC Switch，再启动 SwitchPlus，避免两个程序同时修改客户端配置。".into()))
    } else { Ok(()) }
}

pub fn ensure_read_allowed(path: &Path) -> Result<(), AppError> {
    if is_cc_switch_path(path) {
        return Err(AppError::Config("SwitchPlus 不允许读取 CC Switch 数据目录".into()));
    }
    Ok(())
}

pub fn ensure_write_allowed(path: &Path) -> Result<(), AppError> {
    if is_cc_switch_path(path) || is_cc_switch_path(&device_dir()) || is_cc_switch_path(&data_dir()) {
        return Err(AppError::Config("SwitchPlus 不允许写入 CC Switch 数据目录".into()));
    }
    #[cfg(test)]
    if let Some(home) = std::env::var_os("SWITCHPLUS_TEST_HOME") {
        if path.starts_with(PathBuf::from(home)) { return Ok(()); }
    }
    // Isolated app data can always be saved. Client files require exclusive use.
    if path.starts_with(device_dir()) || path.starts_with(data_dir()) { return Ok(()); }
    ensure_no_cc_switch()
}

/// Runs before Tauri initializes, imports client configuration, or creates app data.
pub fn startup_allowed() -> bool {
    let check = ensure_no_cc_switch()
        .and_then(|_| ensure_read_allowed(&device_dir()))
        .and_then(|_| ensure_read_allowed(&data_dir()));
    let Err(error) = check else { return true; };
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONWARNING, MB_OK};
        let message: Vec<u16> = error.to_string().encode_utf16().chain(Some(0)).collect();
        let title: Vec<u16> = "SwitchPlus".encode_utf16().chain(Some(0)).collect();
        unsafe { MessageBoxW(std::ptr::null_mut(), message.as_ptr(), title.as_ptr(), MB_ICONWARNING | MB_OK); }
    }
    eprintln!("{error}");
    false
}
