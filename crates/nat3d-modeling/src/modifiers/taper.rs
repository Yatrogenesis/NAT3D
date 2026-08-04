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

//! Taper modifier.
//!
//! Scales vertices progressively along an axis.

use nalgebra::Point3;
use std::any::Any;
use super::stack::{Modifier, ModifierMesh};

/// Taper axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaperAxis {
    /// Taper along X axis (scale Y and Z).
    X,
    /// Taper along Y axis (scale X and Z).
    Y,
    /// Taper along Z axis (scale X and Y).
    Z,
}

/// Taper curve type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaperCurve {
    /// Linear taper.
    Linear,
    /// Smooth (ease in/out) taper.
    Smooth,
    /// Exponential taper.
    Exponential,
}

/// Taper modifier.
#[derive(Debug, Clone)]
pub struct TaperModifier {
    /// Modifier name.
    pub name: String,
    /// Whether enabled.
    pub enabled: bool,
    /// Taper amount (0 = no taper, 1 = complete taper).
    pub amount: f64,
    /// Taper curve.
    pub curve: TaperCurve,
    /// Taper axis.
    pub axis: TaperAxis,
    /// Minimum limit along axis (None = no limit).
    pub min_limit: Option<f64>,
    /// Maximum limit along axis (None = no limit).
    pub max_limit: Option<f64>,
    /// Scale uniformly or allow non-uniform scaling.
    pub uniform: bool,
    /// Use vertex groups for weighting.
    pub vertex_group: Option<String>,
}

impl Default for TaperModifier {
    fn default() -> Self {
        Self {
            name: "Taper".to_string(),
            enabled: true,
            amount: 0.5,
            curve: TaperCurve::Linear,
            axis: TaperAxis::Z,
            min_limit: None,
            max_limit: None,
            uniform: true,
            vertex_group: None,
        }
    }
}

impl TaperModifier {
    /// Create new taper modifier.
    pub fn new(amount: f64) -> Self {
        Self {
            amount,
            ..Default::default()
        }
    }

    /// Create taper with axis.
    pub fn with_axis(amount: f64, axis: TaperAxis) -> Self {
        Self {
            amount,
            axis,
            ..Default::default()
        }
    }

    /// Get parameter t (0 to 1) along the taper axis.
    fn get_parameter(&self, p: Point3<f64>, bounds: &(Point3<f64>, Point3<f64>)) -> f64 {
        let (min, max) = bounds;

        let (axis_val, axis_min, axis_max) = match self.axis {
            TaperAxis::X => (p.x, min.x, max.x),
            TaperAxis::Y => (p.y, min.y, max.y),
            TaperAxis::Z => (p.z, min.z, max.z),
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
        }
        if let Some(max_limit) = self.max_limit {
            if t > max_limit {
                return 1.0;
            }
        }

        // Clamp to [0, 1]
        t = t.max(0.0).min(1.0);

        // Apply curve
        match self.curve {
            TaperCurve::Linear => t,
            TaperCurve::Smooth => {
                // Smoothstep
                t * t * (3.0 - 2.0 * t)
            }
            TaperCurve::Exponential => {
                // Exponential falloff
                t * t
            }
        }
    }

    /// Calculate scale factor for parameter t.
    fn calculate_scale(&self, t: f64) -> f64 {
        1.0 - (self.amount * t)
    }

    /// Apply taper transformation to a point.
    fn taper_point(&self, p: Point3<f64>, scale: f64) -> Point3<f64> {
        match self.axis {
            TaperAxis::X => {
                if self.uniform {
                    let avg = (p.y + p.z) / 2.0;
                    Point3::new(p.x, avg * scale, avg * scale)
                } else {
                    Point3::new(p.x, p.y * scale, p.z * scale)
                }
            }
            TaperAxis::Y => {
                if self.uniform {
                    let avg = (p.x + p.z) / 2.0;
                    Point3::new(avg * scale, p.y, avg * scale)
                } else {
                    Point3::new(p.x * scale, p.y, p.z * scale)
                }
            }
            TaperAxis::Z => {
                if self.uniform {
                    let avg = (p.x + p.y) / 2.0;
                    Point3::new(avg * scale, avg * scale, p.z)
                } else {
                    Point3::new(p.x * scale, p.y * scale, p.z)
                }
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

impl Modifier for TaperModifier {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_id(&self) -> &'static str {
        "TaperModifier"
    }

    fn apply(&self, mesh: &ModifierMesh) -> ModifierMesh {
        if mesh.positions.is_empty() || self.amount.abs() < 1e-10 {
            return mesh.clone();
        }

        let mut result = mesh.clone();
        let bounds = mesh.bounds();

        // Transform each vertex
        for i in 0..result.positions.len() {
            let pos = result.positions[i];
            let t = self.get_parameter(pos, &bounds);
            let weight = self.get_vertex_weight(mesh, i);

            let scale = self.calculate_scale(t * weight);
            let tapered = self.taper_point(pos, scale);
            result.positions[i] = tapered;
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
    fn test_taper_basic() {
        // Create a cube
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(-1.0, -1.0, 0.0),
                Point3::new(1.0, -1.0, 0.0),
                Point3::new(1.0, 1.0, 0.0),
                Point3::new(-1.0, 1.0, 0.0),
                Point3::new(-1.0, -1.0, 2.0),
                Point3::new(1.0, -1.0, 2.0),
                Point3::new(1.0, 1.0, 2.0),
                Point3::new(-1.0, 1.0, 2.0),
            ],
            vec![vec![0, 1, 2, 3], vec![4, 5, 6, 7]],
        );

        // Use non-uniform mode so x and y are scaled independently
        // (uniform mode averages x and y, which zeroes out symmetric coords)
        let mut modifier = TaperModifier::with_axis(0.5, TaperAxis::Z);
        modifier.uniform = false;
        let result = modifier.apply(&mesh);

        assert_eq!(result.positions.len(), 8);

        // Bottom vertices (z=0, t=0) should be unchanged (scale=1.0)
        assert!((result.positions[0].x + 1.0).abs() < 0.1);
        assert!((result.positions[1].x - 1.0).abs() < 0.1);

        // Top vertices (z=2, t=1) should be scaled by 0.5 (scale = 1.0 - 0.5*1.0 = 0.5)
        assert!((result.positions[4].x - (-0.5)).abs() < 0.1);
        assert!((result.positions[5].x - 0.5).abs() < 0.1);
    }

    #[test]
    fn test_taper_curves() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 1.0),
            ],
            vec![],
        );

        // Test linear
        let mut modifier = TaperModifier::new(0.5);
        modifier.curve = TaperCurve::Linear;
        let result = modifier.apply(&mesh);
        assert_eq!(result.positions.len(), 2);

        // Test smooth
        modifier.curve = TaperCurve::Smooth;
        let result = modifier.apply(&mesh);
        assert_eq!(result.positions.len(), 2);

        // Test exponential
        modifier.curve = TaperCurve::Exponential;
        let result = modifier.apply(&mesh);
        assert_eq!(result.positions.len(), 2);
    }

    #[test]
    fn test_taper_with_limits() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 1.0),
                Point3::new(1.0, 0.0, 2.0),
            ],
            vec![],
        );

        let mut modifier = TaperModifier::with_axis(0.5, TaperAxis::Z);
        modifier.min_limit = Some(0.3);
        modifier.max_limit = Some(0.7);

        let result = modifier.apply(&mesh);
        assert_eq!(result.positions.len(), 3);
    }
}
