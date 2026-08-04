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

//! Mirror modifier.
//!
//! Mirrors mesh geometry across specified axes.

use super::stack::{Modifier, ModifierMesh};
use nalgebra::{Point3, Vector3};
use std::any::Any;
use std::collections::HashMap;

/// Mirror axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirrorAxis {
    /// Mirror across X axis (YZ plane).
    X,
    /// Mirror across Y axis (XZ plane).
    Y,
    /// Mirror across Z axis (XY plane).
    Z,
}

/// Mirror modifier.
#[derive(Debug, Clone)]
pub struct MirrorModifier {
    /// Modifier name.
    pub name: String,
    /// Whether enabled.
    pub enabled: bool,
    /// Mirror axis.
    pub axis: MirrorAxis,
    /// Mirror origin point.
    pub origin: Point3<f64>,
    /// Merge vertices at mirror seam.
    pub merge: bool,
    /// Merge distance threshold.
    pub merge_threshold: f64,
    /// Flip normals on mirrored geometry.
    pub flip_normals: bool,
    /// Use bisect (cut geometry at mirror plane).
    pub bisect: bool,
    /// Mirror UVs.
    pub mirror_uvs: bool,
    /// UV offset for mirrored geometry.
    pub uv_offset: (f64, f64),
}

impl Default for MirrorModifier {
    fn default() -> Self {
        Self {
            name: "Mirror".to_string(),
            enabled: true,
            axis: MirrorAxis::X,
            origin: Point3::origin(),
            merge: true,
            merge_threshold: 0.001,
            flip_normals: false,
            bisect: false,
            mirror_uvs: true,
            uv_offset: (1.0, 0.0),
        }
    }
}

impl MirrorModifier {
    /// Create new mirror modifier.
    pub fn new(axis: MirrorAxis) -> Self {
        Self {
            axis,
            ..Default::default()
        }
    }

    /// Mirror a point across the axis.
    fn mirror_point(&self, p: Point3<f64>) -> Point3<f64> {
        let rel = p - self.origin;
        let mirrored = match self.axis {
            MirrorAxis::X => Vector3::new(-rel.x, rel.y, rel.z),
            MirrorAxis::Y => Vector3::new(rel.x, -rel.y, rel.z),
            MirrorAxis::Z => Vector3::new(rel.x, rel.y, -rel.z),
        };
        self.origin + mirrored
    }

    /// Mirror a normal across the axis.
    fn mirror_normal(&self, n: Vector3<f64>) -> Vector3<f64> {
        let mirrored = match self.axis {
            MirrorAxis::X => Vector3::new(-n.x, n.y, n.z),
            MirrorAxis::Y => Vector3::new(n.x, -n.y, n.z),
            MirrorAxis::Z => Vector3::new(n.x, n.y, -n.z),
        };
        if self.flip_normals {
            -mirrored
        } else {
            mirrored
        }
    }

    /// Check if point is on mirror side.
    fn on_mirror_side(&self, p: Point3<f64>) -> bool {
        let rel = p - self.origin;
        match self.axis {
            MirrorAxis::X => rel.x >= -self.merge_threshold,
            MirrorAxis::Y => rel.y >= -self.merge_threshold,
            MirrorAxis::Z => rel.z >= -self.merge_threshold,
        }
    }

    /// Get distance to mirror plane.
    fn distance_to_plane(&self, p: Point3<f64>) -> f64 {
        let rel = p - self.origin;
        match self.axis {
            MirrorAxis::X => rel.x.abs(),
            MirrorAxis::Y => rel.y.abs(),
            MirrorAxis::Z => rel.z.abs(),
        }
    }
}

