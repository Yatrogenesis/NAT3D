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

//! Inflation/ballooning for sketch-to-mesh.

use super::path::Path2D;
use super::MeshResult;
use std::f32::consts::PI;

/// Inflate a 2D path to create a balloon-like 3D mesh.
pub fn inflate_path(path: &Path2D, amount: f32, resolution: u32) -> MeshResult {
    let mut mesh = MeshResult::new();
    let points = path.tesselate(resolution);

    if points.len() < 3 {
        return mesh;
    }

    // Find center of the path
    let mut cx = 0.0;
    let mut cy = 0.0;
    for p in &points {
        cx += p.x;
        cy += p.y;
    }
    cx /= points.len() as f32;
    cy /= points.len() as f32;

    // Create inflated mesh using dome-like inflation
    // Front and back hemispheres
    let slices = (resolution / 2).max(4);

    // Create rings from edge to center
    let mut front_rings: Vec<Vec<u32>> = Vec::new();
    let mut back_rings: Vec<Vec<u32>> = Vec::new();

    for si in 0..=slices {
        let t = si as f32 / slices as f32;
        let angle = t * PI / 2.0;
        let scale = angle.cos(); // Shrink toward center
        let z = amount * angle.sin();

        let mut front_ring = Vec::new();
        let mut back_ring = Vec::new();

        for p in &points {
            // Scale point toward center
            let x = cx + (p.x - cx) * scale;
            let y = cy + (p.y - cy) * scale;

            // Front (positive Z)
            let idx = mesh.add_vertex([x, y, z], [0.0, 0.0, 1.0], [t, 0.0]);
            front_ring.push(idx);

            // Back (negative Z)
            let idx = mesh.add_vertex([x, y, -z], [0.0, 0.0, -1.0], [t, 0.0]);
            back_ring.push(idx);
        }

        front_rings.push(front_ring);
        back_rings.push(back_ring);
    }

    // Connect front rings
    let n = points.len();
    for ri in 0..slices as usize {
        let ring0 = &front_rings[ri];
        let ring1 = &front_rings[ri + 1];

        for pi in 0..n {
            let next = (pi + 1) % n;
            mesh.add_quad(ring0[pi], ring0[next], ring1[next], ring1[pi]);
        }
    }

    // Connect back rings (reversed winding)
    for ri in 0..slices as usize {
        let ring0 = &back_rings[ri];
        let ring1 = &back_rings[ri + 1];

        for pi in 0..n {
            let next = (pi + 1) % n;
            mesh.add_quad(ring0[next], ring0[pi], ring1[pi], ring1[next]);
        }
    }

    // Close the top (center point)
    let front_center = mesh.add_vertex([cx, cy, amount], [0.0, 0.0, 1.0], [1.0, 0.5]);
    let back_center = mesh.add_vertex([cx, cy, -amount], [0.0, 0.0, -1.0], [1.0, 0.5]);

    let last_front_ring = front_rings.last().unwrap();
    let last_back_ring = back_rings.last().unwrap();

    for pi in 0..n {
        let next = (pi + 1) % n;
        mesh.add_triangle(last_front_ring[pi], last_front_ring[next], front_center);
        mesh.add_triangle(last_back_ring[next], last_back_ring[pi], back_center);
    }

    mesh.recalculate_normals();
    mesh
}

/// Inflate with variable thickness.
pub fn inflate_variable(
    path: &Path2D,
    center_height: f32,
    edge_height: f32,
    resolution: u32,
) -> MeshResult {
    let mut mesh = MeshResult::new();
    let points = path.tesselate(resolution);

    if points.len() < 3 {
        return mesh;
    }

    // Find center and bounds
    let mut cx = 0.0;
    let mut cy = 0.0;
    for p in &points {
        cx += p.x;
        cy += p.y;
    }
    cx /= points.len() as f32;
    cy /= points.len() as f32;

    // Find max distance from center (for normalization)
    let max_dist = points
        .iter()
        .map(|p| {
            let dx = p.x - cx;
            let dy = p.y - cy;
            (dx * dx + dy * dy).sqrt()
        })
        .fold(0.0f32, f32::max);

    // Create surface
    let grid_res = resolution;
    let bounds = path.bounds();
    let width = bounds.1.x - bounds.0.x;
    let height = bounds.1.y - bounds.0.y;

    // Create grid of vertices
    let mut grid: Vec<Vec<Option<u32>>> = Vec::new();

    for yi in 0..=grid_res {
        let mut row = Vec::new();
        for xi in 0..=grid_res {
            let x = bounds.0.x + width * xi as f32 / grid_res as f32;
            let y = bounds.0.y + height * yi as f32 / grid_res as f32;

            // Check if point is inside path (simplified - use distance from center)
            let dx = x - cx;
            let dy = y - cy;
            let dist = (dx * dx + dy * dy).sqrt();

            if dist <= max_dist * 1.1 {
                let t = dist / max_dist;
                let z = edge_height + (center_height - edge_height) * (1.0 - t * t);

                // Front vertex
                let idx = mesh.add_vertex(
                    [x, y, z],
                    [0.0, 0.0, 1.0],
                    [xi as f32 / grid_res as f32, yi as f32 / grid_res as f32],
                );
                row.push(Some(idx));
            } else {
                row.push(None);
            }
        }
        grid.push(row);
    }

    // Create quads from grid
    for yi in 0..grid_res as usize {
        for xi in 0..grid_res as usize {
            if let (Some(i00), Some(i10), Some(i11), Some(i01)) = (
                grid[yi][xi],
                grid[yi][xi + 1],
                grid[yi + 1][xi + 1],
                grid[yi + 1][xi],
            ) {
                mesh.add_quad(i00, i10, i11, i01);
            }
        }
    }

    mesh.recalculate_normals();
    mesh
}

/// Inflate mesh generator.
pub struct InflateMesh {
    pub amount: f32,
    pub resolution: u32,
    pub symmetric: bool,
}

impl Default for InflateMesh {
    fn default() -> Self {
        Self {
            amount: 0.5,
            resolution: 16,
            symmetric: true,
        }
    }
}

impl InflateMesh {
    /// Create new inflate mesh generator.
    pub fn new(amount: f32) -> Self {
        Self {
            amount,
            ..Default::default()
        }
    }

    /// Generate inflated mesh.
    pub fn generate(&self, path: &Path2D) -> MeshResult {
        inflate_path(path, self.amount, self.resolution)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inflate_circle() {
        let path = Path2D::circle(0.0, 0.0, 1.0);
        let mesh = inflate_path(&path, 0.5, 16);

        assert!(mesh.vertex_count() > 0);
        assert!(mesh.triangle_count() > 0);
    }

    #[test]
    fn test_inflate_star() {
        let path = Path2D::star(0.0, 0.0, 1.0, 0.5, 5);
        let mesh = inflate_path(&path, 0.3, 16);

        assert!(mesh.vertex_count() > 0);
    }
}
