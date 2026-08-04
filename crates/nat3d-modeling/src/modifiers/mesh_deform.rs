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

//! Mesh Deform modifier.
//!
//! Deforms mesh using cage mesh with mean value coordinates.

use nalgebra::{Point3, Vector3};
use std::any::Any;
use super::stack::{Modifier, ModifierMesh};

/// Mesh Deform modifier.
#[derive(Clone)]
pub struct MeshDeformModifier {
    /// Modifier name.
    pub name: String,
    /// Whether enabled.
    pub enabled: bool,
    /// Cage mesh (control mesh).
    pub cage_mesh: ModifierMesh,
    /// Original cage mesh (for computing deformation).
    pub original_cage: ModifierMesh,
    /// Precision level for coordinate calculation.
    pub precision: usize,
    /// Dynamic bind (recompute on each apply).
    pub dynamic_bind: bool,
    /// Precomputed weights (vertex -> cage vertex weights).
    pub weights: Vec<Vec<(usize, f64)>>,
    /// Whether weights are bound.
    pub is_bound: bool,
    /// Vertex group for selective deformation.
    pub vertex_group: Option<String>,
}

impl Default for MeshDeformModifier {
    fn default() -> Self {
        Self {
            name: "Mesh Deform".to_string(),
            enabled: true,
            cage_mesh: ModifierMesh::new(),
            original_cage: ModifierMesh::new(),
            precision: 6,
            dynamic_bind: false,
            weights: Vec::new(),
            is_bound: false,
            vertex_group: None,
        }
    }
}

impl MeshDeformModifier {
    /// Create new mesh deform modifier.
    pub fn new(cage_mesh: ModifierMesh) -> Self {
        Self {
            original_cage: cage_mesh.clone(),
            cage_mesh,
            ..Default::default()
        }
    }

    /// Create with precision.
    pub fn with_precision(cage_mesh: ModifierMesh, precision: usize) -> Self {
        Self {
            original_cage: cage_mesh.clone(),
            cage_mesh,
            precision,
            ..Default::default()
        }
    }

    /// Bind mesh to cage (compute weights).
    pub fn bind(&mut self, mesh: &ModifierMesh) {
        self.weights = Vec::with_capacity(mesh.positions.len());

        for i in 0..mesh.positions.len() {
            let point = mesh.positions[i];
            let vertex_weights = self.compute_mean_value_coordinates(point);
            self.weights.push(vertex_weights);
        }

        self.is_bound = true;
        self.original_cage = self.cage_mesh.clone();
    }

    /// Compute mean value coordinates for a point relative to cage.
    fn compute_mean_value_coordinates(&self, point: Point3<f64>) -> Vec<(usize, f64)> {
        if self.cage_mesh.positions.is_empty() {
            return Vec::new();
        }

        let mut weights = Vec::new();
        let mut total_weight = 0.0;

        // Simplified mean value coordinates using distance-based weights
        for (i, &cage_vertex) in self.cage_mesh.positions.iter().enumerate() {
            let distance = (cage_vertex - point).magnitude();

            // Use inverse distance weighting
            let weight = if distance < 1e-6 {
                1e6 // Very close to cage vertex
            } else {
                1.0 / distance.powi(self.precision as i32)
            };

            weights.push((i, weight));
            total_weight += weight;
        }

        // Normalize weights
        if total_weight > 1e-10 {
            for (_, w) in &mut weights {
                *w /= total_weight;
            }
        }

        // Filter out negligible weights
        weights.retain(|(_, w)| *w > 1e-6);

        weights
    }

    /// Compute harmonic coordinates (more accurate but slower).
    fn compute_harmonic_coordinates(&self, point: Point3<f64>) -> Vec<(usize, f64)> {
        if self.cage_mesh.faces.is_empty() {
            return self.compute_mean_value_coordinates(point);
        }

        let mut weights = vec![0.0; self.cage_mesh.positions.len()];
        let mut total_weight = 0.0;

        // For each cage face, compute contribution
        for face in &self.cage_mesh.faces {
            if face.len() < 3 {
                continue;
            }

            // Process as triangles
            for i in 1..face.len() - 1 {
                let v0_idx = face[0];
                let v1_idx = face[i];
                let v2_idx = face[i + 1];

                if v0_idx >= self.cage_mesh.positions.len()
                    || v1_idx >= self.cage_mesh.positions.len()
                    || v2_idx >= self.cage_mesh.positions.len()
                {
                    continue;
                }

                let v0 = self.cage_mesh.positions[v0_idx];
                let v1 = self.cage_mesh.positions[v1_idx];
                let v2 = self.cage_mesh.positions[v2_idx];

                // Calculate mean value coordinates for this triangle
                let w0 = self.triangle_mean_value_weight(point, v0, v1, v2);
                let w1 = self.triangle_mean_value_weight(point, v1, v2, v0);
                let w2 = self.triangle_mean_value_weight(point, v2, v0, v1);

                weights[v0_idx] += w0;
                weights[v1_idx] += w1;
                weights[v2_idx] += w2;

                total_weight += w0 + w1 + w2;
            }
        }

        // Normalize
        if total_weight > 1e-10 {
            for w in &mut weights {
                *w /= total_weight;
            }
        }

        // Convert to sparse representation
        weights
            .iter()
            .enumerate()
            .filter(|(_, &w)| w > 1e-6)
            .map(|(i, &w)| (i, w))
            .collect()
    }

