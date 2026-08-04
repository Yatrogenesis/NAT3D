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

//! Revolve operations.
//!
//! Implements profile revolution for creating solids of revolution.

use super::sketch::{Profile3D, Sketch};
use nalgebra::{Point3, Vector3};
use std::f64::consts::PI;

/// Revolve parameters.
#[derive(Debug, Clone)]
pub struct RevolveParams {
    /// Axis origin point.
    pub axis_origin: Point3<f64>,
    /// Axis direction.
    pub axis_direction: Vector3<f64>,
    /// Revolution angle in radians (2*PI for full revolution).
    pub angle: f64,
    /// Number of segments around the axis.
    pub segments: usize,
    /// Whether to cap the ends (for partial revolutions).
    pub capped: bool,
}

impl Default for RevolveParams {
    fn default() -> Self {
        Self {
            axis_origin: Point3::origin(),
            axis_direction: Vector3::y(),
            angle: 2.0 * PI,
            segments: 32,
            capped: true,
        }
    }
}

/// Result of a revolve operation.
#[derive(Debug, Clone)]
pub struct RevolvedMesh {
    /// Vertex positions.
    pub vertices: Vec<Point3<f64>>,
    /// Vertex normals.
    pub normals: Vec<Vector3<f64>>,
    /// Triangle indices.
    pub indices: Vec<[u32; 3]>,
}

/// Revolve a sketch around an axis.
pub fn revolve_sketch(sketch: &Sketch, params: &RevolveParams) -> Vec<RevolvedMesh> {
    let profiles = sketch.find_profiles();
    profiles
        .iter()
        .map(|profile| {
            let profile_3d = profile.to_3d(&sketch.plane);
            revolve_profile_3d(&profile_3d, params)
        })
        .collect()
}

/// Revolve a 3D profile around an axis.
pub fn revolve_profile_3d(profile: &Profile3D, params: &RevolveParams) -> RevolvedMesh {
    let axis = params.axis_direction.normalize();

    let mut vertices = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    let n_profile = profile.outer.len();
    if n_profile < 2 {
        return RevolvedMesh {
            vertices,
            normals,
            indices,
        };
    }

    let n_segments = params.segments;
    let is_full = (params.angle - 2.0 * PI).abs() < 0.01;

    // Generate vertices for each segment
    for seg in 0..=n_segments {
        let t = if is_full && seg == n_segments {
            0.0 // Wrap to start for full revolution
        } else {
            seg as f64 / n_segments as f64
        };
        let angle = params.angle * t;

        for &profile_point in &profile.outer {
            let rotated = rotate_point_around_axis(profile_point, params.axis_origin, axis, angle);
            vertices.push(rotated);
        }
    }

    // Calculate normals
    for seg in 0..=n_segments {
        let t = seg as f64 / n_segments as f64;
        let angle = params.angle * t;

        for i in 0..n_profile {
            let profile_point = profile.outer[i];

            // Calculate tangent along profile
            let prev_idx = if i == 0 { n_profile - 1 } else { i - 1 };
            let next_idx = (i + 1) % n_profile;

            let prev = profile.outer[prev_idx];
            let next = profile.outer[next_idx];
            let profile_tangent = (next - prev).normalize();

            // Rotate tangent around axis
            let rotated_tangent = rotate_vector_around_axis(profile_tangent, axis, angle);

            // Calculate circumferential tangent
            let to_axis =
                closest_point_on_axis(profile_point, params.axis_origin, axis) - profile_point;
            let radial = -to_axis.normalize();
            let circumferential = axis.cross(&radial);

            // Normal is perpendicular to both tangents
            let normal = rotated_tangent.cross(&circumferential).normalize();
            normals.push(normal);
        }
    }

    // Generate faces
    let n_rings = n_segments;

    for seg in 0..n_rings {
        let seg_next = if is_full && seg == n_segments - 1 {
            0
        } else {
            seg + 1
        };

        for i in 0..n_profile {
            let next_i = (i + 1) % n_profile;

            let v00 = (seg * n_profile + i) as u32;
            let v01 = (seg * n_profile + next_i) as u32;
            let v10 = (seg_next * n_profile + i) as u32;
            let v11 = (seg_next * n_profile + next_i) as u32;

            indices.push([v00, v01, v10]);
            indices.push([v01, v11, v10]);
        }
    }

    // Add caps for partial revolutions
    if !is_full && params.capped && n_profile >= 3 {
        // Start cap
        let start_cap_center = profile_center(&profile.outer);
        let center_idx = vertices.len() as u32;
        vertices.push(start_cap_center);
        normals.push(-rotation_direction(
            params.axis_origin,
            axis,
            profile.outer[0],
        ));

        for &p in &profile.outer {
            vertices.push(p);
            normals.push(-rotation_direction(params.axis_origin, axis, p));
        }

        for i in 0..n_profile {
            let next = (i + 1) % n_profile;
            indices.push([
                center_idx,
                center_idx + 1 + next as u32,
                center_idx + 1 + i as u32,
            ]);
        }

        // End cap
        let end_center =
            rotate_point_around_axis(start_cap_center, params.axis_origin, axis, params.angle);
        let end_center_idx = vertices.len() as u32;
        vertices.push(end_center);
        normals.push(rotation_direction(
            params.axis_origin,
            axis,
            profile.outer[0],
        ));

        for &p in &profile.outer {
            let rotated = rotate_point_around_axis(p, params.axis_origin, axis, params.angle);
            vertices.push(rotated);
            normals.push(rotation_direction(params.axis_origin, axis, rotated));
        }

        for i in 0..n_profile {
            let next = (i + 1) % n_profile;
            indices.push([
                end_center_idx,
                end_center_idx + 1 + i as u32,
                end_center_idx + 1 + next as u32,
            ]);
        }
    }

    RevolvedMesh {
        vertices,
        normals,
        indices,
    }
}

