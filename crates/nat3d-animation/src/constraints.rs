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

//! Animation constraint system.
//!
//! High-level constraints for procedural animation control.

use nalgebra::{Point3, UnitQuaternion, Vector3};
use std::collections::HashMap;

use crate::rigging::bone::{BoneId, BoneTransform};

/// Constraint evaluation context.
#[derive(Debug, Clone)]
pub struct ConstraintContext {
    /// Current transform.
    pub current: BoneTransform,
    /// Target transforms by bone ID.
    pub targets: HashMap<BoneId, BoneTransform>,
    /// Current time.
    pub time: f64,
    /// Custom data.
    pub custom_data: HashMap<String, f64>,
}

impl ConstraintContext {
    /// Create a new context.
    pub fn new(current: BoneTransform) -> Self {
        Self {
            current,
            targets: HashMap::new(),
            time: 0.0,
            custom_data: HashMap::new(),
        }
    }

    /// Add a target transform.
    pub fn add_target(&mut self, bone_id: BoneId, transform: BoneTransform) {
        self.targets.insert(bone_id, transform);
    }

    /// Get target transform.
    pub fn get_target(&self, bone_id: BoneId) -> Option<&BoneTransform> {
        self.targets.get(&bone_id)
    }

    /// Set custom data.
    pub fn set_custom(&mut self, key: impl Into<String>, value: f64) {
        self.custom_data.insert(key.into(), value);
    }

    /// Get custom data.
    pub fn get_custom(&self, key: &str) -> f64 {
        self.custom_data.get(key).copied().unwrap_or(0.0)
    }
}

/// Constraint trait.
pub trait Constraint {
    /// Evaluate the constraint.
    fn evaluate(&self, context: &ConstraintContext) -> BoneTransform;

    /// Get constraint influence weight.
    fn influence(&self) -> f64;
}

/// Constraint stack for applying multiple constraints in order.
pub struct ConstraintStack {
    constraints: Vec<Box<dyn Constraint>>,
}

impl ConstraintStack {
    /// Create a new constraint stack.
    pub fn new() -> Self {
        Self {
            constraints: Vec::new(),
        }
    }

    /// Add a constraint.
    pub fn add<C: Constraint + 'static>(&mut self, constraint: C) {
        self.constraints.push(Box::new(constraint));
    }

    /// Evaluate all constraints.
    pub fn evaluate(&self, mut context: ConstraintContext) -> BoneTransform {
        let mut transform = context.current;

        for constraint in &self.constraints {
            context.current = transform;
            let constrained = constraint.evaluate(&context);
            let influence = constraint.influence();

            // Blend with influence
            transform = transform.lerp(&constrained, influence);
        }

        transform
    }
}

impl Default for ConstraintStack {
    fn default() -> Self {
        Self::new()
    }
}

/// Copy location constraint.
#[derive(Debug, Clone)]
pub struct CopyLocationConstraint {
    /// Target bone.
    pub target: BoneId,
    /// Influence weight.
    pub influence: f64,
    /// Use X axis.
    pub use_x: bool,
    /// Use Y axis.
    pub use_y: bool,
    /// Use Z axis.
    pub use_z: bool,
    /// Offset to add.
    pub offset: Vector3<f64>,
}

impl CopyLocationConstraint {
    /// Create a new copy location constraint.
    pub fn new(target: BoneId) -> Self {
        Self {
            target,
            influence: 1.0,
            use_x: true,
            use_y: true,
            use_z: true,
            offset: Vector3::zeros(),
        }
    }
}

impl Constraint for CopyLocationConstraint {
    fn evaluate(&self, context: &ConstraintContext) -> BoneTransform {
        let mut result = context.current;

        if let Some(target_tf) = context.get_target(self.target) {
            let target_pos = target_tf.position + self.offset;

            if self.use_x {
                result.position.x = target_pos.x;
            }
            if self.use_y {
                result.position.y = target_pos.y;
            }
            if self.use_z {
                result.position.z = target_pos.z;
            }
        }

        result
    }

    fn influence(&self) -> f64 {
        self.influence
    }
}

