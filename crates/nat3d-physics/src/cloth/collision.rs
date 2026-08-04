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

//! Cloth collision detection.
//!
//! Implements collision detection and response for cloth with various colliders.

use nalgebra::Vector3;

/// Collider types for cloth.
#[derive(Debug, Clone)]
pub enum ClothCollider {
    /// Sphere collider.
    Sphere {
        /// Center of the sphere.
        center: Vector3<f64>,
        /// Radius of the sphere.
        radius: f64,
    },
    /// Capsule collider.
    Capsule {
        /// First endpoint of the capsule axis.
        point_a: Vector3<f64>,
        /// Second endpoint of the capsule axis.
        point_b: Vector3<f64>,
        /// Radius of the capsule.
        radius: f64,
    },
    /// Plane collider.
    Plane {
        /// A point on the plane.
        point: Vector3<f64>,
        /// Normal vector of the plane.
        normal: Vector3<f64>,
    },
    /// Triangle mesh collider.
    Mesh {
        /// Mesh vertices.
        vertices: Vec<Vector3<f64>>,
        /// Triangle indices into the vertex array.
        indices: Vec<[usize; 3]>,
    },
}

impl ClothCollider {
    /// Create a sphere collider.
    pub fn sphere(center: Vector3<f64>, radius: f64) -> Self {
        Self::Sphere { center, radius }
    }

    /// Create a capsule collider.
    pub fn capsule(point_a: Vector3<f64>, point_b: Vector3<f64>, radius: f64) -> Self {
        Self::Capsule {
            point_a,
            point_b,
            radius,
        }
    }

    /// Create a plane collider.
    pub fn plane(point: Vector3<f64>, normal: Vector3<f64>) -> Self {
        Self::Plane {
            point,
            normal: normal.normalize(),
        }
    }

    /// Create a ground plane.
    pub fn ground(height: f64) -> Self {
        Self::Plane {
            point: Vector3::new(0.0, height, 0.0),
            normal: Vector3::new(0.0, 1.0, 0.0),
        }
    }
}

/// Cloth collision detection and response.
pub struct ClothCollision {
    /// List of colliders.
    pub colliders: Vec<ClothCollider>,
    /// Friction coefficient for collisions.
    pub friction: f64,
    /// Use continuous collision detection?
    pub use_ccd: bool,
}

impl ClothCollision {
    /// Create a new cloth collision handler.
    pub fn new() -> Self {
        Self {
            colliders: Vec::new(),
            friction: 0.3,
            use_ccd: false,
        }
    }

    /// Add a collider.
    pub fn add_collider(&mut self, collider: ClothCollider) {
        self.colliders.push(collider);
    }

    /// Clear all colliders.
    pub fn clear(&mut self) {
        self.colliders.clear();
    }

    /// Detect and resolve collisions for a single cloth particle.
    pub fn detect_cloth_collisions(
        &self,
        position: Vector3<f64>,
        velocity: &mut Vector3<f64>,
    ) -> Vector3<f64> {
        let mut total_correction = Vector3::zeros();

        for collider in &self.colliders {
            let correction = match collider {
                ClothCollider::Sphere { center, radius } => {
                    self.resolve_sphere_collision(position, velocity, *center, *radius)
                }
                ClothCollider::Capsule {
                    point_a,
                    point_b,
                    radius,
                } => {
                    self.resolve_capsule_collision(position, velocity, *point_a, *point_b, *radius)
                }
                ClothCollider::Plane { point, normal } => {
                    self.resolve_plane_collision(position, velocity, *point, *normal)
                }
                ClothCollider::Mesh { vertices, indices } => {
                    self.resolve_mesh_collision(position, velocity, vertices, indices)
                }
            };

            total_correction += correction;
        }

        total_correction
    }

    /// Resolve collision with a sphere.
    fn resolve_sphere_collision(
        &self,
        position: Vector3<f64>,
        velocity: &mut Vector3<f64>,
        center: Vector3<f64>,
        radius: f64,
    ) -> Vector3<f64> {
        let delta = position - center;
        let dist = delta.magnitude();

        if dist < radius && dist > 1e-10 {
            let normal = delta / dist;
            let penetration = radius - dist;

            // Apply friction
            let v_n = velocity.dot(&normal) * normal;
            let v_t = *velocity - v_n;

            if v_t.magnitude() > 1e-10 {
                *velocity = v_t * (1.0 - self.friction).max(0.0) + v_n * 0.1;
            }

            // Reflect normal velocity
            if velocity.dot(&normal) < 0.0 {
                *velocity -= v_n * 1.1;
            }

            normal * penetration
        } else {
            Vector3::zeros()
        }
    }

    /// Resolve collision with a capsule.
    fn resolve_capsule_collision(
        &self,
        position: Vector3<f64>,
        velocity: &mut Vector3<f64>,
        point_a: Vector3<f64>,
        point_b: Vector3<f64>,
        radius: f64,
    ) -> Vector3<f64> {
        // Find closest point on capsule axis
        let axis = point_b - point_a;
        let axis_length = axis.magnitude();

        if axis_length < 1e-10 {
            // Degenerate to sphere
            return self.resolve_sphere_collision(position, velocity, point_a, radius);
        }

        let axis_norm = axis / axis_length;
        let delta = position - point_a;
        let t = delta.dot(&axis_norm).clamp(0.0, axis_length);
        let closest_point = point_a + axis_norm * t;

        // Treat as sphere collision at closest point
        self.resolve_sphere_collision(position, velocity, closest_point, radius)
    }