/// Rotate a point around an axis.
fn rotate_point_around_axis(
    point: Point3<f64>,
    axis_origin: Point3<f64>,
    axis: Vector3<f64>,
    angle: f64,
) -> Point3<f64> {
    let v = point - axis_origin;
    let rotated = rotate_vector_around_axis(v, axis, angle);
    axis_origin + rotated
}

/// Rotate a vector around an axis using Rodrigues' rotation formula.
/// Positive angle = counterclockwise when looking at the axis from positive direction.
fn rotate_vector_around_axis(v: Vector3<f64>, axis: Vector3<f64>, angle: f64) -> Vector3<f64> {
    let k = axis.normalize();
    let cos_a = angle.cos();
    let sin_a = angle.sin();

    // Rodrigues' formula with right-hand convention for 3D graphics:
    // v' = v*cos(θ) + (v × k)*sin(θ) + k*(k·v)*(1-cos(θ))
    // Using v × k so positive rotation around Y moves X toward Z
    v * cos_a + v.cross(&k) * sin_a + k * k.dot(&v) * (1.0 - cos_a)
}

/// Find the closest point on the axis to a given point.
fn closest_point_on_axis(
    point: Point3<f64>,
    axis_origin: Point3<f64>,
    axis: Vector3<f64>,
) -> Point3<f64> {
    let v = point - axis_origin;
    let proj_len = v.dot(&axis);
    axis_origin + axis * proj_len
}

/// Get the direction of rotation at a point.
fn rotation_direction(
    axis_origin: Point3<f64>,
    axis: Vector3<f64>,
    point: Point3<f64>,
) -> Vector3<f64> {
    let to_point = point - axis_origin;
    let proj = axis * to_point.dot(&axis);
    let radial = to_point - proj;
    axis.cross(&radial).normalize()
}

/// Calculate the center of a profile.
fn profile_center(points: &[Point3<f64>]) -> Point3<f64> {
    if points.is_empty() {
        return Point3::origin();
    }

    let sum = points.iter().fold(Point3::<f64>::origin(), |acc, p| {
        Point3::new(acc.x + p.x, acc.y + p.y, acc.z + p.z)
    });

    Point3::new(
        sum.x / points.len() as f64,
        sum.y / points.len() as f64,
        sum.z / points.len() as f64,
    )
}

impl RevolvedMesh {
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

