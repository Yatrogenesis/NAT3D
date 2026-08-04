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

//! Boolean mesh operations.
//!
//! Implements CSG operations: union, intersection, and difference.

use nalgebra::{Point3, Vector3};
use std::collections::HashMap;

/// Boolean operation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BooleanOp {
    /// Union (A + B).
    Union,
    /// Intersection (A & B).
    Intersection,
    /// Difference (A - B).
    Difference,
}

/// A polygon for boolean operations.
#[derive(Debug, Clone)]
pub struct BspPolygon {
    /// Vertex positions.
    pub vertices: Vec<Point3<f64>>,
    /// Polygon normal.
    pub normal: Vector3<f64>,
    /// Material/face ID.
    pub material_id: u32,
}

impl BspPolygon {
    /// Create a new polygon.
    pub fn new(vertices: Vec<Point3<f64>>) -> Self {
        let normal = if vertices.len() >= 3 {
            let v0 = vertices[0];
            let v1 = vertices[1];
            let v2 = vertices[2];
            (v1 - v0).cross(&(v2 - v0)).normalize()
        } else {
            Vector3::new(0.0, 1.0, 0.0)
        };

        Self {
            vertices,
            normal,
            material_id: 0,
        }
    }

    /// Flip the polygon (reverse winding).
    pub fn flip(&mut self) {
        self.vertices.reverse();
        self.normal = -self.normal;
    }

    /// Get a flipped copy.
    pub fn flipped(&self) -> Self {
        let mut poly = self.clone();
        poly.flip();
        poly
    }

    /// Calculate the plane for this polygon.
    pub fn plane(&self) -> Plane {
        Plane::from_normal_and_point(self.normal, self.vertices[0])
    }
}

/// Plane for BSP operations.
#[derive(Debug, Clone, Copy)]
pub struct Plane {
    /// Plane normal.
    pub normal: Vector3<f64>,
    /// Distance from origin.
    pub d: f64,
}

/// Classification of a point relative to a plane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointClassification {
    Front,
    Back,
    Coplanar,
}

/// Classification of a polygon relative to a plane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolygonClassification {
    Coplanar,
    Front,
    Back,
    Spanning,
}

impl Plane {
    /// Epsilon for floating point comparisons.
    const EPSILON: f64 = 1e-5;

    /// Create a plane from normal and point.
    pub fn from_normal_and_point(normal: Vector3<f64>, point: Point3<f64>) -> Self {
        let normal = normal.normalize();
        let d = normal.dot(&point.coords);
        Self { normal, d }
    }

    /// Create a plane from three points.
    pub fn from_points(p0: Point3<f64>, p1: Point3<f64>, p2: Point3<f64>) -> Self {
        let normal = (p1 - p0).cross(&(p2 - p0)).normalize();
        let d = normal.dot(&p0.coords);
        Self { normal, d }
    }

    /// Flip the plane.
    pub fn flip(&mut self) {
        self.normal = -self.normal;
        self.d = -self.d;
    }

    /// Signed distance from point to plane.
    pub fn distance_to_point(&self, point: Point3<f64>) -> f64 {
        self.normal.dot(&point.coords) - self.d
    }

    /// Classify a point relative to the plane.
    pub fn classify_point(&self, point: Point3<f64>) -> PointClassification {
        let dist = self.distance_to_point(point);
        if dist > Self::EPSILON {
            PointClassification::Front
        } else if dist < -Self::EPSILON {
            PointClassification::Back
        } else {
            PointClassification::Coplanar
        }
    }

    /// Classify a polygon relative to the plane.
    pub fn classify_polygon(&self, polygon: &BspPolygon) -> PolygonClassification {
        let mut front_count = 0;
        let mut back_count = 0;

        for &vertex in &polygon.vertices {
            match self.classify_point(vertex) {
                PointClassification::Front => front_count += 1,
                PointClassification::Back => back_count += 1,
                PointClassification::Coplanar => {}
            }
        }

        if front_count > 0 && back_count > 0 {
            PolygonClassification::Spanning
        } else if front_count > 0 {
            PolygonClassification::Front
        } else if back_count > 0 {
            PolygonClassification::Back
        } else {
            PolygonClassification::Coplanar
        }
    }

