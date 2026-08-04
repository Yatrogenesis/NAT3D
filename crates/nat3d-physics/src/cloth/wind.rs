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

//! Wind simulation for cloth.
//!
//! Implements various wind models including directional, point source, vortex, and turbulent wind.

use nalgebra::Vector3;

/// Wind source types.
#[derive(Debug, Clone, Copy)]
pub enum WindSource {
    /// Directional wind (like natural wind).
    Directional,
    /// Point source (like a fan).
    Point {
        /// Position of the point source.
        position: Vector3<f64>,
    },
    /// Vortex wind (tornado-like).
    Vortex {
        /// Center of the vortex.
        center: Vector3<f64>,
        /// Axis of the vortex rotation.
        axis: Vector3<f64>,
    },
    /// Turbulent wind with Perlin noise.
    Turbulent,
}

/// Wind field configuration.
#[derive(Debug, Clone)]
pub struct WindField {
    /// Primary wind direction (for directional wind).
    pub direction: Vector3<f64>,
    /// Wind strength/speed.
    pub strength: f64,
    /// Turbulence intensity (0-1).
    pub turbulence: f64,
    /// Gustiness (periodic variations).
    pub gustiness: f64,
    /// Wind source type.
    pub source: WindSource,
    /// Simulation time (for time-varying effects).
    pub time: f64,
    /// Noise seed for turbulence.
    noise_seed: u64,
}

impl WindField {
    /// Create a new directional wind field.
    pub fn new(direction: Vector3<f64>, strength: f64) -> Self {
        Self {
            direction: direction.normalize(),
            strength,
            turbulence: 0.1,
            gustiness: 0.05,
            source: WindSource::Directional,
            time: 0.0,
            noise_seed: 12345,
        }
    }

    /// Create a point source wind (fan).
    pub fn point_source(position: Vector3<f64>, strength: f64) -> Self {
        Self {
            direction: Vector3::zeros(),
            strength,
            turbulence: 0.1,
            gustiness: 0.05,
            source: WindSource::Point { position },
            time: 0.0,
            noise_seed: 12345,
        }
    }

    /// Create a vortex wind.
    pub fn vortex(center: Vector3<f64>, axis: Vector3<f64>, strength: f64) -> Self {
        Self {
            direction: axis.normalize(),
            strength,
            turbulence: 0.1,
            gustiness: 0.05,
            source: WindSource::Vortex {
                center,
                axis: axis.normalize(),
            },
            time: 0.0,
            noise_seed: 12345,
        }
    }

    /// Create turbulent wind.
    pub fn turbulent(direction: Vector3<f64>, strength: f64, turbulence: f64) -> Self {
        Self {
            direction: direction.normalize(),
            strength,
            turbulence,
            gustiness: 0.05,
            source: WindSource::Turbulent,
            time: 0.0,
            noise_seed: 12345,
        }
    }

    /// Set turbulence parameters.
    pub fn with_turbulence(mut self, turbulence: f64, gustiness: f64) -> Self {
        self.turbulence = turbulence;
        self.gustiness = gustiness;
        self
    }

    /// Update wind field (advance time).
    pub fn update(&mut self, dt: f64) {
        self.time += dt;
    }

    /// Compute wind force at a position with surface normal and velocity.
    pub fn compute_wind_force(
        &self,
        position: Vector3<f64>,
        normal: Vector3<f64>,
        velocity: Vector3<f64>,
    ) -> Vector3<f64> {
        // Get base wind velocity at position
        let wind_velocity = self.compute_wind_velocity(position);

        // Relative velocity (wind - object)
        let relative_velocity = wind_velocity - velocity;

        // Wind force depends on surface orientation
        // F = 0.5 * rho * Cd * A * (v · n)^2 * n
        // Simplified: F = k * (v · n) * |v| * n
        let v_dot_n = relative_velocity.dot(&normal);

        if v_dot_n > 0.0 {
            // Wind hitting surface
            let force_magnitude = self.strength * v_dot_n * relative_velocity.magnitude();
            normal * force_magnitude
        } else {
            Vector3::zeros()
        }
    }

