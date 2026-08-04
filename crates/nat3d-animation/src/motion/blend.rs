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

//! Motion blending system.
//!
//! Provides blend trees, pose blending, and additive animation.

use nalgebra::{UnitQuaternion, Vector3};
use std::collections::HashMap;

use crate::rigging::armature::Pose;
use crate::rigging::bone::{BoneId, BoneTransform};

/// Animation clip reference.
#[derive(Debug, Clone)]
pub struct AnimClipRef {
    /// Clip identifier.
    pub clip_id: String,
    /// Start time in clip.
    pub start: f64,
    /// End time in clip.
    pub end: f64,
    /// Playback speed.
    pub speed: f64,
    /// Loop mode.
    pub loop_mode: LoopMode,
}

impl AnimClipRef {
    /// Create a new clip reference.
    pub fn new(clip_id: impl Into<String>) -> Self {
        Self {
            clip_id: clip_id.into(),
            start: 0.0,
            end: f64::MAX,
            speed: 1.0,
            loop_mode: LoopMode::Loop,
        }
    }

    /// Sample clip at given time.
    pub fn sample_time(&self, time: f64) -> f64 {
        let duration = self.end - self.start;
        if duration <= 0.0 {
            return self.start;
        }

        let scaled_time = time * self.speed;

        match self.loop_mode {
            LoopMode::Once => (self.start + scaled_time).min(self.end),
            LoopMode::Loop => {
                let t = scaled_time % duration;
                self.start + t
            }
            LoopMode::PingPong => {
                let cycle = (scaled_time / duration) as i32;
                let t = scaled_time % duration;
                if cycle % 2 == 0 {
                    self.start + t
                } else {
                    self.end - t
                }
            }
            LoopMode::Clamp => {
                if scaled_time < 0.0 {
                    self.start
                } else if scaled_time > duration {
                    self.end
                } else {
                    self.start + scaled_time
                }
            }
        }
    }
}

/// Loop mode for animation clips.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopMode {
    /// Play once and stop at end.
    Once,
    /// Loop continuously.
    Loop,
    /// Ping-pong (forward then backward).
    PingPong,
    /// Clamp to start/end.
    Clamp,
}

/// Blend mask for selective bone blending.
#[derive(Debug, Clone)]
pub struct BlendMask {
    /// Per-bone weights.
    weights: HashMap<BoneId, f64>,
    /// Default weight for unspecified bones.
    pub default_weight: f64,
}

impl BlendMask {
    /// Create a new blend mask.
    pub fn new() -> Self {
        Self {
            weights: HashMap::new(),
            default_weight: 1.0,
        }
    }

    /// Create a full mask (all bones weighted 1.0).
    pub fn full() -> Self {
        Self {
            weights: HashMap::new(),
            default_weight: 1.0,
        }
    }

    /// Create an empty mask (all bones weighted 0.0).
    pub fn empty() -> Self {
        Self {
            weights: HashMap::new(),
            default_weight: 0.0,
        }
    }

    /// Set bone weight.
    pub fn set_weight(&mut self, bone_id: BoneId, weight: f64) {
        self.weights.insert(bone_id, weight.clamp(0.0, 1.0));
    }

    /// Get bone weight.
    pub fn get_weight(&self, bone_id: BoneId) -> f64 {
        self.weights
            .get(&bone_id)
            .copied()
            .unwrap_or(self.default_weight)
    }

    /// Set multiple bone weights.
    pub fn set_weights(&mut self, weights: &[(BoneId, f64)]) {
        for &(bone_id, weight) in weights {
            self.set_weight(bone_id, weight);
        }
    }
}

impl Default for BlendMask {
    fn default() -> Self {
        Self::new()
    }
}

/// Blend tree node.
#[derive(Debug, Clone)]
pub enum BlendNode {
    /// Single animation clip.
    Clip(AnimClipRef),
    /// 1D blend between children based on parameter.
    Blend1D {
        children: Vec<(f64, Box<BlendNode>)>,
        parameter: String,
    },
    /// 2D blend between children based on two parameters.
    Blend2D {
        children: Vec<((f64, f64), Box<BlendNode>)>,
        param_x: String,
        param_y: String,
    },
    /// Additive blend (base + overlay * weight).
    Additive {
        base: Box<BlendNode>,
        overlay: Box<BlendNode>,
        weight: f64,
    },
    /// Override blend with mask.
    Override {
        clip: Box<BlendNode>,
        mask: BlendMask,
    },
}