    /// Split a polygon by the plane.
    pub fn split_polygon(&self, polygon: &BspPolygon) -> SplitResult {
        let classification = self.classify_polygon(polygon);

        match classification {
            PolygonClassification::Coplanar => {
                if self.normal.dot(&polygon.normal) > 0.0 {
                    SplitResult::coplanar_front(polygon.clone())
                } else {
                    SplitResult::coplanar_back(polygon.clone())
                }
            }
            PolygonClassification::Front => SplitResult::front(polygon.clone()),
            PolygonClassification::Back => SplitResult::back(polygon.clone()),
            PolygonClassification::Spanning => self.split_spanning_polygon(polygon),
        }
    }

    /// Split a spanning polygon.
    fn split_spanning_polygon(&self, polygon: &BspPolygon) -> SplitResult {
        let mut front_vertices = Vec::new();
        let mut back_vertices = Vec::new();

        let n = polygon.vertices.len();
        for i in 0..n {
            let vi = polygon.vertices[i];
            let vj = polygon.vertices[(i + 1) % n];

            let ti = self.classify_point(vi);
            let tj = self.classify_point(vj);

            match ti {
                PointClassification::Front => {
                    front_vertices.push(vi);
                }
                PointClassification::Back => {
                    back_vertices.push(vi);
                }
                PointClassification::Coplanar => {
                    front_vertices.push(vi);
                    back_vertices.push(vi);
                }
            }

            // Check if edge crosses plane
            if (ti == PointClassification::Front && tj == PointClassification::Back)
                || (ti == PointClassification::Back && tj == PointClassification::Front)
            {
                // Compute intersection point
                let edge = vj - vi;
                let t = (self.d - self.normal.dot(&vi.coords)) / self.normal.dot(&edge);
                let intersection = vi + edge * t;

                front_vertices.push(intersection);
                back_vertices.push(intersection);
            }
        }

        let mut result = SplitResult::default();

        if front_vertices.len() >= 3 {
            let mut front_poly = BspPolygon::new(front_vertices);
            front_poly.material_id = polygon.material_id;
            result.front.push(front_poly);
        }

        if back_vertices.len() >= 3 {
            let mut back_poly = BspPolygon::new(back_vertices);
            back_poly.material_id = polygon.material_id;
            result.back.push(back_poly);
        }

        result
    }
}

/// Result of splitting polygons.
#[derive(Debug, Clone, Default)]
pub struct SplitResult {
    /// Polygons in front of plane.
    pub front: Vec<BspPolygon>,
    /// Polygons behind plane.
    pub back: Vec<BspPolygon>,
    /// Coplanar polygons facing same direction.
    pub coplanar_front: Vec<BspPolygon>,
    /// Coplanar polygons facing opposite direction.
    pub coplanar_back: Vec<BspPolygon>,
}

impl SplitResult {
    pub fn front(poly: BspPolygon) -> Self {
        Self {
            front: vec![poly],
            ..Default::default()
        }
    }

    pub fn back(poly: BspPolygon) -> Self {
        Self {
            back: vec![poly],
            ..Default::default()
        }
    }

    pub fn coplanar_front(poly: BspPolygon) -> Self {
        Self {
            coplanar_front: vec![poly],
            ..Default::default()
        }
    }

    pub fn coplanar_back(poly: BspPolygon) -> Self {
        Self {
            coplanar_back: vec![poly],
            ..Default::default()
        }
    }
}

/// BSP tree node.
#[derive(Debug, Clone)]
pub struct BspNode {
    /// Dividing plane.
    plane: Option<Plane>,
    /// Front child.
    front: Option<Box<BspNode>>,
    /// Back child.
    back: Option<Box<BspNode>>,
    /// Polygons at this node.
    polygons: Vec<BspPolygon>,
}

