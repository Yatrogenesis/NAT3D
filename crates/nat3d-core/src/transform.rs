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

//! Transform system for 3D objects.
//!
//! Provides transformation matrices and decomposition for position,
//! rotation, and scale of objects in 3D space.

use nalgebra::{Matrix4, Point3, UnitQuaternion, Vector3};
use serde::{Deserialize, Serialize};

/// Type alias for 3D position.
pub type Position3 = Point3<f64>;

/// Type alias for 3D vector.
pub type Vec3 = Vector3<f64>;

/// Decomposed transform components.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TransformComponent {
    /// Position/translation.
    pub position: Vec3,
    /// Rotation as a quaternion.
    pub rotation: UnitQuaternion<f64>,
    /// Scale (non-uniform).
    pub scale: Vec3,
}

impl TransformComponent {
    /// Create a new transform component.
    #[must_use]
    pub fn new(position: Vec3, rotation: UnitQuaternion<f64>, scale: Vec3) -> Self {
        Self {
            position,
            rotation,
            scale,
        }
    }

    /// Create an identity transform component.
    #[must_use]
    pub fn identity() -> Self {
        Self {
            position: Vec3::zeros(),
            rotation: UnitQuaternion::identity(),
            scale: Vec3::new(1.0, 1.0, 1.0),
        }
    }

    /// Create a transform with only position.
    #[must_use]
    pub fn from_position(position: Vec3) -> Self {
        Self {
            position,
            ..Self::identity()
        }
    }

    /// Create a transform with only rotation.
    #[must_use]
    pub fn from_rotation(rotation: UnitQuaternion<f64>) -> Self {
        Self {
            rotation,
            ..Self::identity()
        }
    }

    /// Create a transform with only scale.
    #[must_use]
    pub fn from_scale(scale: Vec3) -> Self {
        Self {
            scale,
            ..Self::identity()
        }
    }

    /// Create a transform with uniform scale.
    #[must_use]
    pub fn from_uniform_scale(scale: f64) -> Self {
        Self::from_scale(Vec3::new(scale, scale, scale))
    }

    /// Get Euler angles (in radians) from the rotation.
    #[must_use]
    pub fn euler_angles(&self) -> (f64, f64, f64) {
        self.rotation.euler_angles()
    }

    /// Set rotation from Euler angles (in radians).
    pub fn set_euler_angles(&mut self, roll: f64, pitch: f64, yaw: f64) {
        self.rotation = UnitQuaternion::from_euler_angles(roll, pitch, yaw);
    }

    /// Get the forward direction vector (-Z in local space).
    #[must_use]
    pub fn forward(&self) -> Vec3 {
        self.rotation * Vec3::new(0.0, 0.0, -1.0)
    }

    /// Get the right direction vector (+X in local space).
    #[must_use]
    pub fn right(&self) -> Vec3 {
        self.rotation * Vec3::new(1.0, 0.0, 0.0)
    }

    /// Get the up direction vector (+Y in local space).
    #[must_use]
    pub fn up(&self) -> Vec3 {
        self.rotation * Vec3::new(0.0, 1.0, 0.0)
    }

    /// Look at a target position.
    pub fn look_at(&mut self, target: Vec3, up: Vec3) {
        let direction = (target - self.position).normalize();
        if direction.magnitude() > f64::EPSILON {
            let rotation_matrix = Matrix4::look_at_rh(
                &Position3::from(self.position),
                &Position3::from(target),
                &up,
            );
            // Extract rotation from view matrix (need to invert)
            let m3: nalgebra::Matrix3<f64> = rotation_matrix.fixed_view::<3, 3>(0, 0).into();
            let rot = UnitQuaternion::from_matrix(&m3);
            self.rotation = rot.inverse();
        }
    }

    /// Linearly interpolate between two transform components.
    #[must_use]
    pub fn lerp(&self, other: &Self, t: f64) -> Self {
        Self {
            position: self.position.lerp(&other.position, t),
            rotation: self.rotation.slerp(&other.rotation, t),
            scale: self.scale.lerp(&other.scale, t),
        }
    }
}

impl Default for TransformComponent {
    fn default() -> Self {
        Self::identity()
    }
}

