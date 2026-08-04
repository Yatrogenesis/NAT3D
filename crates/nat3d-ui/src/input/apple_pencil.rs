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

//! Apple Pencil specific input handling (iPad Pro, iPad Air).
//!
//! Extends tablet input with Apple Pencil-specific features:
//! - Force/pressure with extended range
//! - Tilt with X/Y components
//! - Azimuth (compass direction)
//! - Altitude (angle from surface)

use nalgebra::Vector2;

/// Apple Pencil state (extends standard tablet input).
#[derive(Debug, Clone)]
pub struct ApplePencilState {
    /// Current position (screen coordinates).
    pub position: Vector2<f32>,
    /// Force applied (0.0-1.0, more sensitive than standard pressure).
    pub force: f32,
    /// Tilt vector (X and Y components, 0.0-1.0 each).
    pub tilt: Vector2<f32>,
    /// Azimuth angle (compass direction, radians 0-2π).
    /// 0 = pointing up, π/2 = pointing right.
    pub azimuth: f32,
    /// Altitude angle from surface (radians, 0-π/2).
    /// 0 = parallel to surface, π/2 = perpendicular.
    pub altitude: f32,
    /// Whether pencil is touching the screen.
    pub in_contact: bool,
    /// Whether pencil is hovering (proximity).
    pub in_proximity: bool,
    /// Double-tap gesture detected (quick tap on pencil barrel).
    pub double_tap: bool,
}

impl Default for ApplePencilState {
    fn default() -> Self {
        Self {
            position: Vector2::zeros(),
            force: 0.0,
            tilt: Vector2::zeros(),
            azimuth: 0.0,
            altitude: std::f32::consts::FRAC_PI_2, // Default perpendicular
            in_contact: false,
            in_proximity: false,
            double_tap: false,
        }
    }
}

impl ApplePencilState {
    /// Create new Apple Pencil state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Update position.
    pub fn set_position(&mut self, x: f32, y: f32) {
        self.position = Vector2::new(x, y);
    }

    /// Set force (clamped to 0.0-1.0).
    pub fn set_force(&mut self, force: f32) {
        self.force = force.clamp(0.0, 1.0);
    }

    /// Set tilt vector (clamped to 0.0-1.0 per component).
    pub fn set_tilt(&mut self, tilt_x: f32, tilt_y: f32) {
        self.tilt = Vector2::new(tilt_x.clamp(0.0, 1.0), tilt_y.clamp(0.0, 1.0));
    }

    /// Set azimuth (wrapped to 0-2π).
    pub fn set_azimuth(&mut self, azimuth: f32) {
        self.azimuth = azimuth.rem_euclid(std::f32::consts::TAU);
    }

    /// Set altitude (clamped to 0-π/2).
    pub fn set_altitude(&mut self, altitude: f32) {
        self.altitude = altitude.clamp(0.0, std::f32::consts::FRAC_PI_2);
    }

    /// Set contact state.
    pub fn set_contact(&mut self, in_contact: bool) {
        self.in_contact = in_contact;
    }

    /// Set proximity state.
    pub fn set_proximity(&mut self, in_proximity: bool) {
        self.in_proximity = in_proximity;
    }

    /// Trigger double-tap gesture.
    pub fn trigger_double_tap(&mut self) {
        self.double_tap = true;
    }

    /// Clear double-tap (call after handling).
    pub fn clear_double_tap(&mut self) {
        self.double_tap = false;
    }

    /// Get tilt magnitude (0.0-1.414, diagonal maximum).
    pub fn tilt_magnitude(&self) -> f32 {
        self.tilt.norm()
    }

    /// Convert altitude/azimuth to 3D direction vector.
    /// Returns unit vector pointing in pencil direction.
    pub fn direction_vector(&self) -> (f32, f32, f32) {
        let x = self.altitude.sin() * self.azimuth.cos();
        let y = self.altitude.sin() * self.azimuth.sin();
        let z = self.altitude.cos();
        (x, y, z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_force_clamping() {
        let mut pencil = ApplePencilState::new();

        pencil.set_force(1.5);
        assert_eq!(pencil.force, 1.0);

        pencil.set_force(-0.5);
        assert_eq!(pencil.force, 0.0);
    }

    #[test]
    fn test_altitude_clamping() {
        let mut pencil = ApplePencilState::new();

        pencil.set_altitude(std::f32::consts::PI); // Beyond max
        assert!((pencil.altitude - std::f32::consts::FRAC_PI_2).abs() < 0.001);

        pencil.set_altitude(-1.0); // Below min
        assert_eq!(pencil.altitude, 0.0);
    }

    #[test]
    fn test_direction_vector() {
        let mut pencil = ApplePencilState::new();

        // Perpendicular (pointing straight down)
        pencil.set_altitude(0.0);
        let (x, y, z) = pencil.direction_vector();
        assert!((z - 1.0).abs() < 0.001);
        assert!(x.abs() < 0.001 && y.abs() < 0.001);

        // 45 degrees, pointing right
        pencil.set_altitude(std::f32::consts::FRAC_PI_4);
        pencil.set_azimuth(0.0);
        let (x, y, _z) = pencil.direction_vector();
        assert!((x * x + y * y).sqrt() - 0.707 < 0.01);
    }

    #[test]
    fn test_double_tap() {
        let mut pencil = ApplePencilState::new();

        assert!(!pencil.double_tap);

        pencil.trigger_double_tap();
        assert!(pencil.double_tap);

        pencil.clear_double_tap();
        assert!(!pencil.double_tap);
    }
}
