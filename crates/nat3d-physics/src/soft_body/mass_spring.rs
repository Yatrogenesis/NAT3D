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

//! Mass-spring system for soft bodies.
//!
//! Implements particle-based soft body physics using springs.
//! Uses Verlet integration for stability.

use nalgebra::Vector3;
use serde::{Deserialize, Serialize};

/// A particle in the mass-spring system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Particle {
    /// Current position.
    pub position: Vector3<f64>,
    /// Current velocity.
    pub velocity: Vector3<f64>,
    /// Current acceleration.
    pub acceleration: Vector3<f64>,
    /// Mass of the particle.
    pub mass: f64,
    /// Inverse mass (0 for fixed particles).
    pub inv_mass: f64,
    /// Is this particle fixed (immovable)?
    pub fixed: bool,
    /// External force applied this frame.
    pub external_force: Vector3<f64>,
}

impl Particle {
    /// Create a new particle.
    pub fn new(position: Vector3<f64>, mass: f64) -> Self {
        let inv_mass = if mass > 0.0 { 1.0 / mass } else { 0.0 };
        Self {
            position,
            velocity: Vector3::zeros(),
            acceleration: Vector3::zeros(),
            mass,
            inv_mass,
            fixed: false,
            external_force: Vector3::zeros(),
        }
    }

    /// Create a fixed particle.
    pub fn fixed(position: Vector3<f64>) -> Self {
        Self {
            position,
            velocity: Vector3::zeros(),
            acceleration: Vector3::zeros(),
            mass: f64::INFINITY,
            inv_mass: 0.0,
            fixed: true,
            external_force: Vector3::zeros(),
        }
    }

    /// Apply a force to this particle.
    pub fn apply_force(&mut self, force: Vector3<f64>) {
        if !self.fixed {
            self.external_force += force;
        }
    }
}

/// Type of spring constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpringType {
    /// Structural spring (along edges).
    Structural,
    /// Shear spring (diagonal).
    Shear,
    /// Bending spring (skip one vertex).
    Bend,
}

/// A spring connecting two particles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spring {
    /// Index of first particle.
    pub particle_a: usize,
    /// Index of second particle.
    pub particle_b: usize,
    /// Rest length of the spring.
    pub rest_length: f64,
    /// Spring stiffness coefficient.
    pub stiffness: f64,
    /// Damping coefficient.
    pub damping: f64,
    /// Type of spring.
    pub spring_type: SpringType,
}

impl Spring {
    /// Create a new spring.
    pub fn new(
        particle_a: usize,
        particle_b: usize,
        rest_length: f64,
        stiffness: f64,
        damping: f64,
        spring_type: SpringType,
    ) -> Self {
        Self {
            particle_a,
            particle_b,
            rest_length,
            stiffness,
            damping,
            spring_type,
        }
    }

    /// Compute spring force.
    pub fn compute_force(
        &self,
        pos_a: Vector3<f64>,
        pos_b: Vector3<f64>,
        vel_a: Vector3<f64>,
        vel_b: Vector3<f64>,
    ) -> (Vector3<f64>, Vector3<f64>) {
        let delta = pos_b - pos_a;
        let dist = delta.magnitude();

        if dist < 1e-10 {
            return (Vector3::zeros(), Vector3::zeros());
        }

        let direction = delta / dist;

        // Spring force (Hooke's law)
        let spring_force = self.stiffness * (dist - self.rest_length);

        // Damping force
        let vel_diff = vel_b - vel_a;
        let damping_force = self.damping * vel_diff.dot(&direction);

        let total_force = (spring_force + damping_force) * direction;

        (total_force, -total_force)
    }
}

/// Mass-spring soft body system.
#[derive(Debug, Clone)]
pub struct MassSpringSystem {
    /// All particles in the system.
    pub particles: Vec<Particle>,
    /// All springs in the system.
    pub springs: Vec<Spring>,
    /// Global damping coefficient.
    pub damping: f64,
    /// Gravity vector.
    pub gravity: Vector3<f64>,
    /// Simulation time.
    pub time: f64,
}

impl MassSpringSystem {
    /// Create a new mass-spring system.
    pub fn new() -> Self {
        Self {
            particles: Vec::new(),
            springs: Vec::new(),
            damping: 0.01,
            gravity: Vector3::new(0.0, -9.81, 0.0),
            time: 0.0,
        }
    }

    /// Add a particle to the system.
    pub fn add_particle(&mut self, particle: Particle) -> usize {
        let idx = self.particles.len();
        self.particles.push(particle);
        idx
    }

    /// Add a spring to the system.
    pub fn add_spring(&mut self, spring: Spring) {
        self.springs.push(spring);
    }

    /// Apply gravity to all particles.
    pub fn apply_gravity(&mut self) {
        for particle in &mut self.particles {
            if !particle.fixed {
                let gravity_force = self.gravity * particle.mass;
                particle.external_force += gravity_force;
            }
        }
    }

