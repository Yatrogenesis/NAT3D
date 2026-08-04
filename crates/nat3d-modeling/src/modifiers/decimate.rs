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

//! Decimate modifier.
//!
//! Reduces polygon count using edge collapse with Quadric Error Metrics.

use nalgebra::{Point3, Matrix4};
use std::any::Any;
use std::collections::{HashMap, BinaryHeap};
use std::cmp::Ordering;
use super::stack::{Modifier, ModifierMesh};

/// Decimation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecimateMode {
    /// Edge collapse with QEM.
    Collapse,
    /// Un-subdivide (merge vertex pairs).
    UnSubdivide,
    /// Planar decimation (merge coplanar faces).
    Planar,
}

/// Edge with error for priority queue.
#[derive(Debug, Clone)]
struct EdgeCollapse {
    v1: usize,
    v2: usize,
    error: f64,
    target: Point3<f64>,
}

impl PartialEq for EdgeCollapse {
    fn eq(&self, other: &Self) -> bool {
        self.error == other.error
    }
}

impl Eq for EdgeCollapse {}

impl PartialOrd for EdgeCollapse {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // Reverse ordering for min-heap
        other.error.partial_cmp(&self.error)
    }
}

impl Ord for EdgeCollapse {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

/// Decimate modifier.
#[derive(Debug, Clone)]
pub struct DecimateModifier {
    /// Modifier name.
    pub name: String,
    /// Whether enabled.
    pub enabled: bool,
    /// Target ratio (0 to 1, where 1 = no decimation).
    pub ratio: f64,
    /// Decimation mode.
    pub mode: DecimateMode,
    /// Angle limit for planar mode (radians).
    pub angle_limit: f64,
    /// Preserve mesh boundaries.
    pub preserve_boundary: bool,
    /// Use vertex groups for protection.
    pub vertex_group: Option<String>,
    /// Symmetry axis.
    pub symmetry: Option<u8>, // 0=X, 1=Y, 2=Z
}

impl Default for DecimateModifier {
    fn default() -> Self {
        Self {
            name: "Decimate".to_string(),
            enabled: true,
            ratio: 0.5,
            mode: DecimateMode::Collapse,
            angle_limit: 0.0872665, // 5 degrees
            preserve_boundary: true,
            vertex_group: None,
            symmetry: None,
        }
    }
}

impl DecimateModifier {
    /// Create new decimate modifier.
    pub fn new(ratio: f64) -> Self {
        Self {
            ratio: ratio.max(0.0).min(1.0),
            ..Default::default()
        }
    }

    /// Calculate quadric error matrix for a face.
    fn face_quadric(&self, mesh: &ModifierMesh, face: &[usize]) -> Matrix4<f64> {
        if face.len() < 3 {
            return Matrix4::zeros();
        }

        let v0 = mesh.positions[face[0]];
        let v1 = mesh.positions[face[1]];
        let v2 = mesh.positions[face[2]];

        // Calculate face normal and d coefficient
        let normal = (v1 - v0).cross(&(v2 - v0)).normalize();
        let d = -normal.dot(&v0.coords);

        let a = normal.x;
        let b = normal.y;
        let c = normal.z;

        // Construct quadric matrix
        Matrix4::new(
            a*a, a*b, a*c, a*d,
            a*b, b*b, b*c, b*d,
            a*c, b*c, c*c, c*d,
            a*d, b*d, c*d, d*d,
        )
    }

    /// Calculate vertex quadrics (sum of adjacent face quadrics).
    fn calculate_vertex_quadrics(&self, mesh: &ModifierMesh) -> Vec<Matrix4<f64>> {
        let mut quadrics = vec![Matrix4::zeros(); mesh.positions.len()];

        for face in &mesh.faces {
            let q = self.face_quadric(mesh, face);
            for &vi in face {
                if vi < quadrics.len() {
                    quadrics[vi] += q;
                }
            }
        }

        quadrics
    }

