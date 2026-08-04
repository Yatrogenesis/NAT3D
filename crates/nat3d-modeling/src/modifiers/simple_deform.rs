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

//! Simple Deform modifier.
//!
//! Combines twist, bend, taper, and stretch deformations in one modifier.

use nalgebra::{Point3, Vector3};
use std::any::Any;
use std::f64::consts::PI;
use super::stack::{Modifier, ModifierMesh};

/// Simple deform mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimpleDeformMode {
    /// Twist around axis.
    Twist,
    /// Bend around axis.
    Bend,
    /// Taper along axis.
    Taper,
    /// Stretch along axis.
    Stretch,
}

/// Deform axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeformAxis {
    /// X axis.
    X,
    /// Y axis.
    Y,
    /// Z axis.
    Z,
}

/// Simple Deform modifier.
#[derive(Debug, Clone)]
pub struct SimpleDeformModifier {
    /// Modifier name.
    pub name: String,
    /// Whether enabled.
    pub enabled: bool,
    /// Deformation mode.
    pub mode: SimpleDeformMode,
    /// Deformation factor/angle.
    pub factor: f64,
    /// Deform axis.
    pub axis: DeformAxis,
    /// Minimum limit along axis (0 to 1).
    pub min_limit: f64,
    /// Maximum limit along axis (0 to 1).
    pub max_limit: f64,
    /// Lock X deformation.
    pub lock_x: bool,
    /// Lock Y deformation.
    pub lock_y: bool,
    /// Lock Z deformation.
    pub lock_z: bool,
    /// Origin point.
    pub origin: Point3<f64>,
    /// Use vertex groups for weighting.
    pub vertex_group: Option<String>,
}

impl Default for SimpleDeformModifier {
    fn default() -> Self {
        Self {
            name: "SimpleDeform".to_string(),
            enabled: true,
            mode: SimpleDeformMode::Twist,
            factor: PI / 4.0,
            axis: DeformAxis::Z,
            min_limit: 0.0,
            max_limit: 1.0,
            lock_x: false,
            lock_y: false,
            lock_z: false,
            origin: Point3::origin(),
            vertex_group: None,
        }
    }
}

impl SimpleDeformModifier {
    /// Create new simple deform modifier.
    pub fn new(mode: SimpleDeformMode, factor: f64) -> Self {
        Self {
            mode,
            factor,
            ..Default::default()
        }
    }

    /// Get parameter t (0 to 1) along the deform axis.
    fn get_parameter(&self, p: Point3<f64>, bounds: &(Point3<f64>, Point3<f64>)) -> f64 {
        let (min, max) = bounds;

        let (axis_val, axis_min, axis_max) = match self.axis {
            DeformAxis::X => (p.x, min.x, max.x),
            DeformAxis::Y => (p.y, min.y, max.y),
            DeformAxis::Z => (p.z, min.z, max.z),
        };

        let range = axis_max - axis_min;
        if range.abs() < 1e-10 {
            return 0.5;
        }

        let mut t = (axis_val - axis_min) / range;
        t = t.max(0.0).min(1.0);

        // Apply limits
        if t < self.min_limit {
            return 0.0;
        }
        if t > self.max_limit {
            return 1.0;
        }

        // Normalize to [0, 1] within limits
        let limited_range = self.max_limit - self.min_limit;
        if limited_range > 1e-10 {
            (t - self.min_limit) / limited_range
        } else {
            0.5
        }
    }

    /// Apply twist deformation.
    fn apply_twist(&self, p: Point3<f64>, t: f64) -> Point3<f64> {
        let rotation = self.factor * t;
        if rotation.abs() < 1e-10 {
            return p;
        }

        let rel = p - self.origin;
        let cos_a = rotation.cos();
        let sin_a = rotation.sin();

        let rotated = match self.axis {
            DeformAxis::X => {
                let new_y = rel.y * cos_a - rel.z * sin_a;
                let new_z = rel.y * sin_a + rel.z * cos_a;
                Vector3::new(rel.x, new_y, new_z)
            }
            DeformAxis::Y => {
                let new_x = rel.x * cos_a - rel.z * sin_a;
                let new_z = rel.x * sin_a + rel.z * cos_a;
                Vector3::new(new_x, rel.y, new_z)
            }
            DeformAxis::Z => {
                let new_x = rel.x * cos_a - rel.y * sin_a;
                let new_y = rel.x * sin_a + rel.y * cos_a;
                Vector3::new(new_x, new_y, rel.z)
            }
        };

        self.apply_locks(p, self.origin + rotated)
    }

    /// Apply bend deformation.
    fn apply_bend(&self, p: Point3<f64>, t: f64) -> Point3<f64> {
        if self.factor.abs() < 1e-10 {
            return p;
        }

        let rotation = self.factor * t;
        let rel = p - self.origin;

        let bent = match self.axis {
            DeformAxis::X => {
                let radius = if rotation.abs() < 1e-10 {
                    rel.y
                } else {
                    rel.y / rotation
                };
                let new_y = radius * rotation.sin();
                let new_z = rel.z + radius * (1.0 - rotation.cos());
                Vector3::new(rel.x, new_y, new_z)
            }
            DeformAxis::Y => {
                let radius = if rotation.abs() < 1e-10 {
                    rel.x
                } else {
                    rel.x / rotation
                };
                let new_x = radius * rotation.sin();
                let new_z = rel.z + radius * (1.0 - rotation.cos());
                Vector3::new(new_x, rel.y, new_z)
            }
            DeformAxis::Z => {
                let radius = if rotation.abs() < 1e-10 {
                    rel.x
                } else {
                    rel.x / rotation
                };
                let new_x = radius * rotation.sin();
                let new_y = rel.y + radius * (1.0 - rotation.cos());
                Vector3::new(new_x, new_y, rel.z)
            }
        };

        self.apply_locks(p, self.origin + bent)
    }

