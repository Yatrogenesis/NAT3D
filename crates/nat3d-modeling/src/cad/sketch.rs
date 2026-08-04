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

//! 2D sketching.
//!
//! Implements 2D parametric sketching for CAD modeling with support for
//! lines, arcs, circles, splines, and geometric constraints.

use nalgebra::{Matrix4, Point2, Point3, Vector3};
use std::collections::HashMap;

/// Unique identifier for sketch entities.
pub type EntityId = u32;

/// A 2D sketch on a plane.
#[derive(Debug, Clone)]
pub struct Sketch {
    /// Sketch plane definition.
    pub plane: SketchPlane,
    /// Sketch entities (points, lines, etc.).
    entities: HashMap<EntityId, SketchEntity>,
    /// Next entity ID.
    next_id: EntityId,
    /// Construction geometry flag per entity.
    construction: HashMap<EntityId, bool>,
}

/// Definition of the sketch plane.
#[derive(Debug, Clone)]
pub struct SketchPlane {
    /// Origin point on the plane.
    pub origin: Point3<f64>,
    /// Normal vector to the plane.
    pub normal: Vector3<f64>,
    /// X-axis direction on the plane.
    pub x_axis: Vector3<f64>,
    /// Y-axis direction on the plane.
    pub y_axis: Vector3<f64>,
}

/// A sketch entity.
#[derive(Debug, Clone)]
pub enum SketchEntity {
    /// A point.
    Point(SketchPoint),
    /// A line segment.
    Line(SketchLine),
    /// A circular arc.
    Arc(SketchArc),
    /// A full circle.
    Circle(SketchCircle),
    /// An ellipse.
    Ellipse(SketchEllipse),
    /// A spline curve.
    Spline(SketchSpline),
    /// A rectangle (4 connected lines).
    Rectangle(SketchRectangle),
    /// A polygon.
    Polygon(SketchPolygon),
}

/// A sketch point.
#[derive(Debug, Clone)]
pub struct SketchPoint {
    /// Position in sketch coordinates.
    pub position: Point2<f64>,
    /// Whether this is a fixed point.
    pub fixed: bool,
}

/// A sketch line segment.
#[derive(Debug, Clone)]
pub struct SketchLine {
    /// Start point.
    pub start: Point2<f64>,
    /// End point.
    pub end: Point2<f64>,
}

/// A sketch arc.
#[derive(Debug, Clone)]
pub struct SketchArc {
    /// Center point.
    pub center: Point2<f64>,
    /// Radius.
    pub radius: f64,
    /// Start angle in radians.
    pub start_angle: f64,
    /// End angle in radians.
    pub end_angle: f64,
}

/// A sketch circle.
#[derive(Debug, Clone)]
pub struct SketchCircle {
    /// Center point.
    pub center: Point2<f64>,
    /// Radius.
    pub radius: f64,
}

/// A sketch ellipse.
#[derive(Debug, Clone)]
pub struct SketchEllipse {
    /// Center point.
    pub center: Point2<f64>,
    /// Semi-major axis length.
    pub major_radius: f64,
    /// Semi-minor axis length.
    pub minor_radius: f64,
    /// Rotation angle in radians.
    pub rotation: f64,
}

/// A sketch spline.
#[derive(Debug, Clone)]
pub struct SketchSpline {
    /// Control points.
    pub points: Vec<Point2<f64>>,
    /// Spline degree.
    pub degree: usize,
    /// Whether the spline is closed.
    pub closed: bool,
}

/// A sketch rectangle.
#[derive(Debug, Clone)]
pub struct SketchRectangle {
    /// Corner point.
    pub corner: Point2<f64>,
    /// Width (X direction).
    pub width: f64,
    /// Height (Y direction).
    pub height: f64,
    /// Rotation angle in radians.
    pub rotation: f64,
}

/// A sketch polygon.
#[derive(Debug, Clone)]
pub struct SketchPolygon {
    /// Vertices of the polygon.
    pub vertices: Vec<Point2<f64>>,
    /// Whether the polygon is closed.
    pub closed: bool,
}

impl SketchPlane {
    /// Create an XY plane at origin.
    pub fn xy() -> Self {
        Self {
            origin: Point3::origin(),
            normal: Vector3::z(),
            x_axis: Vector3::x(),
            y_axis: Vector3::y(),
        }
    }

    /// Create an XZ plane at origin.
    pub fn xz() -> Self {
        Self {
            origin: Point3::origin(),
            normal: Vector3::y(),
            x_axis: Vector3::x(),
            y_axis: Vector3::z(),
        }
    }

