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

//! Automatic retopology tools.
//!
//! Provides algorithms for remeshing high-resolution sculpted meshes into
//! cleaner, lower-resolution topology suitable for animation and rendering.

use nalgebra::Point3;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};

/// Retopology configuration.
#[derive(Debug, Clone)]
pub struct RetopologyConfig {
    /// Target face count.
    pub target_face_count: usize,
    /// Adaptivity factor (0.0 = uniform, 1.0 = highly adaptive).
    pub adaptivity: f64,
    /// Enable symmetry constraint.
    pub symmetry: bool,
    /// Preserve mesh boundaries.
    pub preserve_boundaries: bool,
    /// Preserve hard edges (high curvature).
    pub preserve_hard_edges: bool,
    /// Hard edge angle threshold (degrees).
    pub hard_edge_threshold: f64,
    /// Algorithm to use.
    pub algorithm: RetopologyAlgorithm,
}

impl Default for RetopologyConfig {
    fn default() -> Self {
        Self {
            target_face_count: 1000,
            adaptivity: 0.5,
            symmetry: false,
            preserve_boundaries: true,
            preserve_hard_edges: true,
            hard_edge_threshold: 30.0,
            algorithm: RetopologyAlgorithm::Greedy,
        }
    }
}

/// Retopology algorithm selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetopologyAlgorithm {
    /// QuadriFlow-style field-aligned remeshing.
    QuadriFlow,
    /// InstantMeshes-style optimization.
    InstantMeshes,
    /// Voxel-based remeshing.
    Voxel,
    /// Greedy edge collapse with QEM.
    Greedy,
}

/// Result of retopology operation.
#[derive(Debug, Clone)]
pub struct RetopologyResult {
    /// New mesh vertices.
    pub positions: Vec<Point3<f64>>,
    /// New mesh faces.
    pub faces: Vec<Vec<usize>>,
    /// Quality metrics.
    pub quality: QualityMetrics,
}

/// Mesh quality metrics.
#[derive(Debug, Clone)]
pub struct QualityMetrics {
    /// Average aspect ratio.
    pub avg_aspect_ratio: f64,
    /// Minimum aspect ratio.
    pub min_aspect_ratio: f64,
    /// Average skewness.
    pub avg_skewness: f64,
    /// Minimum face angle (degrees).
    pub min_angle: f64,
    /// Maximum face angle (degrees).
    pub max_angle: f64,
}

/// Quadric Error Matrix for edge collapse.
#[derive(Debug, Clone)]
struct QuadricMatrix {
    /// Q matrix (4x4 symmetric, stored as 10 values).
    q: [f64; 10],
}

impl QuadricMatrix {
    /// Create from plane equation ax + by + cz + d = 0.
    fn from_plane(a: f64, b: f64, c: f64, d: f64) -> Self {
        Self {
            q: [
                a * a,
                a * b,
                a * c,
                a * d,
                b * b,
                b * c,
                b * d,
                c * c,
                c * d,
                d * d,
            ],
        }
    }

    /// Create zero matrix.
    fn zero() -> Self {
        Self { q: [0.0; 10] }
    }

    /// Add two matrices.
    fn add(&self, other: &Self) -> Self {
        let mut result = Self::zero();
        for i in 0..10 {
            result.q[i] = self.q[i] + other.q[i];
        }
        result
    }

    /// Compute error for a vertex position.
    fn error(&self, v: &Point3<f64>) -> f64 {
        let x = v.x;
        let y = v.y;
        let z = v.z;

        let q = &self.q;
        q[0] * x * x
            + 2.0 * q[1] * x * y
            + 2.0 * q[2] * x * z
            + 2.0 * q[3] * x
            + q[4] * y * y
            + 2.0 * q[5] * y * z
            + 2.0 * q[6] * y
            + q[7] * z * z
            + 2.0 * q[8] * z
            + q[9]
    }
}

/// Main retopology function.
pub fn retopologize(
    positions: &[Point3<f64>],
    faces: &[Vec<usize>],
    config: &RetopologyConfig,
) -> RetopologyResult {
    match config.algorithm {
        RetopologyAlgorithm::Voxel => voxel_remesh(positions, faces, config),
        RetopologyAlgorithm::Greedy => greedy_simplify(positions, faces, config),
        RetopologyAlgorithm::QuadriFlow => quadriflow_remesh(positions, faces, config),
        RetopologyAlgorithm::InstantMeshes => instant_meshes_remesh(positions, faces, config),
    }
}