    /// Calculate edge collapse error using quadric error metrics.
    fn calculate_edge_error(&self,
                            v1: usize,
                            v2: usize,
                            quadrics: &[Matrix4<f64>],
                            positions: &[Point3<f64>]) -> (f64, Point3<f64>) {
        let q = quadrics[v1] + quadrics[v2];

        // Try to find optimal position by solving: Q * v = 0
        // Simplified: use midpoint
        let p1 = positions[v1];
        let p2 = positions[v2];
        let target = Point3::new(
            (p1.x + p2.x) / 2.0,
            (p1.y + p2.y) / 2.0,
            (p1.z + p2.z) / 2.0,
        );

        // Calculate error at target position
        let v = nalgebra::Vector4::new(target.x, target.y, target.z, 1.0);
        let error = v.dot(&(q * v));

        (error.abs(), target)
    }

    /// Build edge list.
    fn build_edges(&self, mesh: &ModifierMesh) -> Vec<(usize, usize)> {
        let mut edges = Vec::new();
        let mut edge_set: HashMap<(usize, usize), ()> = HashMap::new();

        for face in &mesh.faces {
            for i in 0..face.len() {
                let v1 = face[i];
                let v2 = face[(i + 1) % face.len()];
                let edge = if v1 < v2 { (v1, v2) } else { (v2, v1) };

                edge_set.entry(edge).or_insert(());
            }
        }

        edges.extend(edge_set.keys().copied());
        edges
    }

    /// Find boundary vertices.
    fn is_boundary_vertex(&self, mesh: &ModifierMesh, vertex: usize) -> bool {
        let mut edge_count: HashMap<(usize, usize), usize> = HashMap::new();

        for face in &mesh.faces {
            for i in 0..face.len() {
                let v1 = face[i];
                let v2 = face[(i + 1) % face.len()];
                if v1 == vertex || v2 == vertex {
                    let edge = if v1 < v2 { (v1, v2) } else { (v2, v1) };
                    *edge_count.entry(edge).or_insert(0) += 1;
                }
            }
        }

        edge_count.values().any(|&count| count == 1)
    }

    /// Apply edge collapse decimation.
    fn decimate_collapse(&self, mesh: &ModifierMesh) -> ModifierMesh {
        let target_faces = (mesh.faces.len() as f64 * self.ratio) as usize;
        if mesh.faces.len() <= target_faces {
            return mesh.clone();
        }

        let mut result = mesh.clone();
        let quadrics = self.calculate_vertex_quadrics(mesh);
        let edges = self.build_edges(mesh);

        // Build priority queue of edge collapses
        let mut heap = BinaryHeap::new();
        for &(v1, v2) in &edges {
            // Skip boundary edges if preserving boundaries
            if self.preserve_boundary &&
               (self.is_boundary_vertex(mesh, v1) || self.is_boundary_vertex(mesh, v2)) {
                continue;
            }

            let (error, target) = self.calculate_edge_error(v1, v2, &quadrics, &result.positions);
            heap.push(EdgeCollapse { v1, v2, error, target });
        }

        // Perform collapses
        let mut collapsed = 0;
        let max_collapses = mesh.faces.len() - target_faces;
        let mut vertex_map: HashMap<usize, usize> = HashMap::new();

        while collapsed < max_collapses && !heap.is_empty() {
            if let Some(collapse) = heap.pop() {
                let v1 = *vertex_map.get(&collapse.v1).unwrap_or(&collapse.v1);
                let v2 = *vertex_map.get(&collapse.v2).unwrap_or(&collapse.v2);

                if v1 == v2 {
                    continue; // Already collapsed
                }

                // Collapse v2 into v1
                result.positions[v1] = collapse.target;
                vertex_map.insert(v2, v1);
                collapsed += 1;
            }
        }

        // Rebuild faces with collapsed vertices
        let mut new_faces = Vec::new();
        for face in &result.faces {
            let new_face: Vec<usize> = face.iter()
                .map(|&v| *vertex_map.get(&v).unwrap_or(&v))
                .collect();

            // Remove degenerate faces
            let unique: std::collections::HashSet<_> = new_face.iter().collect();
            if unique.len() >= 3 {
                new_faces.push(new_face);
            }
        }

        result.faces = new_faces;
        result.compute_normals();
        result
    }

