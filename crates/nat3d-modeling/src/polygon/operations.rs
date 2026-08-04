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

//! Mesh editing operations.
//!
//! Common mesh manipulation operations like extrude, inset, bevel, etc.

use nalgebra::{Matrix4, Point3, Vector3};
use std::collections::{HashMap, HashSet};

/// Mesh data for operations.
#[derive(Debug, Clone)]
pub struct EditMesh {
    /// Vertex positions.
    pub positions: Vec<Point3<f64>>,
    /// Face vertex indices.
    pub faces: Vec<Vec<usize>>,
    /// Vertex normals.
    pub normals: Vec<Vector3<f64>>,
    /// UV coordinates.
    pub uvs: Vec<(f64, f64)>,
    /// Edge data (vertex pairs).
    edges: Vec<(usize, usize)>,
    /// Face to edge mapping.
    face_edges: Vec<Vec<usize>>,
    /// Edge to face mapping.
    edge_faces: HashMap<usize, Vec<usize>>,
    /// Vertex to face mapping.
    vertex_faces: Vec<Vec<usize>>,
    /// Selection state.
    pub selection: MeshSelection,
}

/// Mesh selection state.
#[derive(Debug, Clone, Default)]
pub struct MeshSelection {
    /// Selected vertices.
    pub vertices: HashSet<usize>,
    /// Selected edges.
    pub edges: HashSet<usize>,
    /// Selected faces.
    pub faces: HashSet<usize>,
}

impl EditMesh {
    /// Create a new edit mesh.
    pub fn new() -> Self {
        Self {
            positions: Vec::new(),
            faces: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            edges: Vec::new(),
            face_edges: Vec::new(),
            edge_faces: HashMap::new(),
            vertex_faces: Vec::new(),
            selection: MeshSelection::default(),
        }
    }

    /// Create from positions and faces.
    pub fn from_mesh(positions: Vec<Point3<f64>>, faces: Vec<Vec<usize>>) -> Self {
        let mut mesh = Self {
            positions,
            faces,
            normals: Vec::new(),
            uvs: Vec::new(),
            edges: Vec::new(),
            face_edges: Vec::new(),
            edge_faces: HashMap::new(),
            vertex_faces: Vec::new(),
            selection: MeshSelection::default(),
        };
        mesh.rebuild_topology();
        mesh.compute_normals();
        mesh
    }

    /// Rebuild topology data.
    pub fn rebuild_topology(&mut self) {
        self.edges.clear();
        self.face_edges.clear();
        self.edge_faces.clear();
        self.vertex_faces = vec![Vec::new(); self.positions.len()];

        let mut edge_map: HashMap<(usize, usize), usize> = HashMap::new();

        for (face_idx, face) in self.faces.iter().enumerate() {
            let mut face_edge_indices = Vec::new();

            for i in 0..face.len() {
                let v0 = face[i];
                let v1 = face[(i + 1) % face.len()];

                // Track vertex to face
                self.vertex_faces[v0].push(face_idx);

                // Get or create edge
                let edge_key = if v0 < v1 { (v0, v1) } else { (v1, v0) };
                let edge_idx = if let Some(&idx) = edge_map.get(&edge_key) {
                    idx
                } else {
                    let idx = self.edges.len();
                    self.edges.push(edge_key);
                    edge_map.insert(edge_key, idx);
                    idx
                };

                face_edge_indices.push(edge_idx);

                // Track edge to face
                self.edge_faces.entry(edge_idx).or_default().push(face_idx);
            }

            self.face_edges.push(face_edge_indices);
        }
    }

    /// Compute vertex normals.
    pub fn compute_normals(&mut self) {
        self.normals = vec![Vector3::zeros(); self.positions.len()];

        for face in &self.faces {
            if face.len() < 3 {
                continue;
            }

            // Compute face normal
            let v0 = self.positions[face[0]];
            let v1 = self.positions[face[1]];
            let v2 = self.positions[face[2]];
            let normal = (v1 - v0).cross(&(v2 - v0));

            // Accumulate to vertices
            for &vi in face {
                self.normals[vi] += normal;
            }
        }

        // Normalize
        for normal in &mut self.normals {
            let len = normal.magnitude();
            if len > 1e-10 {
                *normal /= len;
            }
        }
    }

