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

//! NURBS surfaces.
//!
//! Implements Non-Uniform Rational B-Spline surfaces using the tensor product
//! of two NURBS curves, evaluated via de Boor's algorithm.

use nalgebra::{Point3, Vector3};

/// A NURBS surface.
#[derive(Debug, Clone)]
pub struct NurbsSurface {
    /// Control points arranged in a grid [u_count][v_count].
    control_points: Vec<Vec<Point3<f64>>>,
    /// Weights for each control point.
    weights: Vec<Vec<f64>>,
    /// Knot vector in U direction.
    knots_u: Vec<f64>,
    /// Knot vector in V direction.
    knots_v: Vec<f64>,
    /// Degree in U direction.
    degree_u: usize,
    /// Degree in V direction.
    degree_v: usize,
}

/// Surface evaluation result with position and derivatives.
#[derive(Debug, Clone)]
pub struct SurfacePoint {
    /// Position on surface.
    pub position: Point3<f64>,
    /// Partial derivative in U direction.
    pub du: Vector3<f64>,
    /// Partial derivative in V direction.
    pub dv: Vector3<f64>,
    /// Surface normal.
    pub normal: Vector3<f64>,
}

/// Isocurve direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsoDirection {
    /// Constant U, varying V.
    U,
    /// Constant V, varying U.
    V,
}

impl NurbsSurface {
    /// Create a new NURBS surface.
    pub fn new(
        control_points: Vec<Vec<Point3<f64>>>,
        weights: Vec<Vec<f64>>,
        knots_u: Vec<f64>,
        knots_v: Vec<f64>,
        degree_u: usize,
        degree_v: usize,
    ) -> Option<Self> {
        let u_count = control_points.len();
        if u_count == 0 {
            return None;
        }
        let v_count = control_points[0].len();
        if v_count == 0 {
            return None;
        }

        // Validate dimensions
        if weights.len() != u_count {
            return None;
        }
        for (cp_row, w_row) in control_points.iter().zip(weights.iter()) {
            if cp_row.len() != v_count || w_row.len() != v_count {
                return None;
            }
        }

        // Validate knot vectors
        if knots_u.len() != u_count + degree_u + 1 {
            return None;
        }
        if knots_v.len() != v_count + degree_v + 1 {
            return None;
        }

        Some(Self {
            control_points,
            weights,
            knots_u,
            knots_v,
            degree_u,
            degree_v,
        })
    }

    /// Create a uniform NURBS surface with equal weights.
    pub fn uniform(
        control_points: Vec<Vec<Point3<f64>>>,
        degree_u: usize,
        degree_v: usize,
    ) -> Option<Self> {
        let u_count = control_points.len();
        if u_count == 0 {
            return None;
        }
        let v_count = control_points[0].len();
        if v_count == 0 {
            return None;
        }

        let weights = vec![vec![1.0; v_count]; u_count];
        let knots_u = Self::uniform_knot_vector(u_count, degree_u);
        let knots_v = Self::uniform_knot_vector(v_count, degree_v);

        Self::new(
            control_points,
            weights,
            knots_u,
            knots_v,
            degree_u,
            degree_v,
        )
    }

    /// Generate a uniform knot vector.
    fn uniform_knot_vector(n_points: usize, degree: usize) -> Vec<f64> {
        let n_knots = n_points + degree + 1;
        let mut knots = Vec::with_capacity(n_knots);

        // Clamped knot vector
        knots.extend(std::iter::repeat_n(0.0_f64, degree + 1));

        let internal_knots = n_points - degree;
        for i in 1..internal_knots {
            knots.push(i as f64 / internal_knots as f64);
        }

        knots.extend(std::iter::repeat_n(1.0_f64, degree + 1));

        knots
    }

    /// Number of control points in U direction.
    pub fn u_count(&self) -> usize {
        self.control_points.len()
    }

    /// Number of control points in V direction.
    pub fn v_count(&self) -> usize {
        self.control_points[0].len()
    }

    /// Degree in U direction.
    pub fn degree_u(&self) -> usize {
        self.degree_u
    }

    /// Degree in V direction.
    pub fn degree_v(&self) -> usize {
        self.degree_v
    }

