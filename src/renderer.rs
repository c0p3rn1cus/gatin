use crate::cat::{CANVAS_H, CANVAS_W, CatState, KeyPopup, PawMode, PawState};
use crate::keyboard::KeyAccent;
use crate::pet::{PetKind, PetStyle, Species};
use egui::{
    Align2, Color32, CornerRadius, FontId, Mesh, Painter, Pos2, Rect, Shape, Stroke, Vec2,
    epaint::{PathShape, Vertex},
};

// Palette matched closely to the reference video frames (pet coats live in `pet.rs`)
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
    let hc = cat.head_center();
    let head_dx = (canvas_pt[0] - hc[0]) / 47.0;
    let head_dy = (canvas_pt[1] - (hc[1] - 4.0)) / 46.0;
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

    let st = cat.pet.style();

    draw_desk_shadow(painter, &tf, cat);
    // Light coats get a soft contour: paint the whole silhouette slightly fatter underneath first
    if let Some(oc) = st.outline {
        draw_tail(painter, &tf, cat, &st, Some(oc));
        draw_hind_paws(painter, &tf, cat, &st, Some(oc));
        draw_body_and_head(painter, &tf, cat, &st, Some(oc));
    }
    draw_tail(painter, &tf, cat, &st, None);
    draw_hind_paws(painter, &tf, cat, &st, None);
    draw_body_and_head(painter, &tf, cat, &st, None);
    draw_face(painter, &tf, cat, &st);

    // Desk props (mouse + 180-degree rotated 60% mechanical keyboard)
    draw_mouse(painter, &tf, cat);
    draw_keyboard(painter, &tf, cat);

    // Front arms & paws (rendered OVER body and keyboard/mouse — or covering the eyes in password mode!)
    if cat.cover_blend > 0.02 {
        draw_covering_paws(painter, &tf, cat, &st);
    } else {
        draw_paw(painter, &tf, cat, &st, &cat.left_paw, true);
        draw_paw(painter, &tf, cat, &st, &cat.right_paw, false);
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
        tf.pt(80.0, 302.0),
        tf.pt(310.0, 302.0),
        tf.pt(316.0, 310.0),
        tf.pt(74.0, 310.0),
    ];
    fill_polygon_no_fringe(painter, &kbd_shadow, shadow_col);

    let m = cat.mouse_current_pos();
    let shadow = sample_ellipse(tf.pt(m[0] + 1.0, m[1] + 5.5), tf.s(15.0), tf.s(14.0), 24);
    fill_polygon_no_fringe(painter, &shadow, Color32::from_rgba_unmultiplied(0, 0, 0, 30));
}

/// Fills a silhouette piece, or — in the outline pre-pass — strokes its fattened contour.
fn fill_part(
    painter: &Painter,
    tf: &CanvasTransform,
    pts: &[Pos2],
    color: Color32,
    pass: Option<Color32>,
) {
    match pass {
        Some(oc) => {
            painter.add(Shape::Path(PathShape::closed_line(
                pts.to_vec(),
                Stroke::new(tf.s(2.8), oc),
            )));
        }
        None => fill_smooth_polygon(painter, pts, color),
    }
}

/// Maps a point of the seated torso so markings follow the body as it leans and bounces.
fn body_pt(cat: &CatState, x: f32, y: f32) -> [f32; 2] {
    let k = ((213.0 - y) / 81.0).clamp(0.0, 1.0);
    let dy = cat.head_center()[1] - 108.0;
    [x + cat.lean_x * (0.14 + 0.78 * k), y + dy * k]
}

fn body_ellipse(tf: &CanvasTransform, cat: &CatState, x: f32, y: f32, rx: f32, ry: f32) -> Vec<Pos2> {
    let c = body_pt(cat, x, y);
    sample_ellipse(tf.pt(c[0], c[1]), tf.s(rx), tf.s(ry), 22)
}

/// Head outline offset for angle `a` (0 = viewer's right, PI/2 = chin).
fn head_outline(species: Species, a: f32) -> (f32, f32) {
    match species {
        Species::Cat => {
            let cheek = if a.sin() > 0.0 { 1.0 + 0.055 * a.sin() } else { 1.0 };
            (a.cos() * 43.5 * cheek, a.sin() * 36.0)
        }
        Species::Dog => {
            let cheek = if a.sin() > 0.0 { 1.0 + 0.03 * a.sin() } else { 1.0 };
            (a.cos() * 40.5 * cheek, a.sin() * 37.0)
        }
        Species::Dino => {
            // Round skull that widens into a chunky snout
            let (jaw, ry) = if a.sin() > 0.0 { (1.0 + 0.16 * a.sin(), 40.0) } else { (1.0, 36.5) };
            (a.cos() * 39.5 * jaw, a.sin() * ry)
        }
        Species::Alien => {
            // Big-brained dome tapering to a small pointed chin
            let taper = 1.0 - 0.4 * a.sin().max(0.0);
            (a.cos() * 45.0 * taper, a.sin() * 38.0)
        }
        Species::Capybara => {
            // Tall blocky head: flat narrow crown widening into a big square muzzle
            let sq = |v: f32| v.signum() * v.abs().powf(0.74);
            (sq(a.cos()) * (39.0 + 3.5 * a.sin()), sq(a.sin()) * 40.0)
        }
    }
}

fn draw_tail(
    painter: &Painter,
    tf: &CanvasTransform,
    cat: &CatState,
    st: &PetStyle,
    pass: Option<Color32>,
) {
    let grow = if pass.is_some() { tf.s(2.6) } else { 0.0 };
    let col = pass.unwrap_or(st.tail);

    match st.species {
        // Capybaras barely have a tail at all
        Species::Capybara | Species::Alien => {}
        Species::Dino => {
            // Heavy tail tapering to a point, with a row of plates along the top
            let sway = (cat.tail_phase * 0.8).sin();
            let sway2 = (cat.tail_phase * 0.8 - 1.0).sin();
            let p0 = tf.pt(210.0 + cat.lean_x * 0.15, 208.0);
            let p1 = tf.pt(250.0, 216.0 + sway * 2.0);
            let p2 = tf.pt(272.0 + sway2 * 3.0, 205.0 + sway2 * 3.0);
            let p3 = tf.pt(296.0 + sway2 * 5.0, 182.0 + sway2 * 8.0);
            let pts = sample_cubic_bezier(p0, p1, p2, p3, 24);
            let (w0, w1) = (tf.s(31.0), tf.s(5.0));
            if pass.is_none() {
                for i in [4usize, 8, 12, 16, 20] {
                    let t = i as f32 / 24.0;
                    let dir = (pts[i + 1] - pts[i - 1]).normalized();
                    let up = Vec2::new(dir.y, -dir.x);
                    let hw = limb_half_width(t, w0, w1);
                    let size = tf.s(8.5 - t * 4.5);
                    let base = pts[i] + up * (hw - tf.s(1.5));
                    let plate = vec![base - dir * size, base + up * size * 1.25, base + dir * size];
                    fill_smooth_polygon(painter, &plate, st.accent);
                }
            }
            draw_tapered_limb(painter, &pts, w0 + grow, w1 + grow, col);
        }
        Species::Dog => {
            // Short upright tail with a fast, wide, happy wag
            let ph = cat.tail_phase * 2.4;
            let wag = ph.sin();
            let wag2 = (ph - 0.7).sin();
            let amp = 0.55 + cat.typing_energy.min(1.0) * 0.3 + cat.pet_level * 0.45;
            let p0 = tf.pt(216.0 + cat.lean_x * 0.15, 207.0);
            let p1 = tf.pt(240.0, 208.0);
            let p2 = tf.pt(246.0 + wag * 6.0 * amp, 188.0);
            let p3 = tf.pt(251.0 + wag2 * 15.0 * amp, 165.0 + wag2.abs() * 3.0);
            let pts = sample_cubic_bezier(p0, p1, p2, p3, 20);
            draw_tapered_limb(painter, &pts, tf.s(12.0) + grow, tf.s(8.0) + grow, col);
            if let (None, Some(tip)) = (pass, st.tail_tip) {
                draw_limb_range(painter, &pts, tf.s(12.0), tf.s(8.0), 16, 20, tip, true);
            }
        }
        Species::Cat => {
            // A wave travels from the base to the tip, so the tip always swings the most
            let wag = cat.tail_phase.sin();
            let wag2 = (cat.tail_phase - 0.9).sin();
            let wag3 = (cat.tail_phase - 1.8).sin();

            let p0 = tf.pt(214.0 + cat.lean_x * 0.15, 210.0);
            let p1 = tf.pt(247.0, 214.0 + wag * 3.0);
            let p2 = tf.pt(252.0 + wag2 * 5.0, 177.0 + wag2 * 4.0);
            let p3 = tf.pt(284.0 + wag3 * 7.0, 174.0 + wag3 * 7.0);

            let pts = sample_cubic_bezier(p0, p1, p2, p3, 26);
            // Slightly plumper toward the blunt rounded tip, like the reference
            let (w0, w1) = (tf.s(13.0), tf.s(15.5));
            draw_tapered_limb(painter, &pts, w0 + grow, w1 + grow, col);
            if pass.is_none() {
                if cat.pet == PetKind::Bengal {
                    for (i0, i1) in [(8, 10), (13, 15), (18, 20)] {
                        draw_limb_range(painter, &pts, w0, w1, i0, i1, st.marking, false);
                    }
                }
                if let Some(tip) = st.tail_tip {
                    draw_limb_range(painter, &pts, w0, w1, 22, 26, tip, true);
                }
            }
        }
    }
}

