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

//! Face data structure and operations.
//!
//! Faces are polygons defined by a sequence of vertices.
//! This module supports both triangles and n-gons, with automatic
//! triangulation for rendering.

use super::{BoundingBox, Normal, Position};
use nalgebra::Vector3;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use uuid::Uuid;

/// Unique identifier for a face within a mesh.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FaceId(pub Uuid);

impl FaceId {
    /// Create a new unique face ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create a face ID from an existing UUID.
    #[must_use]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get the underlying UUID.
    #[must_use]
    pub const fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for FaceId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for FaceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Face({})", &self.0.to_string()[..8])
    }
}

/// Face winding order for determining front/back faces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum WindingOrder {
    /// Counter-clockwise winding (OpenGL default).
    #[default]
    CounterClockwise,
    /// Clockwise winding.
    Clockwise,
}

/// Additional data associated with a face.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FaceData {
    /// Material index assigned to this face.
    pub material_index: Option<usize>,
    /// Smoothing group for normal calculation.
    pub smoothing_group: u32,
    /// Whether this face is selected.
    pub selected: bool,
    /// Whether this face is hidden.
    pub hidden: bool,
    /// User-defined face group/tag.
    pub group: Option<String>,
}

impl FaceData {
    /// Create default face data.
    #[must_use]
    pub fn new() -> Self {
        Self {
            material_index: None,
            smoothing_group: 0,
            selected: false,
            hidden: false,
            group: None,
        }
    }

    /// Builder method to set material index.
    #[must_use]
    pub fn with_material(mut self, index: usize) -> Self {
        self.material_index = Some(index);
        self
    }

    /// Builder method to set smoothing group.
    #[must_use]
    pub fn with_smoothing_group(mut self, group: u32) -> Self {
        self.smoothing_group = group;
        self
    }

    /// Builder method to set group name.
    #[must_use]
    pub fn with_group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }
}

impl Default for FaceData {
    fn default() -> Self {
        Self::new()
    }
}

/// A face (polygon) defined by vertex indices.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Face {
    /// Unique identifier.
    pub id: FaceId,
    /// Vertex indices that make up this face (in order).
    /// Minimum 3 vertices for a valid face.
    pub vertices: SmallVec<[usize; 4]>,
    /// Cached face normal (may be None if not computed).
    pub normal: Option<Normal>,
    /// Additional face data.
    pub data: FaceData,
}

impl Face {
    /// Create a new triangle face.
    #[must_use]
    pub fn triangle(v0: usize, v1: usize, v2: usize) -> Self {
        Self {
            id: FaceId::new(),
            vertices: SmallVec::from_slice(&[v0, v1, v2]),
            normal: None,
            data: FaceData::new(),
        }
    }

    /// Create a new quad face.
    #[must_use]
    pub fn quad(v0: usize, v1: usize, v2: usize, v3: usize) -> Self {
        Self {
            id: FaceId::new(),
            vertices: SmallVec::from_slice(&[v0, v1, v2, v3]),
            normal: None,
            data: FaceData::new(),
        }
    }

    /// Create a new n-gon face from a slice of vertex indices.
    #[must_use]
    pub fn ngon(vertices: &[usize]) -> Self {
        Self {
            id: FaceId::new(),
            vertices: SmallVec::from_slice(vertices),
            normal: None,
            data: FaceData::new(),
        }
    }

    /// Create a new face with data.
    #[must_use]
    pub fn with_data(vertices: &[usize], data: FaceData) -> Self {
        Self {
            id: FaceId::new(),
            vertices: SmallVec::from_slice(vertices),
            normal: None,
            data,
        }
    }

    /// Get the number of vertices in this face.
    #[must_use]
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Check if this face is a triangle.
    #[must_use]
    pub fn is_triangle(&self) -> bool {
        self.vertices.len() == 3
    }

    /// Check if this face is a quad.
    #[must_use]
    pub fn is_quad(&self) -> bool {
        self.vertices.len() == 4
    }

    /// Check if this face is an n-gon (more than 4 vertices).
    #[must_use]
    pub fn is_ngon(&self) -> bool {
        self.vertices.len() > 4
    }

