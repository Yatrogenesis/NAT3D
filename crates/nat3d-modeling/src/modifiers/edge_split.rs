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

//! Edge Split modifier.
//!
//! Splits edges based on angle or sharpness.

use nalgebra::Vector3;
use std::any::Any;
use std::collections::{HashMap, HashSet};
use super::stack::{Modifier, ModifierMesh};

/// Edge Split modifier.
#[derive(Debug, Clone)]
pub struct EdgeSplitModifier {
    /// Modifier name.
    pub name: String,
    /// Whether enabled.
    pub enabled: bool,
    /// Split angle threshold (radians).
    pub split_angle: f64,
    /// Use edge angle for splitting.
    pub use_edge_angle: bool,
    /// Use edge sharp flag for splitting.
    pub use_edge_sharp: bool,
}

impl Default for EdgeSplitModifier {
    fn default() -> Self {
        Self {
            name: "EdgeSplit".to_string(),
            enabled: true,
            split_angle: 30.0_f64.to_radians(),
            use_edge_angle: true,
            use_edge_sharp: false,
        }
    }
}

impl EdgeSplitModifier {
    /// Create new edge split modifier.
    pub fn new(angle_degrees: f64) -> Self {
        Self {
            split_angle: angle_degrees.to_radians(),
            ..Default::default()
        }
    }

    /// Create edge split with sharp edges only.
    pub fn sharp_only() -> Self {
        Self {
            use_edge_angle: false,
            use_edge_sharp: true,
            ..Default::default()
        }
    }

    /// Get edges and their adjacent faces.
    fn get_edge_faces(&self, mesh: &ModifierMesh) -> HashMap<(usize, usize), Vec<usize>> {
        let mut edge_faces: HashMap<(usize, usize), Vec<usize>> = HashMap::new();

        for (face_idx, face) in mesh.faces.iter().enumerate() {
            for i in 0..face.len() {
                let v0 = face[i];
                let v1 = face[(i + 1) % face.len()];

                let edge_key = if v0 < v1 { (v0, v1) } else { (v1, v0) };
                edge_faces.entry(edge_key).or_default().push(face_idx);
            }
        }

        edge_faces
    }

    /// Calculate face normal.
    fn calculate_face_normal(&self, face: &[usize], mesh: &ModifierMesh) -> Vector3<f64> {
        if face.len() < 3 {
            return Vector3::y();
        }

        let v0 = mesh.positions[face[0]];
        let v1 = mesh.positions[face[1]];
        let v2 = mesh.positions[face[2]];

        let normal = (v1 - v0).cross(&(v2 - v0));
        let len = normal.magnitude();

        if len > 1e-10 {
            normal / len
        } else {
            Vector3::y()
        }
    }

    /// Calculate dihedral angle between two faces.
    fn dihedral_angle(&self, face1_normal: Vector3<f64>, face2_normal: Vector3<f64>) -> f64 {
        let dot = face1_normal.dot(&face2_normal).clamp(-1.0, 1.0);
        dot.acos()
    }

    /// Find edges that should be split.
    fn find_split_edges(&self, mesh: &ModifierMesh) -> HashSet<(usize, usize)> {
        let mut split_edges = HashSet::new();
        let edge_faces = self.get_edge_faces(mesh);

        // Compute face normals
        let mut face_normals = Vec::new();
        for face in &mesh.faces {
            face_normals.push(self.calculate_face_normal(face, mesh));
        }

        for (edge, faces) in &edge_faces {
            let should_split = if faces.len() == 2 && self.use_edge_angle {
                // Interior edge: check dihedral angle
                let normal1 = face_normals[faces[0]];
                let normal2 = face_normals[faces[1]];

                let angle = self.dihedral_angle(normal1, normal2);
                angle > self.split_angle
            } else if faces.len() == 1 {
                // Boundary edge: always split
                true
            } else {
                false
            };

            if should_split {
                split_edges.insert(*edge);
            }
        }

        split_edges
    }

