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

//! FLIP (Fluid Implicit Particle) and PIC (Particle-In-Cell) methods.
//!
//! Hybrid Eulerian-Lagrangian fluid simulation combining grid-based
//! and particle-based approaches.

use super::grid::MacGrid;
use nalgebra::Vector3;

/// Particle for FLIP/PIC simulation.
#[derive(Debug, Clone)]
pub struct FlipParticle {
    /// Particle position.
    pub position: Vector3<f64>,
    /// Particle velocity.
    pub velocity: Vector3<f64>,
}

/// FLIP/PIC solver.
pub struct FlipSolver {
    /// Particles.
    pub particles: Vec<FlipParticle>,
    /// FLIP/PIC mixing ratio (0 = full PIC, 1 = full FLIP).
    pub flip_ratio: f64,
}

impl Default for FlipSolver {
    fn default() -> Self {
        Self {
            particles: Vec::new(),
            flip_ratio: 0.95, // Mostly FLIP (less numerical dissipation)
        }
    }
}

impl FlipSolver {
    /// Create a new FLIP solver.
    pub fn new(flip_ratio: f64) -> Self {
        Self {
            particles: Vec::new(),
            flip_ratio: flip_ratio.clamp(0.0, 1.0),
        }
    }

    /// Initialize particles from grid.
    pub fn initialize_from_grid(&mut self, grid: &MacGrid, particles_per_cell: usize) {
        self.particles.clear();

        let (nx, ny, nz) = grid.resolution;
        let dx = grid.cell_size;

        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let vel = grid.velocity_at_center(i, j, k);

                    // Seed multiple particles per cell
                    for pi in 0..particles_per_cell.max(1).min(8) {
                        let offset_x = (pi as f64 + 0.5) / 8.0;
                        let offset_y = (pi as f64 * 0.7 + 0.3) / 8.0;
                        let offset_z = (pi as f64 * 0.5 + 0.5) / 8.0;

                        let pos = Vector3::new(
                            (i as f64 + offset_x) * dx,
                            (j as f64 + offset_y) * dx,
                            (k as f64 + offset_z) * dx,
                        );

                        self.particles.push(FlipParticle {
                            position: pos,
                            velocity: vel,
                        });
                    }
                }
            }
        }
    }

    /// Transfer particle velocities to grid (P2G).
    pub fn particles_to_grid(&self, grid: &mut MacGrid) {
        grid.clear_velocities();

        let dx = grid.cell_size;
        let (nx, ny, nz) = grid.resolution;

        // Weighted accumulation buffers
        let mut u_weights = vec![0.0; grid.u.len()];
        let mut v_weights = vec![0.0; grid.v.len()];
        let mut w_weights = vec![0.0; grid.w.len()];

        for particle in &self.particles {
            let x = particle.position.x;
            let y = particle.position.y;
            let z = particle.position.z;

            // Get cell indices
            let i = (x / dx).floor() as isize;
            let j = (y / dx).floor() as isize;
            let k = (z / dx).floor() as isize;

            if i >= 0
                && j >= 0
                && k >= 0
                && (i as usize) < nx
                && (j as usize) < ny
                && (k as usize) < nz
            {
                let i = i as usize;
                let j = j as usize;
                let k = k as usize;

                // Simple 1st-order interpolation to grid
                // In production, use trilinear or higher-order interpolation
                let u_idx = grid.u_index(i, j, k);
                grid.u[u_idx] += particle.velocity.x;
                u_weights[u_idx] += 1.0;

                let v_idx = grid.v_index(i, j, k);
                grid.v[v_idx] += particle.velocity.y;
                v_weights[v_idx] += 1.0;

                let w_idx = grid.w_index(i, j, k);
                grid.w[w_idx] += particle.velocity.z;
                w_weights[w_idx] += 1.0;
            }
        }

        // Normalize by weights
        for i in 0..grid.u.len() {
            if u_weights[i] > 0.0 {
                grid.u[i] /= u_weights[i];
            }
        }

        for i in 0..grid.v.len() {
            if v_weights[i] > 0.0 {
                grid.v[i] /= v_weights[i];
            }
        }

        for i in 0..grid.w.len() {
            if w_weights[i] > 0.0 {
                grid.w[i] /= w_weights[i];
            }
        }
    }

    /// Transfer grid velocities to particles (G2P) with FLIP/PIC blending.
    pub fn grid_to_particles(&mut self, grid: &MacGrid, old_grid: &MacGrid) {
        let dx = grid.cell_size;
        let (nx, ny, nz) = grid.resolution;

        for particle in &mut self.particles {
            let x = particle.position.x;
            let y = particle.position.y;
            let z = particle.position.z;

            let i = ((x / dx).floor() as isize).max(0).min(nx as isize - 1) as usize;
            let j = ((y / dx).floor() as isize).max(0).min(ny as isize - 1) as usize;
            let k = ((z / dx).floor() as isize).max(0).min(nz as isize - 1) as usize;

            // Interpolate grid velocities
            let vel_new = grid.velocity_at_center(i, j, k);
            let vel_old = old_grid.velocity_at_center(i, j, k);

            // PIC: directly assign grid velocity
            let vel_pic = vel_new;

            // FLIP: add velocity change from grid
            let vel_flip = particle.velocity + (vel_new - vel_old);

            // Blend FLIP and PIC
            particle.velocity = vel_flip * self.flip_ratio + vel_pic * (1.0 - self.flip_ratio);
        }
    }

    /// Advect particles with their velocities.
    pub fn advect_particles(&mut self, dt: f64, bounds: (f64, f64, f64)) {
        for particle in &mut self.particles {
            particle.position += particle.velocity * dt;

            // Clamp to bounds
            particle.position.x = particle.position.x.max(0.0).min(bounds.0);
            particle.position.y = particle.position.y.max(0.0).min(bounds.1);
            particle.position.z = particle.position.z.max(0.0).min(bounds.2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flip_solver_creation() {
        let solver = FlipSolver::default();
        assert_eq!(solver.flip_ratio, 0.95);
        assert_eq!(solver.particles.len(), 0);
    }

    #[test]
    fn test_particle_initialization() {
        let grid = MacGrid::new((3, 3, 3), 1.0);
        let mut solver = FlipSolver::new(0.95);

        solver.initialize_from_grid(&grid, 2);

        // Should have 2 particles per cell
        assert!(solver.particles.len() >= 3 * 3 * 3 * 2);
    }

    #[test]
    fn test_particles_to_grid() {
        let mut grid = MacGrid::new((3, 3, 3), 1.0);
        let mut solver = FlipSolver::new(0.95);

        // Add a particle with velocity
        solver.particles.push(FlipParticle {
            position: Vector3::new(1.5, 1.5, 1.5),
            velocity: Vector3::new(2.0, 3.0, 4.0),
        });

        solver.particles_to_grid(&mut grid);

        // Grid should have non-zero velocities
        assert!(grid.u.iter().any(|&v| v != 0.0));
    }

    #[test]
    fn test_particle_advection() {
        let mut solver = FlipSolver::new(0.95);

        solver.particles.push(FlipParticle {
            position: Vector3::new(1.0, 1.0, 1.0),
            velocity: Vector3::new(1.0, 0.0, 0.0),
        });

        solver.advect_particles(0.5, (10.0, 10.0, 10.0));

        // Particle should have moved
        assert_eq!(solver.particles[0].position.x, 1.5);
    }
}