/// Copy rotation constraint.
#[derive(Debug, Clone)]
pub struct CopyRotationConstraint {
    /// Target bone.
    pub target: BoneId,
    /// Influence weight.
    pub influence: f64,
    /// Use X axis.
    pub use_x: bool,
    /// Use Y axis.
    pub use_y: bool,
    /// Use Z axis.
    pub use_z: bool,
    /// Invert rotation.
    pub invert: bool,
    /// Mix mode.
    pub mix_mode: RotationMixMode,
}

/// Rotation mix mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotationMixMode {
    /// Replace rotation.
    Replace,
    /// Add rotation.
    Add,
    /// Before original rotation.
    Before,
    /// After original rotation.
    After,
}

impl CopyRotationConstraint {
    /// Create a new copy rotation constraint.
    pub fn new(target: BoneId) -> Self {
        Self {
            target,
            influence: 1.0,
            use_x: true,
            use_y: true,
            use_z: true,
            invert: false,
            mix_mode: RotationMixMode::Replace,
        }
    }
}

impl Constraint for CopyRotationConstraint {
    fn evaluate(&self, context: &ConstraintContext) -> BoneTransform {
        let mut result = context.current;

        if let Some(target_tf) = context.get_target(self.target) {
            let mut target_rot = target_tf.rotation;

            if self.invert {
                target_rot = target_rot.inverse();
            }

            // Filter axes
            if !self.use_x || !self.use_y || !self.use_z {
                let (roll, pitch, yaw) = target_rot.euler_angles();
                let (cur_roll, cur_pitch, cur_yaw) = result.rotation.euler_angles();

                let new_roll = if self.use_x { roll } else { cur_roll };
                let new_pitch = if self.use_y { pitch } else { cur_pitch };
                let new_yaw = if self.use_z { yaw } else { cur_yaw };

                target_rot = UnitQuaternion::from_euler_angles(new_roll, new_pitch, new_yaw);
            }

            result.rotation = match self.mix_mode {
                RotationMixMode::Replace => target_rot,
                RotationMixMode::Add => result.rotation * target_rot,
                RotationMixMode::Before => target_rot * result.rotation,
                RotationMixMode::After => result.rotation * target_rot,
            };
        }

        result
    }

    fn influence(&self) -> f64 {
        self.influence
    }
}

/// Copy scale constraint.
#[derive(Debug, Clone)]
pub struct CopyScaleConstraint {
    /// Target bone.
    pub target: BoneId,
    /// Influence weight.
    pub influence: f64,
    /// Use X axis.
    pub use_x: bool,
    /// Use Y axis.
    pub use_y: bool,
    /// Use Z axis.
    pub use_z: bool,
}

impl CopyScaleConstraint {
    /// Create a new copy scale constraint.
    pub fn new(target: BoneId) -> Self {
        Self {
            target,
            influence: 1.0,
            use_x: true,
            use_y: true,
            use_z: true,
        }
    }
}

impl Constraint for CopyScaleConstraint {
    fn evaluate(&self, context: &ConstraintContext) -> BoneTransform {
        let mut result = context.current;

        if let Some(target_tf) = context.get_target(self.target) {
            if self.use_x {
                result.scale.x = target_tf.scale.x;
            }
            if self.use_y {
                result.scale.y = target_tf.scale.y;
            }
            if self.use_z {
                result.scale.z = target_tf.scale.z;
            }
        }

        result
    }

    fn influence(&self) -> f64 {
        self.influence
    }
}

/// Track-to constraint (aim at target).
#[derive(Debug, Clone)]
pub struct TrackToConstraint {
    /// Target bone.
    pub target: BoneId,
    /// Influence weight.
    pub influence: f64,
    /// Track axis.
    pub track_axis: Axis,
    /// Up axis.
    pub up_axis: Axis,
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

impl TrackToConstraint {
    /// Create a new track-to constraint.
    pub fn new(target: BoneId) -> Self {
        Self {
            target,
            influence: 1.0,
            track_axis: Axis::PosY,
            up_axis: Axis::PosZ,
        }
    }
}

impl Constraint for TrackToConstraint {
    fn evaluate(&self, context: &ConstraintContext) -> BoneTransform {
        let mut result = context.current;

        if let Some(target_tf) = context.get_target(self.target) {
            let from_pos = Point3::from(result.position);
            let to_pos = Point3::from(target_tf.position);
            let direction = (to_pos - from_pos).normalize();

            if direction.magnitude() > 1e-6 {
                let track_dir = self.track_axis.direction();
                let up_dir = self.up_axis.direction();

                if let Some(rot) = compute_look_at_rotation(&track_dir, &direction, &up_dir) {
                    result.rotation = rot;
                }
            }
        }

        result
    }

