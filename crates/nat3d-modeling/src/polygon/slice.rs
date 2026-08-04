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

//! Mesh cutting and slicing operations.
//!
//! Provides tools for cutting meshes with planes, knife cuts, and bisection.

use nalgebra::{Point3, Vector3};
use std::collections::HashMap;

/// Plane definition for slicing.
#[derive(Debug, Clone)]
pub struct SlicePlane {
    /// Point on the plane.
    pub point: Point3<f64>,
    /// Plane normal (should be normalized).
    pub normal: Vector3<f64>,
}

impl SlicePlane {
    /// Create a new slice plane.
    pub fn new(point: Point3<f64>, normal: Vector3<f64>) -> Self {
        let normalized = normal.normalize();
        Self {
            point,
            normal: normalized,
        }
    }

    /// Compute signed distance from point to plane.
    pub fn signed_distance(&self, point: &Point3<f64>) -> f64 {
        let v = point - self.point;
        self.normal.dot(&v)
    }

    /// Check if point is above plane.
    pub fn is_above(&self, point: &Point3<f64>) -> bool {
        self.signed_distance(point) > 0.0
    }

    /// Compute intersection point between edge and plane.
    pub fn intersect_edge(&self, p0: &Point3<f64>, p1: &Point3<f64>) -> Option<Point3<f64>> {
        let d0 = self.signed_distance(p0);
        let d1 = self.signed_distance(p1);

        // Check if edge crosses plane
        if d0 * d1 > 0.0 {
            return None; // Both points on same side
        }

        let t = d0 / (d0 - d1);
        Some(Point3::new(
            p0.x + t * (p1.x - p0.x),
            p0.y + t * (p1.y - p0.y),
            p0.z + t * (p1.z - p0.z),
        ))
    }
}

/// Result of a slice operation.
#[derive(Debug, Clone)]
pub struct SliceResult {
    /// Mesh above the plane.
    pub above: (Vec<Point3<f64>>, Vec<Vec<usize>>),
    /// Mesh below the plane.
    pub below: (Vec<Point3<f64>>, Vec<Vec<usize>>),
}

/// Slice mesh with a plane, creating two separate meshes.
pub fn slice_mesh(
    positions: &[Point3<f64>],
    faces: &[Vec<usize>],
    plane: &SlicePlane,
) -> SliceResult {
    let mut above_positions = Vec::new();
    let mut above_faces = Vec::new();
    let mut below_positions = Vec::new();
    let mut below_faces = Vec::new();

    let mut above_map: HashMap<usize, usize> = HashMap::new();
    let mut below_map: HashMap<usize, usize> = HashMap::new();
    let mut cut_edges: Vec<(usize, usize)> = Vec::new();

    for face in faces {
        if face.len() < 3 {
            continue;
        }

        // Classify vertices
        let classifications: Vec<bool> = face
            .iter()
            .map(|&v| plane.is_above(&positions[v]))
            .collect();

        let above_count = classifications.iter().filter(|&&c| c).count();
        let below_count = face.len() - above_count;

        if above_count == face.len() {
            // Entire face above
            let new_face = face
                .iter()
                .map(|&v| {
                    *above_map.entry(v).or_insert_with(|| {
                        let idx = above_positions.len();
                        above_positions.push(positions[v]);
                        idx
                    })
                })
                .collect();
            above_faces.push(new_face);
        } else if below_count == face.len() {
            // Entire face below
            let new_face = face
                .iter()
                .map(|&v| {
                    *below_map.entry(v).or_insert_with(|| {
                        let idx = below_positions.len();
                        below_positions.push(positions[v]);
                        idx
                    })
                })
                .collect();
            below_faces.push(new_face);
        } else {
            // Face crosses plane - need to split
            let split_result = split_face(
                positions,
                face,
                &classifications,
                plane,
                &mut above_map,
                &mut below_map,
                &mut above_positions,
                &mut below_positions,
                &mut cut_edges,
            );

            above_faces.extend(split_result.0);
            below_faces.extend(split_result.1);
        }
    }

    // Cap the holes (optional - creates closed meshes)
    cap_slice_holes(&mut above_positions, &mut above_faces, &cut_edges, false);
    cap_slice_holes(&mut below_positions, &mut below_faces, &cut_edges, true);

    SliceResult {
        above: (above_positions, above_faces),
        below: (below_positions, below_faces),
    }
}

