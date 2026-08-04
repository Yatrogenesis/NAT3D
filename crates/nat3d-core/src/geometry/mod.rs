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

//! Geometry module for NAT3D.
//!
//! This module provides all geometric primitives and mesh operations:
//! - Vertices, edges, and faces
//! - Mesh data structure with half-edge topology
//! - Primitive generation (cube, sphere, cylinder, etc.)
//! - Bounding boxes and spatial queries

pub mod edge;
pub mod face;
pub mod mesh;
pub mod primitives;
pub mod vertex;

use nalgebra::{Point3, Vector3};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Re-exports
pub use edge::{Edge, EdgeData, EdgeId, EdgeKey};
pub use face::{Face, FaceData, FaceId};
pub use mesh::{Mesh, MeshData, MeshId, MeshTopology};
pub use primitives::{Primitive, PrimitiveParams};
pub use vertex::{Vertex, VertexData, VertexId};

/// Type alias for 3D position.
pub type Position = Point3<f64>;

/// Type alias for 3D normal vector.
pub type Normal = Vector3<f64>;

/// Type alias for 2D texture coordinate.
pub type TexCoord = nalgebra::Point2<f64>;

/// Axis-aligned bounding box in 3D space.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoundingBox {
    /// Minimum corner of the bounding box.
    pub min: Position,
    /// Maximum corner of the bounding box.
    pub max: Position,
}

impl BoundingBox {
    /// Create a new bounding box from min and max points.
    ///
    /// # Arguments
    /// * `min` - Minimum corner
    /// * `max` - Maximum corner
    ///
    /// # Panics
    /// Panics if min > max in any dimension (in debug builds).
    #[must_use]
    pub fn new(min: Position, max: Position) -> Self {
        debug_assert!(
            min.x <= max.x && min.y <= max.y && min.z <= max.z,
            "BoundingBox min must be <= max in all dimensions"
        );
        Self { min, max }
    }

    /// Create an empty (inverted) bounding box suitable for expansion.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            min: Position::new(f64::INFINITY, f64::INFINITY, f64::INFINITY),
            max: Position::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY),
        }
    }

    /// Create a bounding box centered at origin with given half-extents.
    #[must_use]
    pub fn from_half_extents(half_extents: Vector3<f64>) -> Self {
        Self {
            min: Position::new(-half_extents.x, -half_extents.y, -half_extents.z),
            max: Position::new(half_extents.x, half_extents.y, half_extents.z),
        }
    }

    /// Create a bounding box from a center point and size.
    #[must_use]
    pub fn from_center_size(center: Position, size: Vector3<f64>) -> Self {
        let half = size * 0.5;
        Self {
            min: Position::new(center.x - half.x, center.y - half.y, center.z - half.z),
            max: Position::new(center.x + half.x, center.y + half.y, center.z + half.z),
        }
    }

    /// Expand this bounding box to include a point.
    pub fn expand_to_include(&mut self, point: &Position) {
        self.min.x = self.min.x.min(point.x);
        self.min.y = self.min.y.min(point.y);
        self.min.z = self.min.z.min(point.z);
        self.max.x = self.max.x.max(point.x);
        self.max.y = self.max.y.max(point.y);
        self.max.z = self.max.z.max(point.z);
    }

    /// Expand this bounding box to include another bounding box.
    pub fn expand_to_include_box(&mut self, other: &BoundingBox) {
        self.expand_to_include(&other.min);
        self.expand_to_include(&other.max);
    }

    /// Get the center of the bounding box.
    #[must_use]
    pub fn center(&self) -> Position {
        Position::new(
            (self.min.x + self.max.x) * 0.5,
            (self.min.y + self.max.y) * 0.5,
            (self.min.z + self.max.z) * 0.5,
        )
    }

    /// Get the size (extents) of the bounding box.
    #[must_use]
    pub fn size(&self) -> Vector3<f64> {
        Vector3::new(
            self.max.x - self.min.x,
            self.max.y - self.min.y,
            self.max.z - self.min.z,
        )
    }

    /// Get the half-extents of the bounding box.
    #[must_use]
    pub fn half_extents(&self) -> Vector3<f64> {
        self.size() * 0.5
    }

    /// Get the diagonal length of the bounding box.
    #[must_use]
    pub fn diagonal(&self) -> f64 {
        self.size().magnitude()
    }

    /// Get the volume of the bounding box.
    #[must_use]
    pub fn volume(&self) -> f64 {
        let s = self.size();
        s.x * s.y * s.z
    }

    /// Get the surface area of the bounding box.
    #[must_use]
    pub fn surface_area(&self) -> f64 {
        let s = self.size();
        2.0 * (s.x * s.y + s.y * s.z + s.z * s.x)
    }

    /// Check if the bounding box is valid (not empty/inverted).
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.min.x <= self.max.x && self.min.y <= self.max.y && self.min.z <= self.max.z
    }

    /// Check if a point is inside the bounding box.
    #[must_use]
    pub fn contains_point(&self, point: &Position) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
            && point.z >= self.min.z
            && point.z <= self.max.z
    }

    /// Check if this bounding box intersects another.
    #[must_use]
    pub fn intersects(&self, other: &BoundingBox) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
            && self.min.z <= other.max.z
            && self.max.z >= other.min.z
    }

    /// Compute the intersection of two bounding boxes.
    /// Returns None if they don't intersect.
    #[must_use]
    pub fn intersection(&self, other: &BoundingBox) -> Option<BoundingBox> {
        if !self.intersects(other) {
            return None;
        }

        Some(BoundingBox {
            min: Position::new(
                self.min.x.max(other.min.x),
                self.min.y.max(other.min.y),
                self.min.z.max(other.min.z),
            ),
            max: Position::new(
                self.max.x.min(other.max.x),
                self.max.y.min(other.max.y),
                self.max.z.min(other.max.z),
            ),
        })
    }

    /// Compute the union of two bounding boxes.
    #[must_use]
    pub fn union(&self, other: &BoundingBox) -> BoundingBox {
        BoundingBox {
            min: Position::new(
                self.min.x.min(other.min.x),
                self.min.y.min(other.min.y),
                self.min.z.min(other.min.z),
            ),
            max: Position::new(
                self.max.x.max(other.max.x),
                self.max.y.max(other.max.y),
                self.max.z.max(other.max.z),
            ),
        }
    }

    /// Transform this bounding box by a matrix.
    /// Note: This computes the AABB of the transformed corners,
    /// which may be larger than the tightest possible AABB.
    #[must_use]
    pub fn transform(&self, matrix: &nalgebra::Matrix4<f64>) -> BoundingBox {
        let corners = [
            Position::new(self.min.x, self.min.y, self.min.z),
            Position::new(self.max.x, self.min.y, self.min.z),
            Position::new(self.min.x, self.max.y, self.min.z),
            Position::new(self.max.x, self.max.y, self.min.z),
            Position::new(self.min.x, self.min.y, self.max.z),
            Position::new(self.max.x, self.min.y, self.max.z),
            Position::new(self.min.x, self.max.y, self.max.z),
            Position::new(self.max.x, self.max.y, self.max.z),
        ];

        let mut result = BoundingBox::empty();
        for corner in &corners {
            let transformed = matrix.transform_point(corner);
            result.expand_to_include(&transformed);
        }
        result
    }

    /// Get the 8 corners of the bounding box.
    #[must_use]
    pub fn corners(&self) -> [Position; 8] {
        [
            Position::new(self.min.x, self.min.y, self.min.z),
            Position::new(self.max.x, self.min.y, self.min.z),
            Position::new(self.min.x, self.max.y, self.min.z),
            Position::new(self.max.x, self.max.y, self.min.z),
            Position::new(self.min.x, self.min.y, self.max.z),
            Position::new(self.max.x, self.min.y, self.max.z),
            Position::new(self.min.x, self.max.y, self.max.z),
            Position::new(self.max.x, self.max.y, self.max.z),
        ]
    }
}

