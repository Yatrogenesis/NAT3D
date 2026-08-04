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

//! Bones for skeletal animation.
//!
//! Bone hierarchy for rigging and animation.

use nalgebra::{Matrix4, Point3, UnitQuaternion, Vector3};

/// Unique bone identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BoneId(pub u32);

impl BoneId {
    /// Invalid bone ID.
    pub const INVALID: Self = Self(u32::MAX);

    /// Check if bone ID is valid.
    pub fn is_valid(&self) -> bool {
        self.0 != u32::MAX
    }
}

/// A bone in a skeleton.
#[derive(Debug, Clone)]
pub struct Bone {
    /// Bone identifier.
    pub id: BoneId,
    /// Bone name.
    pub name: String,
    /// Parent bone ID.
    pub parent: BoneId,
    /// Child bone IDs.
    pub children: Vec<BoneId>,
    /// Head position in local space.
    pub head: Point3<f64>,
    /// Tail position in local space.
    pub tail: Point3<f64>,
    /// Roll angle around bone axis.
    pub roll: f64,
    /// Bind pose transform (local space).
    pub bind_pose: BoneTransform,
    /// Current pose transform.
    pub pose: BoneTransform,
    /// Bone constraints.
    pub constraints: Vec<BoneConstraint>,
    /// Is bone connected to parent.
    pub connected: bool,
    /// Use inherit rotation from parent.
    pub inherit_rotation: bool,
    /// Use inherit scale from parent.
    pub inherit_scale: bool,
    /// Is bone visible.
    pub visible: bool,
    /// Is bone selectable.
    pub selectable: bool,
}

/// Bone transform (position, rotation, scale).
#[derive(Debug, Clone, Copy)]
pub struct BoneTransform {
    /// Local position.
    pub position: Vector3<f64>,
    /// Local rotation.
    pub rotation: UnitQuaternion<f64>,
    /// Local scale.
    pub scale: Vector3<f64>,
}

impl BoneTransform {
    /// Identity transform.
    pub fn identity() -> Self {
        Self {
            position: Vector3::zeros(),
            rotation: UnitQuaternion::identity(),
            scale: Vector3::new(1.0, 1.0, 1.0),
        }
    }

    /// Convert to 4x4 matrix.
    pub fn to_matrix(&self) -> Matrix4<f64> {
        let translation = Matrix4::new_translation(&self.position);
        let rotation = self.rotation.to_homogeneous();
        let scale = Matrix4::new_nonuniform_scaling(&self.scale);

        translation * rotation * scale
    }

    /// Create from matrix (decompose).
    pub fn from_matrix(matrix: &Matrix4<f64>) -> Self {
        // Extract translation
        let position = Vector3::new(matrix[(0, 3)], matrix[(1, 3)], matrix[(2, 3)]);

        // Extract rotation (assumes orthogonal matrix)
        let rotation_matrix = matrix.fixed_view::<3, 3>(0, 0).into_owned();
        let rotation = UnitQuaternion::from_rotation_matrix(
            &nalgebra::Rotation3::from_matrix_unchecked(rotation_matrix),
        );

        // Extract scale
        let scale = Vector3::new(
            matrix.column(0).xyz().magnitude(),
            matrix.column(1).xyz().magnitude(),
            matrix.column(2).xyz().magnitude(),
        );

        Self {
            position,
            rotation,
            scale,
        }
    }

    /// Interpolate between transforms.
    pub fn lerp(&self, other: &Self, t: f64) -> Self {
        Self {
            position: self.position + (other.position - self.position) * t,
            rotation: self.rotation.slerp(&other.rotation, t),
            scale: self.scale + (other.scale - self.scale) * t,
        }
    }

    /// Combine with another transform.
    pub fn combine(&self, other: &Self) -> Self {
        Self {
            position: self.position + self.rotation * (other.position.component_mul(&self.scale)),
            rotation: self.rotation * other.rotation,
            scale: self.scale.component_mul(&other.scale),
        }
    }

    /// Get inverse transform.
    pub fn inverse(&self) -> Self {
        let inv_rotation = self.rotation.inverse();
        let inv_scale = Vector3::new(1.0 / self.scale.x, 1.0 / self.scale.y, 1.0 / self.scale.z);
        let inv_position = inv_rotation * (-self.position.component_mul(&inv_scale));

        Self {
            position: inv_position,
            rotation: inv_rotation,
            scale: inv_scale,
        }
    }
}

impl Default for BoneTransform {
    fn default() -> Self {
        Self::identity()
    }
}

/// Bone constraint type.
#[derive(Debug, Clone)]
pub enum BoneConstraint {
    /// Copy location from target.
    CopyLocation {
        target: BoneId,
        influence: f64,
        use_x: bool,
        use_y: bool,
        use_z: bool,
        invert_x: bool,
        invert_y: bool,
        invert_z: bool,
    },
    /// Copy rotation from target.
    CopyRotation {
        target: BoneId,
        influence: f64,
        use_x: bool,
        use_y: bool,
        use_z: bool,
    },
    /// Copy scale from target.
    CopyScale {
        target: BoneId,
        influence: f64,
        use_x: bool,
        use_y: bool,
        use_z: bool,
    },
    /// Limit location.
    LimitLocation {
        min: Vector3<f64>,
        max: Vector3<f64>,
        use_min_x: bool,
        use_min_y: bool,
        use_min_z: bool,
        use_max_x: bool,
        use_max_y: bool,
        use_max_z: bool,
        influence: f64,
    },
    /// Limit rotation.
    LimitRotation {
        min: Vector3<f64>,
        max: Vector3<f64>,
        use_x: bool,
        use_y: bool,
        use_z: bool,
        influence: f64,
    },
    /// Track to target.
    TrackTo {
        target: BoneId,
        track_axis: Axis,
        up_axis: Axis,
        influence: f64,
    },
    /// Damped track.
    DampedTrack {
        target: BoneId,
        track_axis: Axis,
        influence: f64,
    },
    /// Inverse kinematics.
    Ik {
        target: BoneId,
        pole_target: Option<BoneId>,
        chain_length: usize,
        iterations: usize,
        influence: f64,
    },
}

