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

//! Array modifier.
//!
//! Creates arrays of mesh copies with various patterns.

use super::stack::{Modifier, ModifierMesh};
use nalgebra::{Matrix4, Point3, UnitQuaternion, Vector3};
use std::any::Any;
use std::f64::consts::PI;

/// Array pattern type.
#[derive(Debug, Clone)]
pub enum ArrayPattern {
    /// Linear array with fixed offset.
    Linear {
        /// Offset between copies.
        offset: Vector3<f64>,
        /// Scale per copy.
        scale: Vector3<f64>,
        /// Rotation per copy (radians).
        rotation: Vector3<f64>,
    },
    /// Radial array around an axis.
    Radial {
        /// Center of rotation.
        center: Point3<f64>,
        /// Axis of rotation.
        axis: Vector3<f64>,
        /// Total angle (radians, 2*PI for full circle).
        angle: f64,
        /// Include endpoint (false for circular patterns).
        end_cap: bool,
    },
    /// Grid array.
    Grid {
        /// X offset.
        offset_x: f64,
        /// Y offset.
        offset_y: f64,
        /// Z offset.
        offset_z: f64,
        /// Count in X.
        count_x: usize,
        /// Count in Y.
        count_y: usize,
        /// Count in Z.
        count_z: usize,
    },
    /// Follow curve (path array).
    Curve {
        /// Curve points.
        points: Vec<Point3<f64>>,
        /// Align to curve tangent.
        align_to_curve: bool,
    },
}

impl Default for ArrayPattern {
    fn default() -> Self {
        ArrayPattern::Linear {
            offset: Vector3::new(2.0, 0.0, 0.0),
            scale: Vector3::new(1.0, 1.0, 1.0),
            rotation: Vector3::zeros(),
        }
    }
}

/// Array modifier.
#[derive(Debug, Clone)]
pub struct ArrayModifier {
    /// Modifier name.
    pub name: String,
    /// Whether enabled.
    pub enabled: bool,
    /// Number of copies.
    pub count: usize,
    /// Array pattern.
    pub pattern: ArrayPattern,
    /// Merge end vertices (for connected arrays).
    pub merge_first_last: bool,
    /// Merge threshold.
    pub merge_threshold: f64,
}

impl Default for ArrayModifier {
    fn default() -> Self {
        Self {
            name: "Array".to_string(),
            enabled: true,
            count: 2,
            pattern: ArrayPattern::default(),
            merge_first_last: false,
            merge_threshold: 0.001,
        }
    }
}

impl ArrayModifier {
    /// Create linear array modifier.
    pub fn linear(offset: Vector3<f64>, count: usize) -> Self {
        Self {
            count,
            pattern: ArrayPattern::Linear {
                offset,
                scale: Vector3::new(1.0, 1.0, 1.0),
                rotation: Vector3::zeros(),
            },
            ..Default::default()
        }
    }

    /// Create radial array modifier.
    pub fn radial(center: Point3<f64>, axis: Vector3<f64>, count: usize) -> Self {
        Self {
            count,
            pattern: ArrayPattern::Radial {
                center,
                axis,
                angle: 2.0 * PI,
                end_cap: false,
            },
            ..Default::default()
        }
    }

    /// Create grid array modifier.
    pub fn grid(spacing: f64, count_x: usize, count_y: usize, count_z: usize) -> Self {
        let total_count = count_x * count_y * count_z;
        Self {
            count: total_count,
            pattern: ArrayPattern::Grid {
                offset_x: spacing,
                offset_y: spacing,
                offset_z: spacing,
                count_x,
                count_y,
                count_z,
            },
            ..Default::default()
        }
    }

