use crate::keyboard::{KeyId, MouseButton};
use std::sync::mpsc;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[derive(Debug, Clone, Copy)]
pub enum InputEvent {
    KeyDown(KeyId),
    KeyUp(KeyId),
    /// Global cursor position: normalized [0..1] screen coords, optional raw screen pixels,
    /// and optional precomputed look direction relative to the cat window [-1..1, -1..1].
    MouseMove {
        norm_x: f32,
        norm_y: f32,
        screen_px: Option<[f32; 2]>,
        look_dir: Option<[f32; 2]>,
    },
    /// Relative mouse delta in raw movement units (used by Linux evdev).
    MouseDelta { dx: f32, dy: f32 },
    MouseDown(MouseButton),
    MouseUp(MouseButton),
}

#[derive(Debug, Clone)]
pub struct InputStatus {
    pub backend: String,
    pub global_active: bool,
    pub hint: Option<String>,
}

pub struct GlobalInput {
    pub rx: mpsc::Receiver<InputEvent>,
    pub status: InputStatus,
}

impl GlobalInput {
    pub fn start() -> Self {
        let (tx, rx) = mpsc::channel();

        #[cfg(target_os = "linux")]
        let status = linux::start(tx);

        #[cfg(target_os = "windows")]
        let status = windows::start(tx);

        #[cfg(target_os = "macos")]
        let status = macos::start(tx);

        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        let status = {
            let _ = tx;
            InputStatus {
                backend: "Local window only".to_string(),
                global_active: false,
                hint: Some("Unsupported OS for global input hooks.".to_string()),
            }
        };

        Self { rx, status }
    }
}

/// Convert an `egui::Key` (used as local fallback when the cat window has focus) to `KeyId`.
pub fn egui_key_to_key_id(key: egui::Key) -> Option<KeyId> {
    use egui::Key;
    Some(match key {
        Key::Escape => KeyId::Escape,
        Key::Num1 => KeyId::Digit1,
        Key::Num2 => KeyId::Digit2,
        Key::Num3 => KeyId::Digit3,
        Key::Num4 => KeyId::Digit4,
        Key::Num5 => KeyId::Digit5,
        Key::Num6 => KeyId::Digit6,
        Key::Num7 => KeyId::Digit7,
        Key::Num8 => KeyId::Digit8,
        Key::Num9 => KeyId::Digit9,
        Key::Num0 => KeyId::Digit0,
        Key::Minus => KeyId::Minus,
        Key::Equals => KeyId::Equal,
        Key::Backspace => KeyId::Backspace,
        Key::Tab => KeyId::Tab,
        Key::Q => KeyId::Q,
        Key::W => KeyId::W,
        Key::E => KeyId::E,
        Key::R => KeyId::R,
        Key::T => KeyId::T,
        Key::Y => KeyId::Y,
        Key::U => KeyId::U,
        Key::I => KeyId::I,
        Key::O => KeyId::O,
        Key::P => KeyId::P,
        Key::OpenBracket => KeyId::LeftBracket,
        Key::CloseBracket => KeyId::RightBracket,
        Key::Backslash => KeyId::Backslash,
        Key::A => KeyId::A,
        Key::S => KeyId::S,
        Key::D => KeyId::D,
        Key::F => KeyId::F,
        Key::G => KeyId::G,
        Key::H => KeyId::H,
        Key::J => KeyId::J,
        Key::K => KeyId::K,
        Key::L => KeyId::L,
        Key::Semicolon => KeyId::Semicolon,
        Key::Quote => KeyId::Quote,
        Key::Enter => KeyId::Enter,
        Key::Z => KeyId::Z,
        Key::X => KeyId::X,
        Key::C => KeyId::C,
        Key::V => KeyId::V,
        Key::B => KeyId::B,
        Key::N => KeyId::N,
        Key::M => KeyId::M,
        Key::Comma => KeyId::Comma,
        Key::Period => KeyId::Period,
        Key::Slash => KeyId::Slash,
        Key::Space => KeyId::Space,
        Key::Backtick => KeyId::Grave,
        Key::Delete => KeyId::Delete,
        Key::ArrowUp => KeyId::ArrowUp,
        Key::ArrowDown => KeyId::ArrowDown,
        Key::ArrowLeft => KeyId::ArrowLeft,
        Key::ArrowRight => KeyId::ArrowRight,
        Key::F1 => KeyId::F1,
        Key::F2 => KeyId::F2,
        Key::F3 => KeyId::F3,
        Key::F4 => KeyId::F4,
        Key::F5 => KeyId::F5,
        Key::F6 => KeyId::F6,
        Key::F7 => KeyId::F7,
        Key::F8 => KeyId::F8,
        Key::F9 => KeyId::F9,
        Key::F10 => KeyId::F10,
        Key::F11 => KeyId::F11,
        Key::F12 => KeyId::F12,
        _ => return None,
    })
}
