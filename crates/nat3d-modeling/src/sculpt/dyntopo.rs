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

//! Dynamic topology.
//!
//! Implements dynamic mesh tessellation during sculpting, automatically
//! subdividing and collapsing edges based on detail level.

use nalgebra::{Point3, Vector3};
use std::collections::{HashMap, HashSet};

/// Dynamic topology mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DyntopoMode {
    /// Subdivide edges that are too long.
    Subdivide,
    /// Collapse edges that are too short.
    Collapse,
    /// Both subdivide and collapse as needed.
    SubdivideCollapse,
    /// Constant detail - maintain uniform edge length.
    ConstantDetail,
    /// Relative detail - scale by brush size.
    RelativeDetail,
}

/// Dynamic topology settings.
#[derive(Debug, Clone)]
pub struct DyntopoSettings {
    /// Mode of operation.
    pub mode: DyntopoMode,
    /// Target edge length for constant detail.
    pub detail_size: f64,
    /// Relative detail multiplier.
    pub relative_detail: f64,
    /// Maximum edge length before subdivision.
    pub subdivide_threshold: f64,
    /// Minimum edge length before collapse.
    pub collapse_threshold: f64,
    /// Whether to smooth new vertices.
    pub smooth_shading: bool,
    /// Maximum iterations per stroke.
    pub max_iterations: usize,
}

impl Default for DyntopoSettings {
    fn default() -> Self {
        Self {
            mode: DyntopoMode::SubdivideCollapse,
            detail_size: 0.05,
            relative_detail: 0.25,
            subdivide_threshold: 1.5,
            collapse_threshold: 0.5,
            smooth_shading: true,
            max_iterations: 3,
        }
    }
}

/// Edge representation for dyntopo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Edge {
    /// First vertex index.
    pub v0: usize,
    /// Second vertex index.
    pub v1: usize,
}

impl Edge {
    /// Create a new edge with sorted vertices.
    pub fn new(a: usize, b: usize) -> Self {
        if a < b {
            Self { v0: a, v1: b }
        } else {
            Self { v0: b, v1: a }
        }
    }
}

/// Dynamic topology mesh.
pub struct DyntopoMesh {
    /// Vertex positions.
    pub positions: Vec<Point3<f64>>,
    /// Vertex normals.
    pub normals: Vec<Vector3<f64>>,
    /// Triangle indices.
    pub triangles: Vec<[usize; 3]>,
    /// Edge to face mapping.
    edge_faces: HashMap<Edge, Vec<usize>>,
    /// Vertex to face mapping.
    vertex_faces: Vec<HashSet<usize>>,
    /// Free vertex slots for reuse.
    free_vertices: Vec<usize>,
    /// Free triangle slots for reuse.
    free_triangles: Vec<usize>,
    /// Settings for dyntopo.
    pub settings: DyntopoSettings,
}

impl DyntopoMesh {
    /// Create a new dyntopo mesh.
    pub fn new(
        positions: Vec<Point3<f64>>,
        triangles: Vec<[usize; 3]>,
        settings: DyntopoSettings,
    ) -> Self {
        let n = positions.len();
        let normals = Self::compute_normals(&positions, &triangles);

        let mut mesh = Self {
            positions,
            normals,
            triangles,
            edge_faces: HashMap::new(),
            vertex_faces: vec![HashSet::new(); n],
            free_vertices: Vec::new(),
            free_triangles: Vec::new(),
            settings,
        };

        mesh.rebuild_topology();
        mesh
    }

    /// Rebuild topology maps.
    fn rebuild_topology(&mut self) {
        self.edge_faces.clear();
        for vf in &mut self.vertex_faces {
            vf.clear();
        }

        for (tri_idx, tri) in self.triangles.iter().enumerate() {
            if tri[0] == usize::MAX {
                continue;
            }

            for &v in tri {
                if v < self.vertex_faces.len() {
                    self.vertex_faces[v].insert(tri_idx);
                }
            }

            for i in 0..3 {
                let edge = Edge::new(tri[i], tri[(i + 1) % 3]);
                self.edge_faces.entry(edge).or_default().push(tri_idx);
            }
        }
    }

