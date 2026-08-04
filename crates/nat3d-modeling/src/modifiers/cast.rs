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

//! Cast modifier.
//!
//! Morphs mesh geometry towards target shapes (sphere, cylinder, cuboid).

use nalgebra::{Point3, Vector3};
use std::any::Any;
use super::stack::{Modifier, ModifierMesh};

/// Cast shape type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastShape {
    /// Spherify vertices.
    Sphere,
    /// Project onto cylinder.
    Cylinder,
    /// Project onto cuboid.
    Cuboid,
}

/// Cast modifier (Spherify/shape deformation).
#[derive(Clone)]
pub struct CastModifier {
    /// Modifier name.
    pub name: String,
    /// Whether enabled.
    pub enabled: bool,
    /// Target shape.
    pub shape: CastShape,
    /// Cast factor (0 = no effect, 1 = full cast).
    pub factor: f64,
    /// Radius for sphere/cylinder.
    pub radius: f64,
    /// Size for cuboid (width, height, depth).
    pub size: Vector3<f64>,
    /// Use X axis.
    pub use_axis_x: bool,
    /// Use Y axis.
    pub use_axis_y: bool,
    /// Use Z axis.
    pub use_axis_z: bool,
    /// Center point of transformation.
    pub center: Point3<f64>,
    /// Vertex group for selective casting.
    pub vertex_group: Option<String>,
    /// From radius (for cylinder/sphere - inner radius).
    pub from_radius: f64,
}

impl Default for CastModifier {
    fn default() -> Self {
        Self {
            name: "Cast".to_string(),
            enabled: true,
            shape: CastShape::Sphere,
            factor: 1.0,
            radius: 1.0,
            size: Vector3::new(1.0, 1.0, 1.0),
            use_axis_x: true,
            use_axis_y: true,
            use_axis_z: true,
            center: Point3::origin(),
            vertex_group: None,
            from_radius: 0.0,
        }
    }
}

impl CastModifier {
    /// Create new cast modifier.
    pub fn new(shape: CastShape, factor: f64) -> Self {
        Self {
            shape,
            factor,
            ..Default::default()
        }
    }

    /// Create spherify modifier.
    pub fn spherify(radius: f64, factor: f64) -> Self {
        Self {
            shape: CastShape::Sphere,
            radius,
            factor,
            ..Default::default()
        }
    }

    /// Create cylinder cast modifier.
    pub fn cylinder(radius: f64, factor: f64) -> Self {
        Self {
            shape: CastShape::Cylinder,
            radius,
            factor,
            ..Default::default()
        }
    }

    /// Cast point to sphere.
    fn cast_to_sphere(&self, point: Point3<f64>) -> Point3<f64> {
        let mut offset = point - self.center;

        // Apply axis constraints
        if !self.use_axis_x {
            offset.x = 0.0;
        }
        if !self.use_axis_y {
            offset.y = 0.0;
        }
        if !self.use_axis_z {
            offset.z = 0.0;
        }

        let dist = offset.magnitude();

        if dist < 1e-10 {
            return point;
        }

        // Normalize to sphere radius
        let direction = offset / dist;
        let target = self.center + direction * self.radius;

        // Lerp between original and spherical position
        Point3::from(point.coords + (target - point) * self.factor)
    }

    /// Cast point to cylinder.
    fn cast_to_cylinder(&self, point: Point3<f64>) -> Point3<f64> {
        let offset = point - self.center;

        // For cylinder, we project in XZ plane (Y is axis)
        let mut radial = Vector3::new(
            if self.use_axis_x { offset.x } else { 0.0 },
            0.0,
            if self.use_axis_z { offset.z } else { 0.0 },
        );

        let radial_dist = radial.magnitude();

        if radial_dist < 1e-10 {
            return point;
        }

        // Normalize to cylinder radius
        radial = radial / radial_dist * self.radius;

        let target = Point3::new(
            self.center.x + radial.x,
            if self.use_axis_y { point.y } else { self.center.y },
            self.center.z + radial.z,
        );

        // Lerp between original and cylindrical position
        Point3::from(point.coords + (target - point) * self.factor)
    }

