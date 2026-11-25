#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod cat;
mod config;
mod demo;
mod input;
mod keyboard;
mod privacy;
mod renderer;

use crate::app::GatinApp;
use crate::cat::{CANVAS_H, CANVAS_W};
use crate::config::AppConfig;
use crate::input::GlobalInput;
use crate::privacy::PrivacyMonitor;
use clap::Parser;
use egui::{ViewportBuilder, WindowLevel};

#[derive(Parser, Debug)]
#[command(
    name = "gatin",
    version,
    about = "Gatinho preto desktop pet em Rust (Linux, Windows e macOS)"
)]
struct Cli {
    /// Inicia no modo demo (digitação, mouse e carinho automáticos)
    #[arg(long)]
    demo: bool,

    /// Fator de escala inicial (ex: 0.75, 1.0, 1.5)
    #[arg(long)]
    scale: Option<f32>,

    /// Oculta as teclinhas brancas flutuantes
    #[arg(long)]
    no_keycaps: bool,

    /// Não mantém a janela sempre no topo
    #[arg(long)]
    no_top: bool,

    /// Inicia em modo click-through (cliques atravessam a janela)
    #[arg(long)]
    click_through: bool,

    /// Exibe logs de diagnóstico da detecção de campos de senha (AT-SPI2/UIA/Carbon)
    #[arg(long)]
    debug_privacy: bool,
}

fn main() -> eframe::Result<()> {
    let cli = Cli::parse();
    let mut config = AppConfig::load();

    if let Some(scale) = cli.scale {
        config.scale = scale.clamp(0.5, 2.5);
    }
    if cli.no_keycaps {
        config.show_keycaps = false;
    }
    if cli.no_top {
        config.always_on_top = false;
    }
    if cli.click_through {
        config.click_through = true;
    }

    let global_input = GlobalInput::start();
    let privacy = PrivacyMonitor::start(cli.debug_privacy);
    eprintln!("[gatin] Input backend: {}", global_input.status.backend);
    eprintln!("[gatin] Privacy detector: {}", privacy.backend);
    if let Some(hint) = &global_input.status.hint {
        eprintln!("[gatin] Aviso: {hint}");
    }

    let width = CANVAS_W * config.scale;
    let height = CANVAS_H * config.scale;

    let mut viewport = ViewportBuilder::default()
        .with_app_id("gatin")
        .with_title("gatin")
        .with_inner_size([width, height])
        .with_min_inner_size([CANVAS_W * 0.5, CANVAS_H * 0.5])
        .with_decorations(false)
        .with_transparent(true)
        .with_has_shadow(false)
        .with_resizable(true)
        .with_mouse_passthrough(config.click_through)
        .with_window_level(if config.always_on_top {
            WindowLevel::AlwaysOnTop
        } else {
            WindowLevel::Normal
        });

    if let Some([x, y]) = config.window_pos {
        viewport = viewport.with_position([x, y]);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    let demo_mode = cli.demo;
    eframe::run_native(
        "gatin",
        options,
        Box::new(move |cc| {
            // Keep egui visuals crisp for the right-click context menu
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Ok(Box::new(GatinApp::new(
                config,
                demo_mode,
                global_input,
                privacy,
            )))
        }),
    )
}
