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

//! Bounding Volume Hierarchy for accelerated ray tracing.
//!
//! Implements a BVH with SAH (Surface Area Heuristic) construction.

use super::ray::{Aabb, Ray};
use nalgebra::Point3;

/// A primitive that can be stored in the BVH.
pub trait BvhPrimitive {
    /// Get the bounding box of this primitive.
    fn bounds(&self) -> Aabb;

    /// Get the centroid of this primitive.
    fn centroid(&self) -> Point3<f64>;

    /// Intersect ray with primitive. Returns hit distance if intersection exists.
    fn intersect(&self, ray: &Ray) -> Option<f64>;
}

/// A triangle primitive for the BVH.
#[derive(Debug, Clone)]
pub struct Triangle {
    /// Vertex positions.
    /// First vertex.
    pub v0: Point3<f64>,
    /// Second vertex.
    pub v1: Point3<f64>,
    /// Third vertex.
    pub v2: Point3<f64>,
    /// Triangle index (for identifying which triangle was hit).
    pub index: usize,
}

impl Triangle {
    /// Create a new triangle.
    pub fn new(v0: Point3<f64>, v1: Point3<f64>, v2: Point3<f64>, index: usize) -> Self {
        Self { v0, v1, v2, index }
    }
}

impl BvhPrimitive for Triangle {
    fn bounds(&self) -> Aabb {
        let mut aabb = Aabb::from_point(self.v0);
        aabb.include_point(self.v1);
        aabb.include_point(self.v2);
        aabb
    }

    fn centroid(&self) -> Point3<f64> {
        Point3::new(
            (self.v0.x + self.v1.x + self.v2.x) / 3.0,
            (self.v0.y + self.v1.y + self.v2.y) / 3.0,
            (self.v0.z + self.v1.z + self.v2.z) / 3.0,
        )
    }

    fn intersect(&self, ray: &Ray) -> Option<f64> {
        // Möller–Trumbore intersection
        let edge1 = self.v1 - self.v0;
        let edge2 = self.v2 - self.v0;
        let h = ray.direction.cross(&edge2);
        let a = edge1.dot(&h);

        if a.abs() < 1e-10 {
            return None;
        }

        let f = 1.0 / a;
        let s = ray.origin - self.v0;
        let u = f * s.dot(&h);

        if !(0.0..=1.0).contains(&u) {
            return None;
        }

        let q = s.cross(&edge1);
        let v = f * ray.direction.dot(&q);

        if v < 0.0 || u + v > 1.0 {
            return None;
        }

        let t = f * edge2.dot(&q);

        if t >= ray.t_min && t <= ray.t_max {
            Some(t)
        } else {
            None
        }
    }
}

/// BVH node.
#[derive(Debug, Clone)]
pub enum BvhNode {
    /// Leaf node containing primitive indices.
    Leaf {
        /// Bounding box of the leaf.
        bounds: Aabb,
        /// Index of the first primitive in the leaf.
        first_prim: usize,
        /// Number of primitives in the leaf.
        prim_count: usize,
    },
    /// Interior node with two children.
    Interior {
        /// Bounding box of the interior node.
        bounds: Aabb,
        /// Left child node.
        left: Box<BvhNode>,
        /// Right child node.
        right: Box<BvhNode>,
        /// Axis used for splitting.
        split_axis: usize,
    },
}

impl BvhNode {
    /// Get the bounds of this node.
    pub fn bounds(&self) -> &Aabb {
        match self {
            BvhNode::Leaf { bounds, .. } => bounds,
            BvhNode::Interior { bounds, .. } => bounds,
        }
    }
}

/// BVH construction parameters.
#[derive(Debug, Clone)]
pub struct BvhParams {
    /// Maximum primitives per leaf.
    pub max_prims_per_leaf: usize,
    /// Maximum tree depth.
    pub max_depth: usize,
    /// Cost of ray-primitive intersection relative to traversal.
    pub intersection_cost: f64,
    /// Cost of traversal step.
    pub traversal_cost: f64,
}

impl Default for BvhParams {
    fn default() -> Self {
        Self {
            max_prims_per_leaf: 4,
            max_depth: 32,
            intersection_cost: 1.0,
            traversal_cost: 1.0,
        }
    }
}

/// Bounding Volume Hierarchy.
pub struct Bvh<P: BvhPrimitive> {
    /// Root node.
    pub root: Option<BvhNode>,
    /// Primitives (reordered during construction).
    pub primitives: Vec<P>,
    /// Construction parameters.
    pub params: BvhParams,
}

