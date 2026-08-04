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

//! Loft operations.
//!
//! Implements profile lofting for creating solids from multiple cross-sections.

use super::sketch::Profile3D;
use nalgebra::{Point3, Vector3};

/// Loft parameters.
#[derive(Debug, Clone)]
pub struct LoftParams {
    /// Number of intermediate sections.
    pub sections: usize,
    /// Continuity at connections.
    pub continuity: Continuity,
    /// Close the loft (connect last to first).
    pub closed: bool,
    /// Cap ends.
    pub capped: bool,
    /// Guide curves (optional).
    pub guides: Vec<GuideCurve>,
}

impl Default for LoftParams {
    fn default() -> Self {
        Self {
            sections: 16,
            continuity: Continuity::C1,
            closed: false,
            capped: true,
            guides: Vec::new(),
        }
    }
}

/// Continuity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Continuity {
    /// Position continuity.
    C0,
    /// Tangent continuity.
    C1,
    /// Curvature continuity.
    C2,
}

/// Guide curve for lofting.
#[derive(Debug, Clone)]
pub struct GuideCurve {
    /// Guide points.
    pub points: Vec<Point3<f64>>,
}

impl GuideCurve {
    /// Create a new guide curve.
    pub fn new(points: Vec<Point3<f64>>) -> Self {
        Self { points }
    }

    /// Sample guide at parameter t (0-1).
    pub fn sample(&self, t: f64) -> Point3<f64> {
        if self.points.is_empty() {
            return Point3::origin();
        }
        if self.points.len() == 1 {
            return self.points[0];
        }

        let t = t.clamp(0.0, 1.0);
        let idx_f = t * (self.points.len() - 1) as f64;
        let idx = idx_f.floor() as usize;
        let frac = idx_f - idx as f64;

        if idx >= self.points.len() - 1 {
            return self.points[self.points.len() - 1];
        }

        let p0 = self.points[idx];
        let p1 = self.points[idx + 1];

        Point3::new(
            p0.x + (p1.x - p0.x) * frac,
            p0.y + (p1.y - p0.y) * frac,
            p0.z + (p1.z - p0.z) * frac,
        )
    }
}

/// Result of a loft operation.
#[derive(Debug, Clone)]
pub struct LoftedMesh {
    /// Vertex positions.
    pub vertices: Vec<Point3<f64>>,
    /// Vertex normals.
    pub normals: Vec<Vector3<f64>>,
    /// Triangle indices.
    pub indices: Vec<[u32; 3]>,
}

