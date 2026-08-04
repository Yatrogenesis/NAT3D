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

//! NURBS (Non-Uniform Rational B-Splines) curves.
//!
//! Provides parametric curve representation with control points and weights.

use nalgebra::{Point3, Vector3};

/// NURBS curve.
#[derive(Debug, Clone)]
pub struct NurbsCurve {
    /// Curve degree.
    pub degree: usize,
    /// Control points.
    control_points: Vec<Point3<f64>>,
    /// Weights for rational curves.
    weights: Vec<f64>,
    /// Knot vector.
    knots: Vec<f64>,
    /// Is curve closed/periodic.
    pub is_closed: bool,
}

impl NurbsCurve {
    /// Create a new NURBS curve.
    pub fn new(degree: usize) -> Self {
        Self {
            degree,
            control_points: Vec::new(),
            weights: Vec::new(),
            knots: Vec::new(),
            is_closed: false,
        }
    }

    /// Create from control points (uniform B-spline).
    pub fn from_points(degree: usize, points: Vec<Point3<f64>>) -> Self {
        let n = points.len();
        let weights = vec![1.0; n];

        // Create uniform knot vector
        let knots = Self::create_uniform_knots(n, degree);

        Self {
            degree,
            control_points: points,
            weights,
            knots,
            is_closed: false,
        }
    }

    /// Create a clamped (open) uniform knot vector.
    fn create_uniform_knots(n: usize, degree: usize) -> Vec<f64> {
        let m = n + degree + 1;
        let mut knots = Vec::with_capacity(m);

        for i in 0..m {
            if i <= degree {
                knots.push(0.0);
            } else if i >= m - degree - 1 {
                knots.push(1.0);
            } else {
                knots.push((i - degree) as f64 / (n - degree) as f64);
            }
        }

        knots
    }

    /// Get control point count.
    pub fn control_point_count(&self) -> usize {
        self.control_points.len()
    }

    /// Get control points.
    pub fn control_points(&self) -> &[Point3<f64>] {
        &self.control_points
    }

    /// Get mutable control points.
    pub fn control_points_mut(&mut self) -> &mut Vec<Point3<f64>> {
        &mut self.control_points
    }

    /// Get weights.
    pub fn weights(&self) -> &[f64] {
        &self.weights
    }

    /// Set weight for a control point.
    pub fn set_weight(&mut self, index: usize, weight: f64) {
        if index < self.weights.len() {
            self.weights[index] = weight.max(0.001); // Prevent zero/negative weights
        }
    }

    /// Get knot vector.
    pub fn knots(&self) -> &[f64] {
        &self.knots
    }

    /// Set knot vector.
    pub fn set_knots(&mut self, knots: Vec<f64>) {
        self.knots = knots;
    }

    /// Add a control point.
    pub fn add_control_point(&mut self, point: Point3<f64>, weight: f64) {
        self.control_points.push(point);
        self.weights.push(weight.max(0.001));

        // Rebuild knot vector
        self.knots = Self::create_uniform_knots(self.control_points.len(), self.degree);
    }

    /// Insert a control point at index.
    pub fn insert_control_point(&mut self, index: usize, point: Point3<f64>, weight: f64) {
        let index = index.min(self.control_points.len());
        self.control_points.insert(index, point);
        self.weights.insert(index, weight.max(0.001));
        self.knots = Self::create_uniform_knots(self.control_points.len(), self.degree);
    }

    /// Remove a control point.
    pub fn remove_control_point(&mut self, index: usize) -> Option<(Point3<f64>, f64)> {
        if index < self.control_points.len() && self.control_points.len() > self.degree + 1 {
            let point = self.control_points.remove(index);
            let weight = self.weights.remove(index);
            self.knots = Self::create_uniform_knots(self.control_points.len(), self.degree);
            Some((point, weight))
        } else {
            None
        }
    }

    /// Get parameter range.
    pub fn parameter_range(&self) -> (f64, f64) {
        if self.knots.is_empty() {
            return (0.0, 1.0);
        }
        (
            self.knots[self.degree],
            self.knots[self.knots.len() - self.degree - 1],
        )
    }

    /// Evaluate curve at parameter t.
    pub fn evaluate(&self, t: f64) -> Point3<f64> {
        if self.control_points.len() <= self.degree {
            return self
                .control_points
                .first()
                .copied()
                .unwrap_or(Point3::origin());
        }

        let (t_min, t_max) = self.parameter_range();
        let t = t.clamp(t_min, t_max);

        // Use de Boor's algorithm for NURBS
        self.de_boor(t)
    }