fn draw_hind_paws(
    painter: &Painter,
    tf: &CanvasTransform,
    cat: &CatState,
    st: &PetStyle,
    pass: Option<Color32>,
) {
    // Cute small rounded hind paws peeking out at the bottom corners of the seated body
    let lx = 148.0 + cat.lean_x * 0.12;
    let rx = 216.0 + cat.lean_x * 0.12;
    let y = 222.5;

    for (cx, side) in [(lx, -1.0_f32), (rx, 1.0_f32)] {
        let ellipse = sample_ellipse(tf.pt(cx, y), tf.s(14.0), tf.s(11.0), 20);
        fill_part(painter, tf, &ellipse, st.hind_paw, pass);
        if pass.is_some() {
            continue;
        }

        // Delicate rim highlight along the lower outer edge
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
            Stroke::new(tf.s(1.3), with_alpha(st.rim, 0.86)),
        )));
    }
}

fn draw_body_and_head(
    painter: &Painter,
    tf: &CanvasTransform,
    cat: &CatState,
    st: &PetStyle,
    pass: Option<Color32>,
) {
    let lx = cat.lean_x;
    let head_center = cat.head_center();
    // Vertical travel of the head (bounce, stretch, breathing) drags the shoulders along
    let dy = head_center[1] - 108.0;
    let br = cat.breath;
    // A key-strike dip squashes the chest outward a touch (squash & stretch)
    let squash = cat.typing_bob.max(0.0) * 0.45;
    let belly = br * 1.1 + squash;

    // 1. Seated pear-shaped torso (bottom anchored at x=182, top leans smoothly with head!)
    let mut torso = Vec::with_capacity(48);
    let left_neck = tf.pt(152.0 + lx * 0.92, 132.0 + dy);
    let right_neck = tf.pt(212.0 + lx * 0.92, 132.0 + dy);
    let left_hip = tf.pt(133.0 + lx * 0.14, 213.0);
    let right_hip = tf.pt(231.0 + lx * 0.14, 213.0);

    torso.extend(sample_cubic_bezier(
        left_neck,
        tf.pt(137.0 + lx * 0.62 - belly * 0.5, 158.0 + dy * 0.6),
        tf.pt(117.0 + lx * 0.22 - belly, 190.0),
        left_hip,
        14,
    ));
    torso.extend(sample_cubic_bezier(
        left_hip,
        tf.pt(146.0 + lx * 0.1, 231.0),
        tf.pt(218.0 + lx * 0.1, 231.0),
        right_hip,
        12,
    ));
    torso.extend(sample_cubic_bezier(
        right_hip,
        tf.pt(247.0 + lx * 0.22 + belly, 190.0),
        tf.pt(227.0 + lx * 0.62 + belly * 0.5, 158.0 + dy * 0.6),
        right_neck,
        14,
    ));
    fill_part(painter, tf, &torso, st.body, pass);
    if pass.is_none() {
        draw_body_markings(painter, tf, cat, st);
    }

    // 2. Head + ears (rotated smoothly by head_tilt!)
    let tilt = cat.head_tilt;
    let rot = |dx: f32, dy: f32| -> Pos2 {
        let c = tilt.cos();
        let s = tilt.sin();
        tf.pt(
            head_center[0] + dx * c - dy * s,
            head_center[1] + dx * s + dy * c,
        )
    };

    // Capybara and dinosaur turn their long snouts sideways (toward the mouse)
    if let Some(outline) = profile_head_outline(st.species) {
        draw_profile_head(painter, tf, cat, st, pass, &rot, &outline);
        if pass.is_none() {
            draw_head_markings(painter, tf, cat, st, &rot);
        }
        return;
    }

    let head_pts: Vec<Pos2> = (0..44)
        .map(|i| {
            let a = (i as f32 / 44.0) * std::f32::consts::TAU;
            let (dx, dy) = head_outline(st.species, a);
            rot(dx, dy)
        })
        .collect();

    if st.species == Species::Alien {
        // Two springy antennae with glowing tips that bob as it types
        for (idx, side) in [(0usize, -1.0_f32), (1usize, 1.0_f32)] {
            let sway = (cat.time * 2.3 + side).sin() * 1.6 + cat.ear_twitch[idx] * 4.0 * side
                - cat.lean_x * 0.25;
            let bob = cat.typing_bob * 1.4 - cat.pet_level * 3.0;
            let base = (side * 15.0, -33.0);
            let tip = (side * 25.0 + sway, -58.0 + bob);
            let ctrl = (side * 15.0 + sway * 0.3, -50.0 + bob * 0.5);
            let stalk: Vec<Pos2> = (0..=10)
                .map(|i| {
                    let t = i as f32 / 10.0;
                    let u = 1.0 - t;
                    rot(
                        u * u * base.0 + 2.0 * u * t * ctrl.0 + t * t * tip.0,
                        u * u * base.1 + 2.0 * u * t * ctrl.1 + t * t * tip.1,
                    )
                })
                .collect();
            let tip_pos = rot(tip.0, tip.1);
            match pass {
                Some(oc) => {
                    painter.add(Shape::Path(PathShape::line(stalk, Stroke::new(tf.s(6.0), oc))));
                    painter.circle_filled(tip_pos, tf.s(6.6), oc);
                }
                None => {
                    painter.add(Shape::Path(PathShape::line(
                        stalk,
                        Stroke::new(tf.s(3.4), st.body),
                    )));
                    let pulse = 0.5 + 0.5 * (cat.time * 3.0 + side).sin();
                    let glow = cat.typing_energy.min(1.0).max(cat.mouse_click_glow);
                    painter.circle_filled(
                        tip_pos,
                        tf.s(7.5 + glow * 2.5),
                        with_alpha(st.accent, 0.16 + 0.1 * pulse + 0.25 * glow),
                    );
                    painter.circle_filled(tip_pos, tf.s(5.0), st.accent);
                    painter.circle_filled(
                        tip_pos + Vec2::new(-tf.s(1.5), -tf.s(1.6)),
                        tf.s(1.5),
                        Color32::from_rgba_unmultiplied(255, 255, 255, 190),
                    );
                }
            }
        }
    }
    fill_part(painter, tf, &head_pts, st.body, pass);

    for (idx, side) in [(0usize, -1.0_f32), (1usize, 1.0_f32)] {
        let tw = cat.ear_twitch[idx];
        // Flick = small rotation around the ear root, plus petting/drowsy flattening
        let flat = cat.pet_level * 0.16 + cat.drowsy * 0.08;
        let ang = side * (tw * 0.2 + flat);
        let (pc, ps) = (ang.cos(), ang.sin());
        let (px, py) = match st.species {
            Species::Cat => (22.0, -22.0),
            Species::Dog => (30.0, -27.0),
            Species::Capybara | Species::Dino | Species::Alien => (0.0, 0.0),
        };
        // Mirrored for the right ear, rotated about the root pivot
        let ear = |dx: f32, dy: f32| -> Pos2 {
            let (x, y) = (dx - px, dy - py);
            let ex = px + x * pc - y * ps * side;
            let ey = py + x * ps * side + y * pc;
            rot(side * ex, ey)
        };

        let mut pts = Vec::new();
        match st.species {
            Species::Cat => {
                // Short, wide-set triangular ears with soft tips
                pts.extend(sample_cubic_bezier(
                    ear(39.0, -11.0),
                    ear(41.0, -27.0),
                    ear(38.0, -44.5),
                    ear(30.5, -47.0),
                    10,
                ));
                pts.extend(sample_cubic_bezier(
                    ear(30.5, -47.0),
                    ear(23.5, -49.0),
                    ear(14.0, -39.0),
                    ear(6.0, -30.0),
                    10,
                ));
                pts.push(ear(21.0, -15.0));
            }
            Species::Dog => {
                // Floppy ears hanging down the sides of the head
                pts.extend(sample_cubic_bezier(
                    ear(20.0, -33.0),
                    ear(34.0, -40.0),
                    ear(50.0, -32.0),
                    ear(51.0, -12.0),
                    10,
                ));
                pts.extend(sample_cubic_bezier(
                    ear(51.0, -12.0),
                    ear(52.0, 4.0),
                    ear(45.0, 13.0),
                    ear(39.0, 8.0),
                    10,
                ));
                pts.extend(sample_cubic_bezier(
                    ear(39.0, 8.0),
                    ear(35.0, 0.0),
                    ear(33.0, -14.0),
                    ear(26.0, -22.0),
                    8,
                ));
            }
            // Drawn by `draw_profile_head`
            Species::Capybara | Species::Dino => continue,
            // Antennae instead of ears (drawn before the head)
            Species::Alien => continue,
        }
        if side > 0.0 {
            pts.reverse();
        }
        fill_part(painter, tf, &pts, st.ears[idx], pass);
    }

    if pass.is_none() {
        draw_head_markings(painter, tf, cat, st, &rot);
    }
}

