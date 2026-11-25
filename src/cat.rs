use crate::input::InputEvent;
use crate::keyboard::{KeyId, KeyboardLayout, MouseButton};
use std::collections::{HashMap, HashSet};

/// Logical virtual canvas dimensions (uniformly scaled to window size).
pub const CANVAS_W: f32 = 360.0;
pub const CANVAS_H: f32 = 320.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PawMode {
    Resting,
    OnMouse,
    Curled,
    PressingKey,
}

#[derive(Debug, Clone)]
pub struct PawState {
    pub mode: PawMode,
    /// Current paw tip position in virtual canvas coordinates.
    pub pos: [f32; 2],
    /// Target paw tip position in virtual canvas coordinates.
    pub target: [f32; 2],
    /// Vertical lift height (for bouncy key strike arc).
    pub lift: f32,
    /// Seconds remaining before returning from PressingKey to Curled/Resting.
    pub strike_timer: f32,
    /// Seconds since this paw was last active on keyboard.
    pub idle_timer: f32,
    /// Smooth blend [0..1] for how curled the paw looks on the chest.
    pub curl_blend: f32,
    /// Smooth blend [0..1] for resting straight vertical leg styling.
    pub rest_blend: f32,
    /// Currently pressed key (if held).
    pub active_key: Option<KeyId>,
}

