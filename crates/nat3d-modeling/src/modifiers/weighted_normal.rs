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

//! Weighted Normal modifier.
//!
//! Recalculates vertex normals using face area and angle weighting.

use nalgebra::{Point3, Vector3};
use std::any::Any;
use std::f64::consts::PI;
use super::stack::{Modifier, ModifierMesh};

/// Normal weighting mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeightMode {
    /// Weight by face area.
    FaceArea,
    /// Weight by corner angle.
    CornerAngle,
    /// Weight by both face area and corner angle.
    FaceAreaAndAngle,
}

/// Weighted Normal modifier.
#[derive(Clone)]
pub struct WeightedNormalModifier {
    /// Modifier name.
    pub name: String,
    /// Whether enabled.
    pub enabled: bool,
    /// Weighting mode.
    pub mode: WeightMode,
    /// Weight multiplier.
    pub weight: f64,
    /// Angle threshold for sharp edges (radians).
    pub threshold: f64,
    /// Keep sharp edges.
    pub keep_sharp: bool,
    /// Use face influence.
    pub face_influence: bool,
    /// Vertex group for selective application.
    pub vertex_group: Option<String>,
}

impl Default for WeightedNormalModifier {
    fn default() -> Self {
        Self {
            name: "Weighted Normal".to_string(),
            enabled: true,
            mode: WeightMode::FaceAreaAndAngle,
            weight: 50.0,
            threshold: PI / 3.0, // 60 degrees
            keep_sharp: true,
            face_influence: false,
            vertex_group: None,
        }
    }
}

impl WeightedNormalModifier {
    /// Create new weighted normal modifier.
    pub fn new(mode: WeightMode) -> Self {
        Self {
            mode,
            ..Default::default()
        }
    }

    /// Create with weight parameter.
    pub fn with_weight(mode: WeightMode, weight: f64) -> Self {
        Self {
            mode,
            weight,
            ..Default::default()
        }
    }

    /// Calculate face area.
    fn face_area(&self, v0: Point3<f64>, v1: Point3<f64>, v2: Point3<f64>) -> f64 {
        let edge1 = v1 - v0;
        let edge2 = v2 - v0;
        edge1.cross(&edge2).magnitude() * 0.5
    }

    /// Calculate corner angle at vertex in triangle.
    fn corner_angle(&self, vertex: Point3<f64>, v1: Point3<f64>, v2: Point3<f64>) -> f64 {
        let edge1 = (v1 - vertex).normalize();
        let edge2 = (v2 - vertex).normalize();

        let dot = edge1.dot(&edge2).clamp(-1.0, 1.0);
        dot.acos()
    }

    /// Calculate weight for a face contribution to a vertex normal.
    fn calculate_weight(
        &self,
        vertex_idx: usize,
        face: &[usize],
        mesh: &ModifierMesh,
    ) -> f64 {
        if face.len() < 3 {
            return 0.0;
        }

        // Find vertex position in face
        let vertex_pos_in_face = face.iter().position(|&idx| idx == vertex_idx);
        if vertex_pos_in_face.is_none() {
            return 0.0;
        }

        let pos_idx = vertex_pos_in_face.unwrap();

        // For triangles, calculate directly
        if face.len() == 3 {
            let v0 = mesh.positions[face[0]];
            let v1 = mesh.positions[face[1]];
            let v2 = mesh.positions[face[2]];

            let vertex = mesh.positions[vertex_idx];

            let area_weight = if matches!(
                self.mode,
                WeightMode::FaceArea | WeightMode::FaceAreaAndAngle
            ) {
                self.face_area(v0, v1, v2)
            } else {
                1.0
            };

            let angle_weight = if matches!(
                self.mode,
                WeightMode::CornerAngle | WeightMode::FaceAreaAndAngle
            ) {
                let prev_idx = if pos_idx == 0 { 2 } else { pos_idx - 1 };
                let next_idx = if pos_idx == 2 { 0 } else { pos_idx + 1 };
                let v_prev = mesh.positions[face[prev_idx]];
                let v_next = mesh.positions[face[next_idx]];
                self.corner_angle(vertex, v_prev, v_next)
            } else {
                1.0
            };

            return match self.mode {
                WeightMode::FaceArea => area_weight,
                WeightMode::CornerAngle => angle_weight,
                WeightMode::FaceAreaAndAngle => area_weight * angle_weight,
            };
        }

        // For polygons, approximate using fan triangulation
        let _v_center = mesh.positions[face[0]];
        let v_prev_idx = if pos_idx == 0 {
            face.len() - 1
        } else {
            pos_idx - 1
        };
        let v_next_idx = if pos_idx == face.len() - 1 {
            0
        } else {
            pos_idx + 1
        };

        let v_prev = mesh.positions[face[v_prev_idx]];
        let v_next = mesh.positions[face[v_next_idx]];
        let vertex = mesh.positions[vertex_idx];

        let area_weight = if matches!(
            self.mode,
            WeightMode::FaceArea | WeightMode::FaceAreaAndAngle
        ) {
            self.face_area(vertex, v_prev, v_next)
        } else {
            1.0
        };

        let angle_weight = if matches!(
            self.mode,
            WeightMode::CornerAngle | WeightMode::FaceAreaAndAngle
        ) {
            self.corner_angle(vertex, v_prev, v_next)
        } else {
            1.0
        };

        match self.mode {
            WeightMode::FaceArea => area_weight,
            WeightMode::CornerAngle => angle_weight,
            WeightMode::FaceAreaAndAngle => area_weight * angle_weight,
        }
    }