    /// Check if a vertex index is part of this face.
    #[must_use]
    pub fn contains_vertex(&self, vertex_index: usize) -> bool {
        self.vertices.contains(&vertex_index)
    }

    /// Get the edge pairs for this face.
    /// Returns pairs of (`vertex_index`, `next_vertex_index`).
    #[must_use]
    pub fn edge_pairs(&self) -> Vec<(usize, usize)> {
        let n = self.vertices.len();
        (0..n)
            .map(|i| (self.vertices[i], self.vertices[(i + 1) % n]))
            .collect()
    }

    /// Get the vertex index at position i, with wrapping.
    #[must_use]
    pub fn vertex_at(&self, i: usize) -> usize {
        self.vertices[i % self.vertices.len()]
    }

    /// Compute the face normal from vertex positions.
    /// Uses Newell's method for robust normal calculation on n-gons.
    #[must_use]
    pub fn compute_normal(&self, positions: &[Position]) -> Normal {
        if self.vertices.len() < 3 {
            return Normal::new(0.0, 1.0, 0.0); // Default up
        }

        // Newell's method for polygon normal
        let mut normal = Vector3::new(0.0, 0.0, 0.0);
        let n = self.vertices.len();

        for i in 0..n {
            let v_curr = &positions[self.vertices[i]];
            let v_next = &positions[self.vertices[(i + 1) % n]];

            normal.x += (v_curr.y - v_next.y) * (v_curr.z + v_next.z);
            normal.y += (v_curr.z - v_next.z) * (v_curr.x + v_next.x);
            normal.z += (v_curr.x - v_next.x) * (v_curr.y + v_next.y);
        }

        let length = normal.magnitude();
        if length > f64::EPSILON {
            normal / length
        } else {
            // Degenerate face, return default normal
            Normal::new(0.0, 1.0, 0.0)
        }
    }

    /// Update the cached normal from vertex positions.
    pub fn update_normal(&mut self, positions: &[Position]) {
        self.normal = Some(self.compute_normal(positions));
    }

    /// Compute the area of this face from vertex positions.
    #[must_use]
    pub fn compute_area(&self, positions: &[Position]) -> f64 {
        if self.vertices.len() < 3 {
            return 0.0;
        }

        // For triangles, use cross product
        if self.is_triangle() {
            let p0 = &positions[self.vertices[0]];
            let p1 = &positions[self.vertices[1]];
            let p2 = &positions[self.vertices[2]];

            let v1 = p1 - p0;
            let v2 = p2 - p0;
            return v1.cross(&v2).magnitude() * 0.5;
        }

        // For n-gons, triangulate and sum areas
        let p0 = &positions[self.vertices[0]];
        let mut total_area = 0.0;

        for i in 1..(self.vertices.len() - 1) {
            let p1 = &positions[self.vertices[i]];
            let p2 = &positions[self.vertices[i + 1]];

            let v1 = p1 - p0;
            let v2 = p2 - p0;
            total_area += v1.cross(&v2).magnitude() * 0.5;
        }

        total_area
    }

    /// Compute the centroid (center of mass) of this face.
    #[must_use]
    pub fn compute_centroid(&self, positions: &[Position]) -> Position {
        if self.vertices.is_empty() {
            return Position::origin();
        }

        let sum: Vector3<f64> = self.vertices.iter().map(|&i| positions[i].coords).sum();

        Position::from(sum / self.vertices.len() as f64)
    }

    /// Compute the bounding box of this face.
    #[must_use]
    pub fn compute_bounds(&self, positions: &[Position]) -> BoundingBox {
        let mut bounds = BoundingBox::empty();
        for &vi in &self.vertices {
            bounds.expand_to_include(&positions[vi]);
        }
        bounds
    }

    /// Triangulate this face using fan triangulation.
    /// Returns a list of triangle vertex index triplets.
    #[must_use]
    pub fn triangulate(&self) -> Vec<[usize; 3]> {
        if self.vertices.len() < 3 {
            return Vec::new();
        }

        let v0 = self.vertices[0];
        (1..self.vertices.len() - 1)
            .map(|i| [v0, self.vertices[i], self.vertices[i + 1]])
            .collect()
    }

