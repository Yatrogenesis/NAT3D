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

//! Skin modifier.
//!
//! Generates mesh surface from skeleton (edge network) with branching support.

use nalgebra::{Point3, Vector3};
use std::any::Any;
use std::f64::consts::PI;
use super::stack::{Modifier, ModifierMesh};

/// Cross-section shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossSectionShape {
    /// Circular cross-section.
    Circle,
    /// Square cross-section.
    Square,
}

/// Skin modifier.
#[derive(Clone)]
pub struct SkinModifier {
    /// Modifier name.
    pub name: String,
    /// Whether enabled.
    pub enabled: bool,
    /// Branch smoothing factor (0 = sharp, 1 = smooth).
    pub branch_smoothing: f64,
    /// Use smooth shading.
    pub smooth_shading: bool,
    /// Number of subdivisions around each edge.
    pub subdivisions: usize,
    /// Root radius (for first vertex in chain).
    pub root_radius: f64,
    /// Tip radius (for last vertex in chain).
    pub tip_radius: f64,
    /// Cross-section shape.
    pub shape: CrossSectionShape,
    /// Use vertex size attribute if available.
    pub use_vertex_size: bool,
    /// Symmetry axis for mirroring.
    pub symmetry_x: bool,
    pub symmetry_y: bool,
    pub symmetry_z: bool,
}

impl Default for SkinModifier {
    fn default() -> Self {
        Self {
            name: "Skin".to_string(),
            enabled: true,
            branch_smoothing: 0.5,
            smooth_shading: true,
            subdivisions: 8,
            root_radius: 0.1,
            tip_radius: 0.05,
            shape: CrossSectionShape::Circle,
            use_vertex_size: false,
            symmetry_x: false,
            symmetry_y: false,
            symmetry_z: false,
        }
    }
}

impl SkinModifier {
    /// Create new skin modifier.
    pub fn new(subdivisions: usize) -> Self {
        Self {
            subdivisions,
            ..Default::default()
        }
    }

    /// Create with radii.
    pub fn with_radii(subdivisions: usize, root_radius: f64, tip_radius: f64) -> Self {
        Self {
            subdivisions,
            root_radius,
            tip_radius,
            ..Default::default()
        }
    }

