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

//! Spatial data structures for efficient queries.
//!
//! Provides octree, BVH, and KD-tree implementations for
//! efficient spatial queries in 3D space.

use nalgebra::{Point3, Vector3};

/// Axis-aligned bounding box.
#[derive(Debug, Clone, Copy)]
pub struct AABB {
    /// Minimum corner.
    pub min: Point3<f64>,
    /// Maximum corner.
    pub max: Point3<f64>,
}

impl Default for AABB {
    fn default() -> Self {
        Self::empty()
    }
}

impl AABB {
    /// Create new AABB from min and max corners.
    #[must_use]
    pub fn new(min: Point3<f64>, max: Point3<f64>) -> Self {
        Self { min, max }
    }

    /// Create empty AABB.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            min: Point3::new(f64::MAX, f64::MAX, f64::MAX),
            max: Point3::new(f64::MIN, f64::MIN, f64::MIN),
        }
    }

    /// Create AABB from center and half extents.
    #[must_use]
    pub fn from_center_half_extents(center: Point3<f64>, half_extents: Vector3<f64>) -> Self {
        Self {
            min: center - half_extents,
            max: center + half_extents,
        }
    }

    /// Get center of AABB.
    #[must_use]
    pub fn center(&self) -> Point3<f64> {
        Point3::new(
            (self.min.x + self.max.x) * 0.5,
            (self.min.y + self.max.y) * 0.5,
            (self.min.z + self.max.z) * 0.5,
        )
    }

    /// Get half extents of AABB.
    #[must_use]
    pub fn half_extents(&self) -> Vector3<f64> {
        Vector3::new(
            (self.max.x - self.min.x) * 0.5,
            (self.max.y - self.min.y) * 0.5,
            (self.max.z - self.min.z) * 0.5,
        )
    }

    /// Get size of AABB.
    #[must_use]
    pub fn size(&self) -> Vector3<f64> {
        Vector3::new(
            self.max.x - self.min.x,
            self.max.y - self.min.y,
            self.max.z - self.min.z,
        )
    }

    /// Get surface area.
    #[must_use]
    pub fn surface_area(&self) -> f64 {
        let s = self.size();
        2.0 * (s.x * s.y + s.y * s.z + s.z * s.x)
    }

    /// Get volume.
    #[must_use]
    pub fn volume(&self) -> f64 {
        let s = self.size();
        s.x * s.y * s.z
    }

    /// Expand AABB to include a point.
    pub fn expand_point(&mut self, point: Point3<f64>) {
        self.min.x = self.min.x.min(point.x);
        self.min.y = self.min.y.min(point.y);
        self.min.z = self.min.z.min(point.z);
        self.max.x = self.max.x.max(point.x);
        self.max.y = self.max.y.max(point.y);
        self.max.z = self.max.z.max(point.z);
    }

    /// Expand AABB to include another AABB.
    pub fn expand_aabb(&mut self, other: &AABB) {
        self.min.x = self.min.x.min(other.min.x);
        self.min.y = self.min.y.min(other.min.y);
        self.min.z = self.min.z.min(other.min.z);
        self.max.x = self.max.x.max(other.max.x);
        self.max.y = self.max.y.max(other.max.y);
        self.max.z = self.max.z.max(other.max.z);
    }

    /// Union of two AABBs.
    #[must_use]
    pub fn union(&self, other: &AABB) -> AABB {
        AABB {
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

    /// Check if point is inside AABB.
    #[must_use]
    pub fn contains_point(&self, point: Point3<f64>) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
            && point.z >= self.min.z
            && point.z <= self.max.z
    }

    /// Check if AABB contains another AABB.
    #[must_use]
    pub fn contains_aabb(&self, other: &AABB) -> bool {
        self.min.x <= other.min.x
            && self.max.x >= other.max.x
            && self.min.y <= other.min.y
            && self.max.y >= other.max.y
            && self.min.z <= other.min.z
            && self.max.z >= other.max.z
    }

    /// Check if two AABBs intersect.
    #[must_use]
    pub fn intersects(&self, other: &AABB) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
            && self.min.z <= other.max.z
            && self.max.z >= other.min.z
    }

    /// Ray-AABB intersection.
    /// Returns (t_enter, t_exit) or None if no intersection.
    #[must_use]
    pub fn ray_intersection(&self, origin: &Point3<f64>, dir: &Vector3<f64>) -> Option<(f64, f64)> {
        let mut t_min = f64::NEG_INFINITY;
        let mut t_max = f64::INFINITY;

        for i in 0..3 {
            if dir[i].abs() < f64::EPSILON {
                if origin[i] < self.min[i] || origin[i] > self.max[i] {
                    return None;
                }
            } else {
                let inv_d = 1.0 / dir[i];
                let mut t0 = (self.min[i] - origin[i]) * inv_d;
                let mut t1 = (self.max[i] - origin[i]) * inv_d;

                if inv_d < 0.0 {
                    std::mem::swap(&mut t0, &mut t1);
                }

                t_min = t_min.max(t0);
                t_max = t_max.min(t1);

                if t_max < t_min {
                    return None;
                }
            }
        }

        Some((t_min, t_max))
    }

    /// Get longest axis (0=X, 1=Y, 2=Z).
    #[must_use]
    pub fn longest_axis(&self) -> usize {
        let s = self.size();
        if s.x >= s.y && s.x >= s.z {
            0
        } else if s.y >= s.z {
            1
        } else {
            2
        }
    }
}

