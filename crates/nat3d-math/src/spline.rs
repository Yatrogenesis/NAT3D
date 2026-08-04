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

//! Spline evaluation for curves and surfaces.
//!
//! B-spline and NURBS evaluation utilities.

use nalgebra::{Point3, Vector3};

/// B-spline basis function (Cox-de Boor recursion).
#[must_use]
pub fn bspline_basis(i: usize, degree: usize, t: f64, knots: &[f64]) -> f64 {
    if degree == 0 {
        if knots[i] <= t && t < knots[i + 1] {
            return 1.0;
        }
        // Handle endpoint: t equals the last knot value and this is the last real span
        let max_knot = knots[knots.len() - 1];
        if (t - max_knot).abs() < 1e-10
            && (knots[i + 1] - max_knot).abs() < 1e-10
            && knots[i] < max_knot - 1e-10
        {
            return 1.0;
        }
        return 0.0;
    }

    let mut result = 0.0;

    let denom1 = knots[i + degree] - knots[i];
    if denom1.abs() > 1e-10 {
        let coeff1 = (t - knots[i]) / denom1;
        result += coeff1 * bspline_basis(i, degree - 1, t, knots);
    }

    let denom2 = knots[i + degree + 1] - knots[i + 1];
    if denom2.abs() > 1e-10 {
        let coeff2 = (knots[i + degree + 1] - t) / denom2;
        result += coeff2 * bspline_basis(i + 1, degree - 1, t, knots);
    }

    result
}

/// B-spline basis function derivative.
#[must_use]
pub fn bspline_basis_derivative(i: usize, degree: usize, t: f64, knots: &[f64]) -> f64 {
    if degree == 0 {
        return 0.0;
    }

    let mut result = 0.0;

    let denom1 = knots[i + degree] - knots[i];
    if denom1.abs() > 1e-10 {
        result += (degree as f64 / denom1) * bspline_basis(i, degree - 1, t, knots);
    }

    let denom2 = knots[i + degree + 1] - knots[i + 1];
    if denom2.abs() > 1e-10 {
        result -= (degree as f64 / denom2) * bspline_basis(i + 1, degree - 1, t, knots);
    }

    result
}

/// Generate uniform knot vector.
#[must_use]
pub fn uniform_knots(n: usize, degree: usize) -> Vec<f64> {
    let knot_count = n + degree + 1;
    (0..knot_count)
        .map(|i| i as f64 / (knot_count - 1) as f64)
        .collect()
}

/// Generate clamped (open) knot vector.
#[must_use]
#[allow(clippy::same_item_push)] // Intentional: clamped knot vectors require repeated 0.0/1.0 values
pub fn clamped_knots(n: usize, degree: usize) -> Vec<f64> {
    let knot_count = n + degree + 1;
    let mut knots = Vec::with_capacity(knot_count);

    knots.extend(std::iter::repeat(0.0).take(degree + 1));

    let internal = n - degree;
    for i in 1..internal {
        knots.push(i as f64 / internal as f64);
    }

    knots.extend(std::iter::repeat(1.0).take(degree + 1));

    knots
}

/// B-spline curve.
#[derive(Debug, Clone)]
pub struct BSplineCurve {
    /// Control points.
    pub control_points: Vec<Point3<f64>>,
    /// Knot vector.
    pub knots: Vec<f64>,
    /// Degree.
    pub degree: usize,
}

impl BSplineCurve {
    /// Create new B-spline curve.
    pub fn new(control_points: Vec<Point3<f64>>, degree: usize) -> Self {
        let knots = clamped_knots(control_points.len(), degree);
        Self {
            control_points,
            knots,
            degree,
        }
    }

    /// Create with custom knots.
    pub fn with_knots(control_points: Vec<Point3<f64>>, knots: Vec<f64>, degree: usize) -> Self {
        Self {
            control_points,
            knots,
            degree,
        }
    }

    /// Evaluate curve at parameter t in [0, 1].
    #[must_use]
    pub fn evaluate(&self, t: f64) -> Point3<f64> {
        let t = t.clamp(0.0, 1.0);
        let t_mapped = self.knots[self.degree]
            + t * (self.knots[self.control_points.len()] - self.knots[self.degree]);

        let mut result = Vector3::zeros();

        for i in 0..self.control_points.len() {
            let basis = bspline_basis(i, self.degree, t_mapped, &self.knots);
            result += self.control_points[i].coords * basis;
        }

        Point3::from(result)
    }

