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

//! Non-Linear Animation (NLA) editor.
//!
//! Layer-based animation mixing, strips, and non-destructive editing.

use crate::motion::blend::BlendTree;
use crate::rigging::armature::Pose;

/// NLA track (layer of animation strips).
#[derive(Debug, Clone)]
pub struct NlaTrack {
    /// Track name.
    pub name: String,
    /// Animation strips.
    strips: Vec<NlaStrip>,
    /// Is muted.
    pub mute: bool,
    /// Is soloed.
    pub solo: bool,
    /// Track weight.
    pub weight: f64,
}

impl NlaTrack {
    /// Create a new NLA track.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            strips: Vec::new(),
            mute: false,
            solo: false,
            weight: 1.0,
        }
    }

    /// Add a strip to the track.
    pub fn add_strip(&mut self, strip: NlaStrip) {
        self.strips.push(strip);
        self.sort_strips();
    }

    /// Remove a strip by index.
    pub fn remove_strip(&mut self, index: usize) {
        if index < self.strips.len() {
            self.strips.remove(index);
        }
    }

    /// Get strip at index.
    pub fn get_strip(&self, index: usize) -> Option<&NlaStrip> {
        self.strips.get(index)
    }

    /// Get mutable strip at index.
    pub fn get_strip_mut(&mut self, index: usize) -> Option<&mut NlaStrip> {
        self.strips.get_mut(index)
    }

    /// Get all strips.
    pub fn strips(&self) -> &[NlaStrip] {
        &self.strips
    }

    /// Sort strips by start frame.
    fn sort_strips(&mut self) {
        self.strips
            .sort_by(|a, b| a.start_frame.partial_cmp(&b.start_frame).unwrap());
    }

    /// Get active strips at given frame.
    pub fn active_strips_at(&self, frame: f64) -> Vec<&NlaStrip> {
        self.strips
            .iter()
            .filter(|s| s.is_active_at(frame))
            .collect()
    }

    /// Evaluate track at given frame.
    pub fn evaluate(&self, frame: f64, bone_count: usize) -> Option<Pose> {
        if self.mute {
            return None;
        }

        let active = self.active_strips_at(frame);
        if active.is_empty() {
            return None;
        }

        // Start with first strip
        let mut result = active[0].evaluate(frame, bone_count);

        // Blend with remaining strips
        for strip in active.iter().skip(1) {
            let strip_pose = strip.evaluate(frame, bone_count);
            let blend_weight = strip.compute_blend_weight(frame);

            result = BlendTree::blend_poses(&result, &strip_pose, blend_weight);
        }

        Some(result)
    }
}

/// NLA strip (animation clip instance).
#[derive(Debug, Clone)]
pub struct NlaStrip {
    /// Strip name.
    pub name: String,
    /// Strip type.
    pub strip_type: NlaStripType,
    /// Action/clip reference.
    pub action_ref: String,
    /// Start frame in timeline.
    pub start_frame: f64,
    /// End frame in timeline.
    pub end_frame: f64,
    /// Action start offset.
    pub action_start: f64,
    /// Action end offset.
    pub action_end: f64,
    /// Repeat count.
    pub repeat: f64,
    /// Scale factor.
    pub scale: f64,
    /// Blend in frames.
    pub blend_in: f64,
    /// Blend out frames.
    pub blend_out: f64,
    /// Extrapolation mode before strip.
    pub extrapolation_before: ExtrapolationMode,
    /// Extrapolation mode after strip.
    pub extrapolation_after: ExtrapolationMode,
    /// Blend mode.
    pub blend_mode: NlaBlendMode,
    /// Is muted.
    pub mute: bool,
}