impl BlendNode {
    /// Create a clip node.
    pub fn clip(clip_id: impl Into<String>) -> Self {
        Self::Clip(AnimClipRef::new(clip_id))
    }

    /// Create a 1D blend node.
    pub fn blend_1d(parameter: impl Into<String>) -> Self {
        Self::Blend1D {
            children: Vec::new(),
            parameter: parameter.into(),
        }
    }

    /// Create a 2D blend node.
    pub fn blend_2d(param_x: impl Into<String>, param_y: impl Into<String>) -> Self {
        Self::Blend2D {
            children: Vec::new(),
            param_x: param_x.into(),
            param_y: param_y.into(),
        }
    }

    /// Create an additive blend node.
    pub fn additive(base: BlendNode, overlay: BlendNode, weight: f64) -> Self {
        Self::Additive {
            base: Box::new(base),
            overlay: Box::new(overlay),
            weight,
        }
    }

    /// Create an override node.
    pub fn override_with_mask(clip: BlendNode, mask: BlendMask) -> Self {
        Self::Override {
            clip: Box::new(clip),
            mask,
        }
    }
}

/// Blend tree for motion blending.
#[derive(Debug, Clone)]
pub struct BlendTree {
    /// Root node of the blend tree.
    root: Option<BlendNode>,
    /// Animation parameters.
    parameters: HashMap<String, f64>,
}

impl BlendTree {
    /// Create a new blend tree.
    pub fn new() -> Self {
        Self {
            root: None,
            parameters: HashMap::new(),
        }
    }

    /// Set the root node.
    pub fn set_root(&mut self, node: BlendNode) {
        self.root = Some(node);
    }

    /// Set a parameter value.
    pub fn set_parameter(&mut self, name: impl Into<String>, value: f64) {
        self.parameters.insert(name.into(), value);
    }

    /// Get a parameter value.
    pub fn get_parameter(&self, name: &str) -> f64 {
        self.parameters.get(name).copied().unwrap_or(0.0)
    }

    /// Evaluate the blend tree at given time.
    pub fn evaluate(&self, time: f64, bone_count: usize) -> Pose {
        match &self.root {
            Some(node) => self.evaluate_node(node, time, bone_count),
            None => Pose::identity(bone_count),
        }
    }

    fn evaluate_node(&self, node: &BlendNode, time: f64, bone_count: usize) -> Pose {
        match node {
            BlendNode::Clip(clip_ref) => {
                let _sample_time = clip_ref.sample_time(time);
                // In a real implementation, this would fetch the pose from an animation clip
                // For now, return an identity pose
                Pose::identity(bone_count)
            }
            BlendNode::Blend1D {
                children,
                parameter,
            } => {
                if children.is_empty() {
                    return Pose::identity(bone_count);
                }

                let param_value = self.get_parameter(parameter);
                self.blend_1d(children, param_value, time, bone_count)
            }
            BlendNode::Blend2D {
                children,
                param_x,
                param_y,
            } => {
                if children.is_empty() {
                    return Pose::identity(bone_count);
                }

                let x = self.get_parameter(param_x);
                let y = self.get_parameter(param_y);
                self.blend_2d(children, x, y, time, bone_count)
            }
            BlendNode::Additive {
                base,
                overlay,
                weight,
            } => {
                let base_pose = self.evaluate_node(base, time, bone_count);
                let overlay_pose = self.evaluate_node(overlay, time, bone_count);
                Self::additive_blend(&base_pose, &overlay_pose, *weight)
            }
            BlendNode::Override { clip, mask } => {
                let pose = self.evaluate_node(clip, time, bone_count);
                self.apply_mask(&pose, mask)
            }
        }
    }

