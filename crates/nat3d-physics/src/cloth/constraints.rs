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

//! Cloth constraints for position-based dynamics.
//!
//! Implements distance, bending, and collision constraints for cloth simulation.

use nalgebra::Vector3;

/// A distance constraint between two particles.
#[derive(Debug, Clone)]
pub struct DistanceConstraint {
    /// First particle index.
    pub p1: usize,
    /// Second particle index.
    pub p2: usize,
    /// Rest length.
    pub rest_length: f64,
    /// Stiffness (0-1).
    pub stiffness: f64,
    /// Constraint type (structural, shear, or bend).
    pub constraint_type: ConstraintType,
}

/// Type of distance constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintType {
    /// Structural constraint (horizontal/vertical neighbors).
    Structural,
    /// Shear constraint (diagonal neighbors).
    Shear,
    /// Bending constraint (skip one particle).
    Bend,
}

impl DistanceConstraint {
    /// Create a new distance constraint.
    pub fn new(
        p1: usize,
        p2: usize,
        rest_length: f64,
        stiffness: f64,
        constraint_type: ConstraintType,
    ) -> Self {
        Self {
            p1,
            p2,
            rest_length,
            stiffness,
            constraint_type,
        }
    }

    /// Create from particle positions.
    pub fn from_positions(
        p1: usize,
        p2: usize,
        pos1: Vector3<f64>,
        pos2: Vector3<f64>,
        stiffness: f64,
        constraint_type: ConstraintType,
    ) -> Self {
        let rest_length = (pos2 - pos1).magnitude();
        Self::new(p1, p2, rest_length, stiffness, constraint_type)
    }

    /// Solve the constraint using position-based dynamics.
    /// Returns the position corrections for both particles.
    pub fn solve(
        &self,
        pos1: Vector3<f64>,
        pos2: Vector3<f64>,
        inv_mass1: f64,
        inv_mass2: f64,
    ) -> (Vector3<f64>, Vector3<f64>) {
        let delta = pos2 - pos1;
        let dist = delta.magnitude();

        if dist < 1e-10 {
            return (Vector3::zeros(), Vector3::zeros());
        }

        let diff = (dist - self.rest_length) / dist;
        let direction = delta / dist;

        let total_inv_mass = inv_mass1 + inv_mass2;
        if total_inv_mass < 1e-10 {
            return (Vector3::zeros(), Vector3::zeros());
        }

        let correction = direction * diff * self.stiffness;

        let corr1 = correction * (inv_mass1 / total_inv_mass);
        let corr2 = -correction * (inv_mass2 / total_inv_mass);

        (corr1, corr2)
    }
}

/// Bending constraint using dihedral angle.
#[derive(Debug, Clone)]
pub struct BendingConstraint {
    /// First particle index (shared edge vertex).
    pub p1: usize,
    /// Second particle index (shared edge vertex).
    pub p2: usize,
    /// Third particle index (first triangle wing).
    pub p3: usize,
    /// Fourth particle index (second triangle wing).
    pub p4: usize,
    /// Rest angle.
    pub rest_angle: f64,
    /// Stiffness.
    pub stiffness: f64,
}

impl BendingConstraint {
    /// Create a new bending constraint.
    pub fn new(
        p1: usize,
        p2: usize,
        p3: usize,
        p4: usize,
        rest_angle: f64,
        stiffness: f64,
    ) -> Self {
        Self {
            p1,
            p2,
            p3,
            p4,
            rest_angle,
            stiffness,
        }
    }

    /// Compute dihedral angle between two triangles.
    pub fn compute_angle(
        pos1: Vector3<f64>,
        pos2: Vector3<f64>,
        pos3: Vector3<f64>,
        pos4: Vector3<f64>,
    ) -> f64 {
        // Triangle 1: p1, p2, p3
        // Triangle 2: p2, p1, p4
        // Shared edge: p1-p2

        let n1 = (pos2 - pos1).cross(&(pos3 - pos1)).normalize();
        let n2 = (pos1 - pos2).cross(&(pos4 - pos2)).normalize();

        let cos_angle = n1.dot(&n2).clamp(-1.0, 1.0);
        cos_angle.acos()
    }

    /// Create from particle positions.
    pub fn from_positions(
        p1: usize,
        p2: usize,
        p3: usize,
        p4: usize,
        pos1: Vector3<f64>,
        pos2: Vector3<f64>,
        pos3: Vector3<f64>,
        pos4: Vector3<f64>,
        stiffness: f64,
    ) -> Self {
        let rest_angle = Self::compute_angle(pos1, pos2, pos3, pos4);
        Self::new(p1, p2, p3, p4, rest_angle, stiffness)
    }
}

/// Collision constraint with a plane.
#[derive(Debug, Clone)]
pub struct PlaneCollisionConstraint {
    /// Plane point.
    pub point: Vector3<f64>,
    /// Plane normal (pointing away from collision).
    pub normal: Vector3<f64>,
    /// Friction coefficient.
    pub friction: f64,
}

impl PlaneCollisionConstraint {
    /// Create a new plane collision constraint.
    pub fn new(point: Vector3<f64>, normal: Vector3<f64>, friction: f64) -> Self {
        Self {
            point,
            normal: normal.normalize(),
            friction,
        }
    }

