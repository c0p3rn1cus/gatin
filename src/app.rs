use crate::autostart;
use crate::cat::{CANVAS_H, CANVAS_W, CatState};
use crate::config::{AppConfig, MAX_SCALE, MIN_SCALE};
use crate::demo::DemoDriver;
use crate::input::{GlobalInput, InputEvent, egui_key_to_key_id};
use crate::keyboard::MouseButton;
use crate::pet::PetKind;
use crate::privacy::PrivacyMonitor;
use crate::renderer::{CanvasTransform, is_on_cat_body, is_on_cat_head, render_scene};
use egui::{
    Align2, CentralPanel, Color32, Context, CursorIcon, FontId, Frame, Pos2, Rect, Sense, Stroke,
    ViewportCommand, WindowLevel, vec2, viewport::ResizeDirection,
};
use std::time::{Duration, Instant};

pub struct GatinApp {
    cat: CatState,
    input: GlobalInput,
    privacy: PrivacyMonitor,
    manual_privacy: bool,
    config: AppConfig,
    demo_mode: bool,
    demo: DemoDriver,
    last_frame: Instant,
    last_hover_pos: Option<Pos2>,
    last_saved_pos: Option<[f32; 2]>,
    last_window_size: Option<[f32; 2]>,
    programmatic_resize_until: Option<Instant>,
    config_dirty: bool,
    last_save: Instant,
    /// Seconds the pet-picker bar stays visible after switching pets.
    pet_toast: f32,
    /// Smooth fade of the hover controls (pet-picker bar + resize grip).
    picker_alpha: f32,
    /// Seconds the scale percentage badge stays visible after resizing.
    size_toast: f32,
    /// Whether OS autostart is supported and currently enabled.
    autostart_supported: bool,
    autostart_enabled: bool,
    /// Whether the first-run autostart card is currently visible.
    show_autostart_prompt: bool,
    /// Checkbox state inside the first-run card.
    prompt_autostart_checked: bool,
    /// Whether the user wants to copy the binary into a permanent per-user folder.
    prompt_copy_permanent: bool,
    /// Whether a permanent per-user install copy is available and not yet in use.
    can_copy_permanent: bool,
    /// Last autostart error message (shown in the card or context menu).
    autostart_error: Option<String>,
    /// Brief status banner shown at the bottom of the window after autostart changes.
    status_toast: Option<(String, f32)>,
    /// Manual drag-resize fallback for platforms without OS drag_resize_window (macOS).
    #[cfg(target_os = "macos")]
    manual_resize_dir: Option<ResizeDirection>,
}

impl GatinApp {
    pub fn new(
        config: AppConfig,
        demo_mode: bool,
        input: GlobalInput,
        privacy: PrivacyMonitor,
    ) -> Self {
        let mut cat = CatState::new();
        cat.pet = config.pet;
        let auto = autostart::inspect();
        let show_autostart_prompt = auto.supported && !config.autostart_prompted && !auto.enabled;
        let can_copy_permanent = auto.can_copy_to_permanent();
        Self {
            cat,
            input,
            privacy,
            manual_privacy: false,
            last_saved_pos: config.window_pos,
            last_window_size: None,
            programmatic_resize_until: None,
            config,
            demo_mode,
            demo: DemoDriver::new(),
            last_frame: Instant::now(),
            last_hover_pos: None,
            config_dirty: false,
            last_save: Instant::now(),
            // Show the picker for a moment on launch so it is discoverable
            pet_toast: 3.5,
            picker_alpha: 0.0,
            size_toast: 0.0,
            autostart_supported: auto.supported,
            autostart_enabled: auto.enabled,
            show_autostart_prompt,
            prompt_autostart_checked: true,
            prompt_copy_permanent: can_copy_permanent && auto.running_from_transient,
            can_copy_permanent,
            autostart_error: None,
            status_toast: None,
            #[cfg(target_os = "macos")]
            manual_resize_dir: None,
        }
    }

    fn set_pet(&mut self, pet: PetKind) {
        if self.cat.pet != pet {
            self.cat.pet = pet;
            self.config.pet = pet;
            self.config_dirty = true;
        }
        self.pet_toast = 1.8;
    }