    fn blend_1d(
        &self,
        children: &[(f64, Box<BlendNode>)],
        param_value: f64,
        time: f64,
        bone_count: usize,
    ) -> Pose {
        // Find the two closest children to blend
        let mut sorted = children.to_vec();
        sorted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

        if sorted.len() == 1 {
            return self.evaluate_node(&sorted[0].1, time, bone_count);
        }

        // Find bracketing values
        let mut lower_idx = 0;
        let mut upper_idx = sorted.len() - 1;

        for (i, &(threshold, _)) in sorted.iter().enumerate() {
            if param_value >= threshold {
                lower_idx = i;
            }
            if param_value <= threshold && i < upper_idx {
                upper_idx = i;
                break;
            }
        }

        if lower_idx == upper_idx {
            return self.evaluate_node(&sorted[lower_idx].1, time, bone_count);
        }

        // Blend between lower and upper
        let lower_threshold = sorted[lower_idx].0;
        let upper_threshold = sorted[upper_idx].0;
        let t = if upper_threshold - lower_threshold > 1e-6 {
            ((param_value - lower_threshold) / (upper_threshold - lower_threshold)).clamp(0.0, 1.0)
        } else {
            0.5
        };

        let pose_a = self.evaluate_node(&sorted[lower_idx].1, time, bone_count);
        let pose_b = self.evaluate_node(&sorted[upper_idx].1, time, bone_count);

        Self::blend_poses(&pose_a, &pose_b, t)
    }

    fn blend_2d(
        &self,
        children: &[((f64, f64), Box<BlendNode>)],
        x: f64,
        y: f64,
        time: f64,
        bone_count: usize,
    ) -> Pose {
        if children.is_empty() {
            return Pose::identity(bone_count);
        }

        if children.len() == 1 {
            return self.evaluate_node(&children[0].1, time, bone_count);
        }

        // Simple triangle-based barycentric interpolation
        // Find three closest points and blend using barycentric coordinates
        let mut closest: Vec<(usize, f64)> = children
            .iter()
            .enumerate()
            .map(|(i, ((px, py), _))| {
                let dx = x - px;
                let dy = y - py;
                (i, (dx * dx + dy * dy).sqrt())
            })
            .collect();

        closest.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        if closest.len() >= 3 {
            let i0 = closest[0].0;
            let i1 = closest[1].0;
            let i2 = closest[2].0;

            let pose0 = self.evaluate_node(&children[i0].1, time, bone_count);
            let pose1 = self.evaluate_node(&children[i1].1, time, bone_count);
            let pose2 = self.evaluate_node(&children[i2].1, time, bone_count);

            // Simple weighted blend based on inverse distance
            let d0 = closest[0].1.max(1e-6);
            let d1 = closest[1].1.max(1e-6);
            let d2 = closest[2].1.max(1e-6);

            let w0 = 1.0 / d0;
            let w1 = 1.0 / d1;
            let w2 = 1.0 / d2;
            let total = w0 + w1 + w2;

            let w0 = w0 / total;
            let w1 = w1 / total;
            let w2 = w2 / total;

            // Blend poses
            let temp = Self::blend_poses(&pose0, &pose1, w1 / (w0 + w1));
            Self::blend_poses(&temp, &pose2, w2)
        } else {
            self.evaluate_node(&children[closest[0].0].1, time, bone_count)
        }
    }

    /// Blend two poses with linear interpolation.
    pub fn blend_poses(pose_a: &Pose, pose_b: &Pose, t: f64) -> Pose {
        let t = t.clamp(0.0, 1.0);
        let bone_count = pose_a.transforms.len().min(pose_b.transforms.len());

        let mut transforms = Vec::with_capacity(bone_count);
        for i in 0..bone_count {
            let a = &pose_a.transforms[i];
            let b = &pose_b.transforms[i];

            // LERP position
            let position = a.position + (b.position - a.position) * t;

            // SLERP rotation
            let rotation = a.rotation.slerp(&b.rotation, t);

            // LERP scale
            let scale = a.scale + (b.scale - a.scale) * t;

            transforms.push(BoneTransform {
                position,
                rotation,
                scale,
            });
        }

        Pose { transforms }
    }

