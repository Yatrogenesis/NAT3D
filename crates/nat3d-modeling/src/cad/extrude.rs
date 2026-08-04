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

//! Extrusion operations.
//!
//! Implements profile extrusion for solid modeling including blind, through,
//! up-to-face, and tapered extrusions.

use super::sketch::{Profile3D, Sketch, SketchProfile};
use nalgebra::{Point3, Vector3};

/// Extrusion type.
#[derive(Debug, Clone)]
pub enum ExtrudeType {
    /// Extrude by a fixed distance.
    Blind(f64),
    /// Extrude in both directions.
    Symmetric(f64),
    /// Extrude up to a plane.
    UpToPlane { normal: Vector3<f64>, distance: f64 },
    /// Extrude through all geometry.
    ThroughAll,
    /// Extrude with a taper angle.
    Tapered { depth: f64, angle: f64 },
    /// Extrude with draft on both sides.
    Draft {
        depth: f64,
        inner_angle: f64,
        outer_angle: f64,
    },
}

/// Boolean operation for extrusion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtrudeOperation {
    /// Add material (boss).
    Add,
    /// Remove material (cut).
    Cut,
    /// Intersect with existing geometry.
    Intersect,
}

/// Extrusion parameters.
#[derive(Debug, Clone)]
pub struct ExtrudeParams {
    /// Type of extrusion.
    pub extrude_type: ExtrudeType,
    /// Direction of extrusion.
    pub direction: Vector3<f64>,
    /// Boolean operation.
    pub operation: ExtrudeOperation,
    /// Whether to cap the ends.
    pub capped: bool,
    /// Number of segments for curved profiles.
    pub segments: usize,
}

impl Default for ExtrudeParams {
    fn default() -> Self {
        Self {
            extrude_type: ExtrudeType::Blind(1.0),
            direction: Vector3::z(),
            operation: ExtrudeOperation::Add,
            capped: true,
            segments: 32,
        }
    }
}

/// Result of an extrusion operation.
#[derive(Debug, Clone)]
pub struct ExtrudedMesh {
    /// Vertex positions.
    pub vertices: Vec<Point3<f64>>,
    /// Vertex normals.
    pub normals: Vec<Vector3<f64>>,
    /// Triangle indices.
    pub indices: Vec<[u32; 3]>,
}

/// Extrude a sketch profile.
pub fn extrude_sketch(sketch: &Sketch, params: &ExtrudeParams) -> Vec<ExtrudedMesh> {
    let profiles = sketch.find_profiles();
    profiles
        .iter()
        .map(|profile| extrude_profile(profile, &sketch.plane, params))
        .collect()
}

/// Extrude a single profile.
pub fn extrude_profile(
    profile: &SketchProfile,
    plane: &super::sketch::SketchPlane,
    params: &ExtrudeParams,
) -> ExtrudedMesh {
    let profile_3d = profile.to_3d(plane);
    extrude_profile_3d(&profile_3d, params)
}

