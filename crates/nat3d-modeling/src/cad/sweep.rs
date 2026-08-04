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

//! Sweep operations.
//!
//! Implements profile sweeping along paths for creating solids.

use super::sketch::Profile3D;
use nalgebra::{Matrix3, Point3, UnitQuaternion, Vector3};
use std::f64::consts::PI;

/// Sweep parameters.
#[derive(Debug, Clone)]
pub struct SweepParams {
    /// Twist angle along path (radians).
    pub twist: f64,
    /// Scale factor at end (1.0 = no scaling).
    pub end_scale: f64,
    /// Number of segments along path.
    pub segments: usize,
    /// Orientation mode.
    pub orientation: OrientationMode,
    /// Cap ends.
    pub capped: bool,
    /// Merge vertices at end for closed paths.
    pub merge_ends: bool,
}

impl Default for SweepParams {
    fn default() -> Self {
        Self {
            twist: 0.0,
            end_scale: 1.0,
            segments: 32,
            orientation: OrientationMode::FrenetSerret,
            capped: true,
            merge_ends: true,
        }
    }
}

/// Orientation mode for profile during sweep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrientationMode {
    /// Use Frenet-Serret frame (natural to curve).
    FrenetSerret,
    /// Keep profile parallel to initial orientation.
    Parallel,
    /// Fixed up vector.
    FixedUp(usize), // index into AXES: 0=X, 1=Y, 2=Z
    /// Binormal orientation (minimizes rotation).
    BinormalMinimize,
}

/// Path for sweeping.
#[derive(Debug, Clone)]
pub struct SweepPath {
    /// Path points.
    pub points: Vec<Point3<f64>>,
    /// Whether the path is closed.
    pub closed: bool,
}

impl SweepPath {
    /// Create a new sweep path.
    pub fn new(points: Vec<Point3<f64>>, closed: bool) -> Self {
        Self { points, closed }
    }

    /// Create a linear path.
    pub fn linear(start: Point3<f64>, end: Point3<f64>, segments: usize) -> Self {
        let mut points = Vec::with_capacity(segments + 1);
        for i in 0..=segments {
            let t = i as f64 / segments as f64;
            let p = Point3::new(
                start.x + (end.x - start.x) * t,
                start.y + (end.y - start.y) * t,
                start.z + (end.z - start.z) * t,
            );
            points.push(p);
        }
        Self {
            points,
            closed: false,
        }
    }

    /// Create a helical path.
    pub fn helix(
        center: Point3<f64>,
        radius: f64,
        height: f64,
        turns: f64,
        segments: usize,
    ) -> Self {
        let mut points = Vec::with_capacity(segments + 1);
        for i in 0..=segments {
            let t = i as f64 / segments as f64;
            let angle = 2.0 * PI * turns * t;
            let p = Point3::new(
                center.x + radius * angle.cos(),
                center.y + height * t,
                center.z + radius * angle.sin(),
            );
            points.push(p);
        }
        Self {
            points,
            closed: false,
        }
    }

    /// Create a circular path.
    pub fn circle(center: Point3<f64>, radius: f64, normal: Vector3<f64>, segments: usize) -> Self {
        let normal = normal.normalize();
        let (u, v) = orthonormal_basis(normal);

        let mut points = Vec::with_capacity(segments);
        for i in 0..segments {
            let angle = 2.0 * PI * i as f64 / segments as f64;
            let p = center + u * (radius * angle.cos()) + v * (radius * angle.sin());
            points.push(p);
        }
        Self {
            points,
            closed: true,
        }
    }

    /// Get path length.
    pub fn length(&self) -> f64 {
        let mut len = 0.0;
        for i in 1..self.points.len() {
            len += (self.points[i] - self.points[i - 1]).magnitude();
        }
        if self.closed && !self.points.is_empty() {
            len += (self.points[0] - self.points[self.points.len() - 1]).magnitude();
        }
        len
    }

    /// Get tangent at index.
    pub fn tangent(&self, index: usize) -> Vector3<f64> {
        let n = self.points.len();
        if n < 2 {
            return Vector3::y();
        }

        let (prev, next) = if self.closed {
            let prev = if index == 0 { n - 1 } else { index - 1 };
            let next = (index + 1) % n;
            (prev, next)
        } else {
            let prev = if index == 0 { 0 } else { index - 1 };
            let next = if index >= n - 1 { n - 1 } else { index + 1 };
            (prev, next)
        };

        (self.points[next] - self.points[prev]).normalize()
    }