    /// Get the U parameter domain.
    pub fn domain_u(&self) -> (f64, f64) {
        (self.knots_u[self.degree_u], self.knots_u[self.u_count()])
    }

    /// Get the V parameter domain.
    pub fn domain_v(&self) -> (f64, f64) {
        (self.knots_v[self.degree_v], self.knots_v[self.v_count()])
    }

    /// Evaluate the surface at parameter (u, v).
    pub fn evaluate(&self, u: f64, v: f64) -> Point3<f64> {
        let (u_min, u_max) = self.domain_u();
        let (v_min, v_max) = self.domain_v();
        let u = u.clamp(u_min, u_max);
        let v = v.clamp(v_min, v_max);

        // First, evaluate in V direction for each U row
        let mut temp_points = Vec::with_capacity(self.u_count());
        let mut temp_weights = Vec::with_capacity(self.u_count());

        for i in 0..self.u_count() {
            let (pt, w) = self.de_boor_v(i, v);
            temp_points.push(pt);
            temp_weights.push(w);
        }

        // Then evaluate in U direction
        self.de_boor_u(&temp_points, &temp_weights, u)
    }

    /// De Boor's algorithm in V direction for a single U row.
    fn de_boor_v(&self, u_index: usize, v: f64) -> (Point3<f64>, f64) {
        let p = self.degree_v;
        let k = self.find_span_v(v);

        let mut d: Vec<(Point3<f64>, f64)> = Vec::with_capacity(p + 1);
        for j in 0..=p {
            let idx = k - p + j;
            let pt = self.control_points[u_index][idx];
            let w = self.weights[u_index][idx];
            d.push((Point3::new(pt.x * w, pt.y * w, pt.z * w), w));
        }

        for r in 1..=p {
            for j in (r..=p).rev() {
                let idx = k - p + j;
                let alpha =
                    (v - self.knots_v[idx]) / (self.knots_v[idx + p + 1 - r] - self.knots_v[idx]);
                let (pt0, w0) = d[j - 1];
                let (pt1, w1) = d[j];
                let x = pt0.x * (1.0 - alpha) + pt1.x * alpha;
                let y = pt0.y * (1.0 - alpha) + pt1.y * alpha;
                let z = pt0.z * (1.0 - alpha) + pt1.z * alpha;
                let w = w0 * (1.0 - alpha) + w1 * alpha;
                d[j] = (Point3::new(x, y, z), w);
            }
        }

        let (pt, w) = d[p];
        (Point3::new(pt.x / w, pt.y / w, pt.z / w), w)
    }

    /// De Boor's algorithm in U direction.
    fn de_boor_u(&self, points: &[Point3<f64>], weights: &[f64], u: f64) -> Point3<f64> {
        let p = self.degree_u;
        let k = self.find_span_u(u);

        let mut d: Vec<(Point3<f64>, f64)> = Vec::with_capacity(p + 1);
        for i in 0..=p {
            let idx = k - p + i;
            let pt = points[idx];
            let w = weights[idx];
            d.push((Point3::new(pt.x * w, pt.y * w, pt.z * w), w));
        }

        for r in 1..=p {
            for i in (r..=p).rev() {
                let idx = k - p + i;
                let alpha =
                    (u - self.knots_u[idx]) / (self.knots_u[idx + p + 1 - r] - self.knots_u[idx]);
                let (pt0, w0) = d[i - 1];
                let (pt1, w1) = d[i];
                let x = pt0.x * (1.0 - alpha) + pt1.x * alpha;
                let y = pt0.y * (1.0 - alpha) + pt1.y * alpha;
                let z = pt0.z * (1.0 - alpha) + pt1.z * alpha;
                let w = w0 * (1.0 - alpha) + w1 * alpha;
                d[i] = (Point3::new(x, y, z), w);
            }
        }

        let (pt, w) = d[p];
        Point3::new(pt.x / w, pt.y / w, pt.z / w)
    }

    /// Find knot span in U direction.
    fn find_span_u(&self, u: f64) -> usize {
        let n = self.u_count() - 1;
        if u >= self.knots_u[n + 1] {
            return n;
        }
        if u <= self.knots_u[self.degree_u] {
            return self.degree_u;
        }

        let mut low = self.degree_u;
        let mut high = n + 1;
        let mut mid = (low + high) / 2;

        while u < self.knots_u[mid] || u >= self.knots_u[mid + 1] {
            if u < self.knots_u[mid] {
                high = mid;
            } else {
                low = mid;
            }
            mid = (low + high) / 2;
        }

        mid
    }

