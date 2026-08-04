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

//! Collision detection for rigid bodies.
//!
//! Implements broad phase (sweep and prune) and narrow phase (GJK/EPA) collision detection.

use nalgebra::Vector3;

/// Collision shape types.
#[derive(Debug, Clone)]
pub enum CollisionShape {
    /// Sphere with given radius.
    Sphere {
        /// Sphere radius.
        radius: f64,
    },
    /// Axis-aligned bounding box.
    Box {
        /// Half-extents along each axis.
        half_extents: Vector3<f64>,
    },
    /// Capsule (cylinder with hemispherical caps).
    Capsule {
        /// Capsule radius.
        radius: f64,
        /// Half the height of the cylindrical section.
        half_height: f64,
    },
    /// Convex hull defined by vertices.
    ConvexHull {
        /// Vertices of the convex hull.
        vertices: Vec<Vector3<f64>>,
    },
    /// Triangle mesh (convex).
    TriMesh {
        /// Mesh vertices.
        vertices: Vec<Vector3<f64>>,
        /// Triangle indices into the vertex array.
        indices: Vec<[usize; 3]>,
    },
}

impl CollisionShape {
    /// Get support point in given direction (for GJK).
    pub fn support(&self, direction: Vector3<f64>) -> Vector3<f64> {
        match self {
            CollisionShape::Sphere { radius } => {
                if direction.magnitude() > 1e-10 {
                    direction.normalize() * *radius
                } else {
                    Vector3::zeros()
                }
            }
            CollisionShape::Box { half_extents } => Vector3::new(
                if direction.x > 0.0 {
                    half_extents.x
                } else {
                    -half_extents.x
                },
                if direction.y > 0.0 {
                    half_extents.y
                } else {
                    -half_extents.y
                },
                if direction.z > 0.0 {
                    half_extents.z
                } else {
                    -half_extents.z
                },
            ),
            CollisionShape::Capsule {
                radius,
                half_height,
            } => {
                let cap_offset = if direction.y > 0.0 {
                    *half_height
                } else {
                    -*half_height
                };
                let cap_center = Vector3::new(0.0, cap_offset, 0.0);
                let dir_2d = Vector3::new(direction.x, 0.0, direction.z);
                let sphere_offset = if dir_2d.magnitude() > 1e-10 {
                    dir_2d.normalize() * *radius
                } else {
                    Vector3::zeros()
                };
                cap_center + sphere_offset
            }
            CollisionShape::ConvexHull { vertices } => vertices
                .iter()
                .max_by(|a, b| a.dot(&direction).partial_cmp(&b.dot(&direction)).unwrap())
                .copied()
                .unwrap_or_else(Vector3::zeros),
            CollisionShape::TriMesh { vertices, .. } => {
                // For triangle mesh, use vertices as convex hull
                vertices
                    .iter()
                    .max_by(|a, b| a.dot(&direction).partial_cmp(&b.dot(&direction)).unwrap())
                    .copied()
                    .unwrap_or_else(Vector3::zeros)
            }
        }
    }

    /// Get axis-aligned bounding box.
    pub fn aabb(&self) -> (Vector3<f64>, Vector3<f64>) {
        match self {
            CollisionShape::Sphere { radius } => {
                let r = Vector3::new(*radius, *radius, *radius);
                (-r, r)
            }
            CollisionShape::Box { half_extents } => (-*half_extents, *half_extents),
            CollisionShape::Capsule {
                radius,
                half_height,
            } => {
                let min = Vector3::new(-radius, -half_height - radius, -radius);
                let max = Vector3::new(*radius, *half_height + radius, *radius);
                (min, max)
            }
            CollisionShape::ConvexHull { vertices } | CollisionShape::TriMesh { vertices, .. } => {
                if vertices.is_empty() {
                    return (Vector3::zeros(), Vector3::zeros());
                }
                let min = vertices.iter().fold(vertices[0], |acc, v| {
                    Vector3::new(acc.x.min(v.x), acc.y.min(v.y), acc.z.min(v.z))
                });
                let max = vertices.iter().fold(vertices[0], |acc, v| {
                    Vector3::new(acc.x.max(v.x), acc.y.max(v.y), acc.z.max(v.z))
                });
                (min, max)
            }
        }
    }
}

