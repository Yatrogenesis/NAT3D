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

//! Curve modifier.
//!
//! Deforms mesh along a curve path using Frenet or minimum rotation frames.

use nalgebra::{Point3, Vector3};
use std::any::Any;
use super::stack::{Modifier, ModifierMesh};

/// Deformation axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeformAxis {
    X,
    Y,
    Z,
}

/// Curve modifier.
#[derive(Clone)]
pub struct CurveModifier {
    /// Modifier name.
    pub name: String,
    /// Whether enabled.
    pub enabled: bool,
    /// Curve points defining the path.
    pub curve_points: Vec<Point3<f64>>,
    /// Primary deformation axis.
    pub axis: DeformAxis,
    /// Secondary deform axis (perpendicular).
    pub deform_axis: DeformAxis,
    /// Stretch mesh to fit curve length.
    pub stretch: bool,
    /// Clamp vertices outside curve bounds.
    pub bounds_clamp: bool,
    /// Use minimum rotation frame instead of Frenet.
    pub use_min_rotation: bool,
    /// Vertex group for selective deformation.
    pub vertex_group: Option<String>,
}

impl Default for CurveModifier {
    fn default() -> Self {
        Self {
            name: "Curve".to_string(),
            enabled: true,
            curve_points: vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
            ],
            axis: DeformAxis::Z,
            deform_axis: DeformAxis::X,
            stretch: false,
            bounds_clamp: false,
            use_min_rotation: true,
            vertex_group: None,
        }
    }
}

impl CurveModifier {
    /// Create new curve modifier.
    pub fn new(curve_points: Vec<Point3<f64>>) -> Self {
        Self {
            curve_points,
            ..Default::default()
        }
    }

    /// Create with axis.
    pub fn with_axis(curve_points: Vec<Point3<f64>>, axis: DeformAxis) -> Self {
        Self {
            curve_points,
            axis,
            ..Default::default()
        }
    }

    /// Get axis value from point.
    fn get_axis_value(&self, point: Point3<f64>, axis: DeformAxis) -> f64 {
        match axis {
            DeformAxis::X => point.x,
            DeformAxis::Y => point.y,
            DeformAxis::Z => point.z,
        }
    }

    /// Set axis value in point.
    fn set_axis_value(&self, mut point: Point3<f64>, axis: DeformAxis, value: f64) -> Point3<f64> {
        match axis {
            DeformAxis::X => point.x = value,
            DeformAxis::Y => point.y = value,
            DeformAxis::Z => point.z = value,
        }
        point
    }

    /// Calculate curve parameter (0 to 1) for given axis value.
    fn calculate_curve_parameter(
        &self,
        axis_value: f64,
        bounds: &(Point3<f64>, Point3<f64>),
    ) -> f64 {
        let (min, max) = bounds;
        let axis_min = self.get_axis_value(*min, self.axis);
        let axis_max = self.get_axis_value(*max, self.axis);

        let range = axis_max - axis_min;
        if range.abs() < 1e-10 {
            return 0.5;
        }

        let t = (axis_value - axis_min) / range;

        if self.bounds_clamp {
            t.clamp(0.0, 1.0)
        } else {
            t
        }
    }

    /// Evaluate curve at parameter t (0 to 1).
    fn evaluate_curve(&self, t: f64) -> Point3<f64> {
        if self.curve_points.is_empty() {
            return Point3::origin();
        }

        if self.curve_points.len() == 1 {
            return self.curve_points[0];
        }

        let t_clamped = t.clamp(0.0, 1.0);
        let segment_count = self.curve_points.len() - 1;
        let segment_t = t_clamped * segment_count as f64;
        let segment_idx = (segment_t.floor() as usize).min(segment_count - 1);
        let local_t = segment_t - segment_idx as f64;

        let p0 = self.curve_points[segment_idx];
        let p1 = self.curve_points[segment_idx + 1];

        // Linear interpolation
        Point3::from(p0.coords + (p1 - p0) * local_t)
    }

    /// Calculate tangent at parameter t.
    fn calculate_tangent(&self, t: f64) -> Vector3<f64> {
        if self.curve_points.len() < 2 {
            return Vector3::y();
        }

        let t_clamped = t.clamp(0.0, 1.0);
        let segment_count = self.curve_points.len() - 1;
        let segment_t = t_clamped * segment_count as f64;
        let segment_idx = (segment_t.floor() as usize).min(segment_count - 1);

        let p0 = self.curve_points[segment_idx];
        let p1 = self.curve_points[segment_idx + 1];

        let tangent = p1 - p0;
        let len = tangent.magnitude();

        if len > 1e-10 {
            tangent / len
        } else {
            Vector3::y()
        }
    }