    /// Find knot span in V direction.
    fn find_span_v(&self, v: f64) -> usize {
        let n = self.v_count() - 1;
        if v >= self.knots_v[n + 1] {
            return n;
        }
        if v <= self.knots_v[self.degree_v] {
            return self.degree_v;
        }

        let mut low = self.degree_v;
        let mut high = n + 1;
        let mut mid = (low + high) / 2;

        while v < self.knots_v[mid] || v >= self.knots_v[mid + 1] {
            if v < self.knots_v[mid] {
                high = mid;
            } else {
                low = mid;
            }
            mid = (low + high) / 2;
        }

        mid
    }

    /// Evaluate surface point with derivatives.
    pub fn evaluate_with_derivatives(&self, u: f64, v: f64) -> SurfacePoint {
        let h = 1e-6;
        let position = self.evaluate(u, v);

        let (u_min, u_max) = self.domain_u();
        let (v_min, v_max) = self.domain_v();

        // Compute du using central difference
        let du = if u + h <= u_max && u - h >= u_min {
            let p1 = self.evaluate(u + h, v);
            let p0 = self.evaluate(u - h, v);
            (p1 - p0) / (2.0 * h)
        } else if u + h <= u_max {
            let p1 = self.evaluate(u + h, v);
            (p1 - position) / h
        } else {
            let p0 = self.evaluate(u - h, v);
            (position - p0) / h
        };

        // Compute dv using central difference
        let dv = if v + h <= v_max && v - h >= v_min {
            let p1 = self.evaluate(u, v + h);
            let p0 = self.evaluate(u, v - h);
            (p1 - p0) / (2.0 * h)
        } else if v + h <= v_max {
            let p1 = self.evaluate(u, v + h);
            (p1 - position) / h
        } else {
            let p0 = self.evaluate(u, v - h);
            (position - p0) / h
        };

        // Normal is cross product of tangents
        let normal = du.cross(&dv).normalize();

        SurfacePoint {
            position,
            du,
            dv,
            normal,
        }
    }

    /// Extract an isocurve from the surface.
    pub fn isocurve(&self, param: f64, direction: IsoDirection) -> Vec<Point3<f64>> {
        match direction {
            IsoDirection::U => {
                // Constant U, varying V
                let v_count = self.v_count();
                let mut points = Vec::with_capacity(v_count);
                for j in 0..v_count {
                    let v = self.knots_v[self.degree_v]
                        + (self.knots_v[v_count] - self.knots_v[self.degree_v]) * j as f64
                            / (v_count - 1) as f64;
                    points.push(self.evaluate(param, v));
                }
                points
            }
            IsoDirection::V => {
                // Constant V, varying U
                let u_count = self.u_count();
                let mut points = Vec::with_capacity(u_count);
                for i in 0..u_count {
                    let u = self.knots_u[self.degree_u]
                        + (self.knots_u[u_count] - self.knots_u[self.degree_u]) * i as f64
                            / (u_count - 1) as f64;
                    points.push(self.evaluate(u, param));
                }
                points
            }
        }
    }

    /// Tessellate the surface into a triangle mesh.
    pub fn tessellate(&self, u_segments: usize, v_segments: usize) -> SurfaceMesh {
        let (u_min, u_max) = self.domain_u();
        let (v_min, v_max) = self.domain_v();

        let mut vertices = Vec::with_capacity((u_segments + 1) * (v_segments + 1));
        let mut normals = Vec::with_capacity((u_segments + 1) * (v_segments + 1));
        let mut uvs = Vec::with_capacity((u_segments + 1) * (v_segments + 1));
        let mut indices = Vec::with_capacity(u_segments * v_segments * 6);

        // Generate vertices
        for i in 0..=u_segments {
            let u = u_min + (u_max - u_min) * i as f64 / u_segments as f64;
            for j in 0..=v_segments {
                let v = v_min + (v_max - v_min) * j as f64 / v_segments as f64;
                let sp = self.evaluate_with_derivatives(u, v);
                vertices.push(sp.position);
                normals.push(sp.normal);
                uvs.push((i as f32 / u_segments as f32, j as f32 / v_segments as f32));
            }
        }

        // Generate indices
        for i in 0..u_segments {
            for j in 0..v_segments {
                let base = i * (v_segments + 1) + j;
                let next_row = base + v_segments + 1;

                // First triangle
                indices.push(base as u32);
                indices.push((base + 1) as u32);
                indices.push(next_row as u32);

                // Second triangle
                indices.push((base + 1) as u32);
                indices.push((next_row + 1) as u32);
                indices.push(next_row as u32);
            }
        }

        SurfaceMesh {
            vertices,
            normals,
            uvs,
            indices,
        }
    }

