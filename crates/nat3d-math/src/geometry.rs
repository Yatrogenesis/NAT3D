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

//! Geometric utilities for 3D graphics.
//!
//! Provides common geometric calculations and operations.

use nalgebra::{Point3, Vector3};

/// Type alias for 3D vector.
pub type Vec3 = Vector3<f64>;
/// 3D point type.
pub type Point = Point3<f64>;

/// Calculate the area of a triangle given three points.
#[must_use]
pub fn triangle_area(p0: &Point, p1: &Point, p2: &Point) -> f64 {
    let v1 = p1 - p0;
    let v2 = p2 - p0;
    v1.cross(&v2).magnitude() * 0.5
}

/// Calculate the normal of a triangle given three points.
#[must_use]
pub fn triangle_normal(p0: &Point, p1: &Point, p2: &Point) -> Vec3 {
    let v1 = p1 - p0;
    let v2 = p2 - p0;
    v1.cross(&v2).normalize()
}

/// Calculate the centroid of a triangle.
#[must_use]
pub fn triangle_centroid(p0: &Point, p1: &Point, p2: &Point) -> Point {
    Point::new(
        (p0.x + p1.x + p2.x) / 3.0,
        (p0.y + p1.y + p2.y) / 3.0,
        (p0.z + p1.z + p2.z) / 3.0,
    )
}

/// Calculate barycentric coordinates of a point in a triangle.
/// Returns (u, v, w) where point = u*p0 + v*p1 + w*p2.
#[must_use]
pub fn barycentric(p: &Point, p0: &Point, p1: &Point, p2: &Point) -> (f64, f64, f64) {
    let v0 = p1 - p0;
    let v1 = p2 - p0;
    let v2 = p - p0;

    let d00 = v0.dot(&v0);
    let d01 = v0.dot(&v1);
    let d11 = v1.dot(&v1);
    let d20 = v2.dot(&v0);
    let d21 = v2.dot(&v1);

    let denom = d00 * d11 - d01 * d01;
    if denom.abs() < f64::EPSILON {
        return (1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0);
    }

    let v = (d11 * d20 - d01 * d21) / denom;
    let w = (d00 * d21 - d01 * d20) / denom;
    let u = 1.0 - v - w;

    (u, v, w)
}

/// Check if a point is inside a triangle (2D projection).
#[must_use]
pub fn point_in_triangle(p: &Point, p0: &Point, p1: &Point, p2: &Point) -> bool {
    let (u, v, w) = barycentric(p, p0, p1, p2);
    u >= 0.0 && v >= 0.0 && w >= 0.0
}

/// Calculate the closest point on a line segment to a given point.
#[must_use]
pub fn closest_point_on_segment(p: &Point, a: &Point, b: &Point) -> Point {
    let ab = b - a;
    let ap = p - a;

    let t = ap.dot(&ab) / ab.dot(&ab);
    let t = t.clamp(0.0, 1.0);

    a + ab * t
}

/// Calculate the distance from a point to a line segment.
#[must_use]
pub fn distance_to_segment(p: &Point, a: &Point, b: &Point) -> f64 {
    let closest = closest_point_on_segment(p, a, b);
    (p - closest).magnitude()
}

/// Calculate the closest point on a plane to a given point.
#[must_use]
pub fn closest_point_on_plane(p: &Point, plane_point: &Point, plane_normal: &Vec3) -> Point {
    let d = (p - plane_point).dot(plane_normal);
    p - plane_normal * d
}

/// Calculate the distance from a point to a plane.
#[must_use]
pub fn distance_to_plane(p: &Point, plane_point: &Point, plane_normal: &Vec3) -> f64 {
    (p - plane_point).dot(plane_normal).abs()
}

/// Signed distance from a point to a plane (positive if on normal side).
#[must_use]
pub fn signed_distance_to_plane(p: &Point, plane_point: &Point, plane_normal: &Vec3) -> f64 {
    (p - plane_point).dot(plane_normal)
}

/// Ray-plane intersection.
/// Returns the parameter t where the intersection occurs (ray_origin + t * ray_dir).
/// Returns None if the ray is parallel to the plane.
#[must_use]
pub fn ray_plane_intersection(
    ray_origin: &Point,
    ray_dir: &Vec3,
    plane_point: &Point,
    plane_normal: &Vec3,
) -> Option<f64> {
    let denom = ray_dir.dot(plane_normal);
    if denom.abs() < f64::EPSILON {
        return None;
    }

    let t = (plane_point - ray_origin).dot(plane_normal) / denom;
    Some(t)
}