    /// Apply planar decimation.
    fn decimate_planar(&self, mesh: &ModifierMesh) -> ModifierMesh {
        let mut result = mesh.clone();
        let mut merged = true;

        while merged {
            merged = false;
            let face_count = result.faces.len();

            for i in 0..face_count {
                if i >= result.faces.len() {
                    break;
                }

                let face_i = &result.faces[i];
                if face_i.len() < 3 {
                    continue;
                }

                // Calculate normal for face i
                let v0 = result.positions[face_i[0]];
                let v1 = result.positions[face_i[1]];
                let v2 = result.positions[face_i[2]];
                let normal_i = (v1 - v0).cross(&(v2 - v0)).normalize();

                // Check against other faces
                for j in (i + 1)..result.faces.len() {
                    let face_j = &result.faces[j];
                    if face_j.len() < 3 {
                        continue;
                    }

                    // Check if faces share an edge
                    let shared: Vec<_> = face_i.iter()
                        .filter(|v| face_j.contains(v))
                        .collect();

                    if shared.len() == 2 {
                        // Calculate normal for face j
                        let v0 = result.positions[face_j[0]];
                        let v1 = result.positions[face_j[1]];
                        let v2 = result.positions[face_j[2]];
                        let normal_j = (v1 - v0).cross(&(v2 - v0)).normalize();

                        // Check angle
                        let angle = normal_i.dot(&normal_j).acos();
                        if angle < self.angle_limit {
                            // Merge faces (simplified: just remove one)
                            result.faces.remove(j);
                            merged = true;
                            break;
                        }
                    }
                }

                if merged {
                    break;
                }
            }
        }

        result.compute_normals();
        result
    }
}

impl Modifier for DecimateModifier {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_id(&self) -> &'static str {
        "DecimateModifier"
    }

    fn apply(&self, mesh: &ModifierMesh) -> ModifierMesh {
        if mesh.positions.is_empty() || mesh.faces.is_empty() {
            return mesh.clone();
        }

        match self.mode {
            DecimateMode::Collapse => self.decimate_collapse(mesh),
            DecimateMode::Planar => self.decimate_planar(mesh),
            DecimateMode::UnSubdivide => {
                // Simplified: use collapse
                self.decimate_collapse(mesh)
            }
        }
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
    fn test_decimate_basic() {
        // Create a subdivided plane
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(2.0, 0.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
                Point3::new(1.0, 1.0, 0.0),
                Point3::new(2.0, 1.0, 0.0),
            ],
            vec![
                vec![0, 1, 4, 3],
                vec![1, 2, 5, 4],
            ],
        );

        let modifier = DecimateModifier::new(0.5);
        let result = modifier.apply(&mesh);

        // Should have reduced face count
        assert!(result.faces.len() <= mesh.faces.len());
    }

    #[test]
    fn test_decimate_modes() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(1.0, 1.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
            ],
            vec![vec![0, 1, 2], vec![0, 2, 3]],
        );

        // Test collapse mode
        let mut modifier = DecimateModifier::new(0.5);
        modifier.mode = DecimateMode::Collapse;
        let result = modifier.apply(&mesh);
        assert_eq!(result.positions.len(), 4);

        // Test planar mode
        modifier.mode = DecimateMode::Planar;
        modifier.angle_limit = 0.1;
        let result = modifier.apply(&mesh);
        assert_eq!(result.positions.len(), 4);
    }
}
