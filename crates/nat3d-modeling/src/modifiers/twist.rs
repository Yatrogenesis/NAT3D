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

//! Twist modifier.
//!
//! Rotates vertices progressively along an axis.

use nalgebra::{Point3, Vector3};
use std::any::Any;
use std::f64::consts::PI;
use super::stack::{Modifier, ModifierMesh};

/// Twist axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TwistAxis {
    /// Twist around X axis.
    X,
    /// Twist around Y axis.
    Y,
    /// Twist around Z axis.
    Z,
}

/// Twist modifier.
#[derive(Debug, Clone)]
pub struct TwistModifier {
    /// Modifier name.
    pub name: String,
    /// Whether enabled.
    pub enabled: bool,
    /// Total twist angle in radians.
    pub angle: f64,
    /// Twist axis.
    pub axis: TwistAxis,
    /// Minimum limit along axis (None = no limit).
    pub min_limit: Option<f64>,
    /// Maximum limit along axis (None = no limit).
    pub max_limit: Option<f64>,
    /// Bias for non-linear twist (-1 to 1, 0 = linear).
    pub bias: f64,
    /// Center point for twist.
    pub center: Point3<f64>,
    /// Use vertex groups for weighting.
    pub vertex_group: Option<String>,
}

impl Default for TwistModifier {
    fn default() -> Self {
        Self {
            name: "Twist".to_string(),
            enabled: true,
            angle: PI, // 180 degrees
            axis: TwistAxis::Z,
            min_limit: None,
            max_limit: None,
            bias: 0.0,
            center: Point3::origin(),
            vertex_group: None,
        }
    }
}

impl TwistModifier {
    /// Create new twist modifier.
    pub fn new(angle: f64) -> Self {
        Self {
            angle,
            ..Default::default()
        }
    }

    /// Create twist with axis.
    pub fn with_axis(angle: f64, axis: TwistAxis) -> Self {
        Self {
            angle,
            axis,
            ..Default::default()
        }
    }

    /// Get parameter t (0 to 1) along the twist axis.
    fn get_parameter(&self, p: Point3<f64>, bounds: &(Point3<f64>, Point3<f64>)) -> f64 {
        let (min, max) = bounds;

        let (axis_val, axis_min, axis_max) = match self.axis {
            TwistAxis::X => (p.x, min.x, max.x),
            TwistAxis::Y => (p.y, min.y, max.y),
            TwistAxis::Z => (p.z, min.z, max.z),
        };

        let range = axis_max - axis_min;
        if range.abs() < 1e-10 {
            return 0.0;
        }

        let mut t = (axis_val - axis_min) / range;

        // Apply limits if specified
        if let Some(min_limit) = self.min_limit {
            if t < min_limit {
                return 0.0;
            }
            t = (t - min_limit) / (1.0 - min_limit);
        }
        if let Some(max_limit) = self.max_limit {
            if t > max_limit {
                return 1.0;
            }
            t /= max_limit;
        }

        // Clamp to [0, 1]
        t = t.max(0.0).min(1.0);

        // Apply bias for non-linear twist
        if self.bias.abs() > 1e-6 {
            if self.bias > 0.0 {
                // Ease out (more twist at the end)
                t = t.powf(1.0 + self.bias * 2.0)
            } else {
                // Ease in (more twist at the start)
                t = 1.0 - (1.0 - t).powf(1.0 - self.bias * 2.0)
            }
        }

        t
    }

    /// Apply twist transformation to a point.
    fn twist_point(&self, p: Point3<f64>, rotation_angle: f64) -> Point3<f64> {
        if rotation_angle.abs() < 1e-10 {
            return p;
        }

        let rel = p - self.center;
        let cos_a = rotation_angle.cos();
        let sin_a = rotation_angle.sin();

        let rotated = match self.axis {
            TwistAxis::X => {
                // Rotate in YZ plane
                let new_y = rel.y * cos_a - rel.z * sin_a;
                let new_z = rel.y * sin_a + rel.z * cos_a;
                Vector3::new(rel.x, new_y, new_z)
            }
            TwistAxis::Y => {
                // Rotate in XZ plane
                let new_x = rel.x * cos_a - rel.z * sin_a;
                let new_z = rel.x * sin_a + rel.z * cos_a;
                Vector3::new(new_x, rel.y, new_z)
            }
            TwistAxis::Z => {
                // Rotate in XY plane
                let new_x = rel.x * cos_a - rel.y * sin_a;
                let new_y = rel.x * sin_a + rel.y * cos_a;
                Vector3::new(new_x, new_y, rel.z)
            }
        };

        self.center + rotated
    }

