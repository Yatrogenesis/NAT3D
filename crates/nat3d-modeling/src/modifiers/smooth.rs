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

//! Smooth modifier.
//!
//! Applies Laplacian smoothing to relax mesh geometry.

use nalgebra::{Point3, Vector3};
use std::any::Any;
use std::collections::{HashMap, HashSet};
use super::stack::{Modifier, ModifierMesh};

/// Smooth modifier.
#[derive(Debug, Clone)]
pub struct SmoothModifier {
    /// Modifier name.
    pub name: String,
    /// Whether enabled.
    pub enabled: bool,
    /// Number of smoothing iterations.
    pub iterations: usize,
    /// Smoothing factor (0 to 1, where 0 = no smoothing, 1 = full relaxation).
    pub factor: f64,
    /// Smooth along X axis.
    pub smooth_x: bool,
    /// Smooth along Y axis.
    pub smooth_y: bool,
    /// Smooth along Z axis.
    pub smooth_z: bool,
    /// Preserve mesh volume.
    pub preserve_volume: bool,
    /// Pin boundary vertices (don't smooth).
    pub pin_boundary: bool,
    /// Use vertex groups for weighting.
    pub vertex_group: Option<String>,
}

impl Default for SmoothModifier {
    fn default() -> Self {
        Self {
            name: "Smooth".to_string(),
            enabled: true,
            iterations: 5,
            factor: 0.5,
            smooth_x: true,
            smooth_y: true,
            smooth_z: true,
            preserve_volume: false,
            pin_boundary: false,
            vertex_group: None,
        }
    }
}

impl SmoothModifier {
    /// Create new smooth modifier.
    pub fn new(iterations: usize, factor: f64) -> Self {
        Self {
            iterations,
            factor: factor.max(0.0).min(1.0),
            ..Default::default()
        }
    }

    /// Build adjacency map (vertex -> connected vertices).
    fn build_adjacency(&self, mesh: &ModifierMesh) -> HashMap<usize, Vec<usize>> {
        let mut adjacency: HashMap<usize, HashSet<usize>> = HashMap::new();

        for face in &mesh.faces {
            for i in 0..face.len() {
                let v1 = face[i];
                let v2 = face[(i + 1) % face.len()];

                adjacency.entry(v1).or_default().insert(v2);
                adjacency.entry(v2).or_default().insert(v1);
            }
        }

        // Convert HashSet to Vec for easier iteration
        adjacency.into_iter()
            .map(|(k, v)| (k, v.into_iter().collect()))
            .collect()
    }

    /// Find boundary vertices (vertices with boundary edges).
    fn find_boundary_vertices(&self, mesh: &ModifierMesh) -> HashSet<usize> {
        let mut edge_count: HashMap<(usize, usize), usize> = HashMap::new();

        for face in &mesh.faces {
            for i in 0..face.len() {
                let v1 = face[i];
                let v2 = face[(i + 1) % face.len()];
                let edge = if v1 < v2 { (v1, v2) } else { (v2, v1) };
                *edge_count.entry(edge).or_insert(0) += 1;
            }
        }

        let mut boundary = HashSet::new();
        for ((v1, v2), count) in edge_count {
            if count == 1 {
                boundary.insert(v1);
                boundary.insert(v2);
            }
        }

        boundary
    }

    /// Calculate centroid of mesh.
    fn calculate_centroid(&self, positions: &[Point3<f64>]) -> Point3<f64> {
        if positions.is_empty() {
            return Point3::origin();
        }

        let sum = positions.iter().fold(Vector3::zeros(), |acc, p| {
            acc + Vector3::new(p.x, p.y, p.z)
        });

        let avg = sum / positions.len() as f64;
        Point3::new(avg.x, avg.y, avg.z)
    }

    /// Calculate mesh volume (approximate).
    fn calculate_volume(&self, mesh: &ModifierMesh) -> f64 {
        let mut volume = 0.0;
        let origin = Point3::origin();

        for face in &mesh.faces {
            if face.len() < 3 {
                continue;
            }

            // Tetrahedron volume from origin
            for i in 1..face.len() - 1 {
                let v0 = mesh.positions[face[0]] - origin;
                let v1 = mesh.positions[face[i]] - origin;
                let v2 = mesh.positions[face[i + 1]] - origin;

                let vol = v0.dot(&v1.cross(&v2)) / 6.0;
                volume += vol;
            }
        }

        volume.abs()
    }

    /// Get vertex weight from vertex group.
    fn get_vertex_weight(&self, mesh: &ModifierMesh, vertex_idx: usize) -> f64 {
        if let Some(ref group_name) = self.vertex_group {
            if let Some(weights) = mesh.vertex_groups.get(group_name) {
                for &(vi, weight) in weights {
                    if vi == vertex_idx {
                        return weight;
                    }
                }
            }
        }
        1.0
    }

