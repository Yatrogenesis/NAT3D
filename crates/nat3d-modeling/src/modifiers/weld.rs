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

//! Weld modifier.
//!
//! Merges vertices within a distance threshold.

use nalgebra::{Point3, Vector3};
use std::any::Any;
use std::collections::HashMap;
use super::stack::{Modifier, ModifierMesh};

/// Weld mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum WeldMode {
    /// Weld all vertices within threshold.
    #[default]
    All,
    /// Weld only connected vertices (share an edge).
    Connected,
}


/// Weld modifier.
#[derive(Debug, Clone)]
pub struct WeldModifier {
    /// Modifier name.
    pub name: String,
    /// Whether enabled.
    pub enabled: bool,
    /// Distance threshold for merging.
    pub threshold: f64,
    /// Weld mode.
    pub mode: WeldMode,
}

impl Default for WeldModifier {
    fn default() -> Self {
        Self {
            name: "Weld".to_string(),
            enabled: true,
            threshold: 0.001,
            mode: WeldMode::default(),
        }
    }
}

impl WeldModifier {
    /// Create new weld modifier.
    pub fn new(threshold: f64) -> Self {
        Self {
            threshold,
            ..Default::default()
        }
    }

    /// Create weld modifier with mode.
    pub fn with_mode(threshold: f64, mode: WeldMode) -> Self {
        Self {
            threshold,
            mode,
            ..Default::default()
        }
    }

    /// Build connectivity map (which vertices share edges).
    fn build_connectivity(&self, mesh: &ModifierMesh) -> HashMap<usize, Vec<usize>> {
        let mut connectivity: HashMap<usize, Vec<usize>> = HashMap::new();

        for face in &mesh.faces {
            for i in 0..face.len() {
                let v0 = face[i];
                let v1 = face[(i + 1) % face.len()];

                connectivity.entry(v0).or_default().push(v1);
                connectivity.entry(v1).or_default().push(v0);
            }
        }

        connectivity
    }

    /// Find vertices to merge using union-find.
    fn find_merge_groups(&self, mesh: &ModifierMesh) -> Vec<usize> {
        let vertex_count = mesh.positions.len();
        let mut parent: Vec<usize> = (0..vertex_count).collect();

        // Union-find helper functions
        fn find(parent: &mut Vec<usize>, i: usize) -> usize {
            if parent[i] != i {
                parent[i] = find(parent, parent[i]);
            }
            parent[i]
        }

        fn union(parent: &mut Vec<usize>, i: usize, j: usize) {
            let root_i = find(parent, i);
            let root_j = find(parent, j);
            if root_i != root_j {
                parent[root_j] = root_i;
            }
        }

        match self.mode {
            WeldMode::All => {
                // Merge all vertices within threshold
                for i in 0..vertex_count {
                    for j in (i + 1)..vertex_count {
                        let dist = (mesh.positions[i] - mesh.positions[j]).magnitude();
                        if dist < self.threshold {
                            union(&mut parent, i, j);
                        }
                    }
                }
            }
            WeldMode::Connected => {
                // Only merge vertices that are connected by edges
                let connectivity = self.build_connectivity(mesh);

                for (v0, neighbors) in &connectivity {
                    for &v1 in neighbors {
                        if v0 < &v1 {
                            let dist = (mesh.positions[*v0] - mesh.positions[v1]).magnitude();
                            if dist < self.threshold {
                                union(&mut parent, *v0, v1);
                            }
                        }
                    }
                }
            }
        }

        // Ensure all parents are resolved
        for i in 0..vertex_count {
            find(&mut parent, i);
        }

        parent
    }

    /// Compute average position for merged vertices.
    fn compute_merged_positions(&self, mesh: &ModifierMesh, groups: &[usize]) -> HashMap<usize, Point3<f64>> {
        let mut group_positions: HashMap<usize, Vec<Point3<f64>>> = HashMap::new();

        for (i, &group) in groups.iter().enumerate() {
            group_positions.entry(group)
                .or_default()
                .push(mesh.positions[i]);
        }

        let mut merged_positions = HashMap::new();
        for (group, positions) in group_positions {
            let mut sum = Vector3::zeros();
            for pos in &positions {
                sum += pos.coords;
            }
            let avg = Point3::from(sum / positions.len() as f64);
            merged_positions.insert(group, avg);
        }

        merged_positions
    }

