//! Per-user logon start of the mount: a value in `HKCU\...\Run` (no administrator needed, survives
//! reboots), started now and stopped through the mount's stop event.
use crate::config::state_dir;
use anyhow::{Context, Result};
use std::os::windows::io::IntoRawHandle;
use std::os::windows::process::CommandExt;
use std::path::Path;
use windows::Win32::Foundation::{CloseHandle, ERROR_FILE_NOT_FOUND, HANDLE};
use windows::Win32::System::Console::{STD_ERROR_HANDLE, SetStdHandle};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_SZ, RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW,
};
use windows::Win32::System::Threading::{
    CREATE_NEW_PROCESS_GROUP, CREATE_NO_WINDOW, DETACHED_PROCESS, EVENT_MODIFY_STATE, OpenEventW, SetEvent,
};
use windows::core::{PCWSTR, w};

const RUN_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const VALUE: PCWSTR = w!("nie-link");

/// Send this process's stderr to `file` (append); used by the headless logon start.
pub fn redirect_stderr(file: &Path) -> Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let f = std::fs::OpenOptions::new().create(true).append(true).open(file)?;
    unsafe { SetStdHandle(STD_ERROR_HANDLE, HANDLE(f.into_raw_handle())) }.context("SetStdHandle")
}

fn args(config: Option<&Path>, record: Option<&str>, prefetch: Option<&str>, log: &Path) -> Vec<String> {
    let mut a = vec!["mount".to_owned()];
    if let Some(c) = config {
        a.push("--config".into());
        a.push(c.display().to_string());
    }
    for (flag, v) in [("--record", record), ("--prefetch", prefetch)] {
        if let Some(v) = v {
            a.push(flag.into());
            a.push(v.into());
        }
    }
    a.push("--log".into());
    a.push(log.display().to_string());
    a
}

fn quote(s: &str) -> String {
    format!("\"{s}\"")
}

pub fn install(config: Option<&Path>, record: Option<&str>, prefetch: Option<&str>) -> Result<()> {
    let exe = std::env::current_exe()?;
    let log = state_dir().join("mount.log");
    let a = args(config, record, prefetch, &log);
    let line = format!(
        "conhost.exe --headless {} {}",
        quote(&exe.display().to_string()),
        a.iter().map(|x| quote(x)).collect::<Vec<_>>().join(" ")
    );
    let wide: Vec<u16> = line.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes: Vec<u8> = wide.iter().flat_map(|c| c.to_le_bytes()).collect();
    let mut key = HKEY::default();
    unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, RUN_KEY, None, KEY_SET_VALUE, &mut key) }
        .ok()
        .context("open HKCU Run")?;
    let r = unsafe { RegSetValueExW(key, VALUE, None, REG_SZ, Some(&bytes)) };
    let _ = unsafe { RegCloseKey(key) };
    r.ok().context("write Run value")?;

    std::process::Command::new(&exe)
        .args(&a)
        .creation_flags((DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW).0)
        .spawn()
        .context("start mount")?;
    println!("registered at logon: {line}");
    println!("started now, log: {}", log.display());
    Ok(())
}

pub fn uninstall() -> Result<()> {
    let mut key = HKEY::default();
    unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, RUN_KEY, None, KEY_SET_VALUE, &mut key) }
        .ok()
        .context("open HKCU Run")?;
    let r = unsafe { RegDeleteValueW(key, VALUE) };
    let _ = unsafe { RegCloseKey(key) };
    if r != ERROR_FILE_NOT_FOUND {
        r.ok().context("delete Run value")?;
    }
    match unsafe { OpenEventW(EVENT_MODIFY_STATE, false, crate::projfs::STOP_EVENT) } {
        Ok(h) => {
            unsafe { SetEvent(h) }.context("stop event")?;
            let _ = unsafe { CloseHandle(h) };
            println!("logon start removed, running mount told to stop");
        }
        Err(_) => println!("logon start removed, no mount was running"),
    }
    Ok(())
}