    /// Get face normal.
    pub fn face_normal(&self, face_idx: usize) -> Vector3<f64> {
        let face = &self.faces[face_idx];
        if face.len() < 3 {
            return Vector3::new(0.0, 1.0, 0.0);
        }

        let v0 = self.positions[face[0]];
        let v1 = self.positions[face[1]];
        let v2 = self.positions[face[2]];
        (v1 - v0).cross(&(v2 - v0)).normalize()
    }

    /// Get face center.
    pub fn face_center(&self, face_idx: usize) -> Point3<f64> {
        let face = &self.faces[face_idx];
        let sum: Vector3<f64> = face.iter().map(|&vi| self.positions[vi].coords).sum();
        Point3::from(sum / face.len() as f64)
    }

    /// Get edge center.
    pub fn edge_center(&self, edge_idx: usize) -> Point3<f64> {
        let (v0, v1) = self.edges[edge_idx];
        Point3::from((self.positions[v0].coords + self.positions[v1].coords) / 2.0)
    }

    /// Get edge length.
    pub fn edge_length(&self, edge_idx: usize) -> f64 {
        let (v0, v1) = self.edges[edge_idx];
        (self.positions[v1] - self.positions[v0]).magnitude()
    }

    /// Check if edge is boundary.
    pub fn is_boundary_edge(&self, edge_idx: usize) -> bool {
        self.edge_faces
            .get(&edge_idx)
            .map_or(true, |f| f.len() == 1)
    }

    /// Get boundary vertices.
    pub fn boundary_vertices(&self) -> HashSet<usize> {
        let mut boundary = HashSet::new();
        for (idx, _) in self.edges.iter().enumerate() {
            if self.is_boundary_edge(idx) {
                let (v0, v1) = self.edges[idx];
                boundary.insert(v0);
                boundary.insert(v1);
            }
        }
        boundary
    }

    /// Add a vertex.
    pub fn add_vertex(&mut self, position: Point3<f64>) -> usize {
        let idx = self.positions.len();
        self.positions.push(position);
        self.normals.push(Vector3::new(0.0, 1.0, 0.0));
        self.vertex_faces.push(Vec::new());
        idx
    }

    /// Add a face.
    pub fn add_face(&mut self, vertices: Vec<usize>) -> usize {
        let idx = self.faces.len();
        self.faces.push(vertices);
        self.rebuild_topology();
        idx
    }

    /// Delete vertices.
    pub fn delete_vertices(&mut self, vertices: &HashSet<usize>) {
        // Find faces that use these vertices
        let faces_to_delete: HashSet<usize> = vertices
            .iter()
            .flat_map(|&v| self.vertex_faces.get(v).cloned().unwrap_or_default())
            .collect();

        self.delete_faces(&faces_to_delete);

        // Create vertex remapping
        let mut new_positions = Vec::new();
        let mut remap = HashMap::new();

        for (old_idx, pos) in self.positions.iter().enumerate() {
            if !vertices.contains(&old_idx) {
                remap.insert(old_idx, new_positions.len());
                new_positions.push(*pos);
            }
        }

        self.positions = new_positions;

        // Remap face indices
        for face in &mut self.faces {
            for vi in face.iter_mut() {
                if let Some(&new_idx) = remap.get(vi) {
                    *vi = new_idx;
                }
            }
        }

        self.rebuild_topology();
        self.compute_normals();
    }

    /// Delete faces.
    pub fn delete_faces(&mut self, faces: &HashSet<usize>) {
        self.faces = self
            .faces
            .iter()
            .enumerate()
            .filter(|(i, _)| !faces.contains(i))
            .map(|(_, f)| f.clone())
            .collect();

        self.rebuild_topology();
        self.compute_normals();
    }

