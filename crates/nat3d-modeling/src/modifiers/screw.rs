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

//! Screw (Lathe) modifier.
//!
//! Creates surface of revolution by rotating a profile around an axis.

use nalgebra::{Point3, Vector3, UnitQuaternion};
use std::any::Any;
use std::f64::consts::PI;
use super::stack::{Modifier, ModifierMesh};

/// Screw axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum ScrewAxis {
    /// X axis.
    X,
    /// Y axis.
    Y,
    /// Z axis.
    #[default]
    Z,
}


impl ScrewAxis {
    /// Get axis as unit vector.
    pub fn to_vector(&self) -> Vector3<f64> {
        match self {
            ScrewAxis::X => Vector3::x(),
            ScrewAxis::Y => Vector3::y(),
            ScrewAxis::Z => Vector3::z(),
        }
    }
}

/// Screw modifier.
#[derive(Debug, Clone)]
pub struct ScrewModifier {
    /// Modifier name.
    pub name: String,
    /// Whether enabled.
    pub enabled: bool,
    /// Axis of revolution.
    pub axis: ScrewAxis,
    /// Total rotation angle (radians).
    pub angle: f64,
    /// Number of steps.
    pub steps: usize,
    /// Screw offset (helical rise per revolution).
    pub screw_offset: f64,
    /// Number of iterations (for multiple revolutions).
    pub iterations: usize,
    /// Flip normals.
    pub flip_normals: bool,
    /// Merge vertices at start/end.
    pub merge_ends: bool,
    /// Merge threshold.
    pub merge_threshold: f64,
}

impl Default for ScrewModifier {
    fn default() -> Self {
        Self {
            name: "Screw".to_string(),
            enabled: true,
            axis: ScrewAxis::default(),
            angle: 2.0 * PI,
            steps: 16,
            screw_offset: 0.0,
            iterations: 1,
            flip_normals: false,
            merge_ends: false,
            merge_threshold: 0.001,
        }
    }
}

impl ScrewModifier {
    /// Create new screw modifier.
    pub fn new(axis: ScrewAxis, steps: usize) -> Self {
        Self {
            axis,
            steps: steps.max(3),
            ..Default::default()
        }
    }

    /// Create lathe (full revolution).
    pub fn lathe(axis: ScrewAxis, steps: usize) -> Self {
        Self {
            axis,
            steps: steps.max(3),
            angle: 2.0 * PI,
            merge_ends: true,
            ..Default::default()
        }
    }

    /// Create helix (screw with offset).
    pub fn helix(axis: ScrewAxis, steps: usize, pitch: f64) -> Self {
        Self {
            axis,
            steps: steps.max(3),
            angle: 2.0 * PI,
            screw_offset: pitch,
            ..Default::default()
        }
    }

    /// Get rotation matrix for step.
    fn get_rotation(&self, step: usize, iteration: usize) -> (UnitQuaternion<f64>, Vector3<f64>) {
        let total_step = step + iteration * self.steps;
        let _total_steps = self.steps * self.iterations;

        let step_angle = self.angle / self.steps as f64;
        let current_angle = step_angle * total_step as f64;

        let axis = nalgebra::Unit::new_normalize(self.axis.to_vector());
        let rotation = UnitQuaternion::from_axis_angle(&axis, current_angle);

        // Calculate screw offset (helical translation)
        let screw_translation = if self.screw_offset != 0.0 {
            let progress = current_angle / (2.0 * PI);
            self.axis.to_vector() * self.screw_offset * progress
        } else {
            Vector3::zeros()
        };

        (rotation, screw_translation)
    }

    /// Transform point.
    fn transform_point(&self, p: Point3<f64>, rotation: &UnitQuaternion<f64>, translation: &Vector3<f64>) -> Point3<f64> {
        let rotated = rotation * p.coords;
        Point3::from(rotated + translation)
    }

    /// Transform normal.
    fn transform_normal(&self, n: Vector3<f64>, rotation: &UnitQuaternion<f64>) -> Vector3<f64> {
        let rotated = rotation * n;
        rotated.normalize()
    }

    /// Check if vertex is on axis (for merging).
    fn is_on_axis(&self, pos: Point3<f64>, threshold: f64) -> bool {
        match self.axis {
            ScrewAxis::X => (pos.y.powi(2) + pos.z.powi(2)).sqrt() < threshold,
            ScrewAxis::Y => (pos.x.powi(2) + pos.z.powi(2)).sqrt() < threshold,
            ScrewAxis::Z => (pos.x.powi(2) + pos.y.powi(2)).sqrt() < threshold,
        }
    }
}

