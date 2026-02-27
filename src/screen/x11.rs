use anyhow::{Context, Result};
use image::{ImageBuffer, RgbaImage};
use x11rb::connection::Connection;
use x11rb::protocol::xproto;
use x11rb::rust_connection::RustConnection;

use super::{ImageFormat, Screenshot};

/// Capture the screen via X11 GetImage.
pub fn capture_x11(format: ImageFormat) -> Result<Screenshot> {
    let (conn, screen_num) = RustConnection::connect(None)
        .context("Failed to connect to X11 display")?;

    let screen = &conn.setup().roots[screen_num];
    let root = screen.root;
    let width = screen.width_in_pixels;
    let height = screen.height_in_pixels;

    // GetImage with ZPixmap format (raw pixels)
    let reply = xproto::get_image(
        &conn,
        xproto::ImageFormat::Z_PIXMAP,
        root,
        0,
        0,
        width,
        height,
        !0, // all planes
    )?
    .reply()
    .context("GetImage failed")?;

    // X11 returns BGRA (or BGRx) pixel data for 32-bit depth
    let depth = reply.depth;
    let data = reply.data;

    let img: RgbaImage = if depth == 24 || depth == 32 {
        // Convert BGRA to RGBA
        let mut rgba = Vec::with_capacity((width as usize) * (height as usize) * 4);
        for chunk in data.chunks(4) {
            if chunk.len() >= 4 {
                rgba.push(chunk[2]); // R (from B position)
                rgba.push(chunk[1]); // G
                rgba.push(chunk[0]); // B (from R position)
                rgba.push(255);      // A (force opaque)
            }
        }
        ImageBuffer::from_raw(width as u32, height as u32, rgba)
            .context("Failed to create image buffer")?
    } else {
        anyhow::bail!("Unsupported screen depth: {}", depth);
    };

    encode_image(&img, format, width as u32, height as u32)
}

fn encode_image(
    img: &RgbaImage,
    format: ImageFormat,
    width: u32,
    height: u32,
) -> Result<Screenshot> {
    let mut buf = Vec::new();
    let cursor = std::io::Cursor::new(&mut buf);

    match format {
        ImageFormat::Png => {
            let encoder = image::codecs::png::PngEncoder::new(cursor);
            image::ImageEncoder::write_image(
                encoder,
                img.as_raw(),
                width,
                height,
                image::ExtendedColorType::Rgba8,
            )
            .context("PNG encoding failed")?;
        }
        ImageFormat::Jpeg => {
            // JPEG doesn't support alpha; convert to RGB
            let rgb: image::RgbImage = image::DynamicImage::ImageRgba8(img.clone()).to_rgb8();
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