    /// Solve all spring constraints.
    fn solve_springs(&mut self) {
        // Compute spring forces
        let mut forces = vec![Vector3::zeros(); self.particles.len()];

        for spring in &self.springs {
            let a = spring.particle_a;
            let b = spring.particle_b;

            let pos_a = self.particles[a].position;
            let pos_b = self.particles[b].position;
            let vel_a = self.particles[a].velocity;
            let vel_b = self.particles[b].velocity;

            let (force_a, force_b) = spring.compute_force(pos_a, pos_b, vel_a, vel_b);

            forces[a] += force_a;
            forces[b] += force_b;
        }

        // Apply forces to particles
        for (i, force) in forces.iter().enumerate() {
            self.particles[i].external_force += force;
        }
    }

    /// Integrate particle motion using Verlet integration.
    fn integrate(&mut self, dt: f64) {
        for particle in &mut self.particles {
            if particle.fixed {
                particle.external_force = Vector3::zeros();
                continue;
            }

            // Compute acceleration
            particle.acceleration = particle.external_force * particle.inv_mass;

            // Verlet integration (velocity form)
            // v(t+dt) = v(t) + a(t) * dt
            // x(t+dt) = x(t) + v(t+dt) * dt
            particle.velocity += particle.acceleration * dt;

            // Apply damping
            particle.velocity *= 1.0 - self.damping;

            particle.position += particle.velocity * dt;

            // Clear forces for next frame
            particle.external_force = Vector3::zeros();
        }
    }

    /// Step the simulation forward by dt.
    pub fn step(&mut self, dt: f64) {
        // Apply gravity
        self.apply_gravity();

        // Solve springs
        self.solve_springs();

        // Integrate motion
        self.integrate(dt);

        self.time += dt;
    }

    /// Create a mass-spring system from a mesh.
    /// Creates particles at vertices and springs along edges.
    pub fn from_mesh(
        positions: &[Vector3<f64>],
        edges: &[(usize, usize)],
        particle_mass: f64,
        stiffness: f64,
        damping: f64,
    ) -> Self {
        let mut system = Self::new();

        // Create particles
        for pos in positions {
            let particle = Particle::new(*pos, particle_mass);
            system.add_particle(particle);
        }

        // Create springs from edges
        for &(a, b) in edges {
            if a < positions.len() && b < positions.len() {
                let rest_length = (positions[b] - positions[a]).magnitude();
                let spring = Spring::new(
                    a,
                    b,
                    rest_length,
                    stiffness,
                    damping,
                    SpringType::Structural,
                );
                system.add_spring(spring);
            }
        }

        system
    }

    /// Create a grid of particles connected by springs.
    pub fn create_grid(
        width: usize,
        height: usize,
        spacing: f64,
        particle_mass: f64,
        stiffness: f64,
        damping: f64,
    ) -> Self {
        let mut system = Self::new();

        // Create particles
        for j in 0..height {
            for i in 0..width {
                let pos = Vector3::new(i as f64 * spacing, j as f64 * spacing, 0.0);
                let particle = Particle::new(pos, particle_mass);
                system.add_particle(particle);
            }
        }

        // Create structural springs (horizontal and vertical)
        for j in 0..height {
            for i in 0..width {
                let idx = j * width + i;

                // Horizontal spring
                if i < width - 1 {
                    let spring = Spring::new(
                        idx,
                        idx + 1,
                        spacing,
                        stiffness,
                        damping,
                        SpringType::Structural,
                    );
                    system.add_spring(spring);
                }

                // Vertical spring
                if j < height - 1 {
                    let spring = Spring::new(
                        idx,
                        idx + width,
                        spacing,
                        stiffness,
                        damping,
                        SpringType::Structural,
                    );
                    system.add_spring(spring);
                }
            }
        }

        // Create shear springs (diagonals)
        for j in 0..height - 1 {
            for i in 0..width - 1 {
                let idx = j * width + i;
                let diag_length = spacing * 2.0_f64.sqrt();

                // Diagonal \
                let spring1 = Spring::new(
                    idx,
                    idx + width + 1,
                    diag_length,
                    stiffness * 0.5,
                    damping,
                    SpringType::Shear,
                );
                system.add_spring(spring1);

                // Diagonal /
                let spring2 = Spring::new(
                    idx + 1,
                    idx + width,
                    diag_length,
                    stiffness * 0.5,
                    damping,
                    SpringType::Shear,
                );
                system.add_spring(spring2);
            }
        }

        // Create bending springs (skip one particle)
        for j in 0..height {
            for i in 0..width {
                let idx = j * width + i;

                // Horizontal bending
                if i < width - 2 {
                    let spring = Spring::new(
                        idx,
                        idx + 2,
                        spacing * 2.0,
                        stiffness * 0.3,
                        damping,
                        SpringType::Bend,
                    );
                    system.add_spring(spring);
                }

                // Vertical bending
                if j < height - 2 {
                    let spring = Spring::new(
                        idx,
                        idx + width * 2,
                        spacing * 2.0,
                        stiffness * 0.3,
                        damping,
                        SpringType::Bend,
                    );
                    system.add_spring(spring);
                }
            }
        }

        system
    }

