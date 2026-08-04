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

//! Triangulate modifier.
//!
//! Converts all polygons to triangles using various algorithms.

use nalgebra::Point3;
use std::any::Any;
use super::stack::{Modifier, ModifierMesh};

/// Triangulation method for quads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum TriangulateMethod {
    /// Beauty fill (minimize overall distortion).
    #[default]
    Beauty,
    /// Fixed diagonal pattern.
    Fixed,
    /// Shortest diagonal.
    ShortestDiagonal,
    /// Longest diagonal.
    LongestDiagonal,
}


/// N-gon triangulation method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum NgonMethod {
    /// Triangle fan from first vertex.
    Fan,
    /// Ear clipping algorithm.
    #[default]
    Ear,
}


/// Triangulate modifier.
#[derive(Debug, Clone)]
pub struct TriangulateModifier {
    /// Modifier name.
    pub name: String,
    /// Whether enabled.
    pub enabled: bool,
    /// Triangulation method for quads.
    pub method: TriangulateMethod,
    /// Try to preserve normals.
    pub keep_normals: bool,
    /// N-gon triangulation method.
    pub ngon_method: NgonMethod,
}

impl Default for TriangulateModifier {
    fn default() -> Self {
        Self {
            name: "Triangulate".to_string(),
            enabled: true,
            method: TriangulateMethod::default(),
            keep_normals: true,
            ngon_method: NgonMethod::default(),
        }
    }
}

impl TriangulateModifier {
    /// Create new triangulate modifier.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with specific method.
    pub fn with_method(method: TriangulateMethod) -> Self {
        Self {
            method,
            ..Default::default()
        }
    }

    /// Triangulate a quad face.
    fn triangulate_quad(&self, face: &[usize], mesh: &ModifierMesh) -> Vec<Vec<usize>> {
        if face.len() != 4 {
            return vec![face.to_vec()];
        }

        let v0 = mesh.positions[face[0]];
        let v1 = mesh.positions[face[1]];
        let v2 = mesh.positions[face[2]];
        let v3 = mesh.positions[face[3]];

        match self.method {
            TriangulateMethod::Fixed => {
                // Always use 0-2 diagonal
                vec![
                    vec![face[0], face[1], face[2]],
                    vec![face[0], face[2], face[3]],
                ]
            }
            TriangulateMethod::ShortestDiagonal => {
                let diag_02 = (v2 - v0).magnitude();
                let diag_13 = (v3 - v1).magnitude();

                if diag_02 < diag_13 {
                    vec![
                        vec![face[0], face[1], face[2]],
                        vec![face[0], face[2], face[3]],
                    ]
                } else {
                    vec![
                        vec![face[0], face[1], face[3]],
                        vec![face[1], face[2], face[3]],
                    ]
                }
            }
            TriangulateMethod::LongestDiagonal => {
                let diag_02 = (v2 - v0).magnitude();
                let diag_13 = (v3 - v1).magnitude();

                if diag_02 > diag_13 {
                    vec![
                        vec![face[0], face[1], face[2]],
                        vec![face[0], face[2], face[3]],
                    ]
                } else {
                    vec![
                        vec![face[0], face[1], face[3]],
                        vec![face[1], face[2], face[3]],
                    ]
                }
            }
            TriangulateMethod::Beauty => {
                // Choose diagonal that creates most equilateral triangles
                let score_02 = self.beauty_score(&[v0, v1, v2]) + self.beauty_score(&[v0, v2, v3]);
                let score_13 = self.beauty_score(&[v0, v1, v3]) + self.beauty_score(&[v1, v2, v3]);

                if score_02 > score_13 {
                    vec![
                        vec![face[0], face[1], face[2]],
                        vec![face[0], face[2], face[3]],
                    ]
                } else {
                    vec![
                        vec![face[0], face[1], face[3]],
                        vec![face[1], face[2], face[3]],
                    ]
                }
            }
        }
    }

    /// Beauty score (higher is better, based on angles).
    fn beauty_score(&self, tri: &[Point3<f64>]) -> f64 {
        if tri.len() != 3 {
            return 0.0;
        }

        let v0 = tri[0];
        let v1 = tri[1];
        let v2 = tri[2];

        let e0 = (v1 - v0).magnitude();
        let e1 = (v2 - v1).magnitude();
        let e2 = (v0 - v2).magnitude();

        if e0 < 1e-10 || e1 < 1e-10 || e2 < 1e-10 {
            return 0.0;
        }

        // Score based on how close to equilateral (equal edge lengths)
        let avg = (e0 + e1 + e2) / 3.0;
        let variance = ((e0 - avg).powi(2) + (e1 - avg).powi(2) + (e2 - avg).powi(2)) / 3.0;

        // Lower variance = better (more equilateral)
        1.0 / (1.0 + variance)
    }