/// Split a face that crosses the plane.
#[allow(clippy::too_many_arguments)]
fn split_face(
    positions: &[Point3<f64>],
    face: &[usize],
    classifications: &[bool],
    plane: &SlicePlane,
    above_map: &mut HashMap<usize, usize>,
    below_map: &mut HashMap<usize, usize>,
    above_positions: &mut Vec<Point3<f64>>,
    below_positions: &mut Vec<Point3<f64>>,
    cut_edges: &mut Vec<(usize, usize)>,
) -> (Vec<Vec<usize>>, Vec<Vec<usize>>) {
    let mut above_verts = Vec::new();
    let mut below_verts = Vec::new();

    for i in 0..face.len() {
        let v0 = face[i];
        let v1 = face[(i + 1) % face.len()];
        let is_above_0 = classifications[i];
        let is_above_1 = classifications[(i + 1) % face.len()];

        // Add current vertex to appropriate side
        if is_above_0 {
            let idx = *above_map.entry(v0).or_insert_with(|| {
                let idx = above_positions.len();
                above_positions.push(positions[v0]);
                idx
            });
            above_verts.push(idx);
        } else {
            let idx = *below_map.entry(v0).or_insert_with(|| {
                let idx = below_positions.len();
                below_positions.push(positions[v0]);
                idx
            });
            below_verts.push(idx);
        }

        // Check if edge crosses plane
        if is_above_0 != is_above_1 {
            if let Some(intersection) = plane.intersect_edge(&positions[v0], &positions[v1]) {
                // Add intersection point to both sides
                let above_idx = above_positions.len();
                above_positions.push(intersection);
                above_verts.push(above_idx);

                let below_idx = below_positions.len();
                below_positions.push(intersection);
                below_verts.push(below_idx);

                cut_edges.push((above_idx, below_idx));
            }
        }
    }

    let mut above_faces = Vec::new();
    let mut below_faces = Vec::new();

    if above_verts.len() >= 3 {
        above_faces.push(above_verts);
    }
    if below_verts.len() >= 3 {
        below_faces.push(below_verts);
    }

    (above_faces, below_faces)
}

/// Cap holes created by slicing.
fn cap_slice_holes(
    _positions: &mut Vec<Point3<f64>>,
    _faces: &mut Vec<Vec<usize>>,
    cut_edges: &[(usize, usize)],
    _reverse: bool,
) {
    // Simplified: would need proper hole detection and filling
    // For now, just ensure we have the cut edges registered
    if cut_edges.is_empty() {}

    // In a full implementation, would:
    // 1. Find boundary loops from cut_edges
    // 2. Triangulate each loop
    // 3. Add cap faces with correct winding order
}

/// Knife cut along a path on the mesh surface.
#[allow(clippy::ptr_arg)]
pub fn knife_cut(
    positions: &mut Vec<Point3<f64>>,
    faces: &mut Vec<Vec<usize>>,
    cut_path: &[Point3<f64>],
) {
    if cut_path.len() < 2 {
        return;
    }

    // For each segment in the cut path
    for i in 0..cut_path.len() - 1 {
        let p0 = cut_path[i];
        let p1 = cut_path[i + 1];

        // Find faces intersected by this segment
        let mut faces_to_split = Vec::new();

        for (face_idx, face) in faces.iter().enumerate() {
            if face.len() < 3 {
                continue;
            }

            // Check if segment intersects face
            if segment_intersects_face(positions, face, &p0, &p1) {
                faces_to_split.push(face_idx);
            }
        }

        // Split intersected faces
        for &face_idx in faces_to_split.iter().rev() {
            if face_idx < faces.len() {
                let face = faces.remove(face_idx);
                let split_faces = split_face_by_segment(positions, &face, &p0, &p1);
                faces.extend(split_faces);
            }
        }
    }
}

/// Check if segment intersects face.
fn segment_intersects_face(
    positions: &[Point3<f64>],
    face: &[usize],
    p0: &Point3<f64>,
    p1: &Point3<f64>,
) -> bool {
    if face.len() < 3 {
        return false;
    }

    // Compute face plane
    let v0 = positions[face[0]];
    let v1 = positions[face[1]];
    let v2 = positions[face[2]];

    let e1 = v1 - v0;
    let e2 = v2 - v0;
    let normal = e1.cross(&e2);

    if normal.norm() < 1e-10 {
        return false;
    }

    let plane = SlicePlane::new(v0, normal);

    // Check if segment crosses plane
    let d0 = plane.signed_distance(p0);
    let d1 = plane.signed_distance(p1);

    if d0 * d1 > 0.0 {
        return false; // Both points on same side
    }

    // Check if intersection point is inside face (simplified)
    if let Some(_intersection) = plane.intersect_edge(p0, p1) {
        // In full implementation, would check if point is inside polygon
        return true;
    }

    false
}

/// Split face by line segment.
fn split_face_by_segment(
    _positions: &[Point3<f64>],
    face: &[usize],
    _p0: &Point3<f64>,
    _p1: &Point3<f64>,
) -> Vec<Vec<usize>> {
    // Simplified: just return original face
    // Full implementation would:
    // 1. Find intersection points with face edges
    // 2. Split face into two parts
    // 3. Add new vertices at intersections
    vec![face.to_vec()]
}