    /// Resolve collision with a plane.
    fn resolve_plane_collision(
        &self,
        position: Vector3<f64>,
        velocity: &mut Vector3<f64>,
        plane_point: Vector3<f64>,
        plane_normal: Vector3<f64>,
    ) -> Vector3<f64> {
        let d = (position - plane_point).dot(&plane_normal);

        if d < 0.0 {
            // Below plane
            let correction = -d * plane_normal;

            // Apply friction
            let v_n = velocity.dot(&plane_normal) * plane_normal;
            let v_t = *velocity - v_n;

            if v_t.magnitude() > 1e-10 {
                *velocity = v_t * (1.0 - self.friction).max(0.0);
            }

            // Reflect normal velocity with damping
            if velocity.dot(&plane_normal) < 0.0 {
                *velocity -= v_n * 1.1;
            }

            correction
        } else {
            Vector3::zeros()
        }
    }

    /// Resolve collision with a triangle mesh (simplified).
    fn resolve_mesh_collision(
        &self,
        position: Vector3<f64>,
        velocity: &mut Vector3<f64>,
        vertices: &[Vector3<f64>],
        indices: &[[usize; 3]],
    ) -> Vector3<f64> {
        let mut min_correction = Vector3::zeros();
        let mut min_dist = f64::INFINITY;

        // Check each triangle
        for tri in indices {
            if tri[0] >= vertices.len() || tri[1] >= vertices.len() || tri[2] >= vertices.len() {
                continue;
            }

            let v0 = vertices[tri[0]];
            let v1 = vertices[tri[1]];
            let v2 = vertices[tri[2]];

            // Compute triangle normal
            let e1 = v1 - v0;
            let e2 = v2 - v0;
            let normal = e1.cross(&e2);
            let area = normal.magnitude();

            if area < 1e-10 {
                continue;
            }

            let normal = normal / area;

            // Project point onto triangle plane
            let d = (position - v0).dot(&normal);

            if d.abs() < min_dist {
                // Check if projection is inside triangle
                let proj = position - normal * d;
                if self.point_in_triangle(proj, v0, v1, v2) {
                    min_dist = d.abs();
                    if d < 0.0 {
                        min_correction = -d * normal;
                    }
                }
            }
        }

        if min_dist < f64::INFINITY {
            // Apply friction
            let normal = min_correction.normalize();
            let v_n = velocity.dot(&normal) * normal;
            let v_t = *velocity - v_n;

            if v_t.magnitude() > 1e-10 {
                *velocity = v_t * (1.0 - self.friction).max(0.0);
            }

            if velocity.dot(&normal) < 0.0 {
                *velocity -= v_n * 1.1;
            }
        }

        min_correction
    }

    /// Check if point is inside triangle (2D test in triangle plane).
    fn point_in_triangle(
        &self,
        p: Vector3<f64>,
        v0: Vector3<f64>,
        v1: Vector3<f64>,
        v2: Vector3<f64>,
    ) -> bool {
        let e0 = v1 - v0;
        let e1 = v2 - v1;
        let e2 = v0 - v2;

        let c0 = p - v0;
        let c1 = p - v1;
        let c2 = p - v2;

        let n = e0.cross(&(v2 - v0));

        let d0 = e0.cross(&c0).dot(&n);
        let d1 = e1.cross(&c1).dot(&n);
        let d2 = e2.cross(&c2).dot(&n);

        (d0 >= 0.0 && d1 >= 0.0 && d2 >= 0.0) || (d0 <= 0.0 && d1 <= 0.0 && d2 <= 0.0)
    }

    /// Resolve penetration for a particle (discrete).
    pub fn resolve_penetration(
        &self,
        position: Vector3<f64>,
        velocity: &mut Vector3<f64>,
    ) -> Vector3<f64> {
        self.detect_cloth_collisions(position, velocity)
    }
}

impl Default for ClothCollision {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sphere_collision() {
        let mut collision = ClothCollision::new();
        collision.add_collider(ClothCollider::sphere(Vector3::zeros(), 1.0));

        let position = Vector3::new(0.5, 0.0, 0.0);
        let mut velocity = Vector3::new(-1.0, 0.0, 0.0);

        let correction = collision.detect_cloth_collisions(position, &mut velocity);

        // Should push particle outward
        assert!(correction.x > 0.0);
    }

    #[test]
    fn test_plane_collision() {
        let mut collision = ClothCollision::new();
        collision.add_collider(ClothCollider::ground(0.0));

        let position = Vector3::new(0.0, -0.5, 0.0);
        let mut velocity = Vector3::new(0.0, -1.0, 0.0);

        let correction = collision.detect_cloth_collisions(position, &mut velocity);

        // Should push particle up
        assert!(correction.y > 0.0);
    }

    #[test]
    fn test_capsule_collision() {
        let mut collision = ClothCollision::new();
        collision.add_collider(ClothCollider::capsule(
            Vector3::new(0.0, -1.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
            0.5,
        ));

        let position = Vector3::new(0.3, 0.0, 0.0);
        let mut velocity = Vector3::zeros();

        let correction = collision.detect_cloth_collisions(position, &mut velocity);

        // Should have some correction
        assert!(correction.magnitude() > 0.0);
    }
}
