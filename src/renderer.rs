use crate::cat::{CANVAS_H, CANVAS_W, CatState, KeyPopup, PawMode, PawState};
use crate::keyboard::KeyAccent;
use egui::{
    Align2, Color32, CornerRadius, FontId, Mesh, Painter, Pos2, Rect, Shape, Stroke, Vec2,
    epaint::{PathShape, Vertex},
};

// Palette matched closely to the reference video frames
const CAT_BLACK: Color32 = Color32::from_rgb(17, 17, 19);
const CAT_RIM: Color32 = Color32::from_rgba_premultiplied(210, 214, 208, 220);
const CAT_WHISKER: Color32 = Color32::from_rgba_premultiplied(220, 224, 218, 230);
const EYE_LIME: Color32 = Color32::from_rgb(206, 240, 74);
const EYE_GLOW: Color32 = Color32::from_rgba_premultiplied(165, 200, 48, 90);
const PUPIL_BLACK: Color32 = Color32::from_rgb(15, 15, 17);

const KBD_CASE: Color32 = Color32::from_rgb(24, 24, 27);
const KBD_CASE_SHADOW: Color32 = Color32::from_rgb(12, 12, 14);
const KEY_WHITE: Color32 = Color32::from_rgb(244, 244, 239);
const KEY_WHITE_SIDE: Color32 = Color32::from_rgb(206, 206, 199);
const KEY_ORANGE: Color32 = Color32::from_rgb(255, 168, 28);
const KEY_ORANGE_SIDE: Color32 = Color32::from_rgb(218, 132, 10);
const KEY_LIME: Color32 = Color32::from_rgb(184, 238, 76);
const KEY_LIME_SIDE: Color32 = Color32::from_rgb(138, 194, 40);

const MOUSE_WHITE: Color32 = Color32::from_rgb(244, 245, 240);
const MOUSE_LIME: Color32 = Color32::from_rgb(198, 242, 72);
const MOUSE_OUTLINE: Color32 = Color32::from_rgb(32, 32, 36);
const MOUSE_WHEEL: Color32 = Color32::from_rgb(245, 162, 32);

const POPUP_BG: Color32 = Color32::from_rgb(248, 248, 245);
const POPUP_LIP: Color32 = Color32::from_rgb(214, 214, 208);
const POPUP_BORDER: Color32 = Color32::from_rgb(30, 30, 34);

pub struct CanvasTransform {
    pub origin: Pos2,
    pub scale: f32,
}

impl CanvasTransform {
    pub fn from_rect(rect: Rect) -> Self {
        let sx = rect.width() / CANVAS_W;
        let sy = rect.height() / CANVAS_H;
        let scale = sx.min(sy);
        let used_w = CANVAS_W * scale;
        let used_h = CANVAS_H * scale;
        let origin = Pos2::new(
            rect.left() + (rect.width() - used_w) * 0.5,
            rect.top() + (rect.height() - used_h) * 0.5,
        );
        Self { origin, scale }
    }

    #[inline]
    pub fn pt(&self, x: f32, y: f32) -> Pos2 {
        Pos2::new(self.origin.x + x * self.scale, self.origin.y + y * self.scale)
    }

    #[inline]
    pub fn s(&self, v: f32) -> f32 {
        v * self.scale
    }

    /// Returns virtual canvas coordinates for a screen point.
    pub fn to_canvas(&self, pos: Pos2) -> [f32; 2] {
        [
            (pos.x - self.origin.x) / self.scale.max(0.001),
            (pos.y - self.origin.y) / self.scale.max(0.001),
        ]
    }
}

/// Returns true if the given virtual canvas point is on the cat's head/ears (petting zone).
pub fn is_on_cat_head(cat: &CatState, canvas_pt: [f32; 2]) -> bool {
    let hx = 182.0 + cat.lean_x;
    let hy = 104.0 + cat.typing_bob;
    let head_dx = (canvas_pt[0] - hx) / 46.0;
    let head_dy = (canvas_pt[1] - hy) / 46.0;
    head_dx * head_dx + head_dy * head_dy <= 1.0
}

/// Returns true if the given virtual canvas point is inside the cat's head or torso.
pub fn is_on_cat_body(cat: &CatState, canvas_pt: [f32; 2]) -> bool {
    if is_on_cat_head(cat, canvas_pt) {
        return true;
    }
    let bx = 182.0 + cat.lean_x * 0.45;
    let by = 184.0;
    let body_dx = (canvas_pt[0] - bx) / 56.0;
    let body_dy = (canvas_pt[1] - by) / 58.0;
    body_dx * body_dx + body_dy * body_dy <= 1.0
}

pub fn render_scene(painter: &Painter, rect: Rect, cat: &CatState, show_keycaps: bool) {
    let tf = CanvasTransform::from_rect(rect);

    draw_desk_shadow(painter, &tf, cat);
    draw_tail(painter, &tf, cat);
    draw_hind_paws(painter, &tf, cat);
    draw_cat_body_and_head(painter, &tf, cat);
    draw_face(painter, &tf, cat);

    // Desk props (mouse + 180-degree rotated 60% mechanical keyboard)
    draw_mouse(painter, &tf, cat);
    draw_keyboard(painter, &tf, cat);

    // Front arms & paws (rendered OVER body and keyboard/mouse — or covering the eyes in password mode!)
    if cat.cover_blend > 0.02 {
        draw_covering_paws(painter, &tf, cat);
    } else {
        draw_paw(painter, &tf, cat, &cat.left_paw, true);
        draw_paw(painter, &tf, cat, &cat.right_paw, false);
    }

    // Floating white keycap badges on top (strictly hidden in password mode!)
    if show_keycaps && !cat.password_mode {
        for popup in &cat.popups {
            draw_key_popup(painter, &tf, popup);
        }
    }
}

fn draw_desk_shadow(painter: &Painter, tf: &CanvasTransform, cat: &CatState) {
    // Very soft shadow beneath keyboard and mouse (no crisp fringe stroke!)
    let shadow_col = Color32::from_rgba_unmultiplied(0, 0, 0, 34);
    let kbd_shadow = vec![
        tf.pt(82.0, 298.0),
        tf.pt(308.0, 298.0),
        tf.pt(313.0, 306.0),
        tf.pt(77.0, 306.0),
    ];
    fill_polygon_no_fringe(painter, &kbd_shadow, shadow_col);

    let m = cat.mouse_current_pos();
    painter.circle_filled(
        tf.pt(m[0], m[1] + 6.0),
        tf.s(18.0),
        Color32::from_rgba_unmultiplied(0, 0, 0, 28),
    );
}

fn draw_tail(painter: &Painter, tf: &CanvasTransform, cat: &CatState) {
    let wag = cat.tail_phase.sin();
    let wag2 = (cat.tail_phase * 1.3 + 0.7).cos();

    let p0 = tf.pt(216.0 + cat.lean_x * 0.15, 208.0);
    let p1 = tf.pt(246.0, 212.0 + wag * 4.0);
    let p2 = tf.pt(254.0 + wag * 5.0, 174.0 + wag2 * 5.0);
    let p3 = tf.pt(286.0 + wag * 8.0, 175.0 + wag2 * 6.0);

    let tail_pts = sample_cubic_bezier(p0, p1, p2, p3, 24);
    draw_thick_round_polyline(painter, &tail_pts, tf.s(14.5), CAT_BLACK);
}