    /// Hover bar above the pet: ‹ name › plus one dot per pet. Click an arrow or a dot to switch.
    fn pet_picker_bar(&mut self, ui: &mut egui::Ui, tf: &CanvasTransform, alpha: f32) {
        let a = |v: f32| (v * alpha).round().clamp(0.0, 255.0) as u8;
        let interactive = alpha > 0.5;
        let sense = if interactive { Sense::click() } else { Sense::hover() };
        let bar_y = 22.0;
        let mut chosen: Option<PetKind> = None;

        // Name pill
        let pill = Rect::from_center_size(tf.pt(180.0, bar_y), vec2(tf.s(104.0), tf.s(22.0)));
        let painter = ui.painter().clone();
        painter.rect_filled(pill, tf.s(11.0), Color32::from_rgba_unmultiplied(22, 22, 26, a(215.0)));
        painter.text(
            pill.center(),
            Align2::CENTER_CENTER,
            self.cat.pet.label(),
            FontId::proportional(tf.s(12.5)),
            Color32::from_rgba_unmultiplied(244, 244, 240, a(255.0)),
        );

        // Previous / next arrows
        for (x, dir) in [(113.0_f32, -1.0_f32), (247.0, 1.0)] {
            let center = tf.pt(x, bar_y);
            let rect = Rect::from_center_size(center, vec2(tf.s(26.0), tf.s(26.0)));
            let resp = ui.allocate_rect(rect, sense);
            let hot = interactive && resp.hovered();
            if hot {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            let bg = if hot {
                Color32::from_rgba_unmultiplied(206, 240, 74, a(255.0))
            } else {
                Color32::from_rgba_unmultiplied(22, 22, 26, a(215.0))
            };
            let fg = if hot {
                Color32::from_rgba_unmultiplied(22, 22, 26, a(255.0))
            } else {
                Color32::from_rgba_unmultiplied(244, 244, 240, a(255.0))
            };
            painter.circle_filled(center, tf.s(11.0), bg);
            let u = tf.s(1.0);
            painter.add(egui::Shape::convex_polygon(
                vec![
                    center + vec2(dir * 4.2 * u, 0.0),
                    center + vec2(-dir * 2.6 * u, -4.6 * u),
                    center + vec2(-dir * 2.6 * u, 4.6 * u),
                ],
                fg,
                Stroke::NONE,
            ));
            if interactive && resp.clicked() {
                chosen = Some(if dir < 0.0 { self.cat.pet.prev() } else { self.cat.pet.next() });
            }
        }

        // One coat-colored dot per pet
        let count = PetKind::VISIBLE.len() as f32;
        for (i, kind) in PetKind::VISIBLE.into_iter().enumerate() {
            let x = 180.0 + (i as f32 - (count - 1.0) * 0.5) * 15.0;
            let center = tf.pt(x, bar_y + 20.0);
            let rect = Rect::from_center_size(center, vec2(tf.s(14.0), tf.s(14.0)));
            let resp = ui.allocate_rect(rect, sense).on_hover_text(kind.label());
            let selected = kind == self.cat.pet;
            let hot = interactive && resp.hovered();
            if hot {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            let [c0, c1] = kind.swatch();
            let r = tf.s(if selected || hot { 5.4 } else { 4.2 });
            let fade = |c: Color32| Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a(255.0));
            painter.circle_filled(center, r + tf.s(1.4), Color32::from_rgba_unmultiplied(22, 22, 26, a(215.0)));
            painter.circle_filled(center, r, fade(c0));
            painter.circle_filled(center, r * 0.45, fade(c1));
            if selected {
                painter.circle_stroke(
                    center,
                    r + tf.s(2.2),
                    Stroke::new(tf.s(1.4), Color32::from_rgba_unmultiplied(206, 240, 74, a(255.0))),
                );
            }
            if interactive && resp.clicked() {
                chosen = Some(kind);
            }
        }

        if let Some(kind) = chosen {
            self.set_pet(kind);
        }
    }

    fn apply_scale(&mut self, ctx: &Context, new_scale: f32) {
        let clamped = new_scale.clamp(MIN_SCALE, MAX_SCALE);
        self.size_toast = 1.2;
        if (clamped - self.config.scale).abs() > 0.005 {
            self.config.scale = clamped;
            self.config_dirty = true;
            self.programmatic_resize_until = Some(Instant::now() + Duration::from_millis(300));
            ctx.send_viewport_cmd(ViewportCommand::InnerSize(vec2(
                CANVAS_W * clamped,
                CANVAS_H * clamped,
            )));
        }
    }

    fn apply_window_flags(&mut self, ctx: &Context) {
        let level = if self.config.always_on_top {
            WindowLevel::AlwaysOnTop
        } else {
            WindowLevel::Normal
        };
        ctx.send_viewport_cmd(ViewportCommand::WindowLevel(level));
        ctx.send_viewport_cmd(ViewportCommand::MousePassthrough(
            self.config.click_through,
        ));
    }

    fn set_autostart(&mut self, enable: bool, copy_to_permanent: bool) -> bool {
        let result = if enable {
            autostart::enable(copy_to_permanent).map(|path| {
                let copied = copy_to_permanent && self.can_copy_permanent;
                let msg = if copied {
                    "Copiado para pasta fixa e ativado!".to_string()
                } else {
                    "Inicialização automática ativada".to_string()
                };
                eprintln!("[gatin] Autostart ativado: {}", path.display());
                msg
            })
        } else {
            autostart::disable().map(|()| "Inicialização automática desativada".to_string())
        };

        match result {
            Ok(msg) => {
                self.autostart_enabled = enable;
                self.autostart_error = None;
                self.status_toast = Some((msg, 3.2));
                true
            }
            Err(err) => {
                self.autostart_error = Some(err);
                false
            }
        }
    }

    fn render_autostart_prompt(&mut self, ctx: &Context, viewport_rect: Rect) {
        let max_w = (viewport_rect.width() - 24.0).clamp(190.0, 290.0);
        egui::Area::new(egui::Id::new("gatin_autostart_prompt"))
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 6.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                Frame::NONE
                    .fill(Color32::from_rgba_unmultiplied(22, 22, 26, 242))
                    .stroke(Stroke::new(
                        1.2_f32,
                        Color32::from_rgba_unmultiplied(206, 240, 74, 180),
                    ))
                    .corner_radius(12.0)
                    .inner_margin(egui::Margin::same(14))
                    .show(ui, |ui| {
                        ui.set_width(max_w);
                        ui.spacing_mut().item_spacing.y = 7.0;

                        ui.label(
                            egui::RichText::new("Iniciar com o sistema?")
                                .strong()
                                .size(14.0)
                                .color(Color32::from_rgb(244, 244, 240)),
                        );

                        ui.checkbox(
                            &mut self.prompt_autostart_checked,
                            "Abrir o Gatin ao ligar o computador",
                        );

                        if self.prompt_autostart_checked && self.can_copy_permanent {
                            ui.checkbox(
                                &mut self.prompt_copy_permanent,
                                "Copiar para pasta fixa",
                            );
                            if let Some(dest_label) = autostart::permanent_install_label() {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "Salva uma cópia em {dest_label} para continuar funcionando mesmo se apagar de Downloads."
                                    ))
                                    .size(11.0)
                                    .color(Color32::from_rgb(175, 175, 182)),
                                );
                            }
                        }

                        if let Some(err) = &self.autostart_error {
                            ui.label(
                                egui::RichText::new(err)
                                    .size(11.0)
                                    .color(Color32::from_rgb(245, 120, 105)),
                            );
                        }

                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            if ui.button("Agora não").clicked() {
                                self.show_autostart_prompt = false;
                                self.autostart_error = None;
                                self.config.autostart_prompted = true;
                                self.config.save();
                            }

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let confirm = egui::Button::new(
                                        egui::RichText::new("Confirmar")
                                            .strong()
                                            .color(Color32::from_rgb(22, 22, 26)),
                                    )
                                    .fill(Color32::from_rgb(206, 240, 74));

                                    if ui.add(confirm).clicked() {
                                        let ok = if self.prompt_autostart_checked {
                                            self.set_autostart(true, self.prompt_copy_permanent)
                                        } else {
                                            true
                                        };
                                        if ok {
                                            self.show_autostart_prompt = false;
                                            self.config.autostart_prompted = true;
                                            self.config.save();
                                        }
                                    }
                                },
                            );
                        });
                    });
            });
    }
}

