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

//! Motion retargeting.
//!
//! Retarget animations between different skeletons with different proportions.

use std::collections::HashMap;

use crate::rigging::armature::{Armature, Pose};
use crate::rigging::bone::{Bone, BoneId, BoneTransform};

/// Retargeting mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetargetMode {
    /// Direct copy of rotations (no scale compensation).
    DirectCopy,
    /// Scale compensated retargeting (adjust for bone length differences).
    ScaleCompensated,
    /// IK-based retargeting (more accurate for limbs).
    IkBased,
}

/// Bone mapping between source and target skeletons.
#[derive(Debug, Clone)]
pub struct BoneMapping {
    /// Source bone ID to target bone ID.
    mappings: HashMap<BoneId, BoneId>,
}

impl BoneMapping {
    /// Create a new bone mapping.
    pub fn new() -> Self {
        Self {
            mappings: HashMap::new(),
        }
    }

    /// Add a bone mapping.
    pub fn add_mapping(&mut self, source: BoneId, target: BoneId) {
        self.mappings.insert(source, target);
    }

    /// Get target bone for source bone.
    pub fn get_target(&self, source: BoneId) -> Option<BoneId> {
        self.mappings.get(&source).copied()
    }

    /// Get all mappings.
    pub fn mappings(&self) -> &HashMap<BoneId, BoneId> {
        &self.mappings
    }

    /// Create identity mapping (assumes same bone structure).
    pub fn identity(bone_count: usize) -> Self {
        let mut mapping = Self::new();
        for i in 0..bone_count {
            mapping.add_mapping(BoneId(i as u32), BoneId(i as u32));
        }
        mapping
    }

    /// Create mapping by matching bone names.
    pub fn from_names(source: &Armature, target: &Armature) -> Self {
        let mut mapping = Self::new();

        for source_bone in source.bones() {
            if let Some(target_id) = target.get_bone_id(&source_bone.name) {
                mapping.add_mapping(source_bone.id, target_id);
            }
        }

        mapping
    }

    /// Create automatic mapping based on hierarchy similarity.
    pub fn auto_detect(source: &Armature, target: &Armature) -> Self {
        let mut mapping = Self::from_names(source, target);

        // Try to match unmapped bones by hierarchy position
        for source_bone in source.bones() {
            if mapping.get_target(source_bone.id).is_none() {
                // Try to find similar bone by position in hierarchy
                if let Some(target_bone) = find_similar_bone(source_bone, source, target) {
                    mapping.add_mapping(source_bone.id, target_bone.id);
                }
            }
        }

        mapping
    }
}

impl Default for BoneMapping {
    fn default() -> Self {
        Self::new()
    }
}

/// Retargeting configuration.
#[derive(Debug, Clone)]
pub struct RetargetConfig {
    /// Source skeleton.
    pub source_skeleton: String,
    /// Target skeleton.
    pub target_skeleton: String,
    /// Bone mapping.
    pub bone_mapping: BoneMapping,
    /// Retargeting mode.
    pub mode: RetargetMode,
    /// Height compensation factor.
    pub height_compensation: f64,
    /// Apply root motion.
    pub apply_root_motion: bool,
}

impl RetargetConfig {
    /// Create a new retarget configuration.
    pub fn new(source_skeleton: impl Into<String>, target_skeleton: impl Into<String>) -> Self {
        Self {
            source_skeleton: source_skeleton.into(),
            target_skeleton: target_skeleton.into(),
            bone_mapping: BoneMapping::new(),
            mode: RetargetMode::ScaleCompensated,
            height_compensation: 1.0,
            apply_root_motion: true,
        }
    }

    /// Set bone mapping.
    pub fn with_bone_mapping(mut self, mapping: BoneMapping) -> Self {
        self.bone_mapping = mapping;
        self
    }

    /// Set retarget mode.
    pub fn with_mode(mut self, mode: RetargetMode) -> Self {
        self.mode = mode;
        self
    }

    /// Set height compensation.
    pub fn with_height_compensation(mut self, factor: f64) -> Self {
        self.height_compensation = factor;
        self
    }

    /// Compute height compensation from skeletons.
    pub fn compute_height_compensation(&mut self, source: &Armature, target: &Armature) {
        let source_height = compute_skeleton_height(source);
        let target_height = compute_skeleton_height(target);

        if source_height > 0.0 {
            self.height_compensation = target_height / source_height;
        }
    }

    /// Retarget a pose from source to target.
    pub fn retarget_pose(
        &self,
        source_pose: &Pose,
        source_armature: &Armature,
        target_armature: &Armature,
    ) -> Pose {
        let mut target_transforms = vec![BoneTransform::identity(); target_armature.bone_count()];

        match self.mode {
            RetargetMode::DirectCopy => {
                self.retarget_direct_copy(
                    source_pose,
                    source_armature,
                    target_armature,
                    &mut target_transforms,
                );
            }
            RetargetMode::ScaleCompensated => {
                self.retarget_scale_compensated(
                    source_pose,
                    source_armature,
                    target_armature,
                    &mut target_transforms,
                );
            }
            RetargetMode::IkBased => {
                self.retarget_ik_based(
                    source_pose,
                    source_armature,
                    target_armature,
                    &mut target_transforms,
                );
            }
        }

        Pose {
            transforms: target_transforms,
        }
    }

