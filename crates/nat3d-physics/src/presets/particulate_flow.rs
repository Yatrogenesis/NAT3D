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

//! Particulate flow simulation preset.
//!
//! Simulates aerosol particles, dust clouds, fog, and similar suspended particulate matter.

/// Configuration for particulate flow simulation.
#[derive(Debug, Clone)]
pub struct ParticulateFlowPreset {
    /// Particle density (particles per cubic meter).
    pub density: f64,
    /// Average particle size in meters.
    pub particle_size: f64,
    /// Diffusion rate.
    pub diffusion: f64,
    /// Settling velocity.
    pub settling_velocity: f64,
    /// Wind influence factor.
    pub wind_influence: f64,
    /// Opacity per unit density.
    pub opacity_factor: f64,
}

impl Default for ParticulateFlowPreset {
    fn default() -> Self {
        Self {
            density: 1000.0,
            particle_size: 0.001,
            diffusion: 0.1,
            settling_velocity: 0.01,
            wind_influence: 0.8,
            opacity_factor: 0.5,
        }
    }
}

impl ParticulateFlowPreset {
    /// Create a new particulate flow preset.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Configure for dust-like behavior.
    #[must_use]
    pub fn dust() -> Self {
        Self {
            density: 500.0,
            particle_size: 0.0001,
            diffusion: 0.2,
            settling_velocity: 0.02,
            wind_influence: 0.9,
            opacity_factor: 0.3,
        }
    }

    /// Configure for fog-like behavior.
    #[must_use]
    pub fn fog() -> Self {
        Self {
            density: 2000.0,
            particle_size: 0.00001,
            diffusion: 0.05,
            settling_velocity: 0.001,
            wind_influence: 0.5,
            opacity_factor: 0.8,
        }
    }

    /// Configure for steam-like behavior.
    #[must_use]
    pub fn steam() -> Self {
        Self {
            density: 800.0,
            particle_size: 0.00005,
            diffusion: 0.15,
            settling_velocity: -0.05,
            wind_influence: 0.7,
            opacity_factor: 0.4,
        }
    }

    /// Configure for mist-like behavior.
    #[must_use]
    pub fn mist() -> Self {
        Self {
            density: 1500.0,
            particle_size: 0.00002,
            diffusion: 0.08,
            settling_velocity: 0.005,
            wind_influence: 0.6,
            opacity_factor: 0.6,
        }
    }
}