impl eframe::App for GatinApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // Fully transparent window background!
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32().clamp(0.001, 0.05);
        self.last_frame = now;

        // 0. Check password-field privacy state BEFORE processing any keystrokes!
        let auto_password = self.privacy.is_password_focused();
        let demo_password = self.demo_mode && self.demo.synthetic_password;
        self.cat
            .set_password_mode(self.manual_privacy || auto_password || demo_password);

        // Read outer window rect once per frame so global cursor coords can aim eyes directly at cursor
        let outer_rect = ctx.input(|i| i.viewport().outer_rect);

        // 1. Drain global OS input events
        while let Ok(mut ev) = self.input.rx.try_recv() {
            if let InputEvent::MouseMove {
                screen_px: Some([sx, sy]),
                look_dir,
                ..
            } = &mut ev
                && look_dir.is_none()
                && let Some(win) = outer_rect
            {
                let eye_x = win.left() + win.width() * 0.505;
                let eye_y = win.top() + win.height() * 0.33;
                let dx = *sx - eye_x;
                let dy = *sy - eye_y;
                let dist = (dx * dx + dy * dy).sqrt();
                let mag = (dist / 220.0).min(1.0);
                *look_dir = Some(if dist > 1.0 {
                    [(dx / dist) * mag, (dy / dist) * mag]
                } else {
                    [0.0, 0.0]
                });
            }
            self.cat.handle_event(ev);
        }

        // 2. Demo driver (if enabled)
        if self.demo_mode {
            for ev in self.demo.update(dt) {
                self.cat.handle_event(ev);
            }
            if self.demo.synthetic_pet > 0.5 {
                let sway = (self.cat.time * 4.5).sin();
                self.cat.register_demo_pet(sway);
            }
        }

        // 3. Local window input fallback (works even if global hooks lack permissions)
        let mut local_events = Vec::new();
        let mut scroll_y = 0.0_f32;
        let mut shortcut_scale: Option<f32> = None;
        let mut press_origin: Option<Pos2> = None;
        let mut pointer_hover: Option<Pos2> = None;
        #[cfg(target_os = "macos")]
        let mut primary_down = false;
        #[cfg(target_os = "macos")]
        let mut pointer_delta = egui::Vec2::ZERO;

        ctx.input(|i| {
            for ev in &i.events {
                if let egui::Event::Key {
                    key,
                    pressed,
                    repeat,
                    ..
                } = ev
                {
                    if *repeat {
                        continue;
                    }
                    if let Some(kid) = egui_key_to_key_id(*key) {
                        if *pressed {
                            local_events.push(InputEvent::KeyDown(kid));
                        } else {
                            local_events.push(InputEvent::KeyUp(kid));
                        }
                    }
                }
            }

            // Keyboard zoom shortcuts when window is focused (Ctrl/Cmd + / - / 0)
            if i.modifiers.command || i.modifiers.ctrl {
                if i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals) {
                    shortcut_scale = Some(self.config.scale + 0.15);
                } else if i.key_pressed(egui::Key::Minus) {
                    shortcut_scale = Some(self.config.scale - 0.15);
                } else if i.key_pressed(egui::Key::Num0) {
                    shortcut_scale = Some(1.0);
                }
            }

            // Collect scroll delta here and apply OUTSIDE ctx.input (send_viewport_cmd locks ctx!)
            scroll_y = i.smooth_scroll_delta.y;
            press_origin = i.pointer.press_origin();
            pointer_hover = i.pointer.hover_pos();
            #[cfg(target_os = "macos")]
            {
                primary_down = i.pointer.primary_down();
                pointer_delta = i.pointer.delta();
            }

            // Remember window outer position for next launch
            if let Some(outer) = i.viewport().outer_rect {
                let pos = [outer.min.x, outer.min.y];
                let changed = self
                    .last_saved_pos
                    .is_none_or(|p| (p[0] - pos[0]).abs() > 2.0 || (p[1] - pos[1]).abs() > 2.0);
                if changed {
                    self.last_saved_pos = Some(pos);
                    self.config.window_pos = Some(pos);
                    self.config_dirty = true;
                }
            }
        });

        if let Some(target) = shortcut_scale {
            self.apply_scale(ctx, target);
        }

        for ev in local_events {
            // Avoid double-feeding if global hooks are active and window is focused
            if !self.input.status.global_active {
                self.cat.handle_event(ev);
            }
        }

        // 4. Advance animation state
        self.cat.update(dt);
        self.size_toast = (self.size_toast - dt).max(0.0);
        if let Some((_, rem)) = &mut self.status_toast {
            *rem -= dt;
            if *rem <= 0.0 {
                self.status_toast = None;
            }
        }

        // 5. Render full-viewport transparent canvas
        let mut viewport_rect = Rect::NOTHING;
        CentralPanel::default()
            .frame(Frame::NONE)
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                viewport_rect = rect;
                let prompt_open = self.show_autostart_prompt;
                let response = ui.allocate_rect(rect, Sense::click_and_drag());
                let tf = CanvasTransform::from_rect(rect);

                // Keep config.scale synced when the window is resized via OS/WM borders or corners
                let cur_size = [rect.width(), rect.height()];
                let in_programmatic = self
                    .programmatic_resize_until
                    .is_some_and(|until| now < until);
                if in_programmatic {
                    self.last_window_size = Some(cur_size);
                } else {
                    if let Some(prev) = self.last_window_size
                        && ((cur_size[0] - prev[0]).abs() > 1.5
                            || (cur_size[1] - prev[1]).abs() > 1.5)
                    {
                        let observed = (cur_size[0] / CANVAS_W)
                            .min(cur_size[1] / CANVAS_H)
                            .clamp(MIN_SCALE, MAX_SCALE);
                        if (observed - self.config.scale).abs() > 0.005 {
                            self.config.scale = observed;
                            self.config_dirty = true;
                        }
                        self.size_toast = 1.2;
                    }
                    self.last_window_size = Some(cur_size);
                }

                // Scroll wheel zooms smoothly anywhere over the window (unless a menu/prompt is open)
                let pointer_inside = pointer_hover.is_some_and(|p| rect.contains(p));
                if scroll_y.abs() > 1.0
                    && pointer_inside
                    && !self.config.click_through
                    && !prompt_open
                    && !egui::Popup::is_any_open(ctx)
                {
                    let target = self.config.scale + scroll_y * 0.002;
                    self.apply_scale(ctx, target);
                }

                // Border/corner resize hit-testing (avoid overlapping the top pet-picker buttons)
                let top_avoid = (self.picker_alpha > 0.5)
                    .then(|| (tf.pt(96.0, 0.0).x, tf.pt(264.0, 0.0).x));
                let hover_resize_dir = if !self.config.click_through && !prompt_open {
                    response
                        .hover_pos()
                        .and_then(|p| resize_direction_at(rect, p, top_avoid))
                } else {
                    None
                };

                // Hovering over the cat -> track cursor with eyes; rubbing on the head pets the cat!
                if !prompt_open && let Some(hover_pos) = response.hover_pos() {
                    let canvas_pt = tf.to_canvas(hover_pos);
                    let on_head = is_on_cat_head(&self.cat, canvas_pt);
                    let on_cat = on_head || is_on_cat_body(&self.cat, canvas_pt);

                    if let Some(dir) = hover_resize_dir {
                        ctx.set_cursor_icon(resize_cursor(dir));
                    } else if on_cat {
                        ctx.set_cursor_icon(if response.dragged() {
                            CursorIcon::Grabbing
                        } else {
                            CursorIcon::Grab
                        });
                    }

                    // Always aim the cat's eyes directly at the cursor when hovering the window!
                    let head = self.cat.head_center();
                    let eye_cx = head[0];
                    let eye_cy = head[1] - 4.0;
                    let ldx = canvas_pt[0] - eye_cx;
                    let ldy = canvas_pt[1] - eye_cy;
                    let ldist = (ldx * ldx + ldy * ldy).sqrt();
                    let lmag = (ldist / 75.0).min(1.0);
                    if ldist > 2.0 {
                        self.cat
                            .set_look_direction([(ldx / ldist) * lmag, (ldy / ldist) * lmag]);
                    }

                    if let Some(prev) = self.last_hover_pos {
                        let delta = hover_pos - prev;
                        let dist = delta.length();
                        if hover_resize_dir.is_none()
                            && on_head
                            && dist > 1.0
                            && !response.dragged()
                        {
                            let head_rel_x = (canvas_pt[0] - eye_cx) / 36.0;
                            self.cat.register_pet(delta.x, delta.y, head_rel_x);
                        } else if !self.input.status.global_active && dist > 0.5 {
                            let nx = ((hover_pos.x - rect.left()) / rect.width().max(1.0))
                                .clamp(0.0, 1.0);
                            let ny = ((hover_pos.y - rect.top()) / rect.height().max(1.0))
                                .clamp(0.0, 1.0);
                            self.cat.handle_event(InputEvent::MouseMove {
                                norm_x: nx,
                                norm_y: ny,
                                screen_px: None,
                                look_dir: Some(if ldist > 2.0 {
                                    [(ldx / ldist) * lmag, (ldy / ldist) * lmag]
                                } else {
                                    [0.0, 0.0]
                                }),
                            });
                        }
                    }
                    self.last_hover_pos = Some(hover_pos);
                } else {
                    self.last_hover_pos = None;
                }

                // Local mouse click highlight when global hook isn't active
                if !self.input.status.global_active {
                    if response.is_pointer_button_down_on() {
                        self.cat.handle_event(InputEvent::MouseDown(MouseButton::Left));
                    } else if !self.cat.mouse_buttons.is_empty() && !self.demo_mode {
                        self.cat.handle_event(InputEvent::MouseUp(MouseButton::Left));
                    }
                }

                // Drag edges/corners to resize, or drag the body to move the window
                if !prompt_open && response.drag_started_by(egui::PointerButton::Primary) {
                    let origin = press_origin.or_else(|| response.interact_pointer_pos());
                    let drag_resize = origin
                        .and_then(|p| resize_direction_at(rect, p, top_avoid))
                        .filter(|_| !self.config.click_through);
                    if let Some(dir) = drag_resize {
                        #[cfg(not(target_os = "macos"))]
                        ctx.send_viewport_cmd(ViewportCommand::BeginResize(dir));
                        #[cfg(target_os = "macos")]
                        {
                            self.manual_resize_dir = Some(dir);
                        }
                    } else {
                        ctx.send_viewport_cmd(ViewportCommand::StartDrag);
                    }
                }

                #[cfg(target_os = "macos")]
                if let Some(dir) = self.manual_resize_dir {
                    if primary_down {
                        ctx.set_cursor_icon(resize_cursor(dir));
                        let ds = manual_resize_scale_delta(dir, pointer_delta);
                        if ds.abs() > 0.0005 {
                            self.apply_scale(ctx, self.config.scale + ds);
                        }
                    } else {
                        self.manual_resize_dir = None;
                    }
                }

                // Paint the cat, desk, keyboard, mouse, paws, and floating keycaps
                render_scene(ui.painter(), rect, &self.cat, self.config.show_keycaps);

                // Hover chrome (pet picker bar + corner resize grip): fades in while pointer is inside
                self.pet_toast = (self.pet_toast - dt).max(0.0);
                let bar_target = if (pointer_inside && !self.config.click_through)
                    || self.pet_toast > 0.0
                {
                    1.0
                } else {
                    0.0
                };
                self.picker_alpha += (bar_target - self.picker_alpha) * (1.0 - (-dt * 12.0).exp());
                if self.picker_alpha > 0.02 {
                    self.pet_picker_bar(ui, &tf, self.picker_alpha);
                    if !self.config.click_through {
                        draw_resize_grip(
                            ui.painter(),
                            rect,
                            self.picker_alpha,
                            hover_resize_dir == Some(ResizeDirection::SouthEast),
                        );
                    }
                }
                if self.size_toast > 0.0 {
                    let toast_alpha = (self.size_toast / 0.35).min(1.0);
                    draw_size_toast(ui.painter(), rect, self.config.scale, toast_alpha);
                }
                if let Some((msg, rem)) = &self.status_toast {
                    let toast_alpha = (*rem / 0.45).min(1.0);
                    draw_status_toast(ui.painter(), rect, msg, toast_alpha);
                }

                // Right-click context menu
                response.context_menu(|ui| {
                    ui.set_min_width(210.0);
                    ui.label(egui::RichText::new("Gatin").strong());
                    ui.separator();
                    ui.label("Pet:");
                    for kind in PetKind::VISIBLE {
                        ui.horizontal(|ui| {
                            let [c0, c1] = kind.swatch();
                            let (dot, _) =
                                ui.allocate_exact_size(egui::vec2(14.0, 14.0), Sense::hover());
                            ui.painter().circle_filled(dot.center(), 6.0, c0);
                            ui.painter().circle_filled(dot.center(), 2.8, c1);
                            if ui
                                .selectable_label(self.cat.pet == kind, kind.label())
                                .clicked()
                            {
                                self.set_pet(kind);
                            }
                        });
                    }
                    ui.separator();
                    ui.small(format!("Input: {}", self.input.status.backend));
                    ui.small(format!("Privacidade: {}", self.privacy.backend));
                    if let Some(hint) = &self.input.status.hint {
                        ui.small(
                            egui::RichText::new(hint)
                                .color(egui::Color32::from_rgb(240, 195, 90)),
                        );
                    }
                    ui.separator();

                    ui.checkbox(&mut self.manual_privacy, "Tampar olhos (modo senha)");

                    if ui
                        .checkbox(&mut self.config.always_on_top, "Sempre no topo")
                        .changed()
                    {
                        self.config_dirty = true;
                        self.apply_window_flags(ctx);
                    }

                    if ui
                        .checkbox(&mut self.config.show_keycaps, "Mostrar teclas flutuantes")
                        .changed()
                    {
                        self.config_dirty = true;
                    }

                    if ui.checkbox(&mut self.demo_mode, "Modo Demo").changed() && self.demo_mode {
                        self.demo = DemoDriver::new();
                    }

                    if ui
                        .checkbox(&mut self.config.click_through, "Click-through")
                        .changed()
                    {
                        self.config_dirty = true;
                        self.apply_window_flags(ctx);
                    }

                    if self.autostart_supported {
                        let mut enabled = self.autostart_enabled;
                        if ui.checkbox(&mut enabled, "Iniciar com o sistema").changed() {
                            let info = autostart::inspect();
                            let copy = enabled
                                && info.can_copy_to_permanent()
                                && info.running_from_transient;
                            self.set_autostart(enabled, copy);
                        }
                        if let Some(err) = &self.autostart_error {
                            ui.small(
                                egui::RichText::new(err)
                                    .color(egui::Color32::from_rgb(245, 120, 105)),
                            );
                        }
                    }

                    ui.separator();
                    let cur_pct = (self.config.scale * 100.0).round() as i32;
                    ui.label(format!("Tamanho ({cur_pct}%):"));
                    let mut scale_pct = (self.config.scale * 100.0).round();
                    if ui
                        .add(
                            egui::Slider::new(
                                &mut scale_pct,
                                (MIN_SCALE * 100.0)..=(MAX_SCALE * 100.0),
                            )
                            .step_by(5.0)
                            .show_value(false),
                        )
                        .changed()
                    {
                        self.apply_scale(ctx, scale_pct / 100.0);
                    }
                    ui.horizontal(|ui| {
                        if ui.small_button("−").clicked() {
                            self.apply_scale(ctx, self.config.scale - 0.15);
                        }
                        for (label, s) in [
                            ("50%", 0.5),
                            ("75%", 0.75),
                            ("100%", 1.0),
                            ("150%", 1.5),
                            ("200%", 2.0),
                        ] {
                            let active = (self.config.scale - s).abs() < 0.03;
                            if ui.selectable_label(active, label).clicked() {
                                self.apply_scale(ctx, s);
                            }
                        }
                        if ui.small_button("+").clicked() {
                            self.apply_scale(ctx, self.config.scale + 0.15);
                        }
                    });

                    ui.separator();
                    if ui.button("Sair").clicked() {
                        self.config.save();
                        ctx.send_viewport_cmd(ViewportCommand::Close);
                    }
                });
            });

        if self.show_autostart_prompt {
            self.render_autostart_prompt(ctx, viewport_rect);
        }

        // Debounced config persistence
        if self.config_dirty && self.last_save.elapsed() >= Duration::from_secs(1) {
            self.config.save();
            self.config_dirty = false;
            self.last_save = Instant::now();
        }

        // Request smooth ~60 FPS updates
        ctx.request_repaint_after(Duration::from_millis(16));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.config.save();
    }
}