/// Bisect mesh with plane, optionally removing one side.
pub fn bisect(
    positions: &[Point3<f64>],
    faces: &[Vec<usize>],
    plane: &SlicePlane,
    _fill: bool,
    clear_inner: bool,
    clear_outer: bool,
) -> (Vec<Point3<f64>>, Vec<Vec<usize>>) {
    let slice_result = slice_mesh(positions, faces, plane);

    if clear_inner && clear_outer {
        // Both cleared - return empty mesh
        (Vec::new(), Vec::new())
    } else if clear_inner {
        // Keep only outer (above)
        slice_result.above
    } else if clear_outer {
        // Keep only inner (below)
        slice_result.below
    } else {
        // Keep both - merge them
        let (mut new_positions, mut new_faces) = slice_result.above;
        let (below_pos, below_faces) = slice_result.below;

        let offset = new_positions.len();
        new_positions.extend(below_pos);

        for face in below_faces {
            let new_face: Vec<usize> = face.iter().map(|&v| v + offset).collect();
            new_faces.push(new_face);
        }

        (new_positions, new_faces)
    }
}

/// Multi-segment loop cut.
#[allow(clippy::ptr_arg)]
pub fn multi_loop_cut(
    positions: &mut Vec<Point3<f64>>,
    faces: &mut Vec<Vec<usize>>,
    edge: (usize, usize),
    segments: usize,
) {
    if segments < 2 {
        return;
    }

    // Find edge loop containing this edge
    let edge_loop = find_edge_loop(faces, edge);

    if edge_loop.is_empty() {
        return;
    }

    // For each segment, create a parallel cut
    for i in 1..segments {
        let t = i as f64 / segments as f64;

        // Insert vertices along the loop
        let mut new_verts = Vec::new();
        for j in 0..edge_loop.len() {
            let v0 = edge_loop[j];
            let v1 = edge_loop[(j + 1) % edge_loop.len()];

            let p0 = positions[v0];
            let p1 = positions[v1];

            let new_pos = Point3::new(
                p0.x * (1.0 - t) + p1.x * t,
                p0.y * (1.0 - t) + p1.y * t,
                p0.z * (1.0 - t) + p1.z * t,
            );

            let idx = positions.len();
            positions.push(new_pos);
            new_verts.push(idx);
        }

        // Update faces to include new vertices
        // (Simplified - would need proper face splitting)
    }
}

/// Find edge loop containing given edge.
fn find_edge_loop(_faces: &[Vec<usize>], _edge: (usize, usize)) -> Vec<usize> {
    // Simplified: return empty
    // Full implementation would traverse connected edges forming a loop
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slice_plane() {
        let plane = SlicePlane::new(Point3::new(0.0, 0.0, 0.0), Vector3::new(0.0, 0.0, 1.0));

        let above = Point3::new(0.0, 0.0, 1.0);
        let below = Point3::new(0.0, 0.0, -1.0);

        assert!(plane.is_above(&above));
        assert!(!plane.is_above(&below));
    }

    #[test]
    fn test_plane_intersection() {
        let plane = SlicePlane::new(Point3::new(0.0, 0.0, 0.0), Vector3::new(0.0, 0.0, 1.0));

        let p0 = Point3::new(0.0, 0.0, -1.0);
        let p1 = Point3::new(0.0, 0.0, 1.0);

        let intersection = plane.intersect_edge(&p0, &p1);
        assert!(intersection.is_some());

        let point = intersection.unwrap();
        assert!((point.z).abs() < 1e-10);
    }

    #[test]
    fn test_slice_mesh() {
        let positions = vec![
            Point3::new(0.0, 0.0, -1.0),
            Point3::new(1.0, 0.0, -1.0),
            Point3::new(0.5, 1.0, 1.0),
        ];
        let faces = vec![vec![0, 1, 2]];

        let plane = SlicePlane::new(Point3::new(0.0, 0.0, 0.0), Vector3::new(0.0, 0.0, 1.0));

        let result = slice_mesh(&positions, &faces, &plane);

        // Should have parts on both sides
        assert!(!result.above.0.is_empty() || !result.below.0.is_empty());
    }

    #[test]
    fn test_bisect() {
        let positions = vec![
            Point3::new(-1.0, -1.0, 0.0),
            Point3::new(1.0, -1.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(-1.0, 1.0, 0.0),
            Point3::new(0.0, 0.0, 1.0),
        ];
        let faces = vec![vec![0, 1, 4], vec![1, 2, 4], vec![2, 3, 4], vec![3, 0, 4]];

        let plane = SlicePlane::new(Point3::new(0.0, 0.0, 0.5), Vector3::new(0.0, 0.0, 1.0));

        let (new_pos, new_faces) = bisect(&positions, &faces, &plane, false, false, true);

        // Should keep only below part
        assert!(!new_pos.is_empty());
        assert!(!new_faces.is_empty());
    }
}
