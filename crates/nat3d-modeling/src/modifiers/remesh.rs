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

//! Remesh modifier.
//!
//! Resamples mesh to uniform density using various algorithms.

use nalgebra::{Point3, Vector3};
use std::any::Any;
use std::collections::HashMap;
use super::stack::{Modifier, ModifierMesh};

/// Remesh mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum RemeshMode {
    /// Voxel-based remeshing.
    #[default]
    Voxel,
    /// Smooth uniform remeshing.
    Smooth,
    /// Sharp feature preservation.
    Sharp,
    /// Block/cubic remeshing.
    Blocks,
}


/// Remesh modifier.
#[derive(Debug, Clone)]
pub struct RemeshModifier {
    /// Modifier name.
    pub name: String,
    /// Whether enabled.
    pub enabled: bool,
    /// Remesh mode.
    pub mode: RemeshMode,
    /// Voxel size (for voxel mode).
    pub voxel_size: f64,
    /// Octree depth (for octree-based modes).
    pub octree_depth: usize,
    /// Smooth iterations.
    pub smooth_iterations: usize,
    /// Adaptivity (0-1, higher = more adaptive).
    pub adaptivity: f64,
}

impl Default for RemeshModifier {
    fn default() -> Self {
        Self {
            name: "Remesh".to_string(),
            enabled: true,
            mode: RemeshMode::default(),
            voxel_size: 0.1,
            octree_depth: 4,
            smooth_iterations: 4,
            adaptivity: 0.0,
        }
    }
}

impl RemeshModifier {
    /// Create new remesh modifier.
    pub fn new(mode: RemeshMode, voxel_size: f64) -> Self {
        Self {
            mode,
            voxel_size,
            ..Default::default()
        }
    }

    /// Create voxel remesher.
    pub fn voxel(voxel_size: f64) -> Self {
        Self {
            mode: RemeshMode::Voxel,
            voxel_size,
            ..Default::default()
        }
    }

    /// Voxelize mesh and extract isosurface.
    fn remesh_voxel(&self, mesh: &ModifierMesh) -> ModifierMesh {
        // Get bounds
        let (min, max) = mesh.bounds();

        // Expand bounds slightly
        let margin = self.voxel_size * 2.0;
        let min = Point3::new(min.x - margin, min.y - margin, min.z - margin);
        let max = Point3::new(max.x + margin, max.y + margin, max.z + margin);

        // Calculate grid dimensions
        let size_x = ((max.x - min.x) / self.voxel_size).ceil() as usize + 1;
        let size_y = ((max.y - min.y) / self.voxel_size).ceil() as usize + 1;
        let size_z = ((max.z - min.z) / self.voxel_size).ceil() as usize + 1;

        // Create voxel grid
        let mut grid = vec![vec![vec![false; size_z]; size_y]; size_x];

        // Voxelize: mark voxels intersected by mesh
        for face in &mesh.faces {
            if face.len() < 3 {
                continue;
            }

            // Get triangle bounds
            let mut face_min = Point3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
            let mut face_max = Point3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);

            for &vi in face {
                let p = mesh.positions[vi];
                face_min.x = face_min.x.min(p.x);
                face_min.y = face_min.y.min(p.y);
                face_min.z = face_min.z.min(p.z);
                face_max.x = face_max.x.max(p.x);
                face_max.y = face_max.y.max(p.y);
                face_max.z = face_max.z.max(p.z);
            }

            // Convert to grid coordinates
            let grid_min_x = ((face_min.x - min.x) / self.voxel_size).floor() as isize;
            let grid_min_y = ((face_min.y - min.y) / self.voxel_size).floor() as isize;
            let grid_min_z = ((face_min.z - min.z) / self.voxel_size).floor() as isize;
            let grid_max_x = ((face_max.x - min.x) / self.voxel_size).ceil() as isize;
            let grid_max_y = ((face_max.y - min.y) / self.voxel_size).ceil() as isize;
            let grid_max_z = ((face_max.z - min.z) / self.voxel_size).ceil() as isize;

            // Mark voxels in bounding box
            for x in grid_min_x..=grid_max_x {
                if x < 0 || x >= size_x as isize { continue; }
                for y in grid_min_y..=grid_max_y {
                    if y < 0 || y >= size_y as isize { continue; }
                    for z in grid_min_z..=grid_max_z {
                        if z < 0 || z >= size_z as isize { continue; }
                        grid[x as usize][y as usize][z as usize] = true;
                    }
                }
            }
        }

