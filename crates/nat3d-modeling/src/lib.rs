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

//! # NAT3D Modeling
//! Modeling tools for NAT3D.

// Clippy allows justified for 3D modeling context:
//
// cast_precision_loss: Mesh indices and segment counts are cast to f32/f64 for
// parametric calculations (e.g., `i as f32 / segments as f32` for UV coordinates,
// interpolation parameters). These values are small integers well within f32/f64
// precision, and the slight precision loss is acceptable for graphics work.
#![allow(clippy::cast_precision_loss)]
//
// many_single_char_names: 3D math uses conventional single-letter variables:
// x, y, z (coordinates), u, v (texture/parametric), t (interpolation parameter),
// n (normal/count), w (homogeneous weight), i, j, k (indices). These are standard
// in graphics literature and more readable than verbose alternatives.
#![allow(clippy::many_single_char_names)]
//
// too_many_arguments: Bezier curves (cubic_to: 6 coords), arcs (arc_to: 7 params),
// and CSG operations require many parameters by mathematical definition. Wrapping
// these in builder patterns would add complexity without clarity benefit.
#![allow(clippy::too_many_arguments)]
//
// dead_code: Development in progress; not all APIs are exercised yet.
#![allow(dead_code)]
pub mod cad;
pub mod modifiers;
pub mod nurbs;
pub mod polygon;
pub mod sculpt;
pub mod sketch2mesh;
pub mod spectral;
pub mod uv;
