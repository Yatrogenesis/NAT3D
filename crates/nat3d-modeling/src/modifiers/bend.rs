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

//! Bend modifier.
//!
//! Deforms mesh around an axis by progressively rotating vertices.

use nalgebra::Point3;
use std::any::Any;
use std::f64::consts::PI;
use super::stack::{Modifier, ModifierMesh};

/// Bend axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BendAxis {
    /// Bend around X axis (affects Y and Z).
    X,
    /// Bend around Y axis (affects X and Z).
    Y,
    /// Bend around Z axis (affects X and Y).
    Z,
}

/// Bend modifier.
#[derive(Debug, Clone)]
pub struct BendModifier {
    /// Modifier name.
    pub name: String,
    /// Whether enabled.
    pub enabled: bool,
    /// Bend angle in radians.
    pub angle: f64,
    /// Bend axis.
    pub axis: BendAxis,
    /// Minimum limit along axis (None = no limit).
    pub min_limit: Option<f64>,
    /// Maximum limit along axis (None = no limit).
    pub max_limit: Option<f64>,
    /// Center offset along bend axis.
    pub center: f64,
    /// Use vertex groups for weighting.
    pub vertex_group: Option<String>,
    /// Clamp vertices outside limits.
    pub clamp: bool,
}

impl Default for BendModifier {
    fn default() -> Self {
        Self {
            name: "Bend".to_string(),
            enabled: true,
            angle: PI / 4.0, // 45 degrees
            axis: BendAxis::Z,
            min_limit: None,
            max_limit: None,
            center: 0.0,
            vertex_group: None,
            clamp: false,
        }
    }
}

impl BendModifier {
    /// Create new bend modifier.
    pub fn new(angle: f64) -> Self {
        Self {
            angle,
            ..Default::default()
        }
    }

    /// Create bend with axis.
    pub fn with_axis(angle: f64, axis: BendAxis) -> Self {
        Self {
            angle,
            axis,
            ..Default::default()
        }
    }

    /// Get parameter t (0 to 1) along the bend axis for a point.
    fn get_parameter(&self, p: Point3<f64>, bounds: &(Point3<f64>, Point3<f64>)) -> f64 {
        let (min, max) = bounds;

        let (axis_val, axis_min, axis_max) = match self.axis {
            BendAxis::X => (p.x, min.x, max.x),
            BendAxis::Y => (p.y, min.y, max.y),
            BendAxis::Z => (p.z, min.z, max.z),
        };

        let range = axis_max - axis_min;
        if range.abs() < 1e-10 {
            return 0.0;
        }

        let t = (axis_val - axis_min - self.center) / range;

        // Apply limits if specified
        if let Some(min_limit) = self.min_limit {
            if t < min_limit {
                return if self.clamp { min_limit } else { t };
            }
        }
        if let Some(max_limit) = self.max_limit {
            if t > max_limit {
                return if self.clamp { max_limit } else { t };
            }
        }

        t
    }

    /// Apply bend transformation to a point.
    fn bend_point(&self, p: Point3<f64>, t: f64) -> Point3<f64> {
        let rotation = self.angle * t;

        // When rotation is near zero, the point is essentially unbent
        if rotation.abs() < 1e-10 {
            return p;
        }

        match self.axis {
            BendAxis::X => {
                // Rotate in YZ plane
                let radius = p.y / rotation;

                let new_y = radius * rotation.sin();
                let new_z = p.z + radius * (1.0 - rotation.cos());

                Point3::new(p.x, new_y, new_z)
            }
            BendAxis::Y => {
                // Rotate in XZ plane
                let radius = p.x / rotation;

                let new_x = radius * rotation.sin();
                let new_z = p.z + radius * (1.0 - rotation.cos());

                Point3::new(new_x, p.y, new_z)
            }
            BendAxis::Z => {
                // Rotate in XY plane
                let radius = p.x / rotation;

                let new_x = radius * rotation.sin();
                let new_y = p.y + radius * (1.0 - rotation.cos());

                Point3::new(new_x, new_y, p.z)
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

impl Modifier for BendModifier {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_id(&self) -> &'static str {
        "BendModifier"
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

            // Skip if outside limits and not clamping
            if !self.clamp {
                if let Some(min_limit) = self.min_limit {
                    if t < min_limit {
                        continue;
                    }
                }
                if let Some(max_limit) = self.max_limit {
                    if t > max_limit {
                        continue;
                    }
                }
            }

            let bent = self.bend_point(pos, t * weight);
            result.positions[i] = bent;
        }

        // Recompute normals after deformation
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
    fn test_bend_basic() {
        // Create a vertical line along Y axis
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(1.0, 1.0, 0.0),
                Point3::new(1.0, 2.0, 0.0),
            ],
            vec![vec![0, 1, 2]],
        );

        // Use BendAxis::Y so the parameter t varies along Y (where vertices differ)
        let modifier = BendModifier::with_axis(PI / 2.0, BendAxis::Y);
        let result = modifier.apply(&mesh);

        // Vertices should be bent
        assert_eq!(result.positions.len(), 3);

        // First vertex (t=0) should remain near original x
        assert!((result.positions[0].x - 1.0).abs() < 0.2);

        // Last vertex should be curved (displaced in z)
        assert!(result.positions[2].z.abs() > 0.1);
    }

    #[test]
    fn test_bend_with_limits() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
                Point3::new(0.0, 2.0, 0.0),
                Point3::new(0.0, 3.0, 0.0),
            ],
            vec![vec![0, 1, 2, 3]],
        );

        let mut modifier = BendModifier::with_axis(PI / 2.0, BendAxis::Y);
        modifier.min_limit = Some(0.3);
        modifier.max_limit = Some(0.7);

        let result = modifier.apply(&mesh);

        // Vertices outside limits should remain unchanged
        assert_eq!(result.positions.len(), 4);
    }

    #[test]
    fn test_bend_different_axes() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(1.0, 1.0, 0.0),
            ],
            vec![],
        );

        // Test X axis
        let modifier_x = BendModifier::with_axis(PI / 4.0, BendAxis::X);
        let result_x = modifier_x.apply(&mesh);
        assert_eq!(result_x.positions.len(), 2);

        // Test Y axis
        let modifier_y = BendModifier::with_axis(PI / 4.0, BendAxis::Y);
        let result_y = modifier_y.apply(&mesh);
        assert_eq!(result_y.positions.len(), 2);

        // Test Z axis
        let modifier_z = BendModifier::with_axis(PI / 4.0, BendAxis::Z);
        let result_z = modifier_z.apply(&mesh);
        assert_eq!(result_z.positions.len(), 2);
    }
}