fn draw_hind_paws(painter: &Painter, tf: &CanvasTransform, cat: &CatState) {
    // Cute small rounded hind paws peeking out at the bottom corners of the cat's seated body
    let lx = 148.0 + cat.lean_x * 0.12;
    let rx = 216.0 + cat.lean_x * 0.12;
    let y = 226.0;

    for (cx, side) in [(lx, -1.0_f32), (rx, 1.0_f32)] {
        let ellipse = sample_ellipse(tf.pt(cx, y), tf.s(14.0), tf.s(11.0), 20);
        fill_smooth_polygon(painter, &ellipse, CAT_BLACK);

        // Delicate pale rim highlight along the lower outer edge (visible in frame_01/03/04)
        let mut arc = Vec::with_capacity(12);
        for i in 0..=10 {
            let t = i as f32 / 10.0;
            let ang = if side < 0.0 {
                std::f32::consts::PI * (0.28 + t * 0.62)
            } else {
                std::f32::consts::PI * (0.10 + t * 0.62)
            };
            arc.push(tf.pt(cx + ang.cos() * 14.0, y + ang.sin() * 10.8));
        }
        painter.add(Shape::Path(PathShape::line(
            arc,
            Stroke::new(tf.s(1.3), CAT_RIM),
        )));
    }
}

fn draw_cat_body_and_head(painter: &Painter, tf: &CanvasTransform, cat: &CatState) {
    let lx = cat.lean_x;
    let bob = cat.typing_bob;

    // 1. Seated pear-shaped torso (bottom anchored at x=182, top leans smoothly with head!)
    let mut torso = Vec::with_capacity(48);
    // Start at left neck, curve out along left shoulder/hip, across bottom, up right hip/shoulder
    let left_neck = tf.pt(153.0 + lx * 0.92, 134.0 + bob);
    let right_neck = tf.pt(211.0 + lx * 0.92, 134.0 + bob);
    let left_hip = tf.pt(136.0 + lx * 0.16, 214.0);
    let right_hip = tf.pt(228.0 + lx * 0.16, 214.0);

    // Left flank
    torso.extend(sample_cubic_bezier(
        left_neck,
        tf.pt(142.0 + lx * 0.62, 162.0 + bob * 0.6),
        tf.pt(126.0 + lx * 0.25, 188.0),
        left_hip,
        14,
    ));
    // Bottom curve
    torso.extend(sample_cubic_bezier(
        left_hip,
        tf.pt(150.0 + lx * 0.1, 232.0),
        tf.pt(214.0 + lx * 0.1, 232.0),
        right_hip,
        12,
    ));
    // Right flank
    torso.extend(sample_cubic_bezier(
        right_hip,
        tf.pt(238.0 + lx * 0.25, 188.0),
        tf.pt(222.0 + lx * 0.62, 162.0 + bob * 0.6),
        right_neck,
        14,
    ));
    fill_smooth_polygon(painter, &torso, CAT_BLACK);

    // 2. Head + rounded-point ears (rotated smoothly by head_tilt!)
    let head_center = [182.0 + lx, 108.0 + bob];
    let tilt = cat.head_tilt;
    let rot = |dx: f32, dy: f32| -> Pos2 {
        let c = tilt.cos();
        let s = tilt.sin();
        let rx = dx * c - dy * s;
        let ry = dx * s + dy * c;
        tf.pt(head_center[0] + rx, head_center[1] + ry)
    };

    // Main rounded head ellipse (slightly wider cheeks like the video)
    let mut head_pts = Vec::with_capacity(40);
    for i in 0..40 {
        let a = (i as f32 / 40.0) * std::f32::consts::TAU;
        // Slightly fuller lower cheeks for extra cuteness
        let cheek = if a.sin() > 0.0 { 1.0 + 0.06 * a.sin() } else { 1.0 };
        let dx = a.cos() * 41.5 * cheek;
        let dy = a.sin() * 34.5;
        head_pts.push(rot(dx, dy));
    }
    fill_smooth_polygon(painter, &head_pts, CAT_BLACK);

    // Left ear (viewer's left) — soft rounded triangular cat ear
    let mut left_ear = Vec::new();
    left_ear.extend(sample_cubic_bezier(
        rot(-35.0, -14.0),
        rot(-37.0, -34.0),
        rot(-33.0, -54.0),
        rot(-24.0, -55.0),
        10,
    ));
    left_ear.extend(sample_cubic_bezier(
        rot(-24.0, -55.0),
        rot(-16.0, -56.0),
        rot(-10.0, -40.0),
        rot(-6.0, -28.0),
        10,
    ));
    left_ear.push(rot(-20.0, -18.0));
    fill_smooth_polygon(painter, &left_ear, CAT_BLACK);

    // Right ear (viewer's right)
    let mut right_ear = Vec::new();
    right_ear.extend(sample_cubic_bezier(
        rot(6.0, -28.0),
        rot(10.0, -40.0),
        rot(16.0, -56.0),
        rot(24.0, -55.0),
        10,
    ));
    right_ear.extend(sample_cubic_bezier(
        rot(24.0, -55.0),
        rot(33.0, -54.0),
        rot(37.0, -34.0),
        rot(35.0, -14.0),
        10,
    ));
    right_ear.push(rot(20.0, -18.0));
    fill_smooth_polygon(painter, &right_ear, CAT_BLACK);
}

