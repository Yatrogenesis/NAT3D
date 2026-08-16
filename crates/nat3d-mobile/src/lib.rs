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

    // The handle has to be handed to eframe, not merely accepted. winit builds
    // the Android event loop from it, and eframe treats it as required: with
    // the field left at its default of None, `run_native` returns
    // "`NativeOptions` is missing required `android_app`" and the unwrap below
    // turns that into a panic at launch. The application would install, start
    // and die before drawing a frame, which is exactly what the released
    // v0.2.0 package did.
    let options = eframe::NativeOptions {
        android_app: Some(app),
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
