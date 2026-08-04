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

//! Graphics tablet input handling (Wacom, Huion, XP-Pen, etc).
//!
//! Provides pressure-sensitive input for drawing tablets and styluses.

use nalgebra::Vector2;

/// Tablet pen/stylus state.
#[derive(Debug, Clone)]
pub struct TabletState {
    /// Current pen position (screen coordinates).
    pub position: Vector2<f32>,
    /// Pressure level (0.0 = no pressure, 1.0 = maximum pressure).
    pub pressure: f32,
    /// Pen tilt angle from vertical (0.0 = perpendicular, 1.0 = fully tilted).
    pub tilt: f32,
    /// Pen rotation around its axis (radians, 0 to 2π).
    pub rotation: f32,
    /// Whether pen is currently touching the tablet surface.
    pub in_contact: bool,
    /// Whether pen is in proximity (hovering) above tablet.
    pub in_proximity: bool,
    /// Pen button states (typically 2 side buttons).
    pub buttons: [bool; 2],
}

impl Default for TabletState {
    fn default() -> Self {
        Self {
            position: Vector2::zeros(),
            pressure: 0.0,
            tilt: 0.0,
            rotation: 0.0,
            in_contact: false,
            in_proximity: false,
            buttons: [false; 2],
        }
    }
}

impl TabletState {
    /// Create new tablet state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Update pen position.
    pub fn set_position(&mut self, x: f32, y: f32) {
        self.position = Vector2::new(x, y);
    }

    /// Set pressure level (clamped to 0.0-1.0).
    pub fn set_pressure(&mut self, pressure: f32) {
        self.pressure = pressure.clamp(0.0, 1.0);
    }

    /// Set tilt angle (clamped to 0.0-1.0).
    pub fn set_tilt(&mut self, tilt: f32) {
        self.tilt = tilt.clamp(0.0, 1.0);
    }

    /// Set rotation angle (wrapped to 0-2π).
    pub fn set_rotation(&mut self, rotation: f32) {
        self.rotation = rotation.rem_euclid(std::f32::consts::TAU);
    }

    /// Set contact state.
    pub fn set_contact(&mut self, in_contact: bool) {
        self.in_contact = in_contact;
    }

    /// Set proximity state.
    pub fn set_proximity(&mut self, in_proximity: bool) {
        self.in_proximity = in_proximity;
    }

    /// Set pen button state.
    pub fn set_button(&mut self, index: usize, pressed: bool) {
        if index < 2 {
            self.buttons[index] = pressed;
        }
    }

    /// Check if any pen button is pressed.
    pub fn any_button_pressed(&self) -> bool {
        self.buttons.iter().any(|&b| b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pressure_clamping() {
        let mut tablet = TabletState::new();

        tablet.set_pressure(1.5);
        assert_eq!(tablet.pressure, 1.0);

        tablet.set_pressure(-0.5);
        assert_eq!(tablet.pressure, 0.0);

        tablet.set_pressure(0.7);
        assert_eq!(tablet.pressure, 0.7);
    }

    #[test]
    fn test_rotation_wrapping() {
        let mut tablet = TabletState::new();

        tablet.set_rotation(std::f32::consts::TAU + 1.0);
        assert!((tablet.rotation - 1.0).abs() < 0.001);

        tablet.set_rotation(-1.0);
        assert!((tablet.rotation - (std::f32::consts::TAU - 1.0)).abs() < 0.001);
    }
}
