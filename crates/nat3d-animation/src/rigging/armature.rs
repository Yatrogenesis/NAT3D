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

//! Armature/skeleton system.
//!
//! Complete skeleton management with bone hierarchy, pose evaluation, and animation.

use nalgebra::{Matrix4, Point3, UnitQuaternion, Vector3};
use std::collections::HashMap;

use super::bone::{Bone, BoneConstraint, BoneId, BoneTransform};

/// Armature (skeleton) containing bones.
#[derive(Debug, Clone)]
pub struct Armature {
    /// Armature name.
    pub name: String,
    /// All bones in the armature.
    bones: Vec<Bone>,
    /// Bone name to index mapping.
    bone_map: HashMap<String, usize>,
    /// Root bone indices.
    roots: Vec<usize>,
    /// Cached world transforms.
    world_transforms: Vec<Matrix4<f64>>,
    /// Cached inverse bind matrices.
    inverse_bind_matrices: Vec<Matrix4<f64>>,
    /// Is transform cache valid.
    cache_valid: bool,
    /// Armature transform.
    pub transform: Matrix4<f64>,
    /// Rest pose (bind pose).
    rest_pose: Vec<BoneTransform>,
}

impl Armature {
    /// Create a new armature.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            bones: Vec::new(),
            bone_map: HashMap::new(),
            roots: Vec::new(),
            world_transforms: Vec::new(),
            inverse_bind_matrices: Vec::new(),
            cache_valid: false,
            transform: Matrix4::identity(),
            rest_pose: Vec::new(),
        }
    }

    /// Add a bone to the armature.
    pub fn add_bone(&mut self, mut bone: Bone) -> BoneId {
        let index = self.bones.len();
        bone.id = BoneId(index as u32);

        // Update parent's children list
        if bone.parent.is_valid() {
            let parent_idx = bone.parent.0 as usize;
            if parent_idx < self.bones.len() {
                self.bones[parent_idx].add_child(bone.id);
            }
        } else {
            self.roots.push(index);
        }

        self.bone_map.insert(bone.name.clone(), index);
        self.bones.push(bone);
        self.world_transforms.push(Matrix4::identity());
        self.inverse_bind_matrices.push(Matrix4::identity());
        self.rest_pose.push(BoneTransform::identity());
        self.cache_valid = false;

        BoneId(index as u32)
    }

    /// Get bone by ID.
    pub fn get_bone(&self, id: BoneId) -> Option<&Bone> {
        self.bones.get(id.0 as usize)
    }

    /// Get mutable bone by ID.
    pub fn get_bone_mut(&mut self, id: BoneId) -> Option<&mut Bone> {
        self.cache_valid = false;
        self.bones.get_mut(id.0 as usize)
    }

    /// Get bone by name.
    pub fn get_bone_by_name(&self, name: &str) -> Option<&Bone> {
        self.bone_map.get(name).and_then(|&idx| self.bones.get(idx))
    }

    /// Get bone ID by name.
    pub fn get_bone_id(&self, name: &str) -> Option<BoneId> {
        self.bone_map.get(name).map(|&idx| BoneId(idx as u32))
    }

    /// Get bone count.
    pub fn bone_count(&self) -> usize {
        self.bones.len()
    }

    /// Iterate over all bones.
    pub fn bones(&self) -> impl Iterator<Item = &Bone> {
        self.bones.iter()
    }

    /// Get root bones.
    pub fn root_bones(&self) -> impl Iterator<Item = &Bone> {
        self.roots.iter().filter_map(|&idx| self.bones.get(idx))
    }

    /// Update bone hierarchy and compute transforms.
    pub fn update(&mut self) {
        if self.cache_valid {
            return;
        }

        // First, apply constraints
        self.apply_constraints();

        // Then compute world transforms
        for &root_idx in &self.roots.clone() {
            self.compute_world_transform_recursive(root_idx, self.transform);
        }

        self.cache_valid = true;
    }

    fn compute_world_transform_recursive(&mut self, bone_idx: usize, parent_world: Matrix4<f64>) {
        let bone = &self.bones[bone_idx];
        let local = bone.pose.to_matrix();
        let world = parent_world * local;

        self.world_transforms[bone_idx] = world;

        let children = bone.children.clone();
        for child_id in children {
            self.compute_world_transform_recursive(child_id.0 as usize, world);
        }
    }

    /// Get world transform for a bone.
    pub fn world_transform(&self, id: BoneId) -> Option<Matrix4<f64>> {
        self.world_transforms.get(id.0 as usize).copied()
    }

    /// Get all world transforms for skinning.
    pub fn world_transforms(&self) -> &[Matrix4<f64>] {
        &self.world_transforms
    }

    /// Compute and cache bind pose.
    pub fn compute_bind_pose(&mut self) {
        // Store current pose as rest pose
        for (i, bone) in self.bones.iter().enumerate() {
            self.rest_pose[i] = bone.bind_pose;
        }

        // Compute inverse bind matrices
        self.update();

        for i in 0..self.bones.len() {
            if let Some(inv) = self.world_transforms[i].try_inverse() {
                self.inverse_bind_matrices[i] = inv;
            }
        }
    }

    /// Get inverse bind matrices for skinning.
    pub fn inverse_bind_matrices(&self) -> &[Matrix4<f64>] {
        &self.inverse_bind_matrices
    }

    /// Get skinning matrices (world * inverse_bind).
    pub fn skinning_matrices(&self) -> Vec<Matrix4<f64>> {
        self.world_transforms
            .iter()
            .zip(self.inverse_bind_matrices.iter())
            .map(|(world, inv_bind)| world * inv_bind)
            .collect()
    }

    /// Reset all bones to bind pose.
    pub fn reset_pose(&mut self) {
        for bone in &mut self.bones {
            bone.reset_pose();
        }
        self.cache_valid = false;
    }

    /// Set bone pose.
    pub fn set_bone_pose(&mut self, id: BoneId, transform: BoneTransform) {
        if let Some(bone) = self.bones.get_mut(id.0 as usize) {
            bone.pose = transform;
            self.cache_valid = false;
        }
    }

    /// Get bone pose.
    pub fn get_bone_pose(&self, id: BoneId) -> Option<BoneTransform> {
        self.bones.get(id.0 as usize).map(|b| b.pose)
    }

    /// Apply all bone constraints.
    fn apply_constraints(&mut self) {
        // Process bones in hierarchy order
        for &root_idx in &self.roots.clone() {
            self.apply_constraints_recursive(root_idx);
        }
    }

    fn apply_constraints_recursive(&mut self, bone_idx: usize) {
        // Apply constraints for this bone
        let constraints = self.bones[bone_idx].constraints.clone();
        for constraint in constraints {
            self.apply_constraint(bone_idx, &constraint);
        }

        // Process children
        let children = self.bones[bone_idx].children.clone();
        for child_id in children {
            self.apply_constraints_recursive(child_id.0 as usize);
        }
    }

    fn apply_constraint(&mut self, bone_idx: usize, constraint: &BoneConstraint) {
        match constraint {
            BoneConstraint::CopyLocation {
                target,
                influence,
                use_x,
                use_y,
                use_z,
                invert_x,
                invert_y,
                invert_z,
            } => {
                if let Some(target_bone) = self.bones.get(target.0 as usize) {
                    let target_pos = target_bone.pose.position;
                    let bone = &mut self.bones[bone_idx];

                    let mut delta = Vector3::zeros();
                    if *use_x {
                        delta.x = if *invert_x {
                            -target_pos.x
                        } else {
                            target_pos.x
                        };
                    }
                    if *use_y {
                        delta.y = if *invert_y {
                            -target_pos.y
                        } else {
                            target_pos.y
                        };
                    }
                    if *use_z {
                        delta.z = if *invert_z {
                            -target_pos.z
                        } else {
                            target_pos.z
                        };
                    }

                    bone.pose.position = bone.pose.position.lerp(&delta, *influence);
                }
            }
            BoneConstraint::CopyRotation {
                target,
                influence,
                use_x,
                use_y,
                use_z,
            } => {
                if let Some(target_bone) = self.bones.get(target.0 as usize) {
                    let target_rot = target_bone.pose.rotation;
                    let bone = &mut self.bones[bone_idx];

                    // Extract Euler angles and selectively copy
                    let (roll, pitch, yaw) = target_rot.euler_angles();
                    let (cur_roll, cur_pitch, cur_yaw) = bone.pose.rotation.euler_angles();

                    let new_roll = if *use_x { roll } else { cur_roll };
                    let new_pitch = if *use_y { pitch } else { cur_pitch };
                    let new_yaw = if *use_z { yaw } else { cur_yaw };

                    let new_rot = UnitQuaternion::from_euler_angles(new_roll, new_pitch, new_yaw);
                    bone.pose.rotation = bone.pose.rotation.slerp(&new_rot, *influence);
                }
            }
            BoneConstraint::CopyScale {
                target,
                influence,
                use_x,
                use_y,
                use_z,
            } => {
                if let Some(target_bone) = self.bones.get(target.0 as usize) {
                    let target_scale = target_bone.pose.scale;
                    let bone = &mut self.bones[bone_idx];

                    let mut new_scale = bone.pose.scale;
                    if *use_x {
                        new_scale.x =
                            bone.pose.scale.x + (target_scale.x - bone.pose.scale.x) * influence;
                    }
                    if *use_y {
                        new_scale.y =
                            bone.pose.scale.y + (target_scale.y - bone.pose.scale.y) * influence;
                    }
                    if *use_z {
                        new_scale.z =
                            bone.pose.scale.z + (target_scale.z - bone.pose.scale.z) * influence;
                    }

                    bone.pose.scale = new_scale;
                }
            }
            BoneConstraint::LimitLocation {
                min,
                max,
                use_min_x,
                use_min_y,
                use_min_z,
                use_max_x,
                use_max_y,
                use_max_z,
                influence,
            } => {
                let bone = &mut self.bones[bone_idx];
                let mut limited = bone.pose.position;

                if *use_min_x {
                    limited.x = limited.x.max(min.x);
                }
                if *use_max_x {
                    limited.x = limited.x.min(max.x);
                }
                if *use_min_y {
                    limited.y = limited.y.max(min.y);
                }
                if *use_max_y {
                    limited.y = limited.y.min(max.y);
                }
                if *use_min_z {
                    limited.z = limited.z.max(min.z);
                }
                if *use_max_z {
                    limited.z = limited.z.min(max.z);
                }

                bone.pose.position = bone.pose.position.lerp(&limited, *influence);
            }
            BoneConstraint::LimitRotation {
                min,
                max,
                use_x,
                use_y,
                use_z,
                influence,
            } => {
                let bone = &mut self.bones[bone_idx];
                let (roll, pitch, yaw) = bone.pose.rotation.euler_angles();

                let new_roll = if *use_x {
                    roll.clamp(min.x, max.x)
                } else {
                    roll
                };
                let new_pitch = if *use_y {
                    pitch.clamp(min.y, max.y)
                } else {
                    pitch
                };
                let new_yaw = if *use_z { yaw.clamp(min.z, max.z) } else { yaw };

                let limited_rot = UnitQuaternion::from_euler_angles(new_roll, new_pitch, new_yaw);
                bone.pose.rotation = bone.pose.rotation.slerp(&limited_rot, *influence);
            }
            BoneConstraint::TrackTo {
                target,
                track_axis,
                up_axis,
                influence,
            } => {
                if let Some(target_bone) = self.bones.get(target.0 as usize) {
                    let target_pos = Point3::from(target_bone.pose.position);
                    let bone = &mut self.bones[bone_idx];
                    let bone_pos = Point3::from(bone.pose.position);

                    let direction = (target_pos - bone_pos).normalize();
                    let track_dir = track_axis.direction();
                    let up_dir = up_axis.direction();

                    // Compute rotation to align track_axis with direction
                    if let Some(rot) = rotation_between_vectors(&track_dir, &direction, &up_dir) {
                        bone.pose.rotation = bone.pose.rotation.slerp(&rot, *influence);
                    }
                }
            }
            BoneConstraint::DampedTrack {
                target,
                track_axis,
                influence,
            } => {
                if let Some(target_bone) = self.bones.get(target.0 as usize) {
                    let target_pos = Point3::from(target_bone.pose.position);
                    let bone = &mut self.bones[bone_idx];
                    let bone_pos = Point3::from(bone.pose.position);

                    let direction = (target_pos - bone_pos).normalize();
                    let track_dir = bone.pose.rotation * track_axis.direction();

                    if let Some(rot) = UnitQuaternion::rotation_between(&track_dir, &direction) {
                        let new_rot = rot * bone.pose.rotation;
                        bone.pose.rotation = bone.pose.rotation.slerp(&new_rot, *influence);
                    }
                }
            }
            BoneConstraint::Ik { .. } => {
                // IK is handled separately by the IK solver
            }
        }
    }

    /// Create a pose snapshot.
    pub fn capture_pose(&self) -> Pose {
        Pose {
            transforms: self.bones.iter().map(|b| b.pose).collect(),
        }
    }

    /// Apply a pose snapshot.
    pub fn apply_pose(&mut self, pose: &Pose) {
        for (i, transform) in pose.transforms.iter().enumerate() {
            if i < self.bones.len() {
                self.bones[i].pose = *transform;
            }
        }
        self.cache_valid = false;
    }

    /// Blend between two poses.
    pub fn blend_poses(pose_a: &Pose, pose_b: &Pose, t: f64) -> Pose {
        let transforms = pose_a
            .transforms
            .iter()
            .zip(pose_b.transforms.iter())
            .map(|(a, b)| a.lerp(b, t))
            .collect();

        Pose { transforms }
    }

    /// Get bone chain from end to root (for IK).
    pub fn get_bone_chain(&self, end_bone: BoneId, length: usize) -> Vec<BoneId> {
        let mut chain = Vec::new();
        let mut current = Some(end_bone);

        while let Some(bone_id) = current {
            if chain.len() >= length {
                break;
            }
            chain.push(bone_id);
            current = self
                .bones
                .get(bone_id.0 as usize)
                .filter(|b| b.parent.is_valid())
                .map(|b| b.parent);
        }

        chain
    }

    /// Invalidate transform cache.
    pub fn invalidate_cache(&mut self) {
        self.cache_valid = false;
    }
}