    /// de Boor's algorithm for NURBS evaluation.
    fn de_boor(&self, t: f64) -> Point3<f64> {
        let n = self.control_points.len();
        let p = self.degree;

        // Find knot span
        let k = self.find_knot_span(t);

        // Calculate basis functions
        let mut d: Vec<(Point3<f64>, f64)> = Vec::with_capacity(p + 1);

        for i in 0..=p {
            let idx = k - p + i;
            if idx < n {
                let cp = self.control_points[idx];
                let w = self.weights[idx];
                // Homogeneous coordinates
                d.push((Point3::new(cp.x * w, cp.y * w, cp.z * w), w));
            }
        }

        // de Boor recursion
        for r in 1..=p {
            for j in (r..=p).rev() {
                let i = k - p + j;
                let alpha = (t - self.knots[i]) / (self.knots[i + p + 1 - r] - self.knots[i]);

                let (p0, w0) = d[j - 1];
                let (p1, w1) = d[j];

                let new_w = (1.0 - alpha) * w0 + alpha * w1;
                let new_p = Point3::from(p0.coords * (1.0 - alpha) + p1.coords * alpha);

                d[j] = (new_p, new_w);
            }
        }

        let (result, w) = d[p];
        if w.abs() > 1e-10 {
            Point3::new(result.x / w, result.y / w, result.z / w)
        } else {
            result
        }
    }

    /// Find knot span index.
    fn find_knot_span(&self, t: f64) -> usize {
        let n = self.control_points.len();
        let p = self.degree;

        // Handle boundary cases
        if t >= self.knots[n] {
            return n - 1;
        }
        if t <= self.knots[p] {
            return p;
        }

        // Binary search
        let mut low = p;
        let mut high = n;

        while low < high {
            let mid = (low + high) / 2;
            if t < self.knots[mid] {
                high = mid;
            } else if t >= self.knots[mid + 1] {
                low = mid + 1;
            } else {
                return mid;
            }
        }

        low
    }

    /// Evaluate derivative at parameter t.
    pub fn derivative(&self, t: f64) -> Vector3<f64> {
        let epsilon = 0.0001;
        let (t_min, t_max) = self.parameter_range();

        let t0 = (t - epsilon).max(t_min);
        let t1 = (t + epsilon).min(t_max);
        let dt = t1 - t0;

        if dt > 1e-10 {
            let p0 = self.evaluate(t0);
            let p1 = self.evaluate(t1);
            (p1 - p0) / dt
        } else {
            Vector3::zeros()
        }
    }

    /// Evaluate second derivative at parameter t.
    pub fn second_derivative(&self, t: f64) -> Vector3<f64> {
        let epsilon = 0.0001;
        let (t_min, t_max) = self.parameter_range();

        let t0 = (t - epsilon).max(t_min);
        let t1 = (t + epsilon).min(t_max);

        let d0 = self.derivative(t0);
        let d1 = self.derivative(t1);
        let dt = t1 - t0;

        if dt > 1e-10 {
            (d1 - d0) / dt
        } else {
            Vector3::zeros()
        }
    }

    /// Get tangent vector at parameter t.
    pub fn tangent(&self, t: f64) -> Vector3<f64> {
        self.derivative(t).normalize()
    }

    /// Get normal vector at parameter t.
    pub fn normal(&self, t: f64) -> Vector3<f64> {
        let tangent = self.tangent(t);
        let second = self.second_derivative(t);

        // Normal is perpendicular to tangent
        let cross = tangent.cross(&second);
        if cross.magnitude() > 1e-10 {
            cross.cross(&tangent).normalize()
        } else {
            // Fallback: find any perpendicular vector
            let up = if tangent.y.abs() < 0.9 {
                Vector3::new(0.0, 1.0, 0.0)
            } else {
                Vector3::new(1.0, 0.0, 0.0)
            };
            tangent.cross(&up).normalize()
        }
    }

    /// Get binormal vector at parameter t.
    pub fn binormal(&self, t: f64) -> Vector3<f64> {
        self.tangent(t).cross(&self.normal(t))
    }

    /// Get curvature at parameter t.
    pub fn curvature(&self, t: f64) -> f64 {
        let d1 = self.derivative(t);
        let d2 = self.second_derivative(t);

        let cross = d1.cross(&d2);
        let d1_mag = d1.magnitude();

        if d1_mag > 1e-10 {
            cross.magnitude() / d1_mag.powi(3)
        } else {
            0.0
        }
    }

    /// Calculate arc length from t0 to t1.
    pub fn arc_length(&self, t0: f64, t1: f64, segments: usize) -> f64 {
        let mut length = 0.0;
        let dt = (t1 - t0) / segments as f64;

        let mut prev_point = self.evaluate(t0);
        for i in 1..=segments {
            let t = t0 + i as f64 * dt;
            let point = self.evaluate(t);
            length += (point - prev_point).magnitude();
            prev_point = point;
        }

        length
    }