/// Side-view head outline (snout pointing to the viewer's left) as closed polygon offsets
/// from the head center, for the long-snouted pets.
fn profile_head_outline(species: Species) -> Option<Vec<(f32, f32)>> {
    type Seg = [(f32, f32); 4];
    let segs: [Seg; 7] = match species {
        // Round skull, long snout with a nostril bump, deep jaw
        Species::Dino => [
            [(22.0, -34.0), (4.0, -40.0), (-10.0, -32.0), (-14.0, -20.0)],
            [(-14.0, -20.0), (-22.0, -10.0), (-36.0, -15.0), (-46.0, -9.0)],
            [(-46.0, -9.0), (-52.0, -8.0), (-55.0, 0.0), (-53.0, 8.0)],
            [(-53.0, 8.0), (-53.0, 17.0), (-50.0, 23.0), (-43.0, 25.0)],
            [(-43.0, 25.0), (-33.0, 29.0), (-20.0, 31.0), (-6.0, 30.0)],
            [(-6.0, 30.0), (10.0, 30.0), (30.0, 28.0), (40.0, 6.0)],
            [(40.0, 6.0), (42.0, -12.0), (36.0, -29.0), (22.0, -34.0)],
        ],
        // One blunt brick: flat crown, square muzzle as deep as the skull
        Species::Capybara => [
            [(28.0, -31.0), (10.0, -35.0), (-22.0, -30.0), (-40.0, -23.0)],
            [(-40.0, -23.0), (-48.0, -20.0), (-52.0, -12.0), (-52.0, -3.0)],
            [(-52.0, -3.0), (-52.0, 10.0), (-51.0, 20.0), (-44.0, 23.0)],
            [(-44.0, 23.0), (-36.0, 27.0), (-20.0, 31.0), (-6.0, 30.0)],
            [(-6.0, 30.0), (10.0, 30.0), (30.0, 28.0), (40.0, 6.0)],
            [(40.0, 6.0), (42.0, -6.0), (41.0, -16.0), (38.0, -23.0)],
            [(38.0, -23.0), (36.0, -28.0), (33.0, -30.0), (28.0, -31.0)],
        ],
        _ => return None,
    };
    let mut out = Vec::with_capacity(70);
    for [p0, p1, p2, p3] in segs {
        for i in 0..10 {
            let t = i as f32 / 10.0;
            let u = 1.0 - t;
            let b = |a: f32, b: f32, c: f32, d: f32| {
                u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * d
            };
            out.push((b(p0.0, p1.0, p2.0, p3.0), b(p0.1, p1.1, p2.1, p3.1)));
        }
    }
    Some(out)
}

fn draw_profile_head(
    painter: &Painter,
    tf: &CanvasTransform,
    cat: &CatState,
    st: &PetStyle,
    pass: Option<Color32>,
    rot: &dyn Fn(f32, f32) -> Pos2,
    outline: &[(f32, f32)],
) {
    let oval = |cx: f32, cy: f32, rx: f32, ry: f32, lean: f32| -> Vec<Pos2> {
        (0..18)
            .map(|i| {
                let a = (i as f32 / 18.0) * std::f32::consts::TAU;
                let (x, y) = (a.cos() * rx, a.sin() * ry);
                rot(cx + x * lean.cos() - y * lean.sin(), cy + x * lean.sin() + y * lean.cos())
            })
            .collect()
    };
    let flick = cat.ear_twitch[1] * 2.5 - cat.pet_level * 2.0;

    match st.species {
        Species::Dino if pass.is_none() => {
            // Plates running over the crown and down the back of the skull
            let n = outline.len();
            for (idx, h) in [(63usize, 9.0_f32), (67, 12.0), (1, 12.0), (5, 9.0)] {
                let p = outline[idx % n];
                let a = outline[(idx + n - 1) % n];
                let b = outline[(idx + 1) % n];
                let (tx, ty) = (b.0 - a.0, b.1 - a.1);
                let len = (tx * tx + ty * ty).sqrt().max(0.001);
                let (tx, ty) = (tx / len, ty / len);
                // Outward normal: the one pointing away from the skull center
                let (mut nx, mut ny) = (-ty, tx);
                if nx * (p.0 - 8.0) + ny * (p.1 + 4.0) < 0.0 {
                    nx = -nx;
                    ny = -ny;
                }
                let plate = vec![
                    rot(p.0 - tx * 7.0 - nx * 3.0, p.1 - ty * 7.0 - ny * 3.0),
                    rot(p.0 + nx * h, p.1 + ny * h),
                    rot(p.0 + tx * 7.0 - nx * 3.0, p.1 + ty * 7.0 - ny * 3.0),
                ];
                fill_smooth_polygon(painter, &plate, st.accent);
            }
        }
        Species::Capybara => {
            // Far ear peeks out from behind the crown
            let far = oval(11.0, -34.0 - flick * 0.5, 5.5, 6.5, -0.2);
            fill_part(painter, tf, &far, lerp_color(st.body, st.rim, 0.35), pass);
        }
        _ => {}
    }

    let head: Vec<Pos2> = outline.iter().map(|&(x, y)| rot(x, y)).collect();
    fill_part(painter, tf, &head, st.body, pass);

    if st.species == Species::Capybara {
        // Near ear: a small rounded flap set far back on the head
        let near = oval(27.0, -29.0 - flick, 7.0, 8.5, 0.35);
        fill_part(painter, tf, &near, lerp_color(st.body, st.rim, 0.18), pass);
        if pass.is_none() {
            let inner = oval(27.5, -28.0 - flick, 3.6, 5.0, 0.35);
            fill_smooth_polygon(painter, &inner, st.rim);
        }
    }
}

/// Coat markings on the torso (drawn before the head so bibs tuck under the chin).
fn draw_body_markings(painter: &Painter, tf: &CanvasTransform, cat: &CatState, st: &PetStyle) {
    match cat.pet {
        PetKind::BlackCat | PetKind::Siamese => {}
        PetKind::Alien => {
            let belly = body_ellipse(tf, cat, 182.0, 182.0, 17.0, 27.0);
            fill_smooth_polygon(painter, &belly, st.marking);
        }
        PetKind::Capybara => {
            // Shaggy chest fur ticks
            let stroke = Stroke::new(tf.s(1.3), with_alpha(st.rim, 0.75));
            for (x, y) in [(175.0, 156.0), (190.0, 162.0), (181.0, 171.0), (187.0, 181.0)] {
                let a = body_pt(cat, x - 3.2, y);
                let b = body_pt(cat, x, y + 3.4);
                let c = body_pt(cat, x + 3.2, y);
                painter.add(Shape::Path(PathShape::line(
                    vec![tf.pt(a[0], a[1]), tf.pt(b[0], b[1]), tf.pt(c[0], c[1])],
                    stroke,
                )));
            }
        }
        PetKind::Dino => {
            // Darker scales on the flanks
            let scale_col = lerp_color(st.body, st.rim, 0.45);
            for (x, y, r) in [
                (147.0, 174.0, 4.2),
                (139.0, 193.0, 3.6),
                (152.0, 205.0, 3.2),
                (218.0, 174.0, 4.2),
                (227.0, 193.0, 3.6),
                (213.0, 205.0, 3.2),
            ] {
                let spot = body_ellipse(tf, cat, x, y, r, r * 0.85);
                fill_smooth_polygon(painter, &spot, scale_col);
            }
            // Pale belly plate with segment creases
            let belly = body_ellipse(tf, cat, 182.0, 181.0, 19.0, 31.0);
            fill_smooth_polygon(painter, &belly, st.marking);
            let crease = Stroke::new(tf.s(1.2), lerp_color(st.marking, st.rim, 0.4));
            for (y, hw) in [(162.0_f32, 14.0_f32), (173.0, 17.5), (184.0, 18.5), (195.0, 17.0)] {
                let pts: Vec<Pos2> = (0..=8)
                    .map(|i| {
                        let t = i as f32 / 8.0 * 2.0 - 1.0;
                        let p = body_pt(cat, 182.0 + t * hw, y + (1.0 - t * t) * 2.2);
                        tf.pt(p[0], p[1])
                    })
                    .collect();
                painter.add(Shape::Path(PathShape::line(pts, crease)));
            }
        }
        PetKind::Tuxedo => {
            let bib = body_ellipse(tf, cat, 182.0, 172.0, 16.0, 27.0);
            fill_smooth_polygon(painter, &bib, st.marking);
        }
        PetKind::Dog => {
            let chest = body_ellipse(tf, cat, 182.0, 170.0, 13.0, 23.0);
            fill_smooth_polygon(painter, &chest, st.marking);
        }
        PetKind::Bengal => {
            let chest = body_ellipse(tf, cat, 182.0, 171.0, 12.0, 22.0);
            fill_smooth_polygon(painter, &chest, st.accent);
            // Rosette-like spots scattered over both flanks
            let spots = [
                (148.0, 164.0, 4.6, 3.4),
                (138.0, 184.0, 4.2, 3.6),
                (152.0, 199.0, 4.8, 3.2),
                (160.0, 181.0, 3.6, 3.0),
                (143.0, 209.0, 3.8, 2.8),
                (216.0, 164.0, 4.6, 3.4),
                (227.0, 185.0, 4.2, 3.6),
                (212.0, 200.0, 4.8, 3.2),
                (205.0, 182.0, 3.6, 3.0),
                (222.0, 209.0, 3.8, 2.8),
            ];
            for (x, y, rx, ry) in spots {
                let spot = body_ellipse(tf, cat, x, y, rx, ry);
                fill_smooth_polygon(painter, &spot, st.marking);
            }
        }
        PetKind::Calico => {
            let orange = body_ellipse(tf, cat, 215.0, 183.0, 14.0, 18.0);
            fill_smooth_polygon(painter, &orange, st.marking);
            let black = body_ellipse(tf, cat, 148.0, 199.0, 12.5, 10.5);
            fill_smooth_polygon(painter, &black, st.accent);
        }
    }
}