impl Modifier for MirrorModifier {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_id(&self) -> &'static str {
        "MirrorModifier"
    }

    fn apply(&self, mesh: &ModifierMesh) -> ModifierMesh {
        let mut result = mesh.clone();

        // Optionally bisect the mesh first
        if self.bisect {
            // Remove vertices/faces on wrong side of mirror plane
            let keep_vertices: Vec<bool> = mesh
                .positions
                .iter()
                .map(|p| self.on_mirror_side(*p))
                .collect();

            // Rebuild mesh with only kept vertices
            let mut vertex_map: HashMap<usize, usize> = HashMap::new();
            let mut new_positions = Vec::new();
            let mut new_normals = Vec::new();

            for (i, &keep) in keep_vertices.iter().enumerate() {
                if keep {
                    vertex_map.insert(i, new_positions.len());
                    new_positions.push(mesh.positions[i]);
                    if i < mesh.normals.len() {
                        new_normals.push(mesh.normals[i]);
                    }
                }
            }

            let new_faces: Vec<Vec<usize>> = mesh
                .faces
                .iter()
                .filter_map(|face| {
                    let new_face: Vec<usize> = face
                        .iter()
                        .filter_map(|&vi| vertex_map.get(&vi).copied())
                        .collect();
                    if new_face.len() >= 3 {
                        Some(new_face)
                    } else {
                        None
                    }
                })
                .collect();

            result.positions = new_positions;
            result.normals = new_normals;
            result.faces = new_faces;
        }

        let original_vertex_count = result.positions.len();
        let original_face_count = result.faces.len();

        // Build vertex index map: maps original vertex index to its mirrored vertex index
        // For merged vertices, this maps to the original index; for non-merged, to the new index
        let mut vertex_index_map: HashMap<usize, usize> = HashMap::new();

        // Add mirrored vertices, tracking actual indices
        let mut next_idx = original_vertex_count;
        for i in 0..original_vertex_count {
            let pos = result.positions[i];
            if self.merge && self.distance_to_plane(pos) < self.merge_threshold {
                // Vertex on mirror plane - maps to itself
                vertex_index_map.insert(i, i);
            } else {
                // Add new mirrored vertex
                let mirrored_pos = self.mirror_point(pos);
                result.positions.push(mirrored_pos);

                let mirrored_normal = if i < result.normals.len() {
                    self.mirror_normal(result.normals[i])
                } else {
                    Vector3::y()
                };
                result.normals.push(mirrored_normal);

                vertex_index_map.insert(i, next_idx);
                next_idx += 1;
            }
        }

        // Add mirrored faces with reversed winding
        for face_idx in 0..original_face_count {
            let face = &result.faces[face_idx];
            let mirrored_face: Vec<usize> = face
                .iter()
                .rev() // Reverse winding for correct normals
                .map(|&vi| *vertex_index_map.get(&vi).unwrap_or(&vi))
                .collect();

            // Only add if not degenerate
            if mirrored_face.len() >= 3 {
                result.faces.push(mirrored_face);
            }
        }

        // Mirror UVs if enabled
        if self.mirror_uvs && !result.uvs.is_empty() {
            let original_uvs = result.uvs.clone();
            for i in 0..original_vertex_count {
                if i < original_uvs.len() {
                    // Only add UV for vertices that got a new mirrored position
                    if let Some(&mirrored_idx) = vertex_index_map.get(&i) {
                        if mirrored_idx >= original_vertex_count {
                            let (u, v) = original_uvs[i];
                            result
                                .uvs
                                .push((u + self.uv_offset.0, v + self.uv_offset.1));
                        }
                    }
                }
            }
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
    fn test_mirror_x() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(2.0, 0.0, 0.0),
                Point3::new(1.0, 1.0, 0.0),
            ],
            vec![vec![0, 1, 2]],
        );

        let modifier = MirrorModifier::new(MirrorAxis::X);
        let result = modifier.apply(&mesh);

        // Should have 6 vertices (3 original + 3 mirrored)
        assert_eq!(result.positions.len(), 6);
        // Should have 2 faces
        assert_eq!(result.faces.len(), 2);

        // Check mirrored positions
        assert!((result.positions[3].x + 1.0).abs() < 1e-10);
        assert!((result.positions[4].x + 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_mirror_with_merge() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0), // On mirror plane
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.0, 1.0, 0.0), // On mirror plane
            ],
            vec![vec![0, 1, 2]],
        );

        let mut modifier = MirrorModifier::new(MirrorAxis::X);
        modifier.merge = true;
        modifier.merge_threshold = 0.001;

        let result = modifier.apply(&mesh);

        // Vertices on mirror plane should be merged
        // 3 original + 1 mirrored (the non-zero x vertex)
        assert_eq!(result.positions.len(), 4);
    }

    #[test]
    fn test_mirror_y() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 1.0, 0.0),
                Point3::new(1.0, 1.0, 0.0),
                Point3::new(0.0, 2.0, 0.0),
            ],
            vec![vec![0, 1, 2]],
        );

        let modifier = MirrorModifier::new(MirrorAxis::Y);
        let result = modifier.apply(&mesh);

        assert_eq!(result.positions.len(), 6);

        // Check mirrored Y positions
        assert!((result.positions[3].y + 1.0).abs() < 1e-10);
        assert!((result.positions[5].y + 2.0).abs() < 1e-10);
    }
}
