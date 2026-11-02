// SPDX-License-Identifier: AGPL-3.0-or-later OR LicenseRef-Commercial
// Copyright (C) 2026 Francisco Molina-Burgos

//! # NAT3D
//!
//! Umbrella crate for the NAT3D stack. Adding `nat3d` as a dependency brings in
//! every NAT3D library crate; each one is re-exported under a short name.
//!
//! | Module | Crate | Purpose |
//! |---|---|---|
//! | [`core`] | `nat3d-core` | Scene, entities and shared traits |
//! | [`math`] | `nat3d-math` | Vectors, matrices and geometry |
//! | [`physics`] | `nat3d-physics` | Physics simulation |
//! | [`render`] | `nat3d-render` | Rendering |
//! | [`ui`] | `nat3d-ui` | User interface components |
//! | [`modeling`] | `nat3d-modeling` | Modeling and CAD |
//! | [`animation`] | `nat3d-animation` | Animation |
//! | [`io`] | `nat3d-io` | File formats and secure storage |
//! | [`scripting`] | `nat3d-scripting` | Scripting |
//! | [`cloud`] | `nat3d-cloud` | Cloud services |
//! | [`sync`] | `nat3d-sync` | Device synchronisation and render farm |
//! | [`app`] | `nat3d-app` | The desktop application |
//!
//! ## Features
//!
//! * `python` — Python scripting (needs a Python interpreter at build time).
//! * `ipad` — iPad touch and stylus input.
//! * `file-dialog` — native file dialogs.
//! * `mobile` — the Android/iOS wrapper, re-exported as `mobile`.
//! * `full` — all of the above.
//!
//! By default none of them is enabled, so the crate builds with Rust alone.
//!
//! ## Installing the program
//!
//! This crate is a library. The command line tool is `nat3d-cli`
//! (`cargo install nat3d-cli`, binary `nat3d`) and the application is
//! `nat3d-app` (`cargo install nat3d-app`).

#![forbid(unsafe_code)]

pub use nat3d_animation as animation;
pub use nat3d_app as app;
pub use nat3d_cloud as cloud;
pub use nat3d_core as core;
pub use nat3d_io as io;
pub use nat3d_math as math;
#[cfg(feature = "mobile")]
pub use nat3d_mobile as mobile;
pub use nat3d_modeling as modeling;
pub use nat3d_physics as physics;
pub use nat3d_render as render;
pub use nat3d_scripting as scripting;
pub use nat3d_sync as sync;
pub use nat3d_ui as ui;