    /// Get control point at (u_index, v_index).
    pub fn control_point(&self, u_index: usize, v_index: usize) -> Point3<f64> {
        self.control_points[u_index][v_index]
    }

    /// Set control point at (u_index, v_index).
    pub fn set_control_point(&mut self, u_index: usize, v_index: usize, point: Point3<f64>) {
        self.control_points[u_index][v_index] = point;
    }

    /// Get weight at (u_index, v_index).
    pub fn weight(&self, u_index: usize, v_index: usize) -> f64 {
        self.weights[u_index][v_index]
    }

    /// Set weight at (u_index, v_index).
    pub fn set_weight(&mut self, u_index: usize, v_index: usize, weight: f64) {
        self.weights[u_index][v_index] = weight;
    }

    /// Insert a knot in U direction.
    pub fn insert_knot_u(&mut self, u: f64) {
        let (u_min, u_max) = self.domain_u();
        if u <= u_min || u >= u_max {
            return;
        }

        let k = self.find_span_u(u);
        let p = self.degree_u;

        // Insert knot
        self.knots_u.insert(k + 1, u);

        // Update control points
        let mut new_points: Vec<Vec<Point3<f64>>> = Vec::with_capacity(self.u_count() + 1);
        let mut new_weights: Vec<Vec<f64>> = Vec::with_capacity(self.u_count() + 1);

        for j in 0..self.v_count() {
            let mut row_points = Vec::with_capacity(self.u_count() + 1);
            let mut row_weights = Vec::with_capacity(self.u_count() + 1);

            for i in 0..=self.u_count() {
                if i <= k - p {
                    if j == 0 {
                        row_points.push(self.control_points[i][j]);
                        row_weights.push(self.weights[i][j]);
                    } else {
                        row_points.push(new_points[i][j]);
                        row_weights.push(new_weights[i][j]);
                    }
                } else if i > k {
                    row_points.push(self.control_points[i - 1][j]);
                    row_weights.push(self.weights[i - 1][j]);
                } else {
                    let alpha = (u - self.knots_u[i]) / (self.knots_u[i + p] - self.knots_u[i]);
                    let w0 = self.weights[i - 1][j];
                    let w1 = self.weights[i][j];
                    let p0 = self.control_points[i - 1][j];
                    let p1 = self.control_points[i][j];

                    let new_w = w0 * (1.0 - alpha) + w1 * alpha;
                    let new_p = Point3::new(
                        (p0.x * w0 * (1.0 - alpha) + p1.x * w1 * alpha) / new_w,
                        (p0.y * w0 * (1.0 - alpha) + p1.y * w1 * alpha) / new_w,
                        (p0.z * w0 * (1.0 - alpha) + p1.z * w1 * alpha) / new_w,
                    );
                    row_points.push(new_p);
                    row_weights.push(new_w);
                }
            }

            if j == 0 {
                for pt in row_points {
                    new_points.push(vec![pt]);
                }
                for w in row_weights {
                    new_weights.push(vec![w]);
                }
            } else {
                for (i, (pt, w)) in row_points.into_iter().zip(row_weights).enumerate() {
                    new_points[i].push(pt);
                    new_weights[i].push(w);
                }
            }
        }

        self.control_points = new_points;
        self.weights = new_weights;
    }

