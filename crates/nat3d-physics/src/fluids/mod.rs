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

//! Fluid simulation module.
//!
//! Provides multiple fluid simulation methods:
//! - Navier-Stokes (grid-based)
//! - SPH (Smoothed Particle Hydrodynamics)
//! - FLIP/APIC (hybrid)

pub mod boundary;
pub mod flip;
pub mod grid;
pub mod navier_stokes;
pub mod navier_stokes_fast;
pub mod pressure;
pub mod sph;
pub mod surface;
pub mod viscosity;

pub mod sph_fem;
