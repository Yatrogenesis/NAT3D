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

//! Surface trimming.
//!
//! Implements trimming operations for NURBS surfaces using 2D trim curves
//! in parameter space.

use super::surface::NurbsSurface;
use nalgebra::{Point2, Point3, Vector3};

/// A 2D trim curve in parameter space.
#[derive(Debug, Clone)]
pub struct TrimCurve {
    /// Control points in UV parameter space.
    control_points: Vec<Point2<f64>>,
    /// Weights for rational curves.
    weights: Vec<f64>,
    /// Knot vector.
    knots: Vec<f64>,
    /// Curve degree.
    degree: usize,
}

/// Trim loop direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrimDirection {
    /// Outer boundary (counter-clockwise).
    Outer,
    /// Inner hole (clockwise).
    Inner,
}

/// A closed trim loop.
#[derive(Debug, Clone)]
pub struct TrimLoop {
    /// Curves forming the loop.
    curves: Vec<TrimCurve>,
    /// Direction of the loop.
    direction: TrimDirection,
}

/// A trimmed NURBS surface.
#[derive(Debug, Clone)]
pub struct TrimmedSurface {
    /// The underlying surface.
    surface: NurbsSurface,
    /// Outer boundary loops.
    outer_loops: Vec<TrimLoop>,
    /// Inner hole loops.
    inner_loops: Vec<TrimLoop>,
}

impl TrimCurve {
    /// Create a new trim curve.
    pub fn new(
        control_points: Vec<Point2<f64>>,
        weights: Vec<f64>,
        knots: Vec<f64>,
        degree: usize,
    ) -> Option<Self> {
        if control_points.len() != weights.len() {
            return None;
        }
        if knots.len() != control_points.len() + degree + 1 {
            return None;
        }
        if control_points.len() <= degree {
            return None;
        }

        Some(Self {
            control_points,
            weights,
            knots,
            degree,
        })
    }

    /// Create a uniform trim curve with equal weights.
    pub fn uniform(control_points: Vec<Point2<f64>>, degree: usize) -> Option<Self> {
        let n = control_points.len();
        if n <= degree {
            return None;
        }

        let weights = vec![1.0; n];
        let mut knots = Vec::with_capacity(n + degree + 1);

        // Clamped uniform knot vector
        knots.extend(std::iter::repeat(0.0_f64).take(degree + 1));
        let internal = n - degree;
        for i in 1..internal {
            knots.push(i as f64 / internal as f64);
        }
        knots.extend(std::iter::repeat(1.0_f64).take(degree + 1));

        Some(Self {
            control_points,
            weights,
            knots,
            degree,
        })
    }

    /// Create a line segment.
    pub fn line(start: Point2<f64>, end: Point2<f64>) -> Self {
        Self {
            control_points: vec![start, end],
            weights: vec![1.0, 1.0],
            knots: vec![0.0, 0.0, 1.0, 1.0],
            degree: 1,
        }
    }

    /// Create a circular arc in parameter space.
    pub fn arc(center: Point2<f64>, radius: f64, start_angle: f64, end_angle: f64) -> Self {
        use std::f64::consts::PI;

        let angle_span = end_angle - start_angle;
        let num_arcs = ((angle_span.abs() / (PI / 2.0)).ceil() as usize).max(1);
        let arc_angle = angle_span / num_arcs as f64;

        let w_mid = (arc_angle / 2.0).cos();

        let mut control_points = Vec::with_capacity(num_arcs * 2 + 1);
        let mut weights = Vec::with_capacity(num_arcs * 2 + 1);

        for i in 0..=num_arcs {
            let angle = start_angle + i as f64 * arc_angle;
            let p = Point2::new(
                center.x + radius * angle.cos(),
                center.y + radius * angle.sin(),
            );
            control_points.push(p);
            weights.push(1.0);

            if i < num_arcs {
                let mid_angle = angle + arc_angle / 2.0;
                let mid_r = radius / w_mid.max(0.001);
                let mid_p = Point2::new(
                    center.x + mid_r * mid_angle.cos(),
                    center.y + mid_r * mid_angle.sin(),
                );
                control_points.push(mid_p);
                weights.push(w_mid);
            }
        }

        let n = control_points.len();
        let degree = 2;
        let mut knots = vec![0.0; degree + 1];
        for i in 1..num_arcs {
            let t = i as f64 / num_arcs as f64;
            knots.push(t);
            knots.push(t);
        }
        knots.extend(std::iter::repeat(1.0_f64).take(degree + 1));

        // Adjust knot vector size
        while knots.len() < n + degree + 1 {
            let t = knots.len() as f64 / (n + degree) as f64;
            knots.insert(knots.len() - degree - 1, t);
        }

        Self {
            control_points,
            weights,
            knots,
            degree,
        }
    }

