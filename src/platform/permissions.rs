use std::path::Path;

/// Check if the current user has access to /dev/uinput for virtual input devices.
pub fn check_uinput_access() -> bool {
    let path = Path::new("/dev/uinput");
    if !path.exists() {
        return false;
    }
    // Try opening for write to verify permission
    std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .is_ok()
}

/// Check if AT-SPI2 D-Bus service is available.
/// This is a quick synchronous check — tests if the AT-SPI2 bus address is set
/// or if the registryd process is running.
pub fn check_at_spi2_available() -> bool {
    // Check if AT_SPI_BUS_ADDRESS is set (most reliable)
    if std::env::var("AT_SPI_BUS_ADDRESS").is_ok() {
        return true;
    }

    // Check if the AT-SPI2 registryd is running
    if let Ok(output) = std::process::Command::new("pgrep")
        .args(["-f", "at-spi2-registryd"])
        .output()
    {
        if output.status.success() {
            return true;
        }
    }

    // Check for the well-known D-Bus name (org.a11y.Bus)
    if let Ok(output) = std::process::Command::new("dbus-send")
        .args([
            "--session",
            "--dest=org.freedesktop.DBus",
            "--type=method_call",
            "--print-reply",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus.ListNames",
        ])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("org.a11y.Bus") {
            return true;
        }
    }

    false
}

/// Check if screen capture is available.
/// On X11 this is always true; on Wayland it depends on xdg-desktop-portal.
pub fn check_screen_capture_available() -> bool {
    // On X11, screen capture is always available via SHM
    if let Ok(session) = std::env::var("XDG_SESSION_TYPE") {
        if session == "x11" {
            return true;
        }
    }

    // On Wayland, check if xdg-desktop-portal is available
    if let Ok(output) = std::process::Command::new("dbus-send")
        .args([
            "--session",
            "--dest=org.freedesktop.DBus",
            "--type=method_call",
            "--print-reply",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus.ListNames",
        ])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("org.freedesktop.portal.Desktop") {
            return true;
        }
    }

    false
}