impl Modifier for ScrewModifier {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_id(&self) -> &'static str {
        "ScrewModifier"
    }

    fn apply(&self, mesh: &ModifierMesh) -> ModifierMesh {
        let mut result = ModifierMesh::new();

        let _total_steps = self.steps * self.iterations;

        // Map from (step, original_vertex) to result vertex index
        let mut vertex_map: std::collections::HashMap<(usize, usize), usize> = std::collections::HashMap::new();

        // Generate vertices for each step
        for iter in 0..self.iterations {
            for step in 0..self.steps {
                let (rotation, translation) = self.get_rotation(step, iter);

                for (orig_idx, orig_pos) in mesh.positions.iter().enumerate() {
                    // Check if vertex is on axis
                    let on_axis = self.is_on_axis(*orig_pos, self.merge_threshold);

                    let new_pos = self.transform_point(*orig_pos, &rotation, &translation);

                    // For vertices on axis, reuse the same vertex index
                    let result_idx = if on_axis && step > 0 {
                        // Find the first occurrence of this vertex
                        *vertex_map.get(&(0, orig_idx)).unwrap_or(&result.positions.len())
                    } else {
                        let idx = result.positions.len();
                        result.positions.push(new_pos);

                        // Transform normal
                        if orig_idx < mesh.normals.len() {
                            let new_normal = self.transform_normal(mesh.normals[orig_idx], &rotation);
                            result.normals.push(if self.flip_normals { -new_normal } else { new_normal });
                        }

                        idx
                    };

                    vertex_map.insert((step + iter * self.steps, orig_idx), result_idx);
                }
            }
        }

        // Generate faces by connecting adjacent steps
        for iter in 0..self.iterations {
            for step in 0..self.steps {
                let next_step = if step == self.steps - 1 && iter == self.iterations - 1 && self.merge_ends {
                    0 // Wrap to first step
                } else {
                    step + 1
                };

                if next_step >= self.steps && iter >= self.iterations - 1 {
                    continue; // Skip last step if not merging
                }

                let current_step_idx = step + iter * self.steps;
                let next_step_idx = if next_step == 0 && self.merge_ends {
                    0
                } else {
                    next_step + iter * self.steps
                };

                // Connect faces from original mesh
                for face in &mesh.faces {
                    if face.len() < 2 {
                        continue;
                    }

                    // For each edge in the face, create a quad
                    for i in 0..face.len() {
                        let v0 = face[i];
                        let v1 = face[(i + 1) % face.len()];

                        // Get indices in result mesh
                        let i0 = vertex_map[&(current_step_idx, v0)];
                        let i1 = vertex_map[&(current_step_idx, v1)];
                        let i2 = vertex_map[&(next_step_idx, v1)];
                        let i3 = vertex_map[&(next_step_idx, v0)];

                        // Skip degenerate faces (vertices on axis)
                        if i0 == i3 || i1 == i2 {
                            continue;
                        }

                        // Create quad face
                        if self.flip_normals {
                            result.faces.push(vec![i0, i3, i2, i1]);
                        } else {
                            result.faces.push(vec![i0, i1, i2, i3]);
                        }
                    }
                }
            }
        }

        if result.normals.is_empty() {
            result.compute_normals();
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
    fn test_screw_basic() {
        // Create a vertical line profile
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 1.0),
            ],
            vec![vec![0, 1]],
        );

        let modifier = ScrewModifier::lathe(ScrewAxis::Z, 16);
        let result = modifier.apply(&mesh);

        // Should create a cylindrical surface
        assert!(result.positions.len() > mesh.positions.len());
        assert!(result.faces.len() > 0);
    }

    #[test]
    fn test_screw_helix() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.5),
            ],
            vec![vec![0, 1]],
        );

        let modifier = ScrewModifier::helix(ScrewAxis::Z, 32, 2.0);
        let result = modifier.apply(&mesh);

        // Helix should have offset in Z
        assert!(result.positions.len() > 0);

        // Check that Z coordinates vary (helix rises)
        let min_z = result.positions.iter().map(|p| p.z).fold(f64::INFINITY, f64::min);
        let max_z = result.positions.iter().map(|p| p.z).fold(f64::NEG_INFINITY, f64::max);
        assert!(max_z > min_z);
    }

    #[test]
    fn test_screw_partial_angle() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(2.0, 0.0, 0.0),
            ],
            vec![vec![0, 1]],
        );

        let mut modifier = ScrewModifier::new(ScrewAxis::Z, 8);
        modifier.angle = PI; // Half revolution
        modifier.merge_ends = false;

        let result = modifier.apply(&mesh);

        // Should create partial revolution
        assert!(result.positions.len() > 0);
    }
}