    /// Evaluate curve derivative at parameter t.
    #[must_use]
    pub fn derivative(&self, t: f64) -> Vector3<f64> {
        let t = t.clamp(0.0, 1.0);
        let t_mapped = self.knots[self.degree]
            + t * (self.knots[self.control_points.len()] - self.knots[self.degree]);
        let dt = self.knots[self.control_points.len()] - self.knots[self.degree];

        let mut result = Vector3::zeros();

        for i in 0..self.control_points.len() {
            let basis_deriv = bspline_basis_derivative(i, self.degree, t_mapped, &self.knots);
            result += self.control_points[i].coords * basis_deriv * dt;
        }

        result
    }

    /// Get tangent at parameter t.
    #[must_use]
    pub fn tangent(&self, t: f64) -> Vector3<f64> {
        self.derivative(t).normalize()
    }

    /// Sample curve at n points.
    pub fn sample(&self, n: usize) -> Vec<Point3<f64>> {
        (0..n)
            .map(|i| {
                let t = i as f64 / (n - 1) as f64;
                self.evaluate(t)
            })
            .collect()
    }

    /// Approximate arc length.
    #[must_use]
    pub fn arc_length(&self, segments: usize) -> f64 {
        let mut length = 0.0;
        let mut prev = self.evaluate(0.0);

        for i in 1..=segments {
            let t = i as f64 / segments as f64;
            let curr = self.evaluate(t);
            length += (curr - prev).magnitude();
            prev = curr;
        }

        length
    }

    /// Insert a knot (knot insertion algorithm).
    pub fn insert_knot(&mut self, t: f64) {
        let t_mapped = self.knots[self.degree]
            + t * (self.knots[self.control_points.len()] - self.knots[self.degree]);

        // Find knot span
        let mut k = self.degree;
        for i in self.degree..self.control_points.len() {
            if t_mapped >= self.knots[i] && t_mapped < self.knots[i + 1] {
                k = i;
                break;
            }
        }

        // Calculate new control points
        let mut new_points = Vec::with_capacity(self.control_points.len() + 1);

        for i in 0..=k - self.degree {
            new_points.push(self.control_points[i]);
        }

        for i in (k - self.degree + 1)..=k {
            let alpha = (t_mapped - self.knots[i]) / (self.knots[i + self.degree] - self.knots[i]);
            let p = Point3::from(
                self.control_points[i - 1].coords * (1.0 - alpha)
                    + self.control_points[i].coords * alpha,
            );
            new_points.push(p);
        }

        for i in k..self.control_points.len() {
            new_points.push(self.control_points[i]);
        }

        // Insert knot
        let mut insert_idx = self.degree + 1;
        for i in self.degree + 1..self.knots.len() {
            if t_mapped < self.knots[i] {
                insert_idx = i;
                break;
            }
        }

        self.knots.insert(insert_idx, t_mapped);
        self.control_points = new_points;
    }
}

/// NURBS curve (rational B-spline).
#[derive(Debug, Clone)]
pub struct NurbsCurve {
    /// Control points.
    pub control_points: Vec<Point3<f64>>,
    /// Weights.
    pub weights: Vec<f64>,
    /// Knot vector.
    pub knots: Vec<f64>,
    /// Degree.
    pub degree: usize,
}

impl NurbsCurve {
    /// Create new NURBS curve with uniform weights.
    pub fn new(control_points: Vec<Point3<f64>>, degree: usize) -> Self {
        let n = control_points.len();
        let weights = vec![1.0; n];
        let knots = clamped_knots(n, degree);
        Self {
            control_points,
            weights,
            knots,
            degree,
        }
    }

    /// Create with custom weights.
    pub fn with_weights(
        control_points: Vec<Point3<f64>>,
        weights: Vec<f64>,
        degree: usize,
    ) -> Self {
        let knots = clamped_knots(control_points.len(), degree);
        Self {
            control_points,
            weights,
            knots,
            degree,
        }
    }

    /// Evaluate curve at parameter t.
    #[must_use]
    pub fn evaluate(&self, t: f64) -> Point3<f64> {
        let t = t.clamp(0.0, 1.0);
        let t_mapped = self.knots[self.degree]
            + t * (self.knots[self.control_points.len()] - self.knots[self.degree]);

        let mut numerator = Vector3::zeros();
        let mut denominator = 0.0;

        for i in 0..self.control_points.len() {
            let basis = bspline_basis(i, self.degree, t_mapped, &self.knots);
            let weighted_basis = basis * self.weights[i];
            numerator += self.control_points[i].coords * weighted_basis;
            denominator += weighted_basis;
        }

        if denominator.abs() < 1e-10 {
            return self.control_points[0];
        }

        Point3::from(numerator / denominator)
    }

