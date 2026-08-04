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

//! Keyframe animation.
//!
//! Core keyframe types and interpolation for animation tracks.

use nalgebra::{UnitQuaternion, Vector3};
use std::collections::BTreeMap;

/// Interpolation mode between keyframes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InterpolationMode {
    /// Step (constant until next keyframe).
    Step,
    /// Linear interpolation.
    #[default]
    Linear,
    /// Cubic Bezier interpolation.
    Bezier,
    /// Catmull-Rom spline.
    CatmullRom,
    /// Hermite spline.
    Hermite,
}

/// Keyframe tangent handles for Bezier curves.
#[derive(Debug, Clone, Copy)]
pub struct TangentHandles {
    /// Incoming tangent.
    pub in_tangent: f64,
    /// Outgoing tangent.
    pub out_tangent: f64,
    /// Incoming weight.
    pub in_weight: f64,
    /// Outgoing weight.
    pub out_weight: f64,
}

impl Default for TangentHandles {
    fn default() -> Self {
        Self {
            in_tangent: 0.0,
            out_tangent: 0.0,
            in_weight: 1.0 / 3.0,
            out_weight: 1.0 / 3.0,
        }
    }
}

/// A single keyframe with value and interpolation settings.
#[derive(Debug, Clone)]
pub struct Keyframe<T: Clone> {
    /// Frame time.
    pub time: f64,
    /// Keyframe value.
    pub value: T,
    /// Interpolation mode.
    pub interpolation: InterpolationMode,
    /// Tangent handles (for Bezier).
    pub tangents: TangentHandles,
    /// Is tangent locked (in and out are equal).
    pub locked_tangent: bool,
}

impl<T: Clone> Keyframe<T> {
    /// Create a new keyframe.
    pub fn new(time: f64, value: T) -> Self {
        Self {
            time,
            value,
            interpolation: InterpolationMode::Linear,
            tangents: TangentHandles::default(),
            locked_tangent: true,
        }
    }

    /// Set interpolation mode.
    pub fn with_interpolation(mut self, mode: InterpolationMode) -> Self {
        self.interpolation = mode;
        self
    }
}

/// Animation track for a single property.
#[derive(Debug, Clone)]
pub struct AnimationTrack<T: Clone + Interpolate> {
    /// Track name.
    pub name: String,
    /// Keyframes sorted by time.
    keyframes: BTreeMap<ordered_float::OrderedFloat<f64>, Keyframe<T>>,
    /// Pre-infinity behavior.
    pub pre_infinity: InfinityMode,
    /// Post-infinity behavior.
    pub post_infinity: InfinityMode,
}

/// Behavior beyond the keyframe range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InfinityMode {
    /// Hold the first/last value.
    #[default]
    Constant,
    /// Repeat the animation.
    Cycle,
    /// Repeat with offset (value accumulates).
    CycleOffset,
    /// Oscillate (ping-pong).
    Oscillate,
    /// Linear extrapolation.
    Linear,
}

/// Trait for interpolatable values.
pub trait Interpolate: Clone {
    /// Linear interpolation.
    fn lerp(&self, other: &Self, t: f64) -> Self;

    /// Cubic Bezier interpolation.
    fn bezier(&self, other: &Self, t: f64, p1: f64, p2: f64) -> Self {
        // Default to lerp with adjusted t
        let t2 = cubic_bezier_t(t, p1, p2);
        self.lerp(other, t2)
    }
}

impl Interpolate for f64 {
    fn lerp(&self, other: &Self, t: f64) -> Self {
        self + (other - self) * t
    }
}

impl Interpolate for Vector3<f64> {
    fn lerp(&self, other: &Self, t: f64) -> Self {
        self + (other - self) * t
    }
}

impl Interpolate for UnitQuaternion<f64> {
    fn lerp(&self, other: &Self, t: f64) -> Self {
        self.slerp(other, t)
    }
}

/// Cubic bezier curve evaluation.
fn cubic_bezier_t(t: f64, p1: f64, p2: f64) -> f64 {
    let t2 = t * t;
    let t3 = t2 * t;
    let mt = 1.0 - t;
    let mt2 = mt * mt;
    let _mt3 = mt2 * mt;

    3.0 * mt2 * t * p1 + 3.0 * mt * t2 * p2 + t3
}

