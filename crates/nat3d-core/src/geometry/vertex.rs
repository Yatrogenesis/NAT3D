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

//! Vertex data structure and operations.
//!
//! Vertices are the fundamental building blocks of 3D geometry,
//! representing points in 3D space with optional attributes like
//! normals, texture coordinates, and colors.

use super::{Normal, Position, TexCoord};
use nalgebra::Vector4;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for a vertex within a mesh.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VertexId(pub Uuid);

impl VertexId {
    /// Create a new unique vertex ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create a vertex ID from an existing UUID.
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

impl Default for VertexId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for VertexId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Vertex({})", &self.0.to_string()[..8])
    }
}

/// Vertex color in RGBA format.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VertexColor {
    /// Red component (0.0 - 1.0).
    pub r: f32,
    /// Green component (0.0 - 1.0).
    pub g: f32,
    /// Blue component (0.0 - 1.0).
    pub b: f32,
    /// Alpha component (0.0 - 1.0).
    pub a: f32,
}

impl VertexColor {
    /// Create a new vertex color.
    #[must_use]
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// White color.
    pub const WHITE: Self = Self::new(1.0, 1.0, 1.0, 1.0);
    /// Black color.
    pub const BLACK: Self = Self::new(0.0, 0.0, 0.0, 1.0);
    /// Red color.
    pub const RED: Self = Self::new(1.0, 0.0, 0.0, 1.0);
    /// Green color.
    pub const GREEN: Self = Self::new(0.0, 1.0, 0.0, 1.0);
    /// Blue color.
    pub const BLUE: Self = Self::new(0.0, 0.0, 1.0, 1.0);

    /// Convert to a Vector4 for GPU upload.
    #[must_use]
    pub fn to_vec4(&self) -> Vector4<f32> {
        Vector4::new(self.r, self.g, self.b, self.a)
    }

    /// Create from a Vector4.
    #[must_use]
    pub fn from_vec4(v: Vector4<f32>) -> Self {
        Self::new(v.x, v.y, v.z, v.w)
    }

    /// Linear interpolation between two colors.
    #[must_use]
    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        Self {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
            a: self.a + (other.a - self.a) * t,
        }
    }
}

impl Default for VertexColor {
    fn default() -> Self {
        Self::WHITE
    }
}

/// Complete vertex data including all attributes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VertexData {
    /// Position in 3D space.
    pub position: Position,
    /// Normal vector (may be computed or user-defined).
    pub normal: Option<Normal>,
    /// Primary texture coordinate.
    pub uv: Option<TexCoord>,
    /// Secondary texture coordinate (for lightmaps, etc.).
    pub uv2: Option<TexCoord>,
    /// Vertex color.
    pub color: Option<VertexColor>,
    /// Tangent vector for normal mapping.
    pub tangent: Option<Normal>,
    /// Bitangent vector for normal mapping.
    pub bitangent: Option<Normal>,
}

impl VertexData {
    /// Create vertex data with just a position.
    #[must_use]
    pub fn from_position(position: Position) -> Self {
        Self {
            position,
            normal: None,
            uv: None,
            uv2: None,
            color: None,
            tangent: None,
            bitangent: None,
        }
    }

    /// Create vertex data with position and normal.
    #[must_use]
    pub fn with_normal(position: Position, normal: Normal) -> Self {
        Self {
            position,
            normal: Some(normal),
            uv: None,
            uv2: None,
            color: None,
            tangent: None,
            bitangent: None,
        }
    }

    /// Create vertex data with position, normal, and UV.
    #[must_use]
    pub fn with_uv(position: Position, normal: Normal, uv: TexCoord) -> Self {
        Self {
            position,
            normal: Some(normal),
            uv: Some(uv),
            uv2: None,
            color: None,
            tangent: None,
            bitangent: None,
        }
    }

    /// Builder method to set the normal.
    #[must_use]
    pub fn normal(mut self, normal: Normal) -> Self {
        self.normal = Some(normal);
        self
    }

    /// Builder method to set the UV coordinate.
    #[must_use]
    pub fn uv(mut self, uv: TexCoord) -> Self {
        self.uv = Some(uv);
        self
    }

    /// Builder method to set the secondary UV coordinate.
    #[must_use]
    pub fn uv2(mut self, uv2: TexCoord) -> Self {
        self.uv2 = Some(uv2);
        self
    }

    /// Builder method to set the color.
    #[must_use]
    pub fn color(mut self, color: VertexColor) -> Self {
        self.color = Some(color);
        self
    }

    /// Builder method to set tangent and bitangent.
    #[must_use]
    pub fn tangent_space(mut self, tangent: Normal, bitangent: Normal) -> Self {
        self.tangent = Some(tangent);
        self.bitangent = Some(bitangent);
        self
    }