/// Loft multiple profiles together.
pub fn loft_profiles(profiles: &[Profile3D], params: &LoftParams) -> LoftedMesh {
    if profiles.len() < 2 {
        return LoftedMesh {
            vertices: Vec::new(),
            normals: Vec::new(),
            indices: Vec::new(),
        };
    }

    // Normalize profiles to have the same number of points
    let target_points = profiles.iter().map(|p| p.outer.len()).max().unwrap_or(0);
    if target_points < 3 {
        return LoftedMesh {
            vertices: Vec::new(),
            normals: Vec::new(),
            indices: Vec::new(),
        };
    }

    let normalized_profiles: Vec<Vec<Point3<f64>>> = profiles
        .iter()
        .map(|p| resample_profile(&p.outer, target_points))
        .collect();

    let mut vertices = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    let n_profile_points = target_points;
    let n_sections = params.sections;
    let n_profiles = profiles.len();

    // Generate interpolated sections between each pair of profiles
    for profile_idx in 0..(if params.closed {
        n_profiles
    } else {
        n_profiles - 1
    }) {
        let next_profile_idx = (profile_idx + 1) % n_profiles;

        let profile_a = &normalized_profiles[profile_idx];
        let profile_b = &normalized_profiles[next_profile_idx];

        let vertex_base = vertices.len();

        // Generate intermediate sections
        for section in 0..=n_sections {
            let t = section as f64 / n_sections as f64;

            // Apply interpolation based on continuity
            let blend_t = match params.continuity {
                Continuity::C0 => t,
                Continuity::C1 => smoothstep(t),
                Continuity::C2 => smootherstep(t),
            };

            for point_idx in 0..n_profile_points {
                let pa = profile_a[point_idx];
                let pb = profile_b[point_idx];

                // Linear interpolation with optional guide influence
                let mut point = Point3::new(
                    pa.x + (pb.x - pa.x) * blend_t,
                    pa.y + (pb.y - pa.y) * blend_t,
                    pa.z + (pb.z - pa.z) * blend_t,
                );

                // Apply guide curve influence
                if !params.guides.is_empty() {
                    let global_t = (profile_idx as f64 + t) / n_profiles as f64;
                    for guide in &params.guides {
                        let guide_pos = guide.sample(global_t);
                        let influence = 0.1; // Adjust influence factor
                        point = Point3::new(
                            point.x + (guide_pos.x - point.x) * influence,
                            point.y + (guide_pos.y - point.y) * influence,
                            point.z + (guide_pos.z - point.z) * influence,
                        );
                    }
                }

                vertices.push(point);
                normals.push(Vector3::zeros()); // Will be computed later
            }
        }

        // Generate faces for this segment
        for section in 0..n_sections {
            for point_idx in 0..n_profile_points {
                let next_point_idx = (point_idx + 1) % n_profile_points;

                let v00 = (vertex_base + section * n_profile_points + point_idx) as u32;
                let v01 = (vertex_base + section * n_profile_points + next_point_idx) as u32;
                let v10 = (vertex_base + (section + 1) * n_profile_points + point_idx) as u32;
                let v11 = (vertex_base + (section + 1) * n_profile_points + next_point_idx) as u32;

                indices.push([v00, v01, v10]);
                indices.push([v01, v11, v10]);
            }
        }
    }

    // Add caps
    if params.capped && !params.closed {
        // Start cap
        let start_center = profile_center(&normalized_profiles[0]);
        let start_center_idx = vertices.len() as u32;
        vertices.push(start_center);
        normals.push(-profile_normal(&normalized_profiles[0]));

        for i in 0..n_profile_points {
            let next = (i + 1) % n_profile_points;
            indices.push([start_center_idx, next as u32, i as u32]);
        }

        // End cap
        let last_profile = &normalized_profiles[n_profiles - 1];
        let end_center = profile_center(last_profile);
        let end_center_idx = vertices.len() as u32;
        vertices.push(end_center);
        normals.push(profile_normal(last_profile));

        let base = (vertices.len() - 2 - n_profile_points) as u32;
        for i in 0..n_profile_points {
            let next = (i + 1) % n_profile_points;
            indices.push([end_center_idx, base + i as u32, base + next as u32]);
        }
    }

    // Compute normals
    compute_normals(&mut normals, &vertices, &indices);

    LoftedMesh {
        vertices,
        normals,
        indices,
    }
}