    /// Get Frenet-Serret frame at index.
    pub fn frenet_frame(&self, index: usize) -> (Vector3<f64>, Vector3<f64>, Vector3<f64>) {
        let tangent = self.tangent(index);

        // Compute curvature vector for normal
        let n = self.points.len();
        let (prev, next) = if self.closed {
            let prev = if index == 0 { n - 1 } else { index - 1 };
            let next = (index + 1) % n;
            (prev, next)
        } else {
            let prev = if index == 0 { 0 } else { index - 1 };
            let next = if index >= n - 1 { n - 1 } else { index + 1 };
            (prev, next)
        };

        let t_prev = if prev > 0 || self.closed {
            (self.points[index] - self.points[prev]).normalize()
        } else {
            tangent
        };

        let t_next = if next < n - 1 || self.closed {
            (self.points[next] - self.points[index]).normalize()
        } else {
            tangent
        };

        let curvature = t_next - t_prev;
        let mut normal = if curvature.magnitude() > 1e-10 {
            curvature.normalize()
        } else {
            // Use default up for straight lines
            perpendicular_vector(tangent)
        };

        // Ensure normal is perpendicular to tangent
        normal = (normal - tangent * normal.dot(&tangent)).normalize();

        let binormal = tangent.cross(&normal).normalize();

        (tangent, normal, binormal)
    }
}

/// Result of a sweep operation.
#[derive(Debug, Clone)]
pub struct SweptMesh {
    /// Vertex positions.
    pub vertices: Vec<Point3<f64>>,
    /// Vertex normals.
    pub normals: Vec<Vector3<f64>>,
    /// Triangle indices.
    pub indices: Vec<[u32; 3]>,
}

/// Sweep a profile along a path.
pub fn sweep_profile(profile: &Profile3D, path: &SweepPath, params: &SweepParams) -> SweptMesh {
    let n_profile = profile.outer.len();
    if n_profile < 2 || path.points.len() < 2 {
        return SweptMesh {
            vertices: Vec::new(),
            normals: Vec::new(),
            indices: Vec::new(),
        };
    }

    let n_path = path.points.len();
    let mut vertices = Vec::with_capacity(n_profile * n_path);
    let mut normals = Vec::with_capacity(n_profile * n_path);
    let mut indices = Vec::new();

    // Calculate profile center and radius for positioning
    let profile_center = profile_centroid(&profile.outer);

    // Get initial frame
    let (init_tangent, init_normal, init_binormal) = path.frenet_frame(0);
    let init_rotation = frame_to_rotation(init_tangent, init_normal, init_binormal);

    let mut prev_rotation = init_rotation;

    // Generate vertices along path
    for (path_idx, &path_point) in path.points.iter().enumerate() {
        let t = path_idx as f64 / (n_path - 1) as f64;

        // Get local frame
        let (tangent, normal, binormal) = match params.orientation {
            OrientationMode::FrenetSerret => path.frenet_frame(path_idx),
            OrientationMode::Parallel => (init_tangent, init_normal, init_binormal),
            OrientationMode::FixedUp(axis) => {
                let up = match axis {
                    0 => Vector3::x(),
                    1 => Vector3::y(),
                    _ => Vector3::z(),
                };
                let tangent = path.tangent(path_idx);
                let binormal = tangent.cross(&up).normalize();
                let normal = binormal.cross(&tangent).normalize();
                (tangent, normal, binormal)
            }
            OrientationMode::BinormalMinimize => {
                let tangent = path.tangent(path_idx);
                // Project previous binormal using column extraction
                let prev_binormal = prev_rotation.column(2).into_owned();
                let binormal = (prev_binormal - tangent * prev_binormal.dot(&tangent)).normalize();
                let normal = binormal.cross(&tangent).normalize();
                (tangent, normal, binormal)
            }
        };

        let rotation = frame_to_rotation(tangent, normal, binormal);
        prev_rotation = rotation;

        // Apply twist
        let twist_angle = params.twist * t;
        let twist_rotation =
            UnitQuaternion::from_axis_angle(&nalgebra::Unit::new_normalize(tangent), twist_angle);

        // Apply scale
        let scale = 1.0 + (params.end_scale - 1.0) * t;

        // Transform profile points
        for profile_point in &profile.outer {
            // Center profile
            let local = profile_point - profile_center;

            // Scale
            let scaled = local * scale;

            // Apply twist
            let twisted = twist_rotation * scaled;

            // Transform to path frame
            let transformed = rotation * twisted;

            // Position along path
            let vertex = path_point + transformed;
            vertices.push(vertex);

            // Calculate normal (perpendicular to tangent and profile edge)
            let profile_normal = rotation * Vector3::new(local.x, local.y, 0.0).normalize();
            normals.push(profile_normal);
        }
    }

    // Generate faces
    let n_rings = if path.closed && params.merge_ends {
        n_path
    } else {
        n_path - 1
    };

    for ring in 0..n_rings {
        let ring_next = if path.closed && params.merge_ends && ring == n_path - 1 {
            0
        } else {
            ring + 1
        };

        for i in 0..n_profile {
            let next_i = (i + 1) % n_profile;

            let v00 = (ring * n_profile + i) as u32;
            let v01 = (ring * n_profile + next_i) as u32;
            let v10 = (ring_next * n_profile + i) as u32;
            let v11 = (ring_next * n_profile + next_i) as u32;

            indices.push([v00, v01, v10]);
            indices.push([v01, v11, v10]);
        }
    }

    // Add caps
    if params.capped && !path.closed {
        // Start cap
        let start_center = path.points[0];
        let start_center_idx = vertices.len() as u32;
        vertices.push(start_center);
        normals.push(-path.tangent(0));

        for i in 0..n_profile {
            let next = (i + 1) % n_profile;
            indices.push([start_center_idx, next as u32, i as u32]);
        }

        // End cap
        let end_center = path.points[n_path - 1];
        let end_center_idx = vertices.len() as u32;
        vertices.push(end_center);
        normals.push(path.tangent(n_path - 1));

        let base = ((n_path - 1) * n_profile) as u32;
        for i in 0..n_profile {
            let next = (i + 1) % n_profile;
            indices.push([end_center_idx, base + i as u32, base + next as u32]);
        }
    }

    // Recalculate normals for smooth shading
    recalculate_normals(&mut normals, &vertices, &indices);

    SweptMesh {
        vertices,
        normals,
        indices,
    }
}

