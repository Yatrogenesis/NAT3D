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

//! Corrective Smooth modifier (Delta Mush).
//!
//! Smooths mesh while preserving volume and detail using delta vectors.

use nalgebra::{Point3, Vector3};
use std::any::Any;
use super::stack::{Modifier, ModifierMesh};

/// Smoothing type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmoothType {
    /// Simple Laplacian smoothing.
    Simple,
    /// Length-weighted Laplacian.
    LengthWeighted,
}

/// Corrective Smooth modifier (Delta Mush).
#[derive(Clone)]
pub struct CorrectiveSmoothModifier {
    /// Modifier name.
    pub name: String,
    /// Whether enabled.
    pub enabled: bool,
    /// Number of smoothing iterations.
    pub iterations: usize,
    /// Smoothing factor (0 = no smooth, 1 = full smooth).
    pub factor: f64,
    /// Scale factor for detail preservation.
    pub scale: f64,
    /// Smooth type.
    pub smooth_type: SmoothType,
    /// Pin boundary vertices.
    pub pin_boundaries: bool,
    /// Use vertex group for selective smoothing.
    pub use_vertex_group: bool,
    /// Vertex group name.
    pub vertex_group: Option<String>,
    /// Repeat count for stabilization.
    pub repeat: usize,
}

impl Default for CorrectiveSmoothModifier {
    fn default() -> Self {
        Self {
            name: "Corrective Smooth".to_string(),
            enabled: true,
            iterations: 5,
            factor: 0.5,
            scale: 1.0,
            smooth_type: SmoothType::Simple,
            pin_boundaries: true,
            use_vertex_group: false,
            vertex_group: None,
            repeat: 1,
        }
    }
}

impl CorrectiveSmoothModifier {
    /// Create new corrective smooth modifier.
    pub fn new(iterations: usize, factor: f64) -> Self {
        Self {
            iterations,
            factor,
            ..Default::default()
        }
    }

    /// Create with smooth type.
    pub fn with_type(iterations: usize, factor: f64, smooth_type: SmoothType) -> Self {
        Self {
            iterations,
            factor,
            smooth_type,
            ..Default::default()
        }
    }

    /// Build adjacency graph.
    fn build_adjacency(&self, mesh: &ModifierMesh) -> Vec<Vec<usize>> {
        let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); mesh.positions.len()];

        for face in &mesh.faces {
            for i in 0..face.len() {
                let v1 = face[i];
                let v2 = face[(i + 1) % face.len()];

                if v1 < adjacency.len() && v2 < adjacency.len() {
                    if !adjacency[v1].contains(&v2) {
                        adjacency[v1].push(v2);
                    }
                    if !adjacency[v2].contains(&v1) {
                        adjacency[v2].push(v1);
                    }
                }
            }
        }

        adjacency
    }

    /// Check if vertex is on boundary.
    fn is_boundary_vertex(&self, vertex_idx: usize, adjacency: &[Vec<usize>]) -> bool {
        if vertex_idx >= adjacency.len() {
            return false;
        }

        // Simple heuristic: boundary vertices have fewer neighbors
        adjacency[vertex_idx].len() < 4
    }

    /// Calculate Laplacian smooth position.
    fn laplacian_smooth(
        &self,
        vertex_idx: usize,
        positions: &[Point3<f64>],
        adjacency: &[Vec<usize>],
    ) -> Point3<f64> {
        if vertex_idx >= adjacency.len() || adjacency[vertex_idx].is_empty() {
            return positions[vertex_idx];
        }

        let neighbors = &adjacency[vertex_idx];
        let current_pos = positions[vertex_idx];

        match self.smooth_type {
            SmoothType::Simple => {
                // Simple average of neighbors
                let sum: Vector3<f64> = neighbors.iter()
                    .filter(|&&n| n < positions.len())
                    .map(|&n| positions[n].coords)
                    .sum();

                let avg = sum / neighbors.len() as f64;
                Point3::from(avg)
            }
            SmoothType::LengthWeighted => {
                // Weight by edge length
                let mut weighted_sum = Vector3::zeros();
                let mut total_weight = 0.0;

                for &neighbor_idx in neighbors {
                    if neighbor_idx >= positions.len() {
                        continue;
                    }

                    let neighbor_pos = positions[neighbor_idx];
                    let edge_length = (neighbor_pos - current_pos).magnitude();

                    if edge_length > 1e-10 {
                        let weight = 1.0 / edge_length;
                        weighted_sum += neighbor_pos.coords * weight;
                        total_weight += weight;
                    }
                }

                if total_weight > 1e-10 {
                    Point3::from(weighted_sum / total_weight)
                } else {
                    current_pos
                }
            }
        }
    }

    /// Perform Laplacian smoothing.
    fn smooth_positions(
        &self,
        positions: &[Point3<f64>],
        adjacency: &[Vec<usize>],
        weights: &[f64],
    ) -> Vec<Point3<f64>> {
        let mut result = positions.to_vec();

        for _iter in 0..self.iterations {
            let mut new_positions = Vec::with_capacity(positions.len());

            for i in 0..result.len() {
                let weight = weights[i];

                // Skip if pinned or no weight
                if weight < 1e-6 {
                    new_positions.push(result[i]);
                    continue;
                }

                // Skip boundaries if pinned
                if self.pin_boundaries && self.is_boundary_vertex(i, adjacency) {
                    new_positions.push(result[i]);
                    continue;
                }

                let smoothed = self.laplacian_smooth(i, &result, adjacency);
                let original = result[i];

                // Blend based on factor and weight
                let blended = Point3::from(
                    original.coords + (smoothed - original) * self.factor * weight
                );

                new_positions.push(blended);
            }

            result = new_positions;
        }

        result
    }

    /// Calculate local coordinate frames.
    fn calculate_local_frames(
        &self,
        positions: &[Point3<f64>],
        adjacency: &[Vec<usize>],
    ) -> Vec<(Vector3<f64>, Vector3<f64>, Vector3<f64>)> {
        let mut frames = Vec::with_capacity(positions.len());

        for i in 0..positions.len() {
            if adjacency[i].is_empty() {
                frames.push((Vector3::x(), Vector3::y(), Vector3::z()));
                continue;
            }

            let pos = positions[i];
            let neighbors = &adjacency[i];

            // Calculate average neighbor direction as tangent
            let mut tangent = Vector3::zeros();
            for &n in neighbors {
                if n < positions.len() {
                    tangent += (positions[n] - pos).normalize();
                }
            }

            tangent = if tangent.magnitude() > 1e-10 {
                tangent.normalize()
            } else {
                Vector3::x()
            };

            // Choose perpendicular vectors
            let normal = if tangent.y.abs() < 0.9 {
                Vector3::y().cross(&tangent).normalize()
            } else {
                Vector3::x().cross(&tangent).normalize()
            };

            let binormal = tangent.cross(&normal).normalize();

            frames.push((tangent, normal, binormal));
        }

        frames
    }

    /// Calculate delta vectors in local space.
    fn calculate_deltas(
        &self,
        original: &[Point3<f64>],
        smoothed: &[Point3<f64>],
        frames: &[(Vector3<f64>, Vector3<f64>, Vector3<f64>)],
    ) -> Vec<Vector3<f64>> {
        let mut deltas = Vec::with_capacity(original.len());

        for i in 0..original.len() {
            let delta_world = original[i] - smoothed[i];
            let (tangent, normal, binormal) = frames[i];

            // Transform delta to local space
            let delta_local = Vector3::new(
                delta_world.dot(&tangent),
                delta_world.dot(&normal),
                delta_world.dot(&binormal),
            );

            deltas.push(delta_local * self.scale);
        }

        deltas
    }

    /// Apply deltas to smoothed positions.
    fn apply_deltas(
        &self,
        smoothed: &[Point3<f64>],
        deltas: &[Vector3<f64>],
        frames: &[(Vector3<f64>, Vector3<f64>, Vector3<f64>)],
    ) -> Vec<Point3<f64>> {
        let mut result = Vec::with_capacity(smoothed.len());

        for i in 0..smoothed.len() {
            let (tangent, normal, binormal) = frames[i];
            let delta_local = deltas[i];

            // Transform delta back to world space
            let delta_world = tangent * delta_local.x
                + normal * delta_local.y
                + binormal * delta_local.z;

            result.push(smoothed[i] + delta_world);
        }

        result
    }

    /// Get vertex weight from vertex group.
    fn get_vertex_weight(&self, mesh: &ModifierMesh, vertex_idx: usize) -> f64 {
        if !self.use_vertex_group {
            return 1.0;
        }

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
}