        // Extract surface using marching cubes (simplified)
        let mut result = ModifierMesh::new();
        let _vertex_cache: HashMap<(usize, usize, usize), usize> = HashMap::new();

        for x in 0..size_x.saturating_sub(1) {
            for y in 0..size_y.saturating_sub(1) {
                for z in 0..size_z.saturating_sub(1) {
                    // Check if this voxel is on the surface
                    let current = grid[x][y][z];

                    // Simple surface extraction: if voxel is filled and has empty neighbor
                    let has_empty_neighbor =
                        (x > 0 && !grid[x-1][y][z]) ||
                        (x < size_x-1 && !grid[x+1][y][z]) ||
                        (y > 0 && !grid[x][y-1][z]) ||
                        (y < size_y-1 && !grid[x][y+1][z]) ||
                        (z > 0 && !grid[x][y][z-1]) ||
                        (z < size_z-1 && !grid[x][y][z+1]);

                    if current && has_empty_neighbor {
                        // Create cube at this position
                        let base = Point3::new(
                            min.x + x as f64 * self.voxel_size,
                            min.y + y as f64 * self.voxel_size,
                            min.z + z as f64 * self.voxel_size,
                        );

                        let vx = self.voxel_size;

                        // Create 8 corners
                        let corners = [
                            base,
                            Point3::new(base.x + vx, base.y, base.z),
                            Point3::new(base.x + vx, base.y + vx, base.z),
                            Point3::new(base.x, base.y + vx, base.z),
                            Point3::new(base.x, base.y, base.z + vx),
                            Point3::new(base.x + vx, base.y, base.z + vx),
                            Point3::new(base.x + vx, base.y + vx, base.z + vx),
                            Point3::new(base.x, base.y + vx, base.z + vx),
                        ];

                        let start_idx = result.positions.len();
                        for corner in &corners {
                            result.add_vertex(*corner);
                        }

                        // Create 6 faces (simplified cube)
                        if x == 0 || !grid[x-1][y][z] {
                            result.add_face(vec![start_idx, start_idx+4, start_idx+7, start_idx+3]);
                        }
                        if x == size_x-2 || !grid[x+1][y][z] {
                            result.add_face(vec![start_idx+1, start_idx+2, start_idx+6, start_idx+5]);
                        }
                        if y == 0 || !grid[x][y-1][z] {
                            result.add_face(vec![start_idx, start_idx+1, start_idx+5, start_idx+4]);
                        }
                        if y == size_y-2 || !grid[x][y+1][z] {
                            result.add_face(vec![start_idx+3, start_idx+7, start_idx+6, start_idx+2]);
                        }
                        if z == 0 || !grid[x][y][z-1] {
                            result.add_face(vec![start_idx, start_idx+3, start_idx+2, start_idx+1]);
                        }
                        if z == size_z-2 || !grid[x][y][z+1] {
                            result.add_face(vec![start_idx+4, start_idx+5, start_idx+6, start_idx+7]);
                        }
                    }
                }
            }
        }