/// Sweep with guide rails.
pub fn sweep_with_rails(
    profile: &Profile3D,
    path: &SweepPath,
    rails: &[SweepPath],
    params: &SweepParams,
) -> SweptMesh {
    if rails.is_empty() {
        return sweep_profile(profile, path, params);
    }

    let n_profile = profile.outer.len();
    if n_profile < 2 || path.points.len() < 2 {
        return SweptMesh {
            vertices: Vec::new(),
            normals: Vec::new(),
            indices: Vec::new(),
        };
    }

    let n_path = path.points.len();
    let mut vertices = Vec::with_capacity(n_profile * n_path);
    let mut normals = Vec::with_capacity(n_profile * n_path);
    let mut indices = Vec::new();

    let profile_center = profile_centroid(&profile.outer);

    // Generate vertices along path using rail guidance
    for path_idx in 0..n_path {
        let t = path_idx as f64 / (n_path - 1) as f64;
        let path_point = path.points[path_idx];

        // Get rail positions at this parameter
        let rail_positions: Vec<Point3<f64>> = rails
            .iter()
            .map(|rail| {
                let idx = ((rail.points.len() - 1) as f64 * t).round() as usize;
                rail.points[idx.min(rail.points.len() - 1)]
            })
            .collect();

        // Calculate local frame from path and rails
        let tangent = path.tangent(path_idx);
        let (_, normal, binormal) = path.frenet_frame(path_idx);

        // Calculate scale factors from rail distances
        let rail_scales: Vec<f64> = rail_positions
            .iter()
            .map(|rail_pos| (rail_pos - path_point).magnitude())
            .collect();

        let avg_scale = if !rail_scales.is_empty() {
            rail_scales.iter().sum::<f64>() / rail_scales.len() as f64
        } else {
            1.0
        };

        let rotation = frame_to_rotation(tangent, normal, binormal);

        // Apply twist
        let twist_angle = params.twist * t;
        let twist_rotation =
            UnitQuaternion::from_axis_angle(&nalgebra::Unit::new_normalize(tangent), twist_angle);

        // Transform profile points
        for (i, profile_point) in profile.outer.iter().enumerate() {
            let local = profile_point - profile_center;

            // Scale based on rails
            let scale = if i < rail_scales.len() {
                rail_scales[i] / local.magnitude().max(1e-10)
            } else {
                avg_scale
            };

            let scaled = local * scale;
            let twisted = twist_rotation * scaled;
            let transformed = rotation * twisted;

            let vertex = path_point + transformed;
            vertices.push(vertex);

            let profile_normal = rotation * Vector3::new(local.x, local.y, 0.0).normalize();
            normals.push(profile_normal);
        }
    }

    // Generate faces (same as basic sweep)
    let n_rings = if path.closed && params.merge_ends {
        n_path
    } else {
        n_path - 1
    };

    for ring in 0..n_rings {
        let ring_next = if path.closed && params.merge_ends && ring == n_path - 1 {
            0
        } else {
            ring + 1
        };

        for i in 0..n_profile {
            let next_i = (i + 1) % n_profile;

            let v00 = (ring * n_profile + i) as u32;
            let v01 = (ring * n_profile + next_i) as u32;
            let v10 = (ring_next * n_profile + i) as u32;
            let v11 = (ring_next * n_profile + next_i) as u32;

            indices.push([v00, v01, v10]);
            indices.push([v01, v11, v10]);
        }
    }

    recalculate_normals(&mut normals, &vertices, &indices);

    SweptMesh {
        vertices,
        normals,
        indices,
    }
}

