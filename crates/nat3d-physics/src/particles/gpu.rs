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

//! GPU-accelerated particle simulation.
//!
//! Placeholder for compute shader-based particle systems.

use nalgebra::{Point3, Vector3};

/// GPU particle system (CPU fallback for now).
pub struct GpuParticleSystem {
    /// Particles stored on CPU (to be migrated to GPU buffers).
    pub particles: Vec<GpuParticle>,
    /// Maximum particle count.
    pub max_particles: usize,
}

/// Particle structure for GPU simulation.
#[derive(Debug, Clone, Copy)]
pub struct GpuParticle {
    /// Position.
    pub position: Point3<f32>,
    /// Velocity.
    pub velocity: Vector3<f32>,
    /// Age (seconds).
    pub age: f32,
    /// Lifetime (seconds).
    pub lifetime: f32,
}

impl Default for GpuParticleSystem {
    fn default() -> Self {
        Self {
            particles: Vec::new(),
            max_particles: 100_000,
        }
    }
}

impl GpuParticleSystem {
    /// Create a new GPU particle system.
    pub fn new(max_particles: usize) -> Self {
        Self {
            particles: Vec::with_capacity(max_particles),
            max_particles,
        }
    }

    /// Spawn a new particle.
    pub fn spawn(&mut self, position: Point3<f32>, velocity: Vector3<f32>, lifetime: f32) {
        if self.particles.len() < self.max_particles {
            self.particles.push(GpuParticle {
                position,
                velocity,
                age: 0.0,
                lifetime,
            });
        }
    }

    /// Update particles (CPU fallback, to be replaced with compute shader).
    pub fn update_cpu(&mut self, dt: f32, gravity: Vector3<f32>) {
        // Update particles
        for particle in &mut self.particles {
            particle.age += dt;
            particle.velocity += gravity * dt;
            particle.position += particle.velocity * dt;
        }

        // Remove dead particles
        self.particles.retain(|p| p.age < p.lifetime);
    }

    /// Get particle count.
    pub fn count(&self) -> usize {
        self.particles.len()
    }

    /// Clear all particles.
    pub fn clear(&mut self) {
        self.particles.clear();
    }

    /// Get particle positions for rendering.
    pub fn positions(&self) -> Vec<[f32; 3]> {
        self.particles
            .iter()
            .map(|p| [p.position.x, p.position.y, p.position.z])
            .collect()
    }
}

// TODO: Implement GPU compute shader integration with wgpu
// - Create compute pipeline
// - Allocate GPU buffers for particles
// - Dispatch compute shader for parallel update
// - Use storage buffers for particle data
// - Implement indirect rendering for particles

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_system_creation() {
        let system = GpuParticleSystem::default();
        assert_eq!(system.max_particles, 100_000);
        assert_eq!(system.count(), 0);
    }

    #[test]
    fn test_particle_spawn() {
        let mut system = GpuParticleSystem::new(10);

        system.spawn(Point3::new(0.0, 0.0, 0.0), Vector3::new(0.0, 1.0, 0.0), 5.0);

        assert_eq!(system.count(), 1);
    }

    #[test]
    fn test_particle_update() {
        let mut system = GpuParticleSystem::new(10);

        system.spawn(Point3::new(0.0, 0.0, 0.0), Vector3::new(0.0, 1.0, 0.0), 5.0);

        system.update_cpu(0.1, Vector3::new(0.0, -9.81, 0.0));

        assert_eq!(system.count(), 1);
        assert!(system.particles[0].position.y > 0.0); // Should have moved up
    }

    #[test]
    fn test_particle_lifetime() {
        let mut system = GpuParticleSystem::new(10);

        system.spawn(
            Point3::new(0.0, 0.0, 0.0),
            Vector3::zeros(),
            0.5, // Short lifetime
        );

        system.update_cpu(1.0, Vector3::zeros()); // Exceed lifetime

        assert_eq!(system.count(), 0); // Should be removed
    }

    #[test]
    fn test_max_particles() {
        let mut system = GpuParticleSystem::new(5);

        for _ in 0..10 {
            system.spawn(Point3::origin(), Vector3::zeros(), 1.0);
        }

        assert_eq!(system.count(), 5); // Should cap at max
    }
}