/// Returns which window border or corner is under `pos` for frameless drag-resizing.
fn resize_direction_at(
    rect: Rect,
    pos: Pos2,
    top_avoid_x: Option<(f32, f32)>,
) -> Option<ResizeDirection> {
    if !rect.contains(pos) {
        return None;
    }
    const EDGE: f32 = 8.0;
    const CORNER: f32 = 18.0;
    const SE_GRIP: f32 = 26.0;

    let d_left = pos.x - rect.left();
    let d_right = rect.right() - pos.x;
    let d_top = pos.y - rect.top();
    let d_bottom = rect.bottom() - pos.y;

    if d_right <= SE_GRIP && d_bottom <= SE_GRIP {
        return Some(ResizeDirection::SouthEast);
    }
    if d_left <= CORNER && d_top <= CORNER {
        return Some(ResizeDirection::NorthWest);
    }
    if d_right <= CORNER && d_top <= CORNER {
        return Some(ResizeDirection::NorthEast);
    }
    if d_left <= CORNER && d_bottom <= CORNER {
        return Some(ResizeDirection::SouthWest);
    }
    if d_top <= EDGE {
        if let Some((x0, x1)) = top_avoid_x
            && pos.x >= x0
            && pos.x <= x1
        {
            return None;
        }
        return Some(ResizeDirection::North);
    }
    if d_bottom <= EDGE {
        return Some(ResizeDirection::South);
    }
    if d_left <= EDGE {
        return Some(ResizeDirection::West);
    }
    if d_right <= EDGE {
        return Some(ResizeDirection::East);
    }
    None
}

