use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyId {
    // Row 4 (visual bottom, physical number row)
    Escape,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    Digit0,
    Minus,
    Equal,
    Backspace,

    // Row 3 (physical QWERTY row)
    Tab,
    Q,
    W,
    E,
    R,
    T,
    Y,
    U,
    I,
    O,
    P,
    LeftBracket,
    RightBracket,
    Backslash,

    // Row 2 (physical ASDF home row)
    CapsLock,
    A,
    S,
    D,
    F,
    G,
    H,
    J,
    K,
    L,
    Semicolon,
    Quote,
    Enter,

    // Row 1 (physical ZXCV row)
    LeftShift,
    Z,
    X,
    C,
    V,
    B,
    N,
    M,
    Comma,
    Period,
    Slash,
    RightShift,

    // Row 0 (visual top closest to cat, physical bottom row)
    LeftCtrl,
    LeftMeta,
    LeftAlt,
    Space,
    RightAlt,
    RightMeta,
    Menu,
    RightCtrl,

    // Extra keys mapped visually onto the 60% deck with distinct popup labels
    Grave,
    Delete,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
}

impl KeyId {
    /// Short label rendered inside the floating white keycap popup.
    pub fn label(self) -> &'static str {
        match self {
            KeyId::Escape => "ESC",
            KeyId::Digit1 => "1",
            KeyId::Digit2 => "2",
            KeyId::Digit3 => "3",
            KeyId::Digit4 => "4",
            KeyId::Digit5 => "5",
            KeyId::Digit6 => "6",
            KeyId::Digit7 => "7",
            KeyId::Digit8 => "8",
            KeyId::Digit9 => "9",
            KeyId::Digit0 => "0",
            KeyId::Minus => "-",
            KeyId::Equal => "=",
            KeyId::Backspace => "⌫",
            KeyId::Tab => "⇥",
            KeyId::Q => "Q",
            KeyId::W => "W",
            KeyId::E => "E",
            KeyId::R => "R",
            KeyId::T => "T",
            KeyId::Y => "Y",
            KeyId::U => "U",
            KeyId::I => "I",
            KeyId::O => "O",
            KeyId::P => "P",
            KeyId::LeftBracket => "[",
            KeyId::RightBracket => "]",
            KeyId::Backslash => "\\",
            KeyId::CapsLock => "⇪",
            KeyId::A => "A",
            KeyId::S => "S",
            KeyId::D => "D",
            KeyId::F => "F",
            KeyId::G => "G",
            KeyId::H => "H",
            KeyId::J => "J",
            KeyId::K => "K",
            KeyId::L => "L",
            KeyId::Semicolon => ";",
            KeyId::Quote => "'",
            KeyId::Enter => "↵",
            KeyId::LeftShift | KeyId::RightShift => "⇧",
            KeyId::Z => "Z",
            KeyId::X => "X",
            KeyId::C => "C",
            KeyId::V => "V",
            KeyId::B => "B",
            KeyId::N => "N",
            KeyId::M => "M",
            KeyId::Comma => ",",
            KeyId::Period => ".",
            KeyId::Slash => "/",
            KeyId::LeftCtrl | KeyId::RightCtrl => "CTRL",
            KeyId::LeftMeta | KeyId::RightMeta => "⌘",
            KeyId::LeftAlt | KeyId::RightAlt => "ALT",
            KeyId::Space => " ",
            KeyId::Menu => "FN",
            KeyId::Grave => "`",
            KeyId::Delete => "DEL",
            KeyId::ArrowUp => "↑",
            KeyId::ArrowDown => "↓",
            KeyId::ArrowLeft => "←",
            KeyId::ArrowRight => "→",
            KeyId::F1 => "F1",
            KeyId::F2 => "F2",
            KeyId::F3 => "F3",
            KeyId::F4 => "F4",
            KeyId::F5 => "F5",
            KeyId::F6 => "F6",
            KeyId::F7 => "F7",
            KeyId::F8 => "F8",
            KeyId::F9 => "F9",
            KeyId::F10 => "F10",
            KeyId::F11 => "F11",
            KeyId::F12 => "F12",
        }
    }

    /// Maps extra keys (arrows, function keys, grave, delete) to a physical key slot on the 60% keyboard.
    pub fn physical_slot(self) -> KeyId {
        match self {
            KeyId::Grave => KeyId::Escape,
            KeyId::Delete => KeyId::Backspace,
            KeyId::ArrowLeft => KeyId::RightAlt,
            KeyId::ArrowDown => KeyId::RightMeta,
            KeyId::ArrowUp => KeyId::RightShift,
            KeyId::ArrowRight => KeyId::RightCtrl,
            KeyId::F1 => KeyId::Digit1,
            KeyId::F2 => KeyId::Digit2,
            KeyId::F3 => KeyId::Digit3,
            KeyId::F4 => KeyId::Digit4,
            KeyId::F5 => KeyId::Digit5,
            KeyId::F6 => KeyId::Digit6,
            KeyId::F7 => KeyId::Digit7,
            KeyId::F8 => KeyId::Digit8,
            KeyId::F9 => KeyId::Digit9,
            KeyId::F10 => KeyId::Digit0,
            KeyId::F11 => KeyId::Minus,
            KeyId::F12 => KeyId::Equal,
            other => other,
        }
    }

    pub fn is_wide_popup(self) -> bool {
        matches!(
            self,
            KeyId::Space | KeyId::LeftCtrl | KeyId::RightCtrl | KeyId::Escape | KeyId::Delete
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAccent {
    White,
    Orange,
}

#[derive(Debug, Clone, Copy)]
pub struct KeyCapSpec {
    pub key: KeyId,
    /// Visual row 0..=4 (0 = top closest to cat / spacebar row, 4 = bottom closest to viewer / number row)
    pub row: usize,
    /// Normalized horizontal start [0.0, 1.0] across the 15u keyboard width (left to right from viewer's perspective)
    pub u_start: f32,
    /// Normalized width [0.0, 1.0]
    pub u_width: f32,
    pub accent: KeyAccent,
}

impl KeyCapSpec {
    /// Center coordinate in keyboard UV space: u in [0, 1] (left->right), v in [0, 1] (top->bottom).
    pub fn center_uv(&self) -> (f32, f32) {
        let u = self.u_start + self.u_width * 0.5;
        let v = (self.row as f32 + 0.5) / 5.0;
        (u, v)
    }
}

pub struct KeyboardLayout {
    pub keys: Vec<KeyCapSpec>,
    index_by_key: HashMap<KeyId, usize>,
}

impl KeyboardLayout {
    /// Builds a 60% mechanical keyboard rotated 180° so it faces the cat (matching the video!).
    /// Viewer's left = Cat's right hand (Enter orange bar, Backspace, P/O/I, mouse to the left).
    /// Viewer's right = Cat's left hand (WASD, Shift/Caps, Esc orange corner key).
    pub fn new_facing_cat() -> Self {
        const TOTAL_U: f32 = 15.0;
        let rows: [Vec<(KeyId, f32, KeyAccent)>; 5] = [
            // Row 0 (top, closest to cat): physical bottom row reversed
            vec![
                (KeyId::RightCtrl, 1.25, KeyAccent::White),
                (KeyId::Menu, 1.25, KeyAccent::White),
                (KeyId::RightMeta, 1.25, KeyAccent::White),
                (KeyId::RightAlt, 1.25, KeyAccent::White),
                (KeyId::Space, 6.25, KeyAccent::White),
                (KeyId::LeftAlt, 1.25, KeyAccent::White),
                (KeyId::LeftMeta, 1.25, KeyAccent::White),
                (KeyId::LeftCtrl, 1.25, KeyAccent::White),
            ],
            // Row 1: physical ZXCV row reversed
            vec![
                (KeyId::RightShift, 2.75, KeyAccent::White),
                (KeyId::Slash, 1.0, KeyAccent::White),
                (KeyId::Period, 1.0, KeyAccent::White),
                (KeyId::Comma, 1.0, KeyAccent::White),
                (KeyId::M, 1.0, KeyAccent::White),
                (KeyId::N, 1.0, KeyAccent::White),
                (KeyId::B, 1.0, KeyAccent::White),
                (KeyId::V, 1.0, KeyAccent::White),
                (KeyId::C, 1.0, KeyAccent::White),
                (KeyId::X, 1.0, KeyAccent::White),
                (KeyId::Z, 1.0, KeyAccent::White),
                (KeyId::LeftShift, 2.25, KeyAccent::White),
            ],
            // Row 2: physical ASDF home row reversed — Enter on viewer's left is ORANGE
            vec![
                (KeyId::Enter, 2.25, KeyAccent::Orange),
                (KeyId::Quote, 1.0, KeyAccent::White),
                (KeyId::Semicolon, 1.0, KeyAccent::White),
                (KeyId::L, 1.0, KeyAccent::White),
                (KeyId::K, 1.0, KeyAccent::White),
                (KeyId::J, 1.0, KeyAccent::White),
                (KeyId::H, 1.0, KeyAccent::White),
                (KeyId::G, 1.0, KeyAccent::White),
                (KeyId::F, 1.0, KeyAccent::White),
                (KeyId::D, 1.0, KeyAccent::White),
                (KeyId::S, 1.0, KeyAccent::White),
                (KeyId::A, 1.0, KeyAccent::White),
                (KeyId::CapsLock, 1.75, KeyAccent::White),
            ],
            // Row 3: physical QWERTY row reversed
            vec![
                (KeyId::Backslash, 1.5, KeyAccent::White),
                (KeyId::RightBracket, 1.0, KeyAccent::White),
                (KeyId::LeftBracket, 1.0, KeyAccent::White),
                (KeyId::P, 1.0, KeyAccent::White),
                (KeyId::O, 1.0, KeyAccent::White),
                (KeyId::I, 1.0, KeyAccent::White),
                (KeyId::U, 1.0, KeyAccent::White),
                (KeyId::Y, 1.0, KeyAccent::White),
                (KeyId::T, 1.0, KeyAccent::White),
                (KeyId::R, 1.0, KeyAccent::White),
                (KeyId::E, 1.0, KeyAccent::White),
                (KeyId::W, 1.0, KeyAccent::White),
                (KeyId::Q, 1.0, KeyAccent::White),
                (KeyId::Tab, 1.5, KeyAccent::White),
            ],
            // Row 4 (bottom, closest to viewer): physical number row reversed — Esc on bottom-right is ORANGE
            vec![
                (KeyId::Backspace, 2.0, KeyAccent::White),
                (KeyId::Equal, 1.0, KeyAccent::White),
                (KeyId::Minus, 1.0, KeyAccent::White),
                (KeyId::Digit0, 1.0, KeyAccent::White),
                (KeyId::Digit9, 1.0, KeyAccent::White),
                (KeyId::Digit8, 1.0, KeyAccent::White),
                (KeyId::Digit7, 1.0, KeyAccent::White),
                (KeyId::Digit6, 1.0, KeyAccent::White),
                (KeyId::Digit5, 1.0, KeyAccent::White),
                (KeyId::Digit4, 1.0, KeyAccent::White),
                (KeyId::Digit3, 1.0, KeyAccent::White),
                (KeyId::Digit2, 1.0, KeyAccent::White),
                (KeyId::Digit1, 1.0, KeyAccent::White),
                (KeyId::Escape, 1.0, KeyAccent::Orange),
            ],
        ];

        let mut keys = Vec::with_capacity(64);
        let mut index_by_key = HashMap::with_capacity(64);

        for (row_idx, row_keys) in rows.into_iter().enumerate() {
            let mut cursor_u = 0.0_f32;
            for (key, width_u, accent) in row_keys {
                let spec = KeyCapSpec {
                    key,
                    row: row_idx,
                    u_start: cursor_u / TOTAL_U,
                    u_width: width_u / TOTAL_U,
                    accent,
                };
                let idx = keys.len();
                keys.push(spec);
                index_by_key.insert(key, idx);
                cursor_u += width_u;
            }
            debug_assert!(
                (cursor_u - TOTAL_U).abs() < 0.01,
                "Row {row_idx} width was {cursor_u}, expected {TOTAL_U}"
            );
        }

        Self { keys, index_by_key }
    }

    pub fn spec_for(&self, key: KeyId) -> Option<&KeyCapSpec> {
        let slot = key.physical_slot();
        self.index_by_key.get(&slot).map(|&idx| &self.keys[idx])
    }

    pub fn center_uv(&self, key: KeyId) -> (f32, f32) {
        self.spec_for(key)
            .map(KeyCapSpec::center_uv)
            .unwrap_or((0.5, 0.5))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_rows_sum_to_full_width() {
        let layout = KeyboardLayout::new_facing_cat();
        assert_eq!(layout.keys.len(), 61);
        // Escape should be bottom-right orange accent
        let esc = layout.spec_for(KeyId::Escape).unwrap();
        assert_eq!(esc.row, 4);
        assert_eq!(esc.accent, KeyAccent::Orange);
        assert!(esc.u_start > 0.9);
        // Enter should be on viewer's left, orange accent
        let enter = layout.spec_for(KeyId::Enter).unwrap();
        assert_eq!(enter.row, 2);
        assert_eq!(enter.accent, KeyAccent::Orange);
        assert!(enter.u_start < 0.01);
        // Space should be top row (closest to cat)
        let space = layout.spec_for(KeyId::Space).unwrap();
        assert_eq!(space.row, 0);
        // A/S/D should be on viewer's right (cat's left hand)
        let (a_u, _) = layout.center_uv(KeyId::A);
        let (p_u, _) = layout.center_uv(KeyId::P);
        assert!(a_u > 0.7);
        assert!(p_u < 0.35);
    }
}