/// Octree node.
#[derive(Debug)]
pub struct OctreeNode<T> {
    /// Bounding box of this node.
    pub bounds: AABB,
    /// Items stored in this node (leaf) or empty (internal).
    pub items: Vec<(Point3<f64>, T)>,
    /// Child nodes (8 children for internal nodes).
    pub children: Option<Box<[OctreeNode<T>; 8]>>,
}

/// Octree for spatial partitioning.
#[derive(Debug)]
pub struct Octree<T> {
    root: OctreeNode<T>,
    max_depth: usize,
    max_items_per_node: usize,
}

impl<T: Clone> Octree<T> {
    /// Create new octree with given bounds.
    #[must_use]
    pub fn new(bounds: AABB, max_depth: usize, max_items_per_node: usize) -> Self {
        Self {
            root: OctreeNode {
                bounds,
                items: Vec::new(),
                children: None,
            },
            max_depth,
            max_items_per_node,
        }
    }

    /// Insert item into octree.
    pub fn insert(&mut self, point: Point3<f64>, item: T) {
        Self::insert_into_node(
            &mut self.root,
            point,
            item,
            0,
            self.max_depth,
            self.max_items_per_node,
        );
    }

    fn insert_into_node(
        node: &mut OctreeNode<T>,
        point: Point3<f64>,
        item: T,
        depth: usize,
        max_depth: usize,
        max_items: usize,
    ) {
        if !node.bounds.contains_point(point) {
            return;
        }

        if node.children.is_some() {
            let idx = Self::get_child_index(&node.bounds, point);
            if let Some(ref mut children) = node.children {
                Self::insert_into_node(
                    &mut children[idx],
                    point,
                    item,
                    depth + 1,
                    max_depth,
                    max_items,
                );
            }
        } else if depth < max_depth && node.items.len() >= max_items {
            Self::subdivide(node);
            let idx = Self::get_child_index(&node.bounds, point);
            if let Some(ref mut children) = node.children {
                Self::insert_into_node(
                    &mut children[idx],
                    point,
                    item,
                    depth + 1,
                    max_depth,
                    max_items,
                );
            }
        } else {
            node.items.push((point, item));
        }
    }

    fn subdivide(node: &mut OctreeNode<T>) {
        let center = node.bounds.center();
        let half = node.bounds.half_extents() * 0.5;

        let children: [OctreeNode<T>; 8] = std::array::from_fn(|i| {
            let offset = Vector3::new(
                if i & 1 != 0 { half.x } else { -half.x },
                if i & 2 != 0 { half.y } else { -half.y },
                if i & 4 != 0 { half.z } else { -half.z },
            );
            let child_center = center + offset;
            OctreeNode {
                bounds: AABB::from_center_half_extents(child_center, half),
                items: Vec::new(),
                children: None,
            }
        });

        node.children = Some(Box::new(children));

        let items = std::mem::take(&mut node.items);
        for (point, item) in items {
            let idx = Self::get_child_index(&node.bounds, point);
            if let Some(ref mut children) = node.children {
                children[idx].items.push((point, item));
            }
        }
    }

    fn get_child_index(bounds: &AABB, point: Point3<f64>) -> usize {
        let center = bounds.center();
        let mut idx = 0;
        if point.x >= center.x {
            idx |= 1;
        }
        if point.y >= center.y {
            idx |= 2;
        }
        if point.z >= center.z {
            idx |= 4;
        }
        idx
    }