/// Extrude a 3D profile directly.
pub fn extrude_profile_3d(profile: &Profile3D, params: &ExtrudeParams) -> ExtrudedMesh {
    let direction = params.direction.normalize();

    let (start_offset, end_offset) = match &params.extrude_type {
        ExtrudeType::Blind(depth) => (0.0, *depth),
        ExtrudeType::Symmetric(depth) => (-depth / 2.0, depth / 2.0),
        ExtrudeType::UpToPlane { distance, .. } => (0.0, *distance),
        ExtrudeType::ThroughAll => (0.0, 1000.0), // Large value
        ExtrudeType::Tapered { depth, .. } => (0.0, *depth),
        ExtrudeType::Draft { depth, .. } => (0.0, *depth),
    };

    let taper_factor = match &params.extrude_type {
        ExtrudeType::Tapered { depth, angle } => {
            if *depth > 0.0 {
                1.0 - (angle.tan() * depth / profile_radius(profile))
            } else {
                1.0
            }
        }
        _ => 1.0,
    };

    let mut vertices = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    // Generate side walls
    let n = profile.outer.len();
    if n < 2 {
        return ExtrudedMesh {
            vertices,
            normals,
            indices,
        };
    }

    // Bottom ring
    let bottom_start = vertices.len();
    for point in &profile.outer {
        vertices.push(*point + direction * start_offset);
    }

    // Top ring (possibly scaled for taper)
    let top_start = vertices.len();
    let center = profile_center(profile);
    for point in &profile.outer {
        let scaled = if (taper_factor - 1.0).abs() > 1e-10 {
            let to_point = *point - center;
            center + to_point * taper_factor
        } else {
            *point
        };
        vertices.push(scaled + direction * end_offset);
    }

    // Generate side normals and faces
    for i in 0..n {
        let next = (i + 1) % n;

        // Calculate edge normal (perpendicular to edge and direction)
        let edge = profile.outer[next] - profile.outer[i];
        let edge_dir = Vector3::new(edge.x, edge.y, edge.z);
        let normal = edge_dir.cross(&direction).normalize();

        let bi = bottom_start + i;
        let bn = bottom_start + next;
        let ti = top_start + i;
        let tn = top_start + next;

        // Add normals for the 4 vertices of this quad
        normals.push(normal);
        normals.push(normal);
        normals.push(normal);
        normals.push(normal);

        // Two triangles per quad
        let _base = normals.len() - 4;
        indices.push([bi as u32, bn as u32, ti as u32]);
        indices.push([bn as u32, tn as u32, ti as u32]);
    }

    // Generate caps if requested
    if params.capped {
        // Bottom cap
        let _bottom_cap_start = vertices.len() as u32;
        let bottom_normal = -direction;

        // Simple fan triangulation for convex profiles
        let center_bottom = profile_center(profile) + direction * start_offset;
        let center_idx = vertices.len();
        vertices.push(center_bottom);
        normals.push(bottom_normal);

        for point in &profile.outer {
            vertices.push(*point + direction * start_offset);
            normals.push(bottom_normal);
        }

        for i in 0..n {
            let next = (i + 1) % n;
            indices.push([
                center_idx as u32,
                (center_idx + 1 + next) as u32,
                (center_idx + 1 + i) as u32,
            ]);
        }

        // Top cap
        let top_normal = direction;
        let center_top = profile_center(profile) + direction * end_offset;

        let center_idx = vertices.len();
        vertices.push(center_top);
        normals.push(top_normal);

        for point in &profile.outer {
            let scaled = if (taper_factor - 1.0).abs() > 1e-10 {
                let to_point = *point - center;
                center + to_point * taper_factor
            } else {
                *point
            };
            vertices.push(scaled + direction * end_offset);
            normals.push(top_normal);
        }

        for i in 0..n {
            let next = (i + 1) % n;
            indices.push([
                center_idx as u32,
                (center_idx + 1 + i) as u32,
                (center_idx + 1 + next) as u32,
            ]);
        }
    }

    // Handle holes
    for hole in &profile.holes {
        let hole_mesh = extrude_loop(
            hole,
            &direction,
            start_offset,
            end_offset,
            taper_factor,
            &center,
            params.capped,
        );
        let vertex_offset = vertices.len() as u32;

        vertices.extend(hole_mesh.vertices);
        normals.extend(hole_mesh.normals);

        for tri in hole_mesh.indices {
            indices.push([
                tri[0] + vertex_offset,
                tri[2] + vertex_offset, // Reversed for inside-out
                tri[1] + vertex_offset,
            ]);
        }
    }

    ExtrudedMesh {
        vertices,
        normals,
        indices,
    }
}

fn extrude_loop(
    loop_points: &[Point3<f64>],
    direction: &Vector3<f64>,
    start_offset: f64,
    end_offset: f64,
    taper_factor: f64,
    center: &Point3<f64>,
    _capped: bool,
) -> ExtrudedMesh {
    let mut vertices = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    let n = loop_points.len();
    if n < 2 {
        return ExtrudedMesh {
            vertices,
            normals,
            indices,
        };
    }

    // Bottom ring
    for point in loop_points {
        vertices.push(*point + direction * start_offset);
    }

    // Top ring
    for point in loop_points {
        let scaled = if (taper_factor - 1.0).abs() > 1e-10 {
            let to_point = *point - *center;
            *center + to_point * taper_factor
        } else {
            *point
        };
        vertices.push(scaled + direction * end_offset);
    }

    // Side faces
    for i in 0..n {
        let next = (i + 1) % n;
        let edge = loop_points[next] - loop_points[i];
        let edge_dir = Vector3::new(edge.x, edge.y, edge.z);
        let normal = direction.cross(&edge_dir).normalize();

        normals.push(normal);
        normals.push(normal);
        normals.push(normal);
        normals.push(normal);

        indices.push([i as u32, (n + i) as u32, next as u32]);
        indices.push([next as u32, (n + i) as u32, (n + next) as u32]);
    }

    ExtrudedMesh {
        vertices,
        normals,
        indices,
    }
}

fn profile_center(profile: &Profile3D) -> Point3<f64> {
    if profile.outer.is_empty() {
        return Point3::<f64>::origin();
    }

    let sum: Point3<f64> = profile
        .outer
        .iter()
        .fold(Point3::<f64>::origin(), |acc, p| {
            Point3::new(acc.x + p.x, acc.y + p.y, acc.z + p.z)
        });

    Point3::new(
        sum.x / profile.outer.len() as f64,
        sum.y / profile.outer.len() as f64,
        sum.z / profile.outer.len() as f64,
    )
}

fn profile_radius(profile: &Profile3D) -> f64 {
    let center = profile_center(profile);

    profile
        .outer
        .iter()
        .map(|p| (p - center).norm())
        .fold(0.0, f64::max)
}