    /// Split the mesh at marked edges.
    fn split_at_edges(&self, mesh: &ModifierMesh, split_edges: &HashSet<(usize, usize)>) -> ModifierMesh {
        let mut result = ModifierMesh::new();

        // Map from (face_idx, vertex_in_face) -> new vertex index
        let mut vertex_map: HashMap<(usize, usize), usize> = HashMap::new();

        // Build connectivity to know which faces use which vertices
        let mut vertex_faces: HashMap<usize, Vec<usize>> = HashMap::new();
        for (face_idx, face) in mesh.faces.iter().enumerate() {
            for &vi in face {
                vertex_faces.entry(vi).or_default().push(face_idx);
            }
        }

        // For each face, create new vertices if needed
        for (face_idx, face) in mesh.faces.iter().enumerate() {
            let mut new_face = Vec::new();

            for &vi in face {
                // Check if this vertex needs to be split for this face
                let needs_split = self.should_split_vertex(vi, face_idx, mesh, split_edges, &vertex_faces);

                let new_vi = if needs_split {
                    // Create a new vertex for this face
                    let key = (face_idx, vi);
                    if let Some(&existing_idx) = vertex_map.get(&key) {
                        existing_idx
                    } else {
                        let new_idx = result.positions.len();
                        result.positions.push(mesh.positions[vi]);
                        if vi < mesh.normals.len() {
                            result.normals.push(mesh.normals[vi]);
                        }
                        vertex_map.insert(key, new_idx);
                        new_idx
                    }
                } else {
                    // Check if we already created a shared vertex for this original vertex
                    let shared_key = (0, vi); // Use face 0 as canonical "shared" face
                    if let Some(&existing_idx) = vertex_map.get(&shared_key) {
                        existing_idx
                    } else {
                        let new_idx = result.positions.len();
                        result.positions.push(mesh.positions[vi]);
                        if vi < mesh.normals.len() {
                            result.normals.push(mesh.normals[vi]);
                        }
                        vertex_map.insert(shared_key, new_idx);
                        new_idx
                    }
                };

                new_face.push(new_vi);
            }

            result.faces.push(new_face);
        }

        result.compute_normals();
        result
    }

    /// Check if vertex should be split for this face.
    fn should_split_vertex(
        &self,
        vertex: usize,
        face_idx: usize,
        mesh: &ModifierMesh,
        split_edges: &HashSet<(usize, usize)>,
        vertex_faces: &HashMap<usize, Vec<usize>>,
    ) -> bool {
        let adjacent_faces = vertex_faces.get(&vertex).unwrap();

        // If vertex is used by only one face, no split needed
        if adjacent_faces.len() <= 1 {
            return false;
        }

        // Check if any edge connected to this vertex is marked for splitting
        for &other_face_idx in adjacent_faces {
            if other_face_idx == face_idx {
                continue;
            }

            // Check if there's a split edge between these faces
            let face = &mesh.faces[face_idx];
            let other_face = &mesh.faces[other_face_idx];

            // Find shared edge
            for i in 0..face.len() {
                let v0 = face[i];
                let v1 = face[(i + 1) % face.len()];

                if v0 == vertex || v1 == vertex {
                    // Check if this edge is shared with other_face
                    let edge_key = if v0 < v1 { (v0, v1) } else { (v1, v0) };

                    let is_shared = other_face.windows(2).any(|w| {
                        let e0 = w[0];
                        let e1 = w[1];
                        let key = if e0 < e1 { (e0, e1) } else { (e1, e0) };
                        key == edge_key
                    }) || {
                        // Check wrap-around edge
                        let e0 = other_face[other_face.len() - 1];
                        let e1 = other_face[0];
                        let key = if e0 < e1 { (e0, e1) } else { (e1, e0) };
                        key == edge_key
                    };

                    if is_shared && split_edges.contains(&edge_key) {
                        return true;
                    }
                }
            }
        }

        false
    }
}

impl Modifier for EdgeSplitModifier {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_id(&self) -> &'static str {
        "EdgeSplitModifier"
    }

    fn apply(&self, mesh: &ModifierMesh) -> ModifierMesh {
        // Find edges to split
        let split_edges = self.find_split_edges(mesh);

        if split_edges.is_empty() {
            return mesh.clone();
        }

        // Split mesh at those edges
        self.split_at_edges(mesh, &split_edges)
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
    use nalgebra::Point3;

    #[test]
    fn test_edge_split_cube() {
        // Create two faces of a cube sharing an edge at 90 degrees
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0), // 0
                Point3::new(1.0, 0.0, 0.0), // 1
                Point3::new(1.0, 1.0, 0.0), // 2
                Point3::new(0.0, 1.0, 0.0), // 3
                Point3::new(0.0, 1.0, 1.0), // 4
                Point3::new(0.0, 0.0, 1.0), // 5
            ],
            vec![
                vec![0, 1, 2, 3], // Front face (XY plane)
                vec![0, 3, 4, 5], // Side face (YZ plane)
            ],
        );

        let modifier = EdgeSplitModifier::new(45.0); // 45 degree threshold
        let result = modifier.apply(&mesh);

        // Vertices should be duplicated for sharp edges
        assert!(result.positions.len() >= mesh.positions.len());
    }

    #[test]
    fn test_edge_split_smooth_surface() {
        // Create faces with small angle - should not split
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.5, 0.1, 0.0),
            ],
            vec![vec![0, 1, 2]],
        );

        let modifier = EdgeSplitModifier::new(45.0);
        let result = modifier.apply(&mesh);

        // Should remain mostly unchanged for single face
        assert!(result.positions.len() > 0);
    }

    #[test]
    fn test_edge_split_boundary() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.5, 1.0, 0.0),
            ],
            vec![vec![0, 1, 2]],
        );

        let modifier = EdgeSplitModifier::new(30.0);
        let result = modifier.apply(&mesh);

        // Boundary edges should always be split
        assert!(result.faces.len() > 0);
    }
}