    /// Evaluate the curve at parameter t.
    pub fn evaluate(&self, t: f64) -> Point2<f64> {
        let t = t.clamp(
            self.knots[self.degree],
            self.knots[self.control_points.len()],
        );
        let k = self.find_span(t);

        let mut d: Vec<(Point2<f64>, f64)> = Vec::with_capacity(self.degree + 1);
        for i in 0..=self.degree {
            let idx = k - self.degree + i;
            let pt = self.control_points[idx];
            let w = self.weights[idx];
            d.push((Point2::new(pt.x * w, pt.y * w), w));
        }

        for r in 1..=self.degree {
            for j in (r..=self.degree).rev() {
                let idx = k - self.degree + j;
                let alpha = (t - self.knots[idx])
                    / (self.knots[idx + self.degree + 1 - r] - self.knots[idx]);
                let (pt0, w0) = d[j - 1];
                let (pt1, w1) = d[j];
                let x = pt0.x * (1.0 - alpha) + pt1.x * alpha;
                let y = pt0.y * (1.0 - alpha) + pt1.y * alpha;
                let w = w0 * (1.0 - alpha) + w1 * alpha;
                d[j] = (Point2::new(x, y), w);
            }
        }

        let (pt, w) = d[self.degree];
        Point2::new(pt.x / w, pt.y / w)
    }

    fn find_span(&self, t: f64) -> usize {
        let n = self.control_points.len() - 1;
        if t >= self.knots[n + 1] {
            return n;
        }
        if t <= self.knots[self.degree] {
            return self.degree;
        }

        let mut low = self.degree;
        let mut high = n + 1;
        let mut mid = (low + high) / 2;

        while t < self.knots[mid] || t >= self.knots[mid + 1] {
            if t < self.knots[mid] {
                high = mid;
            } else {
                low = mid;
            }
            mid = (low + high) / 2;
        }

        mid
    }

    /// Sample the curve at uniform parameters.
    pub fn sample(&self, num_samples: usize) -> Vec<Point2<f64>> {
        let t_min = self.knots[self.degree];
        let t_max = self.knots[self.control_points.len()];

        (0..num_samples)
            .map(|i| {
                let t = t_min + (t_max - t_min) * i as f64 / (num_samples - 1).max(1) as f64;
                self.evaluate(t)
            })
            .collect()
    }

    /// Get the start point.
    pub fn start(&self) -> Point2<f64> {
        self.evaluate(self.knots[self.degree])
    }

    /// Get the end point.
    pub fn end(&self) -> Point2<f64> {
        self.evaluate(self.knots[self.control_points.len()])
    }
}

impl TrimLoop {
    /// Create a new trim loop from curves.
    pub fn new(curves: Vec<TrimCurve>, direction: TrimDirection) -> Option<Self> {
        if curves.is_empty() {
            return None;
        }

        // Verify curves form a closed loop
        let eps = 1e-6;
        for i in 0..curves.len() {
            let next = (i + 1) % curves.len();
            let end = curves[i].end();
            let start = curves[next].start();
            let dist = ((end.x - start.x).powi(2) + (end.y - start.y).powi(2)).sqrt();
            if dist > eps {
                return None;
            }
        }

        Some(Self { curves, direction })
    }

    /// Create a rectangular trim loop.
    pub fn rectangle(
        u_min: f64,
        u_max: f64,
        v_min: f64,
        v_max: f64,
        direction: TrimDirection,
    ) -> Self {
        let p0 = Point2::new(u_min, v_min);
        let p1 = Point2::new(u_max, v_min);
        let p2 = Point2::new(u_max, v_max);
        let p3 = Point2::new(u_min, v_max);

        let curves = match direction {
            TrimDirection::Outer => vec![
                TrimCurve::line(p0, p1),
                TrimCurve::line(p1, p2),
                TrimCurve::line(p2, p3),
                TrimCurve::line(p3, p0),
            ],
            TrimDirection::Inner => vec![
                TrimCurve::line(p0, p3),
                TrimCurve::line(p3, p2),
                TrimCurve::line(p2, p1),
                TrimCurve::line(p1, p0),
            ],
        };

        Self { curves, direction }
    }

    /// Create a circular trim loop.
    pub fn circle(center: Point2<f64>, radius: f64, direction: TrimDirection) -> Self {
        use std::f64::consts::PI;

        let (start, end) = match direction {
            TrimDirection::Outer => (0.0, 2.0 * PI),
            TrimDirection::Inner => (2.0 * PI, 0.0),
        };

        let curve = TrimCurve::arc(center, radius, start, end);
        Self {
            curves: vec![curve],
            direction,
        }
    }