    /// Delete edges (dissolve).
    pub fn dissolve_edges(&mut self, edges: &HashSet<usize>) {
        for &edge_idx in edges {
            if edge_idx >= self.edges.len() {
                continue;
            }

            if let Some(face_indices) = self.edge_faces.get(&edge_idx).cloned() {
                if face_indices.len() == 2 {
                    // Merge two faces
                    let f0 = face_indices[0];
                    let f1 = face_indices[1];

                    let (v0, v1) = self.edges[edge_idx];

                    // Create merged face
                    let face0 = &self.faces[f0];
                    let face1 = &self.faces[f1];

                    let mut merged = Vec::new();

                    // Add vertices from face0, skipping shared edge
                    for &vi in face0 {
                        if vi != v0 && vi != v1 || !face1.contains(&vi) {
                            merged.push(vi);
                        }
                    }

                    // Add vertices from face1, skipping shared edge
                    for &vi in face1 {
                        if vi != v0 && vi != v1 && !merged.contains(&vi) {
                            merged.push(vi);
                        }
                    }

                    // Replace first face with merged, mark second for deletion
                    self.faces[f0] = merged;
                    self.faces[f1] = Vec::new();
                }
            }
        }

        // Remove empty faces
        self.faces.retain(|f| !f.is_empty());
        self.rebuild_topology();
        self.compute_normals();
    }
}

impl Default for EditMesh {
    fn default() -> Self {
        Self::new()
    }
}

/// Extrude operation.
#[derive(Debug, Clone)]
pub struct ExtrudeOperation {
    /// Extrusion amount.
    pub amount: f64,
    /// Extrude along normals.
    pub along_normals: bool,
    /// Custom direction.
    pub direction: Option<Vector3<f64>>,
    /// Keep original faces.
    pub keep_original: bool,
}

impl Default for ExtrudeOperation {
    fn default() -> Self {
        Self {
            amount: 1.0,
            along_normals: true,
            direction: None,
            keep_original: false,
        }
    }
}

impl ExtrudeOperation {
    /// Extrude selected faces.
    pub fn execute(&self, mesh: &mut EditMesh) -> Result<(), &'static str> {
        let selected_faces: Vec<usize> = mesh.selection.faces.iter().copied().collect();
        if selected_faces.is_empty() {
            return Err("No faces selected");
        }

        // Collect vertices from selected faces
        let mut selected_vertices: HashSet<usize> = HashSet::new();
        for &face_idx in &selected_faces {
            for &vi in &mesh.faces[face_idx] {
                selected_vertices.insert(vi);
            }
        }

        // Create duplicate vertices
        let mut vertex_map: HashMap<usize, usize> = HashMap::new();
        for &vi in &selected_vertices {
            let new_idx = mesh.add_vertex(mesh.positions[vi]);
            vertex_map.insert(vi, new_idx);
        }

        // Move new vertices
        for (&old_vi, &new_vi) in &vertex_map {
            let direction = if self.along_normals {
                mesh.normals[old_vi]
            } else {
                self.direction.unwrap_or(Vector3::new(0.0, 1.0, 0.0))
            };
            mesh.positions[new_vi] += direction * self.amount;
        }

        // Update face vertices to point to new vertices
        for &face_idx in &selected_faces {
            for vi in &mut mesh.faces[face_idx] {
                if let Some(&new_vi) = vertex_map.get(vi) {
                    *vi = new_vi;
                }
            }
        }