    /// Create a YZ plane at origin.
    pub fn yz() -> Self {
        Self {
            origin: Point3::origin(),
            normal: Vector3::x(),
            x_axis: Vector3::y(),
            y_axis: Vector3::z(),
        }
    }

    /// Create a plane from origin and normal.
    pub fn from_normal(origin: Point3<f64>, normal: Vector3<f64>) -> Self {
        let normal = normal.normalize();

        // Find perpendicular vectors
        let x_axis = if normal.x.abs() < 0.9 {
            Vector3::x().cross(&normal).normalize()
        } else {
            Vector3::y().cross(&normal).normalize()
        };
        let y_axis = normal.cross(&x_axis);

        Self {
            origin,
            normal,
            x_axis,
            y_axis,
        }
    }

    /// Convert 2D sketch coordinates to 3D world coordinates.
    pub fn to_world(&self, point: Point2<f64>) -> Point3<f64> {
        self.origin + self.x_axis * point.x + self.y_axis * point.y
    }

    /// Convert 3D world coordinates to 2D sketch coordinates.
    pub fn to_sketch(&self, point: Point3<f64>) -> Point2<f64> {
        let relative = point - self.origin;
        Point2::new(relative.dot(&self.x_axis), relative.dot(&self.y_axis))
    }

    /// Get the plane transform matrix.
    pub fn transform(&self) -> Matrix4<f64> {
        Matrix4::new(
            self.x_axis.x,
            self.y_axis.x,
            self.normal.x,
            self.origin.x,
            self.x_axis.y,
            self.y_axis.y,
            self.normal.y,
            self.origin.y,
            self.x_axis.z,
            self.y_axis.z,
            self.normal.z,
            self.origin.z,
            0.0,
            0.0,
            0.0,
            1.0,
        )
    }
}

impl Sketch {
    /// Create a new sketch on a plane.
    pub fn new(plane: SketchPlane) -> Self {
        Self {
            plane,
            entities: HashMap::new(),
            next_id: 1,
            construction: HashMap::new(),
        }
    }

    /// Create a sketch on the XY plane.
    pub fn on_xy() -> Self {
        Self::new(SketchPlane::xy())
    }

    /// Create a sketch on the XZ plane.
    pub fn on_xz() -> Self {
        Self::new(SketchPlane::xz())
    }

    /// Create a sketch on the YZ plane.
    pub fn on_yz() -> Self {
        Self::new(SketchPlane::yz())
    }

    /// Add an entity to the sketch.
    fn add_entity(&mut self, entity: SketchEntity, construction: bool) -> EntityId {
        let id = self.next_id;
        self.next_id += 1;
        self.entities.insert(id, entity);
        self.construction.insert(id, construction);
        id
    }

    /// Add a point.
    pub fn add_point(&mut self, position: Point2<f64>) -> EntityId {
        self.add_entity(
            SketchEntity::Point(SketchPoint {
                position,
                fixed: false,
            }),
            false,
        )
    }

    /// Add a line.
    pub fn add_line(&mut self, start: Point2<f64>, end: Point2<f64>) -> EntityId {
        self.add_entity(SketchEntity::Line(SketchLine { start, end }), false)
    }

    /// Add a circle.
    pub fn add_circle(&mut self, center: Point2<f64>, radius: f64) -> EntityId {
        self.add_entity(SketchEntity::Circle(SketchCircle { center, radius }), false)
    }

    /// Add an arc.
    pub fn add_arc(
        &mut self,
        center: Point2<f64>,
        radius: f64,
        start_angle: f64,
        end_angle: f64,
    ) -> EntityId {
        self.add_entity(
            SketchEntity::Arc(SketchArc {
                center,
                radius,
                start_angle,
                end_angle,
            }),
            false,
        )
    }

    /// Add an ellipse.
    pub fn add_ellipse(
        &mut self,
        center: Point2<f64>,
        major_radius: f64,
        minor_radius: f64,
        rotation: f64,
    ) -> EntityId {
        self.add_entity(
            SketchEntity::Ellipse(SketchEllipse {
                center,
                major_radius,
                minor_radius,
                rotation,
            }),
            false,
        )
    }

    /// Add a rectangle.
    pub fn add_rectangle(&mut self, corner: Point2<f64>, width: f64, height: f64) -> EntityId {
        self.add_entity(
            SketchEntity::Rectangle(SketchRectangle {
                corner,
                width,
                height,
                rotation: 0.0,
            }),
            false,
        )
    }

