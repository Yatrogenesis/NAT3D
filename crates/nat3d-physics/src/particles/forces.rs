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

//! Forces affecting particles (gravity, wind, turbulence, etc.).

use nalgebra::{Point3, Vector3};
use rand::Rng;

/// Force field affecting particles.
#[derive(Debug, Clone)]
pub enum ForceField {
    /// Constant directional force (e.g., gravity, wind).
    Constant(Vector3<f64>),
    /// Point attractor/repulsor.
    Point {
        /// Center position of the force field.
        position: Point3<f64>,
        /// Force strength (positive = attract, negative = repel).
        strength: f64,
        /// Maximum distance for force influence.
        max_distance: f64,
    },
    /// Vortex/tornado force.
    Vortex {
        /// Start point of vortex axis.
        axis_start: Point3<f64>,
        /// Direction vector of vortex axis (normalized).
        axis_direction: Vector3<f64>,
        /// Rotational strength.
        strength: f64,
        /// Vortex radius.
        radius: f64,
    },
    /// Turbulence noise.
    Turbulence {
        /// Turbulence strength.
        strength: f64,
        /// Noise frequency.
        frequency: f64,
    },
    /// Drag force (air resistance).
    Drag {
        /// Drag coefficient.
        coefficient: f64,
    },
}

impl ForceField {
    /// Create gravity force field.
    pub fn gravity(strength: f64) -> Self {
        ForceField::Constant(Vector3::new(0.0, -strength, 0.0))
    }

    /// Create wind force field.
    pub fn wind(direction: Vector3<f64>, strength: f64) -> Self {
        ForceField::Constant(direction.normalize() * strength)
    }

    /// Compute force at given position and velocity.
    pub fn compute_force(&self, position: &Point3<f64>, velocity: &Vector3<f64>) -> Vector3<f64> {
        match self {
            ForceField::Constant(force) => *force,

            ForceField::Point {
                position: center,
                strength,
                max_distance,
            } => {
                let diff = center - position;
                let dist = diff.norm();

                if dist > *max_distance || dist < 1e-6 {
                    Vector3::zeros()
                } else {
                    let direction = diff / dist;
                    let falloff = 1.0 - (dist / max_distance).min(1.0);
                    direction * (*strength) * falloff / (dist * dist).max(0.1)
                }
            }

            ForceField::Vortex {
                axis_start,
                axis_direction,
                strength,
                radius,
            } => {
                // Project particle position onto axis
                let to_particle = position - axis_start;
                let proj = to_particle.dot(axis_direction) * axis_direction;
                let radial = to_particle - proj;
                let dist = radial.norm();

                if dist > *radius || dist < 1e-6 {
                    Vector3::zeros()
                } else {
                    let falloff = 1.0 - (dist / radius).min(1.0);
                    let tangent = axis_direction.cross(&radial).normalize();
                    tangent * (*strength) * falloff
                }
            }

            ForceField::Turbulence {
                strength,
                frequency: _,
            } => {
                // Simple noise-based turbulence (simplified)
                let mut rng = rand::rng();
                Vector3::new(
                    rng.random_range(-1.0..1.0),
                    rng.random_range(-1.0..1.0),
                    rng.random_range(-1.0..1.0),
                ) * (*strength)
            }

            ForceField::Drag { coefficient } => -velocity * (*coefficient),
        }
    }
}

/// Collection of force fields.
pub struct ForceFieldCollection {
    /// Active force fields.
    pub fields: Vec<ForceField>,
}

impl Default for ForceFieldCollection {
    fn default() -> Self {
        Self {
            fields: vec![ForceField::gravity(9.81)],
        }
    }
}

impl ForceFieldCollection {
    /// Create empty collection.
    pub fn new() -> Self {
        Self { fields: Vec::new() }
    }

    /// Add a force field.
    pub fn add(&mut self, field: ForceField) {
        self.fields.push(field);
    }

    /// Compute total force at position/velocity.
    pub fn total_force(&self, position: &Point3<f64>, velocity: &Vector3<f64>) -> Vector3<f64> {
        self.fields
            .iter()
            .map(|field| field.compute_force(position, velocity))
            .sum()
    }

    /// Apply forces to particle.
    pub fn apply(&self, position: &Point3<f64>, velocity: &mut Vector3<f64>, dt: f64) {
        let force = self.total_force(position, velocity);
        *velocity += force * dt;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gravity_force() {
        let gravity = ForceField::gravity(9.81);
        let pos = Point3::origin();
        let vel = Vector3::zeros();
        let force = gravity.compute_force(&pos, &vel);

        assert_eq!(force.y, -9.81);
        assert_eq!(force.x, 0.0);
        assert_eq!(force.z, 0.0);
    }

    #[test]
    fn test_point_attractor() {
        let attractor = ForceField::Point {
            position: Point3::new(0.0, 0.0, 0.0),
            strength: 10.0,
            max_distance: 100.0,
        };

        let pos = Point3::new(1.0, 0.0, 0.0);
        let vel = Vector3::zeros();
        let force = attractor.compute_force(&pos, &vel);

        // Should attract towards origin
        assert!(force.x < 0.0);
    }

    #[test]
    fn test_drag_force() {
        let drag = ForceField::Drag { coefficient: 0.1 };
        let pos = Point3::origin();
        let vel = Vector3::new(10.0, 0.0, 0.0);
        let force = drag.compute_force(&pos, &vel);

        // Should oppose velocity
        assert!(force.x < 0.0);
        assert_eq!(force.x, -1.0);
    }

    #[test]
    fn test_force_collection() {
        let mut collection = ForceFieldCollection::new();
        collection.add(ForceField::gravity(9.81));
        collection.add(ForceField::Drag { coefficient: 0.05 });

        let pos = Point3::origin();
        let mut vel = Vector3::new(0.0, 10.0, 0.0);

        collection.apply(&pos, &mut vel, 0.1);

        // Velocity should have changed due to forces
        assert!(vel.y < 10.0); // Gravity pulls down, drag slows
    }
}