    /// Find parameter for given arc length.
    pub fn parameter_at_length(&self, length: f64, segments: usize) -> f64 {
        let (t_min, t_max) = self.parameter_range();
        let total_length = self.arc_length(t_min, t_max, segments);

        if length <= 0.0 {
            return t_min;
        }
        if length >= total_length {
            return t_max;
        }

        // Binary search
        let mut low = t_min;
        let mut high = t_max;

        for _ in 0..20 {
            let mid = (low + high) / 2.0;
            let mid_length = self.arc_length(t_min, mid, segments);

            if mid_length < length {
                low = mid;
            } else {
                high = mid;
            }
        }

        (low + high) / 2.0
    }

    /// Sample curve uniformly by parameter.
    pub fn sample_uniform(&self, count: usize) -> Vec<Point3<f64>> {
        let (t_min, t_max) = self.parameter_range();
        let mut samples = Vec::with_capacity(count);

        for i in 0..count {
            let t = t_min + (t_max - t_min) * (i as f64 / (count - 1).max(1) as f64);
            samples.push(self.evaluate(t));
        }

        samples
    }

    /// Sample curve uniformly by arc length.
    pub fn sample_arc_length(&self, count: usize) -> Vec<Point3<f64>> {
        let (t_min, t_max) = self.parameter_range();
        let total_length = self.arc_length(t_min, t_max, count * 10);
        let mut samples = Vec::with_capacity(count);

        for i in 0..count {
            let target_length = total_length * (i as f64 / (count - 1).max(1) as f64);
            let t = self.parameter_at_length(target_length, count * 10);
            samples.push(self.evaluate(t));
        }

        samples
    }

    /// Insert a knot (degree elevation).
    pub fn insert_knot(&mut self, t: f64) {
        let k = self.find_knot_span(t);
        let p = self.degree;
        let n = self.control_points.len();

        // Create new control points
        let mut new_points = Vec::with_capacity(n + 1);
        let mut new_weights = Vec::with_capacity(n + 1);

        for i in 0..=n {
            if i <= k - p {
                new_points.push(self.control_points[i]);
                new_weights.push(self.weights[i]);
            } else if i > k {
                new_points.push(self.control_points[i - 1]);
                new_weights.push(self.weights[i - 1]);
            } else {
                let alpha = (t - self.knots[i]) / (self.knots[i + p] - self.knots[i]);
                let p_new = Point3::from(
                    self.control_points[i - 1].coords * (1.0 - alpha)
                        + self.control_points[i].coords * alpha,
                );
                let w_new = self.weights[i - 1] * (1.0 - alpha) + self.weights[i] * alpha;
                new_points.push(p_new);
                new_weights.push(w_new);
            }
        }

        // Insert knot
        let mut new_knots = self.knots.clone();
        new_knots.insert(k + 1, t);

        self.control_points = new_points;
        self.weights = new_weights;
        self.knots = new_knots;
    }

    /// Split curve at parameter t.
    pub fn split(&self, t: f64) -> (NurbsCurve, NurbsCurve) {
        let mut curve = self.clone();

        // Insert knot degree+1 times to split
        for _ in 0..=self.degree {
            curve.insert_knot(t);
        }

        let split_index = curve.find_knot_span(t);

        // Create left curve
        let left_points: Vec<_> = curve.control_points[..=split_index - self.degree].to_vec();
        let left_weights: Vec<_> = curve.weights[..=split_index - self.degree].to_vec();
        let left_knots: Vec<_> = curve.knots[..=split_index + 1].to_vec();

        // Create right curve
        let right_points: Vec<_> = curve.control_points[split_index - self.degree..].to_vec();
        let right_weights: Vec<_> = curve.weights[split_index - self.degree..].to_vec();
        let right_knots: Vec<_> = curve.knots[split_index..].to_vec();

        let mut left = NurbsCurve::new(self.degree);
        left.control_points = left_points;
        left.weights = left_weights;
        left.knots = left_knots;

        let mut right = NurbsCurve::new(self.degree);
        right.control_points = right_points;
        right.weights = right_weights;
        right.knots = right_knots;

        (left, right)
    }

    /// Create a line.
    pub fn line(start: Point3<f64>, end: Point3<f64>) -> Self {
        Self::from_points(1, vec![start, end])
    }