impl Default for BoundingBox {
    fn default() -> Self {
        Self::empty()
    }
}

/// Generate a new unique ID.
#[must_use]
pub fn new_id() -> Uuid {
    Uuid::new_v4()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bounding_box_creation() {
        let bb = BoundingBox::new(Position::new(0.0, 0.0, 0.0), Position::new(1.0, 1.0, 1.0));
        assert!(bb.is_valid());
        assert_eq!(bb.volume(), 1.0);
    }

    #[test]
    fn test_bounding_box_from_center_size() {
        let bb = BoundingBox::from_center_size(
            Position::new(0.0, 0.0, 0.0),
            Vector3::new(2.0, 2.0, 2.0),
        );
        assert_eq!(bb.min, Position::new(-1.0, -1.0, -1.0));
        assert_eq!(bb.max, Position::new(1.0, 1.0, 1.0));
    }

    #[test]
    fn test_bounding_box_contains() {
        let bb = BoundingBox::new(Position::new(0.0, 0.0, 0.0), Position::new(1.0, 1.0, 1.0));
        assert!(bb.contains_point(&Position::new(0.5, 0.5, 0.5)));
        assert!(!bb.contains_point(&Position::new(1.5, 0.5, 0.5)));
    }

    #[test]
    fn test_bounding_box_intersection() {
        let bb1 = BoundingBox::new(Position::new(0.0, 0.0, 0.0), Position::new(2.0, 2.0, 2.0));
        let bb2 = BoundingBox::new(Position::new(1.0, 1.0, 1.0), Position::new(3.0, 3.0, 3.0));

        assert!(bb1.intersects(&bb2));
        let intersection = bb1.intersection(&bb2).unwrap();
        assert_eq!(intersection.min, Position::new(1.0, 1.0, 1.0));
        assert_eq!(intersection.max, Position::new(2.0, 2.0, 2.0));
    }

    #[test]
    fn test_bounding_box_expand() {
        let mut bb = BoundingBox::empty();
        bb.expand_to_include(&Position::new(1.0, 2.0, 3.0));
        bb.expand_to_include(&Position::new(-1.0, -2.0, -3.0));

        assert_eq!(bb.min, Position::new(-1.0, -2.0, -3.0));
        assert_eq!(bb.max, Position::new(1.0, 2.0, 3.0));
    }
}

/// Non-Euclidean geometry engine.
pub mod non_euclidean;
