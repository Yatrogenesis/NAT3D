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

//! # NAT3D Physics
//!
//! Physics simulation engine for NAT3D, providing:
//! - Rigid body dynamics
//! - Soft body simulation
//! - Cloth simulation
//! - Fluid dynamics (Navier-Stokes, SPH, FLIP)
//! - Particle systems

#![warn(missing_docs)]
#![allow(clippy::all)]
#![allow(clippy::pedantic)]
#![allow(clippy::nursery)]
#![allow(missing_docs)]

pub mod engine;
pub mod rigid_body;
pub mod soft_body;
pub mod cloth;
pub mod fluids;
pub mod particles;
pub mod presets;

pub use engine::PhysicsEngine;

pub mod quantum;

pub mod xpbd;