    /// Additive blend (base + overlay * weight).
    pub fn additive_blend(base: &Pose, overlay: &Pose, weight: f64) -> Pose {
        let weight = weight.clamp(0.0, 1.0);
        let bone_count = base.transforms.len().min(overlay.transforms.len());

        let mut transforms = Vec::with_capacity(bone_count);
        for i in 0..bone_count {
            let base_tf = &base.transforms[i];
            let overlay_tf = &overlay.transforms[i];

            // Additive position
            let position = base_tf.position + overlay_tf.position * weight;

            // Multiplicative rotation
            let overlay_scaled = if weight < 1.0 {
                // Scale the rotation by weight
                match overlay_tf.rotation.axis_angle() {
                    Some((axis, angle)) => UnitQuaternion::from_axis_angle(&axis, angle * weight),
                    None => UnitQuaternion::identity(),
                }
            } else {
                overlay_tf.rotation
            };
            let rotation = base_tf.rotation * overlay_scaled;

            // Multiplicative scale
            let scale_factor = Vector3::new(
                1.0 + (overlay_tf.scale.x - 1.0) * weight,
                1.0 + (overlay_tf.scale.y - 1.0) * weight,
                1.0 + (overlay_tf.scale.z - 1.0) * weight,
            );
            let scale = base_tf.scale.component_mul(&scale_factor);

            transforms.push(BoneTransform {
                position,
                rotation,
                scale,
            });
        }

        Pose { transforms }
    }

    fn apply_mask(&self, pose: &Pose, mask: &BlendMask) -> Pose {
        let mut transforms = pose.transforms.clone();

        for (i, transform) in transforms.iter_mut().enumerate() {
            let bone_id = BoneId(i as u32);
            let weight = mask.get_weight(bone_id);

            if weight < 1.0 {
                // Blend with identity
                let identity = BoneTransform::identity();
                *transform = identity.lerp(transform, weight);
            }
        }

        Pose { transforms }
    }
}

impl Default for BlendTree {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anim_clip_ref() {
        let clip = AnimClipRef {
            clip_id: "walk".to_string(),
            start: 0.0,
            end: 1.0,
            speed: 1.0,
            loop_mode: LoopMode::Loop,
        };

        assert_eq!(clip.sample_time(0.0), 0.0);
        assert_eq!(clip.sample_time(0.5), 0.5);
        assert_eq!(clip.sample_time(1.5), 0.5); // Looped
    }

    #[test]
    fn test_blend_mask() {
        let mut mask = BlendMask::new();
        mask.set_weight(BoneId(0), 0.5);
        mask.set_weight(BoneId(1), 1.0);

        assert_eq!(mask.get_weight(BoneId(0)), 0.5);
        assert_eq!(mask.get_weight(BoneId(1)), 1.0);
        assert_eq!(mask.get_weight(BoneId(2)), 1.0); // default
    }

    #[test]
    fn test_blend_poses() {
        let pose_a = Pose {
            transforms: vec![BoneTransform {
                position: Vector3::new(0.0, 0.0, 0.0),
                rotation: UnitQuaternion::identity(),
                scale: Vector3::new(1.0, 1.0, 1.0),
            }],
        };

        let pose_b = Pose {
            transforms: vec![BoneTransform {
                position: Vector3::new(1.0, 0.0, 0.0),
                rotation: UnitQuaternion::identity(),
                scale: Vector3::new(1.0, 1.0, 1.0),
            }],
        };

        let blended = BlendTree::blend_poses(&pose_a, &pose_b, 0.5);
        assert!((blended.transforms[0].position.x - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_additive_blend() {
        let base = Pose {
            transforms: vec![BoneTransform {
                position: Vector3::new(1.0, 0.0, 0.0),
                rotation: UnitQuaternion::identity(),
                scale: Vector3::new(1.0, 1.0, 1.0),
            }],
        };

        let overlay = Pose {
            transforms: vec![BoneTransform {
                position: Vector3::new(0.5, 0.0, 0.0),
                rotation: UnitQuaternion::identity(),
                scale: Vector3::new(1.2, 1.0, 1.0),
            }],
        };

        let result = BlendTree::additive_blend(&base, &overlay, 1.0);
        assert!((result.transforms[0].position.x - 1.5).abs() < 1e-6);
        assert!((result.transforms[0].scale.x - 1.2).abs() < 1e-6);
    }

    #[test]
    fn test_blend_tree_parameters() {
        let mut tree = BlendTree::new();
        tree.set_parameter("speed", 1.5);

        assert_eq!(tree.get_parameter("speed"), 1.5);
        assert_eq!(tree.get_parameter("unknown"), 0.0);
    }
}
