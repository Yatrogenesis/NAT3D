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

//! Subdivision surfaces.
//!
//! Catmull-Clark and other subdivision surface algorithms.

use nalgebra::{Point3, Vector3};
use std::collections::HashMap;

/// Subdivision algorithm type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubdivisionType {
    /// Catmull-Clark subdivision (quad-based).
    CatmullClark,
    /// Loop subdivision (triangle-based).
    Loop,
    /// Simple (midpoint) subdivision.
    Simple,
    /// Doo-Sabin subdivision.
    DooSabin,
}

/// Subdivision settings.
#[derive(Debug, Clone)]
pub struct SubdivisionSettings {
    /// Subdivision algorithm.
    pub algorithm: SubdivisionType,
    /// Number of subdivision levels.
    pub levels: usize,
    /// Use smooth normals.
    pub smooth_normals: bool,
    /// Preserve sharp edges.
    pub preserve_sharp_edges: bool,
    /// Sharp edge threshold angle (radians).
    pub sharp_edge_angle: f64,
    /// Boundary interpolation mode.
    pub boundary_mode: BoundaryMode,
    /// UV interpolation mode.
    pub uv_mode: UvMode,
}

impl Default for SubdivisionSettings {
    fn default() -> Self {
        Self {
            algorithm: SubdivisionType::CatmullClark,
            levels: 1,
            smooth_normals: true,
            preserve_sharp_edges: false,
            sharp_edge_angle: std::f64::consts::PI / 6.0,
            boundary_mode: BoundaryMode::EdgeOnly,
            uv_mode: UvMode::Smooth,
        }
    }
}

/// Boundary interpolation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryMode {
    /// No special boundary treatment.
    None,
    /// Interpolate edges only.
    EdgeOnly,
    /// Interpolate edges and corners.
    EdgeAndCorner,
}

/// UV interpolation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UvMode {
    /// No UV interpolation.
    None,
    /// Smooth UV interpolation.
    Smooth,
    /// Sharp UV edges (pin boundaries).
    Sharp,
}

/// Simple mesh representation for subdivision.
#[derive(Debug, Clone)]
pub struct SubdivisionMesh {
    /// Vertex positions.
    pub positions: Vec<Point3<f64>>,
    /// Face indices (variable length faces).
    pub faces: Vec<Vec<usize>>,
    /// Vertex normals.
    pub normals: Vec<Vector3<f64>>,
    /// UV coordinates.
    pub uvs: Vec<(f64, f64)>,
    /// Edge sharpness values.
    pub edge_sharpness: HashMap<(usize, usize), f64>,
    /// Vertex sharpness values.
    pub vertex_sharpness: HashMap<usize, f64>,
}

impl SubdivisionMesh {
    /// Create an empty mesh.
    pub fn new() -> Self {
        Self {
            positions: Vec::new(),
            faces: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            edge_sharpness: HashMap::new(),
            vertex_sharpness: HashMap::new(),
        }
    }

    /// Add a vertex.
    pub fn add_vertex(&mut self, position: Point3<f64>) -> usize {
        let idx = self.positions.len();
        self.positions.push(position);
        idx
    }

    /// Add a face.
    pub fn add_face(&mut self, vertices: Vec<usize>) {
        self.faces.push(vertices);
    }

    /// Get vertex count.
    pub fn vertex_count(&self) -> usize {
        self.positions.len()
    }

    /// Get face count.
    pub fn face_count(&self) -> usize {
        self.faces.len()
    }

    /// Set edge sharpness.
    pub fn set_edge_sharpness(&mut self, v1: usize, v2: usize, sharpness: f64) {
        let key = if v1 < v2 { (v1, v2) } else { (v2, v1) };
        self.edge_sharpness.insert(key, sharpness);
    }

    /// Get edge sharpness.
    pub fn get_edge_sharpness(&self, v1: usize, v2: usize) -> f64 {
        let key = if v1 < v2 { (v1, v2) } else { (v2, v1) };
        self.edge_sharpness.get(&key).copied().unwrap_or(0.0)
    }

