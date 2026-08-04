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

//! Fast Navier-Stokes solver for real-time fluid simulation.
//!
//! Simplified version optimized for interactive performance.

use super::grid::MacGrid;
use super::pressure::PressureSolver;
use super::viscosity::ViscositySolver;
use nalgebra::Vector3;

/// Fast Navier-Stokes solver.
pub struct FastNavierStokesSolver {
    /// Pressure solver.
    pub pressure_solver: PressureSolver,
    /// Viscosity solver (optional for performance).
    pub viscosity_solver: Option<ViscositySolver>,
    /// External forces (e.g., gravity).
    pub external_force: Vector3<f64>,
}

impl Default for FastNavierStokesSolver {
    fn default() -> Self {
        Self {
            pressure_solver: PressureSolver::new(20, 1e-3), // Fewer iterations for speed
            viscosity_solver: None,                         // Disabled by default
            external_force: Vector3::new(0.0, -9.81, 0.0),  // Gravity
        }
    }
}

impl FastNavierStokesSolver {
    /// Create a new fast Navier-Stokes solver.
    pub fn new(pressure_iterations: usize, enable_viscosity: bool) -> Self {
        Self {
            pressure_solver: PressureSolver::new(pressure_iterations, 1e-3),
            viscosity_solver: if enable_viscosity {
                Some(ViscositySolver::new(0.001, 10))
            } else {
                None
            },
            external_force: Vector3::new(0.0, -9.81, 0.0),
        }
    }

    /// Apply external forces to velocity field.
    pub fn apply_forces(&self, grid: &mut MacGrid, dt: f64) {
        let (nx, ny, nz) = grid.resolution;

        // Apply gravity to V velocities (Y-component)
        for k in 0..nz {
            for j in 0..ny + 1 {
                for i in 0..nx {
                    let idx = grid.v_index(i, j, k);
                    grid.v[idx] += self.external_force.y * dt;
                }
            }
        }

        // Apply X and Z forces if non-zero
        if self.external_force.x.abs() > 1e-10 {
            for k in 0..nz {
                for j in 0..ny {
                    for i in 0..nx + 1 {
                        let idx = grid.u_index(i, j, k);
                        grid.u[idx] += self.external_force.x * dt;
                    }
                }
            }
        }

        if self.external_force.z.abs() > 1e-10 {
            for k in 0..nz + 1 {
                for j in 0..ny {
                    for i in 0..nx {
                        let idx = grid.w_index(i, j, k);
                        grid.w[idx] += self.external_force.z * dt;
                    }
                }
            }
        }
    }