        // Create side faces - collect first, then add
        let mut new_side_faces = Vec::new();
        for &face_idx in &selected_faces {
            let face = mesh.faces[face_idx].clone();
            for i in 0..face.len() {
                let v0 = face[i];
                let v1 = face[(i + 1) % face.len()];

                // Find original vertices
                let orig_v0 = vertex_map
                    .iter()
                    .find(|(_, &new)| new == v0)
                    .map(|(&old, _)| old);
                let orig_v1 = vertex_map
                    .iter()
                    .find(|(_, &new)| new == v1)
                    .map(|(&old, _)| old);

                if let (Some(ov0), Some(ov1)) = (orig_v0, orig_v1) {
                    // Check if this edge was shared (internal) or boundary
                    let edge_key = if ov0 < ov1 { (ov0, ov1) } else { (ov1, ov0) };
                    let is_boundary = mesh
                        .edges
                        .iter()
                        .position(|&e| e == edge_key)
                        .map(|idx| mesh.is_boundary_edge(idx))
                        .unwrap_or(true);

                    if is_boundary || !self.keep_original {
                        // Create quad side face
                        new_side_faces.push(vec![ov0, ov1, v1, v0]);
                    }
                }
            }
        }
        mesh.faces.extend(new_side_faces);

        mesh.rebuild_topology();
        mesh.compute_normals();
        Ok(())
    }
}

/// Inset faces operation.
#[derive(Debug, Clone)]
pub struct InsetOperation {
    /// Inset amount (0-1).
    pub thickness: f64,
    /// Depth (extrusion).
    pub depth: f64,
    /// Outset instead of inset.
    pub outset: bool,
    /// Individual face inset.
    pub individual: bool,
}

impl Default for InsetOperation {
    fn default() -> Self {
        Self {
            thickness: 0.1,
            depth: 0.0,
            outset: false,
            individual: false,
        }
    }
}

impl InsetOperation {
    /// Inset selected faces.
    pub fn execute(&self, mesh: &mut EditMesh) -> Result<(), &'static str> {
        let selected_faces: Vec<usize> = mesh.selection.faces.iter().copied().collect();
        if selected_faces.is_empty() {
            return Err("No faces selected");
        }

        for &face_idx in &selected_faces {
            let face = mesh.faces[face_idx].clone();
            let center = mesh.face_center(face_idx);
            let normal = mesh.face_normal(face_idx);

            // Create inset vertices
            let mut inset_vertices = Vec::new();
            for &vi in &face {
                let pos = mesh.positions[vi];
                let to_center = (center - pos)
                    * if self.outset {
                        -self.thickness
                    } else {
                        self.thickness
                    };
                let inset_pos = pos + to_center + normal * self.depth;
                let new_vi = mesh.add_vertex(inset_pos);
                inset_vertices.push(new_vi);
            }

            // Create connecting quads
            for i in 0..face.len() {
                let v0 = face[i];
                let v1 = face[(i + 1) % face.len()];
                let iv0 = inset_vertices[i];
                let iv1 = inset_vertices[(i + 1) % face.len()];

                mesh.faces.push(vec![v0, v1, iv1, iv0]);
            }

            // Update original face to inset vertices
            mesh.faces[face_idx] = inset_vertices;
        }

        mesh.rebuild_topology();
        mesh.compute_normals();
        Ok(())
    }
}

/// Bevel operation.
#[derive(Debug, Clone)]
pub struct BevelOperation {
    /// Bevel offset.
    pub offset: f64,
    /// Number of segments.
    pub segments: usize,
    /// Profile shape (0-1, 0.5 = round).
    pub profile: f64,
    /// Clamp overlap.
    pub clamp_overlap: bool,
}

impl Default for BevelOperation {
    fn default() -> Self {
        Self {
            offset: 0.1,
            segments: 1,
            profile: 0.5,
            clamp_overlap: true,
        }
    }
}

impl BevelOperation {
    /// Bevel selected edges.
    pub fn execute(&self, mesh: &mut EditMesh) -> Result<(), &'static str> {
        let selected_edges: Vec<usize> = mesh.selection.edges.iter().copied().collect();
        if selected_edges.is_empty() {
            return Err("No edges selected");
        }

