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

//! Rapid expansion physics preset.
//!
//! Simulates rapid volumetric expansion effects like bursts, dispersals, and shockwaves.

/// Configuration for rapid expansion simulation.
#[derive(Debug, Clone)]
pub struct RapidExpansionPreset {
    /// Initial expansion velocity.
    pub initial_velocity: f64,
    /// Expansion radius.
    pub radius: f64,
    /// Particle count.
    pub particle_count: usize,
    /// Drag coefficient.
    pub drag: f64,
    /// Duration of the expansion phase.
    pub expansion_duration: f64,
}

impl Default for RapidExpansionPreset {
    fn default() -> Self {
        Self {
            initial_velocity: 50.0,
            radius: 5.0,
            particle_count: 1000,
            drag: 0.1,
            expansion_duration: 2.0,
        }
    }
}

impl RapidExpansionPreset {
    /// Create a new rapid expansion preset.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the initial velocity.
    #[must_use]
    pub fn with_velocity(mut self, velocity: f64) -> Self {
        self.initial_velocity = velocity;
        self
    }

    /// Set the expansion radius.
    #[must_use]
    pub fn with_radius(mut self, radius: f64) -> Self {
        self.radius = radius;
        self
    }

    /// Set particle count.
    #[must_use]
    pub fn with_particle_count(mut self, count: usize) -> Self {
        self.particle_count = count;
        self
    }
}