/// Coat markings fixed to the head (muzzles, blazes, patches, the capybara's tangerine).
fn draw_head_markings(
    painter: &Painter,
    tf: &CanvasTransform,
    cat: &CatState,
    st: &PetStyle,
    rot: &dyn Fn(f32, f32) -> Pos2,
) {
    let oval = |cx: f32, cy: f32, rx: f32, ry: f32| -> Vec<Pos2> {
        (0..22)
            .map(|i| {
                let a = (i as f32 / 22.0) * std::f32::consts::TAU;
                rot(cx + a.cos() * rx, cy + a.sin() * ry)
            })
            .collect()
    };
    // Patch hugging the head outline between two angles, closed by an inner path
    let head_patch = |a0: f32, a1: f32, inner: &[(f32, f32)]| -> Vec<Pos2> {
        let mut pts: Vec<Pos2> = (0..=14)
            .map(|i| {
                let a = (a0 + (a1 - a0) * (i as f32 / 14.0)).to_radians();
                let (dx, dy) = head_outline(st.species, a);
                rot(dx * 0.995, dy * 0.995)
            })
            .collect();
        pts.extend(inner.iter().map(|&(x, y)| rot(x, y)));
        pts
    };

    match cat.pet {
        PetKind::BlackCat => {}
        PetKind::Alien => {
            let spot_col = lerp_color(st.body, st.rim, 0.4);
            for (x, y, r) in [(-10.0, -27.0, 3.2), (2.0, -30.0, 2.2), (13.0, -25.0, 2.6)] {
                fill_smooth_polygon(painter, &oval(x, y, r, r * 0.8), spot_col);
            }
        }
        PetKind::Dino => {
            let scale_col = lerp_color(st.body, st.rim, 0.45);
            for (x, y, r) in [(22.0, -20.0, 3.4), (31.0, -8.0, 2.8), (12.0, -27.0, 2.4), (27.0, 6.0, 2.4)] {
                fill_smooth_polygon(painter, &oval(x, y, r, r * 0.85), scale_col);
            }
        }
        PetKind::Siamese => {
            // Dark mask over the eyes and muzzle
            fill_smooth_polygon(painter, &oval(0.0, 5.0, 31.0, 22.5), st.marking);
        }
        PetKind::Tuxedo => {
            fill_smooth_polygon(painter, &oval(0.0, 16.0, 15.5, 11.5), st.marking);
            let blaze = vec![rot(-2.6, -24.0), rot(2.6, -24.0), rot(6.5, 9.0), rot(-6.5, 9.0)];
            fill_smooth_polygon(painter, &blaze, st.marking);
        }
        PetKind::Bengal => {
            fill_smooth_polygon(painter, &oval(0.0, 15.5, 14.5, 10.5), st.accent);
            let stroke = Stroke::new(tf.s(2.4), st.marking);
            // Tabby "M" on the forehead + a stripe on each cheek
            for (x0, y0, x1, y1) in [
                (-9.0, -31.0, -6.5, -19.0),
                (0.0, -33.0, 0.0, -20.0),
                (9.0, -31.0, 6.5, -19.0),
                (-41.0, -2.0, -32.0, 1.0),
                (41.0, -2.0, 32.0, 1.0),
                (-41.0, 6.0, -33.0, 7.5),
                (41.0, 6.0, 33.0, 7.5),
            ] {
                painter.line_segment([rot(x0, y0), rot(x1, y1)], stroke);
            }
        }
        PetKind::Calico => {
            // Orange over the left eye, black over the right, white blaze down the middle
            let orange = head_patch(
                172.0,
                263.0,
                &[(-5.0, -22.0), (-3.5, -6.0), (-9.0, 6.0), (-24.0, 11.0)],
            );
            fill_smooth_polygon(painter, &orange, st.marking);
            let black = head_patch(
                277.0,
                368.0,
                &[(24.0, 11.0), (9.0, 6.0), (3.5, -6.0), (5.0, -22.0)],
            );
            fill_smooth_polygon(painter, &black, st.accent);
        }
        PetKind::Dog => {
            fill_smooth_polygon(painter, &oval(0.0, 16.0, 17.5, 13.5), st.marking);
        }
        PetKind::Capybara => {
            // The famous tangerine balanced on its flat head
            let wobble = (cat.time * 2.2).sin() * 0.6 + cat.typing_bob * 0.25;
            let (tx, ty) = (-6.0 + wobble, -39.5);
            fill_smooth_polygon(painter, &oval(tx, ty, 8.6, 7.8), st.accent);
            let leaf = vec![
                rot(tx + 0.5, ty - 7.5),
                rot(tx + 4.0, ty - 12.5),
                rot(tx + 8.0, ty - 10.5),
                rot(tx + 4.5, ty - 7.0),
            ];
            fill_smooth_polygon(painter, &leaf, Color32::from_rgb(92, 150, 60));
            painter.line_segment(
                [rot(tx, ty - 7.0), rot(tx - 0.4, ty - 9.5)],
                Stroke::new(tf.s(1.4), Color32::from_rgb(96, 70, 40)),
            );
        }
    }
}

