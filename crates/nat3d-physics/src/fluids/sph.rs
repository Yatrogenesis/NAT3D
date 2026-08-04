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

//! Smoothed Particle Hydrodynamics (SPH) fluid simulation.
//!
//! Lagrangian particle-based fluid simulation using SPH interpolation.

use nalgebra::Vector3;
use std::collections::HashMap;

/// SPH simulation parameters.
#[derive(Debug, Clone)]
pub struct SphParams {
    /// Particle rest density (kg/m³).
    pub rest_density: f64,
    /// Gas stiffness constant.
    pub gas_constant: f64,
    /// Viscosity coefficient.
    pub viscosity: f64,
    /// Surface tension coefficient.
    pub surface_tension: f64,
    /// Smoothing kernel radius.
    pub kernel_radius: f64,
    /// Particle mass.
    pub particle_mass: f64,
    /// Time step.
    pub dt: f64,
    /// Gravity.
    pub gravity: Vector3<f64>,
    /// Boundary damping.
    pub boundary_damping: f64,
}

impl Default for SphParams {
    fn default() -> Self {
        Self {
            rest_density: 1000.0,
            gas_constant: 2000.0,
            viscosity: 0.001,
            surface_tension: 0.0728,
            kernel_radius: 0.04,
            particle_mass: 0.02,
            dt: 0.0008,
            gravity: Vector3::new(0.0, -9.81, 0.0),
            boundary_damping: 0.3,
        }
    }
}

/// SPH particle data.
#[derive(Debug, Clone)]
pub struct SphParticle {
    /// Position.
    pub position: Vector3<f64>,
    /// Velocity.
    pub velocity: Vector3<f64>,
    /// Acceleration.
    pub acceleration: Vector3<f64>,
    /// Density.
    pub density: f64,
    /// Pressure.
    pub pressure: f64,
    /// Color field (for surface detection).
    pub color_field: f64,
    /// Color field gradient (surface normal).
    pub color_gradient: Vector3<f64>,
    /// Color field laplacian.
    pub color_laplacian: f64,
}

impl SphParticle {
    /// Create a new particle at position.
    pub fn new(position: Vector3<f64>) -> Self {
        Self {
            position,
            velocity: Vector3::zeros(),
            acceleration: Vector3::zeros(),
            density: 0.0,
            pressure: 0.0,
            color_field: 0.0,
            color_gradient: Vector3::zeros(),
            color_laplacian: 0.0,
        }
    }
}

/// Spatial hash grid for neighbor search.
pub struct SpatialHash {
    cell_size: f64,
    cells: HashMap<(i64, i64, i64), Vec<usize>>,
}

impl SpatialHash {
    /// Create a new spatial hash.
    pub fn new(cell_size: f64) -> Self {
        Self {
            cell_size,
            cells: HashMap::new(),
        }
    }

    /// Clear the hash.
    pub fn clear(&mut self) {
        self.cells.clear();
    }

    /// Get cell key for position.
    fn cell_key(&self, pos: &Vector3<f64>) -> (i64, i64, i64) {
        (
            (pos.x / self.cell_size).floor() as i64,
            (pos.y / self.cell_size).floor() as i64,
            (pos.z / self.cell_size).floor() as i64,
        )
    }

    /// Insert particle into hash.
    pub fn insert(&mut self, index: usize, pos: &Vector3<f64>) {
        let key = self.cell_key(pos);
        self.cells.entry(key).or_default().push(index);
    }

    /// Get potential neighbors for a position.
    pub fn get_neighbors(&self, pos: &Vector3<f64>) -> Vec<usize> {
        let (cx, cy, cz) = self.cell_key(pos);
        let mut neighbors = Vec::new();

        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(cell) = self.cells.get(&(cx + dx, cy + dy, cz + dz)) {
                        neighbors.extend(cell.iter().copied());
                    }
                }
            }
        }

        neighbors
    }
}

/// SPH fluid simulator.
pub struct SphSimulator {
    /// Simulation parameters.
    pub params: SphParams,
    /// Particles.
    pub particles: Vec<SphParticle>,
    /// Spatial hash for neighbor search.
    spatial_hash: SpatialHash,
    /// Simulation time.
    pub time: f64,
    /// Boundary min.
    pub boundary_min: Vector3<f64>,
    /// Boundary max.
    pub boundary_max: Vector3<f64>,
}

