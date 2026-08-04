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

//! Particle collision detection and resolution.

use nalgebra::{Point3, Vector3};

/// Collision handler for particles.
pub struct ParticleCollisionHandler {
    /// Coefficient of restitution (bounce).
    pub restitution: f64,
    /// Collision radius for sphere-sphere tests.
    pub particle_radius: f64,
}

impl Default for ParticleCollisionHandler {
    fn default() -> Self {
        Self {
            restitution: 0.8,
            particle_radius: 0.05,
        }
    }
}

impl ParticleCollisionHandler {
    /// Create a new collision handler.
    pub fn new(restitution: f64, particle_radius: f64) -> Self {
        Self {
            restitution,
            particle_radius,
        }
    }

    /// Detect collision between two particles.
    pub fn detect_collision(
        &self,
        pos1: &Point3<f64>,
        pos2: &Point3<f64>,
    ) -> Option<CollisionInfo> {
        let dist = (pos2 - pos1).norm();
        let min_dist = self.particle_radius * 2.0;

        if dist < min_dist {
            let normal = (pos2 - pos1).normalize();
            let penetration = min_dist - dist;

            Some(CollisionInfo {
                normal,
                penetration,
                contact_point: Point3::from(pos1.coords + normal * self.particle_radius),
            })
        } else {
            None
        }
    }

    /// Resolve collision between two particles.
    pub fn resolve_collision(
        &self,
        pos1: &mut Point3<f64>,
        vel1: &mut Vector3<f64>,
        pos2: &mut Point3<f64>,
        vel2: &mut Vector3<f64>,
        mass1: f64,
        mass2: f64,
    ) {
        if let Some(info) = self.detect_collision(pos1, pos2) {
            // Separate particles
            let total_mass = mass1 + mass2;
            let correction = info.normal * info.penetration;
            pos1.coords -= correction * (mass2 / total_mass);
            pos2.coords += correction * (mass1 / total_mass);

            // Compute relative velocity
            let relative_vel = *vel2 - *vel1;
            let vel_along_normal = relative_vel.dot(&info.normal);

            // Do not resolve if velocities are separating
            if vel_along_normal > 0.0 {
                return;
            }

            // Calculate impulse scalar
            let impulse_magnitude =
                -(1.0 + self.restitution) * vel_along_normal / (1.0 / mass1 + 1.0 / mass2);

            // Apply impulse
            let impulse = info.normal * impulse_magnitude;
            *vel1 -= impulse / mass1;
            *vel2 += impulse / mass2;
        }
    }

    /// Handle collision with ground plane.
    pub fn ground_collision(&self, pos: &mut Point3<f64>, vel: &mut Vector3<f64>, ground_y: f64) {
        if pos.y < ground_y + self.particle_radius {
            pos.y = ground_y + self.particle_radius;
            vel.y = -vel.y * self.restitution;

            // Apply friction
            vel.x *= 0.95;
            vel.z *= 0.95;
        }
    }
}

/// Collision information.
#[derive(Debug, Clone)]
pub struct CollisionInfo {
    /// Collision normal (from particle 1 to particle 2).
    pub normal: Vector3<f64>,
    /// Penetration depth.
    pub penetration: f64,
    /// Contact point in world space.
    pub contact_point: Point3<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collision_handler_creation() {
        let handler = ParticleCollisionHandler::default();
        assert_eq!(handler.restitution, 0.8);
        assert_eq!(handler.particle_radius, 0.05);
    }

    #[test]
    fn test_collision_detection() {
        let handler = ParticleCollisionHandler::new(0.8, 0.1);

        let p1 = Point3::new(0.0, 0.0, 0.0);
        let p2 = Point3::new(0.15, 0.0, 0.0); // Within collision range

        let collision = handler.detect_collision(&p1, &p2);
        assert!(collision.is_some());

        let p3 = Point3::new(1.0, 0.0, 0.0); // Far away
        let no_collision = handler.detect_collision(&p1, &p3);
        assert!(no_collision.is_none());
    }

    #[test]
    fn test_collision_resolution() {
        let handler = ParticleCollisionHandler::new(1.0, 0.1);

        let mut p1 = Point3::new(0.0, 0.0, 0.0);
        let mut v1 = Vector3::new(1.0, 0.0, 0.0);
        let mut p2 = Point3::new(0.15, 0.0, 0.0);
        let mut v2 = Vector3::new(-1.0, 0.0, 0.0);

        handler.resolve_collision(&mut p1, &mut v1, &mut p2, &mut v2, 1.0, 1.0);

        // Particles should have exchanged velocities (elastic collision, equal mass)
        assert!(v1.x < 0.0);
        assert!(v2.x > 0.0);
    }

    #[test]
    fn test_ground_collision() {
        let handler = ParticleCollisionHandler::new(0.8, 0.1);

        let mut pos = Point3::new(0.0, 0.05, 0.0);
        let mut vel = Vector3::new(0.0, -5.0, 0.0);

        handler.ground_collision(&mut pos, &mut vel, 0.0);

        // Position should be corrected
        assert_eq!(pos.y, 0.1);
        // Velocity should bounce
        assert!(vel.y > 0.0);
    }
}
