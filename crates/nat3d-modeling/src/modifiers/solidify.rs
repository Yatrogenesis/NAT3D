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

//! Solidify modifier.
//!
//! Adds thickness to mesh surfaces.

use super::stack::{Modifier, ModifierMesh};
use nalgebra::Vector3;
use std::any::Any;
use std::collections::HashMap;

/// Solidify mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolidifyMode {
    /// Simple offset (may create self-intersections).
    Simple,
    /// Complex offset with intersection handling.
    Complex,
}

/// Rim fill mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RimFillMode {
    /// No rim fill.
    None,
    /// Fill rim with quads.
    Quads,
    /// Fill rim with triangles.
    Triangles,
}

/// Solidify modifier.
#[derive(Debug, Clone)]
pub struct SolidifyModifier {
    /// Modifier name.
    pub name: String,
    /// Whether enabled.
    pub enabled: bool,
    /// Thickness.
    pub thickness: f64,
    /// Offset (-1 to 1, 0 = centered).
    pub offset: f64,
    /// Solidify mode.
    pub mode: SolidifyMode,
    /// Fill rim.
    pub rim_fill: RimFillMode,
    /// Use even thickness (corrects for sharp angles).
    pub even_thickness: bool,
    /// Flip normals.
    pub flip_normals: bool,
    /// Use vertex group for thickness.
    pub vertex_group: Option<String>,
    /// High quality normals.
    pub high_quality_normals: bool,
}

impl Default for SolidifyModifier {
    fn default() -> Self {
        Self {
            name: "Solidify".to_string(),
            enabled: true,
            thickness: 0.1,
            offset: -1.0, // Outward by default
            mode: SolidifyMode::Simple,
            rim_fill: RimFillMode::Quads,
            even_thickness: true,
            flip_normals: false,
            vertex_group: None,
            high_quality_normals: false,
        }
    }
}

impl SolidifyModifier {
    /// Create new solidify modifier.
    pub fn new(thickness: f64) -> Self {
        Self {
            thickness,
            ..Default::default()
        }
    }

    /// Calculate vertex normal (average of adjacent face normals).
    fn vertex_normal(&self, mesh: &ModifierMesh, vertex_idx: usize) -> Vector3<f64> {
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
            normal / count as f64
        } else {
            Vector3::y()
        }
        .normalize()
    }

    /// Calculate even thickness offset for a vertex.
    fn even_thickness_factor(&self, mesh: &ModifierMesh, vertex_idx: usize) -> f64 {
        if !self.even_thickness {
            return 1.0;
        }

        let vertex_normal = self.vertex_normal(mesh, vertex_idx);

        let mut min_dot: f64 = 1.0;
        for face in &mesh.faces {
            if face.contains(&vertex_idx) && face.len() >= 3 {
                let v0 = mesh.positions[face[0]];
                let v1 = mesh.positions[face[1]];
                let v2 = mesh.positions[face[2]];

                let face_normal = (v1 - v0).cross(&(v2 - v0)).normalize();
                let dot = vertex_normal.dot(&face_normal).abs();
                min_dot = min_dot.min(dot);
            }
        }

        // Prevent division by very small values
        if min_dot < 0.1 {
            min_dot = 0.1;
        }

        1.0 / min_dot
    }

    /// Find boundary edges (edges with only one adjacent face).
    fn find_boundary_edges(&self, mesh: &ModifierMesh) -> Vec<(usize, usize)> {
        let mut edge_count: HashMap<(usize, usize), usize> = HashMap::new();

        for face in &mesh.faces {
            for i in 0..face.len() {
                let v0 = face[i];
                let v1 = face[(i + 1) % face.len()];
                let edge = if v0 < v1 { (v0, v1) } else { (v1, v0) };
                *edge_count.entry(edge).or_insert(0) += 1;
            }
        }

        edge_count
            .into_iter()
            .filter(|(_, count)| *count == 1)
            .map(|(edge, _)| edge)
            .collect()
    }

    /// Get thickness for a vertex, considering vertex groups.
    fn get_vertex_thickness(&self, mesh: &ModifierMesh, vertex_idx: usize) -> f64 {
        let base_thickness = self.thickness;

        if let Some(ref group_name) = self.vertex_group {
            if let Some(weights) = mesh.vertex_groups.get(group_name) {
                for &(vi, weight) in weights {
                    if vi == vertex_idx {
                        return base_thickness * weight;
                    }
                }
            }
        }

        base_thickness
    }
}