    /// Insert a knot in V direction.
    pub fn insert_knot_v(&mut self, v: f64) {
        let (v_min, v_max) = self.domain_v();
        if v <= v_min || v >= v_max {
            return;
        }

        let k = self.find_span_v(v);
        let p = self.degree_v;

        // Insert knot
        self.knots_v.insert(k + 1, v);

        // Update control points
        for i in 0..self.u_count() {
            let mut new_row = Vec::with_capacity(self.v_count() + 1);
            let mut new_weights_row = Vec::with_capacity(self.v_count() + 1);

            for j in 0..=self.v_count() {
                if j <= k - p {
                    new_row.push(self.control_points[i][j]);
                    new_weights_row.push(self.weights[i][j]);
                } else if j > k {
                    new_row.push(self.control_points[i][j - 1]);
                    new_weights_row.push(self.weights[i][j - 1]);
                } else {
                    let alpha = (v - self.knots_v[j]) / (self.knots_v[j + p] - self.knots_v[j]);
                    let w0 = self.weights[i][j - 1];
                    let w1 = self.weights[i][j];
                    let p0 = self.control_points[i][j - 1];
                    let p1 = self.control_points[i][j];

                    let new_w = w0 * (1.0 - alpha) + w1 * alpha;
                    let new_p = Point3::new(
                        (p0.x * w0 * (1.0 - alpha) + p1.x * w1 * alpha) / new_w,
                        (p0.y * w0 * (1.0 - alpha) + p1.y * w1 * alpha) / new_w,
                        (p0.z * w0 * (1.0 - alpha) + p1.z * w1 * alpha) / new_w,
                    );
                    new_row.push(new_p);
                    new_weights_row.push(new_w);
                }
            }

            self.control_points[i] = new_row;
            self.weights[i] = new_weights_row;
        }
    }

    /// Create a bilinear patch (degree 1x1).
    pub fn bilinear(corners: [[Point3<f64>; 2]; 2]) -> Self {
        let control_points = vec![
            vec![corners[0][0], corners[0][1]],
            vec![corners[1][0], corners[1][1]],
        ];
        let weights = vec![vec![1.0, 1.0], vec![1.0, 1.0]];
        let knots_u = vec![0.0, 0.0, 1.0, 1.0];
        let knots_v = vec![0.0, 0.0, 1.0, 1.0];

        Self {
            control_points,
            weights,
            knots_u,
            knots_v,
            degree_u: 1,
            degree_v: 1,
        }
    }

    /// Create a plane surface.
    pub fn plane(
        origin: Point3<f64>,
        u_dir: Vector3<f64>,
        v_dir: Vector3<f64>,
        u_size: f64,
        v_size: f64,
    ) -> Self {
        let p00 = origin;
        let p01 = origin + v_dir * v_size;
        let p10 = origin + u_dir * u_size;
        let p11 = origin + u_dir * u_size + v_dir * v_size;

        Self::bilinear([[p00, p01], [p10, p11]])
    }

    /// Create a cylinder surface.
    pub fn cylinder(center: Point3<f64>, axis: Vector3<f64>, radius: f64, height: f64) -> Self {
        use std::f64::consts::PI;

        let axis = axis.normalize();

        // Find perpendicular vectors
        let u = if axis.x.abs() < 0.9 {
            Vector3::new(1.0, 0.0, 0.0).cross(&axis).normalize()
        } else {
            Vector3::new(0.0, 1.0, 0.0).cross(&axis).normalize()
        };
        let v = axis.cross(&u);

        // Create 9x2 control point grid for a full cylinder
        let angles = [
            0.0,
            PI / 4.0,
            PI / 2.0,
            3.0 * PI / 4.0,
            PI,
            5.0 * PI / 4.0,
            3.0 * PI / 2.0,
            7.0 * PI / 4.0,
            2.0 * PI,
        ];
        let weights_row = [
            1.0,
            (2.0_f64).sqrt() / 2.0,
            1.0,
            (2.0_f64).sqrt() / 2.0,
            1.0,
            (2.0_f64).sqrt() / 2.0,
            1.0,
            (2.0_f64).sqrt() / 2.0,
            1.0,
        ];

        let mut control_points = Vec::with_capacity(9);
        let mut weights = Vec::with_capacity(9);

        for (i, &angle) in angles.iter().enumerate() {
            let cos_a = angle.cos();
            let sin_a = angle.sin();
            let offset = u * (cos_a * radius) + v * (sin_a * radius);

            let p0 = center + offset;
            let p1 = center + offset + axis * height;

            control_points.push(vec![p0, p1]);
            weights.push(vec![weights_row[i], weights_row[i]]);
        }

        let knots_u = vec![
            0.0, 0.0, 0.0, 0.25, 0.25, 0.5, 0.5, 0.75, 0.75, 1.0, 1.0, 1.0,
        ];
        let knots_v = vec![0.0, 0.0, 1.0, 1.0];

        Self {
            control_points,
            weights,
            knots_u,
            knots_v,
            degree_u: 2,
            degree_v: 1,
        }
    }