        for &edge_idx in &selected_edges {
            if edge_idx >= mesh.edges.len() {
                continue;
            }

            let (v0, v1) = mesh.edges[edge_idx];
            let p0 = mesh.positions[v0];
            let p1 = mesh.positions[v1];

            let edge_dir = (p1 - p0).normalize();
            let edge_length = (p1 - p0).magnitude();

            // Clamp offset if needed
            let offset = if self.clamp_overlap {
                self.offset.min(edge_length / 2.0)
            } else {
                self.offset
            };

            // Create bevel vertices along the edge - collect first
            let mut bevel_vertices = Vec::new();
            for seg in 0..=self.segments {
                let t = seg as f64 / self.segments as f64;
                let base_pos = p0 + edge_dir * (offset + t * (edge_length - 2.0 * offset));

                // Offset perpendicular based on profile
                let profile_offset = (1.0 - (2.0 * t - 1.0).powi(2)).sqrt() * offset * self.profile;

                // Find perpendicular direction from adjacent faces
                if let Some(faces) = mesh.edge_faces.get(&edge_idx) {
                    for &face_idx in faces {
                        let normal = mesh.face_normal(face_idx);
                        let perp = edge_dir.cross(&normal).normalize();
                        let bevel_pos = base_pos + perp * profile_offset;
                        bevel_vertices.push(bevel_pos);
                    }
                }
            }
            // Add vertices after collecting
            for pos in bevel_vertices {
                mesh.add_vertex(pos);
            }
        }

        mesh.rebuild_topology();
        mesh.compute_normals();
        Ok(())
    }
}

/// Bridge operation (connect face loops).
#[derive(Debug, Clone)]
pub struct BridgeOperation {
    /// Number of twist steps.
    pub twist: i32,
    /// Number of segments.
    pub segments: usize,
    /// Interpolation profile.
    pub profile: f64,
}

impl Default for BridgeOperation {
    fn default() -> Self {
        Self {
            twist: 0,
            segments: 1,
            profile: 1.0,
        }
    }
}

impl BridgeOperation {
    /// Bridge between two edge loops.
    pub fn execute(
        &self,
        mesh: &mut EditMesh,
        loop1: &[usize],
        loop2: &[usize],
    ) -> Result<(), &'static str> {
        if loop1.len() != loop2.len() {
            return Err("Loops must have same vertex count");
        }

        let count = loop1.len();

        // Apply twist
        let twist_offset = self.twist.rem_euclid(count as i32) as usize;

        for seg in 0..self.segments {
            let t = (seg + 1) as f64 / (self.segments + 1) as f64;

            // Create intermediate vertices
            let mut seg_vertices = Vec::new();
            for i in 0..count {
                let v1 = loop1[i];
                let v2 = loop2[(i + twist_offset) % count];

                let p1 = mesh.positions[v1];
                let p2 = mesh.positions[v2];
                let pos = Point3::from(p1.coords * (1.0 - t) + p2.coords * t);
                seg_vertices.push(mesh.add_vertex(pos));
            }

            // Create faces between segments
            let prev_loop = if seg == 0 { loop1 } else { &[] };
            let _next_loop = if seg == self.segments - 1 {
                loop2
            } else {
                &seg_vertices
            };

            // Connect to previous segment
            if seg == 0 {
                for i in 0..count {
                    let v0 = prev_loop[i];
                    let v1 = prev_loop[(i + 1) % count];
                    let v2 = seg_vertices[(i + 1) % count];
                    let v3 = seg_vertices[i];
                    mesh.faces.push(vec![v0, v1, v2, v3]);
                }
            }
        }

        // Final connection to loop2
        let last_loop: Vec<usize> = (0..count)
            .map(|i| {
                let v1 = loop1[i];
                let v2 = loop2[(i + twist_offset) % count];
                let t = self.segments as f64 / (self.segments + 1) as f64;
                let p1 = mesh.positions[v1];
                let p2 = mesh.positions[v2];
                let pos = Point3::from(p1.coords * (1.0 - t) + p2.coords * t);
                mesh.add_vertex(pos)
            })
            .collect();

        for i in 0..count {
            let v0 = last_loop[i];
            let v1 = last_loop[(i + 1) % count];
            let v2 = loop2[(i + 1 + twist_offset) % count];
            let v3 = loop2[(i + twist_offset) % count];
            mesh.faces.push(vec![v0, v1, v2, v3]);
        }