fn resize_cursor(dir: ResizeDirection) -> CursorIcon {
    match dir {
        ResizeDirection::North => CursorIcon::ResizeNorth,
        ResizeDirection::South => CursorIcon::ResizeSouth,
        ResizeDirection::East => CursorIcon::ResizeEast,
        ResizeDirection::West => CursorIcon::ResizeWest,
        ResizeDirection::NorthEast => CursorIcon::ResizeNorthEast,
        ResizeDirection::SouthEast => CursorIcon::ResizeSouthEast,
        ResizeDirection::NorthWest => CursorIcon::ResizeNorthWest,
        ResizeDirection::SouthWest => CursorIcon::ResizeSouthWest,
    }
}

#[cfg(target_os = "macos")]
fn manual_resize_scale_delta(dir: ResizeDirection, delta: egui::Vec2) -> f32 {
    let sx = match dir {
        ResizeDirection::East | ResizeDirection::NorthEast | ResizeDirection::SouthEast => delta.x,
        ResizeDirection::West | ResizeDirection::NorthWest | ResizeDirection::SouthWest => -delta.x,
        ResizeDirection::North | ResizeDirection::South => 0.0,
    } / CANVAS_W;
    let sy = match dir {
        ResizeDirection::South | ResizeDirection::SouthEast | ResizeDirection::SouthWest => delta.y,
        ResizeDirection::North | ResizeDirection::NorthEast | ResizeDirection::NorthWest => {
            -delta.y
        }
        ResizeDirection::East | ResizeDirection::West => 0.0,
    } / CANVAS_H;
    if sx.abs() >= sy.abs() { sx } else { sy }
}