    /// Check if a point is inside the loop.
    pub fn contains(&self, point: Point2<f64>) -> bool {
        // Ray casting algorithm
        let mut crossings = 0;
        let num_samples = 100;

        for curve in &self.curves {
            let samples = curve.sample(num_samples);
            for i in 0..samples.len() - 1 {
                let p1 = samples[i];
                let p2 = samples[i + 1];

                if (p1.y <= point.y && p2.y > point.y) || (p2.y <= point.y && p1.y > point.y) {
                    let x_intersect = p1.x + (point.y - p1.y) / (p2.y - p1.y) * (p2.x - p1.x);
                    if point.x < x_intersect {
                        crossings += 1;
                    }
                }
            }
        }

        // Returns true if the point is inside the loop geometry
        // For outer loops: point must be inside
        // For inner loops (holes): point being inside the hole means it should be excluded
        crossings % 2 == 1
    }

    /// Sample all curves in the loop.
    pub fn sample(&self, samples_per_curve: usize) -> Vec<Point2<f64>> {
        let mut points = Vec::with_capacity(self.curves.len() * samples_per_curve);
        for curve in &self.curves {
            let samples = curve.sample(samples_per_curve);
            // Skip last point to avoid duplicates at curve junctions
            points.extend(samples.into_iter().take(samples_per_curve - 1));
        }
        points
    }

    /// Compute the signed area of the loop.
    pub fn signed_area(&self) -> f64 {
        let samples = self.sample(50);
        let mut area = 0.0;

        for i in 0..samples.len() {
            let j = (i + 1) % samples.len();
            area += samples[i].x * samples[j].y;
            area -= samples[j].x * samples[i].y;
        }

        area / 2.0
    }
}

impl TrimmedSurface {
    /// Create a new trimmed surface.
    pub fn new(surface: NurbsSurface) -> Self {
        Self {
            surface,
            outer_loops: Vec::new(),
            inner_loops: Vec::new(),
        }
    }

    /// Add an outer boundary loop.
    pub fn add_outer_loop(&mut self, loop_: TrimLoop) {
        self.outer_loops.push(loop_);
    }

    /// Add an inner hole loop.
    pub fn add_inner_loop(&mut self, loop_: TrimLoop) {
        self.inner_loops.push(loop_);
    }

    /// Check if a UV point is inside the trimmed region.
    pub fn is_inside(&self, uv: Point2<f64>) -> bool {
        // Must be inside at least one outer loop
        let inside_outer = if self.outer_loops.is_empty() {
            // No outer loops means the whole surface is valid
            true
        } else {
            self.outer_loops.iter().any(|loop_| loop_.contains(uv))
        };

        if !inside_outer {
            return false;
        }

        // Must not be inside any inner loop
        !self.inner_loops.iter().any(|loop_| loop_.contains(uv))
    }

    /// Evaluate the surface at (u, v) if inside trimmed region.
    pub fn evaluate(&self, u: f64, v: f64) -> Option<Point3<f64>> {
        if self.is_inside(Point2::new(u, v)) {
            Some(self.surface.evaluate(u, v))
        } else {
            None
        }
    }

    /// Tessellate the trimmed surface.
    pub fn tessellate(&self, u_segments: usize, v_segments: usize) -> TrimmedMesh {
        let (u_min, u_max) = self.surface.domain_u();
        let (v_min, v_max) = self.surface.domain_v();

        let mut vertices = Vec::new();
        let mut normals = Vec::new();
        let mut uvs = Vec::new();
        let mut indices = Vec::new();
        let mut vertex_map = std::collections::HashMap::new();

        let du = (u_max - u_min) / u_segments as f64;
        let dv = (v_max - v_min) / v_segments as f64;

        // Generate grid vertices
        for i in 0..=u_segments {
            for j in 0..=v_segments {
                let u = u_min + i as f64 * du;
                let v = v_min + j as f64 * dv;
                let uv = Point2::new(u, v);

                if self.is_inside(uv) {
                    let sp = self.surface.evaluate_with_derivatives(u, v);
                    let idx = vertices.len();
                    vertex_map.insert((i, j), idx);
                    vertices.push(sp.position);
                    normals.push(sp.normal);
                    uvs.push((i as f32 / u_segments as f32, j as f32 / v_segments as f32));
                }
            }
        }

        // Generate triangles
        for i in 0..u_segments {
            for j in 0..v_segments {
                let v00 = vertex_map.get(&(i, j));
                let v10 = vertex_map.get(&(i + 1, j));
                let v01 = vertex_map.get(&(i, j + 1));
                let v11 = vertex_map.get(&(i + 1, j + 1));

                match (v00, v10, v01, v11) {
                    (Some(&a), Some(&b), Some(&c), Some(&d)) => {
                        // Full quad - split into two triangles
                        indices.push(a as u32);
                        indices.push(b as u32);
                        indices.push(c as u32);

                        indices.push(b as u32);
                        indices.push(d as u32);
                        indices.push(c as u32);
                    }
                    (Some(&a), Some(&b), Some(&c), None) => {
                        // Triangle without d
                        indices.push(a as u32);
                        indices.push(b as u32);
                        indices.push(c as u32);
                    }
                    (Some(&a), Some(&b), None, Some(&d)) => {
                        // Triangle without c
                        indices.push(a as u32);
                        indices.push(b as u32);
                        indices.push(d as u32);
                    }
                    (Some(&a), None, Some(&c), Some(&d)) => {
                        // Triangle without b
                        indices.push(a as u32);
                        indices.push(d as u32);
                        indices.push(c as u32);
                    }
                    (None, Some(&b), Some(&c), Some(&d)) => {
                        // Triangle without a
                        indices.push(b as u32);
                        indices.push(d as u32);
                        indices.push(c as u32);
                    }
                    _ => {
                        // Not enough vertices for a valid triangle
                    }
                }
            }
        }

        TrimmedMesh {
            vertices,
            normals,
            uvs,
            indices,
        }
    }

