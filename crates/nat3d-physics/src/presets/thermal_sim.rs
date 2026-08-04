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

//! Thermal simulation preset.
//!
//! Simulates thermal plumes, heat convection, and temperature-driven fluid motion.

/// Configuration for thermal simulation.
#[derive(Debug, Clone)]
pub struct ThermalSimPreset {
    /// Base temperature in Kelvin.
    pub base_temperature: f64,
    /// Heat source temperature.
    pub source_temperature: f64,
    /// Buoyancy coefficient.
    pub buoyancy: f64,
    /// Cooling rate.
    pub cooling_rate: f64,
    /// Turbulence intensity.
    pub turbulence: f64,
}

impl Default for ThermalSimPreset {
    fn default() -> Self {
        Self {
            base_temperature: 300.0,
            source_temperature: 1200.0,
            buoyancy: 0.5,
            cooling_rate: 0.1,
            turbulence: 0.3,
        }
    }
}

impl ThermalSimPreset {
    /// Create a new thermal simulation preset.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Configure for campfire-like behavior.
    #[must_use]
    pub fn campfire() -> Self {
        Self {
            base_temperature: 300.0,
            source_temperature: 900.0,
            buoyancy: 0.4,
            cooling_rate: 0.15,
            turbulence: 0.25,
        }
    }

    /// Configure for torch-like behavior.
    #[must_use]
    pub fn torch() -> Self {
        Self {
            base_temperature: 300.0,
            source_temperature: 1100.0,
            buoyancy: 0.6,
            cooling_rate: 0.2,
            turbulence: 0.2,
        }
    }

    /// Configure for furnace-like behavior.
    #[must_use]
    pub fn furnace() -> Self {
        Self {
            base_temperature: 400.0,
            source_temperature: 1500.0,
            buoyancy: 0.3,
            cooling_rate: 0.05,
            turbulence: 0.1,
        }
    }
}