fn draw_face(painter: &Painter, tf: &CanvasTransform, cat: &CatState, st: &PetStyle) {
    let head_center = cat.head_center();
    let tilt = cat.head_tilt;
    // Slight pseudo-3D yaw shift of face features toward gaze
    // (side-view heads barely shift: their features sit on a snout, not a flat face)
    let profile = matches!(st.species, Species::Capybara | Species::Dino);
    let shift = if profile { 0.25 } else { 1.0 };
    let face_shift_x = cat.gaze[0] * 5.0 * shift;
    let face_shift_y = cat.gaze[1] * 2.8 * shift;

    let rot = |dx: f32, dy: f32| -> Pos2 {
        let c = tilt.cos();
        let s = tilt.sin();
        let x = dx + face_shift_x;
        let y = dy + face_shift_y;
        tf.pt(head_center[0] + x * c - y * s, head_center[1] + x * s + y * c)
    };
    let oval = |cx: f32, cy: f32, rx: f32, ry: f32, n: usize| -> Vec<Pos2> {
        (0..n)
            .map(|i| {
                let a = (i as f32 / n as f32) * std::f32::consts::TAU;
                rot(cx + a.cos() * rx, cy + a.sin() * ry)
            })
            .collect()
    };

    // Eye openness: 1.0 = wide open, 0.0 = closed blink, ~0.16 = happy petted slits
    let squint = cat.pet_level;
    let open_factor = ((1.0 - squint * 0.84) * (1.0 - cat.blink)).max(0.0);

    // (eye index, side sign, x, y) — side-view heads show a single eye
    let eyes: Vec<(usize, f32, f32, f32)> = match st.species {
        Species::Cat => vec![(0, -1.0, -17.5, -3.5), (1, 1.0, 17.5, -3.5)],
        Species::Dog => vec![(0, -1.0, -15.5, -6.0), (1, 1.0, 15.5, -6.0)],
        Species::Capybara => vec![(0, -1.0, 3.0, -12.0)],
        Species::Dino => vec![(0, -1.0, -6.0, -13.0)],
        Species::Alien => vec![(0, -1.0, -17.5, -3.0), (1, 1.0, 17.5, -3.0)],
    };

    for (idx, side, ex, ey) in eyes {
        let closed_col = st.eye_closed[idx];

        if st.species == Species::Alien {
            // Huge glossy almond eyes, slanted up at the outer corners
            let slant = side * 0.42;
            let (sc, ss) = (slant.cos(), slant.sin());
            let cx = ex + cat.gaze[0] * 1.4;
            let cy = ey + cat.gaze[1] * 1.0;
            let almond = |rx: f32, ry: f32, ox: f32, oy: f32, n: usize| -> Vec<Pos2> {
                (0..n)
                    .map(|i| {
                        let a = (i as f32 / n as f32) * std::f32::consts::TAU;
                        let (x, y) = (ox + a.cos() * rx, oy + a.sin() * ry);
                        rot(cx + x * sc - y * ss, cy + x * ss + y * sc)
                    })
                    .collect()
            };
            if open_factor < 0.12 || squint > 0.45 {
                let lift = if squint > 0.45 { 3.2 } else { 0.5 };
                let arc: Vec<Pos2> = (0..=8)
                    .map(|i| {
                        let t = i as f32 / 8.0 * 2.0 - 1.0;
                        let (x, y) = (t * 12.0, 1.0 - (1.0 - t * t) * lift);
                        rot(cx + x * sc - y * ss, cy + x * ss + y * sc)
                    })
                    .collect();
                painter.add(Shape::Path(PathShape::line(arc, Stroke::new(tf.s(2.2), closed_col))));
            } else {
                let open = open_factor * (1.0 - cat.drowsy * 0.45);
                fill_smooth_polygon(painter, &almond(13.5, 9.6 * open, 0.0, 0.0, 26), st.eye);
                if open > 0.45 {
                    let gx = cat.gaze[0] * 3.0;
                    let gy = cat.gaze[1] * 2.0;
                    let white = Color32::from_rgb(250, 252, 255);
                    fill_smooth_polygon(painter, &almond(3.4, 2.6 * open, -4.0 + gx, -3.0 * open + gy, 12), white);
                    fill_smooth_polygon(
                        painter,
                        &almond(1.5, 1.2, 4.5 + gx, 2.6 * open + gy, 8),
                        Color32::from_rgba_unmultiplied(250, 252, 255, 150),
                    );
                }
            }
            continue;
        }

        if matches!(st.species, Species::Dog | Species::Capybara) {
            // Friendly round button eyes with a little glint
            let r = match st.species {
                Species::Dog => 6.4,
                _ => 5.0,
            };
            if open_factor < 0.12 || squint > 0.45 {
                // Blink, or a happy upward arc while being petted
                let lift = if squint > 0.45 { 2.6 } else { 0.4 };
                let arc: Vec<Pos2> = (0..=8)
                    .map(|i| {
                        let t = i as f32 / 8.0 * 2.0 - 1.0;
                        rot(ex + t * (r + 0.8), ey + 1.0 - (1.0 - t * t) * lift)
                    })
                    .collect();
                painter.add(Shape::Path(PathShape::line(
                    arc,
                    Stroke::new(tf.s(2.0), closed_col),
                )));
            } else {
                let open = open_factor * (1.0 - cat.drowsy * 0.45);
                let gx = cat.gaze[0] * 1.6;
                let gy = cat.gaze[1] * 1.3;
                fill_smooth_polygon(painter, &oval(ex + gx, ey + gy, r, r * open, 20), st.eye);
                if open > 0.5 {
                    let glint = oval(
                        ex + gx - 1.8 + cat.gaze[0] * 0.8,
                        ey + gy - 2.0 * open,
                        r * 0.3,
                        r * 0.3,
                        10,
                    );
                    fill_smooth_polygon(painter, &glint, Color32::from_rgb(250, 250, 246));
                }
                if st.species == Species::Capybara {
                    // Permanently unbothered: a heavy lid rests over the top of each eye
                    let (cx, cy) = (ex + gx, ey + gy);
                    let lid_y = cy - r * 0.2;
                    let lid = vec![
                        rot(cx - r - 2.0, cy - r - 2.0),
                        rot(cx + r + 2.0, cy - r - 2.0),
                        rot(cx + r + 2.0, lid_y),
                        rot(cx - r - 2.0, lid_y),
                    ];
                    fill_smooth_polygon(painter, &lid, st.lids[idx]);
                    painter.line_segment(
                        [
                            rot(cx - r - 2.0, lid_y),
                            rot(cx + r + 2.0, lid_y),
                        ],
                        Stroke::new(tf.s(1.5), closed_col),
                    );
                }
            }
            continue;
        }

        if open_factor < 0.06 {
            // Closed blink line
            let hw = if st.species == Species::Dino { 8.0 } else { 10.5 };
            let p0 = rot(ex - hw, ey + 1.0);
            let p1 = rot(ex + hw, ey + 1.0);
            painter.line_segment([p0, p1], Stroke::new(tf.s(1.7), closed_col));
        } else if squint > 0.45 && cat.blink < 0.5 {
            // Blissful slits, gently arched like a smile
            let half_w = if st.species == Species::Dino { 9.0 } else { 12.0 };
            let half_h = (1.9 + (1.0 - squint) * 7.5) * (1.0 - cat.blink);
            let mut slit = Vec::with_capacity(20);
            for i in 0..20 {
                let a = (i as f32 / 20.0) * std::f32::consts::TAU;
                let arc = (a.cos() * a.cos()) * 1.6 * squint;
                slit.push(rot(ex + a.cos() * half_w, ey + 1.0 + a.sin() * half_h + arc));
            }
            fill_smooth_polygon(painter, &slit, closed_col);
        } else {
            // Big luminous eyes with tracking vertical oval pupils
            let dino = st.species == Species::Dino;
            let rx = if dino { 9.2 } else { 12.8 };
            let ry = if dino { 10.2 } else { 11.8 } * open_factor;
            let ring = if st.dark_face { 0.9 } else { 1.5 };

            fill_smooth_polygon(painter, &oval(ex, ey, rx + ring, ry + ring, 28), st.eye_ring);
            fill_smooth_polygon(painter, &oval(ex, ey, rx, ry, 28), st.eye);

            // Pupil: dilates when excited, clamped cleanly inside the iris
            let (pw, ph) = if dino { (4.6, 5.2) } else { (4.3, 7.4) };
            let prx = (pw * cat.pupil).min(rx - 3.0);
            let pry = (ph * cat.pupil * open_factor).min(ry - 1.2).max(1.5);
            let max_dx = (rx - prx - 0.8).max(1.0);
            let max_dy = (ry - pry - 0.6).max(0.8);
            let mut gx = cat.gaze[0] * max_dx;
            let mut gy = cat.gaze[1] * max_dy;
            let norm_dist = ((gx / max_dx).powi(2) + (gy / max_dy).powi(2)).sqrt();
            if norm_dist > 1.0 {
                gx /= norm_dist;
                gy /= norm_dist;
            }
            fill_smooth_polygon(painter, &oval(ex + gx, ey + gy, prx, pry, 20), st.pupil);

            // Sleepy eyelid: a fur-colored lid simply slides down over the eye
            if cat.drowsy > 0.02 {
                let lid_y = ey - ry - 1.5 + (2.0 * ry + 1.5) * 0.48 * cat.drowsy;
                let lid = vec![
                    rot(ex - rx - 2.0, ey - ry - 3.0),
                    rot(ex + rx + 2.0, ey - ry - 3.0),
                    rot(ex + rx + 2.0, lid_y - side * 0.8),
                    rot(ex - rx - 2.0, lid_y + side * 0.8),
                ];
                fill_smooth_polygon(painter, &lid, st.lids[idx]);
            }
        }
    }

    // Nose + mouth (the all-black cat keeps its minimalist face)
    let quad = |p0: (f32, f32), p1: (f32, f32), p2: (f32, f32)| -> Vec<Pos2> {
        (0..=8)
            .map(|i| {
                let t = i as f32 / 8.0;
                let u = 1.0 - t;
                rot(
                    u * u * p0.0 + 2.0 * u * t * p1.0 + t * t * p2.0,
                    u * u * p0.1 + 2.0 * u * t * p1.1 + t * t * p2.1,
                )
            })
            .collect()
    };
    match st.species {
        Species::Cat => {
            if let Some(mouth) = st.mouth {
                let stroke = Stroke::new(tf.s(1.2), mouth);
                painter.line_segment([rot(0.0, 12.0), rot(0.0, 14.8)], stroke);
                for side in [-1.0_f32, 1.0] {
                    let curve = quad((0.0, 14.8), (side * 2.6, 18.2), (side * 5.6, 15.2));
                    painter.add(Shape::Path(PathShape::line(curve, stroke)));
                }
            }
            if let Some(nose) = st.nose {
                let tri = vec![rot(-3.4, 8.6), rot(3.4, 8.6), rot(0.0, 12.4)];
                fill_smooth_polygon(painter, &tri, nose);
            }
        }
        Species::Dog => {
            let happy = cat.typing_energy.min(1.0).max(cat.pet_level);
            // Panting tongue pops out when the dog is excited or being petted
            if happy > 0.25 {
                let len = 3.0 + 5.5 * ((happy - 0.25) / 0.75).min(1.0)
                    + (cat.time * 9.0).sin() * 0.6;
                fill_smooth_polygon(painter, &oval(0.0, 19.0 + len * 0.5, 4.2, len, 14), st.accent);
            }
            if let Some(mouth) = st.mouth {
                let stroke = Stroke::new(tf.s(1.4), mouth);
                painter.line_segment([rot(0.0, 11.0), rot(0.0, 16.0)], stroke);
                for side in [-1.0_f32, 1.0] {
                    let curve = quad((0.0, 16.0), (side * 4.0, 21.5), (side * 9.0, 17.0));
                    painter.add(Shape::Path(PathShape::line(curve, stroke)));
                }
            }
            if let Some(nose) = st.nose {
                fill_smooth_polygon(painter, &oval(0.0, 8.4, 6.2, 4.4, 18), nose);
                let shine = oval(-1.8, 7.0, 1.7, 1.0, 10);
                fill_smooth_polygon(painter, &shine, Color32::from_rgb(120, 112, 112));
            }
        }
        Species::Alien => {
            if let Some(nose) = st.nose {
                for side in [-1.0_f32, 1.0] {
                    fill_smooth_polygon(painter, &oval(side * 2.2, 13.5, 0.9, 1.4, 8), nose);
                }
            }
            if let Some(mouth) = st.mouth {
                let smile = quad((-4.5, 20.0), (0.0, 23.5), (4.5, 20.0));
                painter.add(Shape::Path(PathShape::line(smile, Stroke::new(tf.s(1.4), mouth))));
            }
        }
        Species::Dino => {
            if let Some(nose) = st.nose {
                fill_smooth_polygon(painter, &oval(-45.0, -3.0, 1.9, 2.6, 10), nose);
            }
            if let Some(mouth) = st.mouth {
                // Long toothy grin running the whole length of the snout
                let grin = quad((-52.0, 10.0), (-30.0, 20.0), (-3.0, 13.0));
                for i in 1..8usize {
                    let along = (grin[i + 1] - grin[i - 1]).normalized();
                    let mut down = Vec2::new(-along.y, along.x);
                    if down.y < 0.0 {
                        down = -down;
                    }
                    let tooth = vec![
                        grin[i] - along * tf.s(2.2),
                        grin[i] + along * tf.s(2.2),
                        grin[i] + down * tf.s(4.2),
                    ];
                    fill_smooth_polygon(painter, &tooth, Color32::from_rgb(252, 252, 244));
                }
                let stroke = Stroke::new(tf.s(1.7), mouth);
                painter.add(Shape::Path(PathShape::line(grin, stroke)));
                // Smile curl at the corner of the mouth
                let curl = quad((-3.0, 13.0), (1.5, 11.5), (2.5, 7.0));
                painter.add(Shape::Path(PathShape::line(curl, stroke)));
            }
            let blush = oval(9.0, 7.0, 5.0, 3.2, 12);
            fill_smooth_polygon(painter, &blush, Color32::from_rgba_unmultiplied(240, 130, 120, 110));
        }
        Species::Capybara => {
            if let Some(nose) = st.nose {
                // Nostril high on the blunt front of the muzzle
                fill_smooth_polygon(painter, &oval(-46.0, -10.0, 2.3, 3.6, 12), nose);
            }
            if let Some(mouth) = st.mouth {
                let stroke = Stroke::new(tf.s(1.5), mouth);
                let line = quad((-51.5, 11.0), (-42.0, 17.5), (-29.0, 14.0));
                painter.add(Shape::Path(PathShape::line(line, stroke)));
            }
            let blush = oval(-12.0, 6.0, 5.0, 3.0, 12);
            fill_smooth_polygon(painter, &blush, Color32::from_rgba_unmultiplied(226, 120, 96, 80));
        }
    }

    // 3 short delicate whiskers on each cheek, fanning out past the cheek edge
    if let Some(whisker) = st.whisker {
        let whisker_stroke = Stroke::new(tf.s(1.2), with_alpha(whisker, 0.9));
        let quiver = (cat.time * 31.0).sin() * 0.5 * cat.typing_energy.min(1.0);
        for side in [-1.0_f32, 1.0_f32] {
            let base_x = side * 24.0;
            let specs = [
                (10.5_f32, 0.0_f32, 11.5_f32, -1.6_f32),
                (14.2, 0.6, 12.5, 1.4),
                (17.8, 0.0, 11.0, 4.6),
            ];
            for (by, dx0, len, dy1) in specs {
                let p0 = rot(base_x + side * dx0, by);
                let p1 = rot(base_x + side * (dx0 + len), by + dy1 + quiver);
                painter.line_segment([p0, p1], whisker_stroke);
            }
        }
    }
}

