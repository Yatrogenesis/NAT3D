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

//! Ray intersection data and utilities.
//!
//! Stores information about ray-surface intersections for shading.

use nalgebra::{Point2, Point3, Vector3};

/// Information about a ray intersection.
#[derive(Debug, Clone)]
pub struct Intersection {
    /// Hit point in world space.
    pub point: Point3<f64>,
    /// Surface normal at hit point.
    pub normal: Vector3<f64>,
    /// Geometric normal (before normal mapping).
    pub geometric_normal: Vector3<f64>,
    /// Texture coordinates.
    pub uv: Point2<f64>,
    /// Distance along ray.
    pub t: f64,
    /// Primitive index.
    pub primitive_id: usize,
    /// Material index.
    pub material_id: usize,
    /// Whether hit was on front face.
    pub front_face: bool,
    /// Tangent vector for normal mapping.
    pub tangent: Vector3<f64>,
    /// Bitangent vector for normal mapping.
    pub bitangent: Vector3<f64>,
}

impl Intersection {
    /// Create a new intersection.
    pub fn new(
        point: Point3<f64>,
        normal: Vector3<f64>,
        uv: Point2<f64>,
        t: f64,
        primitive_id: usize,
    ) -> Self {
        Self {
            point,
            normal,
            geometric_normal: normal,
            uv,
            t,
            primitive_id,
            material_id: 0,
            front_face: true,
            tangent: Vector3::new(1.0, 0.0, 0.0),
            bitangent: Vector3::new(0.0, 1.0, 0.0),
        }
    }

    /// Set the face normal based on ray direction.
    pub fn set_face_normal(&mut self, ray_direction: &Vector3<f64>, outward_normal: &Vector3<f64>) {
        self.front_face = ray_direction.dot(outward_normal) < 0.0;
        self.normal = if self.front_face {
            *outward_normal
        } else {
            -outward_normal
        };
    }

    /// Compute tangent frame from normal.
    pub fn compute_tangent_frame(&mut self) {
        let n = self.normal;

        let tangent = if n.x.abs() > 0.9 {
            Vector3::new(0.0, 1.0, 0.0)
        } else {
            Vector3::new(1.0, 0.0, 0.0)
        };

        self.bitangent = n.cross(&tangent).normalize();
        self.tangent = self.bitangent.cross(&n).normalize();
    }

    /// Apply normal map perturbation.
    pub fn apply_normal_map(&mut self, normal_map: Vector3<f64>) {
        let perturbed = self.tangent * normal_map.x
            + self.bitangent * normal_map.y
            + self.normal * normal_map.z;

        self.normal = perturbed.normalize();
    }

    /// Offset intersection point to avoid self-intersection.
    pub fn offset_point(&self, direction: &Vector3<f64>) -> Point3<f64> {
        let offset = if direction.dot(&self.geometric_normal) > 0.0 {
            self.geometric_normal * 1e-4
        } else {
            -self.geometric_normal * 1e-4
        };

        self.point + offset
    }
}

/// Barycentric coordinates for triangle interpolation.
#[derive(Debug, Clone, Copy)]
pub struct BarycentricCoords {
    /// u coordinate.
    pub u: f64,
    /// v coordinate.
    pub v: f64,
    /// w coordinate.
    pub w: f64,
}

impl BarycentricCoords {
    /// Create from u, v (w is computed).
    pub fn new(u: f64, v: f64) -> Self {
        Self {
            u,
            v,
            w: 1.0 - u - v,
        }
    }

    /// Interpolate a value using barycentric coordinates.
    pub fn interpolate<T>(&self, a: T, b: T, c: T) -> T
    where
        T: std::ops::Mul<f64, Output = T> + std::ops::Add<Output = T>,
    {
        a * self.w + b * self.u + c * self.v
    }

    /// Check if coordinates are valid (inside triangle).
    pub fn is_valid(&self) -> bool {
        self.u >= 0.0
            && self.v >= 0.0
            && self.w >= 0.0
            && (self.u + self.v + self.w - 1.0).abs() < 1e-6
    }
}

/// Compute barycentric coordinates for a point in a triangle.
pub fn compute_barycentric(
    p: Point3<f64>,
    v0: Point3<f64>,
    v1: Point3<f64>,
    v2: Point3<f64>,
) -> BarycentricCoords {
    let v0v1 = v1 - v0;
    let v0v2 = v2 - v0;
    let v0p = p - v0;

    let d00 = v0v1.dot(&v0v1);
    let d01 = v0v1.dot(&v0v2);
    let d11 = v0v2.dot(&v0v2);
    let d20 = v0p.dot(&v0v1);
    let d21 = v0p.dot(&v0v2);

    let denom = d00 * d11 - d01 * d01;

    if denom.abs() < 1e-10 {
        return BarycentricCoords::new(0.0, 0.0);
    }

    let v = (d11 * d20 - d01 * d21) / denom;
    let w = (d00 * d21 - d01 * d20) / denom;

    BarycentricCoords {
        u: v,
        v: w,
        w: 1.0 - v - w,
    }
}

/// Interpolate triangle attributes.
pub fn interpolate_triangle<T>(bary: &BarycentricCoords, v0: T, v1: T, v2: T) -> T
where
    T: std::ops::Mul<f64, Output = T> + std::ops::Add<Output = T>,
{
    bary.interpolate(v0, v1, v2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_barycentric_center() {
        let v0 = Point3::new(0.0, 0.0, 0.0);
        let v1 = Point3::new(1.0, 0.0, 0.0);
        let v2 = Point3::new(0.0, 1.0, 0.0);

        let center = Point3::new(1.0 / 3.0, 1.0 / 3.0, 0.0);
        let bary = compute_barycentric(center, v0, v1, v2);

        assert!(bary.is_valid());
        assert!((bary.u - 1.0 / 3.0).abs() < 1e-6);
        assert!((bary.v - 1.0 / 3.0).abs() < 1e-6);
        assert!((bary.w - 1.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn test_barycentric_vertex() {
        let v0 = Point3::new(0.0, 0.0, 0.0);
        let v1 = Point3::new(1.0, 0.0, 0.0);
        let v2 = Point3::new(0.0, 1.0, 0.0);

        let bary = compute_barycentric(v0, v0, v1, v2);
        assert!((bary.w - 1.0).abs() < 1e-6);

        let bary = compute_barycentric(v1, v0, v1, v2);
        assert!((bary.u - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_intersection_offset() {
        let isect = Intersection::new(
            Point3::new(0.0, 0.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
            Point2::new(0.5, 0.5),
            1.0,
            0,
        );

        let outgoing = Vector3::new(0.0, 1.0, 0.0);
        let offset_point = isect.offset_point(&outgoing);

        assert!(offset_point.y > 0.0);
    }
}