    /// Compute wind velocity at a position.
    fn compute_wind_velocity(&self, position: Vector3<f64>) -> Vector3<f64> {
        let base_velocity = match self.source {
            WindSource::Directional => self.direction * self.strength,

            WindSource::Point { position: source } => {
                let delta = position - source;
                let dist = delta.magnitude();

                if dist > 1e-10 {
                    // Wind falls off with distance
                    let falloff = 1.0 / (1.0 + dist * dist);
                    delta.normalize() * self.strength * falloff
                } else {
                    Vector3::zeros()
                }
            }

            WindSource::Vortex { center, axis } => {
                let delta = position - center;

                // Component perpendicular to axis
                let parallel = delta.dot(&axis) * axis;
                let perpendicular = delta - parallel;
                let r = perpendicular.magnitude();

                if r > 1e-10 {
                    // Tangential velocity (cross product)
                    let tangent = axis.cross(&perpendicular.normalize());

                    // Velocity falls off with distance
                    let v_tangent = self.strength / (1.0 + r);

                    // Add upward component
                    let v_upward = self.strength * 0.3;

                    tangent * v_tangent + axis * v_upward
                } else {
                    axis * self.strength * 0.3
                }
            }

            WindSource::Turbulent => self.direction * self.strength,
        };

        // Add turbulence
        let turbulent_velocity = self.compute_turbulence(position);

        // Add gustiness (time-varying)
        let gust = self.compute_gust();

        base_velocity + turbulent_velocity + gust
    }

    /// Compute turbulence using simplified Perlin-like noise.
    fn compute_turbulence(&self, position: Vector3<f64>) -> Vector3<f64> {
        if self.turbulence < 1e-6 {
            return Vector3::zeros();
        }

        // Simplified noise (pseudo-random based on position and time)
        let freq = 0.1;
        let scale = self.turbulence * self.strength;

        let noise_x = self.noise(position.x * freq, position.y * freq, self.time * 0.5);
        let noise_y = self.noise(position.y * freq, position.z * freq, self.time * 0.5);
        let noise_z = self.noise(position.z * freq, position.x * freq, self.time * 0.5);

        Vector3::new(noise_x, noise_y, noise_z) * scale
    }

    /// Compute gust (periodic variation).
    fn compute_gust(&self) -> Vector3<f64> {
        if self.gustiness < 1e-6 {
            return Vector3::zeros();
        }

        let gust_freq = 0.5;
        let gust_phase = self.time * gust_freq;

        let gust_magnitude =
            (gust_phase * 2.0 * std::f64::consts::PI).sin() * self.gustiness * self.strength;

        self.direction * gust_magnitude
    }

    /// Simplified noise function (pseudo-Perlin).
    fn noise(&self, x: f64, y: f64, z: f64) -> f64 {
        // Hash coordinates
        let ix = x.floor() as i64;
        let iy = y.floor() as i64;
        let iz = z.floor() as i64;

        let fx = x - ix as f64;
        let fy = y - iy as f64;
        let fz = z - iz as f64;

        // Smooth step
        let u = fx * fx * (3.0 - 2.0 * fx);
        let v = fy * fy * (3.0 - 2.0 * fy);
        let w = fz * fz * (3.0 - 2.0 * fz);

        // Hash corners
        let h000 = self.hash3d(ix, iy, iz);
        let h100 = self.hash3d(ix + 1, iy, iz);
        let h010 = self.hash3d(ix, iy + 1, iz);
        let h110 = self.hash3d(ix + 1, iy + 1, iz);
        let h001 = self.hash3d(ix, iy, iz + 1);
        let h101 = self.hash3d(ix + 1, iy, iz + 1);
        let h011 = self.hash3d(ix, iy + 1, iz + 1);
        let h111 = self.hash3d(ix + 1, iy + 1, iz + 1);

        // Trilinear interpolation
        let x00 = Self::lerp(h000, h100, u);
        let x01 = Self::lerp(h001, h101, u);
        let x10 = Self::lerp(h010, h110, u);
        let x11 = Self::lerp(h011, h111, u);

        let y0 = Self::lerp(x00, x10, v);
        let y1 = Self::lerp(x01, x11, v);

        Self::lerp(y0, y1, w) * 2.0 - 1.0
    }

