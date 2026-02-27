use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use anyhow::Result;
use evdev::Key;

use crate::input::backend::InputBackend;
use crate::input::keycode_map;
use crate::models::api::MouseButton;

/// Input backend using Linux uinput virtual device.
/// Works on both X11 and Wayland but requires /dev/uinput access.
pub struct UinputBackend {
    device: Mutex<evdev::uinput::VirtualDevice>,
}

impl UinputBackend {
    pub fn new() -> Result<Self> {
        let mut keys = evdev::AttributeSet::<Key>::new();
        // Add all possible keys we might use
        for code in 0..256u16 {
            keys.insert(Key::new(code));
        }

        let device = evdev::uinput::VirtualDeviceBuilder::new()?
            .name("geisterhand-virtual-input")
            .with_keys(&keys)?
            .with_relative_axes(&{
                let mut axes = evdev::AttributeSet::<evdev::RelativeAxisType>::new();
                axes.insert(evdev::RelativeAxisType::REL_X);
                axes.insert(evdev::RelativeAxisType::REL_Y);
                axes.insert(evdev::RelativeAxisType::REL_WHEEL);
                axes.insert(evdev::RelativeAxisType::REL_HWHEEL);
                axes
            })?
            .with_absolute_axis(&evdev::UinputAbsSetup::new(
                evdev::AbsoluteAxisType::ABS_X,
                evdev::AbsInfo::new(0, 0, 3840, 0, 0, 1),
            ))?
            .with_absolute_axis(&evdev::UinputAbsSetup::new(
                evdev::AbsoluteAxisType::ABS_Y,
                evdev::AbsInfo::new(0, 0, 2160, 0, 0, 1),
            ))?
            .build()?;

        // Give the virtual device time to register
        thread::sleep(Duration::from_millis(100));

        Ok(Self {
            device: Mutex::new(device),
        })
    }

    fn emit_events(&self, events: &[evdev::InputEvent]) -> Result<()> {
        let mut dev = self.device.lock().map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;
        dev.emit(events)?;
        Ok(())
    }

    fn syn(&self) -> Result<()> {
        self.emit_events(&[evdev::InputEvent::new(
            evdev::EventType::SYNCHRONIZATION,
            0, // SYN_REPORT
            0,
        )])
    }
}

impl InputBackend for UinputBackend {
    fn mouse_move(&self, x: f64, y: f64) -> Result<()> {
        self.emit_events(&[
            evdev::InputEvent::new(
                evdev::EventType::ABSOLUTE,
                evdev::AbsoluteAxisType::ABS_X.0,
                x as i32,
            ),
            evdev::InputEvent::new(
                evdev::EventType::ABSOLUTE,
                evdev::AbsoluteAxisType::ABS_Y.0,
                y as i32,
            ),
        ])?;
        self.syn()
    }

    fn mouse_click(&self, button: MouseButton, count: u32) -> Result<()> {
        let btn_code = match button {
            MouseButton::Left => Key::BTN_LEFT,
            MouseButton::Right => Key::BTN_RIGHT,
            MouseButton::Center => Key::BTN_MIDDLE,
        };

        for _ in 0..count {
            self.emit_events(&[evdev::InputEvent::new(
                evdev::EventType::KEY,
                btn_code.code(),
                1, // press
            )])?;
            self.syn()?;
            thread::sleep(Duration::from_millis(10));
            self.emit_events(&[evdev::InputEvent::new(
                evdev::EventType::KEY,
                btn_code.code(),
                0, // release
            )])?;
            self.syn()?;
            if count > 1 {
                thread::sleep(Duration::from_millis(50));
            }
        }
        Ok(())
    }

    fn scroll(&self, delta_x: f64, delta_y: f64) -> Result<()> {
        let mut events = Vec::new();
        if delta_y.abs() > 0.0 {
            events.push(evdev::InputEvent::new(
                evdev::EventType::RELATIVE,
                evdev::RelativeAxisType::REL_WHEEL.0,
                -(delta_y as i32), // negative = scroll down in evdev
            ));
        }
        if delta_x.abs() > 0.0 {
            events.push(evdev::InputEvent::new(
                evdev::EventType::RELATIVE,
                evdev::RelativeAxisType::REL_HWHEEL.0,
                delta_x as i32,
            ));
        }
        if !events.is_empty() {
            self.emit_events(&events)?;
            self.syn()?;
        }
        Ok(())
    }

    fn key_down(&self, keycode: u16) -> Result<()> {
        self.emit_events(&[evdev::InputEvent::new(
            evdev::EventType::KEY,
            keycode,
            1, // press
        )])?;
        self.syn()
    }

    fn key_up(&self, keycode: u16) -> Result<()> {
        self.emit_events(&[evdev::InputEvent::new(
            evdev::EventType::KEY,
            keycode,
            0, // release
        )])?;
        self.syn()
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
            // Skip characters we can't type (Unicode, etc.)
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "uinput"
    }
}