    /// Compute vertex normals.
    fn compute_normals(positions: &[Point3<f64>], triangles: &[[usize; 3]]) -> Vec<Vector3<f64>> {
        let mut normals = vec![Vector3::zeros(); positions.len()];

        for tri in triangles {
            if tri[0] == usize::MAX {
                continue;
            }

            let v0 = positions[tri[0]];
            let v1 = positions[tri[1]];
            let v2 = positions[tri[2]];

            let edge1 = v1 - v0;
            let edge2 = v2 - v0;
            let face_normal = edge1.cross(&edge2);

            for &v in tri {
                if v < normals.len() {
                    normals[v] += face_normal;
                }
            }
        }

        for n in &mut normals {
            let len = n.norm();
            if len > 1e-10 {
                *n /= len;
            }
        }

        normals
    }

    /// Update normals for affected vertices.
    pub fn update_normals(&mut self, vertices: &[usize]) {
        for &v in vertices {
            if v >= self.normals.len() {
                continue;
            }
            self.normals[v] = Vector3::zeros();
        }

        let affected_set: HashSet<usize> = vertices.iter().copied().collect();

        for tri in &self.triangles {
            if tri[0] == usize::MAX {
                continue;
            }

            let has_affected = tri.iter().any(|&v| affected_set.contains(&v));
            if !has_affected {
                continue;
            }

            let v0 = self.positions[tri[0]];
            let v1 = self.positions[tri[1]];
            let v2 = self.positions[tri[2]];

            let edge1 = v1 - v0;
            let edge2 = v2 - v0;
            let face_normal = edge1.cross(&edge2);

            for &v in tri {
                if affected_set.contains(&v) {
                    self.normals[v] += face_normal;
                }
            }
        }

        for &v in vertices {
            if v >= self.normals.len() {
                continue;
            }
            let len = self.normals[v].norm();
            if len > 1e-10 {
                self.normals[v] /= len;
            }
        }
    }

    /// Get edge length.
    fn edge_length(&self, edge: &Edge) -> f64 {
        let p0 = self.positions[edge.v0];
        let p1 = self.positions[edge.v1];
        (p1 - p0).norm()
    }

    /// Apply dyntopo in a region.
    pub fn apply_in_region(&mut self, center: Point3<f64>, radius: f64, brush_size: f64) {
        let detail = match self.settings.mode {
            DyntopoMode::ConstantDetail => self.settings.detail_size,
            DyntopoMode::RelativeDetail => brush_size * self.settings.relative_detail,
            _ => self.settings.detail_size,
        };

        let subdivide_len = detail * self.settings.subdivide_threshold;
        let collapse_len = detail * self.settings.collapse_threshold;

        for _ in 0..self.settings.max_iterations {
            let mut changed = false;

            let edges_in_region: Vec<Edge> = self
                .edge_faces
                .keys()
                .filter(|edge| {
                    let p0 = self.positions[edge.v0];
                    let p1 = self.positions[edge.v1];
                    let mid = Point3::new(
                        (p0.x + p1.x) / 2.0,
                        (p0.y + p1.y) / 2.0,
                        (p0.z + p1.z) / 2.0,
                    );
                    (mid - center).norm() <= radius
                })
                .copied()
                .collect();

            if matches!(
                self.settings.mode,
                DyntopoMode::Subdivide
                    | DyntopoMode::SubdivideCollapse
                    | DyntopoMode::ConstantDetail
                    | DyntopoMode::RelativeDetail
            ) {
                for edge in &edges_in_region {
                    if self.edge_length(edge) > subdivide_len && self.subdivide_edge(*edge) {
                        changed = true;
                    }
                }
            }

            if matches!(
                self.settings.mode,
                DyntopoMode::Collapse
                    | DyntopoMode::SubdivideCollapse
                    | DyntopoMode::ConstantDetail
                    | DyntopoMode::RelativeDetail
            ) {
                for edge in &edges_in_region {
                    if self.edge_faces.contains_key(edge)
                        && self.edge_length(edge) < collapse_len
                        && self.collapse_edge(*edge)
                    {
                        changed = true;
                    }
                }
            }

            if !changed {
                break;
            }
        }

        let affected: Vec<usize> = (0..self.positions.len())
            .filter(|&i| (self.positions[i] - center).norm() <= radius * 1.5)
            .collect();
        self.update_normals(&affected);
    }