    fn retarget_direct_copy(
        &self,
        source_pose: &Pose,
        _source_armature: &Armature,
        _target_armature: &Armature,
        target_transforms: &mut [BoneTransform],
    ) {
        for (source_id, target_id) in self.bone_mapping.mappings() {
            let source_idx = source_id.0 as usize;
            let target_idx = target_id.0 as usize;

            if source_idx < source_pose.transforms.len() && target_idx < target_transforms.len() {
                let source_tf = &source_pose.transforms[source_idx];

                // Copy rotation directly
                target_transforms[target_idx].rotation = source_tf.rotation;

                // Handle root motion
                if self.apply_root_motion && source_idx == 0 {
                    target_transforms[target_idx].position =
                        source_tf.position * self.height_compensation;
                }
            }
        }
    }

    fn retarget_scale_compensated(
        &self,
        source_pose: &Pose,
        source_armature: &Armature,
        target_armature: &Armature,
        target_transforms: &mut [BoneTransform],
    ) {
        for (source_id, target_id) in self.bone_mapping.mappings() {
            let source_idx = source_id.0 as usize;
            let target_idx = target_id.0 as usize;

            if source_idx >= source_pose.transforms.len() || target_idx >= target_transforms.len() {
                continue;
            }

            let source_bone = match source_armature.get_bone(*source_id) {
                Some(b) => b,
                None => continue,
            };

            let target_bone = match target_armature.get_bone(*target_id) {
                Some(b) => b,
                None => continue,
            };

            let source_tf = &source_pose.transforms[source_idx];

            // Copy rotation
            target_transforms[target_idx].rotation = source_tf.rotation;

            // Scale compensation for position
            let source_length = source_bone.length();
            let target_length = target_bone.length();
            let length_ratio = if source_length > 0.0 {
                target_length / source_length
            } else {
                1.0
            };

            let position = if self.apply_root_motion && source_idx == 0 {
                source_tf.position * self.height_compensation
            } else {
                source_tf.position * length_ratio
            };

            target_transforms[target_idx].position = position;

            // Scale
            target_transforms[target_idx].scale = source_tf.scale;
        }
    }

    fn retarget_ik_based(
        &self,
        source_pose: &Pose,
        source_armature: &Armature,
        target_armature: &Armature,
        target_transforms: &mut [BoneTransform],
    ) {
        // First pass: direct copy
        self.retarget_scale_compensated(
            source_pose,
            source_armature,
            target_armature,
            target_transforms,
        );

        // TODO: Second pass would apply IK to ensure end effectors match
        // This requires an IK solver which should be implemented separately
    }
}

/// Find similar bone in target skeleton based on hierarchy.
fn find_similar_bone<'a>(
    source_bone: &Bone,
    source: &Armature,
    target: &'a Armature,
) -> Option<&'a Bone> {
    // Compute source bone's position in hierarchy
    let source_depth = compute_bone_depth(source_bone, source);
    let source_parent_name = if source_bone.parent.is_valid() {
        source.get_bone(source_bone.parent).map(|b| b.name.as_str())
    } else {
        None
    };

    // Find target bone with similar depth and parent
    for target_bone in target.bones() {
        let target_depth = compute_bone_depth(target_bone, target);
        if target_depth != source_depth {
            continue;
        }

        let target_parent_name = if target_bone.parent.is_valid() {
            target.get_bone(target_bone.parent).map(|b| b.name.as_str())
        } else {
            None
        };

        if source_parent_name == target_parent_name {
            return Some(target_bone);
        }
    }

    None
}

/// Compute bone depth in hierarchy.
fn compute_bone_depth(bone: &Bone, armature: &Armature) -> usize {
    let mut depth = 0;
    let mut current = bone.parent;

    while current.is_valid() {
        depth += 1;
        if let Some(parent) = armature.get_bone(current) {
            current = parent.parent;
        } else {
            break;
        }
    }

    depth
}

/// Compute approximate skeleton height.
fn compute_skeleton_height(armature: &Armature) -> f64 {
    let mut max_height: f64 = 0.0;

    for bone in armature.bones() {
        let height = if bone.head.y > bone.tail.y {
            bone.head.y
        } else {
            bone.tail.y
        };
        max_height = if height > max_height {
            height
        } else {
            max_height
        };
    }

    max_height
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rigging::armature::ArmatureBuilder;
    use nalgebra::Point3;

    #[test]
    fn test_bone_mapping() {
        let mut mapping = BoneMapping::new();
        mapping.add_mapping(BoneId(0), BoneId(0));
        mapping.add_mapping(BoneId(1), BoneId(2));

        assert_eq!(mapping.get_target(BoneId(0)), Some(BoneId(0)));
        assert_eq!(mapping.get_target(BoneId(1)), Some(BoneId(2)));
        assert_eq!(mapping.get_target(BoneId(3)), None);
    }

    #[test]
    fn test_identity_mapping() {
        let mapping = BoneMapping::identity(5);
        assert_eq!(mapping.get_target(BoneId(0)), Some(BoneId(0)));
        assert_eq!(mapping.get_target(BoneId(4)), Some(BoneId(4)));
    }

    #[test]
    fn test_retarget_config() {
        let config = RetargetConfig::new("SourceSkeleton", "TargetSkeleton")
            .with_mode(RetargetMode::ScaleCompensated)
            .with_height_compensation(1.2);

        assert_eq!(config.mode, RetargetMode::ScaleCompensated);
        assert_eq!(config.height_compensation, 1.2);
    }

    #[test]
    fn test_height_computation() {
        let armature = ArmatureBuilder::new("Test")
            .add_root(
                "Root",
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
            )
            .add_child("Spine", Point3::new(0.0, 1.5, 0.0))
            .add_child("Head", Point3::new(0.0, 2.0, 0.0))
            .build();

        let height = compute_skeleton_height(&armature);
        assert!(height >= 2.0);
    }
}