fn draw_mouse(painter: &Painter, tf: &CanvasTransform, cat: &CatState) {
    let m = cat.mouse_current_pos();
    let tilt = cat.mouse_tilt;
    let glow = cat.mouse_click_glow;
    let press = cat.mouse_press;

    let rot = |dx: f32, dy: f32, y_off: f32| -> Pos2 {
        let c = tilt.cos();
        let s = tilt.sin();
        tf.pt(m[0] + dx * c - dy * s, m[1] + y_off + dx * s + dy * c)
    };

    // Small egg-shaped mouse seen from the front: palm rest under the paw (top), buttons and
    // wheel pointing down toward the viewer — the pet faces us, so its mouse does too
    const RX: f32 = 13.4;
    const RY: f32 = 15.4;
    let egg = |a: f32, grow: f32| -> (f32, f32) {
        let narrow = 1.0 - 0.13 * a.sin().max(0.0);
        (a.cos() * (RX + grow) * narrow, a.sin() * (RY + grow))
    };
    let sample_egg = |a0: f32, a1: f32, n: usize, grow: f32, y_off: f32| -> Vec<Pos2> {
        (0..n)
            .map(|i| {
                let a = a0 + (a1 - a0) * (i as f32 / n as f32);
                let (x, y) = egg(a, grow);
                rot(x, y, y_off)
            })
            .collect()
    };
    let tau = std::f32::consts::TAU;
    let pi = std::f32::consts::PI;

    // 3D bottom lip (gets thinner while a button is held down)
    let lip = sample_egg(0.0, tau, 28, 0.3, 2.8);
    fill_smooth_polygon(painter, &lip, Color32::from_rgb(196, 198, 192));

    // Main shell sinks a hair on click
    let sink = press * 1.3;
    let whole_lime = if cat.mouse_click_side < 0 { glow } else { glow * 0.18 };
    let body = sample_egg(0.0, tau, 28, 0.0, sink);
    fill_smooth_polygon(painter, &body, lerp_color(MOUSE_WHITE, MOUSE_LIME, whole_lime));

    // Right click lights only the pet's right button — on the viewer's left, like the keyboard
    if cat.mouse_click_side > 0 && glow > 0.02 {
        let mut half = sample_egg(pi * 0.5, pi * 1.02, 12, -0.4, sink);
        half.push(rot(0.0, -0.8, sink));
        fill_smooth_polygon(painter, &half, lerp_color(MOUSE_WHITE, MOUSE_LIME, glow));
    }

    painter.add(Shape::Path(PathShape::closed_line(
        body,
        Stroke::new(tf.s(1.5), MOUSE_OUTLINE),
    )));

    // Button split seam
    let seam = Color32::from_rgba_unmultiplied(32, 32, 36, 70);
    painter.line_segment(
        [rot(0.0, RY - 1.0, sink), rot(0.0, 11.0, sink)],
        Stroke::new(tf.s(1.0), seam),
    );
    painter.line_segment(
        [rot(0.0, 4.0, sink), rot(0.0, -0.5, sink)],
        Stroke::new(tf.s(1.0), seam),
    );

    // Amber scroll wheel (flashes lime on middle click)
    let wheel_col = if cat.mouse_click_side == 0 {
        lerp_color(MOUSE_WHEEL, KEY_LIME_SIDE, glow)
    } else if cat.mouse_click_side < 0 && glow > 0.3 {
        Color32::from_rgb(42, 48, 22)
    } else {
        MOUSE_WHEEL
    };
    painter.line_segment(
        [rot(0.0, 10.0, sink), rot(0.0, 5.0, sink)],
        Stroke::new(tf.s(2.4), wheel_col),
    );
}