/// Loft with blending parameters per profile.
pub fn loft_with_blending(
    profiles: &[Profile3D],
    blend_params: &[BlendParams],
    params: &LoftParams,
) -> LoftedMesh {
    if profiles.len() < 2 || blend_params.len() != profiles.len() {
        return loft_profiles(profiles, params);
    }

    let target_points = profiles.iter().map(|p| p.outer.len()).max().unwrap_or(0);
    if target_points < 3 {
        return LoftedMesh {
            vertices: Vec::new(),
            normals: Vec::new(),
            indices: Vec::new(),
        };
    }

    let normalized_profiles: Vec<Vec<Point3<f64>>> = profiles
        .iter()
        .map(|p| resample_profile(&p.outer, target_points))
        .collect();

    let mut vertices = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    let n_profile_points = target_points;
    let n_sections = params.sections;
    let n_profiles = profiles.len();

    for profile_idx in 0..(if params.closed {
        n_profiles
    } else {
        n_profiles - 1
    }) {
        let next_profile_idx = (profile_idx + 1) % n_profiles;

        let profile_a = &normalized_profiles[profile_idx];
        let profile_b = &normalized_profiles[next_profile_idx];

        let blend_a = &blend_params[profile_idx];
        let blend_b = &blend_params[next_profile_idx];

        let vertex_base = vertices.len();

        for section in 0..=n_sections {
            let t = section as f64 / n_sections as f64;

            // Hermite interpolation with tangent weights
            let h00 = 2.0 * t.powi(3) - 3.0 * t.powi(2) + 1.0;
            let h10 = t.powi(3) - 2.0 * t.powi(2) + t;
            let h01 = -2.0 * t.powi(3) + 3.0 * t.powi(2);
            let h11 = t.powi(3) - t.powi(2);

            for point_idx in 0..n_profile_points {
                let pa = profile_a[point_idx];
                let pb = profile_b[point_idx];

                // Tangent at profile a (pointing toward b)
                let ta = Vector3::new(
                    (pb.x - pa.x) * blend_a.tangent_weight,
                    (pb.y - pa.y) * blend_a.tangent_weight,
                    (pb.z - pa.z) * blend_a.tangent_weight,
                );

                // Tangent at profile b (pointing from a)
                let tb = Vector3::new(
                    (pb.x - pa.x) * blend_b.tangent_weight,
                    (pb.y - pa.y) * blend_b.tangent_weight,
                    (pb.z - pa.z) * blend_b.tangent_weight,
                );

                // Hermite interpolation
                let point = Point3::new(
                    h00 * pa.x + h10 * ta.x + h01 * pb.x + h11 * tb.x,
                    h00 * pa.y + h10 * ta.y + h01 * pb.y + h11 * tb.y,
                    h00 * pa.z + h10 * ta.z + h01 * pb.z + h11 * tb.z,
                );

                // Apply scale
                let scale = blend_a.scale + (blend_b.scale - blend_a.scale) * t;
                let center = profile_center(profile_a);
                let scaled = Point3::new(
                    center.x + (point.x - center.x) * scale,
                    center.y + (point.y - center.y) * scale,
                    center.z + (point.z - center.z) * scale,
                );

                vertices.push(scaled);
                normals.push(Vector3::zeros());
            }
        }

        // Generate faces
        for section in 0..n_sections {
            for point_idx in 0..n_profile_points {
                let next_point_idx = (point_idx + 1) % n_profile_points;

                let v00 = (vertex_base + section * n_profile_points + point_idx) as u32;
                let v01 = (vertex_base + section * n_profile_points + next_point_idx) as u32;
                let v10 = (vertex_base + (section + 1) * n_profile_points + point_idx) as u32;
                let v11 = (vertex_base + (section + 1) * n_profile_points + next_point_idx) as u32;

                indices.push([v00, v01, v10]);
                indices.push([v01, v11, v10]);
            }
        }
    }

    compute_normals(&mut normals, &vertices, &indices);

    LoftedMesh {
        vertices,
        normals,
        indices,
    }
}

/// Blending parameters for a profile.
#[derive(Debug, Clone)]
pub struct BlendParams {
    /// Tangent weight (0-1).
    pub tangent_weight: f64,
    /// Scale factor.
    pub scale: f64,
    /// Rotation (radians).
    pub rotation: f64,
}

impl Default for BlendParams {
    fn default() -> Self {
        Self {
            tangent_weight: 1.0,
            scale: 1.0,
            rotation: 0.0,
        }
    }
}