    fn influence(&self) -> f64 {
        self.influence
    }
}

/// Damped track constraint (smoothly aim at target).
#[derive(Debug, Clone)]
pub struct DampedTrackConstraint {
    /// Target bone.
    pub target: BoneId,
    /// Influence weight.
    pub influence: f64,
    /// Track axis.
    pub track_axis: Axis,
}

impl DampedTrackConstraint {
    /// Create a new damped track constraint.
    pub fn new(target: BoneId) -> Self {
        Self {
            target,
            influence: 1.0,
            track_axis: Axis::PosY,
        }
    }
}

impl Constraint for DampedTrackConstraint {
    fn evaluate(&self, context: &ConstraintContext) -> BoneTransform {
        let mut result = context.current;

        if let Some(target_tf) = context.get_target(self.target) {
            let from_pos = Point3::from(result.position);
            let to_pos = Point3::from(target_tf.position);
            let direction = (to_pos - from_pos).normalize();

            if direction.magnitude() > 1e-6 {
                let track_dir = result.rotation * self.track_axis.direction();

                if let Some(rot) = UnitQuaternion::rotation_between(&track_dir, &direction) {
                    result.rotation = rot * result.rotation;
                }
            }
        }

        result
    }

    fn influence(&self) -> f64 {
        self.influence
    }
}

/// Limit location constraint.
#[derive(Debug, Clone)]
pub struct LimitLocationConstraint {
    /// Minimum bounds.
    pub min: Vector3<f64>,
    /// Maximum bounds.
    pub max: Vector3<f64>,
    /// Influence weight.
    pub influence: f64,
}

impl LimitLocationConstraint {
    /// Create a new limit location constraint.
    pub fn new(min: Vector3<f64>, max: Vector3<f64>) -> Self {
        Self {
            min,
            max,
            influence: 1.0,
        }
    }
}

impl Constraint for LimitLocationConstraint {
    fn evaluate(&self, context: &ConstraintContext) -> BoneTransform {
        let mut result = context.current;

        result.position.x = result.position.x.clamp(self.min.x, self.max.x);
        result.position.y = result.position.y.clamp(self.min.y, self.max.y);
        result.position.z = result.position.z.clamp(self.min.z, self.max.z);

        result
    }

    fn influence(&self) -> f64 {
        self.influence
    }
}

/// Limit rotation constraint.
#[derive(Debug, Clone)]
pub struct LimitRotationConstraint {
    /// Minimum angles (radians).
    pub min: Vector3<f64>,
    /// Maximum angles (radians).
    pub max: Vector3<f64>,
    /// Influence weight.
    pub influence: f64,
}

impl LimitRotationConstraint {
    /// Create a new limit rotation constraint.
    pub fn new(min: Vector3<f64>, max: Vector3<f64>) -> Self {
        Self {
            min,
            max,
            influence: 1.0,
        }
    }
}

impl Constraint for LimitRotationConstraint {
    fn evaluate(&self, context: &ConstraintContext) -> BoneTransform {
        let mut result = context.current;

        let (roll, pitch, yaw) = result.rotation.euler_angles();

        let clamped_roll = roll.clamp(self.min.x, self.max.x);
        let clamped_pitch = pitch.clamp(self.min.y, self.max.y);
        let clamped_yaw = yaw.clamp(self.min.z, self.max.z);

        result.rotation =
            UnitQuaternion::from_euler_angles(clamped_roll, clamped_pitch, clamped_yaw);

        result
    }