impl NlaStrip {
    /// Create a new NLA strip.
    pub fn new(name: impl Into<String>, action_ref: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            strip_type: NlaStripType::Clip,
            action_ref: action_ref.into(),
            start_frame: 0.0,
            end_frame: 100.0,
            action_start: 0.0,
            action_end: 100.0,
            repeat: 1.0,
            scale: 1.0,
            blend_in: 0.0,
            blend_out: 0.0,
            extrapolation_before: ExtrapolationMode::Nothing,
            extrapolation_after: ExtrapolationMode::Nothing,
            blend_mode: NlaBlendMode::Replace,
            mute: false,
        }
    }

    /// Check if strip is active at given frame.
    pub fn is_active_at(&self, frame: f64) -> bool {
        if self.mute {
            return false;
        }

        match self.strip_type {
            NlaStripType::Clip => {
                // Include extrapolation regions
                match self.extrapolation_before {
                    ExtrapolationMode::Nothing => {}
                    _ => {
                        if frame < self.start_frame {
                            return true;
                        }
                    }
                }

                if frame >= self.start_frame && frame <= self.end_frame {
                    return true;
                }

                match self.extrapolation_after {
                    ExtrapolationMode::Nothing => false,
                    _ => frame > self.end_frame,
                }
            }
            NlaStripType::Transition => frame >= self.start_frame && frame <= self.end_frame,
            NlaStripType::Meta => frame >= self.start_frame && frame <= self.end_frame,
            NlaStripType::Sound => false, // Sound handled separately
        }
    }

    /// Compute blend weight at given frame (considering blend in/out).
    pub fn compute_blend_weight(&self, frame: f64) -> f64 {
        if frame < self.start_frame || frame > self.end_frame {
            return 0.0;
        }

        let mut weight = 1.0;

        // Blend in
        if self.blend_in > 0.0 {
            let blend_in_end = self.start_frame + self.blend_in;
            if frame < blend_in_end {
                weight *= (frame - self.start_frame) / self.blend_in;
            }
        }

        // Blend out
        if self.blend_out > 0.0 {
            let blend_out_start = self.end_frame - self.blend_out;
            if frame > blend_out_start {
                weight *= (self.end_frame - frame) / self.blend_out;
            }
        }

        weight.clamp(0.0, 1.0)
    }

    /// Map timeline frame to action frame.
    pub fn map_to_action_frame(&self, timeline_frame: f64) -> f64 {
        if timeline_frame < self.start_frame {
            // Before strip
            match self.extrapolation_before {
                ExtrapolationMode::Hold => self.action_start,
                ExtrapolationMode::HoldForward => {
                    let offset = self.start_frame - timeline_frame;
                    self.action_start - offset * self.scale
                }
                ExtrapolationMode::Nothing => self.action_start,
            }
        } else if timeline_frame > self.end_frame {
            // After strip
            match self.extrapolation_after {
                ExtrapolationMode::Hold => self.action_end,
                ExtrapolationMode::HoldForward => {
                    let offset = timeline_frame - self.end_frame;
                    self.action_end + offset * self.scale
                }
                ExtrapolationMode::Nothing => self.action_end,
            }
        } else {
            // Within strip
            let strip_duration = self.end_frame - self.start_frame;
            let action_duration = self.action_end - self.action_start;
            let normalized = (timeline_frame - self.start_frame) / strip_duration;

            // Apply repeat
            let repeated = (normalized * self.repeat) % 1.0;

            self.action_start + repeated * action_duration * self.scale
        }
    }

    /// Evaluate strip at given frame.
    pub fn evaluate(&self, frame: f64, bone_count: usize) -> Pose {
        let _action_frame = self.map_to_action_frame(frame);

        // In a real implementation, this would fetch the pose from the action
        // For now, return identity
        Pose::identity(bone_count)
    }
}

/// NLA strip type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NlaStripType {
    /// Regular animation clip.
    Clip,
    /// Transition between clips.
    Transition,
    /// Meta strip (contains other strips).
    Meta,
    /// Sound strip.
    Sound,
}

/// Extrapolation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtrapolationMode {
    /// No extrapolation.
    Nothing,
    /// Hold first/last frame.
    Hold,
    /// Continue animation forward.
    HoldForward,
}

/// NLA blend mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NlaBlendMode {
    /// Replace previous tracks.
    Replace,
    /// Combine with previous tracks.
    Combine,
    /// Add to previous tracks.
    Add,
    /// Subtract from previous tracks.
    Subtract,
    /// Multiply with previous tracks.
    Multiply,
}

/// NLA evaluator.
#[derive(Debug)]
pub struct NlaEvaluator {
    /// All tracks.
    tracks: Vec<NlaTrack>,
}

impl NlaEvaluator {
    /// Create a new NLA evaluator.
    pub fn new() -> Self {
        Self { tracks: Vec::new() }
    }

