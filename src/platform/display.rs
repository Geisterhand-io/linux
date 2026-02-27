use std::env;

#[derive(Debug, Clone, PartialEq)]
pub enum DisplayServer {
    X11,
    Wayland,
    Unknown,
}

impl std::fmt::Display for DisplayServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DisplayServer::X11 => write!(f, "x11"),
            DisplayServer::Wayland => write!(f, "wayland"),
            DisplayServer::Unknown => write!(f, "unknown"),
        }
    }
}

pub fn detect_display_server() -> DisplayServer {
    // XDG_SESSION_TYPE is the most reliable indicator
    if let Ok(session_type) = env::var("XDG_SESSION_TYPE") {
        match session_type.to_lowercase().as_str() {
            "wayland" => return DisplayServer::Wayland,
            "x11" => return DisplayServer::X11,
            _ => {}
        }
    }

    // Fallback: check WAYLAND_DISPLAY and DISPLAY
    if env::var("WAYLAND_DISPLAY").is_ok() {
        return DisplayServer::Wayland;
    }
    if env::var("DISPLAY").is_ok() {
        return DisplayServer::X11;
    }

    DisplayServer::Unknown
}

pub fn get_screen_size() -> Option<(f64, f64)> {
    // Phase 3/4 will implement proper screen size detection via X11/Wayland
    // For now, try xdpyinfo or xrandr as a rough fallback
    if let Ok(output) = std::process::Command::new("xdpyinfo")
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("dimensions:") {
                // "dimensions:    1920x1080 pixels ..."
                if let Some(dims) = trimmed.split_whitespace().nth(1) {
                    let parts: Vec<&str> = dims.split('x').collect();
                    if parts.len() == 2 {
                        if let (Ok(w), Ok(h)) = (parts[0].parse::<f64>(), parts[1].parse::<f64>()) {
                            return Some((w, h));
                        }
                    }
                }
            }
        }
    }
    None
}