    /// Create a ground plane.
    pub fn ground(height: f64, friction: f64) -> Self {
        Self::new(
            Vector3::new(0.0, height, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
            friction,
        )
    }

    /// Solve collision for a particle.
    /// Returns the position correction.
    pub fn solve(&self, position: Vector3<f64>, velocity: &mut Vector3<f64>) -> Vector3<f64> {
        let d = (position - self.point).dot(&self.normal);

        if d < 0.0 {
            // Particle is below plane
            let correction = -d * self.normal;

            // Apply friction to tangential velocity
            let v_n = velocity.dot(&self.normal) * self.normal;
            let v_t = *velocity - v_n;

            if v_t.magnitude() > 1e-10 {
                *velocity = v_t * (1.0 - self.friction).max(0.0);
            }

            // Reflect normal velocity
            if velocity.dot(&self.normal) < 0.0 {
                *velocity -= v_n;
            }

            correction
        } else {
            Vector3::zeros()
        }
    }
}

/// Collision constraint with a sphere.
#[derive(Debug, Clone)]
pub struct SphereCollisionConstraint {
    /// Sphere center.
    pub center: Vector3<f64>,
    /// Sphere radius.
    pub radius: f64,
    /// Friction coefficient.
    pub friction: f64,
}

impl SphereCollisionConstraint {
    /// Create a new sphere collision constraint.
    pub fn new(center: Vector3<f64>, radius: f64, friction: f64) -> Self {
        Self {
            center,
            radius,
            friction,
        }
    }

    /// Solve collision for a particle.
    pub fn solve(&self, position: Vector3<f64>, velocity: &mut Vector3<f64>) -> Vector3<f64> {
        let diff = position - self.center;
        let dist = diff.magnitude();

        if dist < self.radius && dist > 1e-10 {
            let normal = diff / dist;
            let penetration = self.radius - dist;
            let correction = normal * penetration;

            // Apply friction
            let v_n = velocity.dot(&normal) * normal;
            let v_t = *velocity - v_n;

            if v_t.magnitude() > 1e-10 {
                *velocity = v_t * (1.0 - self.friction).max(0.0);
            }

            if velocity.dot(&normal) < 0.0 {
                *velocity -= v_n;
            }

            correction
        } else {
            Vector3::zeros()
        }
    }
}

/// Pin constraint (fixes particle to a position).
#[derive(Debug, Clone)]
pub struct PinConstraint {
    /// Particle index.
    pub particle: usize,
    /// Fixed position.
    pub position: Vector3<f64>,
}

impl PinConstraint {
    /// Create a new pin constraint.
    pub fn new(particle: usize, position: Vector3<f64>) -> Self {
        Self { particle, position }
    }

    /// Solve the constraint.
    pub fn solve(&self, current_pos: Vector3<f64>) -> Vector3<f64> {
        self.position - current_pos
    }
}

/// Self-collision constraint between cloth particles.
#[derive(Debug, Clone)]
pub struct SelfCollisionConstraint {
    /// Minimum distance between particles.
    pub min_distance: f64,
    /// Stiffness.
    pub stiffness: f64,
}

impl SelfCollisionConstraint {
    /// Create a new self-collision constraint.
    pub fn new(min_distance: f64, stiffness: f64) -> Self {
        Self {
            min_distance,
            stiffness,
        }
    }

    /// Solve collision between two particles.
    pub fn solve(
        &self,
        pos1: Vector3<f64>,
        pos2: Vector3<f64>,
        inv_mass1: f64,
        inv_mass2: f64,
    ) -> (Vector3<f64>, Vector3<f64>) {
        let diff = pos2 - pos1;
        let dist = diff.magnitude();

        if dist < self.min_distance && dist > 1e-10 {
            let penetration = self.min_distance - dist;
            let normal = diff / dist;

            let total_inv_mass = inv_mass1 + inv_mass2;
            if total_inv_mass < 1e-10 {
                return (Vector3::zeros(), Vector3::zeros());
            }

            let correction = normal * penetration * self.stiffness;

            let corr1 = -correction * (inv_mass1 / total_inv_mass);
            let corr2 = correction * (inv_mass2 / total_inv_mass);

            (corr1, corr2)
        } else {
            (Vector3::zeros(), Vector3::zeros())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distance_constraint() {
        let constraint = DistanceConstraint::new(0, 1, 1.0, 1.0, ConstraintType::Structural);

        let pos1 = Vector3::new(0.0, 0.0, 0.0);
        let pos2 = Vector3::new(2.0, 0.0, 0.0);

        let (corr1, corr2) = constraint.solve(pos1, pos2, 1.0, 1.0);

        // Particles should move toward each other
        assert!(corr1.x > 0.0);
        assert!(corr2.x < 0.0);
    }

    #[test]
    fn test_plane_collision() {
        let plane = PlaneCollisionConstraint::ground(0.0, 0.5);

        let pos = Vector3::new(0.0, -0.5, 0.0);
        let mut vel = Vector3::new(0.0, -1.0, 0.0);

        let correction = plane.solve(pos, &mut vel);

        // Should push particle up
        assert!(correction.y > 0.0);
    }

    #[test]
    fn test_pin_constraint() {
        let pin = PinConstraint::new(0, Vector3::new(1.0, 2.0, 3.0));

        let current = Vector3::new(0.0, 0.0, 0.0);
        let correction = pin.solve(current);

        assert_eq!(correction, Vector3::new(1.0, 2.0, 3.0));
    }
}
