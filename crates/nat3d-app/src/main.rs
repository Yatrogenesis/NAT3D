// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Francisco Molina-Burgos, Avermex Research Division

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

    // Try Vulkan first (more reliable on Intel Iris Xe); fall back to DX12 then DX11.
    // Override with WGPU_BACKEND env var to force a specific backend.
    if std::env::var("WGPU_BACKEND").is_err() {
        std::env::set_var("WGPU_BACKEND", "vulkan,dx12,dx11");
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1600.0, 900.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("NAT3D - 3D Modeling Suite")
            .with_visible(true)
            .with_active(true),
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
