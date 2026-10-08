// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Francisco Molina-Burgos, Avermex Research Division

//! wasm32 entry point for NAT3D running inside a browser tab.
//!
//! This mirrors the standard eframe/egui web template (see the `eframe`
//! project's own `eframe_template` on GitHub): a plain `fn main()`, gated by
//! target, that finds a `<canvas>` element by id and starts
//! `eframe::WebRunner` on it. `wasm-bindgen` does not need a
//! `#[wasm_bindgen(start)]` attribute here — for a `bin` crate compiled to
//! `wasm32-unknown-unknown`, rustc already emits this `main` as the module's
//! start function, which the browser invokes automatically once the .wasm
//! is instantiated.
//!
//! All actual application logic lives in `nat3d_app::Nat3DApp`; this crate
//! only wires it to a canvas.

#[cfg(target_arch = "wasm32")]
fn main() {
    // Surface Rust panics as readable messages in the browser console
    // instead of an opaque "unreachable executed".
    console_error_panic_hook::set_once();
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("no `window` — is this running outside a browser?")
            .document()
            .expect("window had no `document`");

        let canvas = document
            .get_element_by_id("nat3d_canvas")
            .expect("no element with id `nat3d_canvas` — check index.html")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("`nat3d_canvas` element was not a <canvas>");

        let start_result = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(Box::new(nat3d_app::Nat3DApp::new(cc)))),
            )
            .await;

        if let Some(loading_text) = document.get_element_by_id("nat3d_loading") {
            match start_result {
                Ok(()) => loading_text.remove(),
                Err(e) => {
                    loading_text.set_inner_html(
                        "<p>NAT3D failed to start. See the browser's developer console (F12) for details.</p>",
                    );
                    panic!("failed to start eframe: {e:?}");
                }
            }
        }
    });
}

// `wasm_bindgen::JsCast` (for `.dyn_into`) is only needed on wasm32, where
// the rest of this file's body compiles.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast as _;

// This crate has no reason to exist outside wasm32 — it only wires
// nat3d-app's `Nat3DApp` to a browser canvas via `eframe::WebRunner`. This
// stub keeps `cargo check --workspace` (run from a native host, as most of
// this repo's tooling does) green without special-casing the crate out of
// the member list; the real entry point above only compiles for wasm32.
#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!(
        "nat3d-web only runs on wasm32-unknown-unknown. Build it with `trunk build` or \
         `trunk serve` from crates/nat3d-web, not `cargo run`."
    );
}
