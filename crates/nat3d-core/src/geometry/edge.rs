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

//! Edge data structure and operations.
//!
//! Edges connect two vertices and form the boundaries of faces.
//! This module implements edges with half-edge topology support.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for an edge within a mesh.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EdgeId(pub Uuid);

impl EdgeId {
    /// Create a new unique edge ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create an edge ID from an existing UUID.
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

impl Default for EdgeId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for EdgeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Edge({})", &self.0.to_string()[..8])
    }
}

/// Edge sharpness for subdivision surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum EdgeCrease {
    /// Smooth edge (fully subdivided).
    #[default]
    Smooth,
    /// Partially sharp edge with crease weight (0.0 = smooth, 1.0 = fully sharp).
    Crease(f64),
    /// Fully sharp edge (not subdivided).
    Sharp,
}

impl EdgeCrease {
    /// Get the crease weight as a f64 value.
    #[must_use]
    pub fn weight(&self) -> f64 {
        match self {
            EdgeCrease::Smooth => 0.0,
            EdgeCrease::Crease(w) => *w,
            EdgeCrease::Sharp => 1.0,
        }
    }

    /// Create from a weight value, clamped to [0, 1].
    #[must_use]
    pub fn from_weight(weight: f64) -> Self {
        let w = weight.clamp(0.0, 1.0);
        if w <= f64::EPSILON {
            EdgeCrease::Smooth
        } else if (w - 1.0).abs() <= f64::EPSILON {
            EdgeCrease::Sharp
        } else {
            EdgeCrease::Crease(w)
        }
    }
}

/// Additional data associated with an edge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EdgeData {
    /// Crease/sharpness for subdivision.
    pub crease: EdgeCrease,
    /// Whether this edge is marked as a seam (for UV unwrapping).
    pub is_seam: bool,
    /// Whether this edge is marked as sharp (for normal calculation).
    pub is_sharp: bool,
    /// User-defined edge group/tag.
    pub group: Option<String>,
}

impl EdgeData {
    /// Create default edge data.
    #[must_use]
    pub fn new() -> Self {
        Self {
            crease: EdgeCrease::Smooth,
            is_seam: false,
            is_sharp: false,
            group: None,
        }
    }

    /// Builder method to set crease.
    #[must_use]
    pub fn with_crease(mut self, crease: EdgeCrease) -> Self {
        self.crease = crease;
        self
    }

    /// Builder method to mark as seam.
    #[must_use]
    pub fn as_seam(mut self) -> Self {
        self.is_seam = true;
        self
    }

    /// Builder method to mark as sharp.
    #[must_use]
    pub fn as_sharp(mut self) -> Self {
        self.is_sharp = true;
        self
    }

    /// Builder method to set group.
    #[must_use]
    pub fn with_group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }
}

impl Default for EdgeData {
    fn default() -> Self {
        Self::new()
    }
}

/// An edge connecting two vertices.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    /// Unique identifier.
    pub id: EdgeId,
    /// Index of the start vertex.
    pub v0: usize,
    /// Index of the end vertex.
    pub v1: usize,
    /// Additional edge data.
    pub data: EdgeData,
    /// Faces that share this edge (0, 1, or 2 faces typically).
    pub faces: smallvec::SmallVec<[usize; 2]>,
}

impl Edge {
    /// Create a new edge between two vertex indices.
    #[must_use]
    pub fn new(v0: usize, v1: usize) -> Self {
        Self {
            id: EdgeId::new(),
            v0,
            v1,
            data: EdgeData::new(),
            faces: smallvec::SmallVec::new(),
        }
    }

    /// Create a new edge with data.
    #[must_use]
    pub fn with_data(v0: usize, v1: usize, data: EdgeData) -> Self {
        Self {
            id: EdgeId::new(),
            v0,
            v1,
            data,
            faces: smallvec::SmallVec::new(),
        }
    }

    /// Get the vertex indices as a tuple, ordered (smaller, larger).
    #[must_use]
    pub fn ordered_vertices(&self) -> (usize, usize) {
        if self.v0 <= self.v1 {
            (self.v0, self.v1)
        } else {
            (self.v1, self.v0)
        }
    }

    /// Check if this edge contains a vertex index.
    #[must_use]
    pub fn contains_vertex(&self, vertex_index: usize) -> bool {
        self.v0 == vertex_index || self.v1 == vertex_index
    }

    /// Get the other vertex index given one vertex of the edge.
    /// Returns None if the given vertex is not part of this edge.
    #[must_use]
    pub fn other_vertex(&self, vertex_index: usize) -> Option<usize> {
        if self.v0 == vertex_index {
            Some(self.v1)
        } else if self.v1 == vertex_index {
            Some(self.v0)
        } else {
            None
        }
    }

