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

//! Multi-resolution sculpting.
//!
//! Implements multiresolution mesh representation for sculpting with
//! displacement storage at multiple subdivision levels.

use nalgebra::{Point3, Vector3};
use std::collections::HashMap;

/// Multiresolution mesh for sculpting.
pub struct MultiresMesh {
    /// Base mesh vertices.
    base_positions: Vec<Point3<f64>>,
    /// Base mesh triangles.
    base_triangles: Vec<[usize; 3]>,
    /// Subdivision levels (level 0 = base).
    levels: Vec<SubdivisionLevel>,
    /// Current active level.
    active_level: usize,
    /// Maximum allowed levels.
    max_levels: usize,
}

/// A single subdivision level.
#[derive(Clone)]
pub struct SubdivisionLevel {
    /// Vertex positions at this level.
    pub positions: Vec<Point3<f64>>,
    /// Vertex normals at this level.
    pub normals: Vec<Vector3<f64>>,
    /// Triangle indices at this level.
    pub triangles: Vec<[usize; 3]>,
    /// Displacement vectors from smooth subdivision.
    pub displacements: Vec<Vector3<f64>>,
    /// Parent vertex indices for interpolation.
    parent_indices: Vec<ParentData>,
}

/// Parent vertex data for subdivision.
#[derive(Clone)]
struct ParentData {
    /// Type of vertex.
    vertex_type: VertexType,
    /// Parent indices with weights.
    parents: Vec<(usize, f64)>,
}

/// Type of subdivision vertex.
#[derive(Clone, Copy, PartialEq, Eq)]
enum VertexType {
    /// Original vertex from parent level.
    Original(usize),
    /// Edge midpoint vertex.
    Edge(usize, usize),
    /// Face centroid vertex.
    Face(usize),
}

impl MultiresMesh {
    /// Create a new multiresolution mesh from base geometry.
    pub fn new(positions: Vec<Point3<f64>>, triangles: Vec<[usize; 3]>) -> Self {
        let normals = Self::compute_normals(&positions, &triangles);
        let base_level = SubdivisionLevel {
            positions: positions.clone(),
            normals,
            triangles: triangles.clone(),
            displacements: vec![Vector3::zeros(); positions.len()],
            parent_indices: positions
                .iter()
                .enumerate()
                .map(|(i, _)| ParentData {
                    vertex_type: VertexType::Original(i),
                    parents: vec![(i, 1.0)],
                })
                .collect(),
        };

        Self {
            base_positions: positions,
            base_triangles: triangles,
            levels: vec![base_level],
            active_level: 0,
            max_levels: 8,
        }
    }

    /// Compute vertex normals.
    fn compute_normals(positions: &[Point3<f64>], triangles: &[[usize; 3]]) -> Vec<Vector3<f64>> {
        let mut normals = vec![Vector3::zeros(); positions.len()];

        for tri in triangles {
            let v0 = positions[tri[0]];
            let v1 = positions[tri[1]];
            let v2 = positions[tri[2]];

            let edge1 = v1 - v0;
            let edge2 = v2 - v0;
            let face_normal = edge1.cross(&edge2);

            for &v in tri {
                normals[v] += face_normal;
            }
        }

        for n in &mut normals {
            let len = n.norm();
            if len > 1e-10 {
                *n /= len;
            }
        }

        normals
    }

    /// Get the current subdivision level.
    pub fn current_level(&self) -> usize {
        self.active_level
    }

    /// Get the maximum subdivision level available.
    pub fn max_level(&self) -> usize {
        self.levels.len() - 1
    }

    /// Get level data.
    pub fn level(&self, level: usize) -> Option<&SubdivisionLevel> {
        self.levels.get(level)
    }

    /// Get current level data.
    pub fn current(&self) -> &SubdivisionLevel {
        &self.levels[self.active_level]
    }

    /// Get mutable current level data.
    pub fn current_mut(&mut self) -> &mut SubdivisionLevel {
        &mut self.levels[self.active_level]
    }

    /// Subdivide to create a new level.
    pub fn subdivide(&mut self) -> bool {
        if self.levels.len() >= self.max_levels {
            return false;
        }

        let parent = &self.levels[self.levels.len() - 1];
        let new_level = self.subdivide_level(parent);
        self.levels.push(new_level);
        self.active_level = self.levels.len() - 1;

        true
    }