impl LoftedMesh {
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
    pub fn merge(&mut self, other: &LoftedMesh) {
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

fn resample_profile(points: &[Point3<f64>], target_count: usize) -> Vec<Point3<f64>> {
    if points.len() == target_count {
        return points.to_vec();
    }

    if points.is_empty() {
        return vec![Point3::origin(); target_count];
    }

    // Calculate total length
    let mut lengths = vec![0.0];
    let mut total_length = 0.0;
    for i in 1..points.len() {
        total_length += (points[i] - points[i - 1]).magnitude();
        lengths.push(total_length);
    }
    // Close the loop
    total_length += (points[0] - points[points.len() - 1]).magnitude();

    if total_length < 1e-10 {
        return vec![points[0]; target_count];
    }

    // Resample at uniform arc length
    let mut result = Vec::with_capacity(target_count);
    for i in 0..target_count {
        let target_length = total_length * i as f64 / target_count as f64;

        // Find segment
        let mut seg = 0;
        for (j, &len) in lengths.iter().enumerate().skip(1) {
            if len > target_length {
                seg = j - 1;
                break;
            }
            seg = j;
        }

        let seg_start = lengths[seg];
        let next_seg = (seg + 1) % points.len();
        let seg_length = if next_seg == 0 {
            (points[0] - points[seg]).magnitude()
        } else {
            lengths[next_seg] - seg_start
        };

        let t = if seg_length > 1e-10 {
            (target_length - seg_start) / seg_length
        } else {
            0.0
        };

        let pa = points[seg];
        let pb = points[next_seg];

        result.push(Point3::new(
            pa.x + (pb.x - pa.x) * t,
            pa.y + (pb.y - pa.y) * t,
            pa.z + (pb.z - pa.z) * t,
        ));
    }

    result
}

fn profile_center(points: &[Point3<f64>]) -> Point3<f64> {
    if points.is_empty() {
        return Point3::origin();
    }

    let sum = points
        .iter()
        .fold(Vector3::zeros(), |acc, p| acc + p.coords);

    Point3::from(sum / points.len() as f64)
}

fn profile_normal(points: &[Point3<f64>]) -> Vector3<f64> {
    if points.len() < 3 {
        return Vector3::z();
    }

    // Use Newell's method for computing polygon normal
    let mut normal: Vector3<f64> = Vector3::zeros();

    for i in 0..points.len() {
        let curr = points[i];
        let next = points[(i + 1) % points.len()];

        normal.x += (curr.y - next.y) * (curr.z + next.z);
        normal.y += (curr.z - next.z) * (curr.x + next.x);
        normal.z += (curr.x - next.x) * (curr.y + next.y);
    }

    let len = normal.magnitude();
    if len > 1e-10 {
        normal / len
    } else {
        Vector3::z()
    }
}

fn smoothstep(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

fn smootherstep(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn compute_normals(normals: &mut [Vector3<f64>], vertices: &[Point3<f64>], indices: &[[u32; 3]]) {
    // Clear normals
    for n in normals.iter_mut() {
        *n = Vector3::zeros();
    }

    // Accumulate face normals
    for tri in indices {
        if (tri[0] as usize) >= vertices.len()
            || (tri[1] as usize) >= vertices.len()
            || (tri[2] as usize) >= vertices.len()
        {
            continue;
        }

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

    fn create_circle_profile(center: Point3<f64>, radius: f64, segments: usize) -> Profile3D {
        let mut points = Vec::with_capacity(segments);
        for i in 0..segments {
            let angle = 2.0 * std::f64::consts::PI * i as f64 / segments as f64;
            points.push(Point3::new(
                center.x + radius * angle.cos(),
                center.y,
                center.z + radius * angle.sin(),
            ));
        }
        Profile3D {
            outer: points,
            holes: Vec::new(),
        }
    }

    #[test]
    fn test_basic_loft() {
        let profile1 = create_circle_profile(Point3::new(0.0, 0.0, 0.0), 1.0, 16);
        let profile2 = create_circle_profile(Point3::new(0.0, 10.0, 0.0), 1.0, 16);

        let mesh = loft_profiles(&[profile1, profile2], &LoftParams::default());

        assert!(!mesh.vertices.is_empty());
        assert!(!mesh.indices.is_empty());
    }

    #[test]
    fn test_loft_with_different_sizes() {
        let profile1 = create_circle_profile(Point3::new(0.0, 0.0, 0.0), 1.0, 16);
        let profile2 = create_circle_profile(Point3::new(0.0, 10.0, 0.0), 2.0, 16);

        let mesh = loft_profiles(&[profile1, profile2], &LoftParams::default());

        assert!(!mesh.vertices.is_empty());
    }

    #[test]
    fn test_multi_profile_loft() {
        let profile1 = create_circle_profile(Point3::new(0.0, 0.0, 0.0), 1.0, 16);
        let profile2 = create_circle_profile(Point3::new(0.0, 5.0, 0.0), 2.0, 16);
        let profile3 = create_circle_profile(Point3::new(0.0, 10.0, 0.0), 1.0, 16);

        let mesh = loft_profiles(&[profile1, profile2, profile3], &LoftParams::default());

        assert!(!mesh.vertices.is_empty());
    }

    #[test]
    fn test_loft_with_blending() {
        let profile1 = create_circle_profile(Point3::new(0.0, 0.0, 0.0), 1.0, 16);
        let profile2 = create_circle_profile(Point3::new(0.0, 10.0, 0.0), 1.0, 16);

        let blend_params = vec![
            BlendParams {
                tangent_weight: 0.5,
                scale: 1.0,
                rotation: 0.0,
            },
            BlendParams {
                tangent_weight: 0.5,
                scale: 1.5,
                rotation: 0.0,
            },
        ];

        let mesh = loft_with_blending(&[profile1, profile2], &blend_params, &LoftParams::default());

        assert!(!mesh.vertices.is_empty());
    }

    #[test]
    fn test_resample_profile() {
        let original = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
        ];

        let resampled = resample_profile(&original, 8);

        assert_eq!(resampled.len(), 8);
    }
}
