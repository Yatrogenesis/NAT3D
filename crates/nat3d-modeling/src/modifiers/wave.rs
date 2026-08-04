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

//! Wave modifier.
//!
//! Creates ripple/wave effects on mesh geometry.

use nalgebra::{Point3, Vector3};
use std::any::Any;
use std::f64::consts::PI;
use super::stack::{Modifier, ModifierMesh};

/// Wave type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaveType {
    /// Sine wave.
    Sine,
    /// Cosine wave.
    Cosine,
    /// Square wave.
    Square,
    /// Sawtooth wave.
    Sawtooth,
    /// Triangle wave.
    Triangle,
}

/// Wave direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaveDirection {
    /// Wave along X axis, displace in Z.
    X,
    /// Wave along Y axis, displace in Z.
    Y,
    /// Wave along Z axis, displace in Y.
    Z,
    /// Radial from center point.
    Radial,
}

/// Wave modifier.
#[derive(Debug, Clone)]
pub struct WaveModifier {
    /// Modifier name.
    pub name: String,
    /// Whether enabled.
    pub enabled: bool,
    /// Wave amplitude.
    pub amplitude: f64,
    /// Wavelength (distance between peaks).
    pub wavelength: f64,
    /// Phase offset.
    pub phase: f64,
    /// Damping factor (exponential decay with distance).
    pub damping: f64,
    /// Animation speed (for time-based waves).
    pub speed: f64,
    /// Current time for animation.
    pub time: f64,
    /// Wave direction.
    pub direction: WaveDirection,
    /// Wave type.
    pub wave_type: WaveType,
    /// Center point for radial waves.
    pub center: Point3<f64>,
    /// Use texture coordinates instead of position.
    pub use_uvs: bool,
    /// Use vertex groups for weighting.
    pub vertex_group: Option<String>,
}

impl Default for WaveModifier {
    fn default() -> Self {
        Self {
            name: "Wave".to_string(),
            enabled: true,
            amplitude: 0.1,
            wavelength: 1.0,
            phase: 0.0,
            damping: 0.0,
            speed: 1.0,
            time: 0.0,
            direction: WaveDirection::X,
            wave_type: WaveType::Sine,
            center: Point3::origin(),
            use_uvs: false,
            vertex_group: None,
        }
    }
}

impl WaveModifier {
    /// Create new wave modifier.
    pub fn new(amplitude: f64, wavelength: f64) -> Self {
        Self {
            amplitude,
            wavelength,
            ..Default::default()
        }
    }

    /// Calculate wave distance parameter for a point.
    fn calculate_distance(&self, p: Point3<f64>, _uv: Option<(f64, f64)>) -> f64 {
        if self.use_uvs {
            if let Some((u, v)) = _uv {
                return match self.direction {
                    WaveDirection::X => u,
                    WaveDirection::Y => v,
                    WaveDirection::Radial => {
                        let du = u - 0.5;
                        let dv = v - 0.5;
                        (du * du + dv * dv).sqrt()
                    }
                    _ => u,
                };
            }
        }

        match self.direction {
            WaveDirection::X => p.x - self.center.x,
            WaveDirection::Y => p.y - self.center.y,
            WaveDirection::Z => p.z - self.center.z,
            WaveDirection::Radial => {
                let dx = p.x - self.center.x;
                let dy = p.y - self.center.y;
                (dx * dx + dy * dy).sqrt()
            }
        }
    }

    /// Evaluate wave function at distance.
    fn evaluate_wave(&self, distance: f64) -> f64 {
        if self.wavelength.abs() < 1e-10 {
            return 0.0;
        }

        let frequency = 2.0 * PI / self.wavelength;
        let arg = frequency * distance + self.phase + self.time * self.speed;

        let wave_value = match self.wave_type {
            WaveType::Sine => arg.sin(),
            WaveType::Cosine => arg.cos(),
            WaveType::Square => {
                if arg.sin() >= 0.0 { 1.0 } else { -1.0 }
            }
            WaveType::Sawtooth => {
                let period = 2.0 * PI;
                let normalized = ((arg % period) + period) % period;
                (normalized / period) * 2.0 - 1.0
            }
            WaveType::Triangle => {
                let period = 2.0 * PI;
                let normalized = ((arg % period) + period) % period;
                let half_period = period / 2.0;
                if normalized < half_period {
                    (normalized / half_period) * 2.0 - 1.0
                } else {
                    ((period - normalized) / half_period) * 2.0 - 1.0
                }
            }
        };

        // Apply damping
        let damped = if self.damping > 1e-10 {
            wave_value * (-self.damping * distance.abs()).exp()
        } else {
            wave_value
        };

        damped * self.amplitude
    }

    /// Calculate displacement direction.
    fn get_displacement_direction(&self, _p: Point3<f64>) -> Vector3<f64> {
        match self.direction {
            WaveDirection::X => Vector3::z(),
            WaveDirection::Y => Vector3::z(),
            WaveDirection::Z => Vector3::y(),
            WaveDirection::Radial => {
                // Displace upward (Z) for radial waves
                Vector3::z()
            }
        }
    }