/// A 4x4 transformation matrix with caching and decomposition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transform {
    /// The decomposed transform components.
    components: TransformComponent,
    /// Cached matrix (computed lazily).
    #[serde(skip)]
    matrix_cache: Option<Matrix4<f64>>,
    /// Cached inverse matrix.
    #[serde(skip)]
    inverse_cache: Option<Matrix4<f64>>,
}

impl Transform {
    /// Create a new transform from components.
    #[must_use]
    pub fn new(components: TransformComponent) -> Self {
        Self {
            components,
            matrix_cache: None,
            inverse_cache: None,
        }
    }

    /// Create an identity transform.
    #[must_use]
    pub fn identity() -> Self {
        Self::new(TransformComponent::identity())
    }

    /// Create a transform from a position.
    #[must_use]
    pub fn from_position(x: f64, y: f64, z: f64) -> Self {
        Self::new(TransformComponent::from_position(Vec3::new(x, y, z)))
    }

    /// Create a transform from a position vector.
    #[must_use]
    pub fn from_position_vec(position: Vec3) -> Self {
        Self::new(TransformComponent::from_position(position))
    }

    /// Create a transform from axis-angle rotation.
    #[must_use]
    pub fn from_axis_angle(axis: Vec3, angle: f64) -> Self {
        let rotation = UnitQuaternion::from_axis_angle(&nalgebra::Unit::new_normalize(axis), angle);
        Self::new(TransformComponent::from_rotation(rotation))
    }

    /// Create a transform from Euler angles (in radians).
    #[must_use]
    pub fn from_euler(roll: f64, pitch: f64, yaw: f64) -> Self {
        let rotation = UnitQuaternion::from_euler_angles(roll, pitch, yaw);
        Self::new(TransformComponent::from_rotation(rotation))
    }

    /// Create a transform from uniform scale.
    #[must_use]
    pub fn from_scale(scale: f64) -> Self {
        Self::new(TransformComponent::from_uniform_scale(scale))
    }

    /// Create a transform from non-uniform scale.
    #[must_use]
    pub fn from_scale_xyz(x: f64, y: f64, z: f64) -> Self {
        Self::new(TransformComponent::from_scale(Vec3::new(x, y, z)))
    }