    /// Apply one iteration of Laplacian smoothing.
    fn smooth_iteration(&self,
                        positions: &mut [Point3<f64>],
                        adjacency: &HashMap<usize, Vec<usize>>,
                        boundary: &HashSet<usize>,
                        mesh: &ModifierMesh) {
        let original = positions.to_vec();

        for (vertex_idx, neighbors) in adjacency {
            if neighbors.is_empty() {
                continue;
            }

            // Skip boundary vertices if pinned
            if self.pin_boundary && boundary.contains(vertex_idx) {
                continue;
            }

            // Calculate average position of neighbors
            let neighbor_avg = neighbors.iter().fold(Vector3::zeros(), |acc, &ni| {
                acc + Vector3::new(original[ni].x, original[ni].y, original[ni].z)
            }) / neighbors.len() as f64;

            let avg_point = Point3::new(neighbor_avg.x, neighbor_avg.y, neighbor_avg.z);
            let current = original[*vertex_idx];

            // Apply smoothing with factor and axis constraints
            let weight = self.get_vertex_weight(mesh, *vertex_idx);
            let effective_factor = self.factor * weight;

            let mut smoothed = current;
            if self.smooth_x {
                smoothed.x += (avg_point.x - current.x) * effective_factor;
            }
            if self.smooth_y {
                smoothed.y += (avg_point.y - current.y) * effective_factor;
            }
            if self.smooth_z {
                smoothed.z += (avg_point.z - current.z) * effective_factor;
            }

            positions[*vertex_idx] = smoothed;
        }
    }
}

impl Modifier for SmoothModifier {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_id(&self) -> &'static str {
        "SmoothModifier"
    }

    fn apply(&self, mesh: &ModifierMesh) -> ModifierMesh {
        if mesh.positions.is_empty() || mesh.faces.is_empty() || self.iterations == 0 {
            return mesh.clone();
        }

        let mut result = mesh.clone();

        // Build adjacency information
        let adjacency = self.build_adjacency(mesh);
        let boundary = if self.pin_boundary {
            self.find_boundary_vertices(mesh)
        } else {
            HashSet::new()
        };

        // Store original volume if preserving
        let original_volume = if self.preserve_volume {
            self.calculate_volume(mesh)
        } else {
            0.0
        };

        // Apply iterations
        for _ in 0..self.iterations {
            self.smooth_iteration(&mut result.positions, &adjacency, &boundary, mesh);
        }

        // Preserve volume if requested
        if self.preserve_volume && original_volume > 1e-10 {
            let new_volume = self.calculate_volume(&result);
            if new_volume > 1e-10 {
                let scale = (original_volume / new_volume).cbrt();
                let centroid = self.calculate_centroid(&result.positions);

                for pos in &mut result.positions {
                    let rel = *pos - centroid;
                    *pos = centroid + rel * scale;
                }
            }
        }

        // Recompute normals
        result.compute_normals();
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
    fn test_smooth_basic() {
        // Create a simple mesh
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.5, 1.0, 0.5), // Peak
                Point3::new(0.0, 1.0, 0.0),
            ],
            vec![vec![0, 1, 2], vec![0, 2, 3]],
        );

        let modifier = SmoothModifier::new(5, 0.5);
        let result = modifier.apply(&mesh);

        assert_eq!(result.positions.len(), 4);

        // Peak should be smoothed down
        assert!(result.positions[2].z < mesh.positions[2].z);
    }

    #[test]
    fn test_smooth_iterations() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.5, 1.0, 1.0),
            ],
            vec![vec![0, 1, 2]],
        );

        // More iterations = more smoothing
        let modifier1 = SmoothModifier::new(1, 0.5);
        let result1 = modifier1.apply(&mesh);

        let modifier2 = SmoothModifier::new(10, 0.5);
        let result2 = modifier2.apply(&mesh);

        // Result2 should be more smoothed
        let dist1 = (result1.positions[2].z - mesh.positions[2].z).abs();
        let dist2 = (result2.positions[2].z - mesh.positions[2].z).abs();
        assert!(dist2 > dist1);
    }

    #[test]
    fn test_smooth_axis_constraints() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.5, 1.0, 1.0),
            ],
            vec![vec![0, 1, 2]],
        );

        // Smooth only Z axis
        let mut modifier = SmoothModifier::new(5, 0.5);
        modifier.smooth_x = false;
        modifier.smooth_y = false;
        modifier.smooth_z = true;

        let result = modifier.apply(&mesh);

        // X and Y should be unchanged for peak vertex
        assert!((result.positions[2].x - mesh.positions[2].x).abs() < 1e-6);
        assert!((result.positions[2].y - mesh.positions[2].y).abs() < 1e-6);
    }
}