    /// Query all items within radius of point.
    pub fn query_radius(&self, center: Point3<f64>, radius: f64) -> Vec<&T> {
        let mut results = Vec::new();
        let query_bounds =
            AABB::from_center_half_extents(center, Vector3::new(radius, radius, radius));
        Self::query_node(
            &self.root,
            center,
            radius * radius,
            &query_bounds,
            &mut results,
        );
        results
    }

    fn query_node<'a>(
        node: &'a OctreeNode<T>,
        center: Point3<f64>,
        radius_sq: f64,
        query_bounds: &AABB,
        results: &mut Vec<&'a T>,
    ) {
        if !node.bounds.intersects(query_bounds) {
            return;
        }

        for (point, item) in &node.items {
            let dist_sq = (point - center).magnitude_squared();
            if dist_sq <= radius_sq {
                results.push(item);
            }
        }

        if let Some(ref children) = node.children {
            for child in children.iter() {
                Self::query_node(child, center, radius_sq, query_bounds, results);
            }
        }
    }

    /// Query all items within an AABB.
    pub fn query_aabb(&self, bounds: &AABB) -> Vec<&T> {
        let mut results = Vec::new();
        Self::query_aabb_node(&self.root, bounds, &mut results);
        results
    }

    fn query_aabb_node<'a>(node: &'a OctreeNode<T>, bounds: &AABB, results: &mut Vec<&'a T>) {
        if !node.bounds.intersects(bounds) {
            return;
        }

        for (point, item) in &node.items {
            if bounds.contains_point(*point) {
                results.push(item);
            }
        }

        if let Some(ref children) = node.children {
            for child in children.iter() {
                Self::query_aabb_node(child, bounds, results);
            }
        }
    }
}

/// BVH (Bounding Volume Hierarchy) node.
#[derive(Debug, Clone)]
pub struct BVHNode {
    /// Bounding box of this node.
    pub bounds: AABB,
    /// Item index for leaf nodes.
    pub item_idx: Option<usize>,
    /// Left child index.
    pub left: Option<usize>,
    /// Right child index.
    pub right: Option<usize>,
}

/// BVH for efficient ray casting.
#[derive(Debug)]
pub struct BVH<T> {
    nodes: Vec<BVHNode>,
    items: Vec<T>,
}

impl<T> BVH<T> {
    /// Build BVH from items with bounding boxes.
    pub fn build(items: Vec<(AABB, T)>) -> Self {
        if items.is_empty() {
            return Self {
                nodes: Vec::new(),
                items: Vec::new(),
            };
        }

        let mut nodes = Vec::with_capacity(items.len() * 2);
        let mut bvh_items = Vec::with_capacity(items.len());

        let indices: Vec<usize> = (0..items.len()).collect();
        let bounds_list: Vec<AABB> = items.iter().map(|(b, _)| *b).collect();

        for (_, item) in items {
            bvh_items.push(item);
        }

        Self::build_recursive(&mut nodes, &bounds_list, indices);

        Self {
            nodes,
            items: bvh_items,
        }
    }

    fn build_recursive(
        nodes: &mut Vec<BVHNode>,
        bounds_list: &[AABB],
        indices: Vec<usize>,
    ) -> usize {
        let node_idx = nodes.len();

        if indices.len() == 1 {
            let idx = indices[0];
            nodes.push(BVHNode {
                bounds: bounds_list[idx],
                item_idx: Some(idx),
                left: None,
                right: None,
            });
            return node_idx;
        }

        let mut bounds = AABB::empty();
        for &idx in &indices {
            bounds.expand_aabb(&bounds_list[idx]);
        }

        let axis = bounds.longest_axis();

        let mut sorted_indices = indices;
        sorted_indices.sort_by(|&a, &b| {
            let ca = bounds_list[a].center();
            let cb = bounds_list[b].center();
            ca[axis].partial_cmp(&cb[axis]).unwrap()
        });

        let mid = sorted_indices.len() / 2;
        let left_indices: Vec<usize> = sorted_indices[..mid].to_vec();
        let right_indices: Vec<usize> = sorted_indices[mid..].to_vec();

        nodes.push(BVHNode {
            bounds,
            item_idx: None,
            left: None,
            right: None,
        });

        let left_idx = Self::build_recursive(nodes, bounds_list, left_indices);
        let right_idx = Self::build_recursive(nodes, bounds_list, right_indices);

        nodes[node_idx].left = Some(left_idx);
        nodes[node_idx].right = Some(right_idx);

        node_idx
    }