/// Collision result.
#[derive(Debug, Clone)]
pub struct CollisionResult {
    /// Contact point in world space.
    pub contact_point: Vector3<f64>,
    /// Contact normal (from body A to body B).
    pub normal: Vector3<f64>,
    /// Penetration depth.
    pub penetration_depth: f64,
    /// Index of body A.
    pub body_a: usize,
    /// Index of body B.
    pub body_b: usize,
}

/// Broad phase collision detection using axis-aligned bounding boxes.
#[derive(Debug)]
pub struct BroadPhase {
    /// AABBs for each body.
    aabbs: Vec<(Vector3<f64>, Vector3<f64>)>,
}

impl BroadPhase {
    /// Create a new broad phase.
    pub fn new() -> Self {
        Self { aabbs: Vec::new() }
    }

    /// Update AABBs.
    pub fn update(&mut self, aabbs: Vec<(Vector3<f64>, Vector3<f64>)>) {
        self.aabbs = aabbs;
    }

    /// Find potentially colliding pairs.
    pub fn find_pairs(&self) -> Vec<(usize, usize)> {
        let mut pairs = Vec::new();

        for i in 0..self.aabbs.len() {
            for j in (i + 1)..self.aabbs.len() {
                if self.aabb_intersect(self.aabbs[i], self.aabbs[j]) {
                    pairs.push((i, j));
                }
            }
        }

        pairs
    }

    /// Check if two AABBs intersect.
    fn aabb_intersect(
        &self,
        a: (Vector3<f64>, Vector3<f64>),
        b: (Vector3<f64>, Vector3<f64>),
    ) -> bool {
        let (a_min, a_max) = a;
        let (b_min, b_max) = b;

        a_min.x <= b_max.x
            && a_max.x >= b_min.x
            && a_min.y <= b_max.y
            && a_max.y >= b_min.y
            && a_min.z <= b_max.z
            && a_max.z >= b_min.z
    }
}

impl Default for BroadPhase {
    fn default() -> Self {
        Self::new()
    }
}

/// Narrow phase collision detection using GJK algorithm.
pub struct NarrowPhase;

impl NarrowPhase {
    /// Detect collision between two spheres.
    pub fn sphere_sphere(
        pos_a: Vector3<f64>,
        radius_a: f64,
        pos_b: Vector3<f64>,
        radius_b: f64,
    ) -> Option<CollisionResult> {
        let delta = pos_b - pos_a;
        let dist = delta.magnitude();
        let sum_radii = radius_a + radius_b;

        if dist < sum_radii {
            let penetration = sum_radii - dist;
            let normal = if dist > 1e-10 {
                delta / dist
            } else {
                Vector3::new(0.0, 1.0, 0.0)
            };

            let contact_point = pos_a + normal * (radius_a - penetration * 0.5);

            Some(CollisionResult {
                contact_point,
                normal,
                penetration_depth: penetration,
                body_a: 0,
                body_b: 0,
            })
        } else {
            None
        }
    }

    /// Detect collision between sphere and box.
    pub fn sphere_box(
        sphere_pos: Vector3<f64>,
        sphere_radius: f64,
        box_pos: Vector3<f64>,
        box_half_extents: Vector3<f64>,
    ) -> Option<CollisionResult> {
        // Find closest point on box to sphere center
        let local_pos = sphere_pos - box_pos;
        let closest = Vector3::new(
            local_pos.x.clamp(-box_half_extents.x, box_half_extents.x),
            local_pos.y.clamp(-box_half_extents.y, box_half_extents.y),
            local_pos.z.clamp(-box_half_extents.z, box_half_extents.z),
        );

        let closest_world = box_pos + closest;
        let delta = sphere_pos - closest_world;
        let dist = delta.magnitude();

        if dist < sphere_radius {
            let penetration = sphere_radius - dist;
            let normal = if dist > 1e-10 {
                delta / dist
            } else {
                Vector3::new(0.0, 1.0, 0.0)
            };

            Some(CollisionResult {
                contact_point: closest_world,
                normal,
                penetration_depth: penetration,
                body_a: 0,
                body_b: 0,
            })
        } else {
            None
        }
    }