fn draw_face(painter: &Painter, tf: &CanvasTransform, cat: &CatState) {
    let head_center = [182.0 + cat.lean_x, 108.0 + cat.typing_bob];
    let tilt = cat.head_tilt;
    // Slight pseudo-3D yaw shift of face features toward gaze
    let face_shift_x = cat.gaze[0] * 5.2;
    let face_shift_y = cat.gaze[1] * 2.8;

    let rot = |dx: f32, dy: f32| -> Pos2 {
        let c = tilt.cos();
        let s = tilt.sin();
        let x = dx + face_shift_x;
        let y = dy + face_shift_y;
        tf.pt(head_center[0] + x * c - y * s, head_center[1] + x * s + y * c)
    };

    // Eye openness: 1.0 = wide open luminous ovals, 0.0 = closed blink, ~0.16 = happy petted horizontal slits!
    let squint = cat.pet_level;
    let open_factor = ((1.0 - squint * 0.84) * (1.0 - cat.blink)).max(0.0);

    for side in [-1.0_f32, 1.0_f32] {
        let ex = side * 16.5;
        let ey = -4.5;

        if open_factor < 0.06 {
            // Closed blink line
            let p0 = rot(ex - 10.0, ey + 1.0);
            let p1 = rot(ex + 10.0, ey + 1.0);
            painter.line_segment([p0, p1], Stroke::new(tf.s(1.6), EYE_LIME));
        } else if squint > 0.45 && cat.blink < 0.5 {
            // Iconic blissful horizontal chartreuse slits (frame_02.png!)
            let half_w = 11.5;
            let half_h = (2.1 + (1.0 - squint) * 7.5) * (1.0 - cat.blink);
            let mut slit = Vec::with_capacity(20);
            for i in 0..20 {
                let a = (i as f32 / 20.0) * std::f32::consts::TAU;
                // Slight serene droop/smile arc
                let arc = (a.cos() * a.cos()) * -1.1 * squint;
                slit.push(rot(ex + a.cos() * half_w, ey + 2.0 + a.sin() * half_h + arc));
            }
            fill_smooth_polygon(painter, &slit, EYE_LIME);
        } else {
            // Big luminous yellow-green cat eyes with expressive tracking vertical oval pupils!
            let rx = 12.2;
            let ry = 11.0 * open_factor;

            // Soft outer iris rim
            let mut glow = Vec::with_capacity(24);
            for i in 0..24 {
                let a = (i as f32 / 24.0) * std::f32::consts::TAU;
                glow.push(rot(ex + a.cos() * (rx + 0.9), ey + a.sin() * (ry + 0.9)));
            }
            fill_smooth_polygon(painter, &glow, EYE_GLOW);

            let mut iris = Vec::with_capacity(24);
            for i in 0..24 {
                let a = (i as f32 / 24.0) * std::f32::consts::TAU;
                iris.push(rot(ex + a.cos() * rx, ey + a.sin() * ry));
            }
            fill_smooth_polygon(painter, &iris, EYE_LIME);

            // Bold black vertical oval pupil with generous tracking travel clamped cleanly inside the iris
            let prx = 4.1;
            let pry = (6.8 * open_factor).min(ry - 1.2).max(1.5);
            let max_dx = (rx - prx - 0.8).max(1.0);
            let max_dy = (ry - pry - 0.6).max(0.8);
            let mut gx = cat.gaze[0] * max_dx;
            let mut gy = cat.gaze[1] * max_dy;
            // Clamp pupil offset inside the allowed inner ellipse
            let norm_dist = ((gx / max_dx).powi(2) + (gy / max_dy).powi(2)).sqrt();
            if norm_dist > 1.0 {
                gx /= norm_dist;
                gy /= norm_dist;
            }
            let px = ex + gx;
            let py = ey + gy;
            let mut pupil = Vec::with_capacity(20);
            for i in 0..20 {
                let a = (i as f32 / 20.0) * std::f32::consts::TAU;
                pupil.push(rot(px + a.cos() * prx, py + a.sin() * pry));
            }
            fill_smooth_polygon(painter, &pupil, PUPIL_BLACK);
        }
    }

    // 3 short delicate white whiskers on each cheek (matching frame_01..05)
    let whisker_stroke = Stroke::new(tf.s(1.25), CAT_WHISKER);
    for side in [-1.0_f32, 1.0_f32] {
        let base_x = side * 21.5;
        let specs = [
            (10.0_f32, 0.0_f32, 10.5_f32, -0.5_f32),
            (13.5, 0.5, 11.2, 2.2),
            (17.0, 0.0, 10.2, 4.8),
        ];
        for (by, dx0, len, dy1) in specs {
            let p0 = rot(base_x + side * dx0, by);
            let p1 = rot(base_x + side * (dx0 + len), by + dy1);
            painter.line_segment([p0, p1], whisker_stroke);
        }
    }
}

fn draw_mouse(painter: &Painter, tf: &CanvasTransform, cat: &CatState) {
    let m = cat.mouse_current_pos();
    let tilt = cat.mouse_tilt;
    let body_color = lerp_color(MOUSE_WHITE, MOUSE_LIME, cat.mouse_click_glow);

    let rot = |dx: f32, dy: f32, y_off: f32| -> Pos2 {
        let c = tilt.cos();
        let s = tilt.sin();
        tf.pt(m[0] + dx * c - dy * s, m[1] + y_off + dx * s + dy * c)
    };

    let sample_rot_ellipse = |rx: f32, ry: f32, y_off: f32| -> Vec<Pos2> {
        (0..24)
            .map(|i| {
                let a = (i as f32 / 24.0) * std::f32::consts::TAU;
                rot(a.cos() * rx, a.sin() * ry, y_off)
            })
            .collect()
    };

    // Subtle 3D bottom lip
    let lip = sample_rot_ellipse(16.5, 19.0, 2.5);
    fill_smooth_polygon(painter, &lip, Color32::from_rgb(198, 200, 194));

    // Main oval white/lime mouse body
    let body = sample_rot_ellipse(16.2, 18.5, 0.0);
    fill_smooth_polygon(painter, &body, body_color);
    painter.add(Shape::Path(PathShape::closed_line(
        body,
        Stroke::new(tf.s(1.5), MOUSE_OUTLINE),
    )));

    // Tiny orange/amber scroll wheel slit near top-center (clearly visible in frame_01/03!)
    let w0 = rot(0.5, -7.5, 0.0);
    let w1 = rot(-0.5, -1.5, 0.0);
    let wheel_col = if cat.mouse_click_glow > 0.3 {
        Color32::from_rgb(42, 48, 22)
    } else {
        MOUSE_WHEEL
    };
    painter.line_segment([w0, w1], Stroke::new(tf.s(2.1), wheel_col));
}

fn draw_keyboard(painter: &Painter, tf: &CanvasTransform, cat: &CatState) {
    // 1. Dark mechanical keyboard base / bezel with 3D front lip
    let case_bottom = vec![
        tf.pt(90.0, 234.0),
        tf.pt(300.0, 234.0),
        tf.pt(313.0, 297.0),
        tf.pt(311.0, 303.0),
        tf.pt(79.0, 303.0),
        tf.pt(77.0, 297.0),
    ];
    fill_smooth_polygon(painter, &case_bottom, KBD_CASE_SHADOW);

    let case_top = vec![
        tf.pt(92.0, 231.0),
        tf.pt(298.0, 231.0),
        tf.pt(312.0, 296.0),
        tf.pt(78.0, 296.0),
    ];
    fill_smooth_polygon(painter, &case_top, KBD_CASE);

    // 2. Individual 3D perspective keycaps (5 rows, 180-degree rotated to face the cat!)
    let pad_u = 0.0045;
    let pad_v = 0.022;

    for spec in &cat.layout.keys {
        let heat = cat.key_heat.get(&spec.key).copied().unwrap_or(0.0);
        let depress = heat * 2.2;

        let u0 = spec.u_start + pad_u;
        let u1 = spec.u_start + spec.u_width - pad_u;
        let v0 = spec.row as f32 / 5.0 + pad_v;
        let v1 = (spec.row as f32 + 1.0) / 5.0 - pad_v;

        let base_top_col = match spec.accent {
            KeyAccent::White => KEY_WHITE,
            KeyAccent::Orange => KEY_ORANGE,
        };
        let base_side_col = match spec.accent {
            KeyAccent::White => KEY_WHITE_SIDE,
            KeyAccent::Orange => KEY_ORANGE_SIDE,
        };
        let top_col = lerp_color(base_top_col, KEY_LIME, heat);
        let side_col = lerp_color(base_side_col, KEY_LIME_SIDE, heat);

        let tl = CatState::keyboard_uv_to_canvas(u0, v0);
        let tr = CatState::keyboard_uv_to_canvas(u1, v0);
        let br = CatState::keyboard_uv_to_canvas(u1, v1);
        let bl = CatState::keyboard_uv_to_canvas(u0, v1);

        // Keycap bottom 3D skirt
        let skirt = vec![
            tf.pt(bl[0], bl[1] - 0.5 + depress * 0.5),
            tf.pt(br[0], br[1] - 0.5 + depress * 0.5),
            tf.pt(br[0], br[1] + 2.4),
            tf.pt(bl[0], bl[1] + 2.4),
        ];
        fill_smooth_polygon(painter, &skirt, side_col);

        // Keycap top surface
        let cap = vec![
            tf.pt(tl[0], tl[1] + depress),
            tf.pt(tr[0], tr[1] + depress),
            tf.pt(br[0], br[1] + depress),
            tf.pt(bl[0], bl[1] + depress),
        ];
        fill_smooth_polygon(painter, &cap, top_col);
    }
}