    /// Apply taper deformation.
    fn apply_taper(&self, p: Point3<f64>, t: f64) -> Point3<f64> {
        let scale = 1.0 - (self.factor * t);
        let rel = p - self.origin;

        let tapered = match self.axis {
            DeformAxis::X => Vector3::new(rel.x, rel.y * scale, rel.z * scale),
            DeformAxis::Y => Vector3::new(rel.x * scale, rel.y, rel.z * scale),
            DeformAxis::Z => Vector3::new(rel.x * scale, rel.y * scale, rel.z),
        };

        self.apply_locks(p, self.origin + tapered)
    }

    /// Apply stretch deformation.
    fn apply_stretch(&self, p: Point3<f64>, t: f64) -> Point3<f64> {
        let stretch = 1.0 + (self.factor * (t - 0.5));
        let rel = p - self.origin;

        let stretched = match self.axis {
            DeformAxis::X => Vector3::new(rel.x * stretch, rel.y, rel.z),
            DeformAxis::Y => Vector3::new(rel.x, rel.y * stretch, rel.z),
            DeformAxis::Z => Vector3::new(rel.x, rel.y, rel.z * stretch),
        };

        self.apply_locks(p, self.origin + stretched)
    }

    /// Apply axis locks to transformed point.
    fn apply_locks(&self, original: Point3<f64>, transformed: Point3<f64>) -> Point3<f64> {
        Point3::new(
            if self.lock_x { original.x } else { transformed.x },
            if self.lock_y { original.y } else { transformed.y },
            if self.lock_z { original.z } else { transformed.z },
        )
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

impl Modifier for SimpleDeformModifier {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_id(&self) -> &'static str {
        "SimpleDeformModifier"
    }

    fn apply(&self, mesh: &ModifierMesh) -> ModifierMesh {
        if mesh.positions.is_empty() || self.factor.abs() < 1e-10 {
            return mesh.clone();
        }

        let mut result = mesh.clone();
        let bounds = mesh.bounds();

        // Transform each vertex
        for i in 0..result.positions.len() {
            let pos = result.positions[i];
            let t = self.get_parameter(pos, &bounds);
            let weight = self.get_vertex_weight(mesh, i);

            let effective_t = t * weight;

            let deformed = match self.mode {
                SimpleDeformMode::Twist => self.apply_twist(pos, effective_t),
                SimpleDeformMode::Bend => self.apply_bend(pos, effective_t),
                SimpleDeformMode::Taper => self.apply_taper(pos, effective_t),
                SimpleDeformMode::Stretch => self.apply_stretch(pos, effective_t),
            };

            result.positions[i] = deformed;
        }

        // Recompute normals
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
    fn test_simple_deform_twist() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 1.0),
            ],
            vec![],
        );

        let modifier = SimpleDeformModifier::new(SimpleDeformMode::Twist, PI);
        let result = modifier.apply(&mesh);

        assert_eq!(result.positions.len(), 2);
        // Last vertex should be twisted
        assert!(result.positions[1].x < 0.0);
    }

    #[test]
    fn test_simple_deform_bend() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 2.0),
            ],
            vec![],
        );

        let mut modifier = SimpleDeformModifier::new(SimpleDeformMode::Bend, PI / 2.0);
        modifier.axis = DeformAxis::Z;
        let result = modifier.apply(&mesh);

        assert_eq!(result.positions.len(), 2);
    }

    #[test]
    fn test_simple_deform_taper() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 2.0),
            ],
            vec![],
        );

        let modifier = SimpleDeformModifier::new(SimpleDeformMode::Taper, 0.5);
        let result = modifier.apply(&mesh);

        assert_eq!(result.positions.len(), 2);
        // Top should be tapered
        assert!(result.positions[1].x.abs() < 1.0);
    }

    #[test]
    fn test_simple_deform_stretch() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(0.0, 0.0, 1.0),
            ],
            vec![],
        );

        let modifier = SimpleDeformModifier::new(SimpleDeformMode::Stretch, 0.5);
        let result = modifier.apply(&mesh);

        assert_eq!(result.positions.len(), 2);
    }

    #[test]
    fn test_simple_deform_limits() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(0.0, 0.0, 1.0),
                Point3::new(0.0, 0.0, 2.0),
            ],
            vec![],
        );

        let mut modifier = SimpleDeformModifier::new(SimpleDeformMode::Twist, PI);
        modifier.min_limit = 0.25;
        modifier.max_limit = 0.75;

        let result = modifier.apply(&mesh);
        assert_eq!(result.positions.len(), 3);
    }

    #[test]
    fn test_simple_deform_locks() {
        let mesh = ModifierMesh::from_geometry(
            vec![Point3::new(1.0, 1.0, 1.0)],
            vec![],
        );

        let mut modifier = SimpleDeformModifier::new(SimpleDeformMode::Twist, PI);
        modifier.lock_z = true;

        let result = modifier.apply(&mesh);

        // Z should be locked to original value
        assert!((result.positions[0].z - 1.0).abs() < 1e-6);
    }
}