    /// Check if edge is sharp (angle exceeds threshold).
    fn is_sharp_edge(
        &self,
        face1_normal: Vector3<f64>,
        face2_normal: Vector3<f64>,
    ) -> bool {
        if !self.keep_sharp {
            return false;
        }

        let dot = face1_normal.dot(&face2_normal).clamp(-1.0, 1.0);
        let angle = dot.acos();

        angle > self.threshold
    }

    /// Compute weighted normals.
    fn compute_weighted_normals(&self, mesh: &ModifierMesh) -> Vec<Vector3<f64>> {
        let mut normals = vec![Vector3::zeros(); mesh.positions.len()];
        let mut face_normals = Vec::new();

        // Calculate face normals
        for face in &mesh.faces {
            if face.len() < 3 {
                face_normals.push(Vector3::y());
                continue;
            }

            let v0 = mesh.positions[face[0]];
            let v1 = mesh.positions[face[1]];
            let v2 = mesh.positions[face[2]];

            let normal = (v1 - v0).cross(&(v2 - v0));
            let len = normal.magnitude();

            if len > 1e-10 {
                face_normals.push(normal / len);
            } else {
                face_normals.push(Vector3::y());
            }
        }

        // Accumulate weighted normals
        for (face_idx, face) in mesh.faces.iter().enumerate() {
            let face_normal = face_normals[face_idx];

            for &vertex_idx in face {
                if vertex_idx >= normals.len() {
                    continue;
                }

                let weight = self.calculate_weight(vertex_idx, face, mesh);
                let weighted_normal = face_normal * weight;

                normals[vertex_idx] += weighted_normal;
            }
        }

        // Normalize
        for normal in &mut normals {
            let len = normal.magnitude();
            if len > 1e-10 {
                *normal /= len;
            } else {
                *normal = Vector3::y();
            }
        }

        normals
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

impl Modifier for WeightedNormalModifier {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_id(&self) -> &'static str {
        "WeightedNormalModifier"
    }

    fn apply(&self, mesh: &ModifierMesh) -> ModifierMesh {
        if mesh.positions.is_empty() || mesh.faces.is_empty() {
            return mesh.clone();
        }

        let mut result = mesh.clone();
        let weighted_normals = self.compute_weighted_normals(mesh);

        // Apply weighted normals with weight parameter
        let weight_factor = self.weight / 100.0;

        for i in 0..result.normals.len() {
            let vertex_weight = self.get_vertex_weight(mesh, i);

            if vertex_weight < 1e-6 {
                continue;
            }

            let original_normal = mesh.normals.get(i).copied().unwrap_or(Vector3::y());
            let weighted_normal = weighted_normals[i];

            // Blend between original and weighted normal
            let blended = original_normal + (weighted_normal - original_normal) * weight_factor;

            let len = blended.magnitude();
            if len > 1e-10 {
                result.normals[i] = blended / len;
            } else {
                result.normals[i] = weighted_normal;
            }

            // Apply vertex group weight
            if (vertex_weight - 1.0).abs() > 1e-6 {
                let final_normal = original_normal + (result.normals[i] - original_normal) * vertex_weight;
                let len = final_normal.magnitude();
                if len > 1e-10 {
                    result.normals[i] = final_normal / len;
                }
            }
        }

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
    fn test_weighted_normal_face_area() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.5, 0.0, 1.0),
            ],
            vec![vec![0, 1, 2]],
        );

        let modifier = WeightedNormalModifier::new(WeightMode::FaceArea);
        let result = modifier.apply(&mesh);

        assert_eq!(result.normals.len(), 3);
        // Normals should be computed - check they are normalized
        for normal in &result.normals {
            let len = normal.magnitude();
            assert!((len - 1.0).abs() < 0.1); // Should be unit length
        }
        // At least one normal should have significant Y component (upward)
        assert!(result.normals.iter().any(|n| n.y.abs() > 0.5));
    }

    #[test]
    fn test_weighted_normal_corner_angle() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(1.0, 1.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
            ],
            vec![vec![0, 1, 2, 3]],
        );

        let modifier = WeightedNormalModifier::new(WeightMode::CornerAngle);
        let result = modifier.apply(&mesh);

        assert_eq!(result.normals.len(), 4);
        // Check normals are computed
        for normal in &result.normals {
            assert!(normal.magnitude() > 0.9);
        }
    }

    #[test]
    fn test_weighted_normal_combined() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(-1.0, 0.0, -1.0),
                Point3::new(1.0, 0.0, -1.0),
                Point3::new(1.0, 0.0, 1.0),
                Point3::new(-1.0, 0.0, 1.0),
            ],
            vec![vec![0, 1, 2, 3]],
        );

        let modifier = WeightedNormalModifier::new(WeightMode::FaceAreaAndAngle);
        let result = modifier.apply(&mesh);

        assert_eq!(result.normals.len(), 4);
    }
}
