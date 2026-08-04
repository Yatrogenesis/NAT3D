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

//! Mesh data structure and operations.
//!
//! The Mesh is the primary 3D geometry representation in NAT3D,
//! supporting polygonal modeling with full topological information.

use super::{BoundingBox, Edge, EdgeKey, Face, Normal, Position, TexCoord, Vertex, VertexData};
use crate::error::{CoreError, CoreResult};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::collections::HashMap;
use uuid::Uuid;

/// Unique identifier for a mesh.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MeshId(pub Uuid);

impl MeshId {
    /// Create a new unique mesh ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create a mesh ID from an existing UUID.
    #[must_use]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for MeshId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for MeshId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Mesh({})", &self.0.to_string()[..8])
    }
}

/// Topology caching and acceleration structures.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MeshTopology {
    /// Map from edge key (ordered vertex pair) to edge index.
    pub edge_map: HashMap<(usize, usize), usize>,
    /// Vertex-to-face adjacency: for each vertex, list of adjacent face indices.
    pub vertex_faces: Vec<SmallVec<[usize; 8]>>,
    /// Vertex-to-edge adjacency: for each vertex, list of adjacent edge indices.
    pub vertex_edges: Vec<SmallVec<[usize; 8]>>,
    /// Whether the topology cache is valid.
    pub is_valid: bool,
}

impl MeshTopology {
    /// Create empty topology.
    #[must_use]
    pub fn new() -> Self {
        Self {
            edge_map: HashMap::new(),
            vertex_faces: Vec::new(),
            vertex_edges: Vec::new(),
            is_valid: false,
        }
    }

    /// Clear all cached topology data.
    pub fn invalidate(&mut self) {
        self.edge_map.clear();
        self.vertex_faces.clear();
        self.vertex_edges.clear();
        self.is_valid = false;
    }
}

/// Serializable mesh data without topology cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshData {
    /// Mesh name.
    pub name: String,
    /// Vertex positions.
    pub positions: Vec<Position>,
    /// Vertex normals (same length as positions, or empty for auto-compute).
    pub normals: Vec<Normal>,
    /// Texture coordinates (same length as positions, or empty).
    pub uvs: Vec<TexCoord>,
    /// Face definitions as lists of vertex indices.
    pub faces: Vec<Vec<usize>>,
    /// Material indices per face (same length as faces, or empty for single material).
    pub material_indices: Vec<usize>,
}

impl MeshData {
    /// Create empty mesh data.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            positions: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            faces: Vec::new(),
            material_indices: Vec::new(),
        }
    }
}

/// A 3D polygonal mesh with full topological information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mesh {
    /// Unique identifier.
    pub id: MeshId,
    /// Mesh name.
    pub name: String,
    /// Vertices with full data.
    pub vertices: Vec<Vertex>,
    /// Edges with connectivity.
    pub edges: Vec<Edge>,
    /// Faces (polygons).
    pub faces: Vec<Face>,
    /// Cached bounding box.
    #[serde(skip)]
    bounds_cache: Option<BoundingBox>,
    /// Topology acceleration structures.
    #[serde(skip)]
    topology: MeshTopology,
}