impl Modifier for SolidifyModifier {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_id(&self) -> &'static str {
        "SolidifyModifier"
    }

    fn apply(&self, mesh: &ModifierMesh) -> ModifierMesh {
        if mesh.positions.is_empty() || mesh.faces.is_empty() || self.thickness.abs() < 1e-10 {
            return mesh.clone();
        }

        let mut result = ModifierMesh::new();
        let original_vertex_count = mesh.positions.len();
        let _original_face_count = mesh.faces.len();

        // Calculate offset multipliers
        let outer_offset = (1.0 + self.offset) / 2.0;
        let inner_offset = (1.0 - self.offset) / 2.0;

        // Create outer shell vertices
        for i in 0..original_vertex_count {
            let pos = mesh.positions[i];
            let normal = self.vertex_normal(mesh, i);
            let thickness = self.get_vertex_thickness(mesh, i);
            let factor = self.even_thickness_factor(mesh, i);

            let offset = normal * (thickness * outer_offset * factor);
            let new_pos = pos + offset;

            result.positions.push(new_pos);
            result
                .normals
                .push(if self.flip_normals { -normal } else { normal });
        }

        // Create inner shell vertices
        for i in 0..original_vertex_count {
            let pos = mesh.positions[i];
            let normal = self.vertex_normal(mesh, i);
            let thickness = self.get_vertex_thickness(mesh, i);
            let factor = self.even_thickness_factor(mesh, i);

            let offset = normal * (-thickness * inner_offset * factor);
            let new_pos = pos + offset;

            result.positions.push(new_pos);
            result
                .normals
                .push(if self.flip_normals { normal } else { -normal });
        }

        // Copy outer shell faces (original winding)
        for face in &mesh.faces {
            let new_face: Vec<usize> = if self.flip_normals {
                face.iter().rev().copied().collect()
            } else {
                face.clone()
            };
            result.faces.push(new_face);
        }

        // Add inner shell faces (reversed winding)
        for face in &mesh.faces {
            let new_face: Vec<usize> = if self.flip_normals {
                face.iter().map(|&vi| vi + original_vertex_count).collect()
            } else {
                face.iter()
                    .rev()
                    .map(|&vi| vi + original_vertex_count)
                    .collect()
            };
            result.faces.push(new_face);
        }

        // Fill rim (edges)
        if self.rim_fill != RimFillMode::None {
            let boundary_edges = self.find_boundary_edges(mesh);

            for (v0, v1) in boundary_edges {
                // Outer shell vertices
                let outer_v0 = v0;
                let outer_v1 = v1;
                // Inner shell vertices
                let inner_v0 = v0 + original_vertex_count;
                let inner_v1 = v1 + original_vertex_count;

                match self.rim_fill {
                    RimFillMode::Quads => {
                        if self.flip_normals {
                            result
                                .faces
                                .push(vec![outer_v0, inner_v0, inner_v1, outer_v1]);
                        } else {
                            result
                                .faces
                                .push(vec![outer_v0, outer_v1, inner_v1, inner_v0]);
                        }
                    }
                    RimFillMode::Triangles => {
                        if self.flip_normals {
                            result.faces.push(vec![outer_v0, inner_v0, outer_v1]);
                            result.faces.push(vec![inner_v0, inner_v1, outer_v1]);
                        } else {
                            result.faces.push(vec![outer_v0, outer_v1, inner_v0]);
                            result.faces.push(vec![outer_v1, inner_v1, inner_v0]);
                        }
                    }
                    RimFillMode::None => {}
                }
            }
        }

        // Copy UVs for both shells
        if !mesh.uvs.is_empty() {
            result.uvs.extend(&mesh.uvs);
            result.uvs.extend(&mesh.uvs);
        }

        // Copy vertex groups for both shells
        for (name, weights) in &mesh.vertex_groups {
            let mut new_weights = weights.clone();
            // Add weights for inner shell
            for &(vi, w) in weights {
                new_weights.push((vi + original_vertex_count, w));
            }
            result.vertex_groups.insert(name.clone(), new_weights);
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
    use nalgebra::Point3;

    #[test]
    fn test_solidify_plane() {
        // Simple plane
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(1.0, 1.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
            ],
            vec![vec![0, 1, 2, 3]],
        );

        let modifier = SolidifyModifier::new(0.1);
        let result = modifier.apply(&mesh);

        // Should have 8 vertices (4 original + 4 offset)
        assert_eq!(result.positions.len(), 8);

        // Should have at least 2 faces (top and bottom)
        assert!(result.faces.len() >= 2);
    }

    #[test]
    fn test_solidify_with_boundary() {
        // Plane with boundary edges
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(1.0, 1.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
            ],
            vec![vec![0, 1, 2, 3]],
        );

        let mut modifier = SolidifyModifier::new(0.1);
        modifier.rim_fill = RimFillMode::Quads;
        let result = modifier.apply(&mesh);

        // With rim fill, should have more faces
        // 2 faces (top/bottom) + 4 rim faces
        assert_eq!(result.faces.len(), 6);
    }

    #[test]
    fn test_offset_modes() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.5, 1.0, 0.0),
            ],
            vec![vec![0, 1, 2]],
        );

        // Test centered offset
        let mut modifier = SolidifyModifier::new(0.2);
        modifier.offset = 0.0; // Centered
        let result = modifier.apply(&mesh);

        // Outer vertices should be offset by 0.1 (half thickness)
        // Inner vertices should be offset by -0.1
        assert_eq!(result.positions.len(), 6);
    }

    #[test]
    fn test_even_thickness() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(1.0, 1.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
            ],
            vec![vec![0, 1, 2, 3]],
        );

        let mut modifier = SolidifyModifier::new(0.1);
        modifier.even_thickness = true;

        let factor = modifier.even_thickness_factor(&mesh, 0);
        // For a flat plane, factor should be 1.0
        assert!((factor - 1.0).abs() < 1e-6);
    }
}
