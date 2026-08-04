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

//! Animation curves and F-Curves.
//!
//! Provides Bezier-based animation curves with handles for precise control.

use std::collections::BTreeMap;

/// Bezier handle type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HandleType {
    /// Free handles (independent in/out).
    Free,
    /// Aligned handles (same direction, different lengths).
    Aligned,
    /// Vector handles (pointing to neighbors).
    Vector,
    /// Auto handles (smooth automatic).
    #[default]
    Auto,
    /// Auto-clamped (auto but doesn't overshoot).
    AutoClamped,
}

/// A control point on an animation curve.
#[derive(Debug, Clone, Copy)]
pub struct CurvePoint {
    /// Time (X coordinate).
    pub time: f64,
    /// Value (Y coordinate).
    pub value: f64,
    /// Left handle type.
    pub left_handle_type: HandleType,
    /// Right handle type.
    pub right_handle_type: HandleType,
    /// Left handle position (relative to point).
    pub left_handle: (f64, f64),
    /// Right handle position (relative to point).
    pub right_handle: (f64, f64),
    /// Is point selected.
    pub selected: bool,
}

impl CurvePoint {
    /// Create a new curve point.
    pub fn new(time: f64, value: f64) -> Self {
        Self {
            time,
            value,
            left_handle_type: HandleType::Auto,
            right_handle_type: HandleType::Auto,
            left_handle: (-0.3, 0.0),
            right_handle: (0.3, 0.0),
            selected: false,
        }
    }

    /// Create with specific value.
    pub fn at(time: f64, value: f64) -> Self {
        Self::new(time, value)
    }

    /// Set handle type for both handles.
    pub fn with_handle_type(mut self, handle_type: HandleType) -> Self {
        self.left_handle_type = handle_type;
        self.right_handle_type = handle_type;
        self
    }

    /// Get absolute left handle position.
    pub fn left_handle_abs(&self) -> (f64, f64) {
        (
            self.time + self.left_handle.0,
            self.value + self.left_handle.1,
        )
    }

    /// Get absolute right handle position.
    pub fn right_handle_abs(&self) -> (f64, f64) {
        (
            self.time + self.right_handle.0,
            self.value + self.right_handle.1,
        )
    }

    /// Set left handle from absolute position.
    pub fn set_left_handle_abs(&mut self, x: f64, y: f64) {
        self.left_handle = (x - self.time, y - self.value);
    }

    /// Set right handle from absolute position.
    pub fn set_right_handle_abs(&mut self, x: f64, y: f64) {
        self.right_handle = (x - self.time, y - self.value);
    }

    /// Align handles (make them opposite).
    pub fn align_handles(&mut self) {
        let left_len = (self.left_handle.0.powi(2) + self.left_handle.1.powi(2)).sqrt();
        let right_len = (self.right_handle.0.powi(2) + self.right_handle.1.powi(2)).sqrt();

        if right_len > 1e-10 {
            let scale = left_len / right_len;
            self.left_handle = (-self.right_handle.0 * scale, -self.right_handle.1 * scale);
        }
    }
}

/// Extrapolation mode beyond curve range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Extrapolation {
    /// Hold constant value.
    #[default]
    Constant,
    /// Linear extrapolation.
    Linear,
    /// Repeat the curve.
    Cyclic,
    /// Repeat with offset.
    CyclicOffset,
    /// Mirror/ping-pong.
    Mirror,
}

/// Animation curve (F-Curve).
#[derive(Debug, Clone)]
pub struct AnimationCurve {
    /// Curve name/path.
    pub name: String,
    /// Array index (for vector properties).
    pub array_index: i32,
    /// Control points sorted by time.
    points: BTreeMap<ordered_float::OrderedFloat<f64>, CurvePoint>,
    /// Extrapolation before first point.
    pub pre_extrapolation: Extrapolation,
    /// Extrapolation after last point.
    pub post_extrapolation: Extrapolation,
    /// Curve color for UI.
    pub color: [f64; 3],
    /// Is curve muted.
    pub muted: bool,
    /// Is curve locked.
    pub locked: bool,
}

