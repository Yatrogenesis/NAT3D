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

//! Ray structure and operations.
//!
//! Core ray representation for ray tracing and intersection testing.

use nalgebra::{Point3, Vector3};

/// A ray in 3D space.
#[derive(Debug, Clone, Copy)]
pub struct Ray {
    /// Ray origin point.
    pub origin: Point3<f64>,
    /// Ray direction (normalized).
    pub direction: Vector3<f64>,
    /// Inverse direction for fast AABB intersection.
    pub inv_direction: Vector3<f64>,
    /// Sign of direction components for AABB intersection.
    pub sign: [usize; 3],
    /// Minimum distance along ray.
    pub t_min: f64,
    /// Maximum distance along ray.
    pub t_max: f64,
}

impl Ray {
    /// Create a new ray.
    pub fn new(origin: Point3<f64>, direction: Vector3<f64>) -> Self {
        let direction = direction.normalize();
        let inv_direction = Vector3::new(1.0 / direction.x, 1.0 / direction.y, 1.0 / direction.z);
        let sign = [
            (inv_direction.x < 0.0) as usize,
            (inv_direction.y < 0.0) as usize,
            (inv_direction.z < 0.0) as usize,
        ];

        Self {
            origin,
            direction,
            inv_direction,
            sign,
            t_min: 0.0,
            t_max: f64::INFINITY,
        }
    }

    /// Create ray with custom t range.
    pub fn with_range(
        origin: Point3<f64>,
        direction: Vector3<f64>,
        t_min: f64,
        t_max: f64,
    ) -> Self {
        let mut ray = Self::new(origin, direction);
        ray.t_min = t_min;
        ray.t_max = t_max;
        ray
    }

    /// Get point at parameter t.
    pub fn at(&self, t: f64) -> Point3<f64> {
        self.origin + self.direction * t
    }

    /// Transform ray by a matrix.
    pub fn transform(&self, matrix: &nalgebra::Matrix4<f64>) -> Self {
        let new_origin = matrix.transform_point(&self.origin);
        let new_direction = matrix.transform_vector(&self.direction);
        Self::with_range(new_origin, new_direction, self.t_min, self.t_max)
    }
}

/// Axis-aligned bounding box.
#[derive(Debug, Clone, Copy)]
pub struct Aabb {
    /// Minimum corner.
    pub min: Point3<f64>,
    /// Maximum corner.
    pub max: Point3<f64>,
}

impl Aabb {
    /// Create a new AABB.
    pub fn new(min: Point3<f64>, max: Point3<f64>) -> Self {
        Self { min, max }
    }