impl<P: BvhPrimitive> Bvh<P> {
    /// Create a new empty BVH.
    pub fn new(params: BvhParams) -> Self {
        Self {
            root: None,
            primitives: Vec::new(),
            params,
        }
    }

    /// Build BVH from primitives.
    pub fn build(primitives: Vec<P>, params: BvhParams) -> Self {
        if primitives.is_empty() {
            return Self {
                root: None,
                primitives,
                params,
            };
        }

        let mut bvh = Self {
            root: None,
            primitives,
            params,
        };

        let n = bvh.primitives.len();
        let indices: Vec<usize> = (0..n).collect();

        bvh.root = Some(bvh.build_recursive(indices, 0));

        bvh
    }

    /// Recursive BVH construction with SAH.
    fn build_recursive(&self, indices: Vec<usize>, depth: usize) -> BvhNode {
        // Compute bounds
        let mut bounds = Aabb::empty();
        for &i in &indices {
            bounds.include_aabb(&self.primitives[i].bounds());
        }

        let n = indices.len();

        // Create leaf if few primitives or max depth reached
        if n <= self.params.max_prims_per_leaf || depth >= self.params.max_depth {
            return BvhNode::Leaf {
                bounds,
                first_prim: indices[0],
                prim_count: n,
            };
        }

        // Compute centroid bounds
        let mut centroid_bounds = Aabb::empty();
        for &i in &indices {
            centroid_bounds.include_point(self.primitives[i].centroid());
        }

        let split_axis = centroid_bounds.max_axis();

        // If centroids are coincident, create leaf
        let extent = centroid_bounds.extent();
        if extent[split_axis] < 1e-10 {
            return BvhNode::Leaf {
                bounds,
                first_prim: indices[0],
                prim_count: n,
            };
        }

        // SAH binning
        const NUM_BINS: usize = 12;
        let mut bins: Vec<(Aabb, usize)> = vec![(Aabb::empty(), 0); NUM_BINS];

        let bin_scale = NUM_BINS as f64 / extent[split_axis];

        for &i in &indices {
            let centroid = self.primitives[i].centroid();
            let offset = centroid[split_axis] - centroid_bounds.min[split_axis];
            let bin_idx = ((offset * bin_scale) as usize).min(NUM_BINS - 1);
            bins[bin_idx].0.include_aabb(&self.primitives[i].bounds());
            bins[bin_idx].1 += 1;
        }

        // Compute costs for each split
        let mut best_cost = f64::INFINITY;
        let mut best_split = 0;

        for split in 1..NUM_BINS {
            let mut left_bounds = Aabb::empty();
            let mut left_count = 0;
            for bin in &bins[..split] {
                if bin.1 > 0 {
                    left_bounds.include_aabb(&bin.0);
                    left_count += bin.1;
                }
            }

            let mut right_bounds = Aabb::empty();
            let mut right_count = 0;
            for bin in &bins[split..] {
                if bin.1 > 0 {
                    right_bounds.include_aabb(&bin.0);
                    right_count += bin.1;
                }
            }

            if left_count == 0 || right_count == 0 {
                continue;
            }

            let cost = self.params.traversal_cost
                + self.params.intersection_cost
                    * (left_bounds.surface_area() * left_count as f64
                        + right_bounds.surface_area() * right_count as f64)
                    / bounds.surface_area();

            if cost < best_cost {
                best_cost = cost;
                best_split = split;
            }
        }

        // Check if splitting is worth it
        let leaf_cost = self.params.intersection_cost * n as f64;
        if best_cost >= leaf_cost || best_split == 0 {
            return BvhNode::Leaf {
                bounds,
                first_prim: indices[0],
                prim_count: n,
            };
        }

        // Partition primitives
        let split_point = centroid_bounds.min[split_axis]
            + (best_split as f64 / NUM_BINS as f64) * extent[split_axis];

        let (left_indices, right_indices): (Vec<_>, Vec<_>) = indices
            .into_iter()
            .partition(|&i| self.primitives[i].centroid()[split_axis] < split_point);

        // Handle degenerate cases
        if left_indices.is_empty() || right_indices.is_empty() {
            return BvhNode::Leaf {
                bounds,
                first_prim: if !left_indices.is_empty() {
                    left_indices[0]
                } else {
                    right_indices[0]
                },
                prim_count: n,
            };
        }

        // Recursively build children
        let left = self.build_recursive(left_indices, depth + 1);
        let right = self.build_recursive(right_indices, depth + 1);

        BvhNode::Interior {
            bounds,
            left: Box::new(left),
            right: Box::new(right),
            split_axis,
        }
    }