    /// Add a polygon.
    pub fn add_polygon(&mut self, vertices: Vec<Point2<f64>>, closed: bool) -> EntityId {
        self.add_entity(
            SketchEntity::Polygon(SketchPolygon { vertices, closed }),
            false,
        )
    }

    /// Add a spline.
    pub fn add_spline(
        &mut self,
        points: Vec<Point2<f64>>,
        degree: usize,
        closed: bool,
    ) -> EntityId {
        self.add_entity(
            SketchEntity::Spline(SketchSpline {
                points,
                degree,
                closed,
            }),
            false,
        )
    }

    /// Set an entity as construction geometry.
    pub fn set_construction(&mut self, id: EntityId, is_construction: bool) {
        self.construction.insert(id, is_construction);
    }

    /// Check if an entity is construction geometry.
    pub fn is_construction(&self, id: EntityId) -> bool {
        self.construction.get(&id).copied().unwrap_or(false)
    }

    /// Get an entity by ID.
    pub fn get(&self, id: EntityId) -> Option<&SketchEntity> {
        self.entities.get(&id)
    }

    /// Get mutable entity by ID.
    pub fn get_mut(&mut self, id: EntityId) -> Option<&mut SketchEntity> {
        self.entities.get_mut(&id)
    }

    /// Remove an entity.
    pub fn remove(&mut self, id: EntityId) -> Option<SketchEntity> {
        self.construction.remove(&id);
        self.entities.remove(&id)
    }

    /// Get all entity IDs.
    pub fn entity_ids(&self) -> Vec<EntityId> {
        self.entities.keys().copied().collect()
    }

    /// Get all non-construction entities.
    pub fn geometry_entities(&self) -> Vec<EntityId> {
        self.entities
            .keys()
            .filter(|id| !self.is_construction(**id))
            .copied()
            .collect()
    }

    /// Tessellate all entities into line segments.
    pub fn tessellate(&self, segments_per_curve: usize) -> Vec<Vec<Point2<f64>>> {
        let mut result = Vec::new();

        for (id, entity) in &self.entities {
            if self.is_construction(*id) {
                continue;
            }

            match entity {
                SketchEntity::Point(_) => {
                    // Points don't produce tessellation
                }
                SketchEntity::Line(line) => {
                    result.push(vec![line.start, line.end]);
                }
                SketchEntity::Arc(arc) => {
                    result.push(tessellate_arc(arc, segments_per_curve));
                }
                SketchEntity::Circle(circle) => {
                    result.push(tessellate_circle(circle, segments_per_curve));
                }
                SketchEntity::Ellipse(ellipse) => {
                    result.push(tessellate_ellipse(ellipse, segments_per_curve));
                }
                SketchEntity::Rectangle(rect) => {
                    result.push(tessellate_rectangle(rect));
                }
                SketchEntity::Polygon(poly) => {
                    let mut points = poly.vertices.clone();
                    if poly.closed && !points.is_empty() {
                        points.push(points[0]);
                    }
                    result.push(points);
                }
                SketchEntity::Spline(spline) => {
                    result.push(tessellate_spline(spline, segments_per_curve));
                }
            }
        }

        result
    }

    /// Tessellate to 3D world coordinates.
    pub fn tessellate_3d(&self, segments_per_curve: usize) -> Vec<Vec<Point3<f64>>> {
        self.tessellate(segments_per_curve)
            .into_iter()
            .map(|profile| {
                profile
                    .into_iter()
                    .map(|p| self.plane.to_world(p))
                    .collect()
            })
            .collect()
    }

    /// Find closed profiles in the sketch.
    pub fn find_profiles(&self) -> Vec<SketchProfile> {
        let chains = self.tessellate(32);
        let mut profiles = Vec::new();

        for chain in chains {
            if chain.len() >= 3 {
                let first = chain.first().unwrap();
                let last = chain.last().unwrap();
                let dist = ((first.x - last.x).powi(2) + (first.y - last.y).powi(2)).sqrt();

                if dist < 1e-6 {
                    profiles.push(SketchProfile {
                        outer: chain,
                        holes: Vec::new(),
                    });
                }
            }
        }

        profiles
    }