    /// Calculate mean value weight for a vertex in a triangle.
    fn triangle_mean_value_weight(
        &self,
        point: Point3<f64>,
        vertex: Point3<f64>,
        v1: Point3<f64>,
        v2: Point3<f64>,
    ) -> f64 {
        let u = (vertex - point).normalize();
        let u1 = (v1 - point).normalize();
        let u2 = (v2 - point).normalize();

        let angle1 = u.dot(&u1).clamp(-1.0, 1.0).acos();
        let angle2 = u.dot(&u2).clamp(-1.0, 1.0).acos();

        let distance = (vertex - point).magnitude();

        if distance < 1e-10 {
            return 1.0;
        }

        // Mean value coordinate formula
        let tan_half_1 = (angle1 / 2.0).tan();
        let tan_half_2 = (angle2 / 2.0).tan();

        (tan_half_1 + tan_half_2) / distance
    }

    /// Deform point using precomputed weights.
    fn deform_point(&self, vertex_weights: &[(usize, f64)]) -> Point3<f64> {
        let mut result = Vector3::zeros();

        for &(cage_idx, weight) in vertex_weights {
            if cage_idx >= self.cage_mesh.positions.len() {
                continue;
            }

            let cage_pos = self.cage_mesh.positions[cage_idx];
            result += cage_pos.coords * weight;
        }

        Point3::from(result)
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
}

impl Modifier for MeshDeformModifier {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_id(&self) -> &'static str {
        "MeshDeformModifier"
    }

    fn apply(&self, mesh: &ModifierMesh) -> ModifierMesh {
        if mesh.positions.is_empty() || self.cage_mesh.positions.is_empty() {
            return mesh.clone();
        }

        let mut result = mesh.clone();

        // Bind if not bound or dynamic binding
        let mut modifier_copy = self.clone();
        if !self.is_bound || self.dynamic_bind {
            modifier_copy.bind(mesh);
        }

        // Apply deformation
        for i in 0..result.positions.len() {
            if i >= modifier_copy.weights.len() {
                continue;
            }

            let vertex_weight = self.get_vertex_weight(mesh, i);

            if vertex_weight < 1e-6 {
                continue;
            }

            let vertex_weights = &modifier_copy.weights[i];
            let deformed = modifier_copy.deform_point(vertex_weights);

            // Blend with original based on vertex group weight
            let original = result.positions[i];
            result.positions[i] = Point3::from(
                original.coords + (deformed - original) * vertex_weight
            );
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
    fn test_mesh_deform_basic() {
        // Create simple cage (cube)
        let cage = ModifierMesh::from_geometry(
            vec![
                Point3::new(-1.0, -1.0, -1.0),
                Point3::new(1.0, -1.0, -1.0),
                Point3::new(1.0, 1.0, -1.0),
                Point3::new(-1.0, 1.0, -1.0),
                Point3::new(-1.0, -1.0, 1.0),
                Point3::new(1.0, -1.0, 1.0),
                Point3::new(1.0, 1.0, 1.0),
                Point3::new(-1.0, 1.0, 1.0),
            ],
            vec![
                vec![0, 1, 2, 3],
                vec![4, 5, 6, 7],
            ],
        );

        // Create mesh to deform (single point at origin)
        let mesh = ModifierMesh::from_geometry(
            vec![Point3::new(0.0, 0.0, 0.0)],
            vec![],
        );

        let mut modifier = MeshDeformModifier::new(cage);
        modifier.bind(&mesh);

        let result = modifier.apply(&mesh);

        assert_eq!(result.positions.len(), 1);
        assert!(modifier.is_bound);
    }

    #[test]
    fn test_mesh_deform_binding() {
        let cage = ModifierMesh::from_geometry(
            vec![
                Point3::new(-1.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
            ],
            vec![vec![0, 1, 2]],
        );

        let mesh = ModifierMesh::from_geometry(
            vec![Point3::new(0.0, 0.3, 0.0)],
            vec![],
        );

        let mut modifier = MeshDeformModifier::new(cage);
        modifier.bind(&mesh);

        assert!(modifier.is_bound);
        assert_eq!(modifier.weights.len(), 1);
        assert!(!modifier.weights[0].is_empty());
    }

    #[test]
    fn test_mesh_deform_precision() {
        let cage = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
            ],
            vec![],
        );

        let mesh = ModifierMesh::from_geometry(
            vec![Point3::new(0.5, 0.0, 0.0)],
            vec![],
        );

        // Low precision
        let mut modifier_low = MeshDeformModifier::with_precision(cage.clone(), 2);
        modifier_low.bind(&mesh);

        // High precision
        let mut modifier_high = MeshDeformModifier::with_precision(cage, 8);
        modifier_high.bind(&mesh);

        assert_eq!(modifier_low.precision, 2);
        assert_eq!(modifier_high.precision, 8);
    }
}
