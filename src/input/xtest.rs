use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use evdev::Key;
use x11rb::connection::{Connection, RequestConnection};
use x11rb::protocol::xproto;
use x11rb::protocol::xtest;
use x11rb::rust_connection::RustConnection;

use crate::input::backend::InputBackend;
use crate::input::keycode_map;
use crate::models::api::MouseButton;

/// Input backend using X11 XTest extension.
/// Only works on X11, but requires no special permissions.
pub struct XTestBackend {
    conn: RustConnection,
    root_window: u32,
}

impl XTestBackend {
    pub fn new() -> Result<Self> {
        let (conn, screen_num) = RustConnection::connect(None)
            .context("Failed to connect to X11 display")?;

        // Check XTest extension is available
        conn.extension_information(xtest::X11_EXTENSION_NAME)?
            .ok_or_else(|| anyhow::anyhow!("XTest extension not available"))?;

        let screen = &conn.setup().roots[screen_num];
        let root_window = screen.root;

        Ok(Self { conn, root_window })
    }

    /// Convert evdev keycode to X11 keycode (offset by 8).
    fn evdev_to_x11_keycode(&self, evdev_code: u16) -> u8 {
        (evdev_code + 8) as u8
    }

    /// Convert MouseButton to X11 button number.
    fn button_to_x11(&self, button: &MouseButton) -> u8 {
        match button {
            MouseButton::Left => 1,
            MouseButton::Center => 2,
            MouseButton::Right => 3,
        }
    }

    /// Helper to call fake_input with standard defaults.
    fn fake_input(&self, type_: u8, detail: u8, root_x: i16, root_y: i16) -> Result<()> {
        xtest::fake_input(
            &self.conn,
            type_,
            detail,
            0,                  // time (0 = current)
            self.root_window,
            root_x,
            root_y,
            0,                  // deviceid (0 = core device)
        )?;
        Ok(())
    }
}

impl InputBackend for XTestBackend {
    fn mouse_move(&self, x: f64, y: f64) -> Result<()> {
        self.fake_input(xproto::MOTION_NOTIFY_EVENT, 0, x as i16, y as i16)?;
        self.conn.flush()?;
        Ok(())
    }

    fn mouse_click(&self, button: MouseButton, count: u32) -> Result<()> {
        let btn = self.button_to_x11(&button);

        for _ in 0..count {
            self.fake_input(xproto::BUTTON_PRESS_EVENT, btn, 0, 0)?;
            self.conn.flush()?;
            thread::sleep(Duration::from_millis(10));

            self.fake_input(xproto::BUTTON_RELEASE_EVENT, btn, 0, 0)?;
            self.conn.flush()?;

            if count > 1 {
                thread::sleep(Duration::from_millis(50));
            }
        }
        Ok(())
    }

    fn scroll(&self, delta_x: f64, delta_y: f64) -> Result<()> {
        // X11 scroll is button 4 (up), 5 (down), 6 (left), 7 (right)
        let steps_y = delta_y.abs() as u32;
        let steps_x = delta_x.abs() as u32;

        // Vertical scroll
        if delta_y.abs() > 0.0 {
            let btn_v = if delta_y < 0.0 { 4u8 } else { 5u8 };
            for _ in 0..steps_y.max(1) {
                self.fake_input(xproto::BUTTON_PRESS_EVENT, btn_v, 0, 0)?;
                self.fake_input(xproto::BUTTON_RELEASE_EVENT, btn_v, 0, 0)?;
            }
        }

        // Horizontal scroll
        if delta_x.abs() > 0.0 {
            let btn_h = if delta_x > 0.0 { 7u8 } else { 6u8 };
            for _ in 0..steps_x.max(1) {
                self.fake_input(xproto::BUTTON_PRESS_EVENT, btn_h, 0, 0)?;
                self.fake_input(xproto::BUTTON_RELEASE_EVENT, btn_h, 0, 0)?;
            }
        }

        self.conn.flush()?;
        Ok(())
    }

    fn key_down(&self, keycode: u16) -> Result<()> {
        let x11_code = self.evdev_to_x11_keycode(keycode);
        self.fake_input(xproto::KEY_PRESS_EVENT, x11_code, 0, 0)?;
        self.conn.flush()?;
        Ok(())
    }

    fn key_up(&self, keycode: u16) -> Result<()> {
        let x11_code = self.evdev_to_x11_keycode(keycode);
        self.fake_input(xproto::KEY_RELEASE_EVENT, x11_code, 0, 0)?;
        self.conn.flush()?;
        Ok(())
    }

    fn type_text(&self, text: &str, delay_ms: Option<u64>) -> Result<()> {
        let delay = delay_ms.map(Duration::from_millis);

        for ch in text.chars() {
            if let Some((key, needs_shift)) = keycode_map::char_to_key(ch) {
                if needs_shift {
                    self.key_down(Key::KEY_LEFTSHIFT.code())?;
                }
                self.key_press(key.code())?;
                if needs_shift {
                    self.key_up(Key::KEY_LEFTSHIFT.code())?;
                }
                if let Some(d) = delay {
                    thread::sleep(d);
                } else {
                    thread::sleep(Duration::from_millis(5));
                }
            }
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "xtest"
    }
}
