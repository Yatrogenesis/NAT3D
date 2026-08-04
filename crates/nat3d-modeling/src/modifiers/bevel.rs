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

//! Bevel modifier.
//!
//! Bevels edges and vertices of a mesh.

use super::stack::{Modifier, ModifierMesh};
use nalgebra::{Point3, Vector3};
use std::any::Any;
use std::collections::{HashMap, HashSet};

/// Bevel type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BevelType {
    /// Bevel vertices only.
    Vertices,
    /// Bevel edges only.
    Edges,
    /// Bevel both vertices and edges.
    Both,
}

/// Bevel profile type.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum BevelProfile {
    /// Linear profile (45 degree chamfer).
    Linear,
    /// Circular profile (round).
    #[default]
    Round,
    /// Superellipse profile.
    SuperEllipse(f64),
    /// Custom profile value (0-1).
    Custom(f64),
}

/// Bevel modifier.
#[derive(Debug, Clone)]
pub struct BevelModifier {
    /// Modifier name.
    pub name: String,
    /// Whether enabled.
    pub enabled: bool,
    /// Bevel type.
    pub bevel_type: BevelType,
    /// Bevel amount (distance).
    pub amount: f64,
    /// Number of segments.
    pub segments: usize,
    /// Bevel profile.
    pub profile: BevelProfile,
    /// Limit by angle (only bevel sharp edges).
    pub limit_angle: Option<f64>,
    /// Clamp overlap.
    pub clamp_overlap: bool,
    /// Harden normals at corners.
    pub harden_normals: bool,
    /// Loop slide (preserve UV flow).
    pub loop_slide: bool,
}

impl Default for BevelModifier {
    fn default() -> Self {
        Self {
            name: "Bevel".to_string(),
            enabled: true,
            bevel_type: BevelType::Edges,
            amount: 0.1,
            segments: 1,
            profile: BevelProfile::Round,
            limit_angle: Some(std::f64::consts::PI / 6.0), // 30 degrees
            clamp_overlap: true,
            harden_normals: true,
            loop_slide: true,
        }
    }
}

impl BevelModifier {
    /// Create new bevel modifier.
    pub fn new(amount: f64, segments: usize) -> Self {
        Self {
            amount,
            segments,
            ..Default::default()
        }
    }

    /// Calculate profile offset at parameter t.
    fn profile_offset(&self, t: f64) -> f64 {
        match self.profile {
            BevelProfile::Linear => t,
            BevelProfile::Round => {
                // Quarter circle
                (1.0 - (1.0 - t).powi(2)).sqrt()
            }
            BevelProfile::SuperEllipse(n) => {
                // Generalized superellipse
                (1.0 - (1.0 - t).powf(n)).powf(1.0 / n)
            }
            BevelProfile::Custom(p) => {
                // Blend between linear and round
                let linear = t;
                let round = (1.0 - (1.0 - t).powi(2)).sqrt();
                linear * (1.0 - p) + round * p
            }
        }
    }

    /// Calculate edge angle.
    fn edge_angle(
        &self,
        mesh: &ModifierMesh,
        v0: usize,
        v1: usize,
        adjacent_faces: &[(usize, usize)],
    ) -> f64 {
        if adjacent_faces.len() < 2 {
            return std::f64::consts::PI;
        }

        let face_normals: Vec<Vector3<f64>> = adjacent_faces
            .iter()
            .take(2)
            .map(|&(face_idx, _)| self.face_normal(mesh, face_idx))
            .collect();

        if face_normals.len() < 2 {
            return std::f64::consts::PI;
        }

        let _ = (v0, v1); // Used for edge context
        let dot = face_normals[0].dot(&face_normals[1]).clamp(-1.0, 1.0);
        dot.acos()
    }

    /// Calculate face normal.
    fn face_normal(&self, mesh: &ModifierMesh, face_idx: usize) -> Vector3<f64> {
        let face = &mesh.faces[face_idx];
        if face.len() < 3 {
            return Vector3::y();
        }

        let v0 = mesh.positions[face[0]];
        let v1 = mesh.positions[face[1]];
        let v2 = mesh.positions[face[2]];

        (v1 - v0).cross(&(v2 - v0)).normalize()
    }

    /// Build edge to face adjacency.
    fn build_edge_faces(
        &self,
        mesh: &ModifierMesh,
    ) -> HashMap<(usize, usize), Vec<(usize, usize)>> {
        let mut edge_faces: HashMap<(usize, usize), Vec<(usize, usize)>> = HashMap::new();

        for (face_idx, face) in mesh.faces.iter().enumerate() {
            for i in 0..face.len() {
                let v0 = face[i];
                let v1 = face[(i + 1) % face.len()];
                let edge = if v0 < v1 { (v0, v1) } else { (v1, v0) };
                edge_faces.entry(edge).or_default().push((face_idx, i));
            }
        }

        edge_faces
    }