    /// Get the underlying surface.
    pub fn surface(&self) -> &NurbsSurface {
        &self.surface
    }

    /// Get mutable reference to the underlying surface.
    pub fn surface_mut(&mut self) -> &mut NurbsSurface {
        &mut self.surface
    }

    /// Get outer loops.
    pub fn outer_loops(&self) -> &[TrimLoop] {
        &self.outer_loops
    }

    /// Get inner loops.
    pub fn inner_loops(&self) -> &[TrimLoop] {
        &self.inner_loops
    }

    /// Compute the trimmed surface area.
    pub fn area(&self, samples: usize) -> f64 {
        let (u_min, u_max) = self.surface.domain_u();
        let (v_min, v_max) = self.surface.domain_v();

        let du = (u_max - u_min) / samples as f64;
        let dv = (v_max - v_min) / samples as f64;

        let mut total_area = 0.0;

        for i in 0..samples {
            let u = u_min + (i as f64 + 0.5) * du;
            for j in 0..samples {
                let v = v_min + (j as f64 + 0.5) * dv;
                if self.is_inside(Point2::new(u, v)) {
                    let sp = self.surface.evaluate_with_derivatives(u, v);
                    let area_element = sp.du.cross(&sp.dv).norm() * du * dv;
                    total_area += area_element;
                }
            }
        }

        total_area
    }
}

/// Tessellated trimmed mesh.
#[derive(Debug, Clone)]
pub struct TrimmedMesh {
    /// Vertex positions.
    pub vertices: Vec<Point3<f64>>,
    /// Vertex normals.
    pub normals: Vec<Vector3<f64>>,
    /// UV coordinates.
    pub uvs: Vec<(f32, f32)>,
    /// Triangle indices.
    pub indices: Vec<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trim_curve_line() {
        let curve = TrimCurve::line(Point2::new(0.0, 0.0), Point2::new(1.0, 1.0));

        let mid = curve.evaluate(0.5);
        assert!((mid.x - 0.5).abs() < 1e-10);
        assert!((mid.y - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_trim_loop_rectangle() {
        let loop_ = TrimLoop::rectangle(0.0, 1.0, 0.0, 1.0, TrimDirection::Outer);

        assert!(loop_.contains(Point2::new(0.5, 0.5)));
        assert!(!loop_.contains(Point2::new(1.5, 0.5)));
    }

    #[test]
    fn test_trimmed_surface() {
        let corners = [
            [Point3::new(0.0, 0.0, 0.0), Point3::new(0.0, 1.0, 0.0)],
            [Point3::new(1.0, 0.0, 0.0), Point3::new(1.0, 1.0, 0.0)],
        ];
        let surface = NurbsSurface::bilinear(corners);

        let mut trimmed = TrimmedSurface::new(surface);
        let outer = TrimLoop::rectangle(0.0, 1.0, 0.0, 1.0, TrimDirection::Outer);
        trimmed.add_outer_loop(outer);

        // Add a hole in the center
        let inner = TrimLoop::circle(Point2::new(0.5, 0.5), 0.2, TrimDirection::Inner);
        trimmed.add_inner_loop(inner);

        assert!(trimmed.is_inside(Point2::new(0.1, 0.1)));
        assert!(!trimmed.is_inside(Point2::new(0.5, 0.5)));
    }
}