    /// Create a sphere surface.
    pub fn sphere(center: Point3<f64>, radius: f64) -> Self {
        use std::f64::consts::PI;

        let w_diag = (2.0_f64).sqrt() / 2.0;

        // 9x5 control point grid
        let mut control_points = Vec::with_capacity(9);
        let mut weights = Vec::with_capacity(9);

        let theta_angles = [
            0.0,
            PI / 4.0,
            PI / 2.0,
            3.0 * PI / 4.0,
            PI,
            5.0 * PI / 4.0,
            3.0 * PI / 2.0,
            7.0 * PI / 4.0,
            2.0 * PI,
        ];
        let phi_angles = [-PI / 2.0, -PI / 4.0, 0.0, PI / 4.0, PI / 2.0];
        let w_theta = [1.0, w_diag, 1.0, w_diag, 1.0, w_diag, 1.0, w_diag, 1.0];
        let w_phi = [1.0, w_diag, 1.0, w_diag, 1.0];

        for (i, &theta) in theta_angles.iter().enumerate() {
            let mut row = Vec::with_capacity(5);
            let mut w_row = Vec::with_capacity(5);

            let cos_theta = theta.cos();
            let sin_theta = theta.sin();

            for (j, &phi) in phi_angles.iter().enumerate() {
                let cos_phi = phi.cos();
                let sin_phi = phi.sin();

                // For poles, project to axis
                let r = if j == 0 || j == 4 {
                    radius
                } else {
                    radius / cos_phi.abs().max(0.001)
                };

                let x = center.x + r * cos_phi * cos_theta;
                let y = center.y + r * cos_phi * sin_theta;
                let z = center.z + r * sin_phi;

                row.push(Point3::new(x, y, z));
                w_row.push(w_theta[i] * w_phi[j]);
            }

            control_points.push(row);
            weights.push(w_row);
        }

        let knots_u = vec![
            0.0, 0.0, 0.0, 0.25, 0.25, 0.5, 0.5, 0.75, 0.75, 1.0, 1.0, 1.0,
        ];
        let knots_v = vec![0.0, 0.0, 0.0, 0.5, 0.5, 1.0, 1.0, 1.0];

        Self {
            control_points,
            weights,
            knots_u,
            knots_v,
            degree_u: 2,
            degree_v: 2,
        }
    }

    /// Compute surface area using numerical integration.
    pub fn area(&self, u_samples: usize, v_samples: usize) -> f64 {
        let (u_min, u_max) = self.domain_u();
        let (v_min, v_max) = self.domain_v();

        let du = (u_max - u_min) / u_samples as f64;
        let dv = (v_max - v_min) / v_samples as f64;

        let mut total_area = 0.0;

        for i in 0..u_samples {
            let u = u_min + (i as f64 + 0.5) * du;
            for j in 0..v_samples {
                let v = v_min + (j as f64 + 0.5) * dv;
                let sp = self.evaluate_with_derivatives(u, v);
                let area_element = sp.du.cross(&sp.dv).norm() * du * dv;
                total_area += area_element;
            }
        }

        total_area
    }