    /// Create a transform from a 4x4 matrix.
    #[must_use]
    pub fn from_matrix(matrix: Matrix4<f64>) -> Self {
        let components = decompose_matrix(&matrix);
        let mut transform = Self::new(components);
        transform.matrix_cache = Some(matrix);
        transform
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Getters
    // ══════════════════════════════════════════════════════════════════════════

    /// Get the position component.
    #[must_use]
    pub fn position(&self) -> Vec3 {
        self.components.position
    }

    /// Get the rotation component.
    #[must_use]
    pub fn rotation(&self) -> UnitQuaternion<f64> {
        self.components.rotation
    }

    /// Get the scale component.
    #[must_use]
    pub fn scale(&self) -> Vec3 {
        self.components.scale
    }

    /// Get the components.
    #[must_use]
    pub fn components(&self) -> &TransformComponent {
        &self.components
    }

    /// Get the transformation matrix.
    #[must_use]
    pub fn matrix(&mut self) -> Matrix4<f64> {
        if let Some(m) = self.matrix_cache {
            return m;
        }

        let matrix = compose_matrix(&self.components);
        self.matrix_cache = Some(matrix);
        matrix
    }

    /// Get the inverse transformation matrix.
    #[must_use]
    pub fn inverse_matrix(&mut self) -> Option<Matrix4<f64>> {
        if let Some(m) = self.inverse_cache {
            return Some(m);
        }

        let matrix = self.matrix();
        if let Some(inv) = matrix.try_inverse() {
            self.inverse_cache = Some(inv);
            Some(inv)
        } else {
            None
        }
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Setters
    // ══════════════════════════════════════════════════════════════════════════

    /// Set the position.
    pub fn set_position(&mut self, position: Vec3) {
        self.components.position = position;
        self.invalidate_cache();
    }

    /// Set the position from x, y, z.
    pub fn set_position_xyz(&mut self, x: f64, y: f64, z: f64) {
        self.set_position(Vec3::new(x, y, z));
    }

    /// Set the rotation.
    pub fn set_rotation(&mut self, rotation: UnitQuaternion<f64>) {
        self.components.rotation = rotation;
        self.invalidate_cache();
    }

    /// Set the rotation from Euler angles.
    pub fn set_euler(&mut self, roll: f64, pitch: f64, yaw: f64) {
        self.set_rotation(UnitQuaternion::from_euler_angles(roll, pitch, yaw));
    }

    /// Set the scale.
    pub fn set_scale(&mut self, scale: Vec3) {
        self.components.scale = scale;
        self.invalidate_cache();
    }

    /// Set uniform scale.
    pub fn set_uniform_scale(&mut self, scale: f64) {
        self.set_scale(Vec3::new(scale, scale, scale));
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Operations
    // ══════════════════════════════════════════════════════════════════════════

    /// Translate by a vector.
    pub fn translate(&mut self, delta: Vec3) {
        self.components.position += delta;
        self.invalidate_cache();
    }

    /// Translate along x, y, z.
    pub fn translate_xyz(&mut self, x: f64, y: f64, z: f64) {
        self.translate(Vec3::new(x, y, z));
    }

    /// Rotate by a quaternion.
    pub fn rotate(&mut self, rotation: UnitQuaternion<f64>) {
        self.components.rotation = rotation * self.components.rotation;
        self.invalidate_cache();
    }

    /// Rotate around an axis by an angle.
    pub fn rotate_axis(&mut self, axis: Vec3, angle: f64) {
        let rotation = UnitQuaternion::from_axis_angle(&nalgebra::Unit::new_normalize(axis), angle);
        self.rotate(rotation);
    }

    /// Rotate around local X axis.
    pub fn rotate_x(&mut self, angle: f64) {
        self.rotate_axis(Vec3::x(), angle);
    }

    /// Rotate around local Y axis.
    pub fn rotate_y(&mut self, angle: f64) {
        self.rotate_axis(Vec3::y(), angle);
    }

    /// Rotate around local Z axis.
    pub fn rotate_z(&mut self, angle: f64) {
        self.rotate_axis(Vec3::z(), angle);
    }

    /// Scale by a factor.
    pub fn scale_by(&mut self, factor: Vec3) {
        self.components.scale.component_mul_assign(&factor);
        self.invalidate_cache();
    }

    /// Scale uniformly.
    pub fn scale_uniform(&mut self, factor: f64) {
        self.scale_by(Vec3::new(factor, factor, factor));
    }

    /// Transform a point.
    #[must_use]
    pub fn transform_point(&mut self, point: Position3) -> Position3 {
        self.matrix().transform_point(&point)
    }

    /// Transform a vector (ignoring translation).
    #[must_use]
    pub fn transform_vector(&mut self, vector: Vec3) -> Vec3 {
        let m = self.matrix();
        let m3: nalgebra::Matrix3<f64> = m.fixed_view::<3, 3>(0, 0).into();
        m3 * vector
    }

    /// Transform a normal (using inverse transpose).
    #[must_use]
    pub fn transform_normal(&mut self, normal: Vec3) -> Vec3 {
        if let Some(inv) = self.inverse_matrix() {
            let inv_t = inv.transpose();
            let m3: nalgebra::Matrix3<f64> = inv_t.fixed_view::<3, 3>(0, 0).into();
            (m3 * normal).normalize()
        } else {
            normal
        }
    }

    /// Combine with another transform (self * other).
    #[must_use]
    pub fn combine(&mut self, other: &mut Transform) -> Transform {
        let combined = self.matrix() * other.matrix();
        Transform::from_matrix(combined)
    }

    /// Linearly interpolate between two transforms.
    #[must_use]
    pub fn lerp(&self, other: &Self, t: f64) -> Self {
        Self::new(self.components.lerp(&other.components, t))
    }

    /// Invalidate cached matrices.
    fn invalidate_cache(&mut self) {
        self.matrix_cache = None;
        self.inverse_cache = None;
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::identity()
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Helper Functions
// ══════════════════════════════════════════════════════════════════════════════

/// Compose a 4x4 matrix from transform components.
#[must_use]
pub fn compose_matrix(components: &TransformComponent) -> Matrix4<f64> {
    let translation = Matrix4::new_translation(&components.position);
    let rotation = components.rotation.to_homogeneous();
    let scale = Matrix4::new_nonuniform_scaling(&components.scale);

    translation * rotation * scale
}

/// Decompose a 4x4 matrix into transform components.
/// Note: This assumes the matrix has no shear.
#[must_use]
pub fn decompose_matrix(matrix: &Matrix4<f64>) -> TransformComponent {
    // Extract translation
    let position = Vec3::new(matrix[(0, 3)], matrix[(1, 3)], matrix[(2, 3)]);

    // Extract scale (magnitude of each column)
    let sx = Vec3::new(matrix[(0, 0)], matrix[(1, 0)], matrix[(2, 0)]).magnitude();
    let sy = Vec3::new(matrix[(0, 1)], matrix[(1, 1)], matrix[(2, 1)]).magnitude();
    let sz = Vec3::new(matrix[(0, 2)], matrix[(1, 2)], matrix[(2, 2)]).magnitude();
    let scale = Vec3::new(sx, sy, sz);

    // Extract rotation (normalize columns)
    let rotation_matrix = nalgebra::Matrix3::new(
        matrix[(0, 0)] / sx,
        matrix[(0, 1)] / sy,
        matrix[(0, 2)] / sz,
        matrix[(1, 0)] / sx,
        matrix[(1, 1)] / sy,
        matrix[(1, 2)] / sz,
        matrix[(2, 0)] / sx,
        matrix[(2, 1)] / sy,
        matrix[(2, 2)] / sz,
    );

    let rotation = UnitQuaternion::from_matrix(&rotation_matrix);

    TransformComponent {
        position,
        rotation,
        scale,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    const EPSILON: f64 = 1e-10;

    #[test]
    fn test_identity_transform() {
        let mut t = Transform::identity();
        assert_eq!(t.position(), Vec3::zeros());
        assert!((t.scale() - Vec3::new(1.0, 1.0, 1.0)).magnitude() < EPSILON);
        assert!(t.matrix().is_identity(EPSILON));
    }

    #[test]
    fn test_translation() {
        let mut t = Transform::from_position(1.0, 2.0, 3.0);
        let p = Position3::origin();
        let transformed = t.transform_point(p);

        assert!((transformed.x - 1.0).abs() < EPSILON);
        assert!((transformed.y - 2.0).abs() < EPSILON);
        assert!((transformed.z - 3.0).abs() < EPSILON);
    }

    #[test]
    fn test_rotation() {
        let mut t = Transform::from_axis_angle(Vec3::z(), PI / 2.0);
        let p = Position3::new(1.0, 0.0, 0.0);
        let transformed = t.transform_point(p);

        assert!(transformed.x.abs() < EPSILON);
        assert!((transformed.y - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_scale() {
        let mut t = Transform::from_scale_xyz(2.0, 3.0, 4.0);
        let p = Position3::new(1.0, 1.0, 1.0);
        let transformed = t.transform_point(p);

        assert!((transformed.x - 2.0).abs() < EPSILON);
        assert!((transformed.y - 3.0).abs() < EPSILON);
        assert!((transformed.z - 4.0).abs() < EPSILON);
    }

    #[test]
    fn test_matrix_decomposition() {
        let original = TransformComponent {
            position: Vec3::new(1.0, 2.0, 3.0),
            rotation: UnitQuaternion::from_euler_angles(0.1, 0.2, 0.3),
            scale: Vec3::new(1.5, 2.0, 2.5),
        };

        let matrix = compose_matrix(&original);
        let decomposed = decompose_matrix(&matrix);

        assert!((original.position - decomposed.position).magnitude() < EPSILON);
        assert!((original.scale - decomposed.scale).magnitude() < EPSILON);
        // Quaternions can have sign flip, so compare the rotation effect
        let test_vec = Vec3::new(1.0, 0.0, 0.0);
        let orig_rotated = original.rotation * test_vec;
        let decomp_rotated = decomposed.rotation * test_vec;
        assert!((orig_rotated - decomp_rotated).magnitude() < EPSILON);
    }

    #[test]
    fn test_lerp() {
        let t1 = Transform::from_position(0.0, 0.0, 0.0);
        let t2 = Transform::from_position(10.0, 20.0, 30.0);
        let mid = t1.lerp(&t2, 0.5);

        assert!((mid.position().x - 5.0).abs() < EPSILON);
        assert!((mid.position().y - 10.0).abs() < EPSILON);
        assert!((mid.position().z - 15.0).abs() < EPSILON);
    }
}