impl PawState {
    fn new(rest_pos: [f32; 2]) -> Self {
        Self {
            mode: PawMode::Resting,
            pos: rest_pos,
            target: rest_pos,
            lift: 0.0,
            strike_timer: 0.0,
            idle_timer: 10.0,
            curl_blend: 0.0,
            rest_blend: 1.0,
            active_key: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct KeyPopup {
    pub key: KeyId,
    pub label: &'static str,
    pub is_wide: bool,
    /// Base horizontal X position in virtual canvas coordinates.
    pub base_x: f32,
    /// Small vertical stagger so chords stay readable side-by-side.
    pub y_stagger: f32,
    /// Total lifetime elapsed since press.
    pub age: f32,
    /// Whether the key is still physically held down.
    pub held: bool,
    /// Time elapsed since release (controls upward drift & fade-out).
    pub release_age: f32,
}

pub struct CatState {
    pub layout: KeyboardLayout,
    pub time: f32,

    // Keyboard & mouse physical state
    pub pressed_keys: HashSet<KeyId>,
    pub key_heat: HashMap<KeyId, f32>,
    pub mouse_buttons: HashSet<MouseButton>,
    pub mouse_click_glow: f32,

    /// Normalized virtual cursor [0..1, 0..1] across screen.
    pub cursor_norm: [f32; 2],
    /// Direct eye look direction [-1..1, -1..1] relative to the cat's head.
    pub look_target: [f32; 2],
    /// Timer indicating recent absolute screen cursor updates (prevents evdev delta from fighting Hyprland/OS coords).
    abs_cursor_timer: f32,
    /// Target mousepad position in virtual canvas pixels around base mouse position (high-gain!).
    mouse_pad_target: [f32; 2],
    /// Smooth mouse device offset in virtual canvas pixels around base mouse position.
    pub mouse_offset: [f32; 2],
    /// Slight mouse body tilt angle when sliding rapidly.
    pub mouse_tilt: f32,
    /// Time since last mouse movement or click.
    pub mouse_idle_timer: f32,

    // Cat body & facial expressions
    pub gaze: [f32; 2],
    pub lean_x: f32,
    pub head_tilt: f32,
    pub typing_bob: f32,
    pub typing_energy: f32,

    // Blinking
    pub blink: f32,
    next_blink_in: f32,
    blink_phase: f32,

    // Petting (requires real rubbing motion on the head, not just hovering!)
    pub pet_level: f32,
    pet_rub_energy: f32,
    pet_hold_timer: f32,
    pet_lean: f32,
    last_pet_dir: f32,

    // Privacy / password-field mode (cat covers its eyes with both paws & hides all keycaps!)
    pub password_mode: bool,
    pub cover_blend: f32,
    pub shy_wiggle: f32,

    // Tail
    pub tail_phase: f32,

    // Paws: left_paw = viewer's left (cat's RIGHT hand -> mouse + left half of flipped kbd)
    //       right_paw = viewer's right (cat's LEFT hand -> right half of flipped kbd: WASD/Shift/Esc)
    pub left_paw: PawState,
    pub right_paw: PawState,

    // Floating keycap badges
    pub popups: Vec<KeyPopup>,
    popup_stagger_counter: usize,
}

impl CatState {
    pub fn new() -> Self {
        let layout = KeyboardLayout::new_facing_cat();
        Self {
            layout,
            time: 0.0,
            pressed_keys: HashSet::new(),
            key_heat: HashMap::new(),
            mouse_buttons: HashSet::new(),
            mouse_click_glow: 0.0,
            cursor_norm: [0.5, 0.45],
            look_target: [0.0, 0.0],
            abs_cursor_timer: 0.0,
            mouse_pad_target: [0.0, 0.0],
            mouse_offset: [0.0, 0.0],
            mouse_tilt: 0.0,
            mouse_idle_timer: 10.0,
            gaze: [0.0, 0.0],
            lean_x: 0.0,
            head_tilt: 0.0,
            typing_bob: 0.0,
            typing_energy: 0.0,
            blink: 0.0,
            next_blink_in: 3.2,
            blink_phase: 0.0,
            pet_level: 0.0,
            pet_rub_energy: 0.0,
            pet_hold_timer: 0.0,
            pet_lean: 0.0,
            last_pet_dir: 0.0,
            password_mode: false,
            cover_blend: 0.0,
            shy_wiggle: 0.0,
            tail_phase: 0.0,
            left_paw: PawState::new(Self::left_rest_pos()),
            right_paw: PawState::new(Self::right_rest_pos()),
            popups: Vec::new(),
            popup_stagger_counter: 0,
        }
    }

    pub fn left_rest_pos() -> [f32; 2] {
        [163.0, 232.0]
    }

    pub fn right_rest_pos() -> [f32; 2] {
        [201.0, 232.0]
    }

    pub fn left_curled_pos() -> [f32; 2] {
        [154.0, 198.0]
    }

    pub fn right_curled_pos() -> [f32; 2] {
        [204.0, 200.0]
    }

    pub fn mouse_base_pos() -> [f32; 2] {
        [49.0, 256.0]
    }

    pub fn mouse_current_pos(&self) -> [f32; 2] {
        let base = Self::mouse_base_pos();
        [base[0] + self.mouse_offset[0], base[1] + self.mouse_offset[1]]
    }

    /// Maps normalized keyboard (u, v) in [0..1]x[0..1] into perspective virtual canvas coords.
    pub fn keyboard_uv_to_canvas(u: f32, v: f32) -> [f32; 2] {
        // Trapezoid matching the 3D perspective keyboard in the video:
        // top edge (closest to cat): x in [97, 293], y = 236
        // bottom edge (closest to viewer): x in [85, 305], y = 290
        let top_l = 97.0_f32;
        let top_r = 293.0_f32;
        let bot_l = 85.0_f32;
        let bot_r = 305.0_f32;
        let top_y = 236.0_f32;
        let bot_y = 290.0_f32;

        let row_l = top_l + (bot_l - top_l) * v;
        let row_r = top_r + (bot_r - top_r) * v;
        let x = row_l + (row_r - row_l) * u;
        let y = top_y + (bot_y - top_y) * v;
        [x, y]
    }

    /// Toggle password-privacy mode: immediately wipes any visible keycaps/key highlights
    /// and makes the cat cover its eyes with both paws!
    pub fn set_password_mode(&mut self, active: bool) {
        if active && !self.password_mode {
            self.popups.clear();
            self.key_heat.clear();
            self.pressed_keys.clear();
            self.left_paw.active_key = None;
            self.right_paw.active_key = None;
        }
        self.password_mode = active;
    }

    /// Directly aim the cat's eyes toward a normalized direction [-1..1, -1..1] (e.g. local hover).
    pub fn set_look_direction(&mut self, dir: [f32; 2]) {
        self.look_target = [dir[0].clamp(-1.0, 1.0), dir[1].clamp(-1.0, 1.0)];
        self.abs_cursor_timer = 0.25;
    }

    /// Register petting motion when rubbing back-and-forth on the cat's head.
    /// Requires sustained rubbing / direction reversals so casually crossing the cat won't close its eyes!
    pub fn register_pet(&mut self, dx: f32, dy: f32, head_rel_x: f32) {
        let dist = (dx * dx + dy * dy).sqrt();
        if dist < 0.8 {
            return;
        }
        // Extra bonus when reversing direction (classic back-and-forth petting stroke!)
        let reversal_bonus = if dx.abs() > 1.2 && (dx * self.last_pet_dir) < 0.0 {
            0.38
        } else {
            0.0
        };
        if dx.abs() > 1.2 {
            self.last_pet_dir = dx.signum();
        }
        self.pet_rub_energy = (self.pet_rub_energy + dist * 0.018 + reversal_bonus).min(1.6);
        if self.pet_rub_energy >= 0.72 {
            self.pet_hold_timer = (self.pet_hold_timer + 0.35).min(0.85);
            self.pet_lean = (head_rel_x * 0.35).clamp(-1.0, 1.0);
        }
    }

    /// Immediately engage happy petting (used by demo mode).
    pub fn register_demo_pet(&mut self, sway: f32) {
        self.pet_rub_energy = 1.2;
        self.pet_hold_timer = 0.6;
        self.pet_lean = sway * 0.5;
    }

    fn clamp_mouse_pad(x: f32, y: f32) -> [f32; 2] {
        // Generous elliptical mousepad area on viewer's left: rx=24px, ry=24px
        // Stays cleanly inside canvas [25..73] without colliding with the keyboard (x >= 80)
        const RX: f32 = 23.0;
        const RY: f32 = 24.0;
        let nx = x / RX;
        let ny = y / RY;
        let len = (nx * nx + ny * ny).sqrt();
        if len > 1.0 {
            [(nx / len) * RX, (ny / len) * RY]
        } else {
            [x, y]
        }
    }

    pub fn handle_event(&mut self, ev: InputEvent) {
        match ev {
            InputEvent::KeyDown(key) => {
                // In password mode: NEVER expose characters, NEVER light up specific keys!
                if self.password_mode {
                    self.shy_wiggle = (self.shy_wiggle + 0.6).min(1.2);
                    return;
                }

                let first_press = self.pressed_keys.insert(key);
                let slot = key.physical_slot();
                self.key_heat.insert(slot, 1.0);

                if first_press {
                    self.typing_energy = (self.typing_energy + 0.55).min(1.6);
                    // Rapid typing wakes the cat up from petting smoothly
                    self.pet_hold_timer = (self.pet_hold_timer - 0.35).max(0.0);

                    let (u, v) = self.layout.center_uv(key);
                    let key_pos = Self::keyboard_uv_to_canvas(u, v);

                    // Decide which paw hits this key!
                    // Viewer's left = left_paw (also owns mouse), viewer's right = right_paw.
                    // Just like the video (frame_04/05): if left_paw is actively on the mouse
                    // and the key isn't on the extreme far-left, right_paw reaches over to press it!
                    let mouse_busy = self.mouse_idle_timer < 1.35 || !self.mouse_buttons.is_empty();
                    let use_left = if mouse_busy {
                        u < 0.22
                    } else if u < 0.44 {
                        true
                    } else if u > 0.54 {
                        false
                    } else {
                        // Middle zone (e.g. Spacebar / G / H / B / N): alternate paws!
                        self.left_paw.idle_timer >= self.right_paw.idle_timer
                    };

                    if use_left {
                        self.left_paw.mode = PawMode::PressingKey;
                        self.left_paw.target = key_pos;
                        self.left_paw.strike_timer = 0.16;
                        self.left_paw.idle_timer = 0.0;
                        self.left_paw.lift = 14.0;
                        self.left_paw.active_key = Some(key);
                    } else {
                        self.right_paw.mode = PawMode::PressingKey;
                        self.right_paw.target = key_pos;
                        self.right_paw.strike_timer = 0.16;
                        self.right_paw.idle_timer = 0.0;
                        self.right_paw.lift = 14.0;
                        self.right_paw.active_key = Some(key);
                    }

                    self.spawn_popup(key, key_pos[0]);
                }
            }
            InputEvent::KeyUp(key) => {
                if self.password_mode {
                    return;
                }
                self.pressed_keys.remove(&key);
                if self.left_paw.active_key == Some(key) {
                    self.left_paw.active_key = None;
                }
                if self.right_paw.active_key == Some(key) {
                    self.right_paw.active_key = None;
                }
                for popup in &mut self.popups {
                    if popup.key == key && popup.held {
                        popup.held = false;
                        popup.release_age = 0.0;
                    }
                }
            }
            InputEvent::MouseMove {
                norm_x,
                norm_y,
                screen_px: _,
                look_dir,
            } => {
                let nx = norm_x.clamp(0.0, 1.0);
                let ny = norm_y.clamp(0.0, 1.0);
                let dx = nx - self.cursor_norm[0];
                let dy = ny - self.cursor_norm[1];
                self.cursor_norm = [nx, ny];
                self.abs_cursor_timer = 0.35;

                // Update eye look direction immediately
                if let Some(dir) = look_dir {
                    self.look_target = [dir[0].clamp(-1.0, 1.0), dir[1].clamp(-1.0, 1.0)];
                } else {
                    self.look_target = [
                        ((nx - 0.5) * 2.2).clamp(-1.0, 1.0),
                        ((ny - 0.45) * 2.2).clamp(-1.0, 1.0),
                    ];
                }

                if dx.abs() > 0.0003 || dy.abs() > 0.0003 {
                    self.mouse_idle_timer = 0.0;
                    // High-gain mousepad movement: even small desktop gestures visibly move the cat's mouse!
                    let boosted_x = self.mouse_pad_target[0] + dx * 165.0;
                    let boosted_y = self.mouse_pad_target[1] + dy * 135.0;
                    // Also gently anchor to absolute screen quadrant so full sweeps track predictably
                    let abs_x = (nx - 0.5) * 46.0;
                    let abs_y = (ny - 0.5) * 44.0;
                    let mixed_x = boosted_x * 0.82 + abs_x * 0.18;
                    let mixed_y = boosted_y * 0.82 + abs_y * 0.18;
                    self.mouse_pad_target = Self::clamp_mouse_pad(mixed_x, mixed_y);
                }
            }
            InputEvent::MouseDelta { dx, dy } => {
                if dx.abs() > 0.1 || dy.abs() > 0.1 {
                    self.mouse_idle_timer = 0.0;
                    // Direct, lively 1:4 mousepad response to raw mouse movement!
                    let next_x = self.mouse_pad_target[0] + dx * 0.24;
                    let next_y = self.mouse_pad_target[1] + dy * 0.24;
                    self.mouse_pad_target = Self::clamp_mouse_pad(next_x, next_y);

                    // Only update cursor_norm/look_target from relative deltas when no absolute tracker is active
                    if self.abs_cursor_timer <= 0.0 {
                        self.cursor_norm[0] = (self.cursor_norm[0] + dx * 0.0022).clamp(0.0, 1.0);
                        self.cursor_norm[1] = (self.cursor_norm[1] + dy * 0.0028).clamp(0.0, 1.0);
                        self.look_target = [
                            ((self.cursor_norm[0] - 0.5) * 2.2).clamp(-1.0, 1.0),
                            ((self.cursor_norm[1] - 0.45) * 2.2).clamp(-1.0, 1.0),
                        ];
                    }
                }
            }
            InputEvent::MouseDown(btn) => {
                self.mouse_buttons.insert(btn);
                self.mouse_click_glow = 1.0;
                self.mouse_idle_timer = 0.0;
            }
            InputEvent::MouseUp(btn) => {
                self.mouse_buttons.remove(&btn);
            }
        }
    }

    fn spawn_popup(&mut self, key: KeyId, key_x: f32) {
        // Refresh existing held popup for same key if present
        if let Some(existing) = self.popups.iter_mut().find(|p| p.key == key && p.held) {
            existing.age = 0.0;
            return;
        }

        // Compute small stagger if another very fresh popup is horizontally close
        let mut x = key_x.clamp(108.0, 286.0);
        let mut y_stagger = 0.0_f32;
        for other in self.popups.iter().rev().take(4) {
            if (other.base_x - x).abs() < 24.0 && other.release_age < 0.35 {
                let dir = if self.popup_stagger_counter.is_multiple_of(2) {
                    1.0
                } else {
                    -1.0
                };
                x = (x + dir * 13.0).clamp(102.0, 294.0);
                y_stagger = ((self.popup_stagger_counter % 3) as f32) * 11.0;
            }
        }
        self.popup_stagger_counter = self.popup_stagger_counter.wrapping_add(1);

        self.popups.push(KeyPopup {
            key,
            label: key.label(),
            is_wide: key.is_wide_popup(),
            base_x: x,
            y_stagger,
            age: 0.0,
            held: true,
            release_age: 0.0,
        });

        if self.popups.len() > 8 {
            self.popups.remove(0);
        }
    }

    pub fn update(&mut self, dt: f32) {
        let dt = dt.clamp(0.001, 0.05);
        self.time += dt;
        self.mouse_idle_timer += dt;
        self.abs_cursor_timer = (self.abs_cursor_timer - dt).max(0.0);
        self.left_paw.idle_timer += dt;
        self.right_paw.idle_timer += dt;

        // Smooth decay of typing energy
        self.typing_energy = (self.typing_energy - dt * 1.45).max(0.0);

        // Smooth decay of key glow heat
        let pressed_slots: HashSet<KeyId> =
            self.pressed_keys.iter().map(|k| k.physical_slot()).collect();
        self.key_heat.retain(|slot, heat| {
            if pressed_slots.contains(slot) {
                *heat = 1.0;
                true
            } else {
                *heat = (*heat - dt * 4.5).max(0.0);
                *heat > 0.01
            }
        });

        // Mouse click glow
        if !self.mouse_buttons.is_empty() {
            self.mouse_click_glow = 1.0;
        } else {
            self.mouse_click_glow = (self.mouse_click_glow - dt * 5.0).max(0.0);
        }

        // Mouse sliding offset around base position: super snappy & expressive!
        if self.mouse_idle_timer > 1.8 {
            // Gently drift back toward center only after resting
            let recenter = 1.0 - (-dt * 1.8).exp();
            self.mouse_pad_target[0] *= 1.0 - recenter;
            self.mouse_pad_target[1] *= 1.0 - recenter;
        }
        let mouse_lerp = 1.0 - (-dt * 28.0).exp();
        let prev_mx = self.mouse_offset[0];
        self.mouse_offset[0] += (self.mouse_pad_target[0] - self.mouse_offset[0]) * mouse_lerp;
        self.mouse_offset[1] += (self.mouse_pad_target[1] - self.mouse_offset[1]) * mouse_lerp;

        // Subtle dynamic tilt of the white mouse as the cat slides it left/right
        let vx = (self.mouse_offset[0] - prev_mx) / dt;
        let target_mouse_tilt = (vx * 0.0018 + self.mouse_offset[0] * 0.006).clamp(-0.22, 0.22);
        self.mouse_tilt += (target_mouse_tilt - self.mouse_tilt) * (1.0 - (-dt * 16.0).exp());

        // Petting rub decay & level smoothing
        self.pet_rub_energy = (self.pet_rub_energy - dt * 1.35).max(0.0);
        if self.pet_hold_timer > 0.0 {
            self.pet_hold_timer = (self.pet_hold_timer - dt).max(0.0);
            self.pet_level = (self.pet_level + dt * 5.0).min(1.0);
        } else {
            self.pet_level = (self.pet_level - dt * 3.8).max(0.0);
            self.pet_lean *= 1.0 - (dt * 4.5).min(1.0);
        }

        // Natural blinking
        self.next_blink_in -= dt;
        if self.next_blink_in <= 0.0 && self.blink_phase <= 0.0 {
            self.blink_phase = 0.15;
            // Pseudo-random interval between 2.8s and 6.2s
            let pseudo = ((self.time * 13.37).sin() * 0.5 + 0.5).clamp(0.0, 1.0);
            self.next_blink_in = 2.8 + pseudo * 3.4;
        }
        if self.blink_phase > 0.0 {
            self.blink_phase = (self.blink_phase - dt).max(0.0);
            let t = 1.0 - (self.blink_phase / 0.15);
            self.blink = (t * std::f32::consts::PI).sin().clamp(0.0, 1.0);
        } else {
            self.blink = 0.0;
        }

        // Tail animation (gentle happy wag faster when petted or typing)
        let tail_speed = 2.1 + self.pet_level * 2.2 + self.typing_energy.min(1.0) * 1.4;
        self.tail_phase += dt * tail_speed;

        // Privacy cover-eyes blend (smoothly raises both paws over the eyes!)
        let target_cover = if self.password_mode { 1.0 } else { 0.0 };
        let cover_lerp = 1.0 - (-dt * 14.0).exp();
        self.cover_blend += (target_cover - self.cover_blend) * cover_lerp;
        if self.cover_blend < 0.005 && !self.password_mode {
            self.cover_blend = 0.0;
        }
        self.shy_wiggle = (self.shy_wiggle - dt * 3.5).max(0.0);

        // Update left paw (mouse + left keyboard half)
        let mouse_pos = self.mouse_current_pos();
        let mouse_active =
            !self.password_mode && (self.mouse_idle_timer < 1.6 || !self.mouse_buttons.is_empty());
        Self::update_paw(
            &mut self.left_paw,
            dt,
            Self::left_rest_pos(),
            Self::left_curled_pos(),
            if mouse_active { Some(mouse_pos) } else { None },
            self.typing_energy,
        );

        // Update right paw (right keyboard half)
        Self::update_paw(
            &mut self.right_paw,
            dt,
            Self::right_rest_pos(),
            Self::right_curled_pos(),
            None,
            self.typing_energy,
        );

        // Body & head lean: leans left toward mouse (like frame_02!), leans slightly with typing or petting
        let target_lean_x = if self.password_mode {
            (self.time * 18.0).sin() * 2.5 * self.shy_wiggle
        } else if self.left_paw.mode == PawMode::OnMouse {
            -14.0 + self.mouse_offset[0] * 0.45 + self.pet_lean * 6.0
        } else if self.pet_level > 0.1 {
            self.pet_lean * 8.0
        } else if self.typing_energy > 0.15 {
            if self.right_paw.mode == PawMode::PressingKey
                && self.left_paw.mode != PawMode::PressingKey
            {
                4.0
            } else if self.left_paw.mode == PawMode::PressingKey
                && self.right_paw.mode != PawMode::PressingKey
            {
                -4.0
            } else {
                (self.time * 12.0).sin() * 2.0 * self.typing_energy.min(1.0)
            }
        } else {
            0.0
        };

        let lean_lerp = 1.0 - (-dt * 8.0).exp();
        self.lean_x += (target_lean_x - self.lean_x) * lean_lerp;

        let target_tilt = if self.password_mode {
            -0.06 + (self.time * 16.0).sin() * 0.05 * self.shy_wiggle
        } else {
            (self.lean_x * 0.0055) + (self.pet_lean * 0.11 * self.pet_level)
        };
        self.head_tilt += (target_tilt - self.head_tilt) * lean_lerp;

        // Subtle head bob during energetic typing
        let target_bob = if self.typing_energy > 0.1 {
            (self.time * 22.0).sin().abs() * 2.8 * self.typing_energy.min(1.0)
        } else {
            (self.time * 1.8).sin() * 0.9
        };
        self.typing_bob += (target_bob - self.typing_bob) * (1.0 - (-dt * 14.0).exp());

        // Gaze tracking: follow the mouse cursor tightly! Only glance down at keys during heavy typing when mouse is idle.
        let (target_gx, target_gy) = if self.typing_energy > 0.55 && self.mouse_idle_timer > 0.65 {
            let active_x = if self.right_paw.mode == PawMode::PressingKey {
                (self.right_paw.target[0] - 182.0) / 110.0
            } else {
                (self.left_paw.target[0] - 182.0) / 110.0
            };
            (active_x.clamp(-0.7, 0.7), 0.55)
        } else {
            (self.look_target[0], self.look_target[1])
        };
        let gaze_lerp = 1.0 - (-dt * 20.0).exp();
        self.gaze[0] += (target_gx - self.gaze[0]) * gaze_lerp;
        self.gaze[1] += (target_gy - self.gaze[1]) * gaze_lerp;

        // Update floating keycap popups
        for popup in &mut self.popups {
            popup.age += dt;
            if !popup.held {
                popup.release_age += dt;
            }
            // Safety auto-release if a key-up was missed (e.g. window focus change)
            if popup.held && popup.age > 3.5 && !self.pressed_keys.contains(&popup.key) {
                popup.held = false;
            }
        }
        self.popups.retain(|p| p.held || p.release_age < 0.85);
    }

    fn update_paw(
        paw: &mut PawState,
        dt: f32,
        rest_pos: [f32; 2],
        curled_pos: [f32; 2],
        mouse_target: Option<[f32; 2]>,
        typing_energy: f32,
    ) {
        if paw.mode == PawMode::PressingKey {
            if paw.active_key.is_none() {
                paw.strike_timer = (paw.strike_timer - dt).max(0.0);
                if paw.strike_timer <= 0.0 {
                    if let Some(mpos) = mouse_target {
                        paw.mode = PawMode::OnMouse;
                        paw.target = [mpos[0] + 4.0, mpos[1] - 6.0];
                    } else if typing_energy > 0.25 {
                        paw.mode = PawMode::Curled;
                        paw.target = curled_pos;
                    } else {
                        paw.mode = PawMode::Resting;
                        paw.target = rest_pos;
                    }
                }
            }
        } else if let Some(mpos) = mouse_target {
            paw.mode = PawMode::OnMouse;
            paw.target = [mpos[0] + 4.0, mpos[1] - 6.0];
        } else if paw.mode == PawMode::Curled {
            if typing_energy < 0.15 && paw.idle_timer > 0.55 {
                paw.mode = PawMode::Resting;
                paw.target = rest_pos;
            } else {
                paw.target = curled_pos;
            }
        } else {
            paw.mode = PawMode::Resting;
            paw.target = rest_pos;
        }

        // Fast snappy spring for typing strikes and active mouse movement!
        let speed = match paw.mode {
            PawMode::PressingKey => 34.0,
            PawMode::Curled => 22.0,
            PawMode::OnMouse => 32.0,
            PawMode::Resting => 10.0,
        };
        let alpha = 1.0 - (-dt * speed).exp();
        paw.pos[0] += (paw.target[0] - paw.pos[0]) * alpha;
        paw.pos[1] += (paw.target[1] - paw.pos[1]) * alpha;
        paw.lift *= 1.0 - (dt * 24.0).min(1.0);

        let target_curl = if paw.mode == PawMode::Curled { 1.0 } else { 0.0 };
        let target_rest = if paw.mode == PawMode::Resting { 1.0 } else { 0.0 };
        let blend_alpha = 1.0 - (-dt * 12.0).exp();
        paw.curl_blend += (target_curl - paw.curl_blend) * blend_alpha;
        paw.rest_blend += (target_rest - paw.rest_blend) * blend_alpha;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_press_animates_paw_and_spawns_popup() {
        let mut cat = CatState::new();
        cat.handle_event(InputEvent::KeyDown(KeyId::A));
        cat.update(0.016);
        assert_eq!(cat.right_paw.mode, PawMode::PressingKey);
        assert_eq!(cat.popups.len(), 1);
        assert_eq!(cat.popups[0].label, "A");

        cat.handle_event(InputEvent::KeyUp(KeyId::A));
        for _ in 0..70 {
            cat.update(0.016);
        }
        assert!(cat.popups.is_empty());
    }

    #[test]
    fn mouse_move_places_left_paw_on_mouse() {
        let mut cat = CatState::new();
        cat.handle_event(InputEvent::MouseMove {
            norm_x: 0.2,
            norm_y: 0.3,
            screen_px: None,
            look_dir: Some([-0.8, -0.5]),
        });
        cat.update(0.05);
        assert_eq!(cat.left_paw.mode, PawMode::OnMouse);
        assert!(cat.gaze[0] < -0.3);
        assert!(cat.mouse_offset[0].abs() > 5.0);
    }

    #[test]
    fn petting_requires_rubbing_and_raises_pet_level() {
        let mut cat = CatState::new();
        // Single tiny hover pass should NOT close the cat's eyes!
        cat.register_pet(3.0, 0.0, 0.1);
        cat.update(0.016);
        assert!(cat.pet_level < 0.05);

        // Back-and-forth rubbing engages happy squint
        for i in 0..24 {
            let dir = if i % 2 == 0 { 12.0 } else { -12.0 };
            cat.register_pet(dir, 1.0, 0.3);
            cat.update(0.016);
        }
        assert!(cat.pet_level > 0.7);
    }

    #[test]
    fn password_mode_hides_keys_and_covers_eyes() {
        let mut cat = CatState::new();
        cat.handle_event(InputEvent::KeyDown(KeyId::A));
        assert_eq!(cat.popups.len(), 1);

        cat.set_password_mode(true);
        assert!(cat.popups.is_empty());
        assert!(cat.key_heat.is_empty());

        cat.handle_event(InputEvent::KeyDown(KeyId::S));
        cat.handle_event(InputEvent::KeyDown(KeyId::E));
        for _ in 0..30 {
            cat.update(0.016);
        }
        assert!(cat.popups.is_empty());
        assert!(cat.key_heat.is_empty());
        assert!(cat.cover_blend > 0.9);
    }
}