    /// Create a subdivided level from parent.
    fn subdivide_level(&self, parent: &SubdivisionLevel) -> SubdivisionLevel {
        let mut new_positions = Vec::new();
        let mut new_triangles = Vec::new();
        let mut parent_indices = Vec::new();
        let mut edge_vertex_map: HashMap<(usize, usize), usize> = HashMap::new();

        // Copy and adjust original vertices (Catmull-Clark style adjustment)
        for (i, &pos) in parent.positions.iter().enumerate() {
            // For now, simple copy - could implement vertex adjustment
            new_positions.push(pos);
            parent_indices.push(ParentData {
                vertex_type: VertexType::Original(i),
                parents: vec![(i, 1.0)],
            });
        }

        // Process each triangle
        for (face_idx, tri) in parent.triangles.iter().enumerate() {
            // Create edge midpoints
            let mut edge_verts = [0usize; 3];
            for i in 0..3 {
                let v0 = tri[i];
                let v1 = tri[(i + 1) % 3];
                let edge_key = if v0 < v1 { (v0, v1) } else { (v1, v0) };

                edge_verts[i] = *edge_vertex_map.entry(edge_key).or_insert_with(|| {
                    let idx = new_positions.len();
                    let p0 = parent.positions[v0];
                    let p1 = parent.positions[v1];
                    let mid = Point3::new(
                        (p0.x + p1.x) / 2.0,
                        (p0.y + p1.y) / 2.0,
                        (p0.z + p1.z) / 2.0,
                    );
                    new_positions.push(mid);
                    parent_indices.push(ParentData {
                        vertex_type: VertexType::Edge(v0, v1),
                        parents: vec![(v0, 0.5), (v1, 0.5)],
                    });
                    idx
                });
            }

            // Create face centroid
            let face_center_idx = new_positions.len();
            let p0 = parent.positions[tri[0]];
            let p1 = parent.positions[tri[1]];
            let p2 = parent.positions[tri[2]];
            let center = Point3::new(
                (p0.x + p1.x + p2.x) / 3.0,
                (p0.y + p1.y + p2.y) / 3.0,
                (p0.z + p1.z + p2.z) / 3.0,
            );
            new_positions.push(center);
            parent_indices.push(ParentData {
                vertex_type: VertexType::Face(face_idx),
                parents: vec![
                    (tri[0], 1.0 / 3.0),
                    (tri[1], 1.0 / 3.0),
                    (tri[2], 1.0 / 3.0),
                ],
            });

            // Create 3 new triangles (triangle subdivision)
            // Original vertex -> two adjacent edge midpoints -> face center
            for i in 0..3 {
                let corner = tri[i];
                let edge_a = edge_verts[(i + 2) % 3]; // Previous edge
                let edge_b = edge_verts[i]; // Current edge

                new_triangles.push([corner, edge_b, face_center_idx]);
                new_triangles.push([corner, face_center_idx, edge_a]);
            }

            // Create center triangle from edge midpoints
            new_triangles.push([edge_verts[0], edge_verts[1], face_center_idx]);
            new_triangles.push([edge_verts[1], edge_verts[2], face_center_idx]);
            new_triangles.push([edge_verts[2], edge_verts[0], face_center_idx]);
        }

        let normals = Self::compute_normals(&new_positions, &new_triangles);
        let displacements = vec![Vector3::zeros(); new_positions.len()];

        SubdivisionLevel {
            positions: new_positions,
            normals,
            triangles: new_triangles,
            displacements,
            parent_indices,
        }
    }

    /// Set the active level.
    pub fn set_level(&mut self, level: usize) {
        if level < self.levels.len() {
            self.active_level = level;
        }
    }

    /// Apply displacement at current level.
    pub fn apply_displacement(&mut self, vertex: usize, displacement: Vector3<f64>) {
        if vertex < self.levels[self.active_level].displacements.len() {
            // Get smooth position first (requires immutable borrow)
            let smooth_pos = self.get_smooth_position(vertex);
            // Now apply displacement (mutable borrow)
            let level = &mut self.levels[self.active_level];
            level.displacements[vertex] += displacement;
            level.positions[vertex] = smooth_pos + level.displacements[vertex];
        }
    }