    /// Compute average normal for merged vertices.
    fn compute_merged_normals(&self, mesh: &ModifierMesh, groups: &[usize]) -> HashMap<usize, Vector3<f64>> {
        let mut group_normals: HashMap<usize, Vec<Vector3<f64>>> = HashMap::new();

        for (i, &group) in groups.iter().enumerate() {
            if i < mesh.normals.len() {
                group_normals.entry(group)
                    .or_default()
                    .push(mesh.normals[i]);
            }
        }

        let mut merged_normals = HashMap::new();
        for (group, normals) in group_normals {
            let mut sum = Vector3::zeros();
            for normal in &normals {
                sum += normal;
            }
            let len = sum.magnitude();
            let avg = if len > 1e-10 {
                sum / len
            } else {
                Vector3::y()
            };
            merged_normals.insert(group, avg);
        }

        merged_normals
    }
}

impl Modifier for WeldModifier {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_id(&self) -> &'static str {
        "WeldModifier"
    }

    fn apply(&self, mesh: &ModifierMesh) -> ModifierMesh {
        if mesh.positions.is_empty() {
            return mesh.clone();
        }

        // Find merge groups
        let groups = self.find_merge_groups(mesh);

        // Compute merged positions and normals
        let merged_positions = self.compute_merged_positions(mesh, &groups);
        let merged_normals = self.compute_merged_normals(mesh, &groups);

        // Build new vertex mapping (old index -> new index)
        let mut unique_groups: Vec<usize> = groups.clone();
        unique_groups.sort_unstable();
        unique_groups.dedup();

        let mut group_to_new_index: HashMap<usize, usize> = HashMap::new();
        for (new_idx, &group) in unique_groups.iter().enumerate() {
            group_to_new_index.insert(group, new_idx);
        }

        let mut result = ModifierMesh::new();

        // Add merged vertices
        for &group in &unique_groups {
            result.positions.push(merged_positions[&group]);
            if let Some(normal) = merged_normals.get(&group) {
                result.normals.push(*normal);
            }
        }

        // Remap faces
        for face in &mesh.faces {
            let mut new_face = Vec::new();
            let mut prev_idx = None;

            for &old_idx in face {
                let group = groups[old_idx];
                let new_idx = group_to_new_index[&group];

                // Skip duplicate consecutive vertices (caused by merging)
                if prev_idx != Some(new_idx) {
                    new_face.push(new_idx);
                    prev_idx = Some(new_idx);
                }
            }

            // Check for wrap-around duplicate
            if new_face.len() > 1 && new_face[0] == new_face[new_face.len() - 1] {
                new_face.pop();
            }

            // Only add faces with 3+ vertices
            if new_face.len() >= 3 {
                result.faces.push(new_face);
            }
        }

        // Copy other attributes (simplified - doesn't merge UVs)
        result.uvs = mesh.uvs.clone();
        result.vertex_groups = mesh.vertex_groups.clone();
        result.attributes = mesh.attributes.clone();

        result
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
    fn test_weld_duplicate_vertices() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(0.0001, 0.0, 0.0), // Almost duplicate
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.5, 1.0, 0.0),
            ],
            vec![vec![0, 2, 3], vec![1, 2, 3]],
        );

        let modifier = WeldModifier::new(0.001);
        let result = modifier.apply(&mesh);

        // First two vertices should be merged
        assert_eq!(result.positions.len(), 3);
        assert_eq!(result.faces.len(), 2);
    }

    #[test]
    fn test_weld_no_merge() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.5, 1.0, 0.0),
            ],
            vec![vec![0, 1, 2]],
        );

        let modifier = WeldModifier::new(0.001);
        let result = modifier.apply(&mesh);

        // No vertices should be merged
        assert_eq!(result.positions.len(), mesh.positions.len());
    }

    #[test]
    fn test_weld_connected_mode() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(0.0001, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
            ],
            vec![vec![0, 1, 2]],
        );

        let modifier = WeldModifier::with_mode(0.001, WeldMode::Connected);
        let result = modifier.apply(&mesh);

        // Should merge connected vertices
        assert!(result.positions.len() < mesh.positions.len());
    }
}