    /// Subdivide an edge by inserting a vertex at its midpoint.
    fn subdivide_edge(&mut self, edge: Edge) -> bool {
        let faces = match self.edge_faces.get(&edge) {
            Some(f) if !f.is_empty() => f.clone(),
            _ => return false,
        };

        let p0 = self.positions[edge.v0];
        let p1 = self.positions[edge.v1];
        let mid = Point3::new(
            (p0.x + p1.x) / 2.0,
            (p0.y + p1.y) / 2.0,
            (p0.z + p1.z) / 2.0,
        );

        let n0 = self.normals[edge.v0];
        let n1 = self.normals[edge.v1];
        let mid_normal = (n0 + n1).normalize();

        let new_v = self.add_vertex(mid, mid_normal);

        for tri_idx in faces {
            if self.triangles[tri_idx][0] == usize::MAX {
                continue;
            }

            let tri = self.triangles[tri_idx];

            let third = tri.iter().find(|&&v| v != edge.v0 && v != edge.v1).copied();

            let third = match third {
                Some(v) => v,
                None => continue,
            };

            self.remove_triangle(tri_idx);
            self.add_triangle([edge.v0, new_v, third]);
            self.add_triangle([new_v, edge.v1, third]);
        }

        true
    }

    /// Collapse an edge by merging its vertices.
    fn collapse_edge(&mut self, edge: Edge) -> bool {
        if !self.can_collapse_edge(&edge) {
            return false;
        }

        let faces = match self.edge_faces.get(&edge) {
            Some(f) => f.clone(),
            None => return false,
        };

        let p0 = self.positions[edge.v0];
        let p1 = self.positions[edge.v1];
        let mid = Point3::new(
            (p0.x + p1.x) / 2.0,
            (p0.y + p1.y) / 2.0,
            (p0.z + p1.z) / 2.0,
        );

        self.positions[edge.v0] = mid;

        for tri_idx in &faces {
            self.remove_triangle(*tri_idx);
        }

        let v1_faces: Vec<usize> = self.vertex_faces[edge.v1].iter().copied().collect();
        for tri_idx in v1_faces {
            if self.triangles[tri_idx][0] == usize::MAX {
                continue;
            }

            for i in 0..3 {
                if self.triangles[tri_idx][i] == edge.v1 {
                    self.triangles[tri_idx][i] = edge.v0;
                }
            }

            self.vertex_faces[edge.v0].insert(tri_idx);
        }

        self.vertex_faces[edge.v1].clear();
        self.free_vertices.push(edge.v1);
        self.rebuild_topology();

        true
    }

    /// Check if an edge can be safely collapsed.
    fn can_collapse_edge(&self, edge: &Edge) -> bool {
        let neighbors_0: HashSet<usize> = self.vertex_faces[edge.v0]
            .iter()
            .flat_map(|&tri_idx| {
                if self.triangles[tri_idx][0] == usize::MAX {
                    return vec![];
                }
                self.triangles[tri_idx].to_vec()
            })
            .filter(|&v| v != edge.v0 && v != edge.v1)
            .collect();

        let neighbors_1: HashSet<usize> = self.vertex_faces[edge.v1]
            .iter()
            .flat_map(|&tri_idx| {
                if self.triangles[tri_idx][0] == usize::MAX {
                    return vec![];
                }
                self.triangles[tri_idx].to_vec()
            })
            .filter(|&v| v != edge.v0 && v != edge.v1)
            .collect();

        let common: HashSet<_> = neighbors_0.intersection(&neighbors_1).collect();
        common.len() <= 2
    }

    /// Add a new vertex.
    fn add_vertex(&mut self, position: Point3<f64>, normal: Vector3<f64>) -> usize {
        if let Some(idx) = self.free_vertices.pop() {
            self.positions[idx] = position;
            self.normals[idx] = normal;
            idx
        } else {
            let idx = self.positions.len();
            self.positions.push(position);
            self.normals.push(normal);
            self.vertex_faces.push(HashSet::new());
            idx
        }
    }

    /// Add a new triangle.
    fn add_triangle(&mut self, vertices: [usize; 3]) -> usize {
        let idx = if let Some(idx) = self.free_triangles.pop() {
            self.triangles[idx] = vertices;
            idx
        } else {
            let idx = self.triangles.len();
            self.triangles.push(vertices);
            idx
        };

        for &v in &vertices {
            if v < self.vertex_faces.len() {
                self.vertex_faces[v].insert(idx);
            }
        }

        for i in 0..3 {
            let edge = Edge::new(vertices[i], vertices[(i + 1) % 3]);
            self.edge_faces.entry(edge).or_default().push(idx);
        }

        idx
    }

