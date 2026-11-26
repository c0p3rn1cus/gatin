#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod autostart;
mod cat;
mod config;
mod demo;
mod input;
mod keyboard;
mod pet;
mod privacy;
mod renderer;

use crate::app::GatinApp;
use crate::cat::{CANVAS_H, CANVAS_W};
use crate::config::{AppConfig, MAX_SCALE, MIN_SCALE};
use crate::input::GlobalInput;
use crate::pet::PetKind;
use crate::privacy::PrivacyMonitor;
use clap::Parser;
use egui::{ViewportBuilder, WindowLevel};

#[derive(Parser, Debug)]
#[command(
    name = "gatin",
    version,
    about = "Pet de desktop em Rust: gatos, cachorro e capivara que digitam com você (Linux, Windows e macOS)"
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

    /// Escolhe o pet: black-cat, frajola, bengal, tricolor, siames, cachorro
    #[arg(long, value_enum)]
    pet: Option<PetKind>,

    /// Exibe logs de diagnóstico da detecção de campos de senha (AT-SPI2/UIA/Carbon)
    #[arg(long)]
    debug_privacy: bool,
}

fn main() -> eframe::Result<()> {
    let cli = Cli::parse();
    autostart::sync_broken_path();
    let mut config = AppConfig::load();

    if let Some(scale) = cli.scale {
        config.scale = scale.clamp(MIN_SCALE, MAX_SCALE);
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
    if let Some(pet) = cli.pet {
        config.pet = pet;
    }

    let width = CANVAS_W * config.scale;
    let height = CANVAS_H * config.scale;

    let mut viewport = ViewportBuilder::default()
        .with_app_id("gatin")
        .with_title("gatin")
        .with_inner_size([width, height])
        .with_min_inner_size([CANVAS_W * MIN_SCALE, CANVAS_H * MIN_SCALE])
        .with_max_inner_size([CANVAS_W * MAX_SCALE, CANVAS_H * MAX_SCALE])
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

    let demo_mode = cli.demo;
    let debug_privacy = cli.debug_privacy;

    #[cfg(target_os = "windows")]
    {
        let glow_options = eframe::NativeOptions {
            viewport: viewport.clone(),
            renderer: eframe::Renderer::Glow,
            ..Default::default()
        };

        let result = match eframe::run_native(
            "gatin",
            glow_options,
            make_app_creator(config.clone(), demo_mode, debug_privacy),
        ) {
            Ok(()) => Ok(()),
            Err(glow_err) => {
                eprintln!(
                    "[gatin] OpenGL indisponível ({glow_err}); tentando fallback DirectX 12 / wgpu..."
                );
                let wgpu_options = eframe::NativeOptions {
                    viewport,
                    renderer: eframe::Renderer::Wgpu,
                    wgpu_options: windows_wgpu_configuration(),
                    ..Default::default()
                };
                eframe::run_native(
                    "gatin",
                    wgpu_options,
                    make_app_creator(config, demo_mode, debug_privacy),
                )
                .map_err(|wgpu_err| format!("OpenGL: {glow_err}\nDirectX/wgpu: {wgpu_err}"))
            }
        };

        if let Err(msg) = result {
            eprintln!("[gatin] Erro fatal: {msg}");
            show_windows_error(&format!("Não foi possível iniciar o Gatin:\n\n{msg}"));
        }
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        let options = eframe::NativeOptions {
            viewport,
            ..Default::default()
        };
        eframe::run_native(
            "gatin",
            options,
            make_app_creator(config, demo_mode, debug_privacy),
        )
    }
}

fn make_app_creator(
    config: AppConfig,
    demo_mode: bool,
    debug_privacy: bool,
) -> eframe::AppCreator<'static> {
    Box::new(move |cc| {
        let global_input = GlobalInput::start();
        let privacy = PrivacyMonitor::start(debug_privacy);
        eprintln!("[gatin] Input backend: {}", global_input.status.backend);
        eprintln!("[gatin] Privacy detector: {}", privacy.backend);
        if let Some(hint) = &global_input.status.hint {
            eprintln!("[gatin] Aviso: {hint}");
        }
        // Keep egui visuals crisp for the right-click context menu
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        Ok(Box::new(GatinApp::new(
            config,
            demo_mode,
            global_input,
            privacy,
        )))
    })
}

#[cfg(target_os = "windows")]
fn windows_wgpu_configuration() -> eframe::egui_wgpu::WgpuConfiguration {
    use eframe::egui_wgpu::{WgpuConfiguration, WgpuSetup, WgpuSetupCreateNew};
    use eframe::wgpu::{Backends, DeviceType, Dx12SwapchainKind};
    use std::sync::Arc;

    let mut create_new = WgpuSetupCreateNew::default();
    create_new.instance_descriptor.backends =
        Backends::from_env().unwrap_or(Backends::DX12 | Backends::VULKAN);
    create_new
        .instance_descriptor
        .backend_options
        .dx12
        .presentation_system =
        Dx12SwapchainKind::from_env().unwrap_or(Dx12SwapchainKind::DxgiFromVisual);
    // Prefer hardware GPUs when present, but allow CPU/WARP fallback on VMs without 3D acceleration.
    create_new.native_adapter_selector = Some(Arc::new(|adapters, surface| {
        let rank = |dt: DeviceType| -> u8 {
            match dt {
                DeviceType::DiscreteGpu => 0,
                DeviceType::IntegratedGpu => 1,
                DeviceType::VirtualGpu => 2,
                DeviceType::Cpu => 3,
                DeviceType::Other => 4,
            }
        };
        adapters
            .iter()
            .filter(|a| surface.is_none_or(|s| a.is_surface_supported(s)))
            .min_by_key(|a| rank(a.get_info().device_type))
            .cloned()
            .ok_or_else(|| {
                "Nenhum adaptador gráfico compatível encontrado (DX12/Vulkan/WARP)".to_string()
            })
    }));

    WgpuConfiguration {
        wgpu_setup: WgpuSetup::CreateNew(create_new),
        ..Default::default()
    }
}

#[cfg(target_os = "windows")]
fn show_windows_error(message: &str) {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn MessageBoxW(
            hwnd: *mut std::ffi::c_void,
            text: *const u16,
            caption: *const u16,
            utype: u32,
        ) -> i32;
    }
    const MB_OK: u32 = 0x0000_0000;
    const MB_ICONERROR: u32 = 0x0000_0010;

    let text: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();
    let title: Vec<u16> = "Gatin - Erro ao iniciar"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}