    /// Get transform for copy at given index.
    fn get_transform(&self, index: usize) -> Matrix4<f64> {
        match &self.pattern {
            ArrayPattern::Linear {
                offset,
                scale,
                rotation,
            } => {
                let t = index as f64;

                // Translation
                let translation = offset * t;

                // Cumulative scale
                let sx = scale.x.powf(t);
                let sy = scale.y.powf(t);
                let sz = scale.z.powf(t);

                // Cumulative rotation
                let rx = rotation.x * t;
                let ry = rotation.y * t;
                let rz = rotation.z * t;

                let rot_x = UnitQuaternion::from_axis_angle(&Vector3::x_axis(), rx);
                let rot_y = UnitQuaternion::from_axis_angle(&Vector3::y_axis(), ry);
                let rot_z = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), rz);
                let rotation = rot_z * rot_y * rot_x;

                let mut transform = Matrix4::identity();

                // Apply scale
                transform[(0, 0)] = sx;
                transform[(1, 1)] = sy;
                transform[(2, 2)] = sz;

                // Apply rotation
                let rot_mat = rotation.to_homogeneous();
                transform = rot_mat * transform;

                // Apply translation
                transform[(0, 3)] = translation.x;
                transform[(1, 3)] = translation.y;
                transform[(2, 3)] = translation.z;

                transform
            }
            ArrayPattern::Radial {
                center,
                axis,
                angle,
                end_cap,
            } => {
                let total_angle = if *end_cap {
                    *angle
                } else {
                    angle * (self.count - 1) as f64 / self.count as f64
                };

                let step_angle = if self.count > 1 {
                    total_angle / (self.count - 1) as f64
                } else {
                    0.0
                };

                let current_angle = step_angle * index as f64;

                let axis = nalgebra::Unit::new_normalize(*axis);
                let rotation = UnitQuaternion::from_axis_angle(&axis, current_angle);

                // Translate to origin, rotate, translate back
                let to_center = Matrix4::new_translation(&(-center.coords));
                let from_center = Matrix4::new_translation(&center.coords);
                let rot_mat = rotation.to_homogeneous();

                from_center * rot_mat * to_center
            }
            ArrayPattern::Grid {
                offset_x,
                offset_y,
                offset_z,
                count_x,
                count_y,
                ..
            } => {
                let ix = index % count_x;
                let iy = (index / count_x) % count_y;
                let iz = index / (count_x * count_y);

                let translation = Vector3::new(
                    ix as f64 * offset_x,
                    iy as f64 * offset_y,
                    iz as f64 * offset_z,
                );

                Matrix4::new_translation(&translation)
            }
            ArrayPattern::Curve {
                points,
                align_to_curve,
            } => {
                if points.is_empty() {
                    return Matrix4::identity();
                }

                let t = if self.count > 1 {
                    index as f64 / (self.count - 1) as f64
                } else {
                    0.0
                };

                let idx_f = t * (points.len() - 1) as f64;
                let idx = idx_f.floor() as usize;
                let frac = idx_f - idx as f64;

                let position = if idx >= points.len() - 1 {
                    points[points.len() - 1]
                } else {
                    Point3::new(
                        points[idx].x + (points[idx + 1].x - points[idx].x) * frac,
                        points[idx].y + (points[idx + 1].y - points[idx].y) * frac,
                        points[idx].z + (points[idx + 1].z - points[idx].z) * frac,
                    )
                };

                let mut transform = Matrix4::new_translation(&position.coords);

                if *align_to_curve && points.len() >= 2 {
                    // Calculate tangent
                    let tangent = if idx >= points.len() - 1 {
                        (points[points.len() - 1] - points[points.len() - 2]).normalize()
                    } else {
                        (points[idx + 1] - points[idx]).normalize()
                    };

                    // Create rotation to align Z axis with tangent
                    let rotation = UnitQuaternion::rotation_between(&Vector3::z(), &tangent)
                        .unwrap_or(UnitQuaternion::identity());

                    transform =
                        Matrix4::new_translation(&position.coords) * rotation.to_homogeneous();
                }

                transform
            }
        }
    }

    /// Transform a point.
    fn transform_point(&self, p: Point3<f64>, transform: &Matrix4<f64>) -> Point3<f64> {
        let h = transform * nalgebra::Vector4::new(p.x, p.y, p.z, 1.0);
        Point3::new(h.x, h.y, h.z)
    }

    /// Transform a normal.
    fn transform_normal(&self, n: Vector3<f64>, transform: &Matrix4<f64>) -> Vector3<f64> {
        // Use inverse transpose for normals
        let h = transform.fixed_view::<3, 3>(0, 0) * n;
        h.normalize()
    }
}