    /// Triangulate n-gon using ear clipping.
    fn triangulate_ear_clipping(&self, face: &[usize], mesh: &ModifierMesh) -> Vec<Vec<usize>> {
        if face.len() < 3 {
            return vec![];
        }
        if face.len() == 3 {
            return vec![face.to_vec()];
        }

        let mut result = Vec::new();
        let mut remaining: Vec<usize> = face.to_vec();

        while remaining.len() > 3 {
            let mut ear_found = false;

            for i in 0..remaining.len() {
                let prev = remaining[(i + remaining.len() - 1) % remaining.len()];
                let curr = remaining[i];
                let next = remaining[(i + 1) % remaining.len()];

                // Check if this is an ear (convex and no other vertices inside)
                if self.is_ear(prev, curr, next, &remaining, mesh) {
                    result.push(vec![prev, curr, next]);
                    remaining.remove(i);
                    ear_found = true;
                    break;
                }
            }

            if !ear_found {
                // Fallback to fan if ear clipping fails
                return self.triangulate_fan(face);
            }
        }

        if remaining.len() == 3 {
            result.push(remaining);
        }

        result
    }

    /// Check if a vertex forms an ear.
    fn is_ear(&self, prev: usize, curr: usize, next: usize, vertices: &[usize], mesh: &ModifierMesh) -> bool {
        let v_prev = mesh.positions[prev];
        let v_curr = mesh.positions[curr];
        let v_next = mesh.positions[next];

        // Check if triangle is convex (cross product points outward)
        let e1 = v_curr - v_prev;
        let e2 = v_next - v_curr;
        let cross = e1.cross(&e2);

        if cross.z < 0.0 {
            return false; // Concave
        }

        // Check if any other vertex is inside this triangle
        for &vi in vertices {
            if vi == prev || vi == curr || vi == next {
                continue;
            }

            if self.point_in_triangle(mesh.positions[vi], v_prev, v_curr, v_next) {
                return false;
            }
        }

        true
    }

    /// Check if point is inside triangle (2D projection on XY plane).
    fn point_in_triangle(&self, p: Point3<f64>, v0: Point3<f64>, v1: Point3<f64>, v2: Point3<f64>) -> bool {
        // Barycentric coordinates
        let denom = (v1.y - v2.y) * (v0.x - v2.x) + (v2.x - v1.x) * (v0.y - v2.y);
        if denom.abs() < 1e-10 {
            return false;
        }

        let a = ((v1.y - v2.y) * (p.x - v2.x) + (v2.x - v1.x) * (p.y - v2.y)) / denom;
        let b = ((v2.y - v0.y) * (p.x - v2.x) + (v0.x - v2.x) * (p.y - v2.y)) / denom;
        let c = 1.0 - a - b;

        (0.0..=1.0).contains(&a) && (0.0..=1.0).contains(&b) && (0.0..=1.0).contains(&c)
    }

    /// Triangulate n-gon using fan triangulation.
    fn triangulate_fan(&self, face: &[usize]) -> Vec<Vec<usize>> {
        if face.len() < 3 {
            return vec![];
        }
        if face.len() == 3 {
            return vec![face.to_vec()];
        }

        let mut result = Vec::new();
        for i in 1..face.len() - 1 {
            result.push(vec![face[0], face[i], face[i + 1]]);
        }
        result
    }
}

impl Modifier for TriangulateModifier {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_id(&self) -> &'static str {
        "TriangulateModifier"
    }

    fn apply(&self, mesh: &ModifierMesh) -> ModifierMesh {
        let mut result = ModifierMesh::new();

        // Copy vertices and normals
        result.positions = mesh.positions.clone();
        result.normals = mesh.normals.clone();
        result.uvs = mesh.uvs.clone();
        result.vertex_groups = mesh.vertex_groups.clone();
        result.attributes = mesh.attributes.clone();

        // Triangulate each face
        for face in &mesh.faces {
            let triangles = match face.len() {
                0..=2 => vec![],
                3 => vec![face.to_vec()],
                4 => self.triangulate_quad(face, mesh),
                _ => {
                    // N-gon
                    match self.ngon_method {
                        NgonMethod::Fan => self.triangulate_fan(face),
                        NgonMethod::Ear => self.triangulate_ear_clipping(face, mesh),
                    }
                }
            };

            result.faces.extend(triangles);
        }

        if !self.keep_normals {
            result.compute_normals();
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
    fn test_triangulate_quad() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(1.0, 1.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
            ],
            vec![vec![0, 1, 2, 3]],
        );

        let modifier = TriangulateModifier::new();
        let result = modifier.apply(&mesh);

        assert_eq!(result.faces.len(), 2);
        for face in &result.faces {
            assert_eq!(face.len(), 3);
        }
    }

    #[test]
    fn test_triangulate_already_triangles() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.5, 1.0, 0.0),
            ],
            vec![vec![0, 1, 2]],
        );

        let modifier = TriangulateModifier::new();
        let result = modifier.apply(&mesh);

        // Should remain unchanged
        assert_eq!(result.faces.len(), 1);
        assert_eq!(result.faces[0].len(), 3);
    }

    #[test]
    fn test_triangulate_ngon() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(1.5, 0.5, 0.0),
                Point3::new(1.0, 1.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
            ],
            vec![vec![0, 1, 2, 3, 4]],
        );

        let modifier = TriangulateModifier::new();
        let result = modifier.apply(&mesh);

        // Pentagon should become 3 triangles
        assert_eq!(result.faces.len(), 3);
        for face in &result.faces {
            assert_eq!(face.len(), 3);
        }
    }
}