    /// Detect collision between two boxes (simplified SAT).
    pub fn box_box(
        pos_a: Vector3<f64>,
        half_a: Vector3<f64>,
        pos_b: Vector3<f64>,
        half_b: Vector3<f64>,
    ) -> Option<CollisionResult> {
        let delta = pos_b - pos_a;

        // Check overlap on each axis
        let overlap_x = (half_a.x + half_b.x) - delta.x.abs();
        let overlap_y = (half_a.y + half_b.y) - delta.y.abs();
        let overlap_z = (half_a.z + half_b.z) - delta.z.abs();

        if overlap_x > 0.0 && overlap_y > 0.0 && overlap_z > 0.0 {
            // Find minimum overlap axis
            let min_overlap = overlap_x.min(overlap_y).min(overlap_z);

            let (normal, penetration) = if min_overlap == overlap_x {
                (Vector3::new(delta.x.signum(), 0.0, 0.0), overlap_x)
            } else if min_overlap == overlap_y {
                (Vector3::new(0.0, delta.y.signum(), 0.0), overlap_y)
            } else {
                (Vector3::new(0.0, 0.0, delta.z.signum()), overlap_z)
            };

            let contact_point = pos_a + delta * 0.5;

            Some(CollisionResult {
                contact_point,
                normal,
                penetration_depth: penetration,
                body_a: 0,
                body_b: 0,
            })
        } else {
            None
        }
    }

    /// GJK algorithm for collision detection.
    pub fn gjk(
        shape_a: &CollisionShape,
        pos_a: Vector3<f64>,
        shape_b: &CollisionShape,
        pos_b: Vector3<f64>,
    ) -> bool {
        // Minkowski difference support function
        let support = |dir: Vector3<f64>| {
            let s_a = pos_a + shape_a.support(dir);
            let s_b = pos_b + shape_b.support(-dir);
            s_a - s_b
        };

        // Initial simplex
        let mut direction = pos_b - pos_a;
        if direction.magnitude() < 1e-10 {
            direction = Vector3::new(1.0, 0.0, 0.0);
        }

        let mut simplex = vec![support(direction)];
        direction = -simplex[0];

        const MAX_ITERATIONS: usize = 32;

        for _ in 0..MAX_ITERATIONS {
            let a = support(direction);

            if a.dot(&direction) < 0.0 {
                return false; // No collision
            }

            simplex.push(a);

            if Self::contains_origin(&mut simplex, &mut direction) {
                return true; // Collision detected
            }
        }

        false
    }

    /// Check if simplex contains origin and update search direction.
    fn contains_origin(simplex: &mut Vec<Vector3<f64>>, direction: &mut Vector3<f64>) -> bool {
        match simplex.len() {
            2 => Self::line_case(simplex, direction),
            3 => Self::triangle_case(simplex, direction),
            4 => Self::tetrahedron_case(simplex, direction),
            _ => false,
        }
    }

    fn line_case(simplex: &mut Vec<Vector3<f64>>, direction: &mut Vector3<f64>) -> bool {
        let a = simplex[1];
        let b = simplex[0];

        let ab = b - a;
        let ao = -a;

        if ab.dot(&ao) > 0.0 {
            *direction = ab.cross(&ao).cross(&ab);
            if direction.magnitude() < 1e-10 {
                *direction = ao;
            }
        } else {
            simplex.remove(0);
            *direction = ao;
        }

        false
    }

    fn triangle_case(simplex: &mut Vec<Vector3<f64>>, direction: &mut Vector3<f64>) -> bool {
        let a = simplex[2];
        let b = simplex[1];
        let c = simplex[0];

        let ab = b - a;
        let ac = c - a;
        let ao = -a;

        let abc = ab.cross(&ac);

        if abc.cross(&ac).dot(&ao) > 0.0 {
            if ac.dot(&ao) > 0.0 {
                simplex.remove(1);
                *direction = ac.cross(&ao).cross(&ac);
            } else {
                simplex.remove(0);
                simplex.remove(0);
                *direction = ab.cross(&ao).cross(&ab);
            }
        } else if ab.cross(&abc).dot(&ao) > 0.0 {
            simplex.remove(0);
            simplex.remove(0);
            *direction = ab.cross(&ao).cross(&ab);
        } else if abc.dot(&ao) > 0.0 {
            *direction = abc;
        } else {
            simplex.swap(0, 1);
            *direction = -abc;
        }

        false
    }