/// Subtle corner grip drawn at the bottom-right when hovering the window.
fn draw_resize_grip(painter: &egui::Painter, rect: Rect, alpha: f32, hovered: bool) {
    let a = |v: f32| (v * alpha).round().clamp(0.0, 255.0) as u8;
    let badge = Rect::from_min_max(rect.right_bottom() - vec2(24.0, 24.0), rect.right_bottom() - vec2(4.0, 4.0));
    let bg = if hovered {
        Color32::from_rgba_unmultiplied(206, 240, 74, a(240.0))
    } else {
        Color32::from_rgba_unmultiplied(22, 22, 26, a(205.0))
    };
    let fg = if hovered {
        Color32::from_rgba_unmultiplied(22, 22, 26, a(255.0))
    } else {
        Color32::from_rgba_unmultiplied(244, 244, 240, a(225.0))
    };

    painter.rect_filled(badge, 6.0, bg);
    let rb = badge.right_bottom() - vec2(4.0, 4.0);
    let stroke = Stroke::new(1.6_f32, fg);
    for len in [4.5_f32, 8.5, 12.5] {
        painter.line_segment([rb - vec2(len, 0.0), rb - vec2(0.0, len)], stroke);
    }
}

/// Brief pill showing the current scale percentage while resizing.
fn draw_size_toast(painter: &egui::Painter, rect: Rect, scale: f32, alpha: f32) {
    let a = |v: f32| (v * alpha).round().clamp(0.0, 255.0) as u8;
    let center = Pos2::new(rect.right() - 56.0, rect.bottom() - 14.0);
    let pill = Rect::from_center_size(center, vec2(48.0, 20.0));
    painter.rect_filled(
        pill,
        10.0,
        Color32::from_rgba_unmultiplied(22, 22, 26, a(215.0)),
    );
    painter.text(
        pill.center(),
        Align2::CENTER_CENTER,
        format!("{}%", (scale * 100.0).round() as i32),
        FontId::proportional(11.5),
        Color32::from_rgba_unmultiplied(206, 240, 74, a(255.0)),
    );
}