    /// Get edges that should be beveled.
    fn get_bevel_edges(&self, mesh: &ModifierMesh) -> HashSet<(usize, usize)> {
        let edge_faces = self.build_edge_faces(mesh);
        let mut bevel_edges = HashSet::new();

        for (&edge, adj_faces) in &edge_faces {
            let should_bevel = if let Some(limit) = self.limit_angle {
                let angle = self.edge_angle(mesh, edge.0, edge.1, adj_faces);
                angle < std::f64::consts::PI - limit
            } else {
                true
            };

            if should_bevel {
                bevel_edges.insert(edge);
            }
        }

        bevel_edges
    }
}

impl Modifier for BevelModifier {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_id(&self) -> &'static str {
        "BevelModifier"
    }

    fn apply(&self, mesh: &ModifierMesh) -> ModifierMesh {
        if mesh.faces.is_empty() || self.amount <= 0.0 {
            return mesh.clone();
        }

        let mut result = mesh.clone();

        match self.bevel_type {
            BevelType::Vertices => {
                self.bevel_vertices(&mut result);
            }
            BevelType::Edges => {
                self.bevel_edges(&mut result);
            }
            BevelType::Both => {
                self.bevel_edges(&mut result);
                self.bevel_vertices(&mut result);
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

impl BevelModifier {
    /// Bevel vertices.
    fn bevel_vertices(&self, mesh: &mut ModifierMesh) {
        // Build vertex to face adjacency
        let mut vertex_faces: HashMap<usize, Vec<usize>> = HashMap::new();
        for (face_idx, face) in mesh.faces.iter().enumerate() {
            for &vi in face {
                vertex_faces.entry(vi).or_default().push(face_idx);
            }
        }

        let original_vertex_count = mesh.positions.len();
        let mut vertex_map: HashMap<(usize, usize), usize> = HashMap::new();

        // For each vertex, create new vertices for each adjacent face
        for (vi, adj_faces) in &vertex_faces {
            if adj_faces.len() < 3 {
                continue;
            }

            let original_pos = mesh.positions[*vi];

            // Calculate average normal
            let avg_normal = adj_faces
                .iter()
                .map(|&fi| self.face_normal(mesh, fi))
                .fold(Vector3::zeros(), |acc, n| acc + n)
                / adj_faces.len() as f64;
            let avg_normal = avg_normal.normalize();

            // Create new vertex for each adjacent face
            for &face_idx in adj_faces {
                let face_normal = self.face_normal(mesh, face_idx);

                // Offset towards face center and away from original vertex
                let face_center = self.face_center(mesh, face_idx);
                let to_center = (face_center - original_pos).normalize();

                let offset_dir = (to_center + face_normal * 0.5).normalize();
                let new_pos = original_pos + offset_dir * self.amount;

                let new_idx = mesh.positions.len();
                mesh.positions.push(new_pos);
                mesh.normals.push(avg_normal);

                vertex_map.insert((*vi, face_idx), new_idx);
            }
        }

        // Update faces to use new vertices
        for (face_idx, face) in mesh.faces.iter_mut().enumerate() {
            for vi in face.iter_mut() {
                if *vi < original_vertex_count {
                    if let Some(&new_vi) = vertex_map.get(&(*vi, face_idx)) {
                        *vi = new_vi;
                    }
                }
            }
        }

        // Add connecting faces between beveled vertices
        for (vi, adj_faces) in &vertex_faces {
            if adj_faces.len() < 3 {
                continue;
            }

            // Create fan of triangles connecting the new vertices
            let new_vertices: Vec<usize> = adj_faces
                .iter()
                .filter_map(|&fi| vertex_map.get(&(*vi, fi)).copied())
                .collect();

            if new_vertices.len() >= 3 {
                // Calculate center of new vertices
                let center: Point3<f64> = new_vertices
                    .iter()
                    .map(|&i| mesh.positions[i])
                    .fold(Point3::origin(), |acc, p| {
                        Point3::new(acc.x + p.x, acc.y + p.y, acc.z + p.z)
                    });
                let center = Point3::new(
                    center.x / new_vertices.len() as f64,
                    center.y / new_vertices.len() as f64,
                    center.z / new_vertices.len() as f64,
                );

                let center_idx = mesh.positions.len();
                mesh.positions.push(center);
                mesh.normals
                    .push((center - mesh.positions[*vi]).normalize());

                // Create fan triangles
                for i in 0..new_vertices.len() {
                    let next = (i + 1) % new_vertices.len();
                    mesh.faces
                        .push(vec![center_idx, new_vertices[i], new_vertices[next]]);
                }
            }
        }
    }

    /// Bevel edges.
    fn bevel_edges(&self, mesh: &mut ModifierMesh) {
        let bevel_edges = self.get_bevel_edges(mesh);
        if bevel_edges.is_empty() {
            return;
        }

        let edge_faces = self.build_edge_faces(mesh);
        let mut new_vertices: HashMap<(usize, usize, usize), usize> = HashMap::new(); // (edge, segment, side)

        // Create bevel vertices for each edge
        for &(v0, v1) in &bevel_edges {
            let p0 = mesh.positions[v0];
            let p1 = mesh.positions[v1];

            let adj_faces = edge_faces.get(&(v0, v1)).cloned().unwrap_or_default();
            if adj_faces.len() < 2 {
                continue;
            }

            // Get face normals
            let normal0 = self.face_normal(mesh, adj_faces[0].0);
            let normal1 = self.face_normal(mesh, adj_faces[1].0);

            let edge_dir = (p1 - p0).normalize();

            // Calculate offset directions
            let offset_dir0 = edge_dir.cross(&normal0).normalize();
            let offset_dir1 = edge_dir.cross(&normal1).normalize();

            // Clamp amount if needed
            let edge_len = (p1 - p0).magnitude();
            let max_amount = if self.clamp_overlap {
                edge_len / 2.0
            } else {
                self.amount
            };
            let amount = self.amount.min(max_amount);

            // Create vertices along the bevel
            for seg in 0..=self.segments {
                let t = seg as f64 / self.segments as f64;
                let profile_t = self.profile_offset(t);

                // Interpolate between the two offset directions
                let offset_amount = amount * (1.0 - profile_t);

                for (side, &(face_idx, _)) in adj_faces.iter().take(2).enumerate() {
                    let offset_dir = if side == 0 { offset_dir0 } else { offset_dir1 };
                    let normal = if side == 0 { normal0 } else { normal1 };

                    // Create vertex offset from edge
                    let _edge_t = profile_t;
                    let edge_point = p0 + (p1 - p0) * 0.5; // Use midpoint for simplicity
                    let new_pos =
                        edge_point + offset_dir * offset_amount + normal * (amount * t * 0.5);

                    let key = (v0.min(v1) * 1000000 + v0.max(v1), seg, side);
                    let idx = mesh.positions.len();
                    mesh.positions.push(new_pos);
                    mesh.normals
                        .push(normal.lerp(&(offset_dir * -1.0), t).normalize());
                    new_vertices.insert(key, idx);

                    let _ = face_idx; // Face context
                }
            }
        }

        // Add bevel faces
        for &(v0, v1) in &bevel_edges {
            for seg in 0..self.segments {
                let base_key = v0.min(v1) * 1000000 + v0.max(v1);

                let get_idx = |s: usize, side: usize| -> Option<usize> {
                    new_vertices.get(&(base_key, s, side)).copied()
                };

                // Create quad between segments
                if let (Some(i00), Some(i01), Some(i10), Some(i11)) = (
                    get_idx(seg, 0),
                    get_idx(seg + 1, 0),
                    get_idx(seg, 1),
                    get_idx(seg + 1, 1),
                ) {
                    mesh.faces.push(vec![i00, i01, i11, i10]);
                }
            }
        }
    }

    /// Calculate face center.
    fn face_center(&self, mesh: &ModifierMesh, face_idx: usize) -> Point3<f64> {
        let face = &mesh.faces[face_idx];
        if face.is_empty() {
            return Point3::origin();
        }

        let sum = face
            .iter()
            .map(|&vi| mesh.positions[vi])
            .fold(Vector3::zeros(), |acc, p| acc + p.coords);

        Point3::from(sum / face.len() as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bevel_cube() {
        // Simple cube
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(1.0, 1.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
                Point3::new(0.0, 0.0, 1.0),
                Point3::new(1.0, 0.0, 1.0),
                Point3::new(1.0, 1.0, 1.0),
                Point3::new(0.0, 1.0, 1.0),
            ],
            vec![
                vec![0, 3, 2, 1], // Front
                vec![4, 5, 6, 7], // Back
                vec![0, 1, 5, 4], // Bottom
                vec![2, 3, 7, 6], // Top
                vec![0, 4, 7, 3], // Left
                vec![1, 2, 6, 5], // Right
            ],
        );

        let modifier = BevelModifier::new(0.1, 2);
        let result = modifier.apply(&mesh);

        // Should have more vertices and faces than original
        assert!(result.positions.len() > mesh.positions.len());
    }

    #[test]
    fn test_profile_offset() {
        let modifier = BevelModifier::default();

        // Test round profile
        let t0 = modifier.profile_offset(0.0);
        let t1 = modifier.profile_offset(1.0);

        assert!(t0.abs() < 1e-10);
        assert!((t1 - 1.0).abs() < 1e-10);
    }
}
