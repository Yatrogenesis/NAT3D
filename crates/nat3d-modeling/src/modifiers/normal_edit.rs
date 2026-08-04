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

//! Normal Edit modifier.
//!
//! Custom normal editing with directional and radial modes.

use nalgebra::{Point3, Vector3};
use std::any::Any;
use super::stack::{Modifier, ModifierMesh};

/// Normal edit mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NormalEditMode {
    /// Fixed directional normals.
    Directional,
    /// Radial normals from/to target point.
    Radial,
}

/// Normal mix mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MixMode {
    /// Replace normals.
    Copy,
    /// Add to existing normals.
    Add,
    /// Subtract from existing normals.
    Subtract,
    /// Multiply with existing normals.
    Multiply,
}

/// Normal Edit modifier.
#[derive(Clone)]
pub struct NormalEditModifier {
    /// Modifier name.
    pub name: String,
    /// Whether enabled.
    pub enabled: bool,
    /// Edit mode.
    pub mode: NormalEditMode,
    /// Target point for radial mode.
    pub target_point: Point3<f64>,
    /// Fixed direction for directional mode.
    pub direction: Vector3<f64>,
    /// Mix mode for combining with existing normals.
    pub mix_mode: MixMode,
    /// Mix factor (0 = original, 1 = fully modified).
    pub mix_factor: f64,
    /// Use object offset (for radial mode).
    pub use_offset: bool,
    /// Offset distance.
    pub offset: f64,
    /// Vertex group for selective editing.
    pub vertex_group: Option<String>,
    /// Parallel normals (all point same direction in radial mode).
    pub parallel: bool,
}

impl Default for NormalEditModifier {
    fn default() -> Self {
        Self {
            name: "Normal Edit".to_string(),
            enabled: true,
            mode: NormalEditMode::Directional,
            target_point: Point3::origin(),
            direction: Vector3::y(),
            mix_mode: MixMode::Copy,
            mix_factor: 1.0,
            use_offset: false,
            offset: 0.0,
            vertex_group: None,
            parallel: false,
        }
    }
}

impl NormalEditModifier {
    /// Create new normal edit modifier.
    pub fn new(mode: NormalEditMode) -> Self {
        Self {
            mode,
            ..Default::default()
        }
    }

    /// Create directional normal edit.
    pub fn directional(direction: Vector3<f64>) -> Self {
        let len = direction.magnitude();
        let normalized_dir = if len > 1e-10 {
            direction / len
        } else {
            Vector3::y()
        };

        Self {
            mode: NormalEditMode::Directional,
            direction: normalized_dir,
            ..Default::default()
        }
    }

    /// Create radial normal edit.
    pub fn radial(target: Point3<f64>) -> Self {
        Self {
            mode: NormalEditMode::Radial,
            target_point: target,
            ..Default::default()
        }
    }

    /// Calculate new normal for a vertex.
    fn calculate_new_normal(&self, vertex_pos: Point3<f64>) -> Vector3<f64> {
        match self.mode {
            NormalEditMode::Directional => self.direction,
            NormalEditMode::Radial => {
                let direction = if self.parallel {
                    // Use average direction for all vertices
                    self.direction
                } else {
                    // Calculate direction from target to vertex
                    vertex_pos - self.target_point
                };

                let len = direction.magnitude();
                if len > 1e-10 {
                    direction / len
                } else {
                    Vector3::y()
                }
            }
        }
    }

