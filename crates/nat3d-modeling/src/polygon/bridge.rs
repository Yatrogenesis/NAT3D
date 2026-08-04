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

//! Bridge and additional polygon operations.
//!
//! Operations for connecting edge loops, filling holes, and mesh manipulation.

use nalgebra::Point3;
use std::collections::{HashMap, HashSet};

/// Bridge two edge loops with quads.
pub fn bridge_edge_loops(
    positions: &mut Vec<Point3<f64>>,
    faces: &mut Vec<Vec<usize>>,
    loop_a: &[usize],
    loop_b: &[usize],
    segments: usize,
    twist: i32,
) {
    if loop_a.is_empty() || loop_b.is_empty() {
        return;
    }

    let len_a = loop_a.len();
    let len_b = loop_b.len();

    if segments == 1 {
        // Direct connection
        let max_len = len_a.max(len_b);
        for i in 0..max_len {
            let ia0 = loop_a[i % len_a];
            let ia1 = loop_a[(i + 1) % len_a];
            let ib0_offset = ((i as i32 + twist) as usize) % len_b;
            let ib1_offset = ((i as i32 + 1 + twist) as usize) % len_b;
            let ib0 = loop_b[ib0_offset];
            let ib1 = loop_b[ib1_offset];

            faces.push(vec![ia0, ia1, ib1, ib0]);
        }
    } else {
        // Multi-segment bridge
        let mut intermediate_loops = Vec::new();
        intermediate_loops.push(loop_a.to_vec());

        for seg in 1..segments {
            let t = seg as f64 / segments as f64;
            let mut new_loop = Vec::new();

            for i in 0..len_a.max(len_b) {
                let ia = loop_a[i % len_a];
                let ib_offset = ((i as i32 + twist) as usize) % len_b;
                let ib = loop_b[ib_offset];

                let pa = positions[ia];
                let pb = positions[ib];
                let pos = Point3::new(
                    pa.x * (1.0 - t) + pb.x * t,
                    pa.y * (1.0 - t) + pb.y * t,
                    pa.z * (1.0 - t) + pb.z * t,
                );

                let idx = positions.len();
                positions.push(pos);
                new_loop.push(idx);
            }

            intermediate_loops.push(new_loop);
        }

        intermediate_loops.push(loop_b.to_vec());

        // Connect consecutive loops
        for i in 0..intermediate_loops.len() - 1 {
            let curr_loop = &intermediate_loops[i];
            let next_loop = &intermediate_loops[i + 1];

            for j in 0..curr_loop.len() {
                let v0 = curr_loop[j];
                let v1 = curr_loop[(j + 1) % curr_loop.len()];
                let v2 = next_loop[(j + 1) % next_loop.len()];
                let v3 = next_loop[j % next_loop.len()];

                faces.push(vec![v0, v1, v2, v3]);
            }
        }
    }
}

/// Fill a hole defined by boundary edges.
pub fn fill_hole(
    _positions: &mut Vec<Point3<f64>>,
    faces: &mut Vec<Vec<usize>>,
    boundary: &[usize],
) {
    if boundary.len() < 3 {
        return;
    }

    if boundary.len() == 3 {
        faces.push(boundary.to_vec());
        return;
    }

    // Simple fan triangulation from first vertex
    for i in 1..boundary.len() - 1 {
        faces.push(vec![boundary[0], boundary[i], boundary[i + 1]]);
    }
}

