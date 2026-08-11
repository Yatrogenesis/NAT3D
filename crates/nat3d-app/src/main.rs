// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Francisco Molina-Burgos, Avermex Research Division

// On Windows release builds, suppress the console window that a normal
// (console-subsystem) binary always gets attached to by the OS — even when
// launched by double-click. Without this, NAT3D opens with a raw log
// console behind the GUI window (see crash report / support thread: users
// mistook `wgpu`'s startup log lines, e.g. an "unknown backend string"
// warning, for an application error). Debug builds keep the console so
// `cargo run` still shows logs. This is the same attribute the official
// eframe/egui application template ships with by default.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use nat3d_app::Nat3DApp;

fn main() -> eframe::Result<()> {
    // Install crash reporter first — captures panics during GPU/logging init too
    nat3d_app::startup::setup_crash_reporter();

    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    tracing::info!("NAT3D - Next-generation Advanced Technology for 3D");
    tracing::info!("Version: {}", env!("CARGO_PKG_VERSION"));

    // Try Vulkan first, then DX12. (There is no "dx11" wgpu backend as of
    // wgpu 23.x — that string was silently ignored and only produced a
    // confusing "unknown backend string 'dx11'" warning; removed.)
    // Override with WGPU_BACKEND env var to force a specific backend.
    if std::env::var("WGPU_BACKEND").is_err() {
        std::env::set_var("WGPU_BACKEND", "vulkan,dx12");
    }

    // Pick the wgpu adapter ourselves instead of letting egui-wgpu use
    // whichever one `request_adapter()` reports as "best" for the requested
    // `PowerPreference` (egui-wgpu's default `WgpuConfiguration::CreateNew`
    // path — see `egui-wgpu::RenderState::create`). That path tries exactly
    // one adapter; if device creation on it fails, the error propagates and
    // the whole app exits immediately with no fallback.
    //
    // Observed in the wild (reproduced under Wine, but the same adapter
    // shape can occur on real Windows hardware with a partially-working
    // D3D12 driver): a `DiscreteGpu`-labelled DX12 adapter outranks a
    // perfectly working Vulkan one, fails to create a compute pipeline, and
    // the device is lost within the first second of startup — matches the
    // "opens, pauses, error, force-quit, closes" reports. Here we enumerate
    // every adapter ourselves and keep trying until one actually creates a
    // device, preferring Vulkan/Metal (broadest driver support) over DX12.
    let wgpu_setup = pollster::block_on(select_wgpu_setup());

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1600.0, 900.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("NAT3D - 3D Modeling Suite")
            .with_visible(true)
            .with_active(true),
        wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
            wgpu_setup,
            ..Default::default()
        },
        ..Default::default()
    };

    tracing::info!("Launching event loop...");
    let result = eframe::run_native(
        "NAT3D",
        options,
        Box::new(|cc| Ok(Box::new(Nat3DApp::new(cc)))),
    );
    tracing::info!("Event loop exited: {:?}", result);
    result
}

/// Try every available wgpu adapter, in reliability order, until one can
/// actually create a device. Falls back to egui-wgpu's own default selection
/// logic if none of them work (keeps prior behavior — and its error
/// reporting — as a last resort rather than failing silently here).
async fn select_wgpu_setup() -> eframe::egui_wgpu::WgpuSetup {
    let backends = eframe::wgpu::util::backend_bits_from_env().unwrap_or(eframe::wgpu::Backends::PRIMARY);
    let instance = eframe::wgpu::Instance::new(eframe::wgpu::InstanceDescriptor {
        backends,
        ..Default::default()
    });

    let mut candidates = instance.enumerate_adapters(backends);
    candidates.sort_by_key(|a| match a.get_info().backend {
        eframe::wgpu::Backend::Vulkan | eframe::wgpu::Backend::Metal => 0,
        eframe::wgpu::Backend::Dx12 => 1,
        _ => 2,
    });

    for adapter in candidates {
        let info = adapter.get_info();
        match adapter
            .request_device(
                &eframe::wgpu::DeviceDescriptor {
                    label: Some("NAT3D Device"),
                    required_features: eframe::wgpu::Features::empty(),
                    required_limits: eframe::wgpu::Limits::default(),
                    memory_hints: Default::default(),
                },
                None,
            )
            .await
        {
            Ok((device, queue)) => {
                tracing::info!(
                    "Selected GPU adapter: {} ({:?}, {:?})",
                    info.name,
                    info.backend,
                    info.device_type
                );
                return eframe::egui_wgpu::WgpuSetup::Existing {
                    instance: std::sync::Arc::new(instance),
                    adapter: std::sync::Arc::new(adapter),
                    device: std::sync::Arc::new(device),
                    queue: std::sync::Arc::new(queue),
                };
            }
            Err(e) => {
                tracing::warn!(
                    "GPU adapter {} ({:?}) failed to create a device ({e}) — trying next",
                    info.name,
                    info.backend
                );
            }
        }
    }

    tracing::warn!(
        "Manual adapter selection found no usable adapter; falling back to egui-wgpu's default logic"
    );
    eframe::egui_wgpu::WgpuConfiguration::default().wgpu_setup
}