impl Default for Armature {
    fn default() -> Self {
        Self::new("Armature")
    }
}

/// A captured pose state.
#[derive(Debug, Clone)]
pub struct Pose {
    /// Bone transforms.
    pub transforms: Vec<BoneTransform>,
}

impl Pose {
    /// Create an empty pose.
    pub fn new() -> Self {
        Self {
            transforms: Vec::new(),
        }
    }

    /// Create a pose with identity transforms.
    pub fn identity(bone_count: usize) -> Self {
        Self {
            transforms: vec![BoneTransform::identity(); bone_count],
        }
    }
}

impl Default for Pose {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute rotation between two vectors with up hint.
fn rotation_between_vectors(
    from: &Vector3<f64>,
    to: &Vector3<f64>,
    up: &Vector3<f64>,
) -> Option<UnitQuaternion<f64>> {
    let dot = from.dot(to);

    if dot > 0.9999 {
        return Some(UnitQuaternion::identity());
    }

    if dot < -0.9999 {
        // Vectors are opposite, rotate 180 degrees around up
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

/// Builder for creating armatures.
#[derive(Debug)]
pub struct ArmatureBuilder {
    armature: Armature,
    current_parent: BoneId,
}

impl ArmatureBuilder {
    /// Create a new armature builder.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            armature: Armature::new(name),
            current_parent: BoneId::INVALID,
        }
    }

    /// Add a root bone.
    pub fn add_root(
        mut self,
        name: impl Into<String>,
        head: Point3<f64>,
        tail: Point3<f64>,
    ) -> Self {
        let mut bone = Bone::new(BoneId(0), name);
        bone.head = head;
        bone.tail = tail;
        self.current_parent = self.armature.add_bone(bone);
        self
    }

    /// Add a child bone to the current bone.
    pub fn add_child(mut self, name: impl Into<String>, tail: Point3<f64>) -> Self {
        let parent_idx = self.current_parent.0 as usize;
        let head = if parent_idx < self.armature.bones.len() {
            self.armature.bones[parent_idx].tail
        } else {
            Point3::origin()
        };

        let mut bone = Bone::new(BoneId(0), name);
        bone.head = head;
        bone.tail = tail;
        bone.parent = self.current_parent;
        bone.connected = true;
        self.current_parent = self.armature.add_bone(bone);
        self
    }

    /// Add a sibling bone (same parent as current).
    pub fn add_sibling(
        mut self,
        name: impl Into<String>,
        head: Point3<f64>,
        tail: Point3<f64>,
    ) -> Self {
        let parent = if let Some(bone) = self.armature.bones.get(self.current_parent.0 as usize) {
            bone.parent
        } else {
            BoneId::INVALID
        };

        let mut bone = Bone::new(BoneId(0), name);
        bone.head = head;
        bone.tail = tail;
        bone.parent = parent;
        self.current_parent = self.armature.add_bone(bone);
        self
    }

    /// Move to parent bone.
    pub fn parent(mut self) -> Self {
        if let Some(bone) = self.armature.bones.get(self.current_parent.0 as usize) {
            if bone.parent.is_valid() {
                self.current_parent = bone.parent;
            }
        }
        self
    }

    /// Move to a bone by name.
    pub fn goto(mut self, name: &str) -> Self {
        if let Some(&idx) = self.armature.bone_map.get(name) {
            self.current_parent = BoneId(idx as u32);
        }
        self
    }

    /// Build the armature.
    pub fn build(mut self) -> Armature {
        self.armature.compute_bind_pose();
        self.armature
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_armature_creation() {
        let armature = Armature::new("Test");
        assert_eq!(armature.name, "Test");
        assert_eq!(armature.bone_count(), 0);
    }

    #[test]
    fn test_add_bones() {
        let mut armature = Armature::new("Test");

        let root = Bone::new(BoneId(0), "Root");
        let root_id = armature.add_bone(root);

        let mut child = Bone::new(BoneId(0), "Child");
        child.parent = root_id;
        armature.add_bone(child);

        assert_eq!(armature.bone_count(), 2);
        assert_eq!(armature.roots.len(), 1);
    }

    #[test]
    fn test_armature_builder() {
        let armature = ArmatureBuilder::new("Spine")
            .add_root(
                "Hip",
                Point3::new(0.0, 1.0, 0.0),
                Point3::new(0.0, 1.2, 0.0),
            )
            .add_child("Spine1", Point3::new(0.0, 1.4, 0.0))
            .add_child("Spine2", Point3::new(0.0, 1.6, 0.0))
            .add_child("Chest", Point3::new(0.0, 1.8, 0.0))
            .build();

        assert_eq!(armature.bone_count(), 4);
    }

    #[test]
    fn test_pose_capture_apply() {
        let mut armature = ArmatureBuilder::new("Test")
            .add_root("Root", Point3::origin(), Point3::new(0.0, 1.0, 0.0))
            .build();

        armature.set_bone_pose(
            BoneId(0),
            BoneTransform {
                position: Vector3::new(1.0, 0.0, 0.0),
                rotation: UnitQuaternion::identity(),
                scale: Vector3::new(1.0, 1.0, 1.0),
            },
        );

        let pose = armature.capture_pose();
        armature.reset_pose();
        armature.apply_pose(&pose);

        let bone_pose = armature.get_bone_pose(BoneId(0)).unwrap();
        assert!((bone_pose.position.x - 1.0).abs() < 1e-10);
    }
}