    /// Get smooth subdivision position (without displacement).
    fn get_smooth_position(&self, vertex: usize) -> Point3<f64> {
        if self.active_level == 0 {
            return self.base_positions[vertex];
        }

        let level = &self.levels[self.active_level];
        let parent_data = &level.parent_indices[vertex];

        match parent_data.vertex_type {
            VertexType::Original(idx) => {
                if self.active_level == 0 {
                    self.base_positions[idx]
                } else {
                    self.levels[self.active_level - 1].positions[idx]
                }
            }
            VertexType::Edge(v0, v1) => {
                let parent = &self.levels[self.active_level - 1];
                let p0 = parent.positions[v0];
                let p1 = parent.positions[v1];
                Point3::new(
                    (p0.x + p1.x) / 2.0,
                    (p0.y + p1.y) / 2.0,
                    (p0.z + p1.z) / 2.0,
                )
            }
            VertexType::Face(face_idx) => {
                let parent = &self.levels[self.active_level - 1];
                let tri = parent.triangles[face_idx];
                let p0 = parent.positions[tri[0]];
                let p1 = parent.positions[tri[1]];
                let p2 = parent.positions[tri[2]];
                Point3::new(
                    (p0.x + p1.x + p2.x) / 3.0,
                    (p0.y + p1.y + p2.y) / 3.0,
                    (p0.z + p1.z + p2.z) / 3.0,
                )
            }
        }
    }

    /// Propagate changes from current level to higher levels.
    pub fn propagate_up(&mut self) {
        for level_idx in (self.active_level + 1)..self.levels.len() {
            self.update_level_from_parent(level_idx);
        }
    }

    /// Update a level based on its parent.
    fn update_level_from_parent(&mut self, level_idx: usize) {
        if level_idx == 0 || level_idx >= self.levels.len() {
            return;
        }

        let (before, after) = self.levels.split_at_mut(level_idx);
        let parent = &before[level_idx - 1];
        let level = &mut after[0];

        for (i, parent_data) in level.parent_indices.iter().enumerate() {
            let smooth_pos = match parent_data.vertex_type {
                VertexType::Original(idx) => parent.positions[idx],
                VertexType::Edge(v0, v1) => {
                    let p0 = parent.positions[v0];
                    let p1 = parent.positions[v1];
                    Point3::new(
                        (p0.x + p1.x) / 2.0,
                        (p0.y + p1.y) / 2.0,
                        (p0.z + p1.z) / 2.0,
                    )
                }
                VertexType::Face(face_idx) => {
                    let tri = parent.triangles[face_idx];
                    let p0 = parent.positions[tri[0]];
                    let p1 = parent.positions[tri[1]];
                    let p2 = parent.positions[tri[2]];
                    Point3::new(
                        (p0.x + p1.x + p2.x) / 3.0,
                        (p0.y + p1.y + p2.y) / 3.0,
                        (p0.z + p1.z + p2.z) / 3.0,
                    )
                }
            };

            level.positions[i] = smooth_pos + level.displacements[i];
        }

        level.normals = Self::compute_normals(&level.positions, &level.triangles);
    }

    /// Propagate changes from current level down to lower levels.
    pub fn propagate_down(&mut self) {
        if self.active_level == 0 {
            return;
        }

        // Simplified: project displacements to parent level
        for level_idx in (0..self.active_level).rev() {
            self.update_parent_from_level(level_idx);
        }
    }

    /// Update parent level from child.
    fn update_parent_from_level(&mut self, parent_idx: usize) {
        let child_idx = parent_idx + 1;
        if child_idx >= self.levels.len() {
            return;
        }

        // Gather displacement contributions from child vertices
        let mut displacement_accum =
            vec![(Vector3::zeros(), 0.0); self.levels[parent_idx].positions.len()];

        let child = &self.levels[child_idx];
        for (i, parent_data) in child.parent_indices.iter().enumerate() {
            for &(parent_v, weight) in &parent_data.parents {
                if parent_v < displacement_accum.len() {
                    displacement_accum[parent_v].0 += child.displacements[i] * weight;
                    displacement_accum[parent_v].1 += weight;
                }
            }
        }

        // Apply accumulated displacements
        let parent = &mut self.levels[parent_idx];
        for (i, (disp_sum, weight_sum)) in displacement_accum.iter().enumerate() {
            if *weight_sum > 0.0 {
                parent.displacements[i] += *disp_sum / *weight_sum * 0.5;
            }
        }

        // Update positions
        if parent_idx == 0 {
            for (i, pos) in parent.positions.iter_mut().enumerate() {
                *pos = self.base_positions[i] + parent.displacements[i];
            }
        }

        parent.normals = Self::compute_normals(&parent.positions, &parent.triangles);
    }

    /// Delete higher subdivision levels.
    pub fn delete_higher_levels(&mut self) {
        self.levels.truncate(self.active_level + 1);
    }

    /// Apply base mesh modifications.
    pub fn apply_base(&mut self) {
        // Bake current level 0 positions as new base
        self.base_positions = self.levels[0].positions.clone();

        // Clear all displacements at level 0
        self.levels[0].displacements.fill(Vector3::zeros());

        // Recalculate higher levels
        for level_idx in 1..self.levels.len() {
            self.update_level_from_parent(level_idx);
        }
    }