impl Mesh {
    /// Create a new empty mesh with the given name.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: MeshId::new(),
            name: name.into(),
            vertices: Vec::new(),
            edges: Vec::new(),
            faces: Vec::new(),
            bounds_cache: None,
            topology: MeshTopology::new(),
        }
    }

    /// Create a mesh from raw data.
    #[must_use]
    pub fn from_data(data: MeshData) -> Self {
        let mut mesh = Self::new(&data.name);

        // Add vertices
        for (i, pos) in data.positions.iter().enumerate() {
            let mut vertex_data = VertexData::from_position(*pos);
            if i < data.normals.len() {
                vertex_data.normal = Some(data.normals[i]);
            }
            if i < data.uvs.len() {
                vertex_data.uv = Some(data.uvs[i]);
            }
            mesh.vertices.push(Vertex::new(vertex_data, i));
        }

        // Add faces
        for (face_idx, face_verts) in data.faces.iter().enumerate() {
            let mut face = Face::ngon(face_verts);
            if face_idx < data.material_indices.len() {
                face.data.material_index = Some(data.material_indices[face_idx]);
            }
            mesh.faces.push(face);
        }

        mesh.rebuild_topology();
        mesh
    }

    /// Export mesh to serializable data.
    #[must_use]
    pub fn to_data(&self) -> MeshData {
        MeshData {
            name: self.name.clone(),
            positions: self.vertices.iter().map(|v| v.data.position).collect(),
            normals: self.vertices.iter().filter_map(|v| v.data.normal).collect(),
            uvs: self.vertices.iter().filter_map(|v| v.data.uv).collect(),
            faces: self.faces.iter().map(|f| f.vertices.to_vec()).collect(),
            material_indices: self
                .faces
                .iter()
                .filter_map(|f| f.data.material_index)
                .collect(),
        }
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Vertex Operations
    // ══════════════════════════════════════════════════════════════════════════

    /// Get the number of vertices.
    #[must_use]
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Add a vertex and return its index.
    pub fn add_vertex(&mut self, data: VertexData) -> usize {
        let index = self.vertices.len();
        self.vertices.push(Vertex::new(data, index));
        self.invalidate_cache();
        index
    }

    /// Add a vertex at a position and return its index.
    pub fn add_vertex_at(&mut self, position: Position) -> usize {
        self.add_vertex(VertexData::from_position(position))
    }

    /// Get a vertex by index.
    pub fn vertex(&self, index: usize) -> CoreResult<&Vertex> {
        self.vertices
            .get(index)
            .ok_or_else(|| CoreError::invalid_vertex(index, self.vertices.len()))
    }

    /// Get a mutable vertex by index.
    pub fn vertex_mut(&mut self, index: usize) -> CoreResult<&mut Vertex> {
        let count = self.vertices.len();
        self.vertices
            .get_mut(index)
            .ok_or_else(|| CoreError::invalid_vertex(index, count))
    }

    /// Get vertex positions as a slice.
    #[must_use]
    pub fn positions(&self) -> Vec<Position> {
        self.vertices.iter().map(|v| v.data.position).collect()
    }

    /// Set a vertex position.
    pub fn set_vertex_position(&mut self, index: usize, position: Position) -> CoreResult<()> {
        self.vertex_mut(index)?.data.position = position;
        self.invalidate_cache();
        Ok(())
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Edge Operations
    // ══════════════════════════════════════════════════════════════════════════

    /// Get the number of edges.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Find or create an edge between two vertices.
    fn get_or_create_edge(&mut self, v0: usize, v1: usize) -> usize {
        let key = EdgeKey::new(v0, v1);
        if let Some(&edge_idx) = self.topology.edge_map.get(&(key.v0, key.v1)) {
            return edge_idx;
        }

        let edge_idx = self.edges.len();
        self.edges.push(Edge::new(v0, v1));
        self.topology.edge_map.insert((key.v0, key.v1), edge_idx);
        edge_idx
    }

    /// Get an edge by index.
    pub fn edge(&self, index: usize) -> CoreResult<&Edge> {
        self.edges
            .get(index)
            .ok_or_else(|| CoreError::invalid_edge(index, self.edges.len()))
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Face Operations
    // ══════════════════════════════════════════════════════════════════════════

    /// Get the number of faces.
    #[must_use]
    pub fn face_count(&self) -> usize {
        self.faces.len()
    }

    /// Add a triangle face and return its index.
    pub fn add_triangle(&mut self, v0: usize, v1: usize, v2: usize) -> CoreResult<usize> {
        self.validate_vertex_indices(&[v0, v1, v2])?;

        let face_idx = self.faces.len();
        self.faces.push(Face::triangle(v0, v1, v2));

        // Create/update edges
        self.ensure_topology();
        let e0 = self.get_or_create_edge(v0, v1);
        let e1 = self.get_or_create_edge(v1, v2);
        let e2 = self.get_or_create_edge(v2, v0);

        self.edges[e0].add_face(face_idx);
        self.edges[e1].add_face(face_idx);
        self.edges[e2].add_face(face_idx);

        self.invalidate_cache();
        Ok(face_idx)
    }

    /// Add a quad face and return its index.
    pub fn add_quad(&mut self, v0: usize, v1: usize, v2: usize, v3: usize) -> CoreResult<usize> {
        self.validate_vertex_indices(&[v0, v1, v2, v3])?;

        let face_idx = self.faces.len();
        self.faces.push(Face::quad(v0, v1, v2, v3));

        self.ensure_topology();
        let edges = [(v0, v1), (v1, v2), (v2, v3), (v3, v0)];
        for (a, b) in edges {
            let e = self.get_or_create_edge(a, b);
            self.edges[e].add_face(face_idx);
        }

        self.invalidate_cache();
        Ok(face_idx)
    }

    /// Add an n-gon face and return its index.
    pub fn add_ngon(&mut self, vertices: &[usize]) -> CoreResult<usize> {
        if vertices.len() < 3 {
            return Err(CoreError::EmptyMesh {
                required: "at least 3 vertices for a face".into(),
            });
        }
        self.validate_vertex_indices(vertices)?;

        let face_idx = self.faces.len();
        self.faces.push(Face::ngon(vertices));

        self.ensure_topology();
        for i in 0..vertices.len() {
            let v0 = vertices[i];
            let v1 = vertices[(i + 1) % vertices.len()];
            let e = self.get_or_create_edge(v0, v1);
            self.edges[e].add_face(face_idx);
        }

        self.invalidate_cache();
        Ok(face_idx)
    }

    /// Get a face by index.
    pub fn face(&self, index: usize) -> CoreResult<&Face> {
        self.faces
            .get(index)
            .ok_or_else(|| CoreError::invalid_face(index, self.faces.len()))
    }

    /// Get a mutable face by index.
    pub fn face_mut(&mut self, index: usize) -> CoreResult<&mut Face> {
        let count = self.faces.len();
        self.faces
            .get_mut(index)
            .ok_or_else(|| CoreError::invalid_face(index, count))
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Topology Management
    // ══════════════════════════════════════════════════════════════════════════

    /// Rebuild all topology information.
    pub fn rebuild_topology(&mut self) {
        self.topology.invalidate();
        self.edges.clear();

        // Build edge map from faces
        for (face_idx, face) in self.faces.iter().enumerate() {
            for i in 0..face.vertices.len() {
                let v0 = face.vertices[i];
                let v1 = face.vertices[(i + 1) % face.vertices.len()];
                let key = EdgeKey::new(v0, v1);

                let edge_idx = if let Some(&idx) = self.topology.edge_map.get(&(key.v0, key.v1)) {
                    idx
                } else {
                    let idx = self.edges.len();
                    self.edges.push(Edge::new(v0, v1));
                    self.topology.edge_map.insert((key.v0, key.v1), idx);
                    idx
                };

                self.edges[edge_idx].add_face(face_idx);
            }
        }

        // Build vertex adjacency
        self.topology.vertex_faces = vec![SmallVec::new(); self.vertices.len()];
        self.topology.vertex_edges = vec![SmallVec::new(); self.vertices.len()];

        for (face_idx, face) in self.faces.iter().enumerate() {
            for &vi in &face.vertices {
                if vi < self.topology.vertex_faces.len() {
                    self.topology.vertex_faces[vi].push(face_idx);
                }
            }
        }

        for (edge_idx, edge) in self.edges.iter().enumerate() {
            if edge.v0 < self.topology.vertex_edges.len() {
                self.topology.vertex_edges[edge.v0].push(edge_idx);
            }
            if edge.v1 < self.topology.vertex_edges.len() {
                self.topology.vertex_edges[edge.v1].push(edge_idx);
            }
        }

        self.topology.is_valid = true;
    }

    /// Ensure topology is valid, rebuilding if necessary.
    fn ensure_topology(&mut self) {
        if !self.topology.is_valid {
            self.rebuild_topology();
        }
    }

    /// Invalidate caches after modification.
    fn invalidate_cache(&mut self) {
        self.bounds_cache = None;
    }

    /// Validate vertex indices.
    fn validate_vertex_indices(&self, indices: &[usize]) -> CoreResult<()> {
        for &idx in indices {
            if idx >= self.vertices.len() {
                return Err(CoreError::invalid_vertex(idx, self.vertices.len()));
            }
        }
        Ok(())
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Geometry Queries
    // ══════════════════════════════════════════════════════════════════════════

    /// Compute or retrieve the bounding box.
    #[must_use]
    pub fn bounds(&mut self) -> BoundingBox {
        if let Some(bounds) = self.bounds_cache {
            return bounds;
        }

        let mut bounds = BoundingBox::empty();
        for vertex in &self.vertices {
            bounds.expand_to_include(&vertex.data.position);
        }

        self.bounds_cache = Some(bounds);
        bounds
    }

    /// Compute all vertex normals from face normals.
    pub fn compute_vertex_normals(&mut self) {
        self.ensure_topology();

        let positions: Vec<Position> = self.positions();

        // First compute face normals
        for face in &mut self.faces {
            face.update_normal(&positions);
        }

        // Then average face normals at each vertex
        for (vi, vertex) in self.vertices.iter_mut().enumerate() {
            if vi >= self.topology.vertex_faces.len() {
                continue;
            }

            let mut normal_sum = Normal::zeros();
            let mut count = 0;

            for &face_idx in &self.topology.vertex_faces[vi] {
                if let Some(n) = self.faces[face_idx].normal {
                    normal_sum += n;
                    count += 1;
                }
            }

            if count > 0 {
                let avg = normal_sum / f64::from(count);
                let len = avg.magnitude();
                if len > f64::EPSILON {
                    vertex.data.normal = Some(avg / len);
                }
            }
        }
    }

    /// Check if the mesh is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }

    /// Check if the mesh has any non-manifold edges.
    #[must_use]
    pub fn has_non_manifold_edges(&self) -> bool {
        self.edges.iter().any(super::edge::Edge::is_non_manifold)
    }

    /// Check if the mesh has any boundary edges.
    #[must_use]
    pub fn has_boundary(&self) -> bool {
        self.edges.iter().any(super::edge::Edge::is_boundary)
    }

    /// Get all boundary edges.
    #[must_use]
    pub fn boundary_edges(&self) -> Vec<usize> {
        self.edges
            .iter()
            .enumerate()
            .filter(|(_, e)| e.is_boundary())
            .map(|(i, _)| i)
            .collect()
    }

    /// Count triangles (for rendering).
    #[must_use]
    pub fn triangle_count(&self) -> usize {
        self.faces
            .iter()
            .map(|f| {
                if f.vertex_count() >= 3 {
                    f.vertex_count() - 2
                } else {
                    0
                }
            })
            .sum()
    }

    /// Generate triangulated indices for rendering.
    #[must_use]
    pub fn triangulated_indices(&self) -> Vec<u32> {
        let mut indices = Vec::with_capacity(self.triangle_count() * 3);

        for face in &self.faces {
            for tri in face.triangulate() {
                indices.push(tri[0] as u32);
                indices.push(tri[1] as u32);
                indices.push(tri[2] as u32);
            }
        }

        indices
    }
}

impl Default for Mesh {
    fn default() -> Self {
        Self::new("Untitled")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mesh_creation() {
        let mesh = Mesh::new("Test");
        assert_eq!(mesh.name, "Test");
        assert!(mesh.is_empty());
    }

    #[test]
    fn test_add_vertices() {
        let mut mesh = Mesh::new("Test");
        let i0 = mesh.add_vertex_at(Position::new(0.0, 0.0, 0.0));
        let i1 = mesh.add_vertex_at(Position::new(1.0, 0.0, 0.0));
        let i2 = mesh.add_vertex_at(Position::new(0.0, 1.0, 0.0));

        assert_eq!(mesh.vertex_count(), 3);
        assert_eq!(i0, 0);
        assert_eq!(i1, 1);
        assert_eq!(i2, 2);
    }

    #[test]
    fn test_add_triangle() {
        let mut mesh = Mesh::new("Test");
        mesh.add_vertex_at(Position::new(0.0, 0.0, 0.0));
        mesh.add_vertex_at(Position::new(1.0, 0.0, 0.0));
        mesh.add_vertex_at(Position::new(0.0, 1.0, 0.0));

        let face_idx = mesh.add_triangle(0, 1, 2).unwrap();
        assert_eq!(face_idx, 0);
        assert_eq!(mesh.face_count(), 1);
        assert_eq!(mesh.edge_count(), 3);
    }

    #[test]
    fn test_add_quad() {
        let mut mesh = Mesh::new("Test");
        mesh.add_vertex_at(Position::new(0.0, 0.0, 0.0));
        mesh.add_vertex_at(Position::new(1.0, 0.0, 0.0));
        mesh.add_vertex_at(Position::new(1.0, 1.0, 0.0));
        mesh.add_vertex_at(Position::new(0.0, 1.0, 0.0));

        let face_idx = mesh.add_quad(0, 1, 2, 3).unwrap();
        assert_eq!(face_idx, 0);
        assert_eq!(mesh.face_count(), 1);
        assert_eq!(mesh.edge_count(), 4);
    }

    #[test]
    fn test_invalid_vertex_index() {
        let mut mesh = Mesh::new("Test");
        mesh.add_vertex_at(Position::new(0.0, 0.0, 0.0));

        let result = mesh.add_triangle(0, 1, 2);
        assert!(result.is_err());
    }

    #[test]
    fn test_bounds() {
        let mut mesh = Mesh::new("Test");
        mesh.add_vertex_at(Position::new(-1.0, -2.0, -3.0));
        mesh.add_vertex_at(Position::new(1.0, 2.0, 3.0));

        let bounds = mesh.bounds();
        assert_eq!(bounds.min, Position::new(-1.0, -2.0, -3.0));
        assert_eq!(bounds.max, Position::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn test_triangulated_indices() {
        let mut mesh = Mesh::new("Test");
        mesh.add_vertex_at(Position::new(0.0, 0.0, 0.0));
        mesh.add_vertex_at(Position::new(1.0, 0.0, 0.0));
        mesh.add_vertex_at(Position::new(1.0, 1.0, 0.0));
        mesh.add_vertex_at(Position::new(0.0, 1.0, 0.0));
        mesh.add_quad(0, 1, 2, 3).unwrap();

        let indices = mesh.triangulated_indices();
        assert_eq!(indices.len(), 6); // 2 triangles * 3 indices
    }
}
