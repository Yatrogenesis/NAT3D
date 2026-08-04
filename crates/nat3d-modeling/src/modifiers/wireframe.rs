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

//! Wireframe modifier.
//!
//! Converts mesh edges to cylindrical tubes.

use nalgebra::{Point3, Vector3, UnitQuaternion};
use std::any::Any;
use std::collections::HashMap;
use super::stack::{Modifier, ModifierMesh};

/// Wireframe modifier.
#[derive(Debug, Clone)]
pub struct WireframeModifier {
    /// Modifier name.
    pub name: String,
    /// Whether enabled.
    pub enabled: bool,
    /// Wire thickness.
    pub thickness: f64,
    /// Offset from surface.
    pub offset: f64,
    /// Include boundary edges only.
    pub boundary: bool,
    /// Replace original mesh.
    pub replace_original: bool,
    /// Use even thickness (constant screen space).
    pub even_thickness: bool,
    /// Crease weight for edge sharpness.
    pub crease_weight: f64,
    /// Number of segments per wire.
    pub segments: usize,
}

impl Default for WireframeModifier {
    fn default() -> Self {
        Self {
            name: "Wireframe".to_string(),
            enabled: true,
            thickness: 0.02,
            offset: 0.0,
            boundary: false,
            replace_original: true,
            even_thickness: false,
            crease_weight: 0.0,
            segments: 8,
        }
    }
}

impl WireframeModifier {
    /// Create new wireframe modifier.
    pub fn new(thickness: f64) -> Self {
        Self {
            thickness,
            ..Default::default()
        }
    }

    /// Create wireframe with segments.
    pub fn with_segments(thickness: f64, segments: usize) -> Self {
        Self {
            thickness,
            segments: segments.max(3),
            ..Default::default()
        }
    }

    /// Get all edges from mesh.
    fn get_edges(&self, mesh: &ModifierMesh) -> Vec<(usize, usize)> {
        let mut edges = HashMap::new();

        for face in &mesh.faces {
            for i in 0..face.len() {
                let v0 = face[i];
                let v1 = face[(i + 1) % face.len()];

                let key = if v0 < v1 { (v0, v1) } else { (v1, v0) };
                *edges.entry(key).or_insert(0) += 1;
            }
        }

        if self.boundary {
            // Only boundary edges (appear once)
            edges.into_iter()
                .filter(|(_, count)| *count == 1)
                .map(|(edge, _)| edge)
                .collect()
        } else {
            // All edges
            edges.into_keys().collect()
        }
    }

    /// Create tube mesh for an edge.
    fn create_tube(&self, v0: Point3<f64>, v1: Point3<f64>, n0: Vector3<f64>, n1: Vector3<f64>) -> ModifierMesh {
        let mut tube = ModifierMesh::new();

        let edge_vec = v1 - v0;
        let edge_len = edge_vec.magnitude();
        if edge_len < 1e-10 {
            return tube;
        }

        let edge_dir = edge_vec.normalize();

        // Create rotation to align Z axis with edge
        let up = Vector3::z();
        let rotation = if (edge_dir - up).magnitude() < 1e-6 {
            UnitQuaternion::identity()
        } else if (edge_dir + up).magnitude() < 1e-6 {
            UnitQuaternion::from_axis_angle(&Vector3::x_axis(), std::f64::consts::PI)
        } else {
            UnitQuaternion::rotation_between(&up, &edge_dir)
                .unwrap_or(UnitQuaternion::identity())
        };

        // Apply offset along normals
        let offset0 = if self.offset != 0.0 { n0 * self.offset } else { Vector3::zeros() };
        let offset1 = if self.offset != 0.0 { n1 * self.offset } else { Vector3::zeros() };

        let start = Point3::from(v0.coords + offset0);
        let _end = Point3::from(v1.coords + offset1);

        // Calculate radius (thickness adjusted by edge length for even thickness)
        let radius = if self.even_thickness {
            self.thickness
        } else {
            self.thickness * edge_len.sqrt()
        };

        // Create circular profile
        let seg = self.segments;
        for i in 0..seg {
            let angle = (i as f64 / seg as f64) * 2.0 * std::f64::consts::PI;
            let x = angle.cos() * radius;
            let y = angle.sin() * radius;

            // Bottom ring (at v0)
            let local_pos0 = Vector3::new(x, y, 0.0);
            let rotated0 = rotation * local_pos0;
            let world_pos0 = start + rotated0;
            tube.add_vertex(world_pos0);

            // Top ring (at v1)
            let local_pos1 = Vector3::new(x, y, edge_len);
            let rotated1 = rotation * local_pos1;
            let world_pos1 = start + rotated1;
            tube.add_vertex(world_pos1);
        }

        // Create tube faces (quads)
        for i in 0..seg {
            let i0 = i * 2;
            let i1 = i * 2 + 1;
            let i2 = ((i + 1) % seg) * 2 + 1;
            let i3 = ((i + 1) % seg) * 2;

            tube.add_face(vec![i0, i1, i2, i3]);
        }

        tube.compute_normals();
        tube
    }