impl<T: Clone + Interpolate> AnimationTrack<T> {
    /// Create a new animation track.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            keyframes: BTreeMap::new(),
            pre_infinity: InfinityMode::Constant,
            post_infinity: InfinityMode::Constant,
        }
    }

    /// Add a keyframe.
    pub fn add_keyframe(&mut self, keyframe: Keyframe<T>) {
        self.keyframes
            .insert(ordered_float::OrderedFloat(keyframe.time), keyframe);
    }

    /// Remove keyframe at time.
    pub fn remove_keyframe(&mut self, time: f64) -> Option<Keyframe<T>> {
        self.keyframes.remove(&ordered_float::OrderedFloat(time))
    }

    /// Get keyframe at time.
    pub fn get_keyframe(&self, time: f64) -> Option<&Keyframe<T>> {
        self.keyframes.get(&ordered_float::OrderedFloat(time))
    }

    /// Get mutable keyframe at time.
    pub fn get_keyframe_mut(&mut self, time: f64) -> Option<&mut Keyframe<T>> {
        self.keyframes.get_mut(&ordered_float::OrderedFloat(time))
    }

    /// Get all keyframes.
    pub fn keyframes(&self) -> impl Iterator<Item = &Keyframe<T>> {
        self.keyframes.values()
    }

    /// Get keyframe count.
    pub fn keyframe_count(&self) -> usize {
        self.keyframes.len()
    }

    /// Get time range.
    pub fn time_range(&self) -> Option<(f64, f64)> {
        let first = self.keyframes.keys().next()?;
        let last = self.keyframes.keys().next_back()?;
        Some((first.0, last.0))
    }

    /// Evaluate track at time.
    pub fn evaluate(&self, time: f64) -> Option<T> {
        if self.keyframes.is_empty() {
            return None;
        }

        // Handle pre/post infinity
        if let Some((start, end)) = self.time_range() {
            let adjusted_time = if time < start {
                self.apply_infinity(time, start, end, self.pre_infinity, true)
            } else if time > end {
                self.apply_infinity(time, start, end, self.post_infinity, false)
            } else {
                time
            };

            self.evaluate_internal(adjusted_time)
        } else {
            None
        }
    }

    fn apply_infinity(
        &self,
        time: f64,
        start: f64,
        end: f64,
        mode: InfinityMode,
        is_pre: bool,
    ) -> f64 {
        let duration = end - start;
        if duration <= 0.0 {
            return start;
        }

        match mode {
            InfinityMode::Constant => {
                if is_pre {
                    start
                } else {
                    end
                }
            }
            InfinityMode::Cycle => {
                let offset = if is_pre { start - time } else { time - end };
                let cycles = (offset / duration).floor();
                let remainder = offset - cycles * duration;
                if is_pre {
                    end - remainder
                } else {
                    start + remainder
                }
            }
            InfinityMode::CycleOffset => {
                // Similar to cycle but value offsets
                let offset = if is_pre { start - time } else { time - end };
                let remainder = offset % duration;
                if is_pre {
                    end - remainder
                } else {
                    start + remainder
                }
            }
            InfinityMode::Oscillate => {
                let offset = if is_pre { start - time } else { time - end };
                let cycles = (offset / duration).floor() as i64;
                let remainder = offset - (cycles as f64) * duration;
                if cycles % 2 == 0 {
                    if is_pre {
                        end - remainder
                    } else {
                        start + remainder
                    }
                } else if is_pre {
                    start + remainder
                } else {
                    end - remainder
                }
            }
            InfinityMode::Linear => time,
        }
    }

    fn evaluate_internal(&self, time: f64) -> Option<T> {
        let time_key = ordered_float::OrderedFloat(time);

        // Exact match
        if let Some(kf) = self.keyframes.get(&time_key) {
            return Some(kf.value.clone());
        }

        // Find surrounding keyframes
        let before = self.keyframes.range(..time_key).next_back();
        let after = self.keyframes.range(time_key..).next();

        match (before, after) {
            (Some((_, kf1)), Some((_, kf2))) => {
                // Interpolate between keyframes
                let t = (time - kf1.time) / (kf2.time - kf1.time);
                Some(self.interpolate_keyframes(kf1, kf2, t))
            }
            (Some((_, kf)), None) => Some(kf.value.clone()),
            (None, Some((_, kf))) => Some(kf.value.clone()),
            (None, None) => None,
        }
    }

    fn interpolate_keyframes(&self, k1: &Keyframe<T>, k2: &Keyframe<T>, t: f64) -> T {
        match k1.interpolation {
            InterpolationMode::Step => k1.value.clone(),
            InterpolationMode::Linear => k1.value.lerp(&k2.value, t),
            InterpolationMode::Bezier => {
                k1.value
                    .bezier(&k2.value, t, k1.tangents.out_weight, k2.tangents.in_weight)
            }
            InterpolationMode::CatmullRom | InterpolationMode::Hermite => {
                // Simplified: use linear for now
                k1.value.lerp(&k2.value, t)
            }
        }
    }

    /// Bake animation to uniform samples.
    pub fn bake(&self, start: f64, end: f64, sample_rate: f64) -> Vec<(f64, T)> {
        let mut samples = Vec::new();
        let mut time = start;

        while time <= end {
            if let Some(value) = self.evaluate(time) {
                samples.push((time, value));
            }
            time += 1.0 / sample_rate;
        }

        samples
    }
}