    /// Mix normals according to mix mode.
    fn mix_normals(
        &self,
        original: Vector3<f64>,
        new: Vector3<f64>,
        factor: f64,
    ) -> Vector3<f64> {
        let mixed = match self.mix_mode {
            MixMode::Copy => new,
            MixMode::Add => original + new,
            MixMode::Subtract => original - new,
            MixMode::Multiply => Vector3::new(
                original.x * new.x,
                original.y * new.y,
                original.z * new.z,
            ),
        };

        // Apply mix factor
        let result = original + (mixed - original) * factor;

        // Normalize
        let len = result.magnitude();
        if len > 1e-10 {
            result / len
        } else {
            Vector3::y()
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

    /// Calculate average direction for parallel mode.
    fn calculate_parallel_direction(&self, mesh: &ModifierMesh) -> Vector3<f64> {
        if mesh.positions.is_empty() {
            return Vector3::y();
        }

        let center = mesh.positions.iter()
            .fold(Vector3::zeros(), |acc, p| acc + p.coords) / mesh.positions.len() as f64;

        let direction = Point3::from(center) - self.target_point;
        let len = direction.magnitude();

        if len > 1e-10 {
            direction / len
        } else {
            Vector3::y()
        }
    }
}

impl Modifier for NormalEditModifier {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_id(&self) -> &'static str {
        "NormalEditModifier"
    }

    fn apply(&self, mesh: &ModifierMesh) -> ModifierMesh {
        if mesh.positions.is_empty() {
            return mesh.clone();
        }

        let mut result = mesh.clone();

        // Ensure normals exist
        if result.normals.len() != result.positions.len() {
            result.compute_normals();
        }

        // Calculate parallel direction if needed
        let parallel_dir = if self.parallel && self.mode == NormalEditMode::Radial {
            self.calculate_parallel_direction(mesh)
        } else {
            self.direction
        };

        let mut modifier_copy = self.clone();
        modifier_copy.direction = parallel_dir;

        for i in 0..result.positions.len() {
            let vertex_pos = result.positions[i];
            let original_normal = result.normals[i];
            let vertex_weight = self.get_vertex_weight(mesh, i);

            if vertex_weight < 1e-6 {
                continue;
            }

            // Calculate new normal
            let new_normal = modifier_copy.calculate_new_normal(vertex_pos);

            // Apply offset if enabled
            let final_new_normal = if self.use_offset {
                let offset_normal = new_normal * self.offset;
                let len = offset_normal.magnitude();
                if len > 1e-10 {
                    offset_normal / len
                } else {
                    new_normal
                }
            } else {
                new_normal
            };

            // Mix with original normal
            let mixed_normal = self.mix_normals(
                original_normal,
                final_new_normal,
                self.mix_factor * vertex_weight,
            );

            result.normals[i] = mixed_normal;
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
    fn test_normal_edit_directional() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.5, 0.0, 1.0),
            ],
            vec![vec![0, 1, 2]],
        );

        let modifier = NormalEditModifier::directional(Vector3::x());
        let result = modifier.apply(&mesh);

        assert_eq!(result.normals.len(), 3);
        // All normals should point in X direction
        for normal in &result.normals {
            assert!((normal.x - 1.0).abs() < 0.1);
            assert!(normal.y.abs() < 0.1);
        }
    }

    #[test]
    fn test_normal_edit_radial() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
                Point3::new(-1.0, 0.0, 0.0),
            ],
            vec![vec![0, 1, 2]],
        );

        let modifier = NormalEditModifier::radial(Point3::origin());
        let result = modifier.apply(&mesh);

        assert_eq!(result.normals.len(), 3);

        // Check that normals point away from origin
        for i in 0..3 {
            let vertex = result.positions[i];
            let normal = result.normals[i];
            let expected_dir = (vertex - Point3::origin()).normalize();
            let dot = normal.dot(&expected_dir);
            assert!(dot > 0.9); // Should be nearly aligned
        }
    }

    #[test]
    fn test_normal_edit_mix_modes() {
        let mesh = ModifierMesh::from_geometry(
            vec![Point3::new(0.0, 0.0, 0.0)],
            vec![],
        );

        // Test Copy mode
        let mut modifier = NormalEditModifier::directional(Vector3::x());
        modifier.mix_mode = MixMode::Copy;
        let result = modifier.apply(&mesh);
        assert!((result.normals[0].x - 1.0).abs() < 0.1);

        // Test Add mode
        modifier.mix_mode = MixMode::Add;
        let result2 = modifier.apply(&mesh);
        assert_eq!(result2.normals.len(), 1);
    }

    #[test]
    fn test_normal_edit_mix_factor() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.5, 0.0, 1.0),
            ],
            vec![vec![0, 1, 2]],
        );

        // Half mix
        let mut modifier = NormalEditModifier::directional(Vector3::x());
        modifier.mix_factor = 0.5;

        let result = modifier.apply(&mesh);

        assert_eq!(result.normals.len(), 3);
        // Normals should be between original and X direction
        for normal in &result.normals {
            assert!(normal.x > 0.3 && normal.x < 0.9);
        }
    }
}
