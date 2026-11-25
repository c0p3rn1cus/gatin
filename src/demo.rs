use crate::input::InputEvent;
use crate::keyboard::{KeyId, MouseButton};

/// Generates a lively demo sequence (typing bursts, mouse movement + clicks, petting)
/// so users can preview the cat immediately or test without global input permissions.
pub struct DemoDriver {
    elapsed: f32,
    step_timer: f32,
    script_idx: usize,
    held_keys: Vec<(KeyId, f32)>,
    mouse_down_timer: f32,
    pub synthetic_pet: f32,
    pub synthetic_password: bool,
}

impl DemoDriver {
    pub fn new() -> Self {
        Self {
            elapsed: 0.0,
            step_timer: 0.4,
            script_idx: 0,
            held_keys: Vec::new(),
            mouse_down_timer: 0.0,
            synthetic_pet: 0.0,
            synthetic_password: false,
        }
    }

    pub fn update(&mut self, dt: f32) -> Vec<InputEvent> {
        self.elapsed += dt;
        let mut events = Vec::new();

        // Release previously held synthetic keys
        let mut still_held = Vec::new();
        for (key, rem) in self.held_keys.drain(..) {
            let next = rem - dt;
            if next <= 0.0 {
                events.push(InputEvent::KeyUp(key));
            } else {
                still_held.push((key, next));
            }
        }
        self.held_keys = still_held;

        if self.mouse_down_timer > 0.0 {
            self.mouse_down_timer -= dt;
            if self.mouse_down_timer <= 0.0 {
                events.push(InputEvent::MouseUp(MouseButton::Left));
            }
        }

        // Cycle through phases every 17 seconds:
        // 0..3s: mouse movement + clicks
        // 3..5.5s: happy petting!
        // 5.5..11.5s: rapid coding/typing burst
        // 11.5..14s: mixed mouse + WASD gaming
        // 14..17s: password privacy mode (paws covering eyes!)
        let phase = self.elapsed % 17.0;
        self.synthetic_pet = 0.0;
        self.synthetic_password = false;

        if phase < 3.0 {
            let t = phase;
            let nx = 0.5 + 0.35 * (t * 2.1).sin();
            let ny = 0.5 + 0.25 * (t * 1.5).cos();
            events.push(InputEvent::MouseMove {
                norm_x: nx,
                norm_y: ny,
                screen_px: None,
                look_dir: Some([(nx - 0.5) * 2.2, (ny - 0.5) * 2.2]),
            });
            self.step_timer -= dt;
            if self.step_timer <= 0.0 {
                self.step_timer = 0.9;
                events.push(InputEvent::MouseDown(MouseButton::Left));
                self.mouse_down_timer = 0.14;
            }
        } else if phase < 5.5 {
            self.synthetic_pet = 1.0;
            let t = phase - 3.0;
            let nx = 0.48 + 0.08 * (t * 6.5).sin();
            let ny = 0.35 + 0.03 * (t * 4.0).cos();
            events.push(InputEvent::MouseMove {
                norm_x: nx,
                norm_y: ny,
                screen_px: None,
                look_dir: Some([(nx - 0.5) * 2.2, (ny - 0.5) * 2.2]),
            });
        } else if phase < 11.5 {
            const TYPING_SEQ: &[KeyId] = &[
                KeyId::F,
                KeyId::N,
                KeyId::Space,
                KeyId::M,
                KeyId::A,
                KeyId::I,
                KeyId::N,
                KeyId::Space,
                KeyId::G,
                KeyId::A,
                KeyId::T,
                KeyId::I,
                KeyId::N,
                KeyId::Enter,
                KeyId::R,
                KeyId::U,
                KeyId::S,
                KeyId::T,
                KeyId::Space,
                KeyId::L,
                KeyId::O,
                KeyId::V,
                KeyId::E,
                KeyId::LeftShift,
                KeyId::S,
                KeyId::E,
                KeyId::A,
                KeyId::K,
                KeyId::J,
                KeyId::D,
                KeyId::F,
                KeyId::Space,
            ];
            self.step_timer -= dt;
            if self.step_timer <= 0.0 {
                let key = TYPING_SEQ[self.script_idx % TYPING_SEQ.len()];
                self.script_idx = self.script_idx.wrapping_add(1);
                events.push(InputEvent::KeyDown(key));
                self.held_keys.push((key, 0.13));
                self.step_timer = if key == KeyId::Space || key == KeyId::Enter {
                    0.18
                } else {
                    0.095
                };
            }
        } else if phase < 14.0 {
            // Mixed mouse + WASD + Shift (just like the second half of the video!)
            let t = phase - 11.5;
            events.push(InputEvent::MouseMove {
                norm_x: 0.35 + 0.25 * (t * 3.0).sin(),
                norm_y: 0.5 + 0.15 * (t * 2.2).cos(),
                screen_px: None,
                look_dir: None,
            });
            const GAME_SEQ: &[KeyId] = &[
                KeyId::W,
                KeyId::A,
                KeyId::S,
                KeyId::D,
                KeyId::LeftShift,
                KeyId::E,
                KeyId::Space,
                KeyId::A,
            ];
            self.step_timer -= dt;
            if self.step_timer <= 0.0 {
                let key = GAME_SEQ[self.script_idx % GAME_SEQ.len()];
                self.script_idx = self.script_idx.wrapping_add(1);
                events.push(InputEvent::KeyDown(key));
                self.held_keys.push((key, 0.22));
                if self.script_idx.is_multiple_of(3) {
                    events.push(InputEvent::MouseDown(MouseButton::Left));
                    self.mouse_down_timer = 0.14;
                }
                self.step_timer = 0.16;
            }
        } else {
            // Password privacy demo: cat covers eyes and hides typed keys!
            self.synthetic_password = true;
            self.step_timer -= dt;
            if self.step_timer <= 0.0 {
                events.push(InputEvent::KeyDown(KeyId::S));
                self.held_keys.push((KeyId::S, 0.1));
                self.step_timer = 0.22;
            }
        }

        events
    }
}