    /// Get vertex count at current level.
    pub fn vertex_count(&self) -> usize {
        self.levels[self.active_level].positions.len()
    }

    /// Get triangle count at current level.
    pub fn triangle_count(&self) -> usize {
        self.levels[self.active_level].triangles.len()
    }

    /// Get total displacement memory usage estimate.
    pub fn displacement_memory(&self) -> usize {
        self.levels
            .iter()
            .map(|l| l.displacements.len() * std::mem::size_of::<Vector3<f64>>())
            .sum()
    }
}

impl SubdivisionLevel {
    /// Update normals for this level.
    pub fn update_normals(&mut self) {
        self.normals = MultiresMesh::compute_normals(&self.positions, &self.triangles);
    }

    /// Clear all displacements.
    pub fn clear_displacements(&mut self) {
        self.displacements.fill(Vector3::zeros());
    }

    /// Get position with displacement.
    pub fn displaced_position(&self, vertex: usize) -> Point3<f64> {
        self.positions[vertex]
    }

    /// Get raw displacement.
    pub fn displacement(&self, vertex: usize) -> Vector3<f64> {
        self.displacements[vertex]
    }

    /// Set displacement directly.
    pub fn set_displacement(&mut self, vertex: usize, displacement: Vector3<f64>) {
        if vertex < self.displacements.len() {
            self.displacements[vertex] = displacement;
        }
    }
}

/// Statistics about multiresolution mesh.
#[derive(Debug, Clone)]
pub struct MultiresStats {
    /// Number of subdivision levels.
    pub level_count: usize,
    /// Current active level.
    pub active_level: usize,
    /// Vertices per level.
    pub vertices_per_level: Vec<usize>,
    /// Triangles per level.
    pub triangles_per_level: Vec<usize>,
    /// Total memory for displacements.
    pub displacement_memory: usize,
}

impl MultiresMesh {
    /// Get statistics.
    pub fn stats(&self) -> MultiresStats {
        MultiresStats {
            level_count: self.levels.len(),
            active_level: self.active_level,
            vertices_per_level: self.levels.iter().map(|l| l.positions.len()).collect(),
            triangles_per_level: self.levels.iter().map(|l| l.triangles.len()).collect(),
            displacement_memory: self.displacement_memory(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multires_creation() {
        let positions = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.5, 1.0, 0.0),
        ];
        let triangles = vec![[0, 1, 2]];

        let mesh = MultiresMesh::new(positions, triangles);
        assert_eq!(mesh.current_level(), 0);
        assert_eq!(mesh.max_level(), 0);
        assert_eq!(mesh.vertex_count(), 3);
    }

    #[test]
    fn test_subdivide() {
        let positions = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.5, 1.0, 0.0),
        ];
        let triangles = vec![[0, 1, 2]];

        let mut mesh = MultiresMesh::new(positions, triangles);
        assert!(mesh.subdivide());

        assert_eq!(mesh.current_level(), 1);
        assert!(mesh.vertex_count() > 3);
        assert!(mesh.triangle_count() > 1);
    }

    #[test]
    fn test_displacement() {
        let positions = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.5, 1.0, 0.0),
        ];
        let triangles = vec![[0, 1, 2]];

        let mut mesh = MultiresMesh::new(positions, triangles);
        mesh.subdivide();

        let disp = Vector3::new(0.0, 0.0, 0.5);
        mesh.apply_displacement(0, disp);

        let pos = mesh.current().displaced_position(0);
        assert!((pos.z - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_level_switch() {
        let positions = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.5, 1.0, 0.0),
        ];
        let triangles = vec![[0, 1, 2]];

        let mut mesh = MultiresMesh::new(positions, triangles);
        mesh.subdivide();
        mesh.subdivide();

        assert_eq!(mesh.current_level(), 2);

        mesh.set_level(0);
        assert_eq!(mesh.current_level(), 0);
        assert_eq!(mesh.vertex_count(), 3);
    }

    #[test]
    fn test_stats() {
        let positions = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.5, 1.0, 0.0),
        ];
        let triangles = vec![[0, 1, 2]];

        let mut mesh = MultiresMesh::new(positions, triangles);
        mesh.subdivide();

        let stats = mesh.stats();
        assert_eq!(stats.level_count, 2);
        assert_eq!(stats.active_level, 1);
        assert_eq!(stats.vertices_per_level[0], 3);
        assert!(stats.vertices_per_level[1] > 3);
    }
}