    /// Remove a triangle.
    fn remove_triangle(&mut self, idx: usize) {
        let tri = self.triangles[idx];
        if tri[0] == usize::MAX {
            return;
        }

        for &v in &tri {
            if v < self.vertex_faces.len() {
                self.vertex_faces[v].remove(&idx);
            }
        }

        for i in 0..3 {
            let edge = Edge::new(tri[i], tri[(i + 1) % 3]);
            if let Some(faces) = self.edge_faces.get_mut(&edge) {
                faces.retain(|&f| f != idx);
                if faces.is_empty() {
                    self.edge_faces.remove(&edge);
                }
            }
        }

        self.triangles[idx] = [usize::MAX, usize::MAX, usize::MAX];
        self.free_triangles.push(idx);
    }

    /// Get compact mesh (remove deleted elements).
    pub fn compact(&self) -> (Vec<Point3<f64>>, Vec<[usize; 3]>) {
        let mut vertex_map = HashMap::new();
        let mut new_positions = Vec::new();

        for tri in &self.triangles {
            if tri[0] == usize::MAX {
                continue;
            }

            for &v in tri {
                if let std::collections::hash_map::Entry::Vacant(e) = vertex_map.entry(v) {
                    let new_idx = new_positions.len();
                    e.insert(new_idx);
                    new_positions.push(self.positions[v]);
                }
            }
        }

        let new_triangles: Vec<[usize; 3]> = self
            .triangles
            .iter()
            .filter(|tri| tri[0] != usize::MAX)
            .map(|tri| {
                [
                    vertex_map[&tri[0]],
                    vertex_map[&tri[1]],
                    vertex_map[&tri[2]],
                ]
            })
            .collect();

        (new_positions, new_triangles)
    }

    /// Get statistics.
    pub fn stats(&self) -> DyntopoStats {
        let active_triangles = self
            .triangles
            .iter()
            .filter(|tri| tri[0] != usize::MAX)
            .count();

        let active_vertices = self.positions.len() - self.free_vertices.len();

        let edge_lengths: Vec<f64> = self
            .edge_faces
            .keys()
            .map(|e| self.edge_length(e))
            .collect();

        let avg_edge_length = if edge_lengths.is_empty() {
            0.0
        } else {
            edge_lengths.iter().sum::<f64>() / edge_lengths.len() as f64
        };

        let min_edge_length = edge_lengths.iter().copied().fold(f64::MAX, f64::min);
        let max_edge_length = edge_lengths.iter().copied().fold(0.0, f64::max);

        DyntopoStats {
            vertex_count: active_vertices,
            triangle_count: active_triangles,
            edge_count: self.edge_faces.len(),
            avg_edge_length,
            min_edge_length,
            max_edge_length,
        }
    }
}

/// Statistics about dyntopo mesh.
#[derive(Debug, Clone)]
pub struct DyntopoStats {
    /// Number of active vertices.
    pub vertex_count: usize,
    /// Number of active triangles.
    pub triangle_count: usize,
    /// Number of edges.
    pub edge_count: usize,
    /// Average edge length.
    pub avg_edge_length: f64,
    /// Minimum edge length.
    pub min_edge_length: f64,
    /// Maximum edge length.
    pub max_edge_length: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dyntopo_creation() {
        let positions = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.5, 1.0, 0.0),
        ];
        let triangles = vec![[0, 1, 2]];

        let mesh = DyntopoMesh::new(positions, triangles, DyntopoSettings::default());
        let stats = mesh.stats();

        assert_eq!(stats.vertex_count, 3);
        assert_eq!(stats.triangle_count, 1);
        assert_eq!(stats.edge_count, 3);
    }

    #[test]
    fn test_edge_ordering() {
        let e1 = Edge::new(1, 2);
        let e2 = Edge::new(2, 1);
        assert_eq!(e1, e2);
    }

    #[test]
    fn test_subdivide() {
        let positions = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
            Point3::new(1.0, 2.0, 0.0),
        ];
        let triangles = vec![[0, 1, 2]];

        let mut settings = DyntopoSettings::default();
        settings.detail_size = 0.5;
        settings.subdivide_threshold = 1.0;

        let mut mesh = DyntopoMesh::new(positions, triangles, settings);
        mesh.apply_in_region(Point3::new(1.0, 1.0, 0.0), 3.0, 1.0);

        let stats = mesh.stats();
        assert!(stats.vertex_count > 3);
        assert!(stats.triangle_count > 1);
    }

    #[test]
    fn test_compact() {
        let positions = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.5, 1.0, 0.0),
        ];
        let triangles = vec![[0, 1, 2]];

        let mesh = DyntopoMesh::new(positions, triangles, DyntopoSettings::default());
        let (new_pos, new_tri) = mesh.compact();

        assert_eq!(new_pos.len(), 3);
        assert_eq!(new_tri.len(), 1);
    }
}