    /// Query BVH with a ray.
    pub fn ray_query(&self, origin: &Point3<f64>, dir: &Vector3<f64>) -> Vec<usize> {
        let mut results = Vec::new();
        if !self.nodes.is_empty() {
            self.ray_query_node(0, origin, dir, &mut results);
        }
        results
    }

    fn ray_query_node(
        &self,
        node_idx: usize,
        origin: &Point3<f64>,
        dir: &Vector3<f64>,
        results: &mut Vec<usize>,
    ) {
        let node = &self.nodes[node_idx];

        if node.bounds.ray_intersection(origin, dir).is_none() {
            return;
        }

        if let Some(item_idx) = node.item_idx {
            results.push(item_idx);
        }

        if let Some(left) = node.left {
            self.ray_query_node(left, origin, dir, results);
        }
        if let Some(right) = node.right {
            self.ray_query_node(right, origin, dir, results);
        }
    }

    /// Query BVH with AABB.
    pub fn aabb_query(&self, bounds: &AABB) -> Vec<usize> {
        let mut results = Vec::new();
        if !self.nodes.is_empty() {
            self.aabb_query_node(0, bounds, &mut results);
        }
        results
    }

    fn aabb_query_node(&self, node_idx: usize, bounds: &AABB, results: &mut Vec<usize>) {
        let node = &self.nodes[node_idx];

        if !node.bounds.intersects(bounds) {
            return;
        }

        if let Some(item_idx) = node.item_idx {
            results.push(item_idx);
        }

        if let Some(left) = node.left {
            self.aabb_query_node(left, bounds, results);
        }
        if let Some(right) = node.right {
            self.aabb_query_node(right, bounds, results);
        }
    }

    /// Get item at index.
    #[must_use]
    pub fn get(&self, idx: usize) -> Option<&T> {
        self.items.get(idx)
    }
}

/// KD-tree node.
#[derive(Debug)]
struct KDNode {
    point_idx: usize,
    left: Option<Box<KDNode>>,
    right: Option<Box<KDNode>>,
    split_axis: usize,
}

/// KD-tree for nearest neighbor queries.
#[derive(Debug)]
pub struct KDTree {
    root: Option<Box<KDNode>>,
    points: Vec<Point3<f64>>,
}

impl KDTree {
    /// Build KD-tree from points.
    pub fn build(points: Vec<Point3<f64>>) -> Self {
        if points.is_empty() {
            return Self { root: None, points };
        }

        let indices: Vec<usize> = (0..points.len()).collect();
        let root = Self::build_recursive(&points, indices, 0);

        Self { root, points }
    }

    fn build_recursive(
        points: &[Point3<f64>],
        mut indices: Vec<usize>,
        depth: usize,
    ) -> Option<Box<KDNode>> {
        if indices.is_empty() {
            return None;
        }

        let axis = depth % 3;

        indices.sort_by(|&a, &b| points[a][axis].partial_cmp(&points[b][axis]).unwrap());

        let mid = indices.len() / 2;
        let point_idx = indices[mid];

        let left_indices: Vec<usize> = indices[..mid].to_vec();
        let right_indices: Vec<usize> = if mid + 1 < indices.len() {
            indices[mid + 1..].to_vec()
        } else {
            Vec::new()
        };

        Some(Box::new(KDNode {
            point_idx,
            left: Self::build_recursive(points, left_indices, depth + 1),
            right: Self::build_recursive(points, right_indices, depth + 1),
            split_axis: axis,
        }))
    }

    /// Find nearest neighbor to query point.
    #[must_use]
    pub fn nearest(&self, query: &Point3<f64>) -> Option<(usize, f64)> {
        self.root.as_ref().map(|root| {
            let mut best_idx = root.point_idx;
            let mut best_dist = (self.points[best_idx] - query).magnitude_squared();
            Self::nearest_recursive(root, &self.points, query, &mut best_idx, &mut best_dist);
            (best_idx, best_dist.sqrt())
        })
    }

    fn nearest_recursive(
        node: &KDNode,
        points: &[Point3<f64>],
        query: &Point3<f64>,
        best_idx: &mut usize,
        best_dist: &mut f64,
    ) {
        let dist = (points[node.point_idx] - query).magnitude_squared();
        if dist < *best_dist {
            *best_dist = dist;
            *best_idx = node.point_idx;
        }

        let axis = node.split_axis;
        let diff = query[axis] - points[node.point_idx][axis];

        let (first, second) = if diff < 0.0 {
            (&node.left, &node.right)
        } else {
            (&node.right, &node.left)
        };

        if let Some(child) = first {
            Self::nearest_recursive(child, points, query, best_idx, best_dist);
        }

        if diff * diff < *best_dist {
            if let Some(child) = second {
                Self::nearest_recursive(child, points, query, best_idx, best_dist);
            }
        }
    }

