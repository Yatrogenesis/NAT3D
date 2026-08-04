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

//! Apple Pencil input handling.
//!
//! Handles pressure-sensitive input with tilt and azimuth.

use std::time::Instant;

/// Apple Pencil event.
#[derive(Debug, Clone)]
pub struct PencilEvent {
    pub position: (f32, f32),
    pub pressure: f32,
    pub altitude_angle: f32,
    pub azimuth_angle: f32,
    pub timestamp: u64,
}

impl PencilEvent {
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            position: (x, y),
            pressure: 1.0,
            altitude_angle: std::f32::consts::FRAC_PI_2,
            azimuth_angle: 0.0,
            timestamp: Self::current_timestamp(),
        }
    }

    pub fn with_pressure(mut self, pressure: f32) -> Self {
        self.pressure = pressure.clamp(0.0, 1.0);
        self
    }

    pub fn with_altitude(mut self, altitude: f32) -> Self {
        self.altitude_angle = altitude;
        self
    }

    pub fn with_azimuth(mut self, azimuth: f32) -> Self {
        self.azimuth_angle = azimuth;
        self
    }

    fn current_timestamp() -> u64 {
        Instant::now().elapsed().as_millis() as u64
    }
}

/// Apple Pencil input handler.
pub struct PencilInput {
    pressure_curve: PressureCurve,
    tilt_sensitivity: f32,
    last_event: Option<PencilEvent>,
}

impl PencilInput {
    /// Create a new Pencil input handler.
    pub fn new() -> Self {
        Self {
            pressure_curve: PressureCurve::Linear,
            tilt_sensitivity: 1.0,
            last_event: None,
        }
    }

    /// Handle a pencil event.
    pub fn handle_pencil_event(&mut self, event: PencilEvent) -> BrushParams {
        let params = self.map_to_brush_params(&event);
        self.last_event = Some(event);
        params
    }

    /// Map pencil event to brush parameters.
    pub fn map_to_brush_params(&self, event: &PencilEvent) -> BrushParams {
        let pressure = self.apply_pressure_curve(event.pressure);
        let size = self.pressure_to_size(pressure);
        let opacity = self.pressure_to_opacity(pressure);
        let tilt = self.calculate_tilt(event);

        BrushParams {
            position: event.position,
            size,
            opacity,
            tilt,
            rotation: event.azimuth_angle,
        }
    }

    /// Set pressure curve.
    pub fn set_pressure_curve(&mut self, curve: PressureCurve) {
        self.pressure_curve = curve;
    }

    /// Set tilt sensitivity (0.0 to 2.0).
    pub fn set_tilt_sensitivity(&mut self, sensitivity: f32) {
        self.tilt_sensitivity = sensitivity.clamp(0.0, 2.0);
    }

    /// Get last pencil event.
    pub fn last_event(&self) -> Option<&PencilEvent> {
        self.last_event.as_ref()
    }

    fn apply_pressure_curve(&self, pressure: f32) -> f32 {
        match self.pressure_curve {
            PressureCurve::Linear => pressure,
            PressureCurve::Soft => pressure.powf(0.5),
            PressureCurve::Hard => pressure.powf(2.0),
            PressureCurve::Custom(exponent) => pressure.powf(exponent),
        }
    }

    fn pressure_to_size(&self, pressure: f32) -> f32 {
        // Map pressure to brush size (1.0 to 50.0 pixels)
        1.0 + pressure * 49.0
    }

    fn pressure_to_opacity(&self, pressure: f32) -> f32 {
        // Map pressure to opacity (0.1 to 1.0)
        0.1 + pressure * 0.9
    }

    fn calculate_tilt(&self, event: &PencilEvent) -> f32 {
        // Convert altitude angle to tilt (0 = flat, 1 = perpendicular)
        let tilt =
            (std::f32::consts::FRAC_PI_2 - event.altitude_angle) / std::f32::consts::FRAC_PI_2;
        tilt * self.tilt_sensitivity
    }
}

impl Default for PencilInput {
    fn default() -> Self {
        Self::new()
    }
}

/// Pressure curve types.
#[derive(Debug, Clone, Copy)]
pub enum PressureCurve {
    Linear,
    Soft,
    Hard,
    Custom(f32),
}

/// Brush parameters derived from pencil input.
#[derive(Debug, Clone, Copy)]
pub struct BrushParams {
    pub position: (f32, f32),
    pub size: f32,
    pub opacity: f32,
    pub tilt: f32,
    pub rotation: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pencil_event_creation() {
        let event = PencilEvent::new(100.0, 200.0);
        assert_eq!(event.position, (100.0, 200.0));
        assert_eq!(event.pressure, 1.0);
    }

    #[test]
    fn test_pencil_event_with_pressure() {
        let event = PencilEvent::new(100.0, 200.0).with_pressure(0.5);

        assert_eq!(event.pressure, 0.5);
    }

    #[test]
    fn test_pencil_event_with_angles() {
        let event = PencilEvent::new(100.0, 200.0)
            .with_altitude(0.5)
            .with_azimuth(1.0);

        assert_eq!(event.altitude_angle, 0.5);
        assert_eq!(event.azimuth_angle, 1.0);
    }

    #[test]
    fn test_pencil_input_creation() {
        let input = PencilInput::new();
        assert!(matches!(input.pressure_curve, PressureCurve::Linear));
        assert_eq!(input.tilt_sensitivity, 1.0);
    }

    #[test]
    fn test_handle_pencil_event() {
        let mut input = PencilInput::new();
        let event = PencilEvent::new(100.0, 200.0).with_pressure(0.8);

        let params = input.handle_pencil_event(event);
        assert_eq!(params.position, (100.0, 200.0));
        assert!(params.size > 1.0);
        assert!(params.opacity > 0.1);
    }

    #[test]
    fn test_pressure_curves() {
        let mut input = PencilInput::new();

        input.set_pressure_curve(PressureCurve::Linear);
        let linear = input.apply_pressure_curve(0.5);
        assert_eq!(linear, 0.5);

        input.set_pressure_curve(PressureCurve::Soft);
        let soft = input.apply_pressure_curve(0.25);
        assert!(soft > 0.25);

        input.set_pressure_curve(PressureCurve::Hard);
        let hard = input.apply_pressure_curve(0.5);
        assert_eq!(hard, 0.25);
    }

    #[test]
    fn test_tilt_sensitivity() {
        let mut input = PencilInput::new();
        input.set_tilt_sensitivity(2.0);
        assert_eq!(input.tilt_sensitivity, 2.0);

        // Test clamping
        input.set_tilt_sensitivity(5.0);
        assert_eq!(input.tilt_sensitivity, 2.0);
    }

    #[test]
    fn test_last_event() {
        let mut input = PencilInput::new();
        assert!(input.last_event().is_none());

        let event = PencilEvent::new(100.0, 200.0);
        input.handle_pencil_event(event);

        assert!(input.last_event().is_some());
    }

    #[test]
    fn test_brush_params_mapping() {
        let input = PencilInput::new();
        let event = PencilEvent::new(100.0, 200.0).with_pressure(0.5);

        let params = input.map_to_brush_params(&event);

        assert_eq!(params.position, (100.0, 200.0));
        assert!(params.size >= 1.0 && params.size <= 50.0);
        assert!(params.opacity >= 0.1 && params.opacity <= 1.0);
    }
}