impl BspNode {
    /// Create an empty BSP node.
    pub fn new() -> Self {
        Self {
            plane: None,
            front: None,
            back: None,
            polygons: Vec::new(),
        }
    }

    /// Create a BSP tree from polygons.
    pub fn from_polygons(polygons: Vec<BspPolygon>) -> Self {
        let mut node = Self::new();
        node.build(polygons);
        node
    }

    /// Build BSP tree from polygons.
    pub fn build(&mut self, mut polygons: Vec<BspPolygon>) {
        if polygons.is_empty() {
            return;
        }

        // Use first polygon's plane as dividing plane
        if self.plane.is_none() {
            self.plane = Some(polygons[0].plane());
        }

        let plane = self.plane.unwrap();

        let mut front_polys = Vec::new();
        let mut back_polys = Vec::new();

        for poly in polygons.drain(..) {
            let result = plane.split_polygon(&poly);

            self.polygons.extend(result.coplanar_front);
            self.polygons.extend(result.coplanar_back);
            front_polys.extend(result.front);
            back_polys.extend(result.back);
        }

        if !front_polys.is_empty() {
            if self.front.is_none() {
                self.front = Some(Box::new(BspNode::new()));
            }
            self.front.as_mut().unwrap().build(front_polys);
        }

        if !back_polys.is_empty() {
            if self.back.is_none() {
                self.back = Some(Box::new(BspNode::new()));
            }
            self.back.as_mut().unwrap().build(back_polys);
        }
    }

    /// Flip all polygons in the tree.
    pub fn invert(&mut self) {
        for poly in &mut self.polygons {
            poly.flip();
        }

        if let Some(ref mut plane) = self.plane {
            plane.flip();
        }

        std::mem::swap(&mut self.front, &mut self.back);

        if let Some(ref mut front) = self.front {
            front.invert();
        }
        if let Some(ref mut back) = self.back {
            back.invert();
        }
    }

    /// Clip polygons to this BSP tree.
    pub fn clip_polygons(&self, polygons: Vec<BspPolygon>) -> Vec<BspPolygon> {
        let plane = match self.plane {
            Some(p) => p,
            None => return polygons,
        };

        let mut front = Vec::new();
        let mut back = Vec::new();

        for poly in polygons {
            let result = plane.split_polygon(&poly);
            front.extend(result.front);
            front.extend(result.coplanar_front);
            back.extend(result.back);
            back.extend(result.coplanar_back);
        }

        if let Some(ref front_node) = self.front {
            front = front_node.clip_polygons(front);
        }

        if let Some(ref back_node) = self.back {
            back = back_node.clip_polygons(back);
        } else {
            back.clear();
        }

        front.extend(back);
        front
    }

    /// Clip this BSP tree to another.
    pub fn clip_to(&mut self, other: &BspNode) {
        self.polygons = other.clip_polygons(std::mem::take(&mut self.polygons));

        if let Some(ref mut front) = self.front {
            front.clip_to(other);
        }
        if let Some(ref mut back) = self.back {
            back.clip_to(other);
        }
    }

    /// Get all polygons from the tree.
    pub fn all_polygons(&self) -> Vec<BspPolygon> {
        let mut result = self.polygons.clone();

        if let Some(ref front) = self.front {
            result.extend(front.all_polygons());
        }
        if let Some(ref back) = self.back {
            result.extend(back.all_polygons());
        }

        result
    }
}

impl Default for BspNode {
    fn default() -> Self {
        Self::new()
    }
}

/// Boolean mesh for CSG operations.
#[derive(Debug, Clone)]
pub struct BooleanMesh {
    /// Polygons.
    pub polygons: Vec<BspPolygon>,
}

impl BooleanMesh {
    /// Create from positions and faces.
    pub fn from_mesh(positions: &[Point3<f64>], faces: &[Vec<usize>]) -> Self {
        let polygons = faces
            .iter()
            .map(|face| {
                let vertices = face.iter().map(|&i| positions[i]).collect();
                BspPolygon::new(vertices)
            })
            .collect();

        Self { polygons }
    }

