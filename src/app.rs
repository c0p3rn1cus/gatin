use crate::cat::{CANVAS_H, CANVAS_W, CatState};
use crate::config::AppConfig;
use crate::demo::DemoDriver;
use crate::input::{GlobalInput, InputEvent, egui_key_to_key_id};
use crate::keyboard::MouseButton;
use crate::privacy::PrivacyMonitor;
use crate::renderer::{CanvasTransform, is_on_cat_body, is_on_cat_head, render_scene};
use egui::{CentralPanel, Context, CursorIcon, Frame, Pos2, Sense, ViewportCommand, WindowLevel};
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
    config_dirty: bool,
    last_save: Instant,
}

impl GatinApp {
    pub fn new(
        config: AppConfig,
        demo_mode: bool,
        input: GlobalInput,
        privacy: PrivacyMonitor,
    ) -> Self {
        Self {
            cat: CatState::new(),
            input,
            privacy,
            manual_privacy: false,
            last_saved_pos: config.window_pos,
            config,
            demo_mode,
            demo: DemoDriver::new(),
            last_frame: Instant::now(),
            last_hover_pos: None,
            config_dirty: false,
            last_save: Instant::now(),
        }
    }

    fn apply_scale(&mut self, ctx: &Context, new_scale: f32) {
        let clamped = new_scale.clamp(0.5, 2.5);
        if (clamped - self.config.scale).abs() > 0.005 {
            self.config.scale = clamped;
            self.config_dirty = true;
            ctx.send_viewport_cmd(ViewportCommand::InnerSize(egui::vec2(
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

            // Scroll wheel zooms the cat smoothly when hovering the window
            let scroll_y = i.smooth_scroll_delta.y;
            if scroll_y.abs() > 1.0 && i.pointer.hover_pos().is_some() {
                let delta = scroll_y * 0.002;
                let target = self.config.scale + delta;
                self.apply_scale(ctx, target);
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

        for ev in local_events {
            // Avoid double-feeding if global hooks are active and window is focused
            if !self.input.status.global_active {
                self.cat.handle_event(ev);
            }
        }

        // 4. Advance animation state
        self.cat.update(dt);

        // 5. Render full-viewport transparent canvas
        CentralPanel::default()
            .frame(Frame::NONE)
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                let response = ui.allocate_rect(rect, Sense::click_and_drag());
                let tf = CanvasTransform::from_rect(rect);

                // Hovering over the cat -> track cursor with eyes; rubbing on the head pets the cat!
                if let Some(hover_pos) = response.hover_pos() {
                    let canvas_pt = tf.to_canvas(hover_pos);
                    let on_head = is_on_cat_head(&self.cat, canvas_pt);
                    let on_cat = on_head || is_on_cat_body(&self.cat, canvas_pt);

                    if on_cat {
                        ctx.set_cursor_icon(if response.dragged() {
                            CursorIcon::Grabbing
                        } else {
                            CursorIcon::Grab
                        });
                    }

                    // Always aim the cat's eyes directly at the cursor when hovering the window!
                    let eye_cx = 182.0 + self.cat.lean_x;
                    let eye_cy = 104.0 + self.cat.typing_bob;
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
                        if on_head && dist > 1.0 && !response.dragged() {
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

                // Drag anywhere on the cat to reposition it on the desktop!
                if response.drag_started_by(egui::PointerButton::Primary) {
                    ctx.send_viewport_cmd(ViewportCommand::StartDrag);
                }

                // Paint the cat, desk, keyboard, mouse, paws, and floating keycaps
                render_scene(ui.painter(), rect, &self.cat, self.config.show_keycaps);

                // Right-click context menu
                response.context_menu(|ui| {
                    ui.set_min_width(210.0);
                    ui.label(egui::RichText::new("Gatin").strong());
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

                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label("Tamanho:");
                        for (label, s) in [("75%", 0.75), ("100%", 1.0), ("125%", 1.25), ("150%", 1.5)] {
                            if ui.small_button(label).clicked() {
                                self.apply_scale(ctx, s);
                                ui.close();
                            }
                        }
                    });

                    ui.separator();
                    if ui.button("Sair").clicked() {
                        self.config.save();
                        ctx.send_viewport_cmd(ViewportCommand::Close);
                    }
                });
            });

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