impl Modifier for ArrayModifier {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_id(&self) -> &'static str {
        "ArrayModifier"
    }

    fn apply(&self, mesh: &ModifierMesh) -> ModifierMesh {
        if self.count <= 1 {
            return mesh.clone();
        }

        let actual_count = match &self.pattern {
            ArrayPattern::Grid {
                count_x,
                count_y,
                count_z,
                ..
            } => count_x * count_y * count_z,
            _ => self.count,
        };

        let mut result = ModifierMesh::new();

        for copy_idx in 0..actual_count {
            let transform = self.get_transform(copy_idx);

            let vertex_offset = result.positions.len();

            // Transform and add vertices
            for i in 0..mesh.positions.len() {
                let transformed_pos = self.transform_point(mesh.positions[i], &transform);
                result.positions.push(transformed_pos);

                if i < mesh.normals.len() {
                    let transformed_normal = self.transform_normal(mesh.normals[i], &transform);
                    result.normals.push(transformed_normal);
                }
            }

            // Add faces with offset indices
            for face in &mesh.faces {
                let new_face: Vec<usize> = face.iter().map(|&vi| vi + vertex_offset).collect();
                result.faces.push(new_face);
            }

            // Copy UVs
            result.uvs.extend(&mesh.uvs);
        }

        // Optionally merge first and last vertices
        if self.merge_first_last && actual_count > 1 {
            // Merge vertices that are close together between first and last copy
            // This is useful for circular arrays
            let first_count = mesh.positions.len();
            let last_offset = (actual_count - 1) * first_count;

            for i in 0..first_count {
                let first_pos = result.positions[i];
                let last_pos = result.positions[last_offset + i];

                if (first_pos - last_pos).magnitude() < self.merge_threshold {
                    // Replace all references to last_offset + i with i
                    for face in &mut result.faces {
                        for vi in face.iter_mut() {
                            if *vi == last_offset + i {
                                *vi = i;
                            }
                        }
                    }
                }
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
    fn test_linear_array() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
            ],
            vec![vec![0, 1, 2]],
        );

        let modifier = ArrayModifier::linear(Vector3::new(2.0, 0.0, 0.0), 3);
        let result = modifier.apply(&mesh);

        assert_eq!(result.positions.len(), 9); // 3 vertices * 3 copies
        assert_eq!(result.faces.len(), 3); // 1 face * 3 copies

        // Check positions of second copy
        assert!((result.positions[3].x - 2.0).abs() < 1e-10);
        assert!((result.positions[6].x - 4.0).abs() < 1e-10);
    }

    #[test]
    fn test_radial_array() {
        let mesh = ModifierMesh::from_geometry(vec![Point3::new(1.0, 0.0, 0.0)], vec![]);

        let modifier = ArrayModifier::radial(Point3::origin(), Vector3::z(), 4);
        let result = modifier.apply(&mesh);

        assert_eq!(result.positions.len(), 4);

        // Check that vertices are arranged in a circle
        // Second vertex should be at 90 degrees (on Y axis)
        assert!((result.positions[1].x).abs() < 1e-10);
        assert!((result.positions[1].y - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_grid_array() {
        let mesh = ModifierMesh::from_geometry(vec![Point3::origin()], vec![]);

        let modifier = ArrayModifier::grid(1.0, 2, 2, 2);
        let result = modifier.apply(&mesh);

        assert_eq!(result.positions.len(), 8); // 2x2x2 grid
    }
}