    /// Rotate a normal vector.
    fn twist_normal(&self, n: Vector3<f64>, rotation_angle: f64) -> Vector3<f64> {
        if rotation_angle.abs() < 1e-10 {
            return n;
        }

        let cos_a = rotation_angle.cos();
        let sin_a = rotation_angle.sin();

        match self.axis {
            TwistAxis::X => {
                let new_y = n.y * cos_a - n.z * sin_a;
                let new_z = n.y * sin_a + n.z * cos_a;
                Vector3::new(n.x, new_y, new_z)
            }
            TwistAxis::Y => {
                let new_x = n.x * cos_a - n.z * sin_a;
                let new_z = n.x * sin_a + n.z * cos_a;
                Vector3::new(new_x, n.y, new_z)
            }
            TwistAxis::Z => {
                let new_x = n.x * cos_a - n.y * sin_a;
                let new_y = n.x * sin_a + n.y * cos_a;
                Vector3::new(new_x, new_y, n.z)
            }
        }
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

impl Modifier for TwistModifier {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_id(&self) -> &'static str {
        "TwistModifier"
    }

    fn apply(&self, mesh: &ModifierMesh) -> ModifierMesh {
        if mesh.positions.is_empty() || self.angle.abs() < 1e-10 {
            return mesh.clone();
        }

        let mut result = mesh.clone();
        let bounds = mesh.bounds();

        // Transform each vertex
        for i in 0..result.positions.len() {
            let pos = result.positions[i];
            let t = self.get_parameter(pos, &bounds);
            let weight = self.get_vertex_weight(mesh, i);

            let rotation_angle = self.angle * t * weight;
            let twisted = self.twist_point(pos, rotation_angle);
            result.positions[i] = twisted;

            // Also twist normals
            if i < result.normals.len() {
                let normal = result.normals[i];
                let twisted_normal = self.twist_normal(normal, rotation_angle);
                result.normals[i] = twisted_normal.normalize();
            }
        }

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
    fn test_twist_basic() {
        // Create a vertical line
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 1.0),
                Point3::new(1.0, 0.0, 2.0),
            ],
            vec![vec![0, 1, 2]],
        );

        let modifier = TwistModifier::with_axis(PI, TwistAxis::Z);
        let result = modifier.apply(&mesh);

        assert_eq!(result.positions.len(), 3);

        // First vertex should be unchanged (t=0)
        assert!((result.positions[0].x - 1.0).abs() < 0.1);

        // Last vertex should be rotated 180 degrees
        assert!((result.positions[2].x + 1.0).abs() < 0.1);
    }

    #[test]
    fn test_twist_with_limits() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 1.0),
                Point3::new(1.0, 0.0, 2.0),
            ],
            vec![],
        );

        let mut modifier = TwistModifier::with_axis(PI, TwistAxis::Z);
        modifier.min_limit = Some(0.25);
        modifier.max_limit = Some(0.75);

        let result = modifier.apply(&mesh);
        assert_eq!(result.positions.len(), 3);
    }

    #[test]
    fn test_twist_different_axes() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 1.0),
            ],
            vec![],
        );

        // Test X axis
        let modifier_x = TwistModifier::with_axis(PI / 2.0, TwistAxis::X);
        let result_x = modifier_x.apply(&mesh);
        assert_eq!(result_x.positions.len(), 2);

        // Test Y axis
        let modifier_y = TwistModifier::with_axis(PI / 2.0, TwistAxis::Y);
        let result_y = modifier_y.apply(&mesh);
        assert_eq!(result_y.positions.len(), 2);

        // Test Z axis
        let modifier_z = TwistModifier::with_axis(PI / 2.0, TwistAxis::Z);
        let result_z = modifier_z.apply(&mesh);
        assert_eq!(result_z.positions.len(), 2);
    }

    #[test]
    fn test_twist_bias() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 1.0),
            ],
            vec![],
        );

        // Test positive bias (ease out)
        let mut modifier = TwistModifier::new(PI);
        modifier.bias = 0.5;
        let result = modifier.apply(&mesh);
        assert_eq!(result.positions.len(), 2);

        // Test negative bias (ease in)
        modifier.bias = -0.5;
        let result = modifier.apply(&mesh);
        assert_eq!(result.positions.len(), 2);
    }
}