    /// Hash function for 3D coordinates.
    fn hash3d(&self, x: i64, y: i64, z: i64) -> f64 {
        let h = (x
            .wrapping_mul(374761393)
            .wrapping_add(y.wrapping_mul(668265263))
            .wrapping_add(z.wrapping_mul(1274126177))
            .wrapping_add(self.noise_seed as i64)) as u64;

        (h % 1000000) as f64 / 1000000.0
    }

    /// Linear interpolation.
    fn lerp(a: f64, b: f64, t: f64) -> f64 {
        a * (1.0 - t) + b * t
    }
}

impl Default for WindField {
    fn default() -> Self {
        Self::new(Vector3::new(1.0, 0.0, 0.0), 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_directional_wind() {
        let wind = WindField::new(Vector3::new(1.0, 0.0, 0.0), 10.0);

        let position = Vector3::zeros();
        let normal = Vector3::new(1.0, 0.0, 0.0);
        let velocity = Vector3::zeros();

        let force = wind.compute_wind_force(position, normal, velocity);

        // Force should be in positive X direction
        assert!(force.x > 0.0);
    }

    #[test]
    fn test_point_source_wind() {
        let source_pos = Vector3::zeros();
        let wind = WindField::point_source(source_pos, 10.0);

        let position = Vector3::new(1.0, 0.0, 0.0);
        let normal = Vector3::new(1.0, 0.0, 0.0);
        let velocity = Vector3::zeros();

        let force = wind.compute_wind_force(position, normal, velocity);

        // Force should push away from source
        assert!(force.magnitude() > 0.0);
    }

    #[test]
    fn test_vortex_wind() {
        let center = Vector3::zeros();
        let axis = Vector3::new(0.0, 1.0, 0.0);
        let wind = WindField::vortex(center, axis, 10.0);

        let position = Vector3::new(1.0, 0.0, 0.0);
        let normal = Vector3::new(0.0, 1.0, 0.0); // Normal facing up for vortex
        let velocity = Vector3::zeros();

        let force = wind.compute_wind_force(position, normal, velocity);

        // Should have some force
        assert!(force.magnitude() > 0.0);
    }

    #[test]
    fn test_turbulent_wind() {
        let wind = WindField::turbulent(Vector3::new(1.0, 0.0, 0.0), 10.0, 0.5);

        let position = Vector3::new(1.0, 2.0, 3.0);
        let normal = Vector3::new(1.0, 0.0, 0.0);
        let velocity = Vector3::zeros();

        let force = wind.compute_wind_force(position, normal, velocity);

        // Should have force
        assert!(force.magnitude() > 0.0);
    }

    #[test]
    fn test_wind_update() {
        let mut wind = WindField::new(Vector3::new(1.0, 0.0, 0.0), 10.0);

        let initial_time = wind.time;
        wind.update(0.1);

        assert_eq!(wind.time, initial_time + 0.1);
    }

    #[test]
    fn test_no_force_on_back_face() {
        let wind = WindField::new(Vector3::new(1.0, 0.0, 0.0), 10.0);

        let position = Vector3::zeros();
        let normal = Vector3::new(-1.0, 0.0, 0.0); // Facing away from wind
        let velocity = Vector3::zeros();

        let force = wind.compute_wind_force(position, normal, velocity);

        // No force on back face
        assert_eq!(force.magnitude(), 0.0);
    }
}
