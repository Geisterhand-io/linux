use sysinfo::System;

use crate::models::api::AppInfo;

/// Get info about the frontmost (focused) application.
/// On Linux this uses AT-SPI2 or EWMH _NET_ACTIVE_WINDOW.
/// For now, uses /proc + wmctrl-style heuristic.
pub fn get_frontmost_app() -> Option<AppInfo> {
    // Try reading _NET_ACTIVE_WINDOW via xdotool
    let output = std::process::Command::new("xdotool")
        .args(["getactivewindow", "getwindowpid"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let pid_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let pid: i32 = pid_str.parse().ok()?;

    get_app_info_by_pid(pid)
}

/// Get application info by PID.
pub fn get_app_info_by_pid(pid: i32) -> Option<AppInfo> {
    let sys = System::new_all();
    let sysinfo_pid = sysinfo::Pid::from(pid as usize);

    let process = sys.process(sysinfo_pid)?;
    let name = process.name().to_string_lossy().to_string();

    Some(AppInfo {
        name,
        desktop_file: find_desktop_file_for_pid(pid),
        process_identifier: pid,
    })
}

/// Try to find the .desktop file associated with a process.
fn find_desktop_file_for_pid(pid: i32) -> Option<String> {
    // Read /proc/<pid>/cmdline to get the binary name
    let cmdline = std::fs::read_to_string(format!("/proc/{pid}/cmdline")).ok()?;
    let binary = cmdline.split('\0').next()?;
    let binary_name = std::path::Path::new(binary).file_name()?.to_str()?;

    // Search common .desktop file locations
    let search_dirs = [
        "/usr/share/applications",
        "/usr/local/share/applications",
        &format!(
            "{}/.local/share/applications",
            std::env::var("HOME").unwrap_or_default()
        ),
    ];

    for dir in &search_dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "desktop") {
                    if let Ok(contents) = std::fs::read_to_string(&path) {
                        // Check if Exec= line contains the binary name
                        for line in contents.lines() {
                            if line.starts_with("Exec=") && line.contains(binary_name) {
                                return Some(
                                    path.file_name()?.to_str()?.to_string(),
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    None
}