fn draw_covering_paws(painter: &Painter, tf: &CanvasTransform, cat: &CatState) {
    let t = ease_out_cubic(cat.cover_blend.clamp(0.0, 1.0));
    let lx = cat.lean_x;
    let bob = cat.typing_bob;
    let tilt = cat.head_tilt;
    let head_center = [182.0 + lx, 108.0 + bob];
    let shy_press = (cat.time * 24.0).sin() * 1.4 * cat.shy_wiggle;

    let rot_head = |dx: f32, dy: f32| -> [f32; 2] {
        let c = tilt.cos();
        let s = tilt.sin();
        [
            head_center[0] + dx * c - dy * s,
            head_center[1] + dx * s + dy * c,
        ]
    };

    for is_left in [true, false] {
        let side = if is_left { -1.0_f32 } else { 1.0_f32 };
        let paw_state = if is_left { &cat.left_paw } else { &cat.right_paw };

        // Target sits squarely over each bright chartreuse eye!
        let eye_target = rot_head(side * 16.5, -3.5 + shy_press * side);
        let start = paw_state.pos;
        let tip = [
            start[0] + (eye_target[0] - start[0]) * t,
            start[1] + (eye_target[1] - start[1]) * t,
        ];

        // Raised elbow sweeps nicely up from the lower side of the torso
        let shoulder = [182.0 + lx * 0.5 + side * 30.0, 182.0 + bob * 0.3];
        let elbow = [
            182.0 + lx * 0.68 + side * (42.0 - 4.0 * t),
            152.0 - 10.0 * t + bob * 0.4,
        ];
        let ctrl2 = [
            tip[0] + side * 14.0,
            tip[1] + 24.0,
        ];

        let arm_pts = sample_cubic_bezier(
            tf.pt(shoulder[0], shoulder[1]),
            tf.pt(elbow[0], elbow[1]),
            tf.pt(ctrl2[0], ctrl2[1]),
            tf.pt(tip[0], tip[1] + 5.0),
            18,
        );

        // Solid sleek black raised arm
        draw_thick_round_polyline(painter, &arm_pts, tf.s(16.0), CAT_BLACK);

        // Delicate pale rim outlines along the raised forearm so the silhouette reads crisply against the black torso/head!
        let rim_alpha = (t * 215.0).round() as u8;
        let rim_col = Color32::from_rgba_unmultiplied(218, 222, 216, rim_alpha);
        let rim_stroke = Stroke::new(tf.s(1.35), rim_col);
        if arm_pts.len() >= 6 {
            let mut inner_edge = Vec::new();
            let mut outer_edge = Vec::new();
            let half_w = tf.s(8.2);
            for i in 2..(arm_pts.len() - 1) {
                let dir = (arm_pts[i] - arm_pts[i - 1]).normalized();
                let normal = Vec2::new(-dir.y, dir.x) * side;
                inner_edge.push(arm_pts[i] - normal * half_w);
                outer_edge.push(arm_pts[i] + normal * half_w);
            }
            painter.add(Shape::Path(PathShape::line(inner_edge, rim_stroke)));
            // Outer rim only on the upper half near the cheek
            let mid = outer_edge.len() / 2;
            if outer_edge.len() > mid + 2 {
                painter.add(Shape::Path(PathShape::line(
                    outer_edge[mid..].to_vec(),
                    rim_stroke,
                )));
            }
        }

        // Plump rounded paw pad pressed right over the eye!
        let paw_angle = tilt + side * 0.22;
        let paw_pts: Vec<Pos2> = (0..24)
            .map(|i| {
                let a = (i as f32 / 24.0) * std::f32::consts::TAU;
                let rx = 15.2;
                let ry = 13.4;
                let dx = a.cos() * rx;
                let dy = a.sin() * ry;
                let c = paw_angle.cos();
                let s = paw_angle.sin();
                tf.pt(tip[0] + dx * c - dy * s, tip[1] + dx * s + dy * c)
            })
            .collect();
        fill_smooth_polygon(painter, &paw_pts, CAT_BLACK);

        // Crisp cute pale contour arc around the paw top + two tiny toe-bean divider ticks!
        let mut paw_arc = Vec::with_capacity(18);
        for i in 0..=16 {
            let u = i as f32 / 16.0;
            let a = -std::f32::consts::PI * (1.08 - u * 1.16);
            let dx = a.cos() * 15.4;
            let dy = a.sin() * 13.6;
            let c = paw_angle.cos();
            let s = paw_angle.sin();
            paw_arc.push(tf.pt(tip[0] + dx * c - dy * s, tip[1] + dx * s + dy * c));
        }
        painter.add(Shape::Path(PathShape::line(
            paw_arc,
            Stroke::new(tf.s(1.45), rim_col),
        )));

        // Two tiny cute toe lines near the top edge of each paw
        let toe_stroke = Stroke::new(tf.s(1.2), rim_col);
        for toe_x in [-4.5_f32, 4.5_f32] {
            let c = paw_angle.cos();
            let s = paw_angle.sin();
            let p0 = tf.pt(
                tip[0] + toe_x * c - (-12.8) * s,
                tip[1] + toe_x * s + (-12.8) * c,
            );
            let p1 = tf.pt(
                tip[0] + (toe_x * 0.85) * c - (-7.2) * s,
                tip[1] + (toe_x * 0.85) * s + (-7.2) * c,
            );
            painter.line_segment([p0, p1], toe_stroke);
        }
    }
}