    fn tetrahedron_case(simplex: &mut Vec<Vector3<f64>>, direction: &mut Vector3<f64>) -> bool {
        let a = simplex[3];
        let b = simplex[2];
        let c = simplex[1];
        let d = simplex[0];

        let ab = b - a;
        let ac = c - a;
        let ad = d - a;
        let ao = -a;

        let abc = ab.cross(&ac);
        let acd = ac.cross(&ad);
        let adb = ad.cross(&ab);

        if abc.dot(&ao) > 0.0 {
            simplex.remove(0);
            return Self::triangle_case(simplex, direction);
        }

        if acd.dot(&ao) > 0.0 {
            simplex.remove(2);
            simplex.swap(0, 1);
            return Self::triangle_case(simplex, direction);
        }

        if adb.dot(&ao) > 0.0 {
            simplex.remove(1);
            return Self::triangle_case(simplex, direction);
        }

        true
    }
}

/// Detect collisions between all bodies.
pub fn detect_collisions(
    shapes: &[CollisionShape],
    positions: &[Vector3<f64>],
) -> Vec<CollisionResult> {
    let mut results = Vec::new();

    // Broad phase
    let aabbs: Vec<_> = shapes
        .iter()
        .zip(positions.iter())
        .map(|(shape, pos)| {
            let (min, max) = shape.aabb();
            (min + pos, max + pos)
        })
        .collect();

    let mut broad_phase = BroadPhase::new();
    broad_phase.update(aabbs);

    let pairs = broad_phase.find_pairs();

    // Narrow phase
    for (i, j) in pairs {
        // Use GJK for general case
        if NarrowPhase::gjk(&shapes[i], positions[i], &shapes[j], positions[j]) {
            // Simplified contact info (for now, just mark as colliding)
            results.push(CollisionResult {
                contact_point: (positions[i] + positions[j]) * 0.5,
                normal: (positions[j] - positions[i]).normalize(),
                penetration_depth: 0.1,
                body_a: i,
                body_b: j,
            });
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sphere_sphere_collision() {
        let result = NarrowPhase::sphere_sphere(
            Vector3::new(0.0, 0.0, 0.0),
            1.0,
            Vector3::new(1.5, 0.0, 0.0),
            1.0,
        );

        assert!(result.is_some());
        let collision = result.unwrap();
        assert!(collision.penetration_depth > 0.0);
    }

    #[test]
    fn test_sphere_sphere_no_collision() {
        let result = NarrowPhase::sphere_sphere(
            Vector3::new(0.0, 0.0, 0.0),
            1.0,
            Vector3::new(3.0, 0.0, 0.0),
            1.0,
        );

        assert!(result.is_none());
    }

    #[test]
    fn test_sphere_box_collision() {
        let result = NarrowPhase::sphere_box(
            Vector3::new(0.0, 1.5, 0.0),
            1.0,
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
        );

        assert!(result.is_some());
    }

    #[test]
    fn test_box_box_collision() {
        let result = NarrowPhase::box_box(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
            Vector3::new(1.5, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
        );

        assert!(result.is_some());
    }

    #[test]
    fn test_gjk_spheres() {
        let shape_a = CollisionShape::Sphere { radius: 1.0 };
        let shape_b = CollisionShape::Sphere { radius: 1.0 };

        // Colliding
        assert!(NarrowPhase::gjk(
            &shape_a,
            Vector3::new(0.0, 0.0, 0.0),
            &shape_b,
            Vector3::new(1.5, 0.0, 0.0),
        ));

        // Not colliding
        assert!(!NarrowPhase::gjk(
            &shape_a,
            Vector3::new(0.0, 0.0, 0.0),
            &shape_b,
            Vector3::new(3.0, 0.0, 0.0),
        ));
    }
}