/// Brief pill showing autostart confirmation near the bottom center.
fn draw_status_toast(painter: &egui::Painter, rect: Rect, message: &str, alpha: f32) {
    let a = |v: f32| (v * alpha).round().clamp(0.0, 255.0) as u8;
    let width = (message.chars().count() as f32 * 6.5 + 24.0).clamp(140.0, rect.width() - 16.0);
    let center = Pos2::new(rect.center().x, rect.bottom() - 18.0);
    let pill = Rect::from_center_size(center, vec2(width, 22.0));
    painter.rect_filled(
        pill,
        11.0,
        Color32::from_rgba_unmultiplied(22, 22, 26, a(225.0)),
    );
    painter.text(
        pill.center(),
        Align2::CENTER_CENTER,
        message,
        FontId::proportional(11.0),
        Color32::from_rgba_unmultiplied(206, 240, 74, a(255.0)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_hit_testing_covers_corners_edges_and_excludes_picker() {
        let rect = Rect::from_min_size(Pos2::new(0.0, 0.0), vec2(360.0, 320.0));

        assert_eq!(
            resize_direction_at(rect, Pos2::new(350.0, 310.0), None),
            Some(ResizeDirection::SouthEast)
        );
        assert_eq!(
            resize_direction_at(rect, Pos2::new(5.0, 5.0), None),
            Some(ResizeDirection::NorthWest)
        );
        assert_eq!(
            resize_direction_at(rect, Pos2::new(355.0, 5.0), None),
            Some(ResizeDirection::NorthEast)
        );
        assert_eq!(
            resize_direction_at(rect, Pos2::new(5.0, 315.0), None),
            Some(ResizeDirection::SouthWest)
        );
        assert_eq!(
            resize_direction_at(rect, Pos2::new(180.0, 3.0), None),
            Some(ResizeDirection::North)
        );
        assert_eq!(
            resize_direction_at(rect, Pos2::new(180.0, 3.0), Some((100.0, 260.0))),
            None
        );
        assert_eq!(
            resize_direction_at(rect, Pos2::new(180.0, 317.0), None),
            Some(ResizeDirection::South)
        );
        assert_eq!(
            resize_direction_at(rect, Pos2::new(3.0, 160.0), None),
            Some(ResizeDirection::West)
        );
        assert_eq!(
            resize_direction_at(rect, Pos2::new(357.0, 160.0), None),
            Some(ResizeDirection::East)
        );
        assert_eq!(
            resize_direction_at(rect, Pos2::new(180.0, 160.0), None),
            None
        );
    }
}