impl Modifier for CorrectiveSmoothModifier {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_id(&self) -> &'static str {
        "CorrectiveSmoothModifier"
    }

    fn apply(&self, mesh: &ModifierMesh) -> ModifierMesh {
        if mesh.positions.is_empty() || mesh.faces.is_empty() {
            return mesh.clone();
        }

        let mut result = mesh.clone();
        let adjacency = self.build_adjacency(mesh);

        // Get vertex weights
        let weights: Vec<f64> = (0..mesh.positions.len())
            .map(|i| self.get_vertex_weight(mesh, i))
            .collect();

        for _repeat in 0..self.repeat {
            // Calculate local frames before smoothing
            let original_frames = self.calculate_local_frames(&result.positions, &adjacency);

            // Calculate deltas in local space
            let deltas = self.calculate_deltas(
                &mesh.positions,
                &result.positions,
                &original_frames,
            );

            // Apply smoothing
            let smoothed = self.smooth_positions(&result.positions, &adjacency, &weights);

            // Calculate new frames after smoothing
            let smoothed_frames = self.calculate_local_frames(&smoothed, &adjacency);

            // Apply deltas to preserve detail
            result.positions = self.apply_deltas(&smoothed, &deltas, &smoothed_frames);
        }

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
    fn test_corrective_smooth_basic() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(1.0, 1.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
            ],
            vec![vec![0, 1, 2, 3]],
        );

        let modifier = CorrectiveSmoothModifier::new(3, 0.5);
        let result = modifier.apply(&mesh);

        assert_eq!(result.positions.len(), 4);
    }

    #[test]
    fn test_adjacency_building() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.5, 1.0, 0.0),
            ],
            vec![vec![0, 1, 2]],
        );

        let modifier = CorrectiveSmoothModifier::default();
        let adjacency = modifier.build_adjacency(&mesh);

        assert_eq!(adjacency.len(), 3);
        assert!(adjacency[0].contains(&1));
        assert!(adjacency[0].contains(&2));
    }

    #[test]
    fn test_smooth_types() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(-1.0, 0.0, 0.0),
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
            ],
            vec![vec![0, 1, 2]],
        );

        // Simple smoothing
        let modifier_simple = CorrectiveSmoothModifier::with_type(
            1,
            1.0,
            SmoothType::Simple,
        );
        let result_simple = modifier_simple.apply(&mesh);
        assert_eq!(result_simple.positions.len(), 3);

        // Length-weighted smoothing
        let modifier_weighted = CorrectiveSmoothModifier::with_type(
            1,
            1.0,
            SmoothType::LengthWeighted,
        );
        let result_weighted = modifier_weighted.apply(&mesh);
        assert_eq!(result_weighted.positions.len(), 3);
    }
}