fn draw_paw(painter: &Painter, tf: &CanvasTransform, cat: &CatState, paw: &PawState, is_left: bool) {
    let side = if is_left { -1.0_f32 } else { 1.0_f32 };
    let lx = cat.lean_x;

    // 1. Resting straight vertical front leg (like frame_01.png!)
    if paw.rest_blend > 0.75 && paw.mode == PawMode::Resting {
        let top_x = 182.0 + lx * 0.45 + side * 18.5;
        let top_y = 176.0;
        let bot_x = paw.pos[0];
        let bot_y = paw.pos[1];

        let leg = vec![
            tf.pt(top_x - 7.5, top_y),
            tf.pt(top_x + 7.5, top_y),
            tf.pt(bot_x + 8.0, bot_y),
            tf.pt(bot_x - 8.0, bot_y),
        ];
        fill_smooth_polygon(painter, &leg, CAT_BLACK);
        painter.circle_filled(tf.pt(bot_x, bot_y - 1.0), tf.s(8.5), CAT_BLACK);

        // Iconic thin pale-white slightly curved edge lines separating legs from black body (frame_01.png!)
        let alpha = ((paw.rest_blend - 0.5) * 2.0).clamp(0.0, 1.0);
        let rim = Color32::from_rgba_unmultiplied(218, 222, 216, (alpha * 215.0) as u8);
        let stroke = Stroke::new(tf.s(1.3), rim);
        let outer_curve = sample_cubic_bezier(
            tf.pt(top_x + side * 8.2, top_y + 2.0),
            tf.pt(top_x + side * 9.0, (top_y + bot_y) * 0.5),
            tf.pt(bot_x + side * 8.5, bot_y - 14.0),
            tf.pt(bot_x + side * 8.0, bot_y - 4.0),
            8,
        );
        let inner_curve = sample_cubic_bezier(
            tf.pt(top_x - side * 8.0, top_y + 4.0),
            tf.pt(top_x - side * 7.2, (top_y + bot_y) * 0.5),
            tf.pt(bot_x - side * 8.0, bot_y - 14.0),
            tf.pt(bot_x - side * 8.0, bot_y - 4.0),
            8,
        );
        painter.add(Shape::Path(PathShape::line(outer_curve, stroke)));
        painter.add(Shape::Path(PathShape::line(inner_curve, stroke)));
        return;
    }

    // 2. Curled-up cute chest paw while typing fast (like frame_03.png!)
    if paw.curl_blend > 0.55 {
        let cx = paw.pos[0] + lx * 0.35;
        let cy = paw.pos[1];
        let paw_oval = sample_ellipse(tf.pt(cx, cy), tf.s(11.5), tf.s(10.0), 20);
        fill_smooth_polygon(painter, &paw_oval, CAT_BLACK);

        // Cute U-shaped pale white contour underneath the curled paw (frame_03.png!)
        let mut arc = Vec::with_capacity(14);
        for i in 0..=12 {
            let t = i as f32 / 12.0;
            let ang = std::f32::consts::PI * (0.08 + t * 0.84);
            arc.push(tf.pt(cx + ang.cos() * 11.8, cy + ang.sin() * 10.2));
        }
        let alpha = (paw.curl_blend * 230.0) as u8;
        painter.add(Shape::Path(PathShape::line(
            arc,
            Stroke::new(
                tf.s(1.5),
                Color32::from_rgba_unmultiplied(222, 226, 220, alpha),
            ),
        )));
        return;
    }

    // 3. Articulated reaching arm (Mouse or PressingKey — frame_02.png, frame_04.png, frame_05.png!)
    let shoulder = [182.0 + lx * 0.72 + side * 22.0, 158.0 + cat.typing_bob * 0.4];
    let tip = [paw.pos[0], paw.pos[1] - paw.lift];

    // Natural slight outward elbow curve
    let mid_x = (shoulder[0] + tip[0]) * 0.5 + side * 6.0;
    let mid_y = (shoulder[1] + tip[1]) * 0.5 - 3.0;
    let ctrl1 = [
        shoulder[0] + (mid_x - shoulder[0]) * 0.65,
        shoulder[1] + (mid_y - shoulder[1]) * 0.65,
    ];
    let ctrl2 = [
        tip[0] + (mid_x - tip[0]) * 0.55,
        tip[1] + (mid_y - tip[1]) * 0.55,
    ];

    let arm_pts = sample_cubic_bezier(
        tf.pt(shoulder[0], shoulder[1]),
        tf.pt(ctrl1[0], ctrl1[1]),
        tf.pt(ctrl2[0], ctrl2[1]),
        tf.pt(tip[0], tip[1]),
        18,
    );

    // Solid sleek black arm + round paw pad
    draw_thick_round_polyline(painter, &arm_pts, tf.s(15.5), CAT_BLACK);
    painter.circle_filled(tf.pt(tip[0], tip[1]), tf.s(10.2), CAT_BLACK);

    // Subtle pale rim highlight ONLY where the arm overlaps the dark torso (never floating in air!)
    draw_arm_rim_highlights(painter, tf, cat, &arm_pts, tf.s(8.0), tf.s(1.2));
}

fn draw_arm_rim_highlights(
    painter: &Painter,
    tf: &CanvasTransform,
    cat: &CatState,
    pts: &[Pos2],
    half_w: f32,
    stroke_w: f32,
) {
    if pts.len() < 4 {
        return;
    }
    let rim_col = Color32::from_rgba_unmultiplied(212, 216, 210, 185);
    let stroke = Stroke::new(stroke_w, rim_col);

    for side_sign in [-1.0_f32, 1.0_f32] {
        let mut seg = Vec::new();
        for i in 1..(pts.len() - 2) {
            let dir = (pts[i + 1] - pts[i - 1]).normalized();
            let normal = Vec2::new(-dir.y, dir.x) * side_sign;
            let edge_pt = pts[i] + normal * half_w;
            let c_pt = tf.to_canvas(edge_pt);
            // Keep highlight strictly inside torso bounds so it separates black-on-black cleanly
            let bx = 182.0 + cat.lean_x * 0.4;
            let inside_torso =
                ((c_pt[0] - bx) / 46.0).powi(2) + ((c_pt[1] - 190.0) / 46.0).powi(2) <= 1.0
                    && c_pt[1] > 165.0
                    && c_pt[1] < 228.0;
            if inside_torso {
                seg.push(edge_pt);
            } else if seg.len() >= 2 {
                painter.add(Shape::Path(PathShape::line(std::mem::take(&mut seg), stroke)));
            } else {
                seg.clear();
            }
        }
        if seg.len() >= 2 {
            painter.add(Shape::Path(PathShape::line(seg, stroke)));
        }
    }
}