impl SphSimulator {
    /// Create a new SPH simulator.
    pub fn new(params: SphParams) -> Self {
        let cell_size = params.kernel_radius;
        Self {
            params,
            particles: Vec::new(),
            spatial_hash: SpatialHash::new(cell_size),
            time: 0.0,
            boundary_min: Vector3::new(0.0, 0.0, 0.0),
            boundary_max: Vector3::new(1.0, 1.0, 1.0),
        }
    }

    /// Set simulation boundaries.
    pub fn set_boundaries(&mut self, min: Vector3<f64>, max: Vector3<f64>) {
        self.boundary_min = min;
        self.boundary_max = max;
    }

    /// Add a particle.
    pub fn add_particle(&mut self, pos: Vector3<f64>) {
        self.particles.push(SphParticle::new(pos));
    }

    /// Add a block of particles.
    pub fn add_block(&mut self, min: Vector3<f64>, max: Vector3<f64>, spacing: f64) {
        let mut x = min.x;
        while x < max.x {
            let mut y = min.y;
            while y < max.y {
                let mut z = min.z;
                while z < max.z {
                    self.add_particle(Vector3::new(x, y, z));
                    z += spacing;
                }
                y += spacing;
            }
            x += spacing;
        }
    }

    /// Step simulation.
    pub fn step(&mut self) {
        self.build_spatial_hash();
        self.compute_density_pressure();
        self.compute_forces();
        self.integrate();
        self.enforce_boundaries();
        self.time += self.params.dt;
    }

    /// Build spatial hash.
    fn build_spatial_hash(&mut self) {
        self.spatial_hash.clear();
        for (i, p) in self.particles.iter().enumerate() {
            self.spatial_hash.insert(i, &p.position);
        }
    }

    /// Compute density and pressure for all particles.
    fn compute_density_pressure(&mut self) {
        let h = self.params.kernel_radius;
        let mass = self.params.particle_mass;
        let rest_density = self.params.rest_density;
        let k = self.params.gas_constant;

        // Poly6 kernel coefficient
        let poly6_coeff = 315.0 / (64.0 * std::f64::consts::PI * h.powi(9));

        let positions: Vec<Vector3<f64>> = self.particles.iter().map(|p| p.position).collect();

        for i in 0..self.particles.len() {
            let pos_i = positions[i];
            let neighbors = self.spatial_hash.get_neighbors(&pos_i);

            let mut density = 0.0;

            for &j in &neighbors {
                let r = pos_i - positions[j];
                let r_sq = r.magnitude_squared();

                if r_sq < h * h {
                    let diff = h * h - r_sq;
                    density += mass * poly6_coeff * diff * diff * diff;
                }
            }

            self.particles[i].density = density.max(rest_density);
            self.particles[i].pressure = k * (self.particles[i].density - rest_density);
        }
    }

    /// Compute forces (pressure, viscosity, surface tension).
    fn compute_forces(&mut self) {
        let h = self.params.kernel_radius;
        let mass = self.params.particle_mass;
        let viscosity = self.params.viscosity;
        let gravity = self.params.gravity;

        // Spiky gradient kernel coefficient
        let spiky_coeff = -45.0 / (std::f64::consts::PI * h.powi(6));

        // Viscosity laplacian kernel coefficient
        let visc_coeff = 45.0 / (std::f64::consts::PI * h.powi(6));

        let positions: Vec<Vector3<f64>> = self.particles.iter().map(|p| p.position).collect();
        let velocities: Vec<Vector3<f64>> = self.particles.iter().map(|p| p.velocity).collect();
        let densities: Vec<f64> = self.particles.iter().map(|p| p.density).collect();
        let pressures: Vec<f64> = self.particles.iter().map(|p| p.pressure).collect();

        for i in 0..self.particles.len() {
            let pos_i = positions[i];
            let vel_i = velocities[i];
            let density_i = densities[i];
            let pressure_i = pressures[i];

            let neighbors = self.spatial_hash.get_neighbors(&pos_i);

            let mut f_pressure = Vector3::zeros();
            let mut f_viscosity = Vector3::zeros();

            for &j in &neighbors {
                if i == j {
                    continue;
                }

                let r = pos_i - positions[j];
                let r_len = r.magnitude();

                if r_len < h && r_len > 1e-10 {
                    let r_norm = r / r_len;

                    // Pressure force
                    let pressure_term = (pressure_i + pressures[j]) / (2.0 * densities[j]);
                    let spiky_grad = spiky_coeff * (h - r_len) * (h - r_len);
                    f_pressure -= mass * pressure_term * spiky_grad * r_norm;

                    // Viscosity force
                    let visc_lap = visc_coeff * (h - r_len);
                    f_viscosity +=
                        viscosity * mass * (velocities[j] - vel_i) / densities[j] * visc_lap;
                }
            }

            // Total acceleration
            self.particles[i].acceleration = (f_pressure + f_viscosity) / density_i + gravity;
        }
    }