    /// Traverse BVH and find closest intersection.
    pub fn intersect(&self, ray: &Ray) -> Option<(usize, f64)> {
        let root = self.root.as_ref()?;
        self.intersect_recursive(root, ray, f64::INFINITY)
    }

    fn intersect_recursive(
        &self,
        node: &BvhNode,
        ray: &Ray,
        mut closest: f64,
    ) -> Option<(usize, f64)> {
        // Check node bounds
        let (t_enter, _) = node.bounds().intersect(ray)?;

        if t_enter > closest {
            return None;
        }

        match node {
            BvhNode::Leaf {
                first_prim,
                prim_count,
                ..
            } => {
                let mut result = None;

                for i in *first_prim..(*first_prim + *prim_count) {
                    if i < self.primitives.len() {
                        if let Some(t) = self.primitives[i].intersect(ray) {
                            if t < closest {
                                closest = t;
                                result = Some((i, t));
                            }
                        }
                    }
                }

                result
            }
            BvhNode::Interior { left, right, .. } => {
                // Test both children, visiting closer one first
                let left_hit = left.bounds().intersect(ray);
                let right_hit = right.bounds().intersect(ray);

                match (left_hit, right_hit) {
                    (Some((t_left, _)), Some((t_right, _))) => {
                        let (first, second) = if t_left < t_right {
                            (left, right)
                        } else {
                            (right, left)
                        };

                        let mut result = None;

                        if let Some((idx, t)) = self.intersect_recursive(first, ray, closest) {
                            closest = t;
                            result = Some((idx, t));
                        }

                        if let Some((idx, t)) = self.intersect_recursive(second, ray, closest) {
                            result = Some((idx, t));
                        }

                        result
                    }
                    (Some(_), None) => self.intersect_recursive(left, ray, closest),
                    (None, Some(_)) => self.intersect_recursive(right, ray, closest),
                    (None, None) => None,
                }
            }
        }
    }

    /// Check if any intersection exists (for shadow rays).
    pub fn intersect_any(&self, ray: &Ray) -> bool {
        let Some(root) = self.root.as_ref() else {
            return false;
        };
        self.intersect_any_recursive(root, ray)
    }

    fn intersect_any_recursive(&self, node: &BvhNode, ray: &Ray) -> bool {
        if node.bounds().intersect(ray).is_none() {
            return false;
        }

        match node {
            BvhNode::Leaf {
                first_prim,
                prim_count,
                ..
            } => {
                for i in *first_prim..(*first_prim + *prim_count) {
                    if i < self.primitives.len() && self.primitives[i].intersect(ray).is_some() {
                        return true;
                    }
                }
                false
            }
            BvhNode::Interior { left, right, .. } => {
                self.intersect_any_recursive(left, ray) || self.intersect_any_recursive(right, ray)
            }
        }
    }

    /// Get the number of primitives.
    pub fn len(&self) -> usize {
        self.primitives.len()
    }

    /// Check if BVH is empty.
    pub fn is_empty(&self) -> bool {
        self.primitives.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra::Vector3;

    #[test]
    fn test_triangle_intersection() {
        let tri = Triangle::new(
            Point3::new(-1.0, -1.0, 0.0),
            Point3::new(1.0, -1.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
            0,
        );

        // Ray hitting triangle
        let ray = Ray::new(Point3::new(0.0, 0.0, -5.0), Vector3::new(0.0, 0.0, 1.0));

        let t = tri.intersect(&ray);
        assert!(t.is_some());
        assert!((t.unwrap() - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_bvh_construction() {
        let triangles = vec![
            Triangle::new(
                Point3::new(-1.0, -1.0, 0.0),
                Point3::new(1.0, -1.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
                0,
            ),
            Triangle::new(
                Point3::new(2.0, -1.0, 0.0),
                Point3::new(4.0, -1.0, 0.0),
                Point3::new(3.0, 1.0, 0.0),
                1,
            ),
        ];

        let bvh = Bvh::build(triangles, BvhParams::default());
        assert_eq!(bvh.len(), 2);
    }

    #[test]
    fn test_bvh_intersection() {
        let triangles = vec![Triangle::new(
            Point3::new(-1.0, -1.0, 0.0),
            Point3::new(1.0, -1.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
            0,
        )];

        let bvh = Bvh::build(triangles, BvhParams::default());

        let ray = Ray::new(Point3::new(0.0, 0.0, -5.0), Vector3::new(0.0, 0.0, 1.0));

        let hit = bvh.intersect(&ray);
        assert!(hit.is_some());
    }
}