    /// Create an empty AABB.
    pub fn empty() -> Self {
        Self {
            min: Point3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY),
            max: Point3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY),
        }
    }

    /// Create AABB from a single point.
    pub fn from_point(p: Point3<f64>) -> Self {
        Self { min: p, max: p }
    }

    /// Expand AABB to include a point.
    pub fn include_point(&mut self, p: Point3<f64>) {
        self.min.x = self.min.x.min(p.x);
        self.min.y = self.min.y.min(p.y);
        self.min.z = self.min.z.min(p.z);
        self.max.x = self.max.x.max(p.x);
        self.max.y = self.max.y.max(p.y);
        self.max.z = self.max.z.max(p.z);
    }

    /// Expand AABB to include another AABB.
    pub fn include_aabb(&mut self, other: &Aabb) {
        self.include_point(other.min);
        self.include_point(other.max);
    }

    /// Union of two AABBs.
    pub fn union(&self, other: &Aabb) -> Aabb {
        Aabb {
            min: Point3::new(
                self.min.x.min(other.min.x),
                self.min.y.min(other.min.y),
                self.min.z.min(other.min.z),
            ),
            max: Point3::new(
                self.max.x.max(other.max.x),
                self.max.y.max(other.max.y),
                self.max.z.max(other.max.z),
            ),
        }
    }

    /// Check if AABB is valid (min <= max for all axes).
    pub fn is_valid(&self) -> bool {
        self.min.x <= self.max.x && self.min.y <= self.max.y && self.min.z <= self.max.z
    }

    /// Compute center of AABB.
    pub fn center(&self) -> Point3<f64> {
        Point3::new(
            (self.min.x + self.max.x) * 0.5,
            (self.min.y + self.max.y) * 0.5,
            (self.min.z + self.max.z) * 0.5,
        )
    }

    /// Compute extent (size) of AABB.
    pub fn extent(&self) -> Vector3<f64> {
        self.max - self.min
    }

    /// Compute surface area.
    pub fn surface_area(&self) -> f64 {
        let extent = self.extent();
        2.0 * (extent.x * extent.y + extent.y * extent.z + extent.z * extent.x)
    }

    /// Get axis with largest extent.
    pub fn max_axis(&self) -> usize {
        let extent = self.extent();
        if extent.x > extent.y && extent.x > extent.z {
            0
        } else if extent.y > extent.z {
            1
        } else {
            2
        }
    }

    /// Ray-AABB intersection test (slab method).
    /// Returns (t_enter, t_exit) if intersection exists.
    pub fn intersect(&self, ray: &Ray) -> Option<(f64, f64)> {
        let bounds = [self.min, self.max];

        let mut t_min = (bounds[ray.sign[0]].x - ray.origin.x) * ray.inv_direction.x;
        let mut t_max = (bounds[1 - ray.sign[0]].x - ray.origin.x) * ray.inv_direction.x;

        let ty_min = (bounds[ray.sign[1]].y - ray.origin.y) * ray.inv_direction.y;
        let ty_max = (bounds[1 - ray.sign[1]].y - ray.origin.y) * ray.inv_direction.y;

        if t_min > ty_max || ty_min > t_max {
            return None;
        }

        t_min = t_min.max(ty_min);
        t_max = t_max.min(ty_max);

        let tz_min = (bounds[ray.sign[2]].z - ray.origin.z) * ray.inv_direction.z;
        let tz_max = (bounds[1 - ray.sign[2]].z - ray.origin.z) * ray.inv_direction.z;

        if t_min > tz_max || tz_min > t_max {
            return None;
        }

        t_min = t_min.max(tz_min);
        t_max = t_max.min(tz_max);

        if t_max < ray.t_min || t_min > ray.t_max {
            return None;
        }

        Some((t_min.max(ray.t_min), t_max.min(ray.t_max)))
    }

    /// Check if point is inside AABB.
    pub fn contains(&self, p: Point3<f64>) -> bool {
        p.x >= self.min.x
            && p.x <= self.max.x
            && p.y >= self.min.y
            && p.y <= self.max.y
            && p.z >= self.min.z
            && p.z <= self.max.z
    }

    /// Expand AABB by a margin.
    pub fn expand(&self, margin: f64) -> Aabb {
        Aabb {
            min: Point3::new(
                self.min.x - margin,
                self.min.y - margin,
                self.min.z - margin,
            ),
            max: Point3::new(
                self.max.x + margin,
                self.max.y + margin,
                self.max.z + margin,
            ),
        }
    }
}

impl Default for Aabb {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ray_at() {
        let ray = Ray::new(Point3::new(0.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0));

        let p = ray.at(5.0);
        assert!((p.x - 5.0).abs() < 1e-10);
        assert!(p.y.abs() < 1e-10);
        assert!(p.z.abs() < 1e-10);
    }

    #[test]
    fn test_aabb_intersection() {
        let aabb = Aabb::new(Point3::new(-1.0, -1.0, -1.0), Point3::new(1.0, 1.0, 1.0));

        // Ray from outside pointing at box
        let ray = Ray::new(Point3::new(-5.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0));

        let hit = aabb.intersect(&ray);
        assert!(hit.is_some());
        let (t_enter, t_exit) = hit.unwrap();
        assert!((t_enter - 4.0).abs() < 1e-10);
        assert!((t_exit - 6.0).abs() < 1e-10);
    }

    #[test]
    fn test_aabb_miss() {
        let aabb = Aabb::new(Point3::new(-1.0, -1.0, -1.0), Point3::new(1.0, 1.0, 1.0));

        // Ray pointing away from box
        let ray = Ray::new(Point3::new(-5.0, 0.0, 0.0), Vector3::new(-1.0, 0.0, 0.0));

        assert!(aabb.intersect(&ray).is_none());
    }

    #[test]
    fn test_aabb_center() {
        let aabb = Aabb::new(Point3::new(0.0, 0.0, 0.0), Point3::new(2.0, 4.0, 6.0));

        let center = aabb.center();
        assert!((center.x - 1.0).abs() < 1e-10);
        assert!((center.y - 2.0).abs() < 1e-10);
        assert!((center.z - 3.0).abs() < 1e-10);
    }
}
