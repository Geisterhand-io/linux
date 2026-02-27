use anyhow::Result;

use crate::models::api::MouseButton;

/// Trait for input injection backends.
pub trait InputBackend: Send + Sync {
    /// Move the mouse to absolute coordinates.
    fn mouse_move(&self, x: f64, y: f64) -> Result<()>;

    /// Click at the current mouse position.
    fn mouse_click(&self, button: MouseButton, count: u32) -> Result<()>;

    /// Scroll at the current mouse position.
    fn scroll(&self, delta_x: f64, delta_y: f64) -> Result<()>;

    /// Press and hold a key by evdev code.
    fn key_down(&self, keycode: u16) -> Result<()>;

    /// Release a key by evdev code.
    fn key_up(&self, keycode: u16) -> Result<()>;

    /// Type a single key press (down + up).
    fn key_press(&self, keycode: u16) -> Result<()> {
        self.key_down(keycode)?;
        self.key_up(keycode)
    }

    /// Type a string using the keyboard.
    fn type_text(&self, text: &str, delay_ms: Option<u64>) -> Result<()>;

    /// Get the backend name.
    fn name(&self) -> &str;
}