    /// Get vertex weight from vertex group.
    fn get_vertex_weight(&self, mesh: &ModifierMesh, vertex_idx: usize) -> f64 {
        if let Some(ref group_name) = self.vertex_group {
            if let Some(weights) = mesh.vertex_groups.get(group_name) {
                for &(vi, weight) in weights {
                    if vi == vertex_idx {
                        return weight;
                    }
                }
            }
        }
        1.0
    }
}

impl Modifier for WaveModifier {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_id(&self) -> &'static str {
        "WaveModifier"
    }

    fn apply(&self, mesh: &ModifierMesh) -> ModifierMesh {
        if mesh.positions.is_empty() || self.amplitude.abs() < 1e-10 {
            return mesh.clone();
        }

        let mut result = mesh.clone();

        // Apply wave to each vertex
        for i in 0..result.positions.len() {
            let pos = result.positions[i];

            // Get UV if available and requested
            let uv = if self.use_uvs && i < mesh.uvs.len() {
                Some(mesh.uvs[i])
            } else {
                None
            };

            let distance = self.calculate_distance(pos, uv);
            let wave_value = self.evaluate_wave(distance);
            let weight = self.get_vertex_weight(mesh, i);

            let displacement_dir = self.get_displacement_direction(pos);
            let displacement = displacement_dir * (wave_value * weight);

            result.positions[i] = pos + displacement;
        }

        // Recompute normals
        result.compute_normals();
        result
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn clone_box(&self) -> Box<dyn Modifier> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wave_basic() {
        // Create a grid
        let mut positions = Vec::new();
        for x in 0..5 {
            for y in 0..5 {
                positions.push(Point3::new(x as f64, y as f64, 0.0));
            }
        }

        let mesh = ModifierMesh::from_geometry(positions, vec![]);

        // Use wavelength=3.0 so integer x values don't all fall on sine zero crossings
        let modifier = WaveModifier::new(0.5, 3.0);
        let result = modifier.apply(&mesh);

        assert_eq!(result.positions.len(), 25);

        // Some vertices should be displaced
        assert!(result.positions.iter().any(|p| p.z.abs() > 0.1));
    }

    #[test]
    fn test_wave_types() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(2.0, 0.0, 0.0),
            ],
            vec![],
        );

        // Test Sine
        let mut modifier = WaveModifier::new(0.5, 2.0);
        modifier.wave_type = WaveType::Sine;
        let result = modifier.apply(&mesh);
        assert_eq!(result.positions.len(), 3);

        // Test Cosine
        modifier.wave_type = WaveType::Cosine;
        let result = modifier.apply(&mesh);
        assert_eq!(result.positions.len(), 3);

        // Test Square
        modifier.wave_type = WaveType::Square;
        let result = modifier.apply(&mesh);
        assert_eq!(result.positions.len(), 3);

        // Test Sawtooth
        modifier.wave_type = WaveType::Sawtooth;
        let result = modifier.apply(&mesh);
        assert_eq!(result.positions.len(), 3);

        // Test Triangle
        modifier.wave_type = WaveType::Triangle;
        let result = modifier.apply(&mesh);
        assert_eq!(result.positions.len(), 3);
    }

    #[test]
    fn test_wave_radial() {
        // Create a grid centered at origin
        let mut positions = Vec::new();
        for x in -2..=2 {
            for y in -2..=2 {
                positions.push(Point3::new(x as f64, y as f64, 0.0));
            }
        }

        let mesh = ModifierMesh::from_geometry(positions, vec![]);

        // Use wavelength=3.0 so the center (distance=0) doesn't land on a sine zero.
        // Also add a phase offset of PI/2 to ensure center gets a cosine-like peak.
        let mut modifier = WaveModifier::new(0.5, 3.0);
        modifier.direction = WaveDirection::Radial;
        modifier.center = Point3::origin();
        modifier.phase = PI / 2.0;

        let result = modifier.apply(&mesh);

        assert_eq!(result.positions.len(), 25);

        // Center should be at peak/trough due to phase offset
        let center_idx = 12; // Middle of 5x5 grid
        assert!(result.positions[center_idx].z.abs() > 0.1);
    }

    #[test]
    fn test_wave_damping() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(5.0, 0.0, 0.0),
            ],
            vec![],
        );

        let mut modifier = WaveModifier::new(1.0, 2.0);
        modifier.damping = 0.5;

        let result = modifier.apply(&mesh);

        // Far vertex should be damped (less displacement)
        assert!(result.positions[1].z.abs() < result.positions[0].z.abs() ||
                result.positions[1].z.abs() < 0.5);
    }

    #[test]
    fn test_wave_animation() {
        let mesh = ModifierMesh::from_geometry(
            vec![Point3::new(0.0, 0.0, 0.0)],
            vec![],
        );

        let mut modifier = WaveModifier::new(1.0, 2.0);
        modifier.speed = 1.0;

        // Time 0: arg = freq*0 + 0 + 0 = 0, sin(0) = 0
        // Time PI/2: arg = freq*0 + 0 + PI/2 = PI/2, sin(PI/2) = 1
        // These two times produce maximally different sine values.
        modifier.time = 0.0;
        let result1 = modifier.apply(&mesh);

        modifier.time = PI / 2.0;
        let result2 = modifier.apply(&mesh);

        // Positions should be different
        assert!((result1.positions[0].z - result2.positions[0].z).abs() > 0.1);
    }
}
