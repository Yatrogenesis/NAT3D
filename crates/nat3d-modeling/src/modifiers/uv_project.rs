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

//! UV Project modifier.
//!
//! Projects UV coordinates onto mesh using various projection methods.

use nalgebra::{Point3, Vector3, Matrix4};
use std::any::Any;
use std::f64::consts::PI;
use super::stack::{Modifier, ModifierMesh};

/// UV projection type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionType {
    /// Planar projection.
    Planar,
    /// Cylindrical projection.
    Cylindrical,
    /// Spherical projection.
    Spherical,
    /// Box projection.
    Box,
    /// Camera projection.
    Camera,
}

/// UV Project modifier.
#[derive(Clone)]
pub struct UVProjectModifier {
    /// Modifier name.
    pub name: String,
    /// Whether enabled.
    pub enabled: bool,
    /// Projection type.
    pub projection_type: ProjectionType,
    /// Projector position.
    pub position: Point3<f64>,
    /// Projector rotation (Euler angles in radians).
    pub rotation: Vector3<f64>,
    /// Projector scale.
    pub scale: Vector3<f64>,
    /// Aspect ratio.
    pub aspect_ratio: f64,
    /// Scale factor for UVs.
    pub uv_scale: f64,
    /// Offset for UVs.
    pub uv_offset: (f64, f64),
    /// Vertex group for selective projection.
    pub vertex_group: Option<String>,
    /// Correct aspect ratio.
    pub correct_aspect: bool,
}

impl Default for UVProjectModifier {
    fn default() -> Self {
        Self {
            name: "UV Project".to_string(),
            enabled: true,
            projection_type: ProjectionType::Planar,
            position: Point3::origin(),
            rotation: Vector3::zeros(),
            scale: Vector3::new(1.0, 1.0, 1.0),
            aspect_ratio: 1.0,
            uv_scale: 1.0,
            uv_offset: (0.0, 0.0),
            vertex_group: None,
            correct_aspect: true,
        }
    }
}

impl UVProjectModifier {
    /// Create new UV project modifier.
    pub fn new(projection_type: ProjectionType) -> Self {
        Self {
            projection_type,
            ..Default::default()
        }
    }

    /// Create planar projection.
    pub fn planar() -> Self {
        Self::new(ProjectionType::Planar)
    }

    /// Create cylindrical projection.
    pub fn cylindrical() -> Self {
        Self::new(ProjectionType::Cylindrical)
    }

    /// Create spherical projection.
    pub fn spherical() -> Self {
        Self::new(ProjectionType::Spherical)
    }

    /// Build transformation matrix.
    fn build_transform_matrix(&self) -> Matrix4<f64> {
        // Translation
        let translation = Matrix4::new_translation(&Vector3::new(
            -self.position.x,
            -self.position.y,
            -self.position.z,
        ));

        // Rotation (ZYX order)
        let rot_x = Matrix4::from_euler_angles(self.rotation.x, 0.0, 0.0);
        let rot_y = Matrix4::from_euler_angles(0.0, self.rotation.y, 0.0);
        let rot_z = Matrix4::from_euler_angles(0.0, 0.0, self.rotation.z);

        // Scale
        let scale = Matrix4::new_nonuniform_scaling(&Vector3::new(
            1.0 / self.scale.x,
            1.0 / self.scale.y,
            1.0 / self.scale.z,
        ));

        scale * rot_z * rot_y * rot_x * translation
    }

    /// Transform point to projector space.
    fn transform_point(&self, point: Point3<f64>, transform: &Matrix4<f64>) -> Point3<f64> {
        let homogeneous = transform * point.to_homogeneous();
        Point3::from_homogeneous(homogeneous).unwrap_or(point)
    }

    /// Project point to UV (planar).
    fn project_planar(&self, point: Point3<f64>) -> (f64, f64) {
        let u = point.x * self.uv_scale + self.uv_offset.0;
        let v = point.y * self.uv_scale + self.uv_offset.1;

        if self.correct_aspect {
            (u * self.aspect_ratio, v)
        } else {
            (u, v)
        }
    }

    /// Project point to UV (cylindrical).
    fn project_cylindrical(&self, point: Point3<f64>) -> (f64, f64) {
        let angle = point.x.atan2(point.z);
        let u = (angle / (2.0 * PI) + 0.5) * self.uv_scale + self.uv_offset.0;
        let v = point.y * self.uv_scale + self.uv_offset.1;

        (u, v)
    }

    /// Project point to UV (spherical).
    fn project_spherical(&self, point: Point3<f64>) -> (f64, f64) {
        let radius = (point.x * point.x + point.y * point.y + point.z * point.z).sqrt();

        if radius < 1e-10 {
            return (0.5, 0.5);
        }

        let theta = (point.x.atan2(point.z) / (2.0 * PI) + 0.5).clamp(0.0, 1.0);
        let phi = ((point.y / radius).asin() / PI + 0.5).clamp(0.0, 1.0);

        let u = theta * self.uv_scale + self.uv_offset.0;
        let v = phi * self.uv_scale + self.uv_offset.1;

        (u, v)
    }