    /// Convert to mesh data.
    pub fn to_mesh(&self) -> (Vec<Point3<f64>>, Vec<Vec<usize>>) {
        let mut positions = Vec::new();
        let mut faces = Vec::new();
        let mut vertex_map: HashMap<[i64; 3], usize> = HashMap::new();

        let quantize = |p: Point3<f64>| -> [i64; 3] {
            [(p.x * 1e6) as i64, (p.y * 1e6) as i64, (p.z * 1e6) as i64]
        };

        for poly in &self.polygons {
            let mut face = Vec::new();
            for &vertex in &poly.vertices {
                let key = quantize(vertex);
                let idx = if let Some(&idx) = vertex_map.get(&key) {
                    idx
                } else {
                    let idx = positions.len();
                    positions.push(vertex);
                    vertex_map.insert(key, idx);
                    idx
                };
                face.push(idx);
            }
            if face.len() >= 3 {
                faces.push(face);
            }
        }

        (positions, faces)
    }

    /// Create BSP tree from this mesh.
    fn to_bsp(&self) -> BspNode {
        BspNode::from_polygons(self.polygons.clone())
    }

    /// Create mesh from BSP tree.
    fn from_bsp(node: &BspNode) -> Self {
        Self {
            polygons: node.all_polygons(),
        }
    }

    /// Union of two meshes.
    pub fn union(a: &BooleanMesh, b: &BooleanMesh) -> BooleanMesh {
        let mut a_tree = a.to_bsp();
        let mut b_tree = b.to_bsp();

        a_tree.clip_to(&b_tree);
        b_tree.clip_to(&a_tree);
        b_tree.invert();
        b_tree.clip_to(&a_tree);
        b_tree.invert();

        let mut all_polys = a_tree.all_polygons();
        all_polys.extend(b_tree.all_polygons());

        BooleanMesh::from_bsp(&BspNode::from_polygons(all_polys))
    }

    /// Intersection of two meshes.
    pub fn intersection(a: &BooleanMesh, b: &BooleanMesh) -> BooleanMesh {
        let mut a_tree = a.to_bsp();
        let mut b_tree = b.to_bsp();

        a_tree.invert();
        b_tree.clip_to(&a_tree);
        b_tree.invert();
        a_tree.clip_to(&b_tree);
        b_tree.clip_to(&a_tree);

        let mut all_polys = a_tree.all_polygons();
        all_polys.extend(b_tree.all_polygons());

        let mut result = BooleanMesh::from_bsp(&BspNode::from_polygons(all_polys));

        // Invert to get correct orientation
        for poly in &mut result.polygons {
            poly.flip();
        }

        result
    }

    /// Difference of two meshes (A - B).
    pub fn difference(a: &BooleanMesh, b: &BooleanMesh) -> BooleanMesh {
        let mut a_tree = a.to_bsp();
        let mut b_tree = b.to_bsp();

        a_tree.invert();
        a_tree.clip_to(&b_tree);
        b_tree.clip_to(&a_tree);
        b_tree.invert();
        b_tree.clip_to(&a_tree);
        b_tree.invert();

        let mut all_polys = a_tree.all_polygons();
        all_polys.extend(b_tree.all_polygons());

        let mut result = BooleanMesh::from_bsp(&BspNode::from_polygons(all_polys));

        // Invert to get correct orientation
        for poly in &mut result.polygons {
            poly.flip();
        }

        result
    }

    /// Apply boolean operation.
    pub fn apply(op: BooleanOp, a: &BooleanMesh, b: &BooleanMesh) -> BooleanMesh {
        match op {
            BooleanOp::Union => Self::union(a, b),
            BooleanOp::Intersection => Self::intersection(a, b),
            BooleanOp::Difference => Self::difference(a, b),
        }
    }
}