    /// Calculate Frenet frame (tangent, normal, binormal).
    fn calculate_frenet_frame(&self, t: f64) -> (Vector3<f64>, Vector3<f64>, Vector3<f64>) {
        let tangent = self.calculate_tangent(t);

        // Calculate normal (approximate using finite differences)
        let dt = 0.01;
        let t_next = (t + dt).min(1.0);
        let tangent_next = self.calculate_tangent(t_next);

        let delta_tangent = tangent_next - tangent;
        let normal = if delta_tangent.magnitude() > 1e-10 {
            delta_tangent.normalize()
        } else {
            // Fallback to arbitrary perpendicular vector
            if tangent.y.abs() < 0.9 {
                Vector3::y().cross(&tangent).normalize()
            } else {
                Vector3::x().cross(&tangent).normalize()
            }
        };

        let binormal = tangent.cross(&normal).normalize();
        let corrected_normal = binormal.cross(&tangent).normalize();

        (tangent, corrected_normal, binormal)
    }

    /// Deform point along curve.
    fn deform_point(&self, point: Point3<f64>, bounds: &(Point3<f64>, Point3<f64>)) -> Point3<f64> {
        let axis_value = self.get_axis_value(point, self.axis);
        let t = self.calculate_curve_parameter(axis_value, bounds);

        // Get curve position and frame
        let curve_pos = self.evaluate_curve(t);
        let (_tangent, normal, binormal) = self.calculate_frenet_frame(t);

        // Get perpendicular offsets
        let offset_x = match self.deform_axis {
            DeformAxis::X => point.x,
            DeformAxis::Y => point.y,
            DeformAxis::Z => point.z,
        };

        let offset_y = match self.axis {
            DeformAxis::X => if self.deform_axis == DeformAxis::Y { 0.0 } else { point.y },
            DeformAxis::Y => if self.deform_axis == DeformAxis::X { 0.0 } else { point.x },
            DeformAxis::Z => if self.deform_axis == DeformAxis::X { 0.0 } else { point.x },
        };

        // Apply frame transformation
        

        curve_pos + normal * offset_x + binormal * offset_y
    }

    /// Calculate curve length.
    fn curve_length(&self) -> f64 {
        let mut length = 0.0;
        for i in 0..self.curve_points.len().saturating_sub(1) {
            let p0 = self.curve_points[i];
            let p1 = self.curve_points[i + 1];
            length += (p1 - p0).magnitude();
        }
        length
    }

    /// Get vertex weight from vertex group.
    fn get_vertex_weight(&self, mesh: &ModifierMesh, vertex_idx: usize) -> f64 {
        if let Some(ref group_name) = self.vertex_group {
            if let Some(weights) = mesh.vertex_groups.get(group_name) {
                for &(vi, weight) in weights {
                    if vi == vertex_idx {
                        return weight;
                    }
                }
            }
        }
        1.0
    }
}

impl Modifier for CurveModifier {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_id(&self) -> &'static str {
        "CurveModifier"
    }

    fn apply(&self, mesh: &ModifierMesh) -> ModifierMesh {
        if mesh.positions.is_empty() || self.curve_points.len() < 2 {
            return mesh.clone();
        }

        let mut result = mesh.clone();
        let bounds = mesh.bounds();

        for i in 0..result.positions.len() {
            let pos = result.positions[i];
            let weight = self.get_vertex_weight(mesh, i);

            if weight < 1e-6 {
                continue;
            }

            // Check bounds clamping
            let axis_value = self.get_axis_value(pos, self.axis);
            let t = self.calculate_curve_parameter(axis_value, &bounds);

            if self.bounds_clamp && !(0.0..=1.0).contains(&t) {
                continue;
            }

            let deformed = self.deform_point(pos, &bounds);

            // Apply weight
            result.positions[i] = Point3::from(
                pos.coords + (deformed - pos) * weight
            );
        }

        result.compute_normals();
        result
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn clone_box(&self) -> Box<dyn Modifier> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_curve_deform_basic() {
        let curve = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
            Point3::new(0.0, 2.0, 1.0),
        ];

        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(0.0, 0.5, 0.0),
                Point3::new(0.0, 1.0, 0.0),
            ],
            vec![],
        );

        let modifier = CurveModifier::new(curve);
        let result = modifier.apply(&mesh);

        assert_eq!(result.positions.len(), 3);
    }

    #[test]
    fn test_curve_evaluate() {
        let curve = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
        ];

        let modifier = CurveModifier::new(curve);

        let p0 = modifier.evaluate_curve(0.0);
        assert!((p0.x - 0.0).abs() < 1e-6);

        let p1 = modifier.evaluate_curve(1.0);
        assert!((p1.x - 1.0).abs() < 1e-6);

        let p_mid = modifier.evaluate_curve(0.5);
        assert!((p_mid.x - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_curve_tangent() {
        let curve = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
        ];

        let modifier = CurveModifier::new(curve);
        let tangent = modifier.calculate_tangent(0.5);

        // Should point in Y direction
        assert!(tangent.y > 0.9);
        assert!(tangent.x.abs() < 0.1);
    }
}
