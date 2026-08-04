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

//! Pressure projection for incompressible fluid simulation.
//!
//! Solves the Poisson equation to enforce divergence-free velocity fields.

use super::grid::MacGrid;

/// Pressure solver for incompressible fluids.
pub struct PressureSolver {
    /// Maximum iterations for iterative solver.
    pub max_iterations: usize,
    /// Convergence tolerance.
    pub tolerance: f64,
}

impl Default for PressureSolver {
    fn default() -> Self {
        Self {
            max_iterations: 100,
            tolerance: 1e-6,
        }
    }
}

impl PressureSolver {
    /// Create a new pressure solver.
    pub fn new(max_iterations: usize, tolerance: f64) -> Self {
        Self {
            max_iterations,
            tolerance,
        }
    }

    /// Compute velocity divergence at cell center.
    pub fn compute_divergence(&self, grid: &MacGrid, i: usize, j: usize, k: usize) -> f64 {
        let dx = grid.cell_size;

        let u_right = grid.u[grid.u_index(i + 1, j, k)];
        let u_left = grid.u[grid.u_index(i, j, k)];

        let v_top = grid.v[grid.v_index(i, j + 1, k)];
        let v_bottom = grid.v[grid.v_index(i, j, k)];

        let w_front = grid.w[grid.w_index(i, j, k + 1)];
        let w_back = grid.w[grid.w_index(i, j, k)];

        ((u_right - u_left) + (v_top - v_bottom) + (w_front - w_back)) / dx
    }

    /// Solve pressure Poisson equation using Gauss-Seidel.
    pub fn solve(&self, grid: &mut MacGrid, dt: f64) {
        let (nx, ny, nz) = grid.resolution;
        let dx = grid.cell_size;
        let rho = 1000.0; // Water density (kg/m^3)

        // Gauss-Seidel iterations
        for _iter in 0..self.max_iterations {
            let mut max_residual = 0.0_f64;

            for k in 0..nz {
                for j in 0..ny {
                    for i in 0..nx {
                        // Compute divergence
                        let div = self.compute_divergence(grid, i, j, k);

                        // Gather neighbor pressures (with boundary conditions)
                        let p_left = if i > 0 {
                            grid.pressure_at(i - 1, j, k)
                        } else {
                            grid.pressure_at(i, j, k)
                        };

                        let p_right = if i < nx - 1 {
                            grid.pressure_at(i + 1, j, k)
                        } else {
                            grid.pressure_at(i, j, k)
                        };

                        let p_bottom = if j > 0 {
                            grid.pressure_at(i, j - 1, k)
                        } else {
                            grid.pressure_at(i, j, k)
                        };

                        let p_top = if j < ny - 1 {
                            grid.pressure_at(i, j + 1, k)
                        } else {
                            grid.pressure_at(i, j, k)
                        };

                        let p_back = if k > 0 {
                            grid.pressure_at(i, j, k - 1)
                        } else {
                            grid.pressure_at(i, j, k)
                        };

                        let p_front = if k < nz - 1 {
                            grid.pressure_at(i, j, k + 1)
                        } else {
                            grid.pressure_at(i, j, k)
                        };

                        // Update pressure (discrete Poisson equation)
                        let p_new = (p_left + p_right + p_bottom + p_top + p_back + p_front
                            - rho * dx * dx * div / dt)
                            / 6.0;

                        let residual = (p_new - grid.pressure_at(i, j, k)).abs();
                        max_residual = max_residual.max(residual);

                        grid.set_pressure(i, j, k, p_new);
                    }
                }
            }

            // Check convergence
            if max_residual < self.tolerance {
                break;
            }
        }
    }

    /// Apply pressure gradient to make velocity divergence-free.
    pub fn apply_pressure_gradient(&self, grid: &mut MacGrid, dt: f64) {
        let (nx, ny, nz) = grid.resolution;
        let dx = grid.cell_size;
        let rho = 1000.0;

        // Update U velocities
        for k in 0..nz {
            for j in 0..ny {
                for i in 1..nx {
                    let p_left = grid.pressure_at(i - 1, j, k);
                    let p_right = grid.pressure_at(i, j, k);
                    let idx = grid.u_index(i, j, k);
                    grid.u[idx] -= dt * (p_right - p_left) / (rho * dx);
                }
            }
        }

        // Update V velocities
        for k in 0..nz {
            for j in 1..ny {
                for i in 0..nx {
                    let p_bottom = grid.pressure_at(i, j - 1, k);
                    let p_top = grid.pressure_at(i, j, k);
                    let idx = grid.v_index(i, j, k);
                    grid.v[idx] -= dt * (p_top - p_bottom) / (rho * dx);
                }
            }
        }

        // Update W velocities
        for k in 1..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let p_back = grid.pressure_at(i, j, k - 1);
                    let p_front = grid.pressure_at(i, j, k);
                    let idx = grid.w_index(i, j, k);
                    grid.w[idx] -= dt * (p_front - p_back) / (rho * dx);
                }
            }
        }
    }

    /// Project velocity field to be divergence-free.
    pub fn project(&self, grid: &mut MacGrid, dt: f64) {
        self.solve(grid, dt);
        self.apply_pressure_gradient(grid, dt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_solver_creation() {
        let solver = PressureSolver::default();
        assert_eq!(solver.max_iterations, 100);
        assert_eq!(solver.tolerance, 1e-6);
    }

    #[test]
    fn test_divergence_computation() {
        let mut grid = MacGrid::new((5, 5, 5), 1.0);

        // Set up expanding flow (positive divergence)
        let idx1 = grid.u_index(2, 2, 2);
        let idx2 = grid.u_index(3, 2, 2);
        grid.u[idx1] = -1.0;
        grid.u[idx2] = 1.0;

        let solver = PressureSolver::default();
        let div = solver.compute_divergence(&grid, 2, 2, 2);

        assert!(div > 0.0); // Positive divergence (source)
    }

    #[test]
    fn test_pressure_projection() {
        let mut grid = MacGrid::new((5, 5, 5), 0.1);

        // Set up non-divergence-free velocity
        for i in 0..6 {
            let idx = grid.u_index(i, 2, 2);
            grid.u[idx] = i as f64;
        }

        let solver = PressureSolver::new(50, 1e-4);
        solver.project(&mut grid, 0.01);

        // After projection, divergence should be reduced
        let div_after = solver.compute_divergence(&grid, 2, 2, 2);
        assert!(div_after.abs() < 1.0); // Should be closer to zero
    }

    #[test]
    fn test_pressure_gradient() {
        let mut grid = MacGrid::new((5, 5, 5), 1.0);
        grid.set_pressure(1, 2, 2, 0.0);
        grid.set_pressure(2, 2, 2, 100.0);

        let solver = PressureSolver::default();
        solver.apply_pressure_gradient(&mut grid, 0.01);

        // Velocity should be affected by pressure gradient
        let u = grid.u[grid.u_index(2, 2, 2)];
        assert!(u < 0.0); // Flow from high to low pressure
    }
}
