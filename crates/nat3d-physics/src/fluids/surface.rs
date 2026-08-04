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

//! Free surface tracking for fluids.
//!
//! Uses marker particles to track the air-water interface.

use super::grid::MacGrid;
use nalgebra::Vector3;

/// Surface tracker using marker particles.
pub struct SurfaceTracker {
    /// Number of markers per cell (for initialization).
    pub markers_per_cell: usize,
}

impl Default for SurfaceTracker {
    fn default() -> Self {
        Self {
            markers_per_cell: 8,
        }
    }
}

impl SurfaceTracker {
    /// Create a new surface tracker.
    pub fn new(markers_per_cell: usize) -> Self {
        Self { markers_per_cell }
    }

    /// Initialize markers in fluid region.
    pub fn initialize_markers(&self, grid: &mut MacGrid, fluid_height: f64) {
        grid.clear_markers();

        let (nx, ny, nz) = grid.resolution;
        let dx = grid.cell_size;

        // Place markers in cells below fluid surface
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let cell_y = (j as f64 + 0.5) * dx;

                    if cell_y < fluid_height {
                        // Place multiple markers per cell
                        for mi in 0..2 {
                            for mj in 0..2 {
                                for mk in 0..2 {
                                    let offset_x = (mi as f64 + 0.25) * 0.5;
                                    let offset_y = (mj as f64 + 0.25) * 0.5;
                                    let offset_z = (mk as f64 + 0.25) * 0.5;

                                    let pos = Vector3::new(
                                        (i as f64 + offset_x) * dx,
                                        (j as f64 + offset_y) * dx,
                                        (k as f64 + offset_z) * dx,
                                    );

                                    grid.add_marker(pos);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Classify cells as fluid, air, or interface.
    pub fn classify_cells(&self, grid: &MacGrid) -> Vec<CellType> {
        let (nx, ny, nz) = grid.resolution;
        let dx = grid.cell_size;
        let mut cell_types = vec![CellType::Air; nx * ny * nz];

        // Count markers in each cell
        for marker in &grid.markers {
            let i = ((marker.x / dx).floor() as isize)
                .max(0)
                .min(nx as isize - 1) as usize;
            let j = ((marker.y / dx).floor() as isize)
                .max(0)
                .min(ny as isize - 1) as usize;
            let k = ((marker.z / dx).floor() as isize)
                .max(0)
                .min(nz as isize - 1) as usize;

            let idx = grid.cell_index(i, j, k);
            cell_types[idx] = CellType::Fluid;
        }

        // Mark interface cells (fluid cells with air neighbors)
        let mut interface_cells = Vec::new();
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let idx = grid.cell_index(i, j, k);

                    if cell_types[idx] == CellType::Fluid {
                        // Check if any neighbor is air
                        let has_air_neighbor = self.has_air_neighbor(grid, &cell_types, i, j, k);

                        if has_air_neighbor {
                            interface_cells.push(idx);
                        }
                    }
                }
            }
        }

        // Mark interface cells
        for idx in interface_cells {
            cell_types[idx] = CellType::Interface;
        }

        cell_types
    }

    /// Check if cell has an air neighbor.
    fn has_air_neighbor(
        &self,
        grid: &MacGrid,
        cell_types: &[CellType],
        i: usize,
        j: usize,
        k: usize,
    ) -> bool {
        let (nx, ny, nz) = grid.resolution;

        // Check 6 neighbors
        let neighbors = [
            (i.wrapping_sub(1), j, k),
            (i + 1, j, k),
            (i, j.wrapping_sub(1), k),
            (i, j + 1, k),
            (i, j, k.wrapping_sub(1)),
            (i, j, k + 1),
        ];

        for (ni, nj, nk) in neighbors {
            if ni < nx && nj < ny && nk < nz {
                let n_idx = grid.cell_index(ni, nj, nk);
                if cell_types[n_idx] == CellType::Air {
                    return true;
                }
            }
        }

        false
    }

    /// Advect markers with velocity field.
    pub fn advect_markers(&self, grid: &mut MacGrid, dt: f64) {
        let dx = grid.cell_size;
        let (nx, ny, nz) = grid.resolution;

        // Collect velocities first to avoid borrow conflict
        let velocities: Vec<_> = grid
            .markers
            .iter()
            .map(|marker| {
                let i = ((marker.x / dx).floor() as isize)
                    .max(0)
                    .min(nx as isize - 1) as usize;
                let j = ((marker.y / dx).floor() as isize)
                    .max(0)
                    .min(ny as isize - 1) as usize;
                let k = ((marker.z / dx).floor() as isize)
                    .max(0)
                    .min(nz as isize - 1) as usize;
                grid.velocity_at_center(i, j, k)
            })
            .collect();

        // Apply velocities to markers
        for (marker, vel) in grid.markers.iter_mut().zip(velocities.iter()) {
            // Advect marker (Forward Euler)
            marker.x += vel.x * dt;
            marker.y += vel.y * dt;
            marker.z += vel.z * dt;

            // Clamp to grid bounds
            marker.x = marker.x.max(0.0).min((nx as f64) * dx);
            marker.y = marker.y.max(0.0).min((ny as f64) * dx);
            marker.z = marker.z.max(0.0).min((nz as f64) * dx);
        }
    }
}

/// Cell type classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellType {
    /// Air cell (no markers).
    Air,
    /// Fluid cell (contains markers).
    Fluid,
    /// Interface cell (fluid with air neighbor).
    Interface,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_surface_tracker_creation() {
        let tracker = SurfaceTracker::default();
        assert_eq!(tracker.markers_per_cell, 8);
    }

    #[test]
    fn test_marker_initialization() {
        let mut grid = MacGrid::new((5, 5, 5), 1.0);
        let tracker = SurfaceTracker::default();

        tracker.initialize_markers(&mut grid, 2.5);

        // Should have markers in cells below y=2.5
        assert!(grid.markers.len() > 0);
    }

    #[test]
    fn test_cell_classification() {
        let mut grid = MacGrid::new((5, 5, 5), 1.0);
        let tracker = SurfaceTracker::default();

        tracker.initialize_markers(&mut grid, 2.5);
        let cell_types = tracker.classify_cells(&grid);

        // Should have fluid, air, and interface cells
        assert!(cell_types.contains(&CellType::Fluid));
        assert!(cell_types.contains(&CellType::Air));
    }

    #[test]
    fn test_marker_advection() {
        let mut grid = MacGrid::new((5, 5, 5), 1.0);
        grid.add_marker(Vector3::new(2.5, 2.5, 2.5));

        // Set velocity
        let idx = grid.u_index(2, 2, 2);
        grid.u[idx] = 1.0;

        let tracker = SurfaceTracker::default();
        tracker.advect_markers(&mut grid, 0.1);

        // Marker should have moved
        assert!(grid.markers[0].x > 2.5);
    }
}