    /// Build edge adjacency from mesh.
    fn build_edge_graph(&self, mesh: &ModifierMesh) -> Vec<Vec<usize>> {
        let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); mesh.positions.len()];

        // Build from faces (assume edges are face connections)
        for face in &mesh.faces {
            if face.len() == 2 {
                // Explicit edge
                let v0 = face[0];
                let v1 = face[1];
                if v0 < adjacency.len() && v1 < adjacency.len() {
                    if !adjacency[v0].contains(&v1) {
                        adjacency[v0].push(v1);
                    }
                    if !adjacency[v1].contains(&v0) {
                        adjacency[v1].push(v0);
                    }
                }
            }
        }

        adjacency
    }

    /// Calculate radius for vertex.
    fn get_vertex_radius(&self, vertex_idx: usize, total_vertices: usize) -> f64 {
        if self.use_vertex_size {
            // Could read from vertex attributes
            self.root_radius
        } else {
            // Interpolate between root and tip
            let t = vertex_idx as f64 / total_vertices.max(1) as f64;
            self.root_radius + (self.tip_radius - self.root_radius) * t
        }
    }

    /// Generate cross-section circle.
    fn generate_circle_cross_section(
        &self,
        center: Point3<f64>,
        normal: Vector3<f64>,
        radius: f64,
    ) -> Vec<Point3<f64>> {
        let mut points = Vec::with_capacity(self.subdivisions);

        // Choose perpendicular vectors
        let tangent = if normal.y.abs() < 0.9 {
            Vector3::y().cross(&normal).normalize()
        } else {
            Vector3::x().cross(&normal).normalize()
        };

        let bitangent = normal.cross(&tangent).normalize();

        // Generate circle points
        for i in 0..self.subdivisions {
            let angle = 2.0 * PI * i as f64 / self.subdivisions as f64;
            let offset = tangent * angle.cos() * radius + bitangent * angle.sin() * radius;
            points.push(center + offset);
        }

        points
    }

    /// Generate cross-section square.
    fn generate_square_cross_section(
        &self,
        center: Point3<f64>,
        normal: Vector3<f64>,
        radius: f64,
    ) -> Vec<Point3<f64>> {
        let mut points = Vec::with_capacity(self.subdivisions);

        let tangent = if normal.y.abs() < 0.9 {
            Vector3::y().cross(&normal).normalize()
        } else {
            Vector3::x().cross(&normal).normalize()
        };

        let bitangent = normal.cross(&tangent).normalize();

        // Generate square points
        let points_per_side = self.subdivisions / 4;
        let side_length = radius * 2.0;

        for i in 0..self.subdivisions {
            let side = i / points_per_side.max(1);
            let t = (i % points_per_side.max(1)) as f64 / points_per_side.max(1) as f64;

            let offset = match side {
                0 => tangent * (t * side_length - radius) + bitangent * radius,
                1 => tangent * radius + bitangent * ((1.0 - t) * side_length - radius),
                2 => tangent * ((1.0 - t) * side_length - radius) - bitangent * radius,
                _ => -tangent * radius + bitangent * (t * side_length - radius),
            };

            points.push(center + offset);
        }

        points
    }

    /// Generate cross-section.
    fn generate_cross_section(
        &self,
        center: Point3<f64>,
        normal: Vector3<f64>,
        radius: f64,
    ) -> Vec<Point3<f64>> {
        match self.shape {
            CrossSectionShape::Circle => self.generate_circle_cross_section(center, normal, radius),
            CrossSectionShape::Square => self.generate_square_cross_section(center, normal, radius),
        }
    }

    /// Connect two cross-sections with faces.
    fn connect_cross_sections(
        &self,
        section1: &[Point3<f64>],
        _section2: &[Point3<f64>],
        offset1: usize,
        offset2: usize,
    ) -> Vec<Vec<usize>> {
        let mut faces = Vec::new();

        for i in 0..section1.len() {
            let next_i = (i + 1) % section1.len();

            let v0 = offset1 + i;
            let v1 = offset1 + next_i;
            let v2 = offset2 + next_i;
            let v3 = offset2 + i;

            // Create quad (or two triangles)
            faces.push(vec![v0, v1, v2, v3]);
        }

        faces
    }

    /// Generate skin from edge network.
    fn generate_skin(&self, mesh: &ModifierMesh) -> ModifierMesh {
        let mut result = ModifierMesh::new();
        let adjacency = self.build_edge_graph(mesh);

        if mesh.positions.is_empty() {
            return result;
        }

        // Process each edge
        for i in 0..mesh.positions.len() {
            let neighbors = &adjacency[i];

            for &neighbor in neighbors {
                if neighbor <= i {
                    // Process each edge only once
                    continue;
                }

                let v0 = mesh.positions[i];
                let v1 = mesh.positions[neighbor];

                let edge_dir = (v1 - v0).normalize();
                let radius0 = self.get_vertex_radius(i, mesh.positions.len());
                let radius1 = self.get_vertex_radius(neighbor, mesh.positions.len());

                // Generate cross-sections
                let section0 = self.generate_cross_section(v0, edge_dir, radius0);
                let section1 = self.generate_cross_section(v1, edge_dir, radius1);

                // Add vertices
                let offset0 = result.positions.len();
                for p in &section0 {
                    result.add_vertex(*p);
                }

                let offset1 = result.positions.len();
                for p in &section1 {
                    result.add_vertex(*p);
                }

                // Connect sections
                let faces = self.connect_cross_sections(&section0, &section1, offset0, offset1);
                for face in faces {
                    result.add_face(face);
                }
            }
        }

        result.compute_normals();
        result
    }
}

impl Modifier for SkinModifier {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_id(&self) -> &'static str {
        "SkinModifier"
    }

    fn apply(&self, mesh: &ModifierMesh) -> ModifierMesh {
        if mesh.positions.is_empty() {
            return mesh.clone();
        }

        self.generate_skin(mesh)
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
    fn test_skin_basic() {
        // Create simple edge
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
            ],
            vec![vec![0, 1]], // Single edge
        );

        let modifier = SkinModifier::new(8);
        let result = modifier.apply(&mesh);

        // Should generate cylindrical mesh
        assert!(result.vertex_count() > 0);
        assert!(result.face_count() > 0);
    }

    #[test]
    fn test_skin_branching() {
        // Create Y-shaped skeleton
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
                Point3::new(-0.5, 1.5, 0.0),
                Point3::new(0.5, 1.5, 0.0),
            ],
            vec![
                vec![0, 1],
                vec![1, 2],
                vec![1, 3],
            ],
        );

        let modifier = SkinModifier::new(6);
        let result = modifier.apply(&mesh);

        assert!(result.vertex_count() > 0);
        assert!(result.face_count() > 0);
    }

    #[test]
    fn test_skin_shapes() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
            ],
            vec![vec![0, 1]],
        );

        // Circle
        let modifier_circle = SkinModifier::new(8);
        let result_circle = modifier_circle.apply(&mesh);
        assert!(result_circle.vertex_count() > 0);

        // Square
        let mut modifier_square = SkinModifier::new(8);
        modifier_square.shape = CrossSectionShape::Square;
        let result_square = modifier_square.apply(&mesh);
        assert!(result_square.vertex_count() > 0);
    }
}
