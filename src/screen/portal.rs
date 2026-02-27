use std::collections::HashMap;

use anyhow::{Context, Result};
use zbus::zvariant::{OwnedValue, Value};

use super::{ImageFormat, Screenshot};

/// Capture the screen using xdg-desktop-portal Screenshot interface.
/// This is the most reliable approach on GNOME Wayland.
pub async fn capture_portal(format: ImageFormat) -> Result<Screenshot> {
    // Try xdg-desktop-portal first (most reliable on modern GNOME)
    if let Ok(shot) = capture_via_xdg_portal(format).await {
        return Ok(shot);
    }

    // Try gnome-screenshot CLI
    if let Ok(shot) = capture_via_gnome_screenshot(format).await {
        return Ok(shot);
    }

    // Try grim (wlroots Wayland)
    capture_via_grim(format)
}

/// Capture using xdg-desktop-portal Screenshot interface.
async fn capture_via_xdg_portal(format: ImageFormat) -> Result<Screenshot> {
    let conn = zbus::Connection::session()
        .await
        .context("Failed to connect to D-Bus session bus")?;

    // Build options dict
    let mut options: HashMap<String, Value<'_>> = HashMap::new();
    options.insert("interactive".to_string(), Value::Bool(false));

    // Call org.freedesktop.portal.Screenshot.Screenshot
    let reply = conn
        .call_method(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/portal/desktop",
            Some("org.freedesktop.portal.Screenshot"),
            "Screenshot",
            &("", options),
        )
        .await
        .context("Portal Screenshot call failed")?;

    let request_path: zbus::zvariant::OwnedObjectPath = reply
        .body()
        .deserialize()
        .context("Failed to parse request path")?;

    // Subscribe to the Response signal on the request object BEFORE waiting
    // Use a match rule to listen for the signal
    let rule = format!(
        "type='signal',interface='org.freedesktop.portal.Request',member='Response',path='{}'",
        request_path.as_str()
    );

    conn.call_method(
        Some("org.freedesktop.DBus"),
        "/org/freedesktop/DBus",
        Some("org.freedesktop.DBus"),
        "AddMatch",
        &(&rule),
    )
    .await
    .context("Failed to add match rule")?;

    // Wait for the Response signal with timeout
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        wait_for_portal_response(&conn, request_path.as_str()),
    )
    .await
    .context("Portal screenshot timed out")??;

    read_and_encode(&result, format)
}

/// Wait for and parse the Response signal from the portal.
async fn wait_for_portal_response(conn: &zbus::Connection, request_path: &str) -> Result<String> {
    use futures_util::StreamExt;

    let mut stream = zbus::MessageStream::from(conn);

    while let Some(msg) = stream.next().await {
        let msg = msg.context("Error receiving D-Bus message")?;

        // Check if this is the Response signal we're waiting for
        let header = msg.header();
        if header.message_type() != zbus::message::Type::Signal {
            continue;
        }

        let member = header.member().map(|m| m.as_str().to_string());
        let path = header.path().map(|p| p.as_str().to_string());
        let interface = header.interface().map(|i| i.as_str().to_string());

        if member.as_deref() != Some("Response") {
            continue;
        }
        if interface.as_deref() != Some("org.freedesktop.portal.Request") {
            continue;
        }
        if path.as_deref() != Some(request_path) {
            continue;
        }

        // Parse the response
        let (response_code, results): (u32, HashMap<String, OwnedValue>) = msg
            .body()
            .deserialize()
            .context("Failed to parse portal response body")?;

        if response_code != 0 {
            anyhow::bail!(
                "Portal screenshot denied or failed (code: {})",
                response_code
            );
        }

        // Get the URI
        let uri = results
            .get("uri")
            .and_then(|v| {
                let s: Result<String, _> = v.try_clone().unwrap().try_into();
                s.ok()
            })
            .ok_or_else(|| anyhow::anyhow!("No URI in portal response"))?;

        // Convert file:// URI to path
        let path = uri
            .strip_prefix("file://")
            .ok_or_else(|| anyhow::anyhow!("Unexpected URI scheme: {}", uri))?;

        return Ok(path.to_string());
    }

    anyhow::bail!("D-Bus message stream ended without portal response")
}

/// Capture using gnome-screenshot CLI.
async fn capture_via_gnome_screenshot(format: ImageFormat) -> Result<Screenshot> {
    let tmp_path = format!("/tmp/geisterhand-screenshot-{}.png", std::process::id());

    let output = tokio::process::Command::new("gnome-screenshot")
        .args(["-f", &tmp_path])
        .output()
        .await
        .context("gnome-screenshot not found")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("gnome-screenshot failed: {}", stderr);
    }

    read_and_encode(&tmp_path, format)
}

/// Capture using grim (Wayland-native for wlroots compositors).
fn capture_via_grim(format: ImageFormat) -> Result<Screenshot> {
    let tmp_path = format!("/tmp/geisterhand-screenshot-{}.png", std::process::id());

    let output = std::process::Command::new("grim")
        .arg(&tmp_path)
        .output()
        .context("grim not found")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("grim failed: {}", stderr);
    }

    read_and_encode(&tmp_path, format)
}

fn read_and_encode(path: &str, format: ImageFormat) -> Result<Screenshot> {
    let img = image::open(path).context("Failed to read screenshot file")?;
    let _ = std::fs::remove_file(path);

    let width = img.width();
    let height = img.height();

    let mut buf = Vec::new();
    let cursor = std::io::Cursor::new(&mut buf);

    match format {
        ImageFormat::Png => {
            let rgba = img.to_rgba8();
            let encoder = image::codecs::png::PngEncoder::new(cursor);
            image::ImageEncoder::write_image(
                encoder,
                rgba.as_raw(),
                width,
                height,
                image::ExtendedColorType::Rgba8,
            )
            .context("PNG encoding failed")?;
        }
        ImageFormat::Jpeg => {
            // JPEG doesn't support alpha; convert to RGB
            let rgb = img.to_rgb8();
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(cursor, 90);
            image::ImageEncoder::write_image(
                encoder,
                rgb.as_raw(),
                width,
                height,
                image::ExtendedColorType::Rgb8,
            )
            .context("JPEG encoding failed")?;
        }
    }

    Ok(Screenshot {
        data: buf,
        format,
        width,
        height,
    })
}
