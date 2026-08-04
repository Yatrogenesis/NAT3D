/*
 * NAT3D - Next-generation Advanced Technology for 3D
 * Professional 3D Modeling, CAD, Physics Simulation and Rendering Suite
 * 
 * Copyright (C) 2023-2026 Francisco Molina <pako.molina@gmail.com>
 * 
 * This software is dual-licensed:
 * 1. Open Source: GNU Affero General Public License v3.0 or later (AGPL-3.0-or-later)
 * 2. Commercial: For commercial use, please contact <fmolina@avermex.com>
 * 
 * For research information, visit: https://research.avermex.com
 * For collaborations, contact: <pako.molina@gmail.com>
 * 
 * DOI: [PENDING]
 */

//! Image formats.
pub mod exr_format;
pub mod hdr;
pub mod png;
pub mod texture;

// Re-exports for convenience
pub use exr_format::{load_exr, save_exr, ExrChannel, ExrError, ExrImage};
pub use hdr::{load_hdr, save_hdr, HdrError, HdrImage};
pub use png::{load_png, save_png, PngError, PngImage};
pub use texture::{
    load_texture, resize_texture, Texture, TextureCache, TextureError, TextureFormat,
};