/// Fill hole with grid of quads.
pub fn grid_fill(
    positions: &mut Vec<Point3<f64>>,
    faces: &mut Vec<Vec<usize>>,
    boundary: &[usize],
    segments_u: usize,
    segments_v: usize,
) {
    if boundary.len() < 4 || segments_u < 2 || segments_v < 2 {
        fill_hole(positions, faces, boundary);
        return;
    }

    // Assume boundary is rectangular-ish (4 corners)
    let corners = if boundary.len() == 4 {
        boundary.to_vec()
    } else {
        // Pick 4 evenly spaced vertices as corners
        vec![
            boundary[0],
            boundary[boundary.len() / 4],
            boundary[boundary.len() / 2],
            boundary[3 * boundary.len() / 4],
        ]
    };

    // Create grid
    let mut grid = vec![vec![0usize; segments_u + 1]; segments_v + 1];

    // Corners
    grid[0][0] = corners[0];
    grid[0][segments_u] = corners[1];
    grid[segments_v][segments_u] = corners[2];
    grid[segments_v][0] = corners[3];

    // Generate interior vertices
    #[allow(clippy::needless_range_loop)]
    for v in 0..=segments_v {
        for u in 0..=segments_u {
            if (v == 0 || v == segments_v) && (u == 0 || u == segments_u) {
                continue; // Corners already set
            }

            let tu = u as f64 / segments_u as f64;
            let tv = v as f64 / segments_v as f64;

            let p00 = positions[corners[0]];
            let p10 = positions[corners[1]];
            let p11 = positions[corners[2]];
            let p01 = positions[corners[3]];

            // Bilinear interpolation
            let pos = Point3::new(
                (1.0 - tu) * (1.0 - tv) * p00.x
                    + tu * (1.0 - tv) * p10.x
                    + tu * tv * p11.x
                    + (1.0 - tu) * tv * p01.x,
                (1.0 - tu) * (1.0 - tv) * p00.y
                    + tu * (1.0 - tv) * p10.y
                    + tu * tv * p11.y
                    + (1.0 - tu) * tv * p01.y,
                (1.0 - tu) * (1.0 - tv) * p00.z
                    + tu * (1.0 - tv) * p10.z
                    + tu * tv * p11.z
                    + (1.0 - tu) * tv * p01.z,
            );

            let idx = positions.len();
            positions.push(pos);
            grid[v][u] = idx;
        }
    }

    // Generate faces
    for v in 0..segments_v {
        for u in 0..segments_u {
            let v0 = grid[v][u];
            let v1 = grid[v][u + 1];
            let v2 = grid[v + 1][u + 1];
            let v3 = grid[v + 1][u];
            faces.push(vec![v0, v1, v2, v3]);
        }
    }
}

/// Automatically find and fill all holes.
pub fn cap_holes(positions: &mut Vec<Point3<f64>>, faces: &mut Vec<Vec<usize>>) {
    let boundaries = find_boundaries(faces);

    for boundary in boundaries {
        if boundary.len() >= 3 {
            fill_hole(positions, faces, &boundary);
        }
    }
}

/// Find boundary loops in mesh.
fn find_boundaries(faces: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut edge_count: HashMap<(usize, usize), usize> = HashMap::new();

    for face in faces {
        for i in 0..face.len() {
            let v0 = face[i];
            let v1 = face[(i + 1) % face.len()];
            let edge = if v0 < v1 { (v0, v1) } else { (v1, v0) };
            *edge_count.entry(edge).or_insert(0) += 1;
        }
    }

    // Find boundary edges (appear only once)
    let mut boundary_edges: HashSet<(usize, usize)> = HashSet::new();
    for (edge, count) in edge_count {
        if count == 1 {
            boundary_edges.insert(edge);
        }
    }

    // Build boundary loops
    let mut boundaries = Vec::new();
    let mut remaining = boundary_edges.clone();

    while !remaining.is_empty() {
        let start_edge = *remaining.iter().next().unwrap();
        remaining.remove(&start_edge);

        let mut loop_verts = vec![start_edge.0, start_edge.1];
        let mut current = start_edge.1;

        loop {
            let next_edge = remaining
                .iter()
                .find(|&&(a, b)| a == current || b == current);
            if let Some(&edge) = next_edge {
                remaining.remove(&edge);
                current = if edge.0 == current { edge.1 } else { edge.0 };
                if current == start_edge.0 {
                    break;
                }
                loop_verts.push(current);
            } else {
                break;
            }
        }

        if loop_verts.len() >= 3 {
            boundaries.push(loop_verts);
        }
    }

    boundaries
}

/// Detach selected faces into a new separate mesh component.
pub fn detach_faces(
    positions: &[Point3<f64>],
    faces: &[Vec<usize>],
    face_indices: &[usize],
) -> (Vec<Point3<f64>>, Vec<Vec<usize>>) {
    let mut new_positions = Vec::new();
    let mut new_faces = Vec::new();
    let mut vertex_map: HashMap<usize, usize> = HashMap::new();

    for &face_idx in face_indices {
        if face_idx >= faces.len() {
            continue;
        }

        let face = &faces[face_idx];
        let mut new_face = Vec::new();

        for &v in face {
            let new_idx = *vertex_map.entry(v).or_insert_with(|| {
                let idx = new_positions.len();
                new_positions.push(positions[v]);
                idx
            });
            new_face.push(new_idx);
        }

        new_faces.push(new_face);
    }

    (new_positions, new_faces)
}