impl SweptMesh {
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

    /// Calculate volume.
    pub fn volume(&self) -> f64 {
        let mut volume = 0.0;

        for tri in &self.indices {
            let v0 = self.vertices[tri[0] as usize];
            let v1 = self.vertices[tri[1] as usize];
            let v2 = self.vertices[tri[2] as usize];

            let cross = (v1 - v0).cross(&(v2 - v0));
            volume += v0.coords.dot(&cross) / 6.0;
        }

        volume.abs()
    }

    /// Merge with another mesh.
    pub fn merge(&mut self, other: &SweptMesh) {
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

// Helper functions

fn profile_centroid(points: &[Point3<f64>]) -> Point3<f64> {
    if points.is_empty() {
        return Point3::origin();
    }

    let sum = points
        .iter()
        .fold(Vector3::zeros(), |acc, p| acc + p.coords);

    Point3::from(sum / points.len() as f64)
}

fn perpendicular_vector(v: Vector3<f64>) -> Vector3<f64> {
    let abs_v = Vector3::new(v.x.abs(), v.y.abs(), v.z.abs());

    let other = if abs_v.x <= abs_v.y && abs_v.x <= abs_v.z {
        Vector3::x()
    } else if abs_v.y <= abs_v.z {
        Vector3::y()
    } else {
        Vector3::z()
    };

    v.cross(&other).normalize()
}

fn orthonormal_basis(normal: Vector3<f64>) -> (Vector3<f64>, Vector3<f64>) {
    let u = perpendicular_vector(normal);
    let v = normal.cross(&u);
    (u, v)
}

fn frame_to_rotation(
    tangent: Vector3<f64>,
    normal: Vector3<f64>,
    binormal: Vector3<f64>,
) -> Matrix3<f64> {
    Matrix3::from_columns(&[normal, binormal, tangent])
}

fn recalculate_normals(
    normals: &mut [Vector3<f64>],
    vertices: &[Point3<f64>],
    indices: &[[u32; 3]],
) {
    // Clear normals
    for n in normals.iter_mut() {
        *n = Vector3::zeros();
    }

    // Accumulate face normals
    for tri in indices {
        let v0 = vertices[tri[0] as usize];
        let v1 = vertices[tri[1] as usize];
        let v2 = vertices[tri[2] as usize];

        let face_normal = (v1 - v0).cross(&(v2 - v0));

        for &idx in tri {
            if (idx as usize) < normals.len() {
                normals[idx as usize] += face_normal;
            }
        }
    }

    // Normalize
    for n in normals.iter_mut() {
        let len = n.magnitude();
        if len > 1e-10 {
            *n /= len;
        } else {
            *n = Vector3::y();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_path() {
        let path = SweepPath::linear(Point3::origin(), Point3::new(0.0, 10.0, 0.0), 10);

        assert_eq!(path.points.len(), 11);
        assert!((path.length() - 10.0).abs() < 1e-6);
    }

    #[test]
    fn test_helix_path() {
        let path = SweepPath::helix(Point3::origin(), 1.0, 10.0, 2.0, 32);

        assert_eq!(path.points.len(), 33);
        assert!(path.length() > 10.0); // Helix is longer than height
    }

    #[test]
    fn test_basic_sweep() {
        let profile = Profile3D {
            outer: vec![
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
                Point3::new(-1.0, 0.0, 0.0),
                Point3::new(0.0, -1.0, 0.0),
            ],
            holes: Vec::new(),
        };

        let path = SweepPath::linear(Point3::origin(), Point3::new(0.0, 10.0, 0.0), 10);

        let mesh = sweep_profile(&profile, &path, &SweepParams::default());

        assert!(!mesh.vertices.is_empty());
        assert!(!mesh.indices.is_empty());
    }

    #[test]
    fn test_twisted_sweep() {
        let profile = Profile3D {
            outer: vec![
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
                Point3::new(-1.0, 0.0, 0.0),
                Point3::new(0.0, -1.0, 0.0),
            ],
            holes: Vec::new(),
        };

        let path = SweepPath::linear(Point3::origin(), Point3::new(0.0, 10.0, 0.0), 16);

        let params = SweepParams {
            twist: PI, // 180 degree twist
            ..Default::default()
        };

        let mesh = sweep_profile(&profile, &path, &params);

        assert!(!mesh.vertices.is_empty());
    }

    #[test]
    fn test_circular_path() {
        let path = SweepPath::circle(Point3::origin(), 5.0, Vector3::y(), 32);

        assert_eq!(path.points.len(), 32);
        assert!(path.closed);
    }
}