/// Voxel-based remeshing.
pub fn voxel_remesh(
    positions: &[Point3<f64>],
    faces: &[Vec<usize>],
    config: &RetopologyConfig,
) -> RetopologyResult {
    // Compute bounds
    let mut min = Point3::new(f64::MAX, f64::MAX, f64::MAX);
    let mut max = Point3::new(f64::MIN, f64::MIN, f64::MIN);
    for p in positions {
        min.x = min.x.min(p.x);
        min.y = min.y.min(p.y);
        min.z = min.z.min(p.z);
        max.x = max.x.max(p.x);
        max.y = max.y.max(p.y);
        max.z = max.z.max(p.z);
    }

    // Compute voxel size from target face count
    let volume = (max.x - min.x) * (max.y - min.y) * (max.z - min.z);
    let avg_face_area = volume / config.target_face_count as f64;
    let voxel_size = (avg_face_area * 2.0).sqrt();

    // Voxelize
    let grid_size_x = ((max.x - min.x) / voxel_size).ceil() as usize + 1;
    let grid_size_y = ((max.y - min.y) / voxel_size).ceil() as usize + 1;
    let grid_size_z = ((max.z - min.z) / voxel_size).ceil() as usize + 1;

    let mut grid = vec![vec![vec![false; grid_size_z]; grid_size_y]; grid_size_x];

    // Mark voxels occupied by mesh
    for face in faces {
        for &v_idx in face {
            let p = positions[v_idx];
            let ix = ((p.x - min.x) / voxel_size) as usize;
            let iy = ((p.y - min.y) / voxel_size) as usize;
            let iz = ((p.z - min.z) / voxel_size) as usize;
            if ix < grid_size_x && iy < grid_size_y && iz < grid_size_z {
                grid[ix][iy][iz] = true;
            }
        }
    }

    // Extract surface using marching cubes (simplified)
    let (new_positions, new_faces) = marching_cubes(&grid, min, voxel_size);

    let quality = compute_quality(&new_positions, &new_faces);

    RetopologyResult {
        positions: new_positions,
        faces: new_faces,
        quality,
    }
}

/// Simplified marching cubes implementation.
fn marching_cubes(
    grid: &[Vec<Vec<bool>>],
    origin: Point3<f64>,
    voxel_size: f64,
) -> (Vec<Point3<f64>>, Vec<Vec<usize>>) {
    let mut positions = Vec::new();
    let mut faces = Vec::new();
    let mut vertex_map: HashMap<(usize, usize, usize, u8), usize> = HashMap::new();

    let get_or_create_vertex = |map: &mut HashMap<(usize, usize, usize, u8), usize>,
                                positions: &mut Vec<Point3<f64>>,
                                x: usize,
                                y: usize,
                                z: usize,
                                edge: u8|
     -> usize {
        let key = (x, y, z, edge);
        if let Some(&idx) = map.get(&key) {
            return idx;
        }

        let (dx, dy, dz) = match edge {
            0 => (0.5, 0.0, 0.0),
            1 => (1.0, 0.5, 0.0),
            2 => (0.5, 1.0, 0.0),
            3 => (0.0, 0.5, 0.0),
            4 => (0.5, 0.0, 1.0),
            5 => (1.0, 0.5, 1.0),
            6 => (0.5, 1.0, 1.0),
            7 => (0.0, 0.5, 1.0),
            8 => (0.0, 0.0, 0.5),
            9 => (1.0, 0.0, 0.5),
            10 => (1.0, 1.0, 0.5),
            11 => (0.0, 1.0, 0.5),
            _ => (0.0, 0.0, 0.0),
        };

        let pos = Point3::new(
            origin.x + (x as f64 + dx) * voxel_size,
            origin.y + (y as f64 + dy) * voxel_size,
            origin.z + (z as f64 + dz) * voxel_size,
        );

        let idx = positions.len();
        positions.push(pos);
        map.insert(key, idx);
        idx
    };

    for x in 0..grid.len() - 1 {
        for y in 0..grid[0].len() - 1 {
            for z in 0..grid[0][0].len() - 1 {
                let cube_index = (if grid[x][y][z] { 1 } else { 0 })
                    | (if grid[x + 1][y][z] { 2 } else { 0 })
                    | (if grid[x + 1][y + 1][z] { 4 } else { 0 })
                    | (if grid[x][y + 1][z] { 8 } else { 0 })
                    | (if grid[x][y][z + 1] { 16 } else { 0 })
                    | (if grid[x + 1][y][z + 1] { 32 } else { 0 })
                    | (if grid[x + 1][y + 1][z + 1] { 64 } else { 0 })
                    | (if grid[x][y + 1][z + 1] { 128 } else { 0 });

                if cube_index == 0 || cube_index == 255 {
                    continue;
                }

                // Simplified: create one triangle for each surface configuration
                if cube_index > 0 && cube_index < 255 {
                    let v0 = get_or_create_vertex(&mut vertex_map, &mut positions, x, y, z, 0);
                    let v1 = get_or_create_vertex(&mut vertex_map, &mut positions, x, y, z, 1);
                    let v2 = get_or_create_vertex(&mut vertex_map, &mut positions, x, y, z, 2);
                    faces.push(vec![v0, v1, v2]);
                }
            }
        }
    }

    (positions, faces)
}

