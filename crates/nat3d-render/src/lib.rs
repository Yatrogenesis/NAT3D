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

//! # NAT3D Render
//!
//! Rendering engine for NAT3D using wgpu.

#![warn(missing_docs)]
#![allow(dead_code)]

pub mod backend;
pub mod lighting;
pub mod pipeline;
pub mod postprocess;
pub mod raytracing;

/// Rendering error type.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    /// GPU device creation failed.
    #[error("Failed to create GPU device: {0}")]
    DeviceCreation(String),
    /// Shader compilation failed.
    #[error("Shader compilation failed: {0}")]
    ShaderCompilation(String),
    /// Surface error.
    #[error("Surface error: {0}")]
    Surface(String),
}

/// Result type for render operations.
pub type RenderResult<T> = Result<T, RenderError>;