impl AnimationCurve {
    /// Create a new animation curve.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            array_index: 0,
            points: BTreeMap::new(),
            pre_extrapolation: Extrapolation::Constant,
            post_extrapolation: Extrapolation::Constant,
            color: [1.0, 1.0, 1.0],
            muted: false,
            locked: false,
        }
    }

    /// Add a control point.
    pub fn add_point(&mut self, point: CurvePoint) {
        self.points
            .insert(ordered_float::OrderedFloat(point.time), point);
        self.auto_handle_recalc();
    }

    /// Remove a control point.
    pub fn remove_point(&mut self, time: f64) -> Option<CurvePoint> {
        let result = self.points.remove(&ordered_float::OrderedFloat(time));
        self.auto_handle_recalc();
        result
    }

    /// Get point at time.
    pub fn get_point(&self, time: f64) -> Option<&CurvePoint> {
        self.points.get(&ordered_float::OrderedFloat(time))
    }

    /// Get mutable point at time.
    pub fn get_point_mut(&mut self, time: f64) -> Option<&mut CurvePoint> {
        self.points.get_mut(&ordered_float::OrderedFloat(time))
    }

    /// Get all points.
    pub fn points(&self) -> impl Iterator<Item = &CurvePoint> {
        self.points.values()
    }

    /// Get point count.
    pub fn point_count(&self) -> usize {
        self.points.len()
    }

    /// Get time range.
    pub fn time_range(&self) -> Option<(f64, f64)> {
        let first = self.points.keys().next()?;
        let last = self.points.keys().next_back()?;
        Some((first.0, last.0))
    }

    /// Get value range.
    pub fn value_range(&self) -> Option<(f64, f64)> {
        if self.points.is_empty() {
            return None;
        }

        let mut min = f64::MAX;
        let mut max = f64::MIN;

        for point in self.points.values() {
            min = min.min(point.value);
            max = max.max(point.value);
        }

        Some((min, max))
    }

    /// Evaluate curve at time.
    pub fn evaluate(&self, time: f64) -> f64 {
        if self.points.is_empty() {
            return 0.0;
        }

        if self.muted {
            return 0.0;
        }

        let (start, end) = match self.time_range() {
            Some(range) => range,
            None => return 0.0,
        };

        // Handle extrapolation
        if time < start {
            return self.extrapolate_before(time, start, end);
        }
        if time > end {
            return self.extrapolate_after(time, start, end);
        }

        self.evaluate_internal(time)
    }

    /// Evaluate within curve range.
    fn evaluate_internal(&self, time: f64) -> f64 {
        let time_key = ordered_float::OrderedFloat(time);

        // Exact match
        if let Some(point) = self.points.get(&time_key) {
            return point.value;
        }

        // Find surrounding points
        let before = self.points.range(..time_key).next_back();
        let after = self.points.range(time_key..).next();

        match (before, after) {
            (Some((_, p1)), Some((_, p2))) => self.bezier_interpolate(p1, p2, time),
            (Some((_, p)), None) => p.value,
            (None, Some((_, p))) => p.value,
            (None, None) => 0.0,
        }
    }

    /// Cubic Bezier interpolation between two points.
    fn bezier_interpolate(&self, p1: &CurvePoint, p2: &CurvePoint, time: f64) -> f64 {
        let dt = p2.time - p1.time;
        if dt.abs() < 1e-10 {
            return p1.value;
        }

        // Normalize time to [0, 1]
        let t = (time - p1.time) / dt;

        // Control points for cubic Bezier
        let x0 = p1.time;
        let y0 = p1.value;
        let (hx1, hy1) = p1.right_handle_abs();
        let (hx2, hy2) = p2.left_handle_abs();
        let x3 = p2.time;
        let y3 = p2.value;

        // Find t parameter for given x (time) using Newton's method
        let mut u = t;
        for _ in 0..10 {
            let xu = cubic_bezier(x0, hx1, hx2, x3, u);
            let error = xu - time;
            if error.abs() < 1e-10 {
                break;
            }
            let dxu = cubic_bezier_derivative(x0, hx1, hx2, x3, u);
            if dxu.abs() > 1e-10 {
                u -= error / dxu;
                u = u.clamp(0.0, 1.0);
            }
        }

        // Evaluate y at found parameter
        cubic_bezier(y0, hy1, hy2, y3, u)
    }

    /// Extrapolate before first point.
    fn extrapolate_before(&self, time: f64, start: f64, end: f64) -> f64 {
        let first = self.points.values().next().unwrap();

        match self.pre_extrapolation {
            Extrapolation::Constant => first.value,
            Extrapolation::Linear => {
                // Use derivative at first point
                let derivative = self.derivative_at(start);
                first.value + derivative * (time - start)
            }
            Extrapolation::Cyclic => {
                let duration = end - start;
                if duration <= 0.0 {
                    return first.value;
                }
                let offset = (start - time) % duration;
                self.evaluate_internal(end - offset)
            }
            Extrapolation::CyclicOffset => {
                let duration = end - start;
                if duration <= 0.0 {
                    return first.value;
                }
                let cycles = ((start - time) / duration).ceil() as i64;
                let offset = (start - time) % duration;
                let base = self.evaluate_internal(end - offset);
                let last = self.points.values().next_back().unwrap();
                base - cycles as f64 * (last.value - first.value)
            }
            Extrapolation::Mirror => {
                let duration = end - start;
                if duration <= 0.0 {
                    return first.value;
                }
                let offset = (start - time) % (duration * 2.0);
                if offset < duration {
                    self.evaluate_internal(start + offset)
                } else {
                    self.evaluate_internal(end - (offset - duration))
                }
            }
        }
    }

    /// Extrapolate after last point.
    fn extrapolate_after(&self, time: f64, start: f64, end: f64) -> f64 {
        let last = self.points.values().next_back().unwrap();

        match self.post_extrapolation {
            Extrapolation::Constant => last.value,
            Extrapolation::Linear => {
                let derivative = self.derivative_at(end);
                last.value + derivative * (time - end)
            }
            Extrapolation::Cyclic => {
                let duration = end - start;
                if duration <= 0.0 {
                    return last.value;
                }
                let offset = (time - end) % duration;
                self.evaluate_internal(start + offset)
            }
            Extrapolation::CyclicOffset => {
                let duration = end - start;
                if duration <= 0.0 {
                    return last.value;
                }
                let cycles = ((time - end) / duration).ceil() as i64;
                let offset = (time - end) % duration;
                let base = self.evaluate_internal(start + offset);
                let first = self.points.values().next().unwrap();
                base + cycles as f64 * (last.value - first.value)
            }
            Extrapolation::Mirror => {
                let duration = end - start;
                if duration <= 0.0 {
                    return last.value;
                }
                let offset = (time - end) % (duration * 2.0);
                if offset < duration {
                    self.evaluate_internal(end - offset)
                } else {
                    self.evaluate_internal(start + (offset - duration))
                }
            }
        }
    }

    /// Get derivative at time.
    pub fn derivative_at(&self, time: f64) -> f64 {
        let epsilon = 0.001;
        let v1 = self.evaluate_internal(
            (time - epsilon).max(self.time_range().map(|r| r.0).unwrap_or(time)),
        );
        let v2 = self.evaluate_internal(
            (time + epsilon).min(self.time_range().map(|r| r.1).unwrap_or(time)),
        );
        (v2 - v1) / (2.0 * epsilon)
    }

    /// Recalculate auto handles.
    fn auto_handle_recalc(&mut self) {
        // Collect all data we need first
        let _point_data: Vec<(f64, f64, f64, HandleType, HandleType)> = self
            .points
            .values()
            .map(|p| {
                (
                    p.time,
                    p.value,
                    p.time,
                    p.left_handle_type,
                    p.right_handle_type,
                )
            })
            .collect();

        let times: Vec<f64> = self.points.keys().map(|k| k.0).collect();

        // Calculate handle updates
        type HandleUpdate = (f64, Option<(f64, f64)>, Option<(f64, f64)>);
        let mut updates: Vec<HandleUpdate> = Vec::new();

        for i in 0..times.len() {
            let time = times[i];
            let prev_data = if i > 0 {
                self.points
                    .get(&ordered_float::OrderedFloat(times[i - 1]))
                    .map(|p| (p.time, p.value))
            } else {
                None
            };
            let next_data = if i < times.len() - 1 {
                self.points
                    .get(&ordered_float::OrderedFloat(times[i + 1]))
                    .map(|p| (p.time, p.value))
            } else {
                None
            };

            if let Some(point) = self.points.get(&ordered_float::OrderedFloat(time)) {
                let mut left_update = None;
                let mut right_update = None;

                if point.left_handle_type == HandleType::Auto
                    || point.left_handle_type == HandleType::AutoClamped
                {
                    left_update = Some(Self::compute_auto_left_handle(
                        point.time,
                        point.value,
                        prev_data,
                        next_data,
                    ));
                }
                if point.right_handle_type == HandleType::Auto
                    || point.right_handle_type == HandleType::AutoClamped
                {
                    right_update = Some(Self::compute_auto_right_handle(
                        point.time,
                        point.value,
                        prev_data,
                        next_data,
                    ));
                }

                if left_update.is_some() || right_update.is_some() {
                    updates.push((time, left_update, right_update));
                }
            }
        }

        // Apply updates
        for (time, left, right) in updates {
            if let Some(point) = self.points.get_mut(&ordered_float::OrderedFloat(time)) {
                if let Some(l) = left {
                    point.left_handle = l;
                }
                if let Some(r) = right {
                    point.right_handle = r;
                }
            }
        }
    }

    fn compute_auto_left_handle(
        time: f64,
        value: f64,
        prev: Option<(f64, f64)>,
        next: Option<(f64, f64)>,
    ) -> (f64, f64) {
        match (prev, next) {
            (Some((pt, pv)), Some((nt, nv))) => {
                let slope = (nv - pv) / (nt - pt);
                let dx = (time - pt) / 3.0;
                (-dx, -slope * dx)
            }
            (Some((pt, pv)), None) => {
                let slope = (value - pv) / (time - pt);
                let dx = (time - pt) / 3.0;
                (-dx, -slope * dx)
            }
            _ => (-0.3, 0.0),
        }
    }

    fn compute_auto_right_handle(
        time: f64,
        value: f64,
        prev: Option<(f64, f64)>,
        next: Option<(f64, f64)>,
    ) -> (f64, f64) {
        match (prev, next) {
            (Some((pt, pv)), Some((nt, nv))) => {
                let slope = (nv - pv) / (nt - pt);
                let dx = (nt - time) / 3.0;
                (dx, slope * dx)
            }
            (None, Some((nt, nv))) => {
                let slope = (nv - value) / (nt - time);
                let dx = (nt - time) / 3.0;
                (dx, slope * dx)
            }
            _ => (0.3, 0.0),
        }
    }

    /// Bake curve to samples.
    pub fn bake(&self, start: f64, end: f64, sample_rate: f64) -> Vec<(f64, f64)> {
        let mut samples = Vec::new();
        let mut time = start;

        while time <= end {
            samples.push((time, self.evaluate(time)));
            time += 1.0 / sample_rate;
        }

        samples
    }
}