    /// Get total kinetic energy.
    pub fn kinetic_energy(&self) -> f64 {
        self.particles
            .iter()
            .filter(|p| !p.fixed)
            .map(|p| 0.5 * p.mass * p.velocity.magnitude_squared())
            .sum()
    }

    /// Get total potential energy (elastic).
    pub fn potential_energy(&self) -> f64 {
        self.springs
            .iter()
            .map(|spring| {
                let pos_a = self.particles[spring.particle_a].position;
                let pos_b = self.particles[spring.particle_b].position;
                let dist = (pos_b - pos_a).magnitude();
                let extension = dist - spring.rest_length;
                0.5 * spring.stiffness * extension * extension
            })
            .sum()
    }
}

impl Default for MassSpringSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_particle_creation() {
        let p = Particle::new(Vector3::new(0.0, 0.0, 0.0), 1.0);
        assert_eq!(p.mass, 1.0);
        assert!(!p.fixed);
        assert_eq!(p.inv_mass, 1.0);
    }

    #[test]
    fn test_fixed_particle() {
        let p = Particle::fixed(Vector3::new(1.0, 2.0, 3.0));
        assert!(p.fixed);
        assert_eq!(p.inv_mass, 0.0);
    }

    #[test]
    fn test_spring_force() {
        let spring = Spring::new(0, 1, 1.0, 100.0, 0.1, SpringType::Structural);

        let pos_a = Vector3::new(0.0, 0.0, 0.0);
        let pos_b = Vector3::new(2.0, 0.0, 0.0); // Extended
        let vel_a = Vector3::zeros();
        let vel_b = Vector3::zeros();

        let (force_a, force_b) = spring.compute_force(pos_a, pos_b, vel_a, vel_b);

        // Spring should pull particle a to the right
        assert!(force_a.x > 0.0);
        // Spring should pull particle b to the left
        assert!(force_b.x < 0.0);
        // Forces should be equal and opposite
        assert!((force_a + force_b).magnitude() < 1e-10);
    }

    #[test]
    fn test_system_step() {
        let mut system = MassSpringSystem::new();

        let p1 = Particle::new(Vector3::new(0.0, 0.0, 0.0), 1.0);
        let p2 = Particle::new(Vector3::new(2.0, 0.0, 0.0), 1.0);

        system.add_particle(p1);
        system.add_particle(p2);

        let spring = Spring::new(0, 1, 1.0, 100.0, 0.1, SpringType::Structural);
        system.add_spring(spring);

        let initial_dist =
            (system.particles[1].position - system.particles[0].position).magnitude();

        // Step simulation
        system.step(0.01);

        let final_dist = (system.particles[1].position - system.particles[0].position).magnitude();

        // Particles should move closer to rest length
        assert!(final_dist < initial_dist);
    }

    #[test]
    fn test_from_mesh() {
        let positions = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
        ];

        let edges = vec![(0, 1), (1, 2), (2, 0)];

        let system = MassSpringSystem::from_mesh(&positions, &edges, 1.0, 100.0, 0.1);

        assert_eq!(system.particles.len(), 3);
        assert_eq!(system.springs.len(), 3);
    }

    #[test]
    fn test_grid_creation() {
        let system = MassSpringSystem::create_grid(3, 3, 1.0, 1.0, 100.0, 0.1);

        assert_eq!(system.particles.len(), 9);
        // 3*2 horizontal + 3*2 vertical + 2*2*2 diagonal + 1*2 + 1*2 bending
        assert!(system.springs.len() > 0);
    }

    #[test]
    fn test_energy_conservation() {
        let mut system = MassSpringSystem::new();
        system.gravity = Vector3::zeros(); // No gravity for energy conservation test

        let p1 = Particle::new(Vector3::new(0.0, 0.0, 0.0), 1.0);
        let mut p2 = Particle::new(Vector3::new(2.0, 0.0, 0.0), 1.0);
        p2.velocity = Vector3::new(1.0, 0.0, 0.0);

        system.add_particle(p1);
        system.add_particle(p2);

        let spring = Spring::new(0, 1, 1.0, 100.0, 0.0, SpringType::Structural);
        system.add_spring(spring);

        let initial_energy = system.kinetic_energy() + system.potential_energy();

        // Step without damping
        system.damping = 0.0;
        for _ in 0..10 {
            system.step(0.001);
        }

        let final_energy = system.kinetic_energy() + system.potential_energy();

        // Energy should be approximately conserved (within numerical error)
        let energy_diff = (final_energy - initial_energy).abs();
        assert!(
            energy_diff < 0.5,
            "Energy difference too large: {}",
            energy_diff
        );
    }
}