    /// Calculate approximate volume using signed tetrahedron volumes.
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
    pub fn merge(&mut self, other: &RevolvedMesh) {
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

/// Create common revolution shapes.
pub mod primitives {
    use super::*;

    /// Create a sphere by revolving a semicircle.
    pub fn sphere(center: Point3<f64>, radius: f64, segments: usize) -> RevolvedMesh {
        let mut profile_points = Vec::with_capacity(segments + 1);

        for i in 0..=segments {
            let angle = PI * i as f64 / segments as f64 - PI / 2.0;
            let x = center.x + radius * angle.cos();
            let y = center.y + radius * angle.sin();
            profile_points.push(Point3::new(x, y, center.z));
        }

        let profile = Profile3D {
            outer: profile_points,
            holes: Vec::new(),
        };

        let params = RevolveParams {
            axis_origin: center,
            axis_direction: Vector3::y(),
            angle: 2.0 * PI,
            segments,
            capped: false,
        };

        revolve_profile_3d(&profile, &params)
    }

    /// Create a torus by revolving a circle.
    pub fn torus(
        center: Point3<f64>,
        major_radius: f64,
        minor_radius: f64,
        segments: usize,
    ) -> RevolvedMesh {
        let mut profile_points = Vec::with_capacity(segments);

        for i in 0..segments {
            let angle = 2.0 * PI * i as f64 / segments as f64;
            let x = center.x + major_radius + minor_radius * angle.cos();
            let y = center.y + minor_radius * angle.sin();
            profile_points.push(Point3::new(x, y, center.z));
        }

        let profile = Profile3D {
            outer: profile_points,
            holes: Vec::new(),
        };

        let params = RevolveParams {
            axis_origin: center,
            axis_direction: Vector3::y(),
            angle: 2.0 * PI,
            segments,
            capped: false,
        };

        revolve_profile_3d(&profile, &params)
    }

    /// Create a cone by revolving a triangle.
    pub fn cone(
        base_center: Point3<f64>,
        radius: f64,
        height: f64,
        segments: usize,
    ) -> RevolvedMesh {
        let apex = base_center + Vector3::y() * height;

        let profile = Profile3D {
            outer: vec![
                base_center,
                Point3::new(base_center.x + radius, base_center.y, base_center.z),
                apex,
            ],
            holes: Vec::new(),
        };

        let params = RevolveParams {
            axis_origin: base_center,
            axis_direction: Vector3::y(),
            angle: 2.0 * PI,
            segments,
            capped: true,
        };

        revolve_profile_3d(&profile, &params)
    }

    /// Create a cylinder by revolving a rectangle.
    pub fn cylinder(
        base_center: Point3<f64>,
        radius: f64,
        height: f64,
        segments: usize,
    ) -> RevolvedMesh {
        let profile = Profile3D {
            outer: vec![
                base_center,
                Point3::new(base_center.x + radius, base_center.y, base_center.z),
                Point3::new(
                    base_center.x + radius,
                    base_center.y + height,
                    base_center.z,
                ),
                Point3::new(base_center.x, base_center.y + height, base_center.z),
            ],
            holes: Vec::new(),
        };

        let params = RevolveParams {
            axis_origin: base_center,
            axis_direction: Vector3::y(),
            angle: 2.0 * PI,
            segments,
            capped: true,
        };

        revolve_profile_3d(&profile, &params)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rotate_point() {
        let point = Point3::new(1.0, 0.0, 0.0);
        let origin = Point3::origin();
        let axis = Vector3::y();

        let rotated = rotate_point_around_axis(point, origin, axis, PI / 2.0);

        assert!((rotated.x).abs() < 1e-10);
        assert!(rotated.y.abs() < 1e-10);
        assert!((rotated.z - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_full_revolution() {
        let profile = Profile3D {
            outer: vec![
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(2.0, 0.0, 0.0),
                Point3::new(2.0, 1.0, 0.0),
                Point3::new(1.0, 1.0, 0.0),
            ],
            holes: Vec::new(),
        };

        let params = RevolveParams {
            axis_origin: Point3::origin(),
            axis_direction: Vector3::y(),
            angle: 2.0 * PI,
            segments: 16,
            capped: false,
        };

        let mesh = revolve_profile_3d(&profile, &params);

        assert!(!mesh.vertices.is_empty());
        assert!(!mesh.indices.is_empty());
    }

    #[test]
    fn test_partial_revolution() {
        let profile = Profile3D {
            outer: vec![
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(2.0, 0.0, 0.0),
                Point3::new(1.5, 1.0, 0.0),
            ],
            holes: Vec::new(),
        };

        let params = RevolveParams {
            axis_origin: Point3::origin(),
            axis_direction: Vector3::y(),
            angle: PI / 2.0,
            segments: 8,
            capped: true,
        };

        let mesh = revolve_profile_3d(&profile, &params);

        assert!(!mesh.vertices.is_empty());
    }

    #[test]
    fn test_sphere_primitive() {
        let sphere = primitives::sphere(Point3::origin(), 1.0, 16);

        assert!(!sphere.vertices.is_empty());

        let (min, max) = sphere.bounds();
        assert!((min.x + 1.0).abs() < 0.1);
        assert!((max.x - 1.0).abs() < 0.1);
    }

    #[test]
    fn test_torus_primitive() {
        let torus = primitives::torus(Point3::origin(), 2.0, 0.5, 16);

        assert!(!torus.vertices.is_empty());
    }
}
