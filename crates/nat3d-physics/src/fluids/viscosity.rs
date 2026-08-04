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

//! Viscosity solver for fluid simulation.
//!
//! Handles diffusion of momentum due to fluid viscosity.

use super::grid::MacGrid;

/// Viscosity solver for fluids.
pub struct ViscositySolver {
    /// Dynamic viscosity (Pa·s). Water at 20°C: 0.001, Honey: ~10.
    pub viscosity: f64,
    /// Number of Jacobi iterations.
    pub iterations: usize,
}

impl Default for ViscositySolver {
    fn default() -> Self {
        Self {
            viscosity: 0.001, // Water viscosity
            iterations: 20,
        }
    }
}

impl ViscositySolver {
    /// Create a new viscosity solver.
    pub fn new(viscosity: f64, iterations: usize) -> Self {
        Self {
            viscosity,
            iterations,
        }
    }

    /// Apply viscosity diffusion using Jacobi iteration.
    pub fn apply(&self, grid: &mut MacGrid, dt: f64) {
        let (nx, ny, nz) = grid.resolution;
        let dx = grid.cell_size;
        let rho = 1000.0; // Fluid density (kg/m^3)
        let nu = self.viscosity / rho; // Kinematic viscosity (m^2/s)
        let alpha = dx * dx / (nu * dt);

        // Create temporary buffers for velocities
        let mut u_temp = grid.u.clone();
        let mut v_temp = grid.v.clone();
        let mut w_temp = grid.w.clone();

        // Jacobi iterations for U velocity
        for _ in 0..self.iterations {
            for k in 0..nz {
                for j in 0..ny {
                    for i in 1..nx {
                        let idx = grid.u_index(i, j, k);

                        // Gather neighbors with boundary handling
                        let u_left = if i > 1 {
                            u_temp[grid.u_index(i - 1, j, k)]
                        } else {
                            u_temp[idx]
                        };

                        let u_right = if i < nx - 1 {
                            u_temp[grid.u_index(i + 1, j, k)]
                        } else {
                            u_temp[idx]
                        };

                        let u_bottom = if j > 0 {
                            u_temp[grid.u_index(i, j - 1, k)]
                        } else {
                            u_temp[idx]
                        };

                        let u_top = if j < ny - 1 {
                            u_temp[grid.u_index(i, j + 1, k)]
                        } else {
                            u_temp[idx]
                        };

                        let u_back = if k > 0 {
                            u_temp[grid.u_index(i, j, k - 1)]
                        } else {
                            u_temp[idx]
                        };

                        let u_front = if k < nz - 1 {
                            u_temp[grid.u_index(i, j, k + 1)]
                        } else {
                            u_temp[idx]
                        };

                        let u_center = grid.u[idx];

                        grid.u[idx] = (u_center * alpha
                            + u_left
                            + u_right
                            + u_bottom
                            + u_top
                            + u_back
                            + u_front)
                            / (alpha + 6.0);
                    }
                }
            }
            u_temp.copy_from_slice(&grid.u);
        }

        // Jacobi iterations for V velocity
        for _ in 0..self.iterations {
            for k in 0..nz {
                for j in 1..ny {
                    for i in 0..nx {
                        let idx = grid.v_index(i, j, k);

                        let v_left = if i > 0 {
                            v_temp[grid.v_index(i - 1, j, k)]
                        } else {
                            v_temp[idx]
                        };

                        let v_right = if i < nx - 1 {
                            v_temp[grid.v_index(i + 1, j, k)]
                        } else {
                            v_temp[idx]
                        };

                        let v_bottom = if j > 1 {
                            v_temp[grid.v_index(i, j - 1, k)]
                        } else {
                            v_temp[idx]
                        };

                        let v_top = if j < ny - 1 {
                            v_temp[grid.v_index(i, j + 1, k)]
                        } else {
                            v_temp[idx]
                        };

                        let v_back = if k > 0 {
                            v_temp[grid.v_index(i, j, k - 1)]
                        } else {
                            v_temp[idx]
                        };

                        let v_front = if k < nz - 1 {
                            v_temp[grid.v_index(i, j, k + 1)]
                        } else {
                            v_temp[idx]
                        };

                        let v_center = grid.v[idx];

                        grid.v[idx] = (v_center * alpha
                            + v_left
                            + v_right
                            + v_bottom
                            + v_top
                            + v_back
                            + v_front)
                            / (alpha + 6.0);
                    }
                }
            }
            v_temp.copy_from_slice(&grid.v);
        }

        // Jacobi iterations for W velocity
        for _ in 0..self.iterations {
            for k in 1..nz {
                for j in 0..ny {
                    for i in 0..nx {
                        let idx = grid.w_index(i, j, k);

                        let w_left = if i > 0 {
                            w_temp[grid.w_index(i - 1, j, k)]
                        } else {
                            w_temp[idx]
                        };

                        let w_right = if i < nx - 1 {
                            w_temp[grid.w_index(i + 1, j, k)]
                        } else {
                            w_temp[idx]
                        };

                        let w_bottom = if j > 0 {
                            w_temp[grid.w_index(i, j - 1, k)]
                        } else {
                            w_temp[idx]
                        };

                        let w_top = if j < ny - 1 {
                            w_temp[grid.w_index(i, j + 1, k)]
                        } else {
                            w_temp[idx]
                        };

                        let w_back = if k > 1 {
                            w_temp[grid.w_index(i, j, k - 1)]
                        } else {
                            w_temp[idx]
                        };

                        let w_front = if k < nz - 1 {
                            w_temp[grid.w_index(i, j, k + 1)]
                        } else {
                            w_temp[idx]
                        };

                        let w_center = grid.w[idx];

                        grid.w[idx] = (w_center * alpha
                            + w_left
                            + w_right
                            + w_bottom
                            + w_top
                            + w_back
                            + w_front)
                            / (alpha + 6.0);
                    }
                }
            }
            w_temp.copy_from_slice(&grid.w);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_viscosity_solver_creation() {
        let solver = ViscositySolver::default();
        assert_eq!(solver.viscosity, 0.001);
        assert_eq!(solver.iterations, 20);
    }

    #[test]
    fn test_viscosity_diffusion() {
        let mut grid = MacGrid::new((5, 5, 5), 1.0);

        // Set up velocity spike
        let idx = grid.u_index(2, 2, 2);
        grid.u[idx] = 10.0;

        let solver = ViscositySolver::new(0.01, 10);
        solver.apply(&mut grid, 0.01);

        // Velocity should diffuse to neighbors
        let idx_center = grid.u_index(2, 2, 2);
        let u_center = grid.u[idx_center];
        assert!(u_center < 10.0); // Should decrease due to diffusion
    }

    #[test]
    fn test_high_viscosity() {
        let mut grid = MacGrid::new((3, 3, 3), 1.0);
        let idx = grid.u_index(1, 1, 1);
        grid.u[idx] = 5.0;

        // Honey viscosity (high)
        let solver = ViscositySolver::new(10.0, 20);
        solver.apply(&mut grid, 0.01);

        // High viscosity should diffuse velocity more
        let u_center = grid.u[grid.u_index(1, 1, 1)];
        assert!(u_center < 5.0);
    }
}