        mesh.rebuild_topology();
        mesh.compute_normals();
        Ok(())
    }
}

/// Grid fill operation.
#[derive(Debug, Clone)]
pub struct GridFillOperation {
    /// Span to use for grid orientation.
    pub span: usize,
    /// Offset from span.
    pub offset: usize,
}

impl Default for GridFillOperation {
    fn default() -> Self {
        Self { span: 1, offset: 0 }
    }
}

/// Knife/cut operation.
#[derive(Debug, Clone)]
pub struct KnifeOperation {
    /// Cut points (screen space or 3D).
    pub points: Vec<Point3<f64>>,
    /// Cut through all geometry.
    pub cut_through: bool,
}

impl Default for KnifeOperation {
    fn default() -> Self {
        Self {
            points: Vec::new(),
            cut_through: true,
        }
    }
}

/// Loop cut operation.
#[derive(Debug, Clone)]
pub struct LoopCutOperation {
    /// Number of cuts.
    pub cuts: usize,
    /// Smoothness.
    pub smoothness: f64,
    /// Use even spacing.
    pub even: bool,
}

impl Default for LoopCutOperation {
    fn default() -> Self {
        Self {
            cuts: 1,
            smoothness: 0.0,
            even: true,
        }
    }
}

impl LoopCutOperation {
    /// Perform loop cut on selected edge ring.
    pub fn execute(
        &self,
        mesh: &mut EditMesh,
        edge_idx: usize,
    ) -> Result<Vec<usize>, &'static str> {
        if edge_idx >= mesh.edges.len() {
            return Err("Invalid edge");
        }

        // Find edge loop
        let edge_loop = find_edge_loop(mesh, edge_idx);
        if edge_loop.is_empty() {
            return Err("Could not find edge loop");
        }

        let mut new_vertices = Vec::new();

        for cut in 0..self.cuts {
            let t = (cut + 1) as f64 / (self.cuts + 1) as f64;

            for &eidx in &edge_loop {
                let (v0, v1) = mesh.edges[eidx];
                let p0 = mesh.positions[v0];
                let p1 = mesh.positions[v1];
                let cut_pos = Point3::from(p0.coords * (1.0 - t) + p1.coords * t);
                new_vertices.push(mesh.add_vertex(cut_pos));
            }
        }

        // Split faces along the cuts
        // (Simplified - full implementation would subdivide affected faces)

        mesh.rebuild_topology();
        mesh.compute_normals();
        Ok(new_vertices)
    }
}

/// Find edge loop starting from an edge.
fn find_edge_loop(mesh: &EditMesh, start_edge: usize) -> Vec<usize> {
    let mut loop_edges = vec![start_edge];
    let mut visited = HashSet::new();
    visited.insert(start_edge);

    // Traverse in one direction
    let mut current = start_edge;
    while let Some(next) = find_next_loop_edge(mesh, current, &visited) {
        visited.insert(next);
        loop_edges.push(next);
        current = next;
    }

    loop_edges
}

/// Find next edge in loop.
fn find_next_loop_edge(
    mesh: &EditMesh,
    edge_idx: usize,
    visited: &HashSet<usize>,
) -> Option<usize> {
    let (v0, v1) = mesh.edges[edge_idx];

    // For each face adjacent to this edge
    if let Some(faces) = mesh.edge_faces.get(&edge_idx) {
        for &face_idx in faces {
            let face = &mesh.faces[face_idx];

            // Find the opposite edge in quad faces
            if face.len() == 4 {
                // Find position of v0 and v1 in face
                let pos0 = face.iter().position(|&v| v == v0);
                let pos1 = face.iter().position(|&v| v == v1);

                if let (Some(p0), Some(p1)) = (pos0, pos1) {
                    // Opposite edge is 2 positions away
                    let opp0 = (p0 + 2) % 4;
                    let opp1 = (p1 + 2) % 4;

                    let ov0 = face[opp0];
                    let ov1 = face[opp1];

                    // Find this edge
                    let edge_key = if ov0 < ov1 { (ov0, ov1) } else { (ov1, ov0) };
                    for (idx, &e) in mesh.edges.iter().enumerate() {
                        if e == edge_key && !visited.contains(&idx) {
                            return Some(idx);
                        }
                    }
                }
            }
        }
    }

    None
}