impl Default for AnimationCurve {
    fn default() -> Self {
        Self::new("")
    }
}

/// Cubic Bezier evaluation.
fn cubic_bezier(p0: f64, p1: f64, p2: f64, p3: f64, t: f64) -> f64 {
    let mt = 1.0 - t;
    let mt2 = mt * mt;
    let mt3 = mt2 * mt;
    let t2 = t * t;
    let t3 = t2 * t;

    mt3 * p0 + 3.0 * mt2 * t * p1 + 3.0 * mt * t2 * p2 + t3 * p3
}

/// Cubic Bezier derivative.
fn cubic_bezier_derivative(p0: f64, p1: f64, p2: f64, p3: f64, t: f64) -> f64 {
    let mt = 1.0 - t;
    let mt2 = mt * mt;
    let t2 = t * t;

    3.0 * mt2 * (p1 - p0) + 6.0 * mt * t * (p2 - p1) + 3.0 * t2 * (p3 - p2)
}

/// Curve modifier.
#[derive(Debug, Clone)]
pub enum CurveModifier {
    /// Add noise.
    Noise {
        scale: f64,
        strength: f64,
        phase: f64,
    },
    /// Envelope.
    Envelope { min: f64, max: f64 },
    /// Cycles.
    Cycles { before: i32, after: i32 },
    /// Stepped.
    Stepped { step_size: f64 },
    /// Limits.
    Limits { min: f64, max: f64 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_curve_creation() {
        let mut curve = AnimationCurve::new("test");
        curve.add_point(CurvePoint::new(0.0, 0.0));
        curve.add_point(CurvePoint::new(1.0, 1.0));

        assert_eq!(curve.point_count(), 2);
    }

    #[test]
    fn test_curve_evaluation() {
        let mut curve = AnimationCurve::new("test");
        curve.add_point(CurvePoint::new(0.0, 0.0).with_handle_type(HandleType::Vector));
        curve.add_point(CurvePoint::new(1.0, 1.0).with_handle_type(HandleType::Vector));

        // At endpoints
        assert!((curve.evaluate(0.0) - 0.0).abs() < 1e-6);
        assert!((curve.evaluate(1.0) - 1.0).abs() < 1e-6);

        // Midpoint should be around 0.5 for linear-ish curve
        let mid = curve.evaluate(0.5);
        assert!(mid > 0.3 && mid < 0.7);
    }

    #[test]
    fn test_extrapolation_constant() {
        let mut curve = AnimationCurve::new("test");
        curve.add_point(CurvePoint::new(0.0, 5.0));
        curve.add_point(CurvePoint::new(1.0, 10.0));
        curve.pre_extrapolation = Extrapolation::Constant;
        curve.post_extrapolation = Extrapolation::Constant;

        assert!((curve.evaluate(-1.0) - 5.0).abs() < 1e-6);
        assert!((curve.evaluate(2.0) - 10.0).abs() < 1e-6);
    }

    #[test]
    fn test_time_range() {
        let mut curve = AnimationCurve::new("test");
        curve.add_point(CurvePoint::new(1.0, 0.0));
        curve.add_point(CurvePoint::new(5.0, 0.0));

        let (start, end) = curve.time_range().unwrap();
        assert!((start - 1.0).abs() < 1e-10);
        assert!((end - 5.0).abs() < 1e-10);
    }
}