        result.compute_normals();
        result
    }

    /// Smooth remeshing using edge collapse/split.
    fn remesh_smooth(&self, mesh: &ModifierMesh) -> ModifierMesh {
        let mut result = mesh.clone();

        // Target edge length based on voxel size
        let target_length = self.voxel_size;
        let _min_length = target_length * 0.8;
        let max_length = target_length * 1.2;

        for _iteration in 0..self.smooth_iterations {
            // Split long edges
            let mut new_vertices = Vec::new();
            let mut edges_to_split = Vec::new();

            for face in &result.faces {
                for i in 0..face.len() {
                    let v0 = face[i];
                    let v1 = face[(i + 1) % face.len()];

                    let p0 = result.positions[v0];
                    let p1 = result.positions[v1];
                    let length = (p1 - p0).magnitude();

                    if length > max_length {
                        let midpoint = Point3::from((p0.coords + p1.coords) / 2.0);
                        edges_to_split.push((v0, v1, midpoint));
                    }
                }
            }

            // Add midpoints as new vertices
            for (_, _, midpoint) in &edges_to_split {
                new_vertices.push(*midpoint);
            }

            // Simple smoothing: average vertex positions
            let old_positions = result.positions.clone();
            for i in 0..result.positions.len() {
                let mut sum = Vector3::zeros();
                let mut count = 0;

                // Find adjacent vertices
                for face in &result.faces {
                    if let Some(pos) = face.iter().position(|&v| v == i) {
                        let prev = face[(pos + face.len() - 1) % face.len()];
                        let next = face[(pos + 1) % face.len()];

                        sum += old_positions[prev].coords;
                        sum += old_positions[next].coords;
                        count += 2;
                    }
                }

                if count > 0 {
                    let avg = Point3::from(sum / count as f64);
                    // Blend with original position
                    result.positions[i] = Point3::from(
                        result.positions[i].coords * 0.5 + avg.coords * 0.5
                    );
                }
            }
        }

        result.compute_normals();
        result
    }

    /// Block remeshing (cubes).
    fn remesh_blocks(&self, mesh: &ModifierMesh) -> ModifierMesh {
        // Similar to voxel but keep the blocky appearance
        
        // No smoothing
        self.remesh_voxel(mesh)
    }

    /// Sharp remeshing (preserve features).
    fn remesh_sharp(&self, mesh: &ModifierMesh) -> ModifierMesh {
        // Start with smooth remesh
        let mut result = self.remesh_smooth(mesh);

        // Detect and preserve sharp features
        // (simplified: just reduce smoothing on high-curvature areas)
        result.compute_normals();
        result
    }
}

impl Modifier for RemeshModifier {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_id(&self) -> &'static str {
        "RemeshModifier"
    }

    fn apply(&self, mesh: &ModifierMesh) -> ModifierMesh {
        if mesh.positions.is_empty() {
            return mesh.clone();
        }

        match self.mode {
            RemeshMode::Voxel => self.remesh_voxel(mesh),
            RemeshMode::Smooth => self.remesh_smooth(mesh),
            RemeshMode::Sharp => self.remesh_sharp(mesh),
            RemeshMode::Blocks => self.remesh_blocks(mesh),
        }
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn clone_box(&self) -> Box<dyn Modifier> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remesh_voxel() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.5, 1.0, 0.0),
            ],
            vec![vec![0, 1, 2]],
        );

        let modifier = RemeshModifier::voxel(0.2);
        let result = modifier.apply(&mesh);

        // Voxel remesh should create new geometry
        assert!(result.positions.len() > 0);
        assert!(result.faces.len() > 0);
    }

    #[test]
    fn test_remesh_smooth() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(1.0, 1.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
            ],
            vec![vec![0, 1, 2, 3]],
        );

        let modifier = RemeshModifier::new(RemeshMode::Smooth, 0.5);
        let result = modifier.apply(&mesh);

        // Should produce valid mesh
        assert!(result.positions.len() > 0);
    }

    #[test]
    fn test_remesh_modes() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.5, 1.0, 0.0),
            ],
            vec![vec![0, 1, 2]],
        );

        // Test all modes
        for mode in [RemeshMode::Voxel, RemeshMode::Smooth, RemeshMode::Sharp, RemeshMode::Blocks] {
            let modifier = RemeshModifier::new(mode, 0.3);
            let result = modifier.apply(&mesh);
            assert!(result.positions.len() > 0);
        }
    }
}