    /// Create a circle (approximation with 4 arcs).
    pub fn circle(center: Point3<f64>, radius: f64, normal: Vector3<f64>) -> Self {
        let normal = normal.normalize();

        // Find perpendicular vectors
        let up = if normal.y.abs() < 0.9 {
            Vector3::new(0.0, 1.0, 0.0)
        } else {
            Vector3::new(1.0, 0.0, 0.0)
        };
        let u = normal.cross(&up).normalize();
        let v = u.cross(&normal);

        // Control points for circular NURBS (degree 2)
        let w = (2.0_f64).sqrt() / 2.0;

        let points = vec![
            center + u * radius,
            center + u * radius + v * radius,
            center + v * radius,
            center - u * radius + v * radius,
            center - u * radius,
            center - u * radius - v * radius,
            center - v * radius,
            center + u * radius - v * radius,
            center + u * radius, // Close the circle
        ];

        let weights = vec![1.0, w, 1.0, w, 1.0, w, 1.0, w, 1.0];
        let knots = vec![
            0.0, 0.0, 0.0, 0.25, 0.25, 0.5, 0.5, 0.75, 0.75, 1.0, 1.0, 1.0,
        ];

        let mut curve = NurbsCurve::new(2);
        curve.control_points = points;
        curve.weights = weights;
        curve.knots = knots;
        curve.is_closed = true;
        curve
    }

    /// Create an arc.
    pub fn arc(
        center: Point3<f64>,
        radius: f64,
        start_angle: f64,
        end_angle: f64,
        normal: Vector3<f64>,
    ) -> Self {
        let normal = normal.normalize();
        let up = if normal.y.abs() < 0.9 {
            Vector3::new(0.0, 1.0, 0.0)
        } else {
            Vector3::new(1.0, 0.0, 0.0)
        };
        let u = normal.cross(&up).normalize();
        let v = u.cross(&normal);

        let sweep = end_angle - start_angle;
        let half_sweep = sweep / 2.0;
        let w = half_sweep.cos();

        let start_dir = u * start_angle.cos() + v * start_angle.sin();
        let mid_angle = (start_angle + end_angle) / 2.0;
        let mid_dir = u * mid_angle.cos() + v * mid_angle.sin();
        let end_dir = u * end_angle.cos() + v * end_angle.sin();

        let start_point = center + start_dir * radius;
        let end_point = center + end_dir * radius;
        let mid_point = center + mid_dir * (radius / half_sweep.cos());

        let mut curve = NurbsCurve::new(2);
        curve.control_points = vec![start_point, mid_point, end_point];
        curve.weights = vec![1.0, w, 1.0];
        curve.knots = vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
        curve
    }
}

impl Default for NurbsCurve {
    fn default() -> Self {
        Self::new(3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_curve_creation() {
        let curve = NurbsCurve::from_points(
            3,
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 1.0, 0.0),
                Point3::new(2.0, 0.0, 0.0),
                Point3::new(3.0, 1.0, 0.0),
            ],
        );

        assert_eq!(curve.control_point_count(), 4);
        assert_eq!(curve.degree, 3);
    }

    #[test]
    fn test_curve_evaluation() {
        let curve = NurbsCurve::from_points(
            2,
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 1.0, 0.0),
                Point3::new(2.0, 0.0, 0.0),
            ],
        );

        // Start and end should match control points
        let start = curve.evaluate(0.0);
        let end = curve.evaluate(1.0);

        assert!((start - Point3::new(0.0, 0.0, 0.0)).magnitude() < 1e-6);
        assert!((end - Point3::new(2.0, 0.0, 0.0)).magnitude() < 1e-6);
    }

    #[test]
    fn test_line() {
        let line = NurbsCurve::line(Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 1.0, 1.0));

        let mid = line.evaluate(0.5);
        assert!((mid - Point3::new(0.5, 0.5, 0.5)).magnitude() < 1e-6);
    }

    #[test]
    fn test_circle() {
        let circle = NurbsCurve::circle(Point3::origin(), 1.0, Vector3::new(0.0, 0.0, 1.0));

        // All points should be at radius distance
        for t in [0.0, 0.25, 0.5, 0.75] {
            let point = circle.evaluate(t);
            let dist = point.coords.magnitude();
            assert!((dist - 1.0).abs() < 1e-3);
        }
    }

    #[test]
    fn test_derivative() {
        let curve = NurbsCurve::from_points(
            2,
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(2.0, 0.0, 0.0),
            ],
        );

        let tangent = curve.tangent(0.5);
        // Should point in X direction
        assert!(tangent.x > 0.9);
    }

    #[test]
    fn test_arc_length() {
        let line = NurbsCurve::line(Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 0.0, 0.0));

        let length = line.arc_length(0.0, 1.0, 100);
        assert!((length - 1.0).abs() < 1e-3);
    }
}