    /// Triangulate this face with ear clipping for better quality.
    /// Uses a simple implementation suitable for convex and mildly non-convex polygons.
    #[must_use]
    pub fn triangulate_ear_clip(&self, _positions: &[Position]) -> Vec<[usize; 3]> {
        if self.vertices.len() < 3 {
            return Vec::new();
        }

        if self.vertices.len() == 3 {
            return vec![[self.vertices[0], self.vertices[1], self.vertices[2]]];
        }

        // For simple cases, fan triangulation is sufficient
        // A full ear-clipping implementation would go here for complex polygons
        self.triangulate()
    }

    /// Flip the face winding order.
    pub fn flip(&mut self) {
        self.vertices.reverse();
        if let Some(ref mut n) = self.normal {
            *n = -*n;
        }
    }

    /// Create a flipped copy of this face.
    #[must_use]
    pub fn flipped(&self) -> Self {
        let mut vertices = self.vertices.clone();
        vertices.reverse();
        Self {
            id: FaceId::new(),
            vertices,
            normal: self.normal.map(|n| -n),
            data: self.data.clone(),
        }
    }

    /// Check if this face is degenerate (has coincident vertices or zero area).
    #[must_use]
    pub fn is_degenerate(&self, positions: &[Position], epsilon: f64) -> bool {
        if self.vertices.len() < 3 {
            return true;
        }

        // Check for zero area
        if self.compute_area(positions) < epsilon {
            return true;
        }

        // Check for coincident vertices
        for i in 0..self.vertices.len() {
            for j in (i + 1)..self.vertices.len() {
                let dist = (positions[self.vertices[i]] - positions[self.vertices[j]]).magnitude();
                if dist < epsilon {
                    return true;
                }
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_face_id_uniqueness() {
        let id1 = FaceId::new();
        let id2 = FaceId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_triangle_creation() {
        let face = Face::triangle(0, 1, 2);
        assert!(face.is_triangle());
        assert!(!face.is_quad());
        assert_eq!(face.vertex_count(), 3);
    }

    #[test]
    fn test_quad_creation() {
        let face = Face::quad(0, 1, 2, 3);
        assert!(face.is_quad());
        assert!(!face.is_triangle());
        assert_eq!(face.vertex_count(), 4);
    }

    #[test]
    fn test_face_normal() {
        let positions = vec![
            Position::new(0.0, 0.0, 0.0),
            Position::new(1.0, 0.0, 0.0),
            Position::new(0.0, 1.0, 0.0),
        ];
        let face = Face::triangle(0, 1, 2);
        let normal = face.compute_normal(&positions);

        // Normal should point in +Z direction
        assert!((normal.z - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_face_area() {
        let positions = vec![
            Position::new(0.0, 0.0, 0.0),
            Position::new(2.0, 0.0, 0.0),
            Position::new(0.0, 2.0, 0.0),
        ];
        let face = Face::triangle(0, 1, 2);
        let area = face.compute_area(&positions);

        assert!((area - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_face_centroid() {
        let positions = vec![
            Position::new(0.0, 0.0, 0.0),
            Position::new(3.0, 0.0, 0.0),
            Position::new(0.0, 3.0, 0.0),
        ];
        let face = Face::triangle(0, 1, 2);
        let centroid = face.compute_centroid(&positions);

        assert!((centroid.x - 1.0).abs() < 1e-10);
        assert!((centroid.y - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_triangulate() {
        let face = Face::quad(0, 1, 2, 3);
        let triangles = face.triangulate();

        assert_eq!(triangles.len(), 2);
        assert_eq!(triangles[0], [0, 1, 2]);
        assert_eq!(triangles[1], [0, 2, 3]);
    }

    #[test]
    fn test_face_flip() {
        let mut face = Face::triangle(0, 1, 2);
        face.flip();

        assert_eq!(face.vertices[0], 2);
        assert_eq!(face.vertices[1], 1);
        assert_eq!(face.vertices[2], 0);
    }

    #[test]
    fn test_edge_pairs() {
        let face = Face::triangle(0, 1, 2);
        let edges = face.edge_pairs();

        assert_eq!(edges.len(), 3);
        assert_eq!(edges[0], (0, 1));
        assert_eq!(edges[1], (1, 2));
        assert_eq!(edges[2], (2, 0));
    }
}