    /// Check if this edge is a boundary edge (has only one adjacent face).
    #[must_use]
    pub fn is_boundary(&self) -> bool {
        self.faces.len() == 1
    }

    /// Check if this edge is manifold (has exactly two adjacent faces).
    #[must_use]
    pub fn is_manifold(&self) -> bool {
        self.faces.len() == 2
    }

    /// Check if this edge is non-manifold (has more than two adjacent faces).
    #[must_use]
    pub fn is_non_manifold(&self) -> bool {
        self.faces.len() > 2
    }

    /// Check if this edge is isolated (has no adjacent faces).
    #[must_use]
    pub fn is_isolated(&self) -> bool {
        self.faces.is_empty()
    }

    /// Add a face to this edge's adjacency list.
    pub fn add_face(&mut self, face_index: usize) {
        if !self.faces.contains(&face_index) {
            self.faces.push(face_index);
        }
    }

    /// Remove a face from this edge's adjacency list.
    pub fn remove_face(&mut self, face_index: usize) {
        self.faces.retain(|f| *f != face_index);
    }

    /// Check if two edges share a common vertex.
    #[must_use]
    pub fn shares_vertex_with(&self, other: &Edge) -> bool {
        self.contains_vertex(other.v0) || self.contains_vertex(other.v1)
    }

    /// Get the shared vertex with another edge, if any.
    #[must_use]
    pub fn shared_vertex(&self, other: &Edge) -> Option<usize> {
        if self.contains_vertex(other.v0) {
            Some(other.v0)
        } else if self.contains_vertex(other.v1) {
            Some(other.v1)
        } else {
            None
        }
    }
}

/// A key for looking up edges by vertex pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EdgeKey {
    /// Smaller vertex index.
    pub v0: usize,
    /// Larger vertex index.
    pub v1: usize,
}

impl EdgeKey {
    /// Create a new edge key from two vertex indices.
    /// The indices are automatically ordered.
    #[must_use]
    pub fn new(a: usize, b: usize) -> Self {
        if a <= b {
            Self { v0: a, v1: b }
        } else {
            Self { v0: b, v1: a }
        }
    }
}

impl From<&Edge> for EdgeKey {
    fn from(edge: &Edge) -> Self {
        EdgeKey::new(edge.v0, edge.v1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge_id_uniqueness() {
        let id1 = EdgeId::new();
        let id2 = EdgeId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_edge_crease() {
        assert_eq!(EdgeCrease::Smooth.weight(), 0.0);
        assert_eq!(EdgeCrease::Sharp.weight(), 1.0);
        assert_eq!(EdgeCrease::Crease(0.5).weight(), 0.5);

        assert!(matches!(EdgeCrease::from_weight(0.0), EdgeCrease::Smooth));
        assert!(matches!(EdgeCrease::from_weight(1.0), EdgeCrease::Sharp));
        assert!(matches!(
            EdgeCrease::from_weight(0.5),
            EdgeCrease::Crease(_)
        ));
    }

    #[test]
    fn test_edge_ordered_vertices() {
        let e1 = Edge::new(5, 3);
        assert_eq!(e1.ordered_vertices(), (3, 5));

        let e2 = Edge::new(2, 7);
        assert_eq!(e2.ordered_vertices(), (2, 7));
    }

    #[test]
    fn test_edge_other_vertex() {
        let edge = Edge::new(3, 7);
        assert_eq!(edge.other_vertex(3), Some(7));
        assert_eq!(edge.other_vertex(7), Some(3));
        assert_eq!(edge.other_vertex(5), None);
    }

    #[test]
    fn test_edge_boundary() {
        let mut edge = Edge::new(0, 1);
        assert!(edge.is_isolated());

        edge.add_face(0);
        assert!(edge.is_boundary());

        edge.add_face(1);
        assert!(edge.is_manifold());

        edge.add_face(2);
        assert!(edge.is_non_manifold());
    }

    #[test]
    fn test_edge_key() {
        let key1 = EdgeKey::new(3, 7);
        let key2 = EdgeKey::new(7, 3);
        assert_eq!(key1, key2);
        assert_eq!(key1.v0, 3);
        assert_eq!(key1.v1, 7);
    }

    #[test]
    fn test_edge_shared_vertex() {
        let e1 = Edge::new(0, 1);
        let e2 = Edge::new(1, 2);
        let e3 = Edge::new(3, 4);

        assert_eq!(e1.shared_vertex(&e2), Some(1));
        assert_eq!(e1.shared_vertex(&e3), None);
    }
}