/// Extrude along a path (sweep-like extrusion).
pub fn extrude_along_path(profile: &Profile3D, path: &[Point3<f64>], twist: f64) -> ExtrudedMesh {
    let mut vertices = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    if path.len() < 2 || profile.outer.is_empty() {
        return ExtrudedMesh {
            vertices,
            normals,
            indices,
        };
    }

    let path_len = path.len();
    let profile_len = profile.outer.len();

    // Generate vertices along path
    for (i, &path_point) in path.iter().enumerate() {
        let t = i as f64 / (path_len - 1) as f64;
        let twist_angle = twist * t;

        // Calculate local frame
        let tangent = if i == 0 {
            (path[1] - path[0]).normalize()
        } else if i == path_len - 1 {
            (path[path_len - 1] - path[path_len - 2]).normalize()
        } else {
            ((path[i + 1] - path[i]).normalize() + (path[i] - path[i - 1]).normalize()).normalize()
        };

        // Find perpendicular vectors
        let up = if tangent.y.abs() < 0.9 {
            Vector3::y()
        } else {
            Vector3::x()
        };
        let right = tangent.cross(&up).normalize();
        let up = right.cross(&tangent);

        // Apply twist
        let cos_t = twist_angle.cos();
        let sin_t = twist_angle.sin();

        for profile_point in &profile.outer {
            // Transform profile point to path frame
            let local = profile_point - profile_center(profile);
            let rotated_x = local.x * cos_t - local.y * sin_t;
            let rotated_y = local.x * sin_t + local.y * cos_t;

            let world_point = path_point + right * rotated_x + up * rotated_y;
            vertices.push(world_point);

            // Approximate normal
            let normal = (world_point - path_point).normalize();
            normals.push(normal);
        }
    }

    // Generate side faces
    for i in 0..(path_len - 1) {
        for j in 0..profile_len {
            let next_j = (j + 1) % profile_len;

            let v00 = (i * profile_len + j) as u32;
            let v01 = (i * profile_len + next_j) as u32;
            let v10 = ((i + 1) * profile_len + j) as u32;
            let v11 = ((i + 1) * profile_len + next_j) as u32;

            indices.push([v00, v01, v10]);
            indices.push([v01, v11, v10]);
        }
    }

    ExtrudedMesh {
        vertices,
        normals,
        indices,
    }
}

impl ExtrudedMesh {
    /// Calculate bounding box.
    pub fn bounds(&self) -> (Point3<f64>, Point3<f64>) {
        let mut min = Point3::new(f64::MAX, f64::MAX, f64::MAX);
        let mut max = Point3::new(f64::MIN, f64::MIN, f64::MIN);

        for v in &self.vertices {
            min.x = min.x.min(v.x);
            min.y = min.y.min(v.y);
            min.z = min.z.min(v.z);
            max.x = max.x.max(v.x);
            max.y = max.y.max(v.y);
            max.z = max.z.max(v.z);
        }

        (min, max)
    }

    /// Calculate approximate volume.
    pub fn volume(&self) -> f64 {
        let mut volume = 0.0;

        for tri in &self.indices {
            let v0 = self.vertices[tri[0] as usize];
            let v1 = self.vertices[tri[1] as usize];
            let v2 = self.vertices[tri[2] as usize];

            // Signed volume of tetrahedron with origin
            let cross = (v1 - v0).cross(&(v2 - v0));
            volume += v0.coords.dot(&cross) / 6.0;
        }

        volume.abs()
    }

    /// Merge with another mesh.
    pub fn merge(&mut self, other: &ExtrudedMesh) {
        let vertex_offset = self.vertices.len() as u32;

        self.vertices.extend(&other.vertices);
        self.normals.extend(&other.normals);

        for tri in &other.indices {
            self.indices.push([
                tri[0] + vertex_offset,
                tri[1] + vertex_offset,
                tri[2] + vertex_offset,
            ]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_extrude() {
        let profile = Profile3D {
            outer: vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(1.0, 1.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
            ],
            holes: Vec::new(),
        };

        let params = ExtrudeParams {
            extrude_type: ExtrudeType::Blind(2.0),
            direction: Vector3::z(),
            ..Default::default()
        };

        let mesh = extrude_profile_3d(&profile, &params);

        assert!(!mesh.vertices.is_empty());
        assert!(!mesh.indices.is_empty());
    }

    #[test]
    fn test_symmetric_extrude() {
        let profile = Profile3D {
            outer: vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.5, 1.0, 0.0),
            ],
            holes: Vec::new(),
        };

        let params = ExtrudeParams {
            extrude_type: ExtrudeType::Symmetric(2.0),
            direction: Vector3::z(),
            ..Default::default()
        };

        let mesh = extrude_profile_3d(&profile, &params);

        let (min, max) = mesh.bounds();
        assert!(min.z < 0.0);
        assert!(max.z > 0.0);
    }

    #[test]
    fn test_volume_calculation() {
        let profile = Profile3D {
            outer: vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(1.0, 1.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
            ],
            holes: Vec::new(),
        };

        let params = ExtrudeParams {
            extrude_type: ExtrudeType::Blind(1.0),
            direction: Vector3::z(),
            ..Default::default()
        };

        let mesh = extrude_profile_3d(&profile, &params);
        let volume = mesh.volume();

        // Volume of 1x1x1 cube should be approximately 1
        assert!((volume - 1.0).abs() < 0.1);
    }
}