    fn influence(&self) -> f64 {
        self.influence
    }
}

/// Limit scale constraint.
#[derive(Debug, Clone)]
pub struct LimitScaleConstraint {
    /// Minimum scale.
    pub min: Vector3<f64>,
    /// Maximum scale.
    pub max: Vector3<f64>,
    /// Influence weight.
    pub influence: f64,
}

impl LimitScaleConstraint {
    /// Create a new limit scale constraint.
    pub fn new(min: Vector3<f64>, max: Vector3<f64>) -> Self {
        Self {
            min,
            max,
            influence: 1.0,
        }
    }
}

impl Constraint for LimitScaleConstraint {
    fn evaluate(&self, context: &ConstraintContext) -> BoneTransform {
        let mut result = context.current;

        result.scale.x = result.scale.x.clamp(self.min.x, self.max.x);
        result.scale.y = result.scale.y.clamp(self.min.y, self.max.y);
        result.scale.z = result.scale.z.clamp(self.min.z, self.max.z);

        result
    }

    fn influence(&self) -> f64 {
        self.influence
    }
}

/// Floor constraint (prevent passing through floor).
#[derive(Debug, Clone)]
pub struct FloorConstraint {
    /// Floor height.
    pub floor_height: f64,
    /// Influence weight.
    pub influence: f64,
}

impl FloorConstraint {
    /// Create a new floor constraint.
    pub fn new(floor_height: f64) -> Self {
        Self {
            floor_height,
            influence: 1.0,
        }
    }
}

impl Constraint for FloorConstraint {
    fn evaluate(&self, context: &ConstraintContext) -> BoneTransform {
        let mut result = context.current;

        if result.position.y < self.floor_height {
            result.position.y = self.floor_height;
        }

        result
    }

    fn influence(&self) -> f64 {
        self.influence
    }
}

/// Compute look-at rotation.
fn compute_look_at_rotation(
    from: &Vector3<f64>,
    to: &Vector3<f64>,
    up: &Vector3<f64>,
) -> Option<UnitQuaternion<f64>> {
    let dot = from.dot(to);

    if dot > 0.9999 {
        return Some(UnitQuaternion::identity());
    }

    if dot < -0.9999 {
        // Vectors are opposite
        return Some(UnitQuaternion::from_axis_angle(
            &nalgebra::Unit::new_normalize(*up),
            std::f64::consts::PI,
        ));
    }

    let axis = from.cross(to);
    let angle = dot.acos();

    Some(UnitQuaternion::from_axis_angle(
        &nalgebra::Unit::new_normalize(axis),
        angle,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constraint_context() {
        let mut context = ConstraintContext::new(BoneTransform::identity());
        context.add_target(BoneId(1), BoneTransform::identity());
        context.set_custom("weight", 0.5);

        assert!(context.get_target(BoneId(1)).is_some());
        assert_eq!(context.get_custom("weight"), 0.5);
    }

    #[test]
    fn test_copy_location_constraint() {
        let constraint = CopyLocationConstraint::new(BoneId(1));

        let mut context = ConstraintContext::new(BoneTransform::identity());
        context.add_target(
            BoneId(1),
            BoneTransform {
                position: Vector3::new(1.0, 2.0, 3.0),
                rotation: UnitQuaternion::identity(),
                scale: Vector3::new(1.0, 1.0, 1.0),
            },
        );

        let result = constraint.evaluate(&context);
        assert_eq!(result.position, Vector3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn test_limit_location_constraint() {
        let constraint = LimitLocationConstraint::new(
            Vector3::new(-1.0, -1.0, -1.0),
            Vector3::new(1.0, 1.0, 1.0),
        );

        let context = ConstraintContext::new(BoneTransform {
            position: Vector3::new(2.0, -2.0, 0.5),
            rotation: UnitQuaternion::identity(),
            scale: Vector3::new(1.0, 1.0, 1.0),
        });

        let result = constraint.evaluate(&context);
        assert_eq!(result.position.x, 1.0);
        assert_eq!(result.position.y, -1.0);
        assert_eq!(result.position.z, 0.5);
    }

    #[test]
    fn test_constraint_stack() {
        let mut stack = ConstraintStack::new();
        stack.add(LimitLocationConstraint::new(
            Vector3::new(-10.0, 0.0, -10.0),
            Vector3::new(10.0, 10.0, 10.0),
        ));

        let context = ConstraintContext::new(BoneTransform {
            position: Vector3::new(0.0, -5.0, 0.0),
            rotation: UnitQuaternion::identity(),
            scale: Vector3::new(1.0, 1.0, 1.0),
        });

        let result = stack.evaluate(context);
        assert_eq!(result.position.y, 0.0);
    }
}