    /// Linear interpolation between two vertex data.
    #[must_use]
    pub fn lerp(&self, other: &Self, t: f64) -> Self {
        Self {
            position: Position::new(
                self.position.x + (other.position.x - self.position.x) * t,
                self.position.y + (other.position.y - self.position.y) * t,
                self.position.z + (other.position.z - self.position.z) * t,
            ),
            normal: match (&self.normal, &other.normal) {
                (Some(n1), Some(n2)) => Some(n1.lerp(n2, t).normalize()),
                (Some(n), None) | (None, Some(n)) => Some(*n),
                (None, None) => None,
            },
            uv: match (&self.uv, &other.uv) {
                (Some(uv1), Some(uv2)) => Some(TexCoord::new(
                    uv1.x + (uv2.x - uv1.x) * t,
                    uv1.y + (uv2.y - uv1.y) * t,
                )),
                (Some(uv), None) | (None, Some(uv)) => Some(*uv),
                (None, None) => None,
            },
            uv2: match (&self.uv2, &other.uv2) {
                (Some(uv1), Some(uv2)) => Some(TexCoord::new(
                    uv1.x + (uv2.x - uv1.x) * t,
                    uv1.y + (uv2.y - uv1.y) * t,
                )),
                (Some(uv), None) | (None, Some(uv)) => Some(*uv),
                (None, None) => None,
            },
            color: match (&self.color, &other.color) {
                (Some(c1), Some(c2)) => Some(c1.lerp(c2, t as f32)),
                (Some(c), None) | (None, Some(c)) => Some(*c),
                (None, None) => None,
            },
            tangent: match (&self.tangent, &other.tangent) {
                (Some(t1), Some(t2)) => Some(t1.lerp(t2, t).normalize()),
                (Some(tan), None) | (None, Some(tan)) => Some(*tan),
                (None, None) => None,
            },
            bitangent: match (&self.bitangent, &other.bitangent) {
                (Some(b1), Some(b2)) => Some(b1.lerp(b2, t).normalize()),
                (Some(b), None) | (None, Some(b)) => Some(*b),
                (None, None) => None,
            },
        }
    }
}

/// A vertex in a mesh with its unique ID and data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Vertex {
    /// Unique identifier.
    pub id: VertexId,
    /// Vertex data (position, normal, UV, etc.).
    pub data: VertexData,
    /// Index in the mesh's vertex array (for efficient lookup).
    pub index: usize,
}

impl Vertex {
    /// Create a new vertex with the given data.
    #[must_use]
    pub fn new(data: VertexData, index: usize) -> Self {
        Self {
            id: VertexId::new(),
            data,
            index,
        }
    }

    /// Create a new vertex with just a position.
    #[must_use]
    pub fn from_position(position: Position, index: usize) -> Self {
        Self::new(VertexData::from_position(position), index)
    }

    /// Get the position of this vertex.
    #[must_use]
    pub fn position(&self) -> &Position {
        &self.data.position
    }

    /// Get the normal of this vertex, if any.
    #[must_use]
    pub fn normal(&self) -> Option<&Normal> {
        self.data.normal.as_ref()
    }

    /// Get the UV coordinate of this vertex, if any.
    #[must_use]
    pub fn uv(&self) -> Option<&TexCoord> {
        self.data.uv.as_ref()
    }

    /// Set the position of this vertex.
    pub fn set_position(&mut self, position: Position) {
        self.data.position = position;
    }

    /// Set the normal of this vertex.
    pub fn set_normal(&mut self, normal: Normal) {
        self.data.normal = Some(normal);
    }

    /// Set the UV coordinate of this vertex.
    pub fn set_uv(&mut self, uv: TexCoord) {
        self.data.uv = Some(uv);
    }

    /// Calculate squared distance to another vertex.
    #[must_use]
    pub fn distance_squared_to(&self, other: &Vertex) -> f64 {
        let dx = self.data.position.x - other.data.position.x;
        let dy = self.data.position.y - other.data.position.y;
        let dz = self.data.position.z - other.data.position.z;
        dx * dx + dy * dy + dz * dz
    }

    /// Calculate distance to another vertex.
    #[must_use]
    pub fn distance_to(&self, other: &Vertex) -> f64 {
        self.distance_squared_to(other).sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vertex_id_uniqueness() {
        let id1 = VertexId::new();
        let id2 = VertexId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_vertex_color_lerp() {
        let c1 = VertexColor::BLACK;
        let c2 = VertexColor::WHITE;
        let mid = c1.lerp(&c2, 0.5);

        assert!((mid.r - 0.5).abs() < f32::EPSILON);
        assert!((mid.g - 0.5).abs() < f32::EPSILON);
        assert!((mid.b - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_vertex_data_builder() {
        let data = VertexData::from_position(Position::new(1.0, 2.0, 3.0))
            .normal(Normal::new(0.0, 1.0, 0.0))
            .uv(TexCoord::new(0.5, 0.5))
            .color(VertexColor::RED);

        assert_eq!(data.position, Position::new(1.0, 2.0, 3.0));
        assert!(data.normal.is_some());
        assert!(data.uv.is_some());
        assert!(data.color.is_some());
    }

    #[test]
    fn test_vertex_distance() {
        let v1 = Vertex::from_position(Position::new(0.0, 0.0, 0.0), 0);
        let v2 = Vertex::from_position(Position::new(3.0, 4.0, 0.0), 1);

        assert!((v1.distance_to(&v2) - 5.0).abs() < 1e-10);
    }
}