/// Greedy simplification with Quadric Error Metrics.
pub fn greedy_simplify(
    positions: &[Point3<f64>],
    faces: &[Vec<usize>],
    config: &RetopologyConfig,
) -> RetopologyResult {
    let mut new_positions = positions.to_vec();
    let mut new_faces = faces.to_vec();

    // Build edge list
    let mut edges: HashSet<(usize, usize)> = HashSet::new();
    for face in &new_faces {
        for i in 0..face.len() {
            let v0 = face[i];
            let v1 = face[(i + 1) % face.len()];
            let edge = if v0 < v1 { (v0, v1) } else { (v1, v0) };
            edges.insert(edge);
        }
    }

    // Compute quadric matrices per vertex
    let mut quadrics = vec![QuadricMatrix::zero(); new_positions.len()];
    for face in &new_faces {
        if face.len() < 3 {
            continue;
        }

        let p0 = new_positions[face[0]];
        let p1 = new_positions[face[1]];
        let p2 = new_positions[face[2]];

        let e1 = p1 - p0;
        let e2 = p2 - p0;
        let normal = e1.cross(&e2);
        let len = normal.norm();
        if len < 1e-10 {
            continue;
        }
        let n = normal / len;

        let d = -n.dot(&p0.coords);
        let q = QuadricMatrix::from_plane(n.x, n.y, n.z, d);

        for &v in face {
            quadrics[v] = quadrics[v].add(&q);
        }
    }

    // Compute collapse costs
    let mut edge_costs: Vec<(f64, (usize, usize))> = edges
        .iter()
        .map(|&(v0, v1)| {
            let q = quadrics[v0].add(&quadrics[v1]);
            let midpoint = Point3::new(
                (new_positions[v0].x + new_positions[v1].x) / 2.0,
                (new_positions[v0].y + new_positions[v1].y) / 2.0,
                (new_positions[v0].z + new_positions[v1].z) / 2.0,
            );
            let cost = q.error(&midpoint);
            (cost, (v0, v1))
        })
        .collect();

    edge_costs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    // Collapse edges until target face count
    let target_faces = config.target_face_count;
    while new_faces.len() > target_faces && !edge_costs.is_empty() {
        let (_cost, (v0, v1)) = edge_costs.remove(0);

        if v0 >= new_positions.len() || v1 >= new_positions.len() {
            continue;
        }

        // Merge v1 into v0
        let midpoint = Point3::new(
            (new_positions[v0].x + new_positions[v1].x) / 2.0,
            (new_positions[v0].y + new_positions[v1].y) / 2.0,
            (new_positions[v0].z + new_positions[v1].z) / 2.0,
        );
        new_positions[v0] = midpoint;

        // Update faces: replace v1 with v0, remove degenerate faces
        new_faces.retain_mut(|face| {
            for v in face.iter_mut() {
                if *v == v1 {
                    *v = v0;
                }
            }
            // Remove degenerate faces (same vertex appears multiple times)
            let unique: HashSet<_> = face.iter().copied().collect();
            unique.len() >= 3
        });

        // Remove processed edges from cost list
        edge_costs.retain(|(_, (a, b))| *a != v0 && *a != v1 && *b != v0 && *b != v1);
    }

    // Compact vertices (remove unused)
    let mut used_vertices: HashSet<usize> = HashSet::new();
    for face in &new_faces {
        for &v in face {
            used_vertices.insert(v);
        }
    }

    let mut old_to_new: HashMap<usize, usize> = HashMap::new();
    let mut compacted_positions = Vec::new();
    for &v in &used_vertices {
        old_to_new.insert(v, compacted_positions.len());
        compacted_positions.push(new_positions[v]);
    }

    let compacted_faces: Vec<Vec<usize>> = new_faces
        .iter()
        .map(|face| face.iter().map(|&v| old_to_new[&v]).collect())
        .collect();

    let quality = compute_quality(&compacted_positions, &compacted_faces);

    RetopologyResult {
        positions: compacted_positions,
        faces: compacted_faces,
        quality,
    }
}