    /// Evaluate derivative.
    #[must_use]
    pub fn derivative(&self, t: f64) -> Vector3<f64> {
        let t = t.clamp(0.0, 1.0);
        let t_mapped = self.knots[self.degree]
            + t * (self.knots[self.control_points.len()] - self.knots[self.degree]);
        let dt = self.knots[self.control_points.len()] - self.knots[self.degree];

        let mut a = Vector3::zeros();
        let mut w = 0.0;
        let mut a_prime = Vector3::zeros();
        let mut w_prime = 0.0;

        for i in 0..self.control_points.len() {
            let basis = bspline_basis(i, self.degree, t_mapped, &self.knots);
            let basis_deriv = bspline_basis_derivative(i, self.degree, t_mapped, &self.knots) * dt;

            let wb = basis * self.weights[i];
            let wb_prime = basis_deriv * self.weights[i];

            a += self.control_points[i].coords * wb;
            w += wb;
            a_prime += self.control_points[i].coords * wb_prime;
            w_prime += wb_prime;
        }

        if w.abs() < 1e-10 {
            return Vector3::zeros();
        }

        (a_prime * w - a * w_prime) / (w * w)
    }

    /// Sample curve.
    pub fn sample(&self, n: usize) -> Vec<Point3<f64>> {
        (0..n)
            .map(|i| {
                let t = i as f64 / (n - 1) as f64;
                self.evaluate(t)
            })
            .collect()
    }

    /// Create circle arc.
    pub fn circle_arc(center: Point3<f64>, radius: f64, start_angle: f64, end_angle: f64) -> Self {
        let angle = end_angle - start_angle;
        let segments = ((angle.abs() / std::f64::consts::FRAC_PI_2).ceil() as usize).max(1);

        let mut control_points = Vec::new();
        let mut weights = Vec::new();

        let segment_angle = angle / segments as f64;
        let w = (segment_angle / 2.0).cos();

        for seg in 0..=segments {
            let a = start_angle + seg as f64 * segment_angle;

            if seg > 0 {
                // Add middle control point
                let mid_angle = a - segment_angle / 2.0;
                let mid_radius = radius / (segment_angle / 2.0).cos();
                control_points.push(Point3::new(
                    center.x + mid_radius * mid_angle.cos(),
                    center.y + mid_radius * mid_angle.sin(),
                    center.z,
                ));
                weights.push(w);
            }

            // Add endpoint
            control_points.push(Point3::new(
                center.x + radius * a.cos(),
                center.y + radius * a.sin(),
                center.z,
            ));
            weights.push(1.0);
        }

        // Build proper knot vector for piecewise quadratic NURBS arcs
        // Needs double knots at segment boundaries for C1 continuity
        let mut knots = vec![0.0, 0.0, 0.0]; // degree+1 zeros at start
        for i in 1..segments {
            let knot = i as f64 / segments as f64;
            knots.push(knot);
            knots.push(knot); // double interior knots
        }
        knots.push(1.0);
        knots.push(1.0);
        knots.push(1.0); // degree+1 ones at end

        Self {
            control_points,
            weights,
            knots,
            degree: 2,
        }
    }
}

/// Bezier curve.
#[derive(Debug, Clone)]
pub struct BezierCurve {
    /// Control points.
    pub control_points: Vec<Point3<f64>>,
}

impl BezierCurve {
    /// Create new Bezier curve.
    pub fn new(control_points: Vec<Point3<f64>>) -> Self {
        Self { control_points }
    }

    /// Evaluate using de Casteljau's algorithm.
    #[must_use]
    pub fn evaluate(&self, t: f64) -> Point3<f64> {
        let n = self.control_points.len();
        if n == 0 {
            return Point3::origin();
        }
        if n == 1 {
            return self.control_points[0];
        }

        let mut points: Vec<Point3<f64>> = self.control_points.clone();

        for r in 1..n {
            for i in 0..n - r {
                points[i] = Point3::from(points[i].coords * (1.0 - t) + points[i + 1].coords * t);
            }
        }

        points[0]
    }

    /// Evaluate derivative.
    #[must_use]
    pub fn derivative(&self, t: f64) -> Vector3<f64> {
        let n = self.control_points.len();
        if n < 2 {
            return Vector3::zeros();
        }

        // Derivative is degree * (difference of control points curve)
        let mut deriv_points = Vec::with_capacity(n - 1);
        for i in 0..n - 1 {
            deriv_points.push(Point3::from(
                (self.control_points[i + 1] - self.control_points[i]) * (n - 1) as f64,
            ));
        }

        let deriv_curve = BezierCurve::new(deriv_points);
        deriv_curve.evaluate(t).coords
    }

    /// Split curve at parameter t.
    pub fn split(&self, t: f64) -> (BezierCurve, BezierCurve) {
        let n = self.control_points.len();
        let mut left = Vec::with_capacity(n);
        let mut right = Vec::with_capacity(n);

        let mut points = self.control_points.clone();
        left.push(points[0]);
        right.push(points[n - 1]);

        for r in 1..n {
            for i in 0..n - r {
                points[i] = Point3::from(points[i].coords * (1.0 - t) + points[i + 1].coords * t);
            }
            left.push(points[0]);
            right.push(points[n - r - 1]);
        }

        right.reverse();
        (BezierCurve::new(left), BezierCurve::new(right))
    }