    /// Calculate sketch bounds.
    pub fn bounds(&self) -> Option<(Point2<f64>, Point2<f64>)> {
        let mut min = Point2::new(f64::MAX, f64::MAX);
        let mut max = Point2::new(f64::MIN, f64::MIN);
        let mut has_points = false;

        for entity in self.entities.values() {
            let points = match entity {
                SketchEntity::Point(p) => vec![p.position],
                SketchEntity::Line(l) => vec![l.start, l.end],
                SketchEntity::Arc(a) => {
                    vec![
                        Point2::new(a.center.x - a.radius, a.center.y - a.radius),
                        Point2::new(a.center.x + a.radius, a.center.y + a.radius),
                    ]
                }
                SketchEntity::Circle(c) => {
                    vec![
                        Point2::new(c.center.x - c.radius, c.center.y - c.radius),
                        Point2::new(c.center.x + c.radius, c.center.y + c.radius),
                    ]
                }
                SketchEntity::Ellipse(e) => {
                    let r = e.major_radius.max(e.minor_radius);
                    vec![
                        Point2::new(e.center.x - r, e.center.y - r),
                        Point2::new(e.center.x + r, e.center.y + r),
                    ]
                }
                SketchEntity::Rectangle(r) => tessellate_rectangle(r),
                SketchEntity::Polygon(p) => p.vertices.clone(),
                SketchEntity::Spline(s) => s.points.clone(),
            };

            for p in points {
                min.x = min.x.min(p.x);
                min.y = min.y.min(p.y);
                max.x = max.x.max(p.x);
                max.y = max.y.max(p.y);
                has_points = true;
            }
        }

        if has_points {
            Some((min, max))
        } else {
            None
        }
    }
}

/// A closed profile for extrusion.
#[derive(Debug, Clone)]
pub struct SketchProfile {
    /// Outer boundary vertices.
    pub outer: Vec<Point2<f64>>,
    /// Inner hole boundaries.
    pub holes: Vec<Vec<Point2<f64>>>,
}

impl SketchProfile {
    /// Convert to 3D profile on a plane.
    pub fn to_3d(&self, plane: &SketchPlane) -> Profile3D {
        Profile3D {
            outer: self.outer.iter().map(|p| plane.to_world(*p)).collect(),
            holes: self
                .holes
                .iter()
                .map(|h| h.iter().map(|p| plane.to_world(*p)).collect())
                .collect(),
        }
    }

    /// Calculate the signed area (positive for CCW).
    pub fn signed_area(&self) -> f64 {
        let mut area = 0.0;
        for i in 0..self.outer.len() {
            let j = (i + 1) % self.outer.len();
            area += self.outer[i].x * self.outer[j].y;
            area -= self.outer[j].x * self.outer[i].y;
        }
        area / 2.0
    }

    /// Check if profile is counter-clockwise.
    pub fn is_ccw(&self) -> bool {
        self.signed_area() > 0.0
    }

    /// Reverse the profile direction.
    pub fn reverse(&mut self) {
        self.outer.reverse();
        for hole in &mut self.holes {
            hole.reverse();
        }
    }
}

/// A 3D profile for extrusion.
#[derive(Debug, Clone)]
pub struct Profile3D {
    /// Outer boundary vertices.
    pub outer: Vec<Point3<f64>>,
    /// Inner hole boundaries.
    pub holes: Vec<Vec<Point3<f64>>>,
}

// Tessellation helpers

fn tessellate_arc(arc: &SketchArc, segments: usize) -> Vec<Point2<f64>> {
    let mut points = Vec::with_capacity(segments + 1);
    let angle_span = arc.end_angle - arc.start_angle;

    for i in 0..=segments {
        let t = i as f64 / segments as f64;
        let angle = arc.start_angle + t * angle_span;
        points.push(Point2::new(
            arc.center.x + arc.radius * angle.cos(),
            arc.center.y + arc.radius * angle.sin(),
        ));
    }

    points
}

fn tessellate_circle(circle: &SketchCircle, segments: usize) -> Vec<Point2<f64>> {
    use std::f64::consts::PI;

    let mut points = Vec::with_capacity(segments + 1);

    for i in 0..=segments {
        let angle = 2.0 * PI * i as f64 / segments as f64;
        points.push(Point2::new(
            circle.center.x + circle.radius * angle.cos(),
            circle.center.y + circle.radius * angle.sin(),
        ));
    }

    points
}

fn tessellate_ellipse(ellipse: &SketchEllipse, segments: usize) -> Vec<Point2<f64>> {
    use std::f64::consts::PI;

    let mut points = Vec::with_capacity(segments + 1);
    let cos_r = ellipse.rotation.cos();
    let sin_r = ellipse.rotation.sin();

    for i in 0..=segments {
        let angle = 2.0 * PI * i as f64 / segments as f64;
        let x = ellipse.major_radius * angle.cos();
        let y = ellipse.minor_radius * angle.sin();

        // Rotate
        let rx = x * cos_r - y * sin_r;
        let ry = x * sin_r + y * cos_r;

        points.push(Point2::new(ellipse.center.x + rx, ellipse.center.y + ry));
    }

    points
}