    /// Add a track.
    pub fn add_track(&mut self, track: NlaTrack) {
        self.tracks.push(track);
    }

    /// Remove a track.
    pub fn remove_track(&mut self, index: usize) {
        if index < self.tracks.len() {
            self.tracks.remove(index);
        }
    }

    /// Get track at index.
    pub fn get_track(&self, index: usize) -> Option<&NlaTrack> {
        self.tracks.get(index)
    }

    /// Get mutable track at index.
    pub fn get_track_mut(&mut self, index: usize) -> Option<&mut NlaTrack> {
        self.tracks.get_mut(index)
    }

    /// Get all tracks.
    pub fn tracks(&self) -> &[NlaTrack] {
        &self.tracks
    }

    /// Evaluate all tracks at given frame.
    pub fn evaluate(&self, frame: f64, bone_count: usize) -> Pose {
        let mut result = Pose::identity(bone_count);
        let has_solo = self.tracks.iter().any(|t| t.solo);

        for track in &self.tracks {
            // Skip if solo mode is active and this track is not soloed
            if has_solo && !track.solo {
                continue;
            }

            if let Some(track_pose) = track.evaluate(frame, bone_count) {
                result = BlendTree::blend_poses(&result, &track_pose, track.weight);
            }
        }

        result
    }

    /// Bake NLA animation to keyframes.
    pub fn bake_to_keyframes(
        &self,
        start_frame: f64,
        end_frame: f64,
        step: f64,
        bone_count: usize,
    ) -> Vec<(f64, Pose)> {
        let mut keyframes = Vec::new();
        let mut frame = start_frame;

        while frame <= end_frame {
            let pose = self.evaluate(frame, bone_count);
            keyframes.push((frame, pose));
            frame += step;
        }

        keyframes
    }
}

impl Default for NlaEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nla_track_creation() {
        let track = NlaTrack::new("Track1");
        assert_eq!(track.name, "Track1");
        assert!(!track.mute);
        assert_eq!(track.weight, 1.0);
    }

    #[test]
    fn test_nla_strip_creation() {
        let strip = NlaStrip::new("Walk", "WalkAction");
        assert_eq!(strip.name, "Walk");
        assert_eq!(strip.action_ref, "WalkAction");
    }

    #[test]
    fn test_strip_active_at() {
        let mut strip = NlaStrip::new("Test", "TestAction");
        strip.start_frame = 10.0;
        strip.end_frame = 50.0;

        assert!(!strip.is_active_at(5.0));
        assert!(strip.is_active_at(30.0));
        assert!(!strip.is_active_at(60.0));
    }

    #[test]
    fn test_strip_blend_weight() {
        let mut strip = NlaStrip::new("Test", "TestAction");
        strip.start_frame = 0.0;
        strip.end_frame = 100.0;
        strip.blend_in = 10.0;
        strip.blend_out = 10.0;

        assert_eq!(strip.compute_blend_weight(5.0), 0.5);
        assert_eq!(strip.compute_blend_weight(50.0), 1.0);
        assert_eq!(strip.compute_blend_weight(95.0), 0.5);
    }

    #[test]
    fn test_strip_frame_mapping() {
        let mut strip = NlaStrip::new("Test", "TestAction");
        strip.start_frame = 0.0;
        strip.end_frame = 100.0;
        strip.action_start = 0.0;
        strip.action_end = 50.0;
        strip.scale = 1.0;

        let action_frame = strip.map_to_action_frame(50.0);
        assert_eq!(action_frame, 25.0);
    }

    #[test]
    fn test_nla_evaluator() {
        let mut evaluator = NlaEvaluator::new();

        let mut track = NlaTrack::new("Track1");
        let strip = NlaStrip::new("Walk", "WalkAction");
        track.add_strip(strip);

        evaluator.add_track(track);

        let pose = evaluator.evaluate(50.0, 10);
        assert_eq!(pose.transforms.len(), 10);
    }

    #[test]
    fn test_strip_repeat() {
        let mut strip = NlaStrip::new("Test", "TestAction");
        strip.start_frame = 0.0;
        strip.end_frame = 100.0;
        strip.action_start = 0.0;
        strip.action_end = 25.0;
        strip.repeat = 2.0;

        let action_frame = strip.map_to_action_frame(50.0);
        assert!(action_frame <= 25.0);
    }
}