/// Animation clip containing multiple tracks.
#[derive(Debug, Clone)]
pub struct AnimationClip {
    /// Clip name.
    pub name: String,
    /// Frame rate.
    pub frame_rate: f64,
    /// Float tracks.
    pub float_tracks: Vec<AnimationTrack<f64>>,
    /// Vector tracks.
    pub vector_tracks: Vec<AnimationTrack<Vector3<f64>>>,
    /// Rotation tracks.
    pub rotation_tracks: Vec<AnimationTrack<UnitQuaternion<f64>>>,
}

impl AnimationClip {
    /// Create a new animation clip.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            frame_rate: 30.0,
            float_tracks: Vec::new(),
            vector_tracks: Vec::new(),
            rotation_tracks: Vec::new(),
        }
    }

    /// Get duration in seconds.
    pub fn duration(&self) -> f64 {
        let mut max_time = 0.0_f64;

        for track in &self.float_tracks {
            if let Some((_, end)) = track.time_range() {
                max_time = max_time.max(end);
            }
        }
        for track in &self.vector_tracks {
            if let Some((_, end)) = track.time_range() {
                max_time = max_time.max(end);
            }
        }
        for track in &self.rotation_tracks {
            if let Some((_, end)) = track.time_range() {
                max_time = max_time.max(end);
            }
        }

        max_time
    }

    /// Get duration in frames.
    pub fn duration_frames(&self) -> f64 {
        self.duration() * self.frame_rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyframe_creation() {
        let kf = Keyframe::new(0.0, 1.0);
        assert_eq!(kf.time, 0.0);
        assert_eq!(kf.value, 1.0);
    }

    #[test]
    fn test_track_interpolation() {
        let mut track = AnimationTrack::new("test");
        track.add_keyframe(Keyframe::new(0.0, 0.0));
        track.add_keyframe(Keyframe::new(1.0, 10.0));

        let val = track.evaluate(0.5).unwrap();
        assert!((val - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_track_before_first() {
        let mut track = AnimationTrack::new("test");
        track.add_keyframe(Keyframe::new(1.0, 5.0));
        track.add_keyframe(Keyframe::new(2.0, 10.0));

        let val = track.evaluate(0.0).unwrap();
        assert!((val - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_track_after_last() {
        let mut track = AnimationTrack::new("test");
        track.add_keyframe(Keyframe::new(0.0, 0.0));
        track.add_keyframe(Keyframe::new(1.0, 10.0));

        let val = track.evaluate(2.0).unwrap();
        assert!((val - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_vector_interpolation() {
        let mut track = AnimationTrack::new("position");
        track.add_keyframe(Keyframe::new(0.0, Vector3::new(0.0, 0.0, 0.0)));
        track.add_keyframe(Keyframe::new(1.0, Vector3::new(10.0, 10.0, 10.0)));

        let val = track.evaluate(0.5).unwrap();
        assert!((val.x - 5.0).abs() < 1e-10);
        assert!((val.y - 5.0).abs() < 1e-10);
        assert!((val.z - 5.0).abs() < 1e-10);
    }
}
