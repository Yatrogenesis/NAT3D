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

//! MAC (Marker-And-Cell) grid for fluid simulation.
//!
//! Staggered grid where velocity components are stored at cell faces
//! and scalar quantities (pressure, density) are stored at cell centers.

use nalgebra::Vector3;

/// MAC grid for fluid simulation.
#[derive(Debug, Clone)]
pub struct MacGrid {
    /// Grid resolution (nx, ny, nz).
    pub resolution: (usize, usize, usize),
    /// Cell size.
    pub cell_size: f64,
    /// U velocity component (stored at X faces).
    pub u: Vec<f64>,
    /// V velocity component (stored at Y faces).
    pub v: Vec<f64>,
    /// W velocity component (stored at Z faces).
    pub w: Vec<f64>,
    /// Pressure field (cell-centered).
    pub pressure: Vec<f64>,
    /// Density field (cell-centered).
    pub density: Vec<f64>,
    /// Marker particles (for free surface tracking).
    pub markers: Vec<Vector3<f64>>,
}

impl MacGrid {
    /// Create a new MAC grid.
    pub fn new(resolution: (usize, usize, usize), cell_size: f64) -> Self {
        let (nx, ny, nz) = resolution;
        let cell_count = nx * ny * nz;

        Self {
            resolution,
            cell_size,
            u: vec![0.0; (nx + 1) * ny * nz], // (nx+1) × ny × nz
            v: vec![0.0; nx * (ny + 1) * nz], // nx × (ny+1) × nz
            w: vec![0.0; nx * ny * (nz + 1)], // nx × ny × (nz+1)
            pressure: vec![0.0; cell_count],
            density: vec![1000.0; cell_count], // Default: water density (kg/m^3)
            markers: Vec::new(),
        }
    }

    /// Get U velocity index.
    pub fn u_index(&self, i: usize, j: usize, k: usize) -> usize {
        let (nx, ny, _nz) = self.resolution;
        i + j * (nx + 1) + k * (nx + 1) * ny
    }

    /// Get V velocity index.
    pub fn v_index(&self, i: usize, j: usize, k: usize) -> usize {
        let (nx, ny, _nz) = self.resolution;
        i + j * nx + k * nx * (ny + 1)
    }

    /// Get W velocity index.
    pub fn w_index(&self, i: usize, j: usize, k: usize) -> usize {
        let (nx, ny, _) = self.resolution;
        i + j * nx + k * nx * ny
    }

    /// Get cell-centered scalar index.
    pub fn cell_index(&self, i: usize, j: usize, k: usize) -> usize {
        let (nx, ny, _) = self.resolution;
        i + j * nx + k * nx * ny
    }

    /// Get velocity at cell center (interpolated).
    pub fn velocity_at_center(&self, i: usize, j: usize, k: usize) -> Vector3<f64> {
        let u = 0.5 * (self.u[self.u_index(i, j, k)] + self.u[self.u_index(i + 1, j, k)]);
        let v = 0.5 * (self.v[self.v_index(i, j, k)] + self.v[self.v_index(i, j + 1, k)]);
        let w = 0.5 * (self.w[self.w_index(i, j, k)] + self.w[self.w_index(i, j, k + 1)]);
        Vector3::new(u, v, w)
    }

    /// Get pressure at cell.
    pub fn pressure_at(&self, i: usize, j: usize, k: usize) -> f64 {
        self.pressure[self.cell_index(i, j, k)]
    }

    /// Set pressure at cell.
    pub fn set_pressure(&mut self, i: usize, j: usize, k: usize, p: f64) {
        let idx = self.cell_index(i, j, k);
        self.pressure[idx] = p;
    }

    /// Get density at cell.
    pub fn density_at(&self, i: usize, j: usize, k: usize) -> f64 {
        self.density[self.cell_index(i, j, k)]
    }

    /// Clear velocities.
    pub fn clear_velocities(&mut self) {
        self.u.fill(0.0);
        self.v.fill(0.0);
        self.w.fill(0.0);
    }

    /// Add marker particle.
    pub fn add_marker(&mut self, position: Vector3<f64>) {
        self.markers.push(position);
    }

    /// Clear all markers.
    pub fn clear_markers(&mut self) {
        self.markers.clear();
    }

    /// Check if cell is inside grid bounds.
    pub fn is_valid_cell(&self, i: isize, j: isize, k: isize) -> bool {
        let (nx, ny, nz) = self.resolution;
        i >= 0 && j >= 0 && k >= 0 && (i as usize) < nx && (j as usize) < ny && (k as usize) < nz
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_creation() {
        let grid = MacGrid::new((10, 10, 10), 0.1);
        assert_eq!(grid.resolution, (10, 10, 10));
        assert_eq!(grid.cell_size, 0.1);
        assert_eq!(grid.u.len(), 11 * 10 * 10);
        assert_eq!(grid.v.len(), 10 * 11 * 10);
        assert_eq!(grid.w.len(), 10 * 10 * 11);
        assert_eq!(grid.pressure.len(), 1000);
    }

    #[test]
    fn test_indexing() {
        let grid = MacGrid::new((5, 5, 5), 1.0);
        let u_idx = grid.u_index(0, 0, 0);
        let v_idx = grid.v_index(0, 0, 0);
        let w_idx = grid.w_index(0, 0, 0);
        let cell_idx = grid.cell_index(0, 0, 0);

        assert_eq!(u_idx, 0);
        assert_eq!(v_idx, 0);
        assert_eq!(w_idx, 0);
        assert_eq!(cell_idx, 0);
    }

    #[test]
    fn test_velocity_interpolation() {
        let mut grid = MacGrid::new((3, 3, 3), 1.0);
        let idx1 = grid.u_index(1, 1, 1);
        let idx2 = grid.u_index(2, 1, 1);
        grid.u[idx1] = 2.0;
        grid.u[idx2] = 4.0;

        let vel = grid.velocity_at_center(1, 1, 1);
        assert_eq!(vel.x, 3.0); // Average of 2.0 and 4.0
    }

    #[test]
    fn test_pressure_access() {
        let mut grid = MacGrid::new((5, 5, 5), 1.0);
        grid.set_pressure(2, 2, 2, 101325.0); // Atmospheric pressure
        assert_eq!(grid.pressure_at(2, 2, 2), 101325.0);
    }

    #[test]
    fn test_markers() {
        let mut grid = MacGrid::new((5, 5, 5), 1.0);
        grid.add_marker(Vector3::new(1.5, 2.5, 3.5));
        grid.add_marker(Vector3::new(2.5, 3.5, 4.5));
        assert_eq!(grid.markers.len(), 2);

        grid.clear_markers();
        assert_eq!(grid.markers.len(), 0);
    }

    #[test]
    fn test_bounds_checking() {
        let grid = MacGrid::new((5, 5, 5), 1.0);
        assert!(grid.is_valid_cell(0, 0, 0));
        assert!(grid.is_valid_cell(4, 4, 4));
        assert!(!grid.is_valid_cell(-1, 0, 0));
        assert!(!grid.is_valid_cell(5, 0, 0));
        assert!(!grid.is_valid_cell(0, 5, 0));
        assert!(!grid.is_valid_cell(0, 0, 5));
    }
}