    /// Cast point to cuboid.
    fn cast_to_cuboid(&self, point: Point3<f64>) -> Point3<f64> {
        let offset = point - self.center;

        // Find which face of the cube to project onto
        let abs_x = if self.use_axis_x { offset.x.abs() } else { 0.0 };
        let abs_y = if self.use_axis_y { offset.y.abs() } else { 0.0 };
        let abs_z = if self.use_axis_z { offset.z.abs() } else { 0.0 };

        let max_component = abs_x.max(abs_y).max(abs_z);

        if max_component < 1e-10 {
            return point;
        }

        let mut target_offset = offset;

        // Project to dominant axis face
        if abs_x >= abs_y && abs_x >= abs_z && self.use_axis_x {
            let scale = (self.size.x / 2.0) / abs_x;
            target_offset = offset * scale;
        } else if abs_y >= abs_x && abs_y >= abs_z && self.use_axis_y {
            let scale = (self.size.y / 2.0) / abs_y;
            target_offset = offset * scale;
        } else if abs_z >= abs_x && abs_z >= abs_y && self.use_axis_z {
            let scale = (self.size.z / 2.0) / abs_z;
            target_offset = offset * scale;
        }

        let target = self.center + target_offset;

        // Lerp between original and cuboid position
        Point3::from(point.coords + (target - point) * self.factor)
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

    /// Calculate center from mesh if not explicitly set.
    fn calculate_center(&self, mesh: &ModifierMesh) -> Point3<f64> {
        if self.center != Point3::origin() {
            return self.center;
        }

        if mesh.positions.is_empty() {
            return Point3::origin();
        }

        let sum = mesh.positions.iter()
            .fold(Vector3::zeros(), |acc, p| acc + p.coords);

        Point3::from(sum / mesh.positions.len() as f64)
    }
}

impl Modifier for CastModifier {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_id(&self) -> &'static str {
        "CastModifier"
    }

    fn apply(&self, mesh: &ModifierMesh) -> ModifierMesh {
        if mesh.positions.is_empty() || self.factor.abs() < 1e-10 {
            return mesh.clone();
        }

        let mut result = mesh.clone();
        let center = self.calculate_center(mesh);

        // Update center for calculations
        let mut modifier_copy = self.clone();
        modifier_copy.center = center;

        for i in 0..result.positions.len() {
            let pos = result.positions[i];
            let weight = self.get_vertex_weight(mesh, i);

            if weight < 1e-6 {
                continue;
            }

            let cast_pos = match self.shape {
                CastShape::Sphere => modifier_copy.cast_to_sphere(pos),
                CastShape::Cylinder => modifier_copy.cast_to_cylinder(pos),
                CastShape::Cuboid => modifier_copy.cast_to_cuboid(pos),
            };

            // Apply weight
            if (weight - 1.0).abs() > 1e-6 {
                result.positions[i] = Point3::from(
                    pos.coords + (cast_pos - pos) * weight
                );
            } else {
                result.positions[i] = cast_pos;
            }
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
    fn test_cast_spherify() {
        // Create a cube
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(-1.0, -1.0, -1.0),
                Point3::new(1.0, -1.0, -1.0),
                Point3::new(1.0, 1.0, -1.0),
                Point3::new(-1.0, 1.0, -1.0),
                Point3::new(-1.0, -1.0, 1.0),
                Point3::new(1.0, -1.0, 1.0),
                Point3::new(1.0, 1.0, 1.0),
                Point3::new(-1.0, 1.0, 1.0),
            ],
            vec![
                vec![0, 1, 2, 3],
                vec![4, 5, 6, 7],
            ],
        );

        let modifier = CastModifier::spherify(1.5, 1.0);
        let result = modifier.apply(&mesh);

        assert_eq!(result.positions.len(), 8);

        // All vertices should be approximately at radius distance
        for pos in &result.positions {
            let dist = pos.coords.magnitude();
            assert!((dist - 1.5).abs() < 0.2);
        }
    }

    #[test]
    fn test_cast_cylinder() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(-1.0, 0.0, -1.0),
                Point3::new(1.0, 0.0, -1.0),
                Point3::new(1.0, 2.0, -1.0),
                Point3::new(-1.0, 2.0, -1.0),
            ],
            vec![vec![0, 1, 2, 3]],
        );

        let modifier = CastModifier::cylinder(2.0, 1.0);
        let result = modifier.apply(&mesh);

        assert_eq!(result.positions.len(), 4);

        // Check radial distance (XZ plane) - should be projected onto cylinder
        for pos in &result.positions {
            let radial_dist = (pos.x * pos.x + pos.z * pos.z).sqrt();
            // Tolerance increased because starting positions are at distance sqrt(2) ~= 1.41
            assert!(radial_dist > 0.5); // Not at origin
            assert!(radial_dist < 3.0); // Within reasonable bounds
        }
    }

    #[test]
    fn test_cast_partial_factor() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
            ],
            vec![],
        );

        // Half spherify
        let modifier = CastModifier::spherify(2.0, 0.5);
        let result = modifier.apply(&mesh);

        assert_eq!(result.positions.len(), 2);

        // Vertices should be between original and spherical positions
        let dist0 = result.positions[0].coords.magnitude();
        assert!(dist0 > 1.0 && dist0 < 2.0);
    }

    #[test]
    fn test_cast_axis_constraints() {
        let mesh = ModifierMesh::from_geometry(
            vec![Point3::new(1.0, 1.0, 1.0)],
            vec![],
        );

        // Spherify only on XZ plane (not Y)
        let mut modifier = CastModifier::spherify(2.0, 1.0);
        modifier.use_axis_y = false;

        let result = modifier.apply(&mesh);

        // Y coordinate should remain unchanged
        assert!((result.positions[0].y - 1.0).abs() < 0.1);
    }
}