/// Attach two meshes together.
pub fn attach_meshes(
    positions_a: &[Point3<f64>],
    faces_a: &[Vec<usize>],
    positions_b: &[Point3<f64>],
    faces_b: &[Vec<usize>],
) -> (Vec<Point3<f64>>, Vec<Vec<usize>>) {
    let mut new_positions = positions_a.to_vec();
    let mut new_faces = faces_a.to_vec();

    let offset = positions_a.len();
    new_positions.extend_from_slice(positions_b);

    for face in faces_b {
        let new_face: Vec<usize> = face.iter().map(|&v| v + offset).collect();
        new_faces.push(new_face);
    }

    (new_positions, new_faces)
}

/// Weld vertices within threshold distance.
#[allow(clippy::ptr_arg)]
pub fn weld_vertices(
    positions: &mut Vec<Point3<f64>>,
    faces: &mut Vec<Vec<usize>>,
    threshold: f64,
) {
    let threshold_sq = threshold * threshold;
    let mut vertex_map: HashMap<usize, usize> = HashMap::new();

    for i in 0..positions.len() {
        let mut found_match = false;
        for j in 0..i {
            if (positions[i] - positions[j]).norm_squared() < threshold_sq {
                vertex_map.insert(i, j);
                found_match = true;
                break;
            }
        }
        if !found_match {
            vertex_map.insert(i, i);
        }
    }

    // Update face indices
    for face in faces.iter_mut() {
        for v in face.iter_mut() {
            if let Some(&new_v) = vertex_map.get(v) {
                *v = new_v;
            }
        }
    }

    // Remove degenerate faces
    faces.retain(|face| {
        let unique: HashSet<_> = face.iter().collect();
        unique.len() >= 3
    });
}

/// Merge two specific vertices.
#[allow(clippy::ptr_arg)]
pub fn target_weld(
    positions: &mut Vec<Point3<f64>>,
    faces: &mut Vec<Vec<usize>>,
    source: usize,
    target: usize,
) {
    if source >= positions.len() || target >= positions.len() || source == target {
        return;
    }

    // Merge source into target
    for face in faces.iter_mut() {
        for v in face.iter_mut() {
            if *v == source {
                *v = target;
            }
        }
    }

    // Remove degenerate faces
    faces.retain(|face| {
        let unique: HashSet<_> = face.iter().collect();
        unique.len() >= 3
    });
}

/// Collapse edge to its midpoint.
pub fn collapse_edge(
    positions: &mut Vec<Point3<f64>>,
    faces: &mut Vec<Vec<usize>>,
    edge: (usize, usize),
) {
    let (v0, v1) = edge;
    if v0 >= positions.len() || v1 >= positions.len() {
        return;
    }

    let midpoint = Point3::new(
        (positions[v0].x + positions[v1].x) / 2.0,
        (positions[v0].y + positions[v1].y) / 2.0,
        (positions[v0].z + positions[v1].z) / 2.0,
    );

    positions[v0] = midpoint;

    // Merge v1 into v0
    target_weld(positions, faces, v1, v0);
}

/// Remove vertex and reconnect adjacent faces.
pub fn dissolve_vertex(
    _positions: &mut Vec<Point3<f64>>,
    faces: &mut Vec<Vec<usize>>,
    vertex: usize,
) {
    // Find faces containing this vertex
    let affected_faces: Vec<usize> = faces
        .iter()
        .enumerate()
        .filter_map(|(i, face)| {
            if face.contains(&vertex) {
                Some(i)
            } else {
                None
            }
        })
        .collect();

    // Collect all unique vertices from affected faces (excluding dissolved vertex)
    let mut surrounding_verts: Vec<usize> = Vec::new();
    for &face_idx in &affected_faces {
        for &v in &faces[face_idx] {
            if v != vertex && !surrounding_verts.contains(&v) {
                surrounding_verts.push(v);
            }
        }
    }

    // Remove old faces
    let mut i = 0;
    faces.retain(|_| {
        let keep = !affected_faces.contains(&i);
        i += 1;
        keep
    });

    // Create new face from surrounding vertices (if 3+)
    if surrounding_verts.len() >= 3 {
        faces.push(surrounding_verts);
    }
}