fn draw_key_popup(painter: &Painter, tf: &CanvasTransform, popup: &KeyPopup) {
    // Spring pop-in scale
    let pop_t = (popup.age / 0.09).clamp(0.0, 1.0);
    let scale = 0.65 + 0.35 * ease_out_back(pop_t);

    // Gentle upward float & fade-out after release
    let fade_t = (popup.release_age / 0.85).clamp(0.0, 1.0);
    let alpha = (1.0 - fade_t * fade_t).clamp(0.0, 1.0);
    if alpha <= 0.02 {
        return;
    }

    let float_y = popup.release_age * 26.0 + popup.y_stagger;
    let cx = popup.base_x;
    let cy = 205.0 - float_y;

    let w = (if popup.is_wide { 36.0 } else { 25.0 }) * scale;
    let h = 23.0 * scale;

    let rect = Rect::from_center_size(tf.pt(cx, cy), Vec2::new(tf.s(w), tf.s(h)));
    let rounding = CornerRadius::same((tf.s(6.5 * scale)).round().clamp(2.0, 20.0) as u8);

    // Soft drop shadow
    let shadow_rect = rect.translate(Vec2::new(0.0, tf.s(2.6)));
    painter.rect_filled(
        shadow_rect,
        rounding,
        Color32::from_rgba_unmultiplied(0, 0, 0, (alpha * 55.0) as u8),
    );

    // 3D bottom lip of the keycap badge
    let lip_rect = rect.translate(Vec2::new(0.0, tf.s(1.6)));
    painter.rect_filled(lip_rect, rounding, with_alpha(POPUP_LIP, alpha));

    // White keycap face + dark crisp border
    painter.rect_filled(rect, rounding, with_alpha(POPUP_BG, alpha));
    painter.rect_stroke(
        rect,
        rounding,
        Stroke::new(tf.s(1.4), with_alpha(POPUP_BORDER, alpha)),
        egui::StrokeKind::Outside,
    );

    // Label / crisp custom vector symbol (avoids missing Unicode glyphs across OSes!)
    let ink = with_alpha(POPUP_BORDER, alpha);
    let icon_stroke = Stroke::new(tf.s(1.5 * scale), ink);
    let c = rect.center();
    let u = tf.s(scale);

    match popup.key {
        crate::keyboard::KeyId::Space => {
            let bar_y = c.y + 4.2 * u;
            let bar_half = 8.0 * u;
            painter.line_segment(
                [Pos2::new(c.x - bar_half, bar_y), Pos2::new(c.x + bar_half, bar_y)],
                Stroke::new(
                    1.8 * u,
                    with_alpha(Color32::from_rgb(155, 155, 150), alpha),
                ),
            );
        }
        crate::keyboard::KeyId::LeftShift | crate::keyboard::KeyId::RightShift => {
            // Hollow upward Shift arrow (matches frame_04/05!)
            let pts = vec![
                Pos2::new(c.x, c.y - 5.8 * u),
                Pos2::new(c.x + 5.6 * u, c.y + 0.2 * u),
                Pos2::new(c.x + 2.4 * u, c.y + 0.2 * u),
                Pos2::new(c.x + 2.4 * u, c.y + 5.2 * u),
                Pos2::new(c.x - 2.4 * u, c.y + 5.2 * u),
                Pos2::new(c.x - 2.4 * u, c.y + 0.2 * u),
                Pos2::new(c.x - 5.6 * u, c.y + 0.2 * u),
            ];
            painter.add(Shape::Path(PathShape::closed_line(pts, icon_stroke)));
        }
        crate::keyboard::KeyId::CapsLock => {
            // Hollow upward arrow + base bar (CapsLock icon in frame_04/05!)
            let pts = vec![
                Pos2::new(c.x, c.y - 6.2 * u),
                Pos2::new(c.x + 5.4 * u, c.y - 0.6 * u),
                Pos2::new(c.x + 2.2 * u, c.y - 0.6 * u),
                Pos2::new(c.x + 2.2 * u, c.y + 2.6 * u),
                Pos2::new(c.x - 2.2 * u, c.y + 2.6 * u),
                Pos2::new(c.x - 2.2 * u, c.y - 0.6 * u),
                Pos2::new(c.x - 5.4 * u, c.y - 0.6 * u),
            ];
            painter.add(Shape::Path(PathShape::closed_line(pts, icon_stroke)));
            painter.line_segment(
                [
                    Pos2::new(c.x - 3.4 * u, c.y + 5.4 * u),
                    Pos2::new(c.x + 3.4 * u, c.y + 5.4 * u),
                ],
                icon_stroke,
            );
        }
        crate::keyboard::KeyId::Enter => {
            // Clean bent return arrow
            let pts = vec![
                Pos2::new(c.x + 4.8 * u, c.y - 4.2 * u),
                Pos2::new(c.x + 4.8 * u, c.y + 1.8 * u),
                Pos2::new(c.x - 4.4 * u, c.y + 1.8 * u),
            ];
            painter.add(Shape::Path(PathShape::line(pts, icon_stroke)));
            painter.line_segment(
                [Pos2::new(c.x - 4.4 * u, c.y + 1.8 * u), Pos2::new(c.x - 1.4 * u, c.y - 1.2 * u)],
                icon_stroke,
            );
            painter.line_segment(
                [Pos2::new(c.x - 4.4 * u, c.y + 1.8 * u), Pos2::new(c.x - 1.4 * u, c.y + 4.8 * u)],
                icon_stroke,
            );
        }
        crate::keyboard::KeyId::Backspace => {
            // Left arrow
            painter.line_segment(
                [Pos2::new(c.x + 5.0 * u, c.y), Pos2::new(c.x - 4.8 * u, c.y)],
                icon_stroke,
            );
            painter.line_segment(
                [Pos2::new(c.x - 4.8 * u, c.y), Pos2::new(c.x - 1.2 * u, c.y - 3.6 * u)],
                icon_stroke,
            );
            painter.line_segment(
                [Pos2::new(c.x - 4.8 * u, c.y), Pos2::new(c.x - 1.2 * u, c.y + 3.6 * u)],
                icon_stroke,
            );
        }
        crate::keyboard::KeyId::Tab => {
            painter.line_segment(
                [Pos2::new(c.x - 5.2 * u, c.y), Pos2::new(c.x + 3.2 * u, c.y)],
                icon_stroke,
            );
            painter.line_segment(
                [Pos2::new(c.x + 3.2 * u, c.y), Pos2::new(c.x + 0.2 * u, c.y - 3.0 * u)],
                icon_stroke,
            );
            painter.line_segment(
                [Pos2::new(c.x + 3.2 * u, c.y), Pos2::new(c.x + 0.2 * u, c.y + 3.0 * u)],
                icon_stroke,
            );
            painter.line_segment(
                [Pos2::new(c.x + 5.2 * u, c.y - 3.6 * u), Pos2::new(c.x + 5.2 * u, c.y + 3.6 * u)],
                icon_stroke,
            );
        }
        crate::keyboard::KeyId::LeftMeta | crate::keyboard::KeyId::RightMeta => {
            painter.text(
                c,
                Align2::CENTER_CENTER,
                "CMD",
                FontId::proportional(8.5 * u),
                ink,
            );
        }
        crate::keyboard::KeyId::ArrowUp
        | crate::keyboard::KeyId::ArrowDown
        | crate::keyboard::KeyId::ArrowLeft
        | crate::keyboard::KeyId::ArrowRight => {
            let (dx, dy) = match popup.key {
                crate::keyboard::KeyId::ArrowUp => (0.0, -1.0),
                crate::keyboard::KeyId::ArrowDown => (0.0, 1.0),
                crate::keyboard::KeyId::ArrowLeft => (-1.0, 0.0),
                _ => (1.0, 0.0),
            };
            let tip = Pos2::new(c.x + dx * 4.8 * u, c.y + dy * 4.8 * u);
            let tail = Pos2::new(c.x - dx * 4.8 * u, c.y - dy * 4.8 * u);
            let perp_x = -dy * 3.4 * u;
            let perp_y = dx * 3.4 * u;
            let back = Pos2::new(tip.x - dx * 3.4 * u, tip.y - dy * 3.4 * u);
            painter.line_segment([tail, tip], icon_stroke);
            painter.line_segment([tip, Pos2::new(back.x + perp_x, back.y + perp_y)], icon_stroke);
            painter.line_segment([tip, Pos2::new(back.x - perp_x, back.y - perp_y)], icon_stroke);
        }
        _ => {
            let font_size = if popup.label.len() >= 3 {
                9.2 * u
            } else {
                13.0 * u
            };
            painter.text(
                c,
                Align2::CENTER_CENTER,
                popup.label,
                FontId::proportional(font_size),
                ink,
            );
        }
    }
}

// ============================================================================
// Geometry & Anti-Aliased Polygon Helpers
// ============================================================================

fn sample_cubic_bezier(p0: Pos2, p1: Pos2, p2: Pos2, p3: Pos2, steps: usize) -> Vec<Pos2> {
    let mut pts = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let u = 1.0 - t;
        let x = u * u * u * p0.x
            + 3.0 * u * u * t * p1.x
            + 3.0 * u * t * t * p2.x
            + t * t * t * p3.x;
        let y = u * u * u * p0.y
            + 3.0 * u * u * t * p1.y
            + 3.0 * u * t * t * p2.y
            + t * t * t * p3.y;
        pts.push(Pos2::new(x, y));
    }
    pts
}

fn sample_ellipse(center: Pos2, rx: f32, ry: f32, steps: usize) -> Vec<Pos2> {
    let mut pts = Vec::with_capacity(steps);
    for i in 0..steps {
        let a = (i as f32 / steps as f32) * std::f32::consts::TAU;
        pts.push(Pos2::new(center.x + a.cos() * rx, center.y + a.sin() * ry));
    }
    pts
}