/// Transform operation.
#[derive(Debug, Clone)]
pub struct TransformOperation {
    /// Transformation matrix.
    pub matrix: Matrix4<f64>,
}

impl TransformOperation {
    /// Create translation transform.
    pub fn translate(offset: Vector3<f64>) -> Self {
        Self {
            matrix: Matrix4::new_translation(&offset),
        }
    }

    /// Create rotation transform.
    pub fn rotate(axis: Vector3<f64>, angle: f64) -> Self {
        Self {
            matrix: Matrix4::from_axis_angle(&nalgebra::Unit::new_normalize(axis), angle),
        }
    }

    /// Create scale transform.
    pub fn scale(scale: Vector3<f64>) -> Self {
        Self {
            matrix: Matrix4::new_nonuniform_scaling(&scale),
        }
    }

    /// Apply transform to selected vertices.
    pub fn execute(&self, mesh: &mut EditMesh) {
        for &vi in &mesh.selection.vertices {
            let pos = mesh.positions[vi];
            let homogeneous = nalgebra::Vector4::new(pos.x, pos.y, pos.z, 1.0);
            let transformed = self.matrix * homogeneous;
            mesh.positions[vi] = Point3::new(transformed.x, transformed.y, transformed.z);
        }

        mesh.compute_normals();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_cube() -> EditMesh {
        let positions = vec![
            Point3::new(-1.0, -1.0, -1.0),
            Point3::new(1.0, -1.0, -1.0),
            Point3::new(1.0, 1.0, -1.0),
            Point3::new(-1.0, 1.0, -1.0),
            Point3::new(-1.0, -1.0, 1.0),
            Point3::new(1.0, -1.0, 1.0),
            Point3::new(1.0, 1.0, 1.0),
            Point3::new(-1.0, 1.0, 1.0),
        ];

        let faces = vec![
            vec![0, 1, 2, 3], // front
            vec![5, 4, 7, 6], // back
            vec![4, 0, 3, 7], // left
            vec![1, 5, 6, 2], // right
            vec![3, 2, 6, 7], // top
            vec![4, 5, 1, 0], // bottom
        ];

        EditMesh::from_mesh(positions, faces)
    }

    #[test]
    fn test_edit_mesh_creation() {
        let mesh = create_test_cube();
        assert_eq!(mesh.positions.len(), 8);
        assert_eq!(mesh.faces.len(), 6);
    }

    #[test]
    fn test_face_normal() {
        let mesh = create_test_cube();
        let normal = mesh.face_normal(0);
        // Face 0 [0,1,2,3] at z=-1 with CCW winding produces +Z normal
        assert!(normal.z > 0.0);
    }

    #[test]
    fn test_extrude() {
        let mut mesh = create_test_cube();
        mesh.selection.faces.insert(0);

        let op = ExtrudeOperation {
            amount: 1.0,
            along_normals: true,
            ..Default::default()
        };

        op.execute(&mut mesh).unwrap();
        assert!(mesh.positions.len() > 8);
    }

    #[test]
    fn test_inset() {
        let mut mesh = create_test_cube();
        mesh.selection.faces.insert(0);

        let op = InsetOperation {
            thickness: 0.2,
            depth: 0.0,
            ..Default::default()
        };

        op.execute(&mut mesh).unwrap();
        assert!(mesh.faces.len() > 6);
    }

    #[test]
    fn test_transform() {
        let mut mesh = create_test_cube();
        mesh.selection.vertices.insert(0);

        let initial_pos = mesh.positions[0];
        let op = TransformOperation::translate(Vector3::new(1.0, 0.0, 0.0));
        op.execute(&mut mesh);

        assert!((mesh.positions[0].x - initial_pos.x - 1.0).abs() < 1e-10);
    }
}