/// Axis enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    PosX,
    NegX,
    PosY,
    NegY,
    PosZ,
    NegZ,
}

impl Axis {
    /// Get axis direction vector.
    pub fn direction(&self) -> Vector3<f64> {
        match self {
            Axis::PosX => Vector3::new(1.0, 0.0, 0.0),
            Axis::NegX => Vector3::new(-1.0, 0.0, 0.0),
            Axis::PosY => Vector3::new(0.0, 1.0, 0.0),
            Axis::NegY => Vector3::new(0.0, -1.0, 0.0),
            Axis::PosZ => Vector3::new(0.0, 0.0, 1.0),
            Axis::NegZ => Vector3::new(0.0, 0.0, -1.0),
        }
    }
}

impl Bone {
    /// Create a new bone.
    pub fn new(id: BoneId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            parent: BoneId::INVALID,
            children: Vec::new(),
            head: Point3::origin(),
            tail: Point3::new(0.0, 1.0, 0.0),
            roll: 0.0,
            bind_pose: BoneTransform::identity(),
            pose: BoneTransform::identity(),
            constraints: Vec::new(),
            connected: false,
            inherit_rotation: true,
            inherit_scale: true,
            visible: true,
            selectable: true,
        }
    }

    /// Get bone length.
    pub fn length(&self) -> f64 {
        (self.tail - self.head).magnitude()
    }

    /// Get bone direction (normalized).
    pub fn direction(&self) -> Vector3<f64> {
        (self.tail - self.head).normalize()
    }

    /// Get bone axis matrix (local coordinate system).
    pub fn axis_matrix(&self) -> Matrix4<f64> {
        let y_axis = self.direction();

        // Compute x axis from roll
        let temp = if y_axis.y.abs() > 0.99 {
            Vector3::new(1.0, 0.0, 0.0)
        } else {
            Vector3::new(0.0, 1.0, 0.0)
        };

        let z_axis = y_axis.cross(&temp).normalize();
        let x_axis = z_axis.cross(&y_axis).normalize();

        // Apply roll
        let roll_rotation =
            UnitQuaternion::from_axis_angle(&nalgebra::Unit::new_normalize(y_axis), self.roll);

        let x_axis = roll_rotation * x_axis;
        let z_axis = roll_rotation * z_axis;

        Matrix4::new(
            x_axis.x,
            y_axis.x,
            z_axis.x,
            self.head.x,
            x_axis.y,
            y_axis.y,
            z_axis.y,
            self.head.y,
            x_axis.z,
            y_axis.z,
            z_axis.z,
            self.head.z,
            0.0,
            0.0,
            0.0,
            1.0,
        )
    }

    /// Get world-space transform.
    pub fn world_transform(&self, parent_world: &Matrix4<f64>) -> Matrix4<f64> {
        parent_world * self.pose.to_matrix()
    }

    /// Reset pose to bind pose.
    pub fn reset_pose(&mut self) {
        self.pose = self.bind_pose;
    }

    /// Set pose from matrix.
    pub fn set_pose_matrix(&mut self, matrix: Matrix4<f64>) {
        self.pose = BoneTransform::from_matrix(&matrix);
    }

    /// Add child bone.
    pub fn add_child(&mut self, child_id: BoneId) {
        if !self.children.contains(&child_id) {
            self.children.push(child_id);
        }
    }

    /// Remove child bone.
    pub fn remove_child(&mut self, child_id: BoneId) {
        self.children.retain(|&id| id != child_id);
    }

    /// Add constraint.
    pub fn add_constraint(&mut self, constraint: BoneConstraint) {
        self.constraints.push(constraint);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bone_creation() {
        let bone = Bone::new(BoneId(0), "Root");
        assert_eq!(bone.name, "Root");
        assert_eq!(bone.id.0, 0);
    }

    #[test]
    fn test_bone_length() {
        let mut bone = Bone::new(BoneId(0), "Test");
        bone.head = Point3::new(0.0, 0.0, 0.0);
        bone.tail = Point3::new(0.0, 2.0, 0.0);

        assert!((bone.length() - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_bone_transform() {
        let transform = BoneTransform {
            position: Vector3::new(1.0, 2.0, 3.0),
            rotation: UnitQuaternion::identity(),
            scale: Vector3::new(1.0, 1.0, 1.0),
        };

        let matrix = transform.to_matrix();
        let restored = BoneTransform::from_matrix(&matrix);

        assert!((transform.position - restored.position).magnitude() < 1e-10);
    }

    #[test]
    fn test_transform_combine() {
        let t1 = BoneTransform {
            position: Vector3::new(1.0, 0.0, 0.0),
            rotation: UnitQuaternion::identity(),
            scale: Vector3::new(1.0, 1.0, 1.0),
        };

        let t2 = BoneTransform {
            position: Vector3::new(0.0, 1.0, 0.0),
            rotation: UnitQuaternion::identity(),
            scale: Vector3::new(1.0, 1.0, 1.0),
        };

        let combined = t1.combine(&t2);
        assert!((combined.position.x - 1.0).abs() < 1e-10);
        assert!((combined.position.y - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_bone_id_validity() {
        assert!(!BoneId::INVALID.is_valid());
        assert!(BoneId(0).is_valid());
    }
}
