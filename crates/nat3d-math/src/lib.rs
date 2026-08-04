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

//! # NAT3D Math
//!
//! Mathematical utilities for NAT3D, providing re-exports from SciRS
//! and additional functionality for 3D graphics and simulation.
//!
//! ## Features
//!
//! - Linear algebra operations (via scirs-linalg)
//! - FFT transforms (via scirs-fft)
//! - Statistical functions (via scirs-stats)
//! - Optimization algorithms (via scirs-optimize)
//! - Interpolation (bezier, spline, NURBS)
//! - Noise generation (Perlin, Simplex, Worley)
//! - Geometric utilities
//! - Spatial data structures (Octree, BVH, KD-tree)
//! - Easing functions for animation
//! - Spline evaluation (B-spline, NURBS, Bezier)

#![warn(missing_docs)]
#![warn(clippy::all)]
#![allow(unexpected_cfgs)]

pub mod easing;
pub mod geometry;
pub mod interpolate;
pub mod noise;
pub mod spatial;
pub mod spline;

// Re-export yatrosci modules (available when 'yatrosci' feature is enabled)
#[cfg(feature = "yatrosci")]
pub use yatrosci_core as core;
#[cfg(feature = "yatrosci")]
pub use yatrosci_fft as fft;
#[cfg(feature = "yatrosci")]
pub use yatrosci_linalg as linalg;
#[cfg(feature = "yatrosci")]
pub use yatrosci_optimize as optimize;
#[cfg(feature = "yatrosci")]
pub use yatrosci_stats as stats;
// Re-exportados con nombre completo para no colisionar con los módulos propios
// de nat3d-math (geometry, interpolate, spline custom para gráficos 3D).
#[cfg(feature = "yatrosci")]
pub use yatrosci_interpolate;
#[cfg(feature = "yatrosci")]
pub use yatrosci_geometry;
#[cfg(feature = "yatrosci")]
pub use yatrosci_integrate;

// Re-export common math types
pub use glam;
pub use nalgebra;
pub use ndarray;

/// Prelude for convenient imports.
pub mod prelude {
    pub use super::easing::{ease, EasingType};
    pub use super::geometry::*;
    pub use super::interpolate::*;
    pub use super::noise::*;
    pub use super::spatial::{KDTree, Octree, AABB, BVH};
    pub use super::spline::{BSplineCurve, BezierCurve, HermiteSpline, NurbsCurve};
}