fn tessellate_rectangle(rect: &SketchRectangle) -> Vec<Point2<f64>> {
    let cos_r = rect.rotation.cos();
    let sin_r = rect.rotation.sin();

    let corners = [
        (0.0, 0.0),
        (rect.width, 0.0),
        (rect.width, rect.height),
        (0.0, rect.height),
        (0.0, 0.0), // Close the rectangle
    ];

    corners
        .iter()
        .map(|&(x, y)| {
            let rx = x * cos_r - y * sin_r;
            let ry = x * sin_r + y * cos_r;
            Point2::new(rect.corner.x + rx, rect.corner.y + ry)
        })
        .collect()
}

fn tessellate_spline(spline: &SketchSpline, segments: usize) -> Vec<Point2<f64>> {
    if spline.points.len() < 2 {
        return spline.points.clone();
    }

    // Simple Catmull-Rom spline interpolation
    let n = spline.points.len();
    let total_segments = (n - 1) * segments;
    let mut result = Vec::with_capacity(total_segments + 1);

    for i in 0..n - 1 {
        let p0 = if i == 0 {
            if spline.closed {
                spline.points[n - 1]
            } else {
                spline.points[0]
            }
        } else {
            spline.points[i - 1]
        };

        let p1 = spline.points[i];
        let p2 = spline.points[i + 1];

        let p3 = if i + 2 >= n {
            if spline.closed {
                spline.points[(i + 2) % n]
            } else {
                spline.points[n - 1]
            }
        } else {
            spline.points[i + 2]
        };

        for j in 0..segments {
            let t = j as f64 / segments as f64;
            let t2 = t * t;
            let t3 = t2 * t;

            let x = 0.5
                * ((2.0 * p1.x)
                    + (-p0.x + p2.x) * t
                    + (2.0 * p0.x - 5.0 * p1.x + 4.0 * p2.x - p3.x) * t2
                    + (-p0.x + 3.0 * p1.x - 3.0 * p2.x + p3.x) * t3);

            let y = 0.5
                * ((2.0 * p1.y)
                    + (-p0.y + p2.y) * t
                    + (2.0 * p0.y - 5.0 * p1.y + 4.0 * p2.y - p3.y) * t2
                    + (-p0.y + 3.0 * p1.y - 3.0 * p2.y + p3.y) * t3);

            result.push(Point2::new(x, y));
        }
    }

    // Add last point
    result.push(*spline.points.last().unwrap());

    if spline.closed && !result.is_empty() {
        result.push(result[0]);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sketch_plane() {
        let plane = SketchPlane::xy();
        let p2d = Point2::new(1.0, 2.0);
        let p3d = plane.to_world(p2d);

        assert!((p3d.x - 1.0).abs() < 1e-10);
        assert!((p3d.y - 2.0).abs() < 1e-10);
        assert!(p3d.z.abs() < 1e-10);

        let back = plane.to_sketch(p3d);
        assert!((back.x - p2d.x).abs() < 1e-10);
        assert!((back.y - p2d.y).abs() < 1e-10);
    }

    #[test]
    fn test_add_entities() {
        let mut sketch = Sketch::on_xy();

        let line_id = sketch.add_line(Point2::new(0.0, 0.0), Point2::new(1.0, 0.0));
        let circle_id = sketch.add_circle(Point2::new(0.0, 0.0), 1.0);

        assert!(sketch.get(line_id).is_some());
        assert!(sketch.get(circle_id).is_some());
    }

    #[test]
    fn test_rectangle() {
        let mut sketch = Sketch::on_xy();
        sketch.add_rectangle(Point2::new(0.0, 0.0), 2.0, 1.0);

        let bounds = sketch.bounds().unwrap();
        assert!((bounds.0.x - 0.0).abs() < 1e-10);
        assert!((bounds.1.x - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_profile_area() {
        let profile = SketchProfile {
            outer: vec![
                Point2::new(0.0, 0.0),
                Point2::new(1.0, 0.0),
                Point2::new(1.0, 1.0),
                Point2::new(0.0, 1.0),
            ],
            holes: Vec::new(),
        };

        let area = profile.signed_area().abs();
        assert!((area - 1.0).abs() < 1e-10);
    }
}