/// Ray-sphere intersection.
/// Returns the parameters t1 and t2 where intersections occur.
/// Returns None if there's no intersection.
#[must_use]
pub fn ray_sphere_intersection(
    ray_origin: &Point,
    ray_dir: &Vec3,
    sphere_center: &Point,
    sphere_radius: f64,
) -> Option<(f64, f64)> {
    let oc = ray_origin - sphere_center;
    let a = ray_dir.dot(ray_dir);
    let b = 2.0 * oc.dot(ray_dir);
    let c = oc.dot(&oc) - sphere_radius * sphere_radius;

    let discriminant = b * b - 4.0 * a * c;
    if discriminant < 0.0 {
        return None;
    }

    let sqrt_d = discriminant.sqrt();
    let t1 = (-b - sqrt_d) / (2.0 * a);
    let t2 = (-b + sqrt_d) / (2.0 * a);

    Some((t1, t2))
}

/// Ray-box intersection (axis-aligned bounding box).
/// Returns (t_enter, t_exit) or None if no intersection.
#[must_use]
pub fn ray_aabb_intersection(
    ray_origin: &Point,
    ray_dir: &Vec3,
    box_min: &Point,
    box_max: &Point,
) -> Option<(f64, f64)> {
    let mut t_min = f64::NEG_INFINITY;
    let mut t_max = f64::INFINITY;

    for i in 0..3 {
        let inv_d = 1.0 / ray_dir[i];
        let mut t0 = (box_min[i] - ray_origin[i]) * inv_d;
        let mut t1 = (box_max[i] - ray_origin[i]) * inv_d;

        if inv_d < 0.0 {
            std::mem::swap(&mut t0, &mut t1);
        }

        t_min = t_min.max(t0);
        t_max = t_max.min(t1);

        if t_max < t_min {
            return None;
        }
    }

    Some((t_min, t_max))
}

/// Calculate the reflection of a vector around a normal.
#[must_use]
pub fn reflect(incident: &Vec3, normal: &Vec3) -> Vec3 {
    incident - normal * (2.0 * incident.dot(normal))
}

/// Calculate the refraction of a vector through a surface.
/// Returns None if total internal reflection occurs.
#[must_use]
pub fn refract(incident: &Vec3, normal: &Vec3, eta: f64) -> Option<Vec3> {
    let cos_i = -incident.dot(normal);
    let sin_t2 = eta * eta * (1.0 - cos_i * cos_i);

    if sin_t2 > 1.0 {
        return None; // Total internal reflection
    }

    let cos_t = (1.0 - sin_t2).sqrt();
    Some(incident * eta + normal * (eta * cos_i - cos_t))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_triangle_area() {
        let p0 = Point::new(0.0, 0.0, 0.0);
        let p1 = Point::new(2.0, 0.0, 0.0);
        let p2 = Point::new(0.0, 2.0, 0.0);

        let area = triangle_area(&p0, &p1, &p2);
        assert!((area - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_triangle_normal() {
        let p0 = Point::new(0.0, 0.0, 0.0);
        let p1 = Point::new(1.0, 0.0, 0.0);
        let p2 = Point::new(0.0, 1.0, 0.0);

        let normal = triangle_normal(&p0, &p1, &p2);
        assert!((normal.z - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_barycentric() {
        let p0 = Point::new(0.0, 0.0, 0.0);
        let p1 = Point::new(1.0, 0.0, 0.0);
        let p2 = Point::new(0.0, 1.0, 0.0);

        let centroid = triangle_centroid(&p0, &p1, &p2);
        let (u, v, w) = barycentric(&centroid, &p0, &p1, &p2);

        assert!((u - 1.0 / 3.0).abs() < 1e-10);
        assert!((v - 1.0 / 3.0).abs() < 1e-10);
        assert!((w - 1.0 / 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_ray_sphere() {
        let origin = Point::new(0.0, 0.0, -5.0);
        let dir = Vec3::new(0.0, 0.0, 1.0);
        let center = Point::new(0.0, 0.0, 0.0);

        let result = ray_sphere_intersection(&origin, &dir, &center, 1.0);
        assert!(result.is_some());

        let (t1, t2) = result.unwrap();
        assert!((t1 - 4.0).abs() < 1e-10);
        assert!((t2 - 6.0).abs() < 1e-10);
    }

    #[test]
    fn test_reflect() {
        let incident = Vec3::new(1.0, -1.0, 0.0).normalize();
        let normal = Vec3::new(0.0, 1.0, 0.0);

        let reflected = reflect(&incident, &normal);
        assert!((reflected.x - incident.x).abs() < 1e-10);
        assert!((reflected.y + incident.y).abs() < 1e-10);
    }
}