    /// Compute normals.
    pub fn compute_normals(&mut self) {
        self.normals = vec![Vector3::zeros(); self.positions.len()];

        for face in &self.faces {
            if face.len() < 3 {
                continue;
            }

            // Compute face normal
            let p0 = self.positions[face[0]];
            let p1 = self.positions[face[1]];
            let p2 = self.positions[face[2]];

            let edge1 = p1 - p0;
            let edge2 = p2 - p0;
            let normal = edge1.cross(&edge2);

            // Accumulate to vertices
            for &v in face {
                self.normals[v] += normal;
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
}

impl Default for SubdivisionMesh {
    fn default() -> Self {
        Self::new()
    }
}

/// Catmull-Clark subdivision surface.
pub struct CatmullClark;

impl CatmullClark {
    /// Subdivide a mesh.
    pub fn subdivide(mesh: &SubdivisionMesh, settings: &SubdivisionSettings) -> SubdivisionMesh {
        let mut result = mesh.clone();

        for _ in 0..settings.levels {
            result = Self::subdivide_once(&result, settings);
        }

        result.compute_normals();
        result
    }

    fn subdivide_once(mesh: &SubdivisionMesh, settings: &SubdivisionSettings) -> SubdivisionMesh {
        let mut result = SubdivisionMesh::new();

        // Build adjacency info
        let mut vertex_faces: Vec<Vec<usize>> = vec![Vec::new(); mesh.positions.len()];
        let mut edge_faces: HashMap<(usize, usize), Vec<usize>> = HashMap::new();

        for (face_idx, face) in mesh.faces.iter().enumerate() {
            for (i, &v) in face.iter().enumerate() {
                vertex_faces[v].push(face_idx);

                let v2 = face[(i + 1) % face.len()];
                let key = if v < v2 { (v, v2) } else { (v2, v) };
                edge_faces.entry(key).or_default().push(face_idx);
            }
        }

        // 1. Create face points
        let mut face_points: Vec<Point3<f64>> = Vec::new();
        for face in &mesh.faces {
            let centroid = Self::compute_centroid(face, &mesh.positions);
            face_points.push(centroid);
        }

        // 2. Create edge points
        let mut edge_points: HashMap<(usize, usize), usize> = HashMap::new();

        for (&(v1, v2), faces) in &edge_faces {
            let p1 = mesh.positions[v1];
            let p2 = mesh.positions[v2];

            let edge_point = if faces.len() == 2 {
                // Interior edge
                let f1 = face_points[faces[0]];
                let f2 = face_points[faces[1]];
                Point3::from((p1.coords + p2.coords + f1.coords + f2.coords) / 4.0)
            } else {
                // Boundary edge
                Point3::from((p1.coords + p2.coords) / 2.0)
            };

            let idx = result.add_vertex(edge_point);
            edge_points.insert((v1, v2), idx);
        }

        // 3. Create new vertex points
        let face_point_start = result.positions.len();
        for fp in &face_points {
            result.add_vertex(*fp);
        }

        let vertex_point_start = result.positions.len();
        for (v, original_pos) in mesh.positions.iter().enumerate() {
            let adjacent_faces = &vertex_faces[v];
            let n = adjacent_faces.len() as f64;

            if n == 0.0 {
                result.add_vertex(*original_pos);
                continue;
            }

            // Check if boundary vertex
            let is_boundary = Self::is_boundary_vertex(v, &edge_faces);

            let new_pos = if is_boundary {
                match settings.boundary_mode {
                    BoundaryMode::None => *original_pos,
                    BoundaryMode::EdgeOnly | BoundaryMode::EdgeAndCorner => {
                        // Average with adjacent boundary vertices
                        let boundary_neighbors = Self::get_boundary_neighbors(v, &edge_faces);
                        if boundary_neighbors.len() == 2 {
                            let p1 = mesh.positions[boundary_neighbors[0]];
                            let p2 = mesh.positions[boundary_neighbors[1]];
                            Point3::from((original_pos.coords + p1.coords + p2.coords) / 3.0)
                        } else {
                            *original_pos
                        }
                    }
                }
            } else {
                // Interior vertex: Catmull-Clark formula
                let face_avg: Vector3<f64> = adjacent_faces
                    .iter()
                    .map(|&f| face_points[f].coords)
                    .sum::<Vector3<f64>>()
                    / n;

                let edge_avg = Self::compute_edge_average(v, mesh, &edge_faces);

                let new_coords = (face_avg + 2.0 * edge_avg + (n - 3.0) * original_pos.coords) / n;
                Point3::from(new_coords)
            };

            result.add_vertex(new_pos);
        }

        // 4. Create new faces
        for (face_idx, face) in mesh.faces.iter().enumerate() {
            let fp_idx = face_point_start + face_idx;

            for i in 0..face.len() {
                let v = face[i];
                let v_next = face[(i + 1) % face.len()];
                let v_prev = face[(face.len() + i - 1) % face.len()];

                let vp_idx = vertex_point_start + v;

                let edge_key1 = if v < v_next { (v, v_next) } else { (v_next, v) };
                let edge_key2 = if v < v_prev { (v, v_prev) } else { (v_prev, v) };

                let ep1_idx = edge_points[&edge_key1];
                let ep2_idx = edge_points[&edge_key2];

                // New quad face
                result.add_face(vec![vp_idx, ep1_idx, fp_idx, ep2_idx]);
            }
        }

        result
    }

    fn compute_centroid(face: &[usize], positions: &[Point3<f64>]) -> Point3<f64> {
        let sum: Vector3<f64> = face.iter().map(|&v| positions[v].coords).sum();
        Point3::from(sum / face.len() as f64)
    }

    fn is_boundary_vertex(v: usize, edge_faces: &HashMap<(usize, usize), Vec<usize>>) -> bool {
        for (&(v1, v2), faces) in edge_faces {
            if (v1 == v || v2 == v) && faces.len() == 1 {
                return true;
            }
        }
        false
    }

    fn get_boundary_neighbors(
        v: usize,
        edge_faces: &HashMap<(usize, usize), Vec<usize>>,
    ) -> Vec<usize> {
        let mut neighbors = Vec::new();
        for (&(v1, v2), faces) in edge_faces {
            if faces.len() == 1 {
                if v1 == v {
                    neighbors.push(v2);
                } else if v2 == v {
                    neighbors.push(v1);
                }
            }
        }
        neighbors
    }

    fn compute_edge_average(
        v: usize,
        mesh: &SubdivisionMesh,
        edge_faces: &HashMap<(usize, usize), Vec<usize>>,
    ) -> Vector3<f64> {
        let mut sum = Vector3::zeros();
        let mut count = 0.0;

        for &(v1, v2) in edge_faces.keys() {
            if v1 == v {
                sum += mesh.positions[v2].coords;
                count += 1.0;
            } else if v2 == v {
                sum += mesh.positions[v1].coords;
                count += 1.0;
            }
        }

        if count > 0.0 {
            sum / count
        } else {
            mesh.positions[v].coords
        }
    }
}

/// Loop subdivision for triangle meshes.
pub struct LoopSubdivision;

impl LoopSubdivision {
    /// Subdivide a triangle mesh.
    pub fn subdivide(mesh: &SubdivisionMesh, levels: usize) -> SubdivisionMesh {
        let mut result = mesh.clone();

        for _ in 0..levels {
            result = Self::subdivide_once(&result);
        }

        result.compute_normals();
        result
    }

    fn subdivide_once(mesh: &SubdivisionMesh) -> SubdivisionMesh {
        let mut result = SubdivisionMesh::new();

        // Build edge map
        let mut edge_map: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
        for (face_idx, face) in mesh.faces.iter().enumerate() {
            if face.len() != 3 {
                continue; // Loop only works on triangles
            }
            for i in 0..3 {
                let v1 = face[i];
                let v2 = face[(i + 1) % 3];
                let key = if v1 < v2 { (v1, v2) } else { (v2, v1) };
                edge_map.entry(key).or_default().push(face_idx);
            }
        }

        // Create edge vertices
        let mut edge_vertices: HashMap<(usize, usize), usize> = HashMap::new();

        for (&(v1, v2), faces) in &edge_map {
            let p1 = mesh.positions[v1];
            let p2 = mesh.positions[v2];

            let new_pos = if faces.len() == 2 {
                // Interior edge: 3/8 * (A + B) + 1/8 * (C + D)
                // where A, B are edge vertices and C, D are opposite vertices
                let opposite = Self::get_opposite_vertices(v1, v2, faces, &mesh.faces);
                if opposite.len() == 2 {
                    let c = mesh.positions[opposite[0]];
                    let d = mesh.positions[opposite[1]];
                    Point3::from((3.0 * (p1.coords + p2.coords) + c.coords + d.coords) / 8.0)
                } else {
                    Point3::from((p1.coords + p2.coords) / 2.0)
                }
            } else {
                // Boundary edge
                Point3::from((p1.coords + p2.coords) / 2.0)
            };

            let idx = result.add_vertex(new_pos);
            edge_vertices.insert((v1, v2), idx);
        }

        // Update original vertices
        let vertex_start = result.positions.len();
        for (v, &pos) in mesh.positions.iter().enumerate() {
            let neighbors = Self::get_vertex_neighbors(v, &edge_map);
            let n = neighbors.len() as f64;

            if n == 0.0 {
                result.add_vertex(pos);
                continue;
            }

            // Beta coefficient
            let beta = if n == 3.0 {
                3.0 / 16.0
            } else {
                3.0 / (8.0 * n)
            };

            let neighbor_sum: Vector3<f64> =
                neighbors.iter().map(|&nv| mesh.positions[nv].coords).sum();

            let new_pos = (1.0 - n * beta) * pos.coords + beta * neighbor_sum;
            result.add_vertex(Point3::from(new_pos));
        }

        // Create new faces (4 triangles per original triangle)
        for face in &mesh.faces {
            if face.len() != 3 {
                continue;
            }

            let v0 = vertex_start + face[0];
            let v1 = vertex_start + face[1];
            let v2 = vertex_start + face[2];

            let e01 = Self::get_edge_vertex(&edge_vertices, face[0], face[1]);
            let e12 = Self::get_edge_vertex(&edge_vertices, face[1], face[2]);
            let e20 = Self::get_edge_vertex(&edge_vertices, face[2], face[0]);

            result.add_face(vec![v0, e01, e20]);
            result.add_face(vec![v1, e12, e01]);
            result.add_face(vec![v2, e20, e12]);
            result.add_face(vec![e01, e12, e20]);
        }

        result
    }

    fn get_opposite_vertices(
        v1: usize,
        v2: usize,
        faces: &[usize],
        all_faces: &[Vec<usize>],
    ) -> Vec<usize> {
        let mut opposite = Vec::new();
        for &face_idx in faces {
            let face = &all_faces[face_idx];
            for &v in face {
                if v != v1 && v != v2 {
                    opposite.push(v);
                }
            }
        }
        opposite
    }

    fn get_vertex_neighbors(
        v: usize,
        edge_map: &HashMap<(usize, usize), Vec<usize>>,
    ) -> Vec<usize> {
        let mut neighbors = Vec::new();
        for &(v1, v2) in edge_map.keys() {
            if v1 == v {
                neighbors.push(v2);
            } else if v2 == v {
                neighbors.push(v1);
            }
        }
        neighbors
    }

    fn get_edge_vertex(
        edge_vertices: &HashMap<(usize, usize), usize>,
        v1: usize,
        v2: usize,
    ) -> usize {
        let key = if v1 < v2 { (v1, v2) } else { (v2, v1) };
        edge_vertices[&key]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_quad_mesh() -> SubdivisionMesh {
        let mut mesh = SubdivisionMesh::new();
        mesh.add_vertex(Point3::new(0.0, 0.0, 0.0));
        mesh.add_vertex(Point3::new(1.0, 0.0, 0.0));
        mesh.add_vertex(Point3::new(1.0, 1.0, 0.0));
        mesh.add_vertex(Point3::new(0.0, 1.0, 0.0));
        mesh.add_face(vec![0, 1, 2, 3]);
        mesh
    }

    #[test]
    fn test_catmull_clark() {
        let mesh = create_quad_mesh();
        let settings = SubdivisionSettings::default();

        let result = CatmullClark::subdivide(&mesh, &settings);

        // One level of subdivision of a quad should produce 4 quads
        assert_eq!(result.face_count(), 4);
    }

    #[test]
    fn test_subdivision_mesh() {
        let mut mesh = SubdivisionMesh::new();
        let v1 = mesh.add_vertex(Point3::origin());
        let v2 = mesh.add_vertex(Point3::new(1.0, 0.0, 0.0));

        assert_eq!(mesh.vertex_count(), 2);
        assert_eq!(v1, 0);
        assert_eq!(v2, 1);
    }

    #[test]
    fn test_edge_sharpness() {
        let mut mesh = SubdivisionMesh::new();
        mesh.set_edge_sharpness(0, 1, 1.0);
        mesh.set_edge_sharpness(1, 0, 0.5); // Should normalize to same key

        assert_eq!(mesh.get_edge_sharpness(0, 1), 0.5);
        assert_eq!(mesh.get_edge_sharpness(1, 0), 0.5);
    }
}