    /// Project point to UV (box).
    fn project_box(&self, point: Point3<f64>, normal: Vector3<f64>) -> (f64, f64) {
        let abs_normal = Vector3::new(normal.x.abs(), normal.y.abs(), normal.z.abs());

        // Choose projection plane based on dominant normal direction
        if abs_normal.x >= abs_normal.y && abs_normal.x >= abs_normal.z {
            // X face
            let u = if normal.x > 0.0 { -point.z } else { point.z };
            let v = point.y;
            (u * self.uv_scale + self.uv_offset.0, v * self.uv_scale + self.uv_offset.1)
        } else if abs_normal.y >= abs_normal.x && abs_normal.y >= abs_normal.z {
            // Y face
            let u = point.x;
            let v = if normal.y > 0.0 { -point.z } else { point.z };
            (u * self.uv_scale + self.uv_offset.0, v * self.uv_scale + self.uv_offset.1)
        } else {
            // Z face
            let u = if normal.z > 0.0 { point.x } else { -point.x };
            let v = point.y;
            (u * self.uv_scale + self.uv_offset.0, v * self.uv_scale + self.uv_offset.1)
        }
    }

    /// Project point to UV (camera).
    fn project_camera(&self, point: Point3<f64>) -> (f64, f64) {
        // Perspective projection
        if point.z.abs() < 1e-10 {
            return (0.5, 0.5);
        }

        let u = (point.x / point.z + 1.0) * 0.5 * self.uv_scale + self.uv_offset.0;
        let v = (point.y / point.z + 1.0) * 0.5 * self.uv_scale + self.uv_offset.1;

        if self.correct_aspect {
            (u * self.aspect_ratio, v)
        } else {
            (u, v)
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

impl Modifier for UVProjectModifier {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_id(&self) -> &'static str {
        "UVProjectModifier"
    }

    fn apply(&self, mesh: &ModifierMesh) -> ModifierMesh {
        if mesh.positions.is_empty() {
            return mesh.clone();
        }

        let mut result = mesh.clone();
        let transform = self.build_transform_matrix();

        // Ensure normals exist
        if result.normals.len() != result.positions.len() {
            result.compute_normals();
        }

        // Resize UVs to match vertices
        result.uvs.resize(result.positions.len(), (0.0, 0.0));

        for i in 0..result.positions.len() {
            let weight = self.get_vertex_weight(mesh, i);

            if weight < 1e-6 {
                continue;
            }

            let pos = result.positions[i];
            let normal = result.normals.get(i).copied().unwrap_or(Vector3::y());

            // Transform to projector space
            let transformed = self.transform_point(pos, &transform);

            // Project to UV
            let new_uv = match self.projection_type {
                ProjectionType::Planar => self.project_planar(transformed),
                ProjectionType::Cylindrical => self.project_cylindrical(transformed),
                ProjectionType::Spherical => self.project_spherical(transformed),
                ProjectionType::Box => self.project_box(transformed, normal),
                ProjectionType::Camera => self.project_camera(transformed),
            };

            // Apply weight
            let original_uv = mesh.uvs.get(i).copied().unwrap_or((0.0, 0.0));
            let u = original_uv.0 + (new_uv.0 - original_uv.0) * weight;
            let v = original_uv.1 + (new_uv.1 - original_uv.1) * weight;

            result.uvs[i] = (u, v);
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
    fn test_uv_project_planar() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(-1.0, -1.0, 0.0),
                Point3::new(1.0, -1.0, 0.0),
                Point3::new(1.0, 1.0, 0.0),
                Point3::new(-1.0, 1.0, 0.0),
            ],
            vec![vec![0, 1, 2, 3]],
        );

        let modifier = UVProjectModifier::planar();
        let result = modifier.apply(&mesh);

        assert_eq!(result.uvs.len(), 4);
        // UVs should be projected
        for uv in &result.uvs {
            assert!(uv.0.is_finite() && uv.1.is_finite());
        }
    }

    #[test]
    fn test_uv_project_cylindrical() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.0, 0.0, 1.0),
                Point3::new(-1.0, 0.0, 0.0),
                Point3::new(0.0, 0.0, -1.0),
            ],
            vec![vec![0, 1, 2, 3]],
        );

        let modifier = UVProjectModifier::cylindrical();
        let result = modifier.apply(&mesh);

        assert_eq!(result.uvs.len(), 4);
        // Check UV wrapping
        for uv in &result.uvs {
            assert!(uv.0 >= 0.0 && uv.0 <= 2.0);
        }
    }

    #[test]
    fn test_uv_project_spherical() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 1.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.0, -1.0, 0.0),
            ],
            vec![vec![0, 1, 2]],
        );

        let modifier = UVProjectModifier::spherical();
        let result = modifier.apply(&mesh);

        assert_eq!(result.uvs.len(), 3);
        for uv in &result.uvs {
            assert!(uv.0.is_finite() && uv.1.is_finite());
        }
    }
}