fn draw_thick_round_polyline(painter: &Painter, pts: &[Pos2], width: f32, color: Color32) {
    if pts.len() < 2 {
        return;
    }
    let radius = width * 0.5;
    painter.add(Shape::Path(PathShape::line(
        pts.to_vec(),
        Stroke::new(width, color),
    )));
    // Round caps and smooth joints
    for &p in pts {
        painter.circle_filled(p, radius, color);
    }
}

fn fill_polygon_no_fringe(painter: &Painter, points: &[Pos2], color: Color32) {
    if points.len() < 3 {
        return;
    }
    let indices = triangulate_ear_clipping(points);
    if !indices.is_empty() {
        let vertices = points
            .iter()
            .map(|&pos| Vertex {
                pos,
                uv: Pos2::ZERO,
                color,
            })
            .collect();
        let mesh = Mesh {
            indices,
            vertices,
            ..Default::default()
        };
        painter.add(Shape::mesh(mesh));
    }
}

/// Fills an arbitrary simple polygon (convex OR concave!) cleanly using ear-clipping triangulation
/// plus a 1px anti-aliased fringe stroke so edges are buttery smooth.
fn fill_smooth_polygon(painter: &Painter, points: &[Pos2], color: Color32) {
    if points.len() < 3 {
        return;
    }

    // Deduplicate near-identical consecutive points
    let mut clean: Vec<Pos2> = Vec::with_capacity(points.len());
    for &p in points {
        if clean.last().is_none_or(|prev| prev.distance_sq(p) > 0.04) {
            clean.push(p);
        }
    }
    if clean.len() > 2 && clean.first().unwrap().distance_sq(*clean.last().unwrap()) <= 0.04 {
        clean.pop();
    }
    if clean.len() < 3 {
        return;
    }

    let indices = triangulate_ear_clipping(&clean);
    if !indices.is_empty() {
        let vertices = clean
            .iter()
            .map(|&pos| Vertex {
                pos,
                uv: Pos2::ZERO,
                color,
            })
            .collect();
        let mesh = Mesh {
            indices,
            vertices,
            ..Default::default()
        };
        painter.add(Shape::mesh(mesh));
    }

    // Thin matching anti-aliased contour eliminates jagged mesh silhouette edges
    painter.add(Shape::Path(PathShape::closed_line(
        clean,
        Stroke::new(1.0_f32, color),
    )));
}

fn triangulate_ear_clipping(pts: &[Pos2]) -> Vec<u32> {
    let n = pts.len();
    if n < 3 {
        return Vec::new();
    }

    // Signed area (screen coordinates: y down -> CW has positive signed area)
    let mut area = 0.0_f32;
    for i in 0..n {
        let a = pts[i];
        let b = pts[(i + 1) % n];
        area += a.x * b.y - b.x * a.y;
    }
    let ccw_sign = if area >= 0.0 { 1.0_f32 } else { -1.0_f32 };

    let mut remaining: Vec<usize> = (0..n).collect();
    let mut out = Vec::with_capacity((n - 2) * 3);
    let mut guard = n * n;

    while remaining.len() > 3 && guard > 0 {
        guard -= 1;
        let len = remaining.len();
        let mut clipped = false;

        for i in 0..len {
            let prev = remaining[(i + len - 1) % len];
            let curr = remaining[i];
            let next = remaining[(i + 1) % len];

            let a = pts[prev];
            let b = pts[curr];
            let c = pts[next];

            if cross_2d(a, b, c) * ccw_sign <= 1e-4 {
                continue;
            }

            let mut contains_other = false;
            for &other in &remaining {
                if other == prev || other == curr || other == next {
                    continue;
                }
                if point_in_triangle(pts[other], a, b, c, ccw_sign) {
                    contains_other = true;
                    break;
                }
            }

            if !contains_other {
                out.push(prev as u32);
                out.push(curr as u32);
                out.push(next as u32);
                remaining.remove(i);
                clipped = true;
                break;
            }
        }

        if !clipped {
            break;
        }
    }

    if remaining.len() == 3 {
        out.push(remaining[0] as u32);
        out.push(remaining[1] as u32);
        out.push(remaining[2] as u32);
    }

    out
}

#[inline]
fn cross_2d(a: Pos2, b: Pos2, c: Pos2) -> f32 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

#[inline]
fn point_in_triangle(p: Pos2, a: Pos2, b: Pos2, c: Pos2, sign: f32) -> bool {
    let c1 = cross_2d(a, b, p) * sign;
    let c2 = cross_2d(b, c, p) * sign;
    let c3 = cross_2d(c, a, p) * sign;
    c1 >= -1e-4 && c2 >= -1e-4 && c3 >= -1e-4
}

#[inline]
fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let r = (a.r() as f32 + (b.r() as f32 - a.r() as f32) * t).round() as u8;
    let g = (a.g() as f32 + (b.g() as f32 - a.g() as f32) * t).round() as u8;
    let bl = (a.b() as f32 + (b.b() as f32 - a.b() as f32) * t).round() as u8;
    let al = (a.a() as f32 + (b.a() as f32 - a.a() as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(r, g, bl, al)
}

#[inline]
fn with_alpha(c: Color32, alpha: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (c.a() as f32 * alpha).round() as u8)
}