    /// Find k nearest neighbors.
    pub fn k_nearest(&self, query: &Point3<f64>, k: usize) -> Vec<(usize, f64)> {
        let mut results: Vec<(usize, f64)> = Vec::with_capacity(k);

        if let Some(ref root) = self.root {
            Self::k_nearest_recursive(root, &self.points, query, k, &mut results);
        }

        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        results.into_iter().take(k).collect()
    }

    fn k_nearest_recursive(
        node: &KDNode,
        points: &[Point3<f64>],
        query: &Point3<f64>,
        k: usize,
        results: &mut Vec<(usize, f64)>,
    ) {
        let dist = (points[node.point_idx] - query).magnitude();

        if results.len() < k {
            results.push((node.point_idx, dist));
            results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        } else if dist < results[0].1 {
            results[0] = (node.point_idx, dist);
            results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        }

        let axis = node.split_axis;
        let diff = query[axis] - points[node.point_idx][axis];

        let (first, second) = if diff < 0.0 {
            (&node.left, &node.right)
        } else {
            (&node.right, &node.left)
        };

        if let Some(child) = first {
            Self::k_nearest_recursive(child, points, query, k, results);
        }

        let max_dist = if results.len() < k {
            f64::MAX
        } else {
            results[0].1
        };
        if diff.abs() < max_dist {
            if let Some(child) = second {
                Self::k_nearest_recursive(child, points, query, k, results);
            }
        }
    }

    /// Find all points within radius.
    pub fn radius_search(&self, query: &Point3<f64>, radius: f64) -> Vec<(usize, f64)> {
        let mut results = Vec::new();
        if let Some(ref root) = self.root {
            Self::radius_search_recursive(root, &self.points, query, radius * radius, &mut results);
        }
        results.into_iter().map(|(i, d)| (i, d.sqrt())).collect()
    }

    fn radius_search_recursive(
        node: &KDNode,
        points: &[Point3<f64>],
        query: &Point3<f64>,
        radius_sq: f64,
        results: &mut Vec<(usize, f64)>,
    ) {
        let dist_sq = (points[node.point_idx] - query).magnitude_squared();
        if dist_sq <= radius_sq {
            results.push((node.point_idx, dist_sq));
        }

        let axis = node.split_axis;
        let diff = query[axis] - points[node.point_idx][axis];

        let (first, second) = if diff < 0.0 {
            (&node.left, &node.right)
        } else {
            (&node.right, &node.left)
        };

        if let Some(child) = first {
            Self::radius_search_recursive(child, points, query, radius_sq, results);
        }

        if diff * diff <= radius_sq {
            if let Some(child) = second {
                Self::radius_search_recursive(child, points, query, radius_sq, results);
            }
        }
    }

    /// Get point at index.
    #[must_use]
    pub fn get_point(&self, idx: usize) -> Option<&Point3<f64>> {
        self.points.get(idx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aabb_contains() {
        let aabb = AABB::new(Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 1.0, 1.0));

        assert!(aabb.contains_point(Point3::new(0.5, 0.5, 0.5)));
        assert!(!aabb.contains_point(Point3::new(2.0, 0.5, 0.5)));
    }

    #[test]
    fn test_aabb_intersects() {
        let a = AABB::new(Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 1.0, 1.0));
        let b = AABB::new(Point3::new(0.5, 0.5, 0.5), Point3::new(1.5, 1.5, 1.5));
        let c = AABB::new(Point3::new(2.0, 2.0, 2.0), Point3::new(3.0, 3.0, 3.0));

        assert!(a.intersects(&b));
        assert!(!a.intersects(&c));
    }

    #[test]
    fn test_kdtree_nearest() {
        let points = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
        ];

        let tree = KDTree::build(points);

        let (idx, dist) = tree.nearest(&Point3::new(0.1, 0.1, 0.0)).unwrap();
        assert_eq!(idx, 0);
        assert!(dist < 0.2);
    }

    #[test]
    fn test_kdtree_radius() {
        let points = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
            Point3::new(3.0, 0.0, 0.0),
        ];

        let tree = KDTree::build(points);

        let results = tree.radius_search(&Point3::new(1.5, 0.0, 0.0), 1.0);
        assert_eq!(results.len(), 2);
    }
}
