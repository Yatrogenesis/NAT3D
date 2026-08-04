// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Francisco Molina-Burgos, Avermex Research Division

//! NAT3D Mobile - Native iOS/Android wrapper.
//!
//! This crate provides platform-specific entry points and input handling:
//! - Android: Native Activity with touch input
//! - iOS: Metal-backed eframe with Apple Pencil support

pub mod ios;

#[cfg(target_os = "android")]
use android_activity::AndroidApp;

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: AndroidApp) {
    use nat3d_app::Nat3DApp;

    // Set up options
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title("NAT3D Mobile"),
        ..Default::default()
    };

    eframe::run_native(
        "nat3d_mobile",
        options,
        Box::new(|cc| Ok(Box::new(Nat3DApp::new(cc)))),
    )
    .unwrap();
}