    /// Find closest point on surface to a given point.
    pub fn closest_point(
        &self,
        point: Point3<f64>,
        u_samples: usize,
        v_samples: usize,
    ) -> (f64, f64, Point3<f64>) {
        let (u_min, u_max) = self.domain_u();
        let (v_min, v_max) = self.domain_v();

        let mut best_u = u_min;
        let mut best_v = v_min;
        let mut best_dist_sq = f64::MAX;
        let mut best_point = self.evaluate(u_min, v_min);

        // Initial grid search
        for i in 0..=u_samples {
            let u = u_min + (u_max - u_min) * i as f64 / u_samples as f64;
            for j in 0..=v_samples {
                let v = v_min + (v_max - v_min) * j as f64 / v_samples as f64;
                let surf_pt = self.evaluate(u, v);
                let dist_sq = (surf_pt - point).norm_squared();
                if dist_sq < best_dist_sq {
                    best_dist_sq = dist_sq;
                    best_u = u;
                    best_v = v;
                    best_point = surf_pt;
                }
            }
        }

        // Newton-Raphson refinement
        for _ in 0..5 {
            let sp = self.evaluate_with_derivatives(best_u, best_v);
            let diff = sp.position - point;

            // Compute gradient
            let grad_u = diff.dot(&sp.du);
            let grad_v = diff.dot(&sp.dv);

            if grad_u.abs() < 1e-10 && grad_v.abs() < 1e-10 {
                break;
            }

            // Simple gradient descent step
            let step = 0.1;
            let new_u = (best_u - step * grad_u).clamp(u_min, u_max);
            let new_v = (best_v - step * grad_v).clamp(v_min, v_max);

            let new_point = self.evaluate(new_u, new_v);
            let new_dist_sq = (new_point - point).norm_squared();

            if new_dist_sq < best_dist_sq {
                best_u = new_u;
                best_v = new_v;
                best_dist_sq = new_dist_sq;
                best_point = new_point;
            }
        }

        (best_u, best_v, best_point)
    }
}

/// Tessellated surface mesh.
#[derive(Debug, Clone)]
pub struct SurfaceMesh {
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
    fn test_bilinear_patch() {
        let corners = [
            [Point3::new(0.0, 0.0, 0.0), Point3::new(0.0, 1.0, 0.0)],
            [Point3::new(1.0, 0.0, 0.0), Point3::new(1.0, 1.0, 0.0)],
        ];
        let surface = NurbsSurface::bilinear(corners);

        let center = surface.evaluate(0.5, 0.5);
        assert!((center.x - 0.5).abs() < 1e-10);
        assert!((center.y - 0.5).abs() < 1e-10);
        assert!(center.z.abs() < 1e-10);
    }

    #[test]
    fn test_plane_surface() {
        let origin = Point3::new(0.0, 0.0, 0.0);
        let u_dir = Vector3::new(1.0, 0.0, 0.0);
        let v_dir = Vector3::new(0.0, 1.0, 0.0);
        let surface = NurbsSurface::plane(origin, u_dir, v_dir, 2.0, 3.0);

        let pt = surface.evaluate(0.5, 0.5);
        assert!((pt.x - 1.0).abs() < 1e-10);
        assert!((pt.y - 1.5).abs() < 1e-10);
    }

    #[test]
    fn test_surface_normal() {
        let origin = Point3::new(0.0, 0.0, 0.0);
        let u_dir = Vector3::new(1.0, 0.0, 0.0);
        let v_dir = Vector3::new(0.0, 1.0, 0.0);
        let surface = NurbsSurface::plane(origin, u_dir, v_dir, 1.0, 1.0);

        let sp = surface.evaluate_with_derivatives(0.5, 0.5);
        assert!((sp.normal.z - 1.0).abs() < 0.01 || (sp.normal.z + 1.0).abs() < 0.01);
    }

    #[test]
    fn test_tessellation() {
        let corners = [
            [Point3::new(0.0, 0.0, 0.0), Point3::new(0.0, 1.0, 0.0)],
            [Point3::new(1.0, 0.0, 0.0), Point3::new(1.0, 1.0, 0.0)],
        ];
        let surface = NurbsSurface::bilinear(corners);
        let mesh = surface.tessellate(4, 4);

        assert_eq!(mesh.vertices.len(), 25);
        assert_eq!(mesh.indices.len(), 96);
    }

    #[test]
    fn test_isocurve() {
        let corners = [
            [Point3::new(0.0, 0.0, 0.0), Point3::new(0.0, 1.0, 0.0)],
            [Point3::new(1.0, 0.0, 0.0), Point3::new(1.0, 1.0, 0.0)],
        ];
        let surface = NurbsSurface::bilinear(corners);

        let iso = surface.isocurve(0.5, IsoDirection::U);
        assert_eq!(iso.len(), 2);
    }
}