/// High-level boolean operation on mesh data.
pub fn boolean_operation(
    op: BooleanOp,
    positions_a: &[Point3<f64>],
    faces_a: &[Vec<usize>],
    positions_b: &[Point3<f64>],
    faces_b: &[Vec<usize>],
) -> (Vec<Point3<f64>>, Vec<Vec<usize>>) {
    let mesh_a = BooleanMesh::from_mesh(positions_a, faces_a);
    let mesh_b = BooleanMesh::from_mesh(positions_b, faces_b);
    let result = BooleanMesh::apply(op, &mesh_a, &mesh_b);
    result.to_mesh()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_cube(center: Point3<f64>, size: f64) -> (Vec<Point3<f64>>, Vec<Vec<usize>>) {
        let h = size / 2.0;
        let positions = vec![
            Point3::new(center.x - h, center.y - h, center.z - h),
            Point3::new(center.x + h, center.y - h, center.z - h),
            Point3::new(center.x + h, center.y + h, center.z - h),
            Point3::new(center.x - h, center.y + h, center.z - h),
            Point3::new(center.x - h, center.y - h, center.z + h),
            Point3::new(center.x + h, center.y - h, center.z + h),
            Point3::new(center.x + h, center.y + h, center.z + h),
            Point3::new(center.x - h, center.y + h, center.z + h),
        ];

        let faces = vec![
            vec![0, 3, 2, 1], // front
            vec![4, 5, 6, 7], // back
            vec![0, 4, 7, 3], // left
            vec![1, 2, 6, 5], // right
            vec![3, 7, 6, 2], // top
            vec![0, 1, 5, 4], // bottom
        ];

        (positions, faces)
    }

    #[test]
    fn test_plane_creation() {
        let plane = Plane::from_points(
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
        );
        assert!((plane.normal.z - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_point_classification() {
        let plane = Plane::from_normal_and_point(Vector3::new(0.0, 0.0, 1.0), Point3::origin());

        assert_eq!(
            plane.classify_point(Point3::new(0.0, 0.0, 1.0)),
            PointClassification::Front
        );
        assert_eq!(
            plane.classify_point(Point3::new(0.0, 0.0, -1.0)),
            PointClassification::Back
        );
        assert_eq!(
            plane.classify_point(Point3::new(0.0, 0.0, 0.0)),
            PointClassification::Coplanar
        );
    }

    #[test]
    fn test_boolean_union() {
        let (pos_a, faces_a) = create_cube(Point3::new(0.0, 0.0, 0.0), 1.0);
        let (pos_b, faces_b) = create_cube(Point3::new(0.5, 0.0, 0.0), 1.0);

        let mesh_a = BooleanMesh::from_mesh(&pos_a, &faces_a);
        let mesh_b = BooleanMesh::from_mesh(&pos_b, &faces_b);

        let result = BooleanMesh::union(&mesh_a, &mesh_b);
        assert!(!result.polygons.is_empty());
    }

    #[test]
    fn test_boolean_intersection() {
        let (pos_a, faces_a) = create_cube(Point3::new(0.0, 0.0, 0.0), 1.0);
        let (pos_b, faces_b) = create_cube(Point3::new(0.25, 0.0, 0.0), 1.0);

        let mesh_a = BooleanMesh::from_mesh(&pos_a, &faces_a);
        let mesh_b = BooleanMesh::from_mesh(&pos_b, &faces_b);

        let result = BooleanMesh::intersection(&mesh_a, &mesh_b);
        assert!(!result.polygons.is_empty());
    }

    #[test]
    fn test_boolean_difference() {
        let (pos_a, faces_a) = create_cube(Point3::new(0.0, 0.0, 0.0), 1.0);
        let (pos_b, faces_b) = create_cube(Point3::new(0.25, 0.0, 0.0), 0.5);

        let mesh_a = BooleanMesh::from_mesh(&pos_a, &faces_a);
        let mesh_b = BooleanMesh::from_mesh(&pos_b, &faces_b);

        let result = BooleanMesh::difference(&mesh_a, &mesh_b);
        assert!(!result.polygons.is_empty());
    }
}