    /// Get vertex normal (average of adjacent face normals).
    fn get_vertex_normal(&self, vertex_idx: usize, mesh: &ModifierMesh) -> Vector3<f64> {
        if vertex_idx < mesh.normals.len() {
            return mesh.normals[vertex_idx];
        }

        // Fallback: compute from adjacent faces
        let mut normal = Vector3::zeros();
        let mut count = 0;

        for face in &mesh.faces {
            if face.contains(&vertex_idx) && face.len() >= 3 {
                let v0 = mesh.positions[face[0]];
                let v1 = mesh.positions[face[1]];
                let v2 = mesh.positions[face[2]];

                let face_normal = (v1 - v0).cross(&(v2 - v0));
                normal += face_normal;
                count += 1;
            }
        }

        if count > 0 {
            normal /= count as f64;
            let len = normal.magnitude();
            if len > 1e-10 {
                return normal / len;
            }
        }

        Vector3::y()
    }
}

impl Modifier for WireframeModifier {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_id(&self) -> &'static str {
        "WireframeModifier"
    }

    fn apply(&self, mesh: &ModifierMesh) -> ModifierMesh {
        let edges = self.get_edges(mesh);

        if edges.is_empty() {
            return if self.replace_original {
                ModifierMesh::new()
            } else {
                mesh.clone()
            };
        }

        let mut result = if self.replace_original {
            ModifierMesh::new()
        } else {
            mesh.clone()
        };

        // Create tube for each edge
        for (v0_idx, v1_idx) in edges {
            let v0 = mesh.positions[v0_idx];
            let v1 = mesh.positions[v1_idx];

            let n0 = self.get_vertex_normal(v0_idx, mesh);
            let n1 = self.get_vertex_normal(v1_idx, mesh);

            let tube = self.create_tube(v0, v1, n0, n1);
            result.merge(&tube);
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
    fn test_wireframe_basic() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.5, 1.0, 0.0),
            ],
            vec![vec![0, 1, 2]],
        );

        let modifier = WireframeModifier::new(0.05);
        let result = modifier.apply(&mesh);

        // Triangle has 3 edges, each creates a tube
        assert!(result.positions.len() > 0);
        assert!(result.faces.len() > 0);
    }

    #[test]
    fn test_wireframe_boundary_only() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(1.0, 1.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
            ],
            vec![
                vec![0, 1, 2],
                vec![0, 2, 3],
            ],
        );

        let mut modifier = WireframeModifier::new(0.05);
        modifier.boundary = true;

        let result = modifier.apply(&mesh);

        // Should only create wires for boundary edges
        assert!(result.positions.len() > 0);
    }

    #[test]
    fn test_wireframe_keep_original() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.5, 1.0, 0.0),
            ],
            vec![vec![0, 1, 2]],
        );

        let mut modifier = WireframeModifier::new(0.05);
        modifier.replace_original = false;

        let result = modifier.apply(&mesh);

        // Should have more vertices than original (original + wires)
        assert!(result.positions.len() > mesh.positions.len());
    }
}