    /// Advect velocity field (semi-Lagrangian).
    pub fn advect_velocity(&self, grid: &mut MacGrid, dt: f64) {
        let (nx, ny, nz) = grid.resolution;
        let dx = grid.cell_size;

        // Store old velocities
        let u_old = grid.u.clone();
        let v_old = grid.v.clone();
        let w_old = grid.w.clone();

        // Advect U velocities
        for k in 0..nz {
            for j in 0..ny {
                for i in 1..nx {
                    let u_idx = grid.u_index(i, j, k);

                    // U is at (i*dx, (j+0.5)*dx, (k+0.5)*dx)
                    let x = i as f64 * dx;
                    let y = (j as f64 + 0.5) * dx;
                    let z = (k as f64 + 0.5) * dx;

                    let u = u_old[u_idx];
                    let v = 0.5
                        * (v_old[grid.v_index(i.saturating_sub(1), j, k)]
                            + v_old[grid.v_index(i, j, k)]);
                    let w = 0.5
                        * (w_old[grid.w_index(i.saturating_sub(1), j, k)]
                            + w_old[grid.w_index(i, j, k)]);

                    // Trace back
                    let x_back = (x - u * dt).max(0.0).min((nx as f64) * dx);
                    let y_back = (y - v * dt).max(0.0).min((ny as f64) * dx);
                    let z_back = (z - w * dt).max(0.0).min((nz as f64) * dx);

                    // Sample at traced position (simple nearest neighbor for speed)
                    let i_back = (x_back / dx).floor().max(0.0).min((nx - 1) as f64) as usize;
                    let j_back = (y_back / dx - 0.5).floor().max(0.0).min((ny - 1) as f64) as usize;
                    let k_back = (z_back / dx - 0.5).floor().max(0.0).min((nz - 1) as f64) as usize;

                    grid.u[u_idx] = u_old[grid.u_index(i_back.min(nx - 1), j_back, k_back)];
                }
            }
        }

        // Advect V velocities (similar logic)
        for k in 0..nz {
            for j in 1..ny {
                for i in 0..nx {
                    let v_idx = grid.v_index(i, j, k);

                    let x = (i as f64 + 0.5) * dx;
                    let y = j as f64 * dx;
                    let z = (k as f64 + 0.5) * dx;

                    let u = 0.5
                        * (u_old[grid.u_index(i, j.saturating_sub(1), k)]
                            + u_old[grid.u_index(i, j, k)]);
                    let v = v_old[v_idx];
                    let w = 0.5
                        * (w_old[grid.w_index(i, j.saturating_sub(1), k)]
                            + w_old[grid.w_index(i, j, k)]);

                    let x_back = (x - u * dt).max(0.0).min((nx as f64) * dx);
                    let y_back = (y - v * dt).max(0.0).min((ny as f64) * dx);
                    let z_back = (z - w * dt).max(0.0).min((nz as f64) * dx);

                    let i_back = (x_back / dx - 0.5).floor().max(0.0).min((nx - 1) as f64) as usize;
                    let j_back = (y_back / dx).floor().max(0.0).min((ny - 1) as f64) as usize;
                    let k_back = (z_back / dx - 0.5).floor().max(0.0).min((nz - 1) as f64) as usize;

                    grid.v[v_idx] = v_old[grid.v_index(i_back, j_back.min(ny - 1), k_back)];
                }
            }
        }

        // Advect W velocities
        for k in 1..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let w_idx = grid.w_index(i, j, k);

                    let x = (i as f64 + 0.5) * dx;
                    let y = (j as f64 + 0.5) * dx;
                    let z = k as f64 * dx;

                    let u = 0.5
                        * (u_old[grid.u_index(i, j, k.saturating_sub(1))]
                            + u_old[grid.u_index(i, j, k)]);
                    let v = 0.5
                        * (v_old[grid.v_index(i, j, k.saturating_sub(1))]
                            + v_old[grid.v_index(i, j, k)]);
                    let w = w_old[w_idx];

                    let x_back = (x - u * dt).max(0.0).min((nx as f64) * dx);
                    let y_back = (y - v * dt).max(0.0).min((ny as f64) * dx);
                    let z_back = (z - w * dt).max(0.0).min((nz as f64) * dx);

                    let i_back = (x_back / dx - 0.5).floor().max(0.0).min((nx - 1) as f64) as usize;
                    let j_back = (y_back / dx - 0.5).floor().max(0.0).min((ny - 1) as f64) as usize;
                    let k_back = (z_back / dx).floor().max(0.0).min((nz - 1) as f64) as usize;

                    grid.w[w_idx] = w_old[grid.w_index(i_back, j_back, k_back.min(nz - 1))];
                }
            }
        }
    }

    /// Single simulation step.
    pub fn step(&mut self, grid: &mut MacGrid, dt: f64) {
        // 1. Apply external forces
        self.apply_forces(grid, dt);

        // 2. Advect velocity
        self.advect_velocity(grid, dt);

        // 3. Apply viscosity (optional)
        if let Some(ref viscosity_solver) = self.viscosity_solver {
            viscosity_solver.apply(grid, dt);
        }

        // 4. Pressure projection (enforce incompressibility)
        self.pressure_solver.project(grid, dt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fast_solver_creation() {
        let solver = FastNavierStokesSolver::default();
        assert_eq!(solver.pressure_solver.max_iterations, 20);
        assert!(solver.viscosity_solver.is_none());
        assert_eq!(solver.external_force.y, -9.81);
    }

    #[test]
    fn test_force_application() {
        let mut grid = MacGrid::new((3, 3, 3), 1.0);
        let solver = FastNavierStokesSolver::default();

        solver.apply_forces(&mut grid, 0.01);

        // V velocities should be affected by gravity
        assert!(grid.v.iter().any(|&v| v < 0.0));
    }

    #[test]
    fn test_simulation_step() {
        let mut grid = MacGrid::new((5, 5, 5), 0.1);
        let mut solver = FastNavierStokesSolver::new(10, false);

        // Set initial velocity
        let idx = grid.v_index(2, 2, 2);
        grid.v[idx] = 5.0;

        solver.step(&mut grid, 0.01);

        // Velocity field should have evolved
        // (exact values depend on solver implementation)
        assert!(grid.v.iter().sum::<f64>() != 0.0);
    }
}