    /// Sample curve.
    pub fn sample(&self, n: usize) -> Vec<Point3<f64>> {
        (0..n)
            .map(|i| {
                let t = i as f64 / (n - 1) as f64;
                self.evaluate(t)
            })
            .collect()
    }
}

/// Hermite spline segment.
#[derive(Debug, Clone)]
pub struct HermiteSpline {
    /// Points.
    pub points: Vec<Point3<f64>>,
    /// Tangents.
    pub tangents: Vec<Vector3<f64>>,
}

impl HermiteSpline {
    /// Create from points and tangents.
    pub fn new(points: Vec<Point3<f64>>, tangents: Vec<Vector3<f64>>) -> Self {
        Self { points, tangents }
    }

    /// Create with auto-calculated tangents (Catmull-Rom style).
    pub fn catmull_rom(points: Vec<Point3<f64>>) -> Self {
        let n = points.len();
        let mut tangents = Vec::with_capacity(n);

        for i in 0..n {
            let tangent = if i == 0 {
                (points[1] - points[0]) * 0.5
            } else if i == n - 1 {
                (points[n - 1] - points[n - 2]) * 0.5
            } else {
                (points[i + 1] - points[i - 1]) * 0.5
            };
            tangents.push(tangent);
        }

        Self { points, tangents }
    }

    /// Evaluate at parameter t in [0, n-1].
    #[must_use]
    pub fn evaluate(&self, t: f64) -> Point3<f64> {
        let n = self.points.len();
        if n < 2 {
            return self.points.first().copied().unwrap_or_else(Point3::origin);
        }

        let t = t.clamp(0.0, (n - 1) as f64);
        let segment = (t.floor() as usize).min(n - 2);
        let local_t = t - segment as f64;

        let p0 = self.points[segment];
        let p1 = self.points[segment + 1];
        let m0 = self.tangents[segment];
        let m1 = self.tangents[segment + 1];

        let t2 = local_t * local_t;
        let t3 = t2 * local_t;

        let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
        let h10 = t3 - 2.0 * t2 + local_t;
        let h01 = -2.0 * t3 + 3.0 * t2;
        let h11 = t3 - t2;

        Point3::from(p0.coords * h00 + m0 * h10 + p1.coords * h01 + m1 * h11)
    }

    /// Sample spline.
    pub fn sample(&self, points_per_segment: usize) -> Vec<Point3<f64>> {
        let n = self.points.len();
        if n < 2 {
            return self.points.clone();
        }

        let total = (n - 1) * points_per_segment + 1;
        (0..total)
            .map(|i| {
                let t = i as f64 / points_per_segment as f64;
                self.evaluate(t)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bspline_endpoints() {
        let points = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
            Point3::new(3.0, 1.0, 0.0),
        ];

        let curve = BSplineCurve::new(points.clone(), 3);

        let start = curve.evaluate(0.0);
        let end = curve.evaluate(1.0);

        assert!((start.x - points[0].x).abs() < 1e-6);
        assert!((end.x - points[3].x).abs() < 1e-6);
    }

    #[test]
    fn test_nurbs_circle() {
        use std::f64::consts::PI;

        let circle = NurbsCurve::circle_arc(Point3::origin(), 1.0, 0.0, 2.0 * PI);

        // Points should be on unit circle
        for i in 0..10 {
            let t = i as f64 / 10.0;
            let p = circle.evaluate(t);
            let dist = (p.x * p.x + p.y * p.y).sqrt();
            assert!((dist - 1.0).abs() < 0.1);
        }
    }

    #[test]
    fn test_bezier_split() {
        let points = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(2.0, 1.0, 0.0),
            Point3::new(3.0, 0.0, 0.0),
        ];

        let curve = BezierCurve::new(points);
        let mid = curve.evaluate(0.5);
        let (left, right) = curve.split(0.5);

        let left_end = left.evaluate(1.0);
        let right_start = right.evaluate(0.0);

        assert!((left_end - mid).magnitude() < 1e-6);
        assert!((right_start - mid).magnitude() < 1e-6);
    }

    #[test]
    fn test_hermite_catmull_rom() {
        let points = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
            Point3::new(3.0, 1.0, 0.0),
        ];

        let spline = HermiteSpline::catmull_rom(points.clone());

        // Should pass through control points
        for (i, p) in points.iter().enumerate() {
            let eval = spline.evaluate(i as f64);
            assert!((eval - p).magnitude() < 1e-6);
        }
    }
}