fn draw_keyboard(painter: &Painter, tf: &CanvasTransform, cat: &CatState) {
    // 1. Dark mechanical keyboard case: rounded top plate over a chunky front lip
    let plate = [[91.0, 229.5], [299.0, 229.5], [313.5, 297.5], [76.5, 297.5]];
    let lip: Vec<[f32; 2]> = plate.iter().map(|p| [p[0], p[1] + 6.5]).collect();
    fill_smooth_polygon(painter, &rounded_polygon(tf, &lip, 6.0), KBD_CASE_SHADOW);
    fill_smooth_polygon(painter, &rounded_polygon(tf, &plate, 6.0), KBD_CASE);

    // 2. Individual 3D perspective keycaps (5 rows, 180-degree rotated to face the cat!)
    let pad_u = 0.0045;
    let pad_v = 0.022;

    for spec in &cat.layout.keys {
        let heat = cat.key_heat.get(&spec.key).copied().unwrap_or(0.0);
        let press = cat.key_press.get(&spec.key).copied().unwrap_or(0.0);
        let depress = press * 2.3;
        // Ease the lime so it lingers bright, then melts away
        let tint = heat * heat * (3.0 - 2.0 * heat);

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
        let top_col = lerp_color(base_top_col, KEY_LIME, tint);
        let side_col = lerp_color(base_side_col, KEY_LIME_SIDE, tint);

        let tl = CatState::keyboard_uv_to_canvas(u0, v0);
        let tr = CatState::keyboard_uv_to_canvas(u1, v0);
        let br = CatState::keyboard_uv_to_canvas(u1, v1);
        let bl = CatState::keyboard_uv_to_canvas(u0, v1);

        // Keycap front skirt (shrinks as the cap travels down)
        let skirt = vec![
            tf.pt(bl[0], bl[1] - 0.5 + depress * 0.5),
            tf.pt(br[0], br[1] - 0.5 + depress * 0.5),
            tf.pt(br[0], br[1] + 2.5),
            tf.pt(bl[0], bl[1] + 2.5),
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

fn draw_covering_paws(painter: &Painter, tf: &CanvasTransform, cat: &CatState, st: &PetStyle) {
    let t = ease_out_cubic(cat.cover_blend.clamp(0.0, 1.0));
    let lx = cat.lean_x;
    let bob = cat.typing_bob;
    let tilt = cat.head_tilt;
    let head_center = cat.head_center();
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
        let eye_target = match st.species {
            // Side-view heads have one eye: both paws pile up over it
            Species::Dino => rot_head(-6.0 + side * 9.0, -11.0 + shy_press * side),
            Species::Capybara => rot_head(3.0 + side * 9.0, -10.0 + shy_press * side),
            _ => rot_head(side * 16.5, -3.5 + shy_press * side),
        };
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
        draw_tapered_limb(painter, &arm_pts, tf.s(19.0), tf.s(15.0), st.body);

        // Delicate pale rim outlines along the raised forearm so the silhouette reads crisply against the black torso/head!
        let rim_alpha = (t * 215.0).round() as u8;
        let rim_col = Color32::from_rgba_unmultiplied(st.rim.r(), st.rim.g(), st.rim.b(), rim_alpha);
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
        fill_smooth_polygon(painter, &paw_pts, st.paw);

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

/// Half-width of a front limb at parameter t in [0..1] (shoulder -> wrist), in canvas units.
fn limb_half_width(t: f32, w0: f32, w1: f32) -> f32 {
    let e = t * t * (3.0 - 2.0 * t);
    0.5 * (w0 + (w1 - w0) * e)
}

/// One continuous front limb for every pose (resting leg, tucked paw, reaching arm), so the
/// cat flows between poses instead of popping from one drawing to another.
fn draw_paw(
    painter: &Painter,
    tf: &CanvasTransform,
    cat: &CatState,
    st: &PetStyle,
    paw: &PawState,
    is_left: bool,
) {
    let side = if is_left { -1.0_f32 } else { 1.0_f32 };
    let lx = cat.lean_x;
    let dy = cat.head_center()[1] - 108.0;
    let rest = {
        let r = paw.rest_blend.clamp(0.0, 1.0);
        r * r * (3.0 - 2.0 * r)
    };
    let curl = paw.curl_blend.clamp(0.0, 1.0);

    // Shoulder slides from the chest (reaching) down to the leg root (sitting)
    let sh_reach = [182.0 + lx * 0.72 + side * 23.0, 159.0 + dy * 0.5];
    let sh_rest = [182.0 + lx * 0.42 + side * 19.0, 172.0];
    let shoulder = [
        sh_reach[0] + (sh_rest[0] - sh_reach[0]) * rest,
        sh_reach[1] + (sh_rest[1] - sh_reach[1]) * rest,
    ];
    let tip = [paw.pos[0], paw.pos[1] - paw.total_lift()];

    // Natural slight outward elbow bend that straightens out for short limbs
    let len = ((tip[0] - shoulder[0]).powi(2) + (tip[1] - shoulder[1]).powi(2)).sqrt();
    let bend = (1.0 - rest) * (len / 70.0).min(1.0);
    let mid_x = (shoulder[0] + tip[0]) * 0.5 + side * 6.5 * bend;
    let mid_y = (shoulder[1] + tip[1]) * 0.5 - 3.5 * bend;
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

    // Thick at the shoulder, slimmer at the wrist, ending in a plump round paw
    let w0 = 21.0 - 4.0 * rest;
    let w1 = 14.5;
    draw_tapered_limb(painter, &arm_pts, tf.s(w0), tf.s(w1), st.body);
    if cat.pet == PetKind::Siamese {
        // Dark "gloves" fading up the forelegs
        draw_limb_range(painter, &arm_pts, tf.s(w0), tf.s(w1), 13, 18, st.paw, true);
    }
    if cat.pet == PetKind::Bengal {
        for (i0, i1) in [(8, 9), (12, 13)] {
            draw_limb_range(painter, &arm_pts, tf.s(w0), tf.s(w1), i0, i1, st.marking, false);
        }
    }

    // Paw squashes flat for an instant when it slaps a key or clicks the mouse
    let slap = if paw.mode == PawMode::PressingKey {
        (1.0 - paw.total_lift() / 5.0).clamp(0.0, 1.0) * 0.6
    } else if paw.mode == PawMode::OnMouse {
        cat.mouse_press
    } else {
        0.0
    };
    let paw_pts = sample_ellipse(
        tf.pt(tip[0], tip[1] + slap * 0.8),
        tf.s(9.6 + slap * 1.1),
        tf.s(8.8 - slap * 1.0),
        22,
    );
    fill_smooth_polygon(painter, &paw_pts, st.paw);

    if st.species == Species::Dino {
        // Three little claws pointing wherever the arm is heading
        let n = arm_pts.len();
        let d = arm_pts[n - 1] - arm_pts[n - 3];
        let heading = if d.length_sq() > 1e-4 { d.y.atan2(d.x) } else { std::f32::consts::FRAC_PI_2 };
        for off in [-0.62_f32, 0.0, 0.62] {
            let a = heading + off;
            let dir = Vec2::new(a.cos(), a.sin());
            let perp = Vec2::new(-dir.y, dir.x);
            let base = tf.pt(tip[0], tip[1] + slap * 0.8) + dir * tf.s(7.2);
            let claw = vec![
                base - perp * tf.s(2.0),
                base + dir * tf.s(5.4),
                base + perp * tf.s(2.0),
            ];
            fill_smooth_polygon(painter, &claw, Color32::from_rgb(244, 238, 206));
        }
    }

    // Light paws get a contour around the toes so they read over white keys and the mouse
    if let Some(oc) = st.paw_outline {
        let n = arm_pts.len();
        let d = arm_pts[n - 1] - arm_pts[n - 3];
        let heading = if d.length_sq() > 1e-4 { d.y.atan2(d.x) } else { std::f32::consts::FRAC_PI_2 };
        let arc: Vec<Pos2> = (0..=14)
            .map(|i| {
                let a = heading + (i as f32 / 14.0 * 2.0 - 1.0) * 1.95;
                tf.pt(
                    tip[0] + a.cos() * (9.6 + slap * 1.1),
                    tip[1] + slap * 0.8 + a.sin() * (8.8 - slap),
                )
            })
            .collect();
        painter.add(Shape::Path(PathShape::line(arc, Stroke::new(tf.s(1.3), oc))));
    }

    // Rim lines ONLY where the limb overlaps the torso (never floating in air!)
    draw_arm_rim_highlights(painter, tf, cat, st, &arm_pts, w0, w1);

    // Tucked paw: cute U-shaped contour under the paw resting on the chest
    if curl > 0.04 {
        let mut arc = Vec::with_capacity(14);
        for i in 0..=12 {
            let t = i as f32 / 12.0;
            let ang = std::f32::consts::PI * (0.06 + t * 0.88);
            arc.push(tf.pt(tip[0] + ang.cos() * 9.9, tip[1] + ang.sin() * 9.1));
        }
        painter.add(Shape::Path(PathShape::line(
            arc,
            Stroke::new(
                tf.s(1.4),
                with_alpha(st.rim, curl * 0.9),
            ),
        )));
    }
}

fn draw_arm_rim_highlights(
    painter: &Painter,
    tf: &CanvasTransform,
    cat: &CatState,
    st: &PetStyle,
    pts: &[Pos2],
    w0: f32,
    w1: f32,
) {
    if pts.len() < 4 {
        return;
    }
    let rim_stroke = Stroke::new(tf.s(1.25), with_alpha(st.rim, 0.78));
    let n = pts.len();
    let bx = 182.0 + cat.lean_x * 0.4;
    let hc = cat.head_center();

    // 1 = rim line over the torso, 2 = outer contour for light coats, 0 = nothing
    let classify = |c: [f32; 2]| -> u8 {
        // Strictly inside the torso so it separates limb from body cleanly, and off the
        // upper chest so it starts mid-body like the reference
        let inside_torso = ((c[0] - bx) / 45.0).powi(2) + ((c[1] - 188.0) / 47.0).powi(2) <= 1.0
            && c[1] > 181.0
            && c[1] < 229.0;
        if inside_torso {
            return 1;
        }
        if st.outline.is_some() {
            let in_body = ((c[0] - bx) / 53.0).powi(2) + ((c[1] - 184.0) / 56.0).powi(2) <= 1.0;
            let in_head = ((c[0] - hc[0]) / 45.0).powi(2) + ((c[1] - hc[1]) / 39.0).powi(2) <= 1.0;
            if !in_body && !in_head {
                return 2;
            }
        }
        0
    };

    for side_sign in [-1.0_f32, 1.0_f32] {
        let mut seg: Vec<Pos2> = Vec::new();
        let mut kind = 0u8;
        let flush = |seg: &mut Vec<Pos2>, kind: u8| {
            if seg.len() >= 2 {
                let stroke = match (kind, st.outline) {
                    (2, Some(oc)) => Stroke::new(tf.s(1.3), oc),
                    _ => rim_stroke,
                };
                painter.add(Shape::Path(PathShape::line(std::mem::take(seg), stroke)));
            } else {
                seg.clear();
            }
        };
        for i in 1..n {
            let t = i as f32 / (n - 1) as f32;
            let dir = (pts[(i + 1).min(n - 1)] - pts[i - 1]).normalized();
            let normal = Vec2::new(-dir.y, dir.x) * side_sign;
            let edge_pt = pts[i] + normal * tf.s(limb_half_width(t, w0, w1) + 0.2);
            let k = classify(tf.to_canvas(edge_pt));
            if k != kind {
                flush(&mut seg, kind);
                kind = k;
            }
            if k != 0 {
                seg.push(edge_pt);
            }
        }
        flush(&mut seg, kind);
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

/// Draws a smooth limb whose thickness tapers from `w0` at the first point to `w1` at the last.
fn draw_tapered_limb(painter: &Painter, pts: &[Pos2], w0: f32, w1: f32, color: Color32) {
    if pts.len() < 2 {
        return;
    }
    draw_limb_range(painter, pts, w0, w1, 0, pts.len() - 1, color, true);
}

/// Draws the slice `i0..=i1` of a tapered limb (used for tail tips, rings and leg stripes).
/// With `round` off the slice ends in straight cuts across the limb.
#[allow(clippy::too_many_arguments)]
fn draw_limb_range(
    painter: &Painter,
    pts: &[Pos2],
    w0: f32,
    w1: f32,
    i0: usize,
    i1: usize,
    color: Color32,
    round: bool,
) {
    let n = pts.len();
    if n < 2 {
        return;
    }
    let i1 = i1.min(n - 1);
    let radius = |i: usize| limb_half_width(i as f32 / (n - 1) as f32, w0, w1);
    for i in i0..=i1 {
        if round {
            painter.circle_filled(pts[i], radius(i), color);
        }
        if i == i0 {
            continue;
        }
        let d = pts[i] - pts[i - 1];
        if d.length_sq() < 1e-4 {
            continue;
        }
        let nrm = Vec2::new(-d.y, d.x).normalized();
        let (r0, r1) = (radius(i - 1), radius(i));
        painter.add(Shape::convex_polygon(
            vec![
                pts[i - 1] + nrm * r0,
                pts[i] + nrm * r1,
                pts[i] - nrm * r1,
                pts[i - 1] - nrm * r0,
            ],
            color,
            Stroke::NONE,
        ));
    }
}

/// Rounds the corners of a convex polygon given in canvas units.
fn rounded_polygon(tf: &CanvasTransform, corners: &[[f32; 2]], radius: f32) -> Vec<Pos2> {
    let n = corners.len();
    let mut out = Vec::with_capacity(n * 6);
    for i in 0..n {
        let prev = corners[(i + n - 1) % n];
        let cur = corners[i];
        let next = corners[(i + 1) % n];
        let toward = |p: [f32; 2]| -> [f32; 2] {
            let (dx, dy) = (p[0] - cur[0], p[1] - cur[1]);
            let len = (dx * dx + dy * dy).sqrt().max(0.001);
            let r = radius.min(len * 0.45);
            [cur[0] + dx / len * r, cur[1] + dy / len * r]
        };
        let a = toward(prev);
        let b = toward(next);
        for k in 0..=5 {
            let t = k as f32 / 5.0;
            let u = 1.0 - t;
            out.push(tf.pt(
                u * u * a[0] + 2.0 * u * t * cur[0] + t * t * b[0],
                u * u * a[1] + 2.0 * u * t * cur[1] + t * t * b[1],
            ));
        }
    }
    out
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

        // Optional frame strip of a key strike + mouse grab, to eyeball the motion
        if std::env::var("GATIN_PREVIEW_SEQ").is_ok() {
            let mut c = CatState::new();
            c.update(0.016);
            c.handle_event(InputEvent::KeyDown(KeyId::P));
            for f in 0..24 {
                if f == 6 {
                    c.handle_event(InputEvent::KeyUp(KeyId::P));
                    c.handle_event(InputEvent::KeyDown(KeyId::Q));
                }
                if f == 10 {
                    c.handle_event(InputEvent::KeyUp(KeyId::Q));
                }
                if f == 14 {
                    c.handle_event(InputEvent::MouseDelta { dx: 60.0, dy: -40.0 });
                }
                c.update(1.0 / 60.0);
                render_to_ppm(&c, &format!("{out_dir}/seq_{f:02}.ppm"));
            }
        }

        // One sheet per pet: idle, mouse + petting, typing, password
        for kind in PetKind::ALL {
            let name = format!("{kind:?}").to_lowercase();
            let mut a = CatState::new();
            a.pet = kind;
            a.update(0.016);
            render_to_ppm(&a, &format!("{out_dir}/pet_{name}_1.ppm"));

            let mut b = CatState::new();
            b.pet = kind;
            b.handle_event(InputEvent::MouseMove {
                norm_x: 0.25,
                norm_y: 0.45,
                screen_px: None,
                look_dir: None,
            });
            b.handle_event(InputEvent::MouseDown(MouseButton::Right));
            for _ in 0..30 {
                b.register_demo_pet(-0.6);
                b.update(0.016);
            }
            render_to_ppm(&b, &format!("{out_dir}/pet_{name}_2.ppm"));

            let mut c = CatState::new();
            c.pet = kind;
            for key in [KeyId::Space, KeyId::F, KeyId::P] {
                c.handle_event(InputEvent::KeyDown(key));
                c.update(0.04);
                c.handle_event(InputEvent::KeyUp(key));
                c.update(0.04);
            }
            c.handle_event(InputEvent::KeyDown(KeyId::A));
            for _ in 0..6 {
                c.update(0.016);
            }
            render_to_ppm(&c, &format!("{out_dir}/pet_{name}_3.ppm"));

            let mut d = CatState::new();
            d.pet = kind;
            d.set_password_mode(true);
            for _ in 0..40 {
                d.update(0.016);
            }
            render_to_ppm(&d, &format!("{out_dir}/pet_{name}_4.ppm"));
        }

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
