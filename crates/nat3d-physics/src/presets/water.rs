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

// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Francisco Molina-Burgos, Avermex Research Division

//! Water fluid preset with realistic physical parameters.

/// Water fluid parameters.
#[derive(Debug, Clone)]
pub struct WaterPreset {
    /// Density (kg/m³).
    pub density: f64,
    /// Dynamic viscosity (Pa·s).
    pub viscosity: f64,
    /// Surface tension (N/m).
    pub surface_tension: f64,
    /// Temperature (°C).
    pub temperature: f64,
}

impl Default for WaterPreset {
    fn default() -> Self {
        Self::at_temperature(20.0)
    }
}

impl WaterPreset {
    /// Create water preset at specific temperature.
    pub fn at_temperature(temp_celsius: f64) -> Self {
        // Viscosity varies with temperature (simplified model)
        let viscosity = match temp_celsius {
            t if t <= 0.0 => 0.001792,  // Ice/slush (0°C)
            t if t <= 10.0 => 0.001307, // 10°C
            t if t <= 20.0 => 0.001002, // 20°C (standard)
            t if t <= 30.0 => 0.000798, // 30°C
            t if t <= 40.0 => 0.000653, // 40°C
            t if t <= 50.0 => 0.000547, // 50°C
            t if t <= 60.0 => 0.000467, // 60°C
            t if t <= 80.0 => 0.000355, // 80°C
            _ => 0.000282,              // 100°C (near boiling)
        };

        // Density varies slightly with temperature
        let density = 1000.0 - (temp_celsius - 4.0).abs() * 0.2; // Peak at 4°C

        Self {
            density,
            viscosity,
            surface_tension: 0.0728, // N/m at 20°C
            temperature: temp_celsius,
        }
    }

    /// Distilled water at room temperature (20°C).
    pub fn distilled() -> Self {
        Self::at_temperature(20.0)
    }

    /// Sea water (saltwater).
    pub fn sea_water() -> Self {
        Self {
            density: 1025.0,     // Slightly denser than fresh water
            viscosity: 0.001076, // Slightly more viscous
            surface_tension: 0.0728,
            temperature: 15.0,
        }
    }

    /// Ice-cold water (0°C).
    pub fn ice_cold() -> Self {
        Self::at_temperature(0.0)
    }

    /// Hot water (80°C).
    pub fn hot() -> Self {
        Self::at_temperature(80.0)
    }

    /// Boiling water (100°C).
    pub fn boiling() -> Self {
        Self::at_temperature(100.0)
    }

    /// Get kinematic viscosity (m²/s).
    pub fn kinematic_viscosity(&self) -> f64 {
        self.viscosity / self.density
    }

    /// Get Reynolds number for given velocity and length scale.
    pub fn reynolds_number(&self, velocity: f64, length: f64) -> f64 {
        (velocity * length) / self.kinematic_viscosity()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_water() {
        let water = WaterPreset::default();
        assert_eq!(water.temperature, 20.0);
        assert!(water.density > 995.0 && water.density < 1005.0); // Temperature-dependent range
        assert!(water.viscosity > 0.0009 && water.viscosity < 0.0011);
    }

    #[test]
    fn test_temperature_variations() {
        let cold = WaterPreset::ice_cold();
        let room = WaterPreset::distilled();
        let hot = WaterPreset::hot();

        // Viscosity decreases with temperature
        assert!(cold.viscosity > room.viscosity);
        assert!(room.viscosity > hot.viscosity);
    }

    #[test]
    fn test_sea_water() {
        let sea = WaterPreset::sea_water();
        let fresh = WaterPreset::distilled();

        // Sea water is denser
        assert!(sea.density > fresh.density);
    }

    #[test]
    fn test_kinematic_viscosity() {
        let water = WaterPreset::default();
        let nu = water.kinematic_viscosity();

        // Should be around 1e-6 m²/s for water at 20°C
        assert!(nu > 9e-7 && nu < 1.1e-6);
    }

    #[test]
    fn test_reynolds_number() {
        let water = WaterPreset::default();
        let re = water.reynolds_number(1.0, 1.0); // 1 m/s, 1 m length

        // Should be around 1 million for water
        assert!(re > 900_000.0 && re < 1_100_000.0);
    }
}