    /// Integrate particle positions and velocities.
    fn integrate(&mut self) {
        let dt = self.params.dt;

        for p in &mut self.particles {
            p.velocity += p.acceleration * dt;
            p.position += p.velocity * dt;
        }
    }

    /// Enforce boundary conditions.
    fn enforce_boundaries(&mut self) {
        let damping = self.params.boundary_damping;
        let min = self.boundary_min;
        let max = self.boundary_max;

        for p in &mut self.particles {
            // X boundaries
            if p.position.x < min.x {
                p.position.x = min.x;
                p.velocity.x *= -damping;
            }
            if p.position.x > max.x {
                p.position.x = max.x;
                p.velocity.x *= -damping;
            }

            // Y boundaries
            if p.position.y < min.y {
                p.position.y = min.y;
                p.velocity.y *= -damping;
            }
            if p.position.y > max.y {
                p.position.y = max.y;
                p.velocity.y *= -damping;
            }

            // Z boundaries
            if p.position.z < min.z {
                p.position.z = min.z;
                p.velocity.z *= -damping;
            }
            if p.position.z > max.z {
                p.position.z = max.z;
                p.velocity.z *= -damping;
            }
        }
    }

    /// Get particle count.
    pub fn particle_count(&self) -> usize {
        self.particles.len()
    }

    /// Get particle positions for rendering.
    pub fn positions(&self) -> Vec<Vector3<f64>> {
        self.particles.iter().map(|p| p.position).collect()
    }

    /// Get particle velocities.
    pub fn velocities(&self) -> Vec<Vector3<f64>> {
        self.particles.iter().map(|p| p.velocity).collect()
    }

    /// Get particle densities.
    pub fn densities(&self) -> Vec<f64> {
        self.particles.iter().map(|p| p.density).collect()
    }

    /// Compute total kinetic energy.
    pub fn kinetic_energy(&self) -> f64 {
        let mass = self.params.particle_mass;
        self.particles
            .iter()
            .map(|p| 0.5 * mass * p.velocity.magnitude_squared())
            .sum()
    }

    /// Compute average density.
    pub fn average_density(&self) -> f64 {
        if self.particles.is_empty() {
            return 0.0;
        }
        self.particles.iter().map(|p| p.density).sum::<f64>() / self.particles.len() as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sph_creation() {
        let params = SphParams::default();
        let sim = SphSimulator::new(params);
        assert_eq!(sim.particle_count(), 0);
    }

    #[test]
    fn test_add_particles() {
        let params = SphParams::default();
        let mut sim = SphSimulator::new(params);

        sim.add_block(
            Vector3::new(0.2, 0.2, 0.2),
            Vector3::new(0.4, 0.4, 0.4),
            0.02,
        );

        assert!(sim.particle_count() > 0);
    }

    #[test]
    fn test_step() {
        let params = SphParams::default();
        let mut sim = SphSimulator::new(params);

        sim.add_block(
            Vector3::new(0.3, 0.5, 0.3),
            Vector3::new(0.5, 0.7, 0.5),
            0.02,
        );

        let initial_time = sim.time;
        sim.step();

        assert!(sim.time > initial_time);
    }
}