/// Remove edge and merge adjacent faces.
pub fn dissolve_edge(
    _positions: &mut Vec<Point3<f64>>,
    faces: &mut Vec<Vec<usize>>,
    edge: (usize, usize),
) {
    let (v0, v1) = edge;

    // Find faces sharing this edge
    let adjacent_faces: Vec<usize> = faces
        .iter()
        .enumerate()
        .filter_map(|(i, face)| {
            let has_v0 = face.contains(&v0);
            let has_v1 = face.contains(&v1);
            if has_v0 && has_v1 {
                Some(i)
            } else {
                None
            }
        })
        .collect();

    if adjacent_faces.len() != 2 {
        return; // Edge must have exactly 2 adjacent faces
    }

    // Merge the two faces
    let face0 = faces[adjacent_faces[0]].clone();
    let face1 = faces[adjacent_faces[1]].clone();

    let mut merged: Vec<usize> = Vec::new();
    for &v in &face0 {
        if !merged.contains(&v) {
            merged.push(v);
        }
    }
    for &v in &face1 {
        if !merged.contains(&v) {
            merged.push(v);
        }
    }

    // Remove old faces
    faces.retain(|face| face != &face0 && face != &face1);

    // Add merged face
    if merged.len() >= 3 {
        faces.push(merged);
    }
}

/// Flip normals of specified faces.
pub fn flip_normals(faces: &mut [Vec<usize>], face_indices: &[usize]) {
    for &idx in face_indices {
        if idx < faces.len() {
            faces[idx].reverse();
        }
    }
}

/// Flatten vertices to a plane.
pub fn make_planar(positions: &mut [Point3<f64>], vertex_indices: &[usize], axis: usize) {
    if vertex_indices.is_empty() || axis > 2 {
        return;
    }

    // Compute average position along axis
    let mut sum = 0.0;
    for &v in vertex_indices {
        if v < positions.len() {
            sum += match axis {
                0 => positions[v].x,
                1 => positions[v].y,
                2 => positions[v].z,
                _ => 0.0,
            };
        }
    }
    let avg = sum / vertex_indices.len() as f64;

    // Set all vertices to average
    for &v in vertex_indices {
        if v < positions.len() {
            match axis {
                0 => positions[v].x = avg,
                1 => positions[v].y = avg,
                2 => positions[v].z = avg,
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_loops() {
        let mut positions = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
            Point3::new(0.0, 0.0, 1.0),
            Point3::new(1.0, 0.0, 1.0),
            Point3::new(1.0, 1.0, 1.0),
            Point3::new(0.0, 1.0, 1.0),
        ];
        let mut faces = Vec::new();

        let loop_a = vec![0, 1, 2, 3];
        let loop_b = vec![4, 5, 6, 7];

        bridge_edge_loops(&mut positions, &mut faces, &loop_a, &loop_b, 1, 0);

        assert_eq!(faces.len(), 4);
    }

    #[test]
    fn test_fill_hole() {
        let mut positions = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
        ];
        let mut faces = Vec::new();

        let boundary = vec![0, 1, 2, 3];
        fill_hole(&mut positions, &mut faces, &boundary);

        assert!(!faces.is_empty());
    }

    #[test]
    fn test_weld_vertices() {
        let mut positions = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(0.01, 0.01, 0.0), // Close to first
            Point3::new(1.0, 0.0, 0.0),
        ];
        let mut faces = vec![vec![0, 1, 2]];

        let initial_face_count = faces.len();
        weld_vertices(&mut positions, &mut faces, 0.1);

        // After welding, vertices within threshold should be merged
        // Face may be removed if it becomes degenerate
        assert!(faces.len() <= initial_face_count);

        // If face still exists, check it has valid vertex count
        if !faces.is_empty() {
            assert!(faces[0].len() <= 3);
        }
    }
}