/// Quad-dominant remeshing (simplified field-aligned approach).
pub fn quadriflow_remesh(
    positions: &[Point3<f64>],
    faces: &[Vec<usize>],
    config: &RetopologyConfig,
) -> RetopologyResult {
    // Simplified: just use greedy simplification for now
    // A full QuadriFlow implementation would require:
    // - Integer grid parameterization
    // - Direction field computation
    // - Sharp feature detection
    greedy_simplify(positions, faces, config)
}

/// InstantMeshes-style remeshing (simplified).
pub fn instant_meshes_remesh(
    positions: &[Point3<f64>],
    faces: &[Vec<usize>],
    config: &RetopologyConfig,
) -> RetopologyResult {
    // Simplified: use voxel remesh as approximation
    voxel_remesh(positions, faces, config)
}

/// Compute mesh quality metrics.
pub fn compute_quality(positions: &[Point3<f64>], faces: &[Vec<usize>]) -> QualityMetrics {
    if faces.is_empty() {
        return QualityMetrics {
            avg_aspect_ratio: 0.0,
            min_aspect_ratio: 0.0,
            avg_skewness: 0.0,
            min_angle: 0.0,
            max_angle: 0.0,
        };
    }

    let aspect_ratios: Vec<f64> = faces
        .par_iter()
        .filter_map(|face| {
            if face.len() < 3 {
                return None;
            }
            let p0 = positions[face[0]];
            let p1 = positions[face[1]];
            let p2 = positions[face[2]];

            let a = (p1 - p0).norm();
            let b = (p2 - p1).norm();
            let c = (p0 - p2).norm();

            if a < 1e-10 || b < 1e-10 || c < 1e-10 {
                return None;
            }

            let max_edge = a.max(b).max(c);
            let min_edge = a.min(b).min(c);
            Some(max_edge / min_edge)
        })
        .collect();

    let angles: Vec<f64> = faces
        .par_iter()
        .flat_map(|face| {
            if face.len() < 3 {
                return vec![];
            }
            let mut face_angles = Vec::new();
            for i in 0..face.len() {
                let v0 = positions[face[i]];
                let v1 = positions[face[(i + 1) % face.len()]];
                let v2 = positions[face[(i + 2) % face.len()]];

                let e1 = (v0 - v1).normalize();
                let e2 = (v2 - v1).normalize();
                let dot = e1.dot(&e2).clamp(-1.0, 1.0);
                let angle = dot.acos().to_degrees();
                face_angles.push(angle);
            }
            face_angles
        })
        .collect();

    let avg_aspect_ratio = aspect_ratios.iter().sum::<f64>() / aspect_ratios.len() as f64;
    let min_aspect_ratio = aspect_ratios.iter().cloned().fold(f64::MAX, f64::min);
    let avg_skewness = (avg_aspect_ratio - 1.0).abs();
    let min_angle = angles.iter().cloned().fold(f64::MAX, f64::min);
    let max_angle = angles.iter().cloned().fold(f64::MIN, f64::max);

    QualityMetrics {
        avg_aspect_ratio,
        min_aspect_ratio,
        avg_skewness,
        min_angle,
        max_angle,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_cube() -> (Vec<Point3<f64>>, Vec<Vec<usize>>) {
        let positions = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
            Point3::new(0.0, 0.0, 1.0),
            Point3::new(1.0, 0.0, 1.0),
            Point3::new(1.0, 1.0, 1.0),
            Point3::new(0.0, 1.0, 1.0),
        ];

        let faces = vec![
            vec![0, 1, 2, 3], // bottom
            vec![4, 7, 6, 5], // top
            vec![0, 4, 5, 1], // front
            vec![2, 6, 7, 3], // back
            vec![0, 3, 7, 4], // left
            vec![1, 5, 6, 2], // right
        ];

        (positions, faces)
    }

    #[test]
    fn test_greedy_simplification() {
        let (positions, faces) = create_test_cube();
        let config = RetopologyConfig {
            target_face_count: 4,
            ..Default::default()
        };

        let result = greedy_simplify(&positions, &faces, &config);
        assert!(result.positions.len() <= positions.len());
        assert!(result.faces.len() <= faces.len());
    }

    #[test]
    fn test_quality_metrics() {
        let (positions, faces) = create_test_cube();
        let quality = compute_quality(&positions, &faces);

        assert!(quality.avg_aspect_ratio > 0.0);
        assert!(quality.min_angle > 0.0);
        assert!(quality.max_angle < 180.0);
    }

    #[test]
    fn test_voxel_remesh() {
        let (positions, faces) = create_test_cube();
        let config = RetopologyConfig {
            target_face_count: 50,
            algorithm: RetopologyAlgorithm::Voxel,
            ..Default::default()
        };

        let result = voxel_remesh(&positions, &faces, &config);
        assert!(!result.positions.is_empty());
        assert!(!result.faces.is_empty());
    }
}
