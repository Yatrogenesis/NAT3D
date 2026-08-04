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

//! Particle emitters for spawning particles.

use nalgebra::{Point3, Vector3};
use rand::Rng;

/// Particle emitter type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmitterType {
    /// Point emitter (single spawn point).
    Point,
    /// Sphere emitter (spawn on sphere surface).
    Sphere,
    /// Box emitter (spawn in box volume).
    Box,
    /// Cone emitter (spawn in cone shape).
    Cone,
}

/// Particle emitter.
pub struct ParticleEmitter {
    /// Emitter type.
    pub emitter_type: EmitterType,
    /// Position of emitter.
    pub position: Point3<f64>,
    /// Emission rate (particles per second).
    pub rate: f64,
    /// Initial velocity range.
    pub velocity: (Vector3<f64>, Vector3<f64>),
    /// Lifetime range (seconds).
    pub lifetime: (f64, f64),
    /// Size parameters.
    pub size: f64,
    /// Accumulated time for emission.
    pub accumulator: f64,
}

impl Default for ParticleEmitter {
    fn default() -> Self {
        Self {
            emitter_type: EmitterType::Point,
            position: Point3::origin(),
            rate: 10.0,
            velocity: (Vector3::new(0.0, 1.0, 0.0), Vector3::new(0.0, 2.0, 0.0)),
            lifetime: (1.0, 3.0),
            size: 1.0,
            accumulator: 0.0,
        }
    }
}

impl ParticleEmitter {
    /// Create a new particle emitter.
    pub fn new(emitter_type: EmitterType, position: Point3<f64>, rate: f64) -> Self {
        Self {
            emitter_type,
            position,
            rate,
            ..Default::default()
        }
    }

    /// Emit particles for given time step.
    pub fn emit(&mut self, dt: f64) -> Vec<Particle> {
        let mut particles = Vec::new();
        let mut rng = rand::rng();

        self.accumulator += dt;
        let particle_interval = 1.0 / self.rate;

        while self.accumulator >= particle_interval {
            self.accumulator -= particle_interval;

            // Generate particle position based on emitter type
            let pos = match self.emitter_type {
                EmitterType::Point => self.position,
                EmitterType::Sphere => self.position + self.random_on_sphere(&mut rng),
                EmitterType::Box => self.position + self.random_in_box(&mut rng),
                EmitterType::Cone => self.position + self.random_in_cone(&mut rng),
            };

            // Generate random velocity
            let vel = self.random_velocity(&mut rng);

            // Generate random lifetime
            let lifetime = rng.random_range(self.lifetime.0..=self.lifetime.1);

            particles.push(Particle {
                position: pos,
                velocity: vel,
                lifetime,
                age: 0.0,
            });
        }

        particles
    }

    /// Generate random point on sphere surface.
    fn random_on_sphere(&self, rng: &mut impl Rng) -> Vector3<f64> {
        let theta = rng.random_range(0.0..std::f64::consts::TAU);
        let phi = rng.random_range(0.0..std::f64::consts::PI);

        Vector3::new(
            self.size * phi.sin() * theta.cos(),
            self.size * phi.sin() * theta.sin(),
            self.size * phi.cos(),
        )
    }

    /// Generate random point in box volume.
    fn random_in_box(&self, rng: &mut impl Rng) -> Vector3<f64> {
        Vector3::new(
            rng.random_range(-self.size..self.size),
            rng.random_range(-self.size..self.size),
            rng.random_range(-self.size..self.size),
        )
    }

    /// Generate random point in cone.
    fn random_in_cone(&self, rng: &mut impl Rng) -> Vector3<f64> {
        let angle = rng.random_range(0.0..std::f64::consts::TAU);
        let radius = rng.random_range(0.0..self.size);
        let height = rng.random_range(0.0..self.size);

        Vector3::new(radius * angle.cos(), height, radius * angle.sin())
    }

    /// Generate random velocity.
    fn random_velocity(&self, rng: &mut impl Rng) -> Vector3<f64> {
        Vector3::new(
            rng.random_range(self.velocity.0.x..=self.velocity.1.x),
            rng.random_range(self.velocity.0.y..=self.velocity.1.y),
            rng.random_range(self.velocity.0.z..=self.velocity.1.z),
        )
    }
}

/// Particle spawned by emitter.
#[derive(Debug, Clone)]
pub struct Particle {
    /// Position.
    pub position: Point3<f64>,
    /// Velocity.
    pub velocity: Vector3<f64>,
    /// Total lifetime (seconds).
    pub lifetime: f64,
    /// Current age (seconds).
    pub age: f64,
}

impl Particle {
    /// Check if particle is alive.
    pub fn is_alive(&self) -> bool {
        self.age < self.lifetime
    }

    /// Update particle.
    pub fn update(&mut self, dt: f64) {
        self.age += dt;
        self.position += self.velocity * dt;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emitter_creation() {
        let emitter = ParticleEmitter::default();
        assert_eq!(emitter.emitter_type, EmitterType::Point);
        assert_eq!(emitter.rate, 10.0);
    }

    #[test]
    fn test_particle_emission() {
        let mut emitter = ParticleEmitter::new(EmitterType::Point, Point3::origin(), 100.0);

        let particles = emitter.emit(0.1); // 10 particles at 100/sec

        assert!(particles.len() >= 8 && particles.len() <= 12); // Approximately 10
    }

    #[test]
    fn test_particle_lifetime() {
        let mut particle = Particle {
            position: Point3::origin(),
            velocity: Vector3::zeros(),
            lifetime: 1.0,
            age: 0.0,
        };

        assert!(particle.is_alive());

        particle.update(0.5);
        assert!(particle.is_alive());

        particle.update(0.6);
        assert!(!particle.is_alive());
    }
}