#[inline]
fn ease_out_back(t: f32) -> f32 {
    let c1 = 1.70158_f32;
    let c3 = c1 + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

#[inline]
fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

#[cfg(test)]
mod visual_preview {
    use super::*;
    use crate::input::InputEvent;
    use crate::keyboard::{KeyId, MouseButton};

    fn render_to_ppm(cat: &CatState, path: &str) {
        let ppp = 2.0_f32;
        let w = (CANVAS_W * ppp) as usize;
        let h = (CANVAS_H * ppp) as usize;

        let ctx = egui::Context::default();
        let raw_input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(CANVAS_W, CANVAS_H))),
            ..Default::default()
        };

        let mut font_tex: Option<(usize, usize, Vec<Color32>)> = None;
        let mut last_primitives = Vec::new();

        // Run 2 frames so egui initializes its font atlas cleanly
        for _ in 0..2 {
            let output = ctx.run(raw_input.clone(), |ctx| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ctx, |ui| {
                        render_scene(ui.painter(), ui.max_rect(), cat, true);
                    });
            });

            for (id, delta) in &output.textures_delta.set {
                if *id == egui::TextureId::default() {
                    let egui::ImageData::Color(img) = &delta.image;
                    if let Some([ox, oy]) = delta.pos {
                        if let Some((tw, _th, ref mut data)) = font_tex {
                            for y in 0..img.size[1] {
                                for x in 0..img.size[0] {
                                    let dst = (oy + y) * tw + (ox + x);
                                    if dst < data.len() {
                                        data[dst] = img.pixels[y * img.size[0] + x];
                                    }
                                }
                            }
                        }
                    } else {
                        font_tex = Some((img.size[0], img.size[1], img.pixels.clone()));
                    }
                }
            }
            last_primitives = ctx.tessellate(output.shapes, ppp);
        }

        // Neutral light-gray backdrop so transparent silhouette & white keycaps are clearly visible
        let mut fb = vec![[176u8, 180u8, 184u8]; w * h];

        for prim in &last_primitives {
            if let egui::epaint::Primitive::Mesh(mesh) = &prim.primitive {
                for tri in mesh.indices.as_chunks::<3>().0 {
                    let verts = [
                        mesh.vertices[tri[0] as usize],
                        mesh.vertices[tri[1] as usize],
                        mesh.vertices[tri[2] as usize],
                    ];
                    raster_tri(&mut fb, (w, h), ppp, verts, font_tex.as_ref());
                }
            }
        }

        let mut bytes = format!("P6\n{w} {h}\n255\n").into_bytes();
        bytes.reserve(w * h * 3);
        for px in fb {
            bytes.extend_from_slice(&px);
        }
        std::fs::write(path, bytes).expect("write preview ppm");
    }

    fn raster_tri(
        fb: &mut [[u8; 3]],
        (w, h): (usize, usize),
        ppp: f32,
        [v0, v1, v2]: [Vertex; 3],
        tex: Option<&(usize, usize, Vec<Color32>)>,
    ) {
        let p0 = [v0.pos.x * ppp, v0.pos.y * ppp];
        let p1 = [v1.pos.x * ppp, v1.pos.y * ppp];
        let p2 = [v2.pos.x * ppp, v2.pos.y * ppp];

        let min_x = p0[0].min(p1[0]).min(p2[0]).floor().max(0.0) as usize;
        let max_x = (p0[0].max(p1[0]).max(p2[0]).ceil() as usize).min(w.saturating_sub(1));
        let min_y = p0[1].min(p1[1]).min(p2[1]).floor().max(0.0) as usize;
        let max_y = (p0[1].max(p1[1]).max(p2[1]).ceil() as usize).min(h.saturating_sub(1));

        let area = (p1[0] - p0[0]) * (p2[1] - p0[1]) - (p2[0] - p0[0]) * (p1[1] - p0[1]);
        if area.abs() < 1e-4 {
            return;
        }
        let inv_area = 1.0 / area;

        for y in min_y..=max_y {
            let py = y as f32 + 0.5;
            for x in min_x..=max_x {
                let px = x as f32 + 0.5;
                let w0 = ((p1[0] - px) * (p2[1] - py) - (p2[0] - px) * (p1[1] - py)) * inv_area;
                let w1 = ((p2[0] - px) * (p0[1] - py) - (p0[0] - px) * (p2[1] - py)) * inv_area;
                let w2 = 1.0 - w0 - w1;
                if w0 < -1e-4 || w1 < -1e-4 || w2 < -1e-4 {
                    continue;
                }

                let cr = (v0.color.r() as f32 * w0 + v1.color.r() as f32 * w1 + v2.color.r() as f32 * w2) / 255.0;
                let cg = (v0.color.g() as f32 * w0 + v1.color.g() as f32 * w1 + v2.color.g() as f32 * w2) / 255.0;
                let cb = (v0.color.b() as f32 * w0 + v1.color.b() as f32 * w1 + v2.color.b() as f32 * w2) / 255.0;
                let ca = (v0.color.a() as f32 * w0 + v1.color.a() as f32 * w1 + v2.color.a() as f32 * w2) / 255.0;

                let (mut sr, mut sg, mut sb, mut sa) = (cr, cg, cb, ca);
                if let Some((tw, th, data)) = tex {
                    let u = (v0.uv.x * w0 + v1.uv.x * w1 + v2.uv.x * w2).clamp(0.0, 1.0);
                    let v = (v0.uv.y * w0 + v1.uv.y * w1 + v2.uv.y * w2).clamp(0.0, 1.0);
                    let tx = ((u * (*tw as f32)) as usize).min(tw.saturating_sub(1));
                    let ty = ((v * (*th as f32)) as usize).min(th.saturating_sub(1));
                    let tc = data[ty * tw + tx];
                    sr *= tc.r() as f32 / 255.0;
                    sg *= tc.g() as f32 / 255.0;
                    sb *= tc.b() as f32 / 255.0;
                    sa *= tc.a() as f32 / 255.0;
                }

                let dst = &mut fb[y * w + x];
                // egui vertex colors are premultiplied
                let inv_a = (1.0 - sa).clamp(0.0, 1.0);
                dst[0] = ((sr * 255.0 + dst[0] as f32 * inv_a).clamp(0.0, 255.0)) as u8;
                dst[1] = ((sg * 255.0 + dst[1] as f32 * inv_a).clamp(0.0, 255.0)) as u8;
                dst[2] = ((sb * 255.0 + dst[2] as f32 * inv_a).clamp(0.0, 255.0)) as u8;
            }
        }
    }

    #[test]
    fn render_visual_previews() {
        let Ok(out_dir) = std::env::var("GATIN_PREVIEW_DIR") else {
            return;
        };
        std::fs::create_dir_all(&out_dir).unwrap();

        // 1. Idle sitting pose (like frame_01)
        let mut cat = CatState::new();
        cat.update(0.016);
        render_to_ppm(&cat, &format!("{out_dir}/preview_01_idle.ppm"));

        // 2. Mouse + happy petting squint (like frame_02)
        let mut cat2 = CatState::new();
        cat2.handle_event(InputEvent::MouseMove {
            norm_x: 0.25,
            norm_y: 0.45,
            screen_px: None,
            look_dir: None,
        });
        for _ in 0..30 {
            cat2.register_demo_pet(-0.6);
            cat2.update(0.016);
        }
        render_to_ppm(&cat2, &format!("{out_dir}/preview_02_pet.ppm"));

        // 3. Fast typing with curled paw + floating keycaps (like frame_03)
        let mut cat3 = CatState::new();
        for key in [KeyId::Space, KeyId::F, KeyId::E, KeyId::A, KeyId::I] {
            cat3.handle_event(InputEvent::KeyDown(key));
            cat3.update(0.04);
            cat3.handle_event(InputEvent::KeyUp(key));
            cat3.update(0.04);
        }
        cat3.update(0.10);
        render_to_ppm(&cat3, &format!("{out_dir}/preview_03_typing.ppm"));

        // 4. Mouse click (green!) + right paw pressing A/Shift (like frame_04/05)
        let mut cat4 = CatState::new();
        cat4.handle_event(InputEvent::MouseMove {
            norm_x: 0.15,
            norm_y: 0.25,
            screen_px: None,
            look_dir: Some([-0.9, -0.7]),
        });
        cat4.handle_event(InputEvent::MouseDown(MouseButton::Left));
        cat4.handle_event(InputEvent::KeyDown(KeyId::E));
        cat4.update(0.04);
        cat4.handle_event(InputEvent::KeyUp(KeyId::E));
        cat4.handle_event(InputEvent::KeyDown(KeyId::A));
        cat4.handle_event(InputEvent::KeyDown(KeyId::LeftShift));
        for _ in 0..18 {
            cat4.update(0.016);
        }
        render_to_ppm(&cat4, &format!("{out_dir}/preview_04_gaming.ppm"));

        // 5. Password field privacy mode: both paws covering eyes, zero keycaps shown!
        let mut cat5 = CatState::new();
        cat5.set_password_mode(true);
        cat5.handle_event(InputEvent::KeyDown(KeyId::S));
        cat5.handle_event(InputEvent::KeyDown(KeyId::E));
        for _ in 0..40 {
            cat5.update(0.016);
        }
        render_to_ppm(&cat5, &format!("{out_dir}/preview_05_password.ppm"));
    }
}
