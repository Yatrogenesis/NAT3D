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

//! Animation timeline management.
//!
//! Provides timeline state, playback control, and time management.

use std::collections::HashMap;

/// Timeline playback state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaybackState {
    /// Stopped at current frame.
    #[default]
    Stopped,
    /// Playing forward.
    Playing,
    /// Playing in reverse.
    Reverse,
    /// Paused (can resume).
    Paused,
}

/// Playback mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaybackMode {
    /// Play once and stop.
    Once,
    /// Loop continuously.
    #[default]
    Loop,
    /// Ping-pong (play forward, then reverse).
    PingPong,
}

/// Time display format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimeFormat {
    /// Frame numbers.
    #[default]
    Frames,
    /// Seconds.
    Seconds,
    /// SMPTE timecode (HH:MM:SS:FF).
    Timecode,
}

/// Animation timeline.
#[derive(Debug, Clone)]
pub struct Timeline {
    /// Current frame.
    current_frame: f64,
    /// Start frame.
    pub start_frame: f64,
    /// End frame.
    pub end_frame: f64,
    /// Preview start (for range playback).
    pub preview_start: f64,
    /// Preview end.
    pub preview_end: f64,
    /// Use preview range.
    pub use_preview_range: bool,
    /// Frame rate (fps).
    pub frame_rate: f64,
    /// Playback state.
    pub state: PlaybackState,
    /// Playback mode.
    pub mode: PlaybackMode,
    /// Playback speed multiplier.
    pub speed: f64,
    /// Time display format.
    pub time_format: TimeFormat,
    /// Snap to frames.
    pub snap_to_frames: bool,
    /// Audio sync enabled.
    pub sync_audio: bool,
    /// Frame drop enabled (maintain realtime).
    pub frame_drop: bool,
    /// Markers.
    markers: HashMap<String, TimelineMarker>,
    /// Key poses (important frames).
    key_poses: Vec<f64>,
    /// Playback direction (1 or -1).
    direction: f64,
    /// Accumulated time for sub-frame accuracy.
    accumulated_time: f64,
}

impl Timeline {
    /// Create a new timeline.
    pub fn new() -> Self {
        Self {
            current_frame: 0.0,
            start_frame: 0.0,
            end_frame: 250.0,
            preview_start: 0.0,
            preview_end: 100.0,
            use_preview_range: false,
            frame_rate: 24.0,
            state: PlaybackState::Stopped,
            mode: PlaybackMode::Loop,
            speed: 1.0,
            time_format: TimeFormat::Frames,
            snap_to_frames: true,
            sync_audio: false,
            frame_drop: true,
            markers: HashMap::new(),
            key_poses: Vec::new(),
            direction: 1.0,
            accumulated_time: 0.0,
        }
    }

    /// Get current frame.
    pub fn current_frame(&self) -> f64 {
        self.current_frame
    }

    /// Get current time in seconds.
    pub fn current_time(&self) -> f64 {
        self.current_frame / self.frame_rate
    }

    /// Set current frame.
    pub fn set_frame(&mut self, frame: f64) {
        self.current_frame = if self.snap_to_frames {
            frame.round()
        } else {
            frame
        };
        self.clamp_to_range();
    }

    /// Set current time in seconds.
    pub fn set_time(&mut self, time: f64) {
        self.set_frame(time * self.frame_rate);
    }

    /// Get effective playback range.
    pub fn playback_range(&self) -> (f64, f64) {
        if self.use_preview_range {
            (self.preview_start, self.preview_end)
        } else {
            (self.start_frame, self.end_frame)
        }
    }

    /// Get duration in frames.
    pub fn duration_frames(&self) -> f64 {
        let (start, end) = self.playback_range();
        end - start
    }

    /// Get duration in seconds.
    pub fn duration_seconds(&self) -> f64 {
        self.duration_frames() / self.frame_rate
    }

    /// Clamp current frame to range.
    fn clamp_to_range(&mut self) {
        let (start, end) = self.playback_range();
        self.current_frame = self.current_frame.clamp(start, end);
    }

    /// Start playback.
    pub fn play(&mut self) {
        self.state = PlaybackState::Playing;
        self.direction = 1.0;
        self.accumulated_time = 0.0;
    }

    /// Play in reverse.
    pub fn play_reverse(&mut self) {
        self.state = PlaybackState::Reverse;
        self.direction = -1.0;
        self.accumulated_time = 0.0;
    }

    /// Pause playback.
    pub fn pause(&mut self) {
        if self.state == PlaybackState::Playing || self.state == PlaybackState::Reverse {
            self.state = PlaybackState::Paused;
        }
    }

    /// Resume from pause.
    pub fn resume(&mut self) {
        if self.state == PlaybackState::Paused {
            self.state = if self.direction > 0.0 {
                PlaybackState::Playing
            } else {
                PlaybackState::Reverse
            };
        }
    }

    /// Stop playback.
    pub fn stop(&mut self) {
        self.state = PlaybackState::Stopped;
        self.accumulated_time = 0.0;
    }

    /// Toggle play/pause.
    pub fn toggle_playback(&mut self) {
        match self.state {
            PlaybackState::Stopped | PlaybackState::Paused => self.play(),
            PlaybackState::Playing | PlaybackState::Reverse => self.pause(),
        }
    }

    /// Is playing (forward or reverse).
    pub fn is_playing(&self) -> bool {
        matches!(self.state, PlaybackState::Playing | PlaybackState::Reverse)
    }

    /// Go to start of range.
    pub fn goto_start(&mut self) {
        let (start, _) = self.playback_range();
        self.set_frame(start);
    }

    /// Go to end of range.
    pub fn goto_end(&mut self) {
        let (_, end) = self.playback_range();
        self.set_frame(end);
    }

    /// Step forward one frame.
    pub fn step_forward(&mut self) {
        self.current_frame += 1.0;
        self.handle_wrap();
        if self.snap_to_frames {
            self.current_frame = self.current_frame.round();
        }
    }

    /// Step backward one frame.
    pub fn step_backward(&mut self) {
        self.current_frame -= 1.0;
        self.handle_wrap();
        if self.snap_to_frames {
            self.current_frame = self.current_frame.round();
        }
    }

    /// Update timeline (call each frame with delta time).
    pub fn update(&mut self, dt: f64) -> bool {
        if !self.is_playing() {
            return false;
        }

        // Accumulate time
        self.accumulated_time += dt * self.speed * self.direction;

        // Convert to frames
        let frame_time = 1.0 / self.frame_rate;
        let mut frame_changed = false;

        while self.accumulated_time.abs() >= frame_time {
            if self.accumulated_time > 0.0 {
                self.current_frame += 1.0;
                self.accumulated_time -= frame_time;
            } else {
                self.current_frame -= 1.0;
                self.accumulated_time += frame_time;
            }
            frame_changed = true;
            self.handle_wrap();
        }

        // Handle frame dropping
        if self.frame_drop && self.accumulated_time.abs() > frame_time * 2.0 {
            let skip_frames = (self.accumulated_time / frame_time).floor();
            self.current_frame += skip_frames;
            self.accumulated_time -= skip_frames * frame_time;
            frame_changed = true;
            self.handle_wrap();
        }

        if self.snap_to_frames {
            self.current_frame = self.current_frame.round();
        }

        frame_changed
    }

    /// Handle wrapping at range boundaries.
    fn handle_wrap(&mut self) {
        let (start, end) = self.playback_range();

        match self.mode {
            PlaybackMode::Once => {
                if self.current_frame >= end || self.current_frame <= start {
                    self.current_frame = self.current_frame.clamp(start, end);
                    self.stop();
                }
            }
            PlaybackMode::Loop => {
                if self.current_frame > end {
                    self.current_frame = start + (self.current_frame - end - 1.0);
                } else if self.current_frame < start {
                    self.current_frame = end - (start - self.current_frame - 1.0);
                }
            }
            PlaybackMode::PingPong => {
                if self.current_frame > end {
                    self.current_frame = end - (self.current_frame - end);
                    self.direction = -1.0;
                    self.state = PlaybackState::Reverse;
                } else if self.current_frame < start {
                    self.current_frame = start + (start - self.current_frame);
                    self.direction = 1.0;
                    self.state = PlaybackState::Playing;
                }
            }
        }
    }

    /// Add a marker.
    pub fn add_marker(&mut self, name: impl Into<String>, frame: f64) {
        let name = name.into();
        self.markers.insert(
            name.clone(),
            TimelineMarker {
                name,
                frame,
                color: [1.0, 1.0, 1.0],
            },
        );
    }

    /// Remove a marker.
    pub fn remove_marker(&mut self, name: &str) {
        self.markers.remove(name);
    }

    /// Get marker by name.
    pub fn get_marker(&self, name: &str) -> Option<&TimelineMarker> {
        self.markers.get(name)
    }

    /// Get all markers.
    pub fn markers(&self) -> impl Iterator<Item = &TimelineMarker> {
        self.markers.values()
    }

    /// Go to marker.
    pub fn goto_marker(&mut self, name: &str) -> bool {
        if let Some(marker) = self.markers.get(name) {
            self.set_frame(marker.frame);
            true
        } else {
            false
        }
    }

    /// Go to next marker.
    pub fn goto_next_marker(&mut self) {
        let current = self.current_frame;
        if let Some(marker) = self
            .markers
            .values()
            .filter(|m| m.frame > current)
            .min_by(|a, b| a.frame.partial_cmp(&b.frame).unwrap())
        {
            self.set_frame(marker.frame);
        }
    }

    /// Go to previous marker.
    pub fn goto_prev_marker(&mut self) {
        let current = self.current_frame;
        if let Some(marker) = self
            .markers
            .values()
            .filter(|m| m.frame < current)
            .max_by(|a, b| a.frame.partial_cmp(&b.frame).unwrap())
        {
            self.set_frame(marker.frame);
        }
    }

    /// Add key pose frame.
    pub fn add_key_pose(&mut self, frame: f64) {
        if !self.key_poses.contains(&frame) {
            self.key_poses.push(frame);
            self.key_poses.sort_by(|a, b| a.partial_cmp(b).unwrap());
        }
    }

    /// Remove key pose.
    pub fn remove_key_pose(&mut self, frame: f64) {
        self.key_poses.retain(|&f| (f - frame).abs() > 0.5);
    }

    /// Go to next key pose.
    pub fn goto_next_key_pose(&mut self) {
        let current = self.current_frame;
        if let Some(&frame) = self.key_poses.iter().find(|&&f| f > current + 0.5) {
            self.set_frame(frame);
        }
    }

    /// Go to previous key pose.
    pub fn goto_prev_key_pose(&mut self) {
        let current = self.current_frame;
        if let Some(&frame) = self.key_poses.iter().rev().find(|&&f| f < current - 0.5) {
            self.set_frame(frame);
        }
    }

    /// Format time for display.
    pub fn format_time(&self, frame: f64) -> String {
        match self.time_format {
            TimeFormat::Frames => format!("{:.0}", frame),
            TimeFormat::Seconds => format!("{:.2}s", frame / self.frame_rate),
            TimeFormat::Timecode => {
                let total_seconds = frame / self.frame_rate;
                let hours = (total_seconds / 3600.0).floor() as u32;
                let minutes = ((total_seconds % 3600.0) / 60.0).floor() as u32;
                let seconds = (total_seconds % 60.0).floor() as u32;
                let frames = (frame % self.frame_rate).floor() as u32;
                format!("{:02}:{:02}:{:02}:{:02}", hours, minutes, seconds, frames)
            }
        }
    }

    /// Get playback progress (0-1).
    pub fn progress(&self) -> f64 {
        let (start, end) = self.playback_range();
        let duration = end - start;
        if duration > 0.0 {
            (self.current_frame - start) / duration
        } else {
            0.0
        }
    }

    /// Set playback progress (0-1).
    pub fn set_progress(&mut self, progress: f64) {
        let (start, end) = self.playback_range();
        self.set_frame(start + (end - start) * progress.clamp(0.0, 1.0));
    }
}

impl Default for Timeline {
    fn default() -> Self {
        Self::new()
    }
}

/// Timeline marker.
#[derive(Debug, Clone)]
pub struct TimelineMarker {
    /// Marker name.
    pub name: String,
    /// Frame position.
    pub frame: f64,
    /// Display color.
    pub color: [f64; 3],
}

/// Timeline track for organization.
#[derive(Debug, Clone)]
pub struct TimelineTrack {
    /// Track name.
    pub name: String,
    /// Track color.
    pub color: [f64; 3],
    /// Is track visible.
    pub visible: bool,
    /// Is track locked.
    pub locked: bool,
    /// Is track solo.
    pub solo: bool,
    /// Is track muted.
    pub muted: bool,
    /// Track height (in UI units).
    pub height: f64,
    /// Child tracks.
    pub children: Vec<TimelineTrack>,
    /// Is expanded.
    pub expanded: bool,
}

impl TimelineTrack {
    /// Create a new track.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            color: [0.5, 0.5, 0.5],
            visible: true,
            locked: false,
            solo: false,
            muted: false,
            height: 24.0,
            children: Vec::new(),
            expanded: true,
        }
    }
}

/// Onion skinning settings.
#[derive(Debug, Clone)]
pub struct OnionSkinning {
    /// Is enabled.
    pub enabled: bool,
    /// Frames before.
    pub before_count: usize,
    /// Frames after.
    pub after_count: usize,
    /// Step between frames.
    pub step: usize,
    /// Before color.
    pub before_color: [f64; 4],
    /// After color.
    pub after_color: [f64; 4],
    /// Opacity falloff.
    pub opacity_falloff: f64,
}

impl Default for OnionSkinning {
    fn default() -> Self {
        Self {
            enabled: false,
            before_count: 3,
            after_count: 3,
            step: 1,
            before_color: [0.0, 1.0, 0.0, 0.5],
            after_color: [0.0, 0.0, 1.0, 0.5],
            opacity_falloff: 0.7,
        }
    }
}

impl OnionSkinning {
    /// Get frames to display.
    pub fn get_frames(&self, current: f64) -> Vec<(f64, [f64; 4])> {
        if !self.enabled {
            return Vec::new();
        }

        let mut frames = Vec::new();

        // Before frames
        for i in 1..=self.before_count {
            let frame = current - (i * self.step) as f64;
            let opacity = self.before_color[3] * self.opacity_falloff.powi(i as i32);
            let mut color = self.before_color;
            color[3] = opacity;
            frames.push((frame, color));
        }

        // After frames
        for i in 1..=self.after_count {
            let frame = current + (i * self.step) as f64;
            let opacity = self.after_color[3] * self.opacity_falloff.powi(i as i32);
            let mut color = self.after_color;
            color[3] = opacity;
            frames.push((frame, color));
        }

        frames
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeline_creation() {
        let timeline = Timeline::new();
        assert_eq!(timeline.current_frame(), 0.0);
        assert_eq!(timeline.frame_rate, 24.0);
    }

    #[test]
    fn test_frame_setting() {
        let mut timeline = Timeline::new();
        timeline.set_frame(50.0);
        assert_eq!(timeline.current_frame(), 50.0);
    }

    #[test]
    fn test_time_conversion() {
        let mut timeline = Timeline::new();
        timeline.frame_rate = 30.0;
        timeline.set_time(2.0);
        assert!((timeline.current_frame() - 60.0).abs() < 0.5);
    }

    #[test]
    fn test_playback() {
        let mut timeline = Timeline::new();
        timeline.play();
        assert!(timeline.is_playing());
        timeline.pause();
        assert_eq!(timeline.state, PlaybackState::Paused);
        timeline.stop();
        assert_eq!(timeline.state, PlaybackState::Stopped);
    }

    #[test]
    fn test_step() {
        let mut timeline = Timeline::new();
        timeline.set_frame(10.0);
        timeline.step_forward();
        assert_eq!(timeline.current_frame(), 11.0);
        timeline.step_backward();
        assert_eq!(timeline.current_frame(), 10.0);
    }

    #[test]
    fn test_markers() {
        let mut timeline = Timeline::new();
        timeline.add_marker("start", 0.0);
        timeline.add_marker("middle", 50.0);
        timeline.add_marker("end", 100.0);

        assert!(timeline.goto_marker("middle"));
        assert_eq!(timeline.current_frame(), 50.0);
    }

    #[test]
    fn test_timecode_format() {
        let mut timeline = Timeline::new();
        timeline.frame_rate = 24.0;
        timeline.time_format = TimeFormat::Timecode;

        // 2 hours, 30 minutes, 15 seconds, 12 frames
        let frame = (2.0 * 3600.0 + 30.0 * 60.0 + 15.0) * 24.0 + 12.0;
        let formatted = timeline.format_time(frame);
        assert_eq!(formatted, "02:30:15:12");
    }

    #[test]
    fn test_loop_wrap() {
        let mut timeline = Timeline::new();
        timeline.start_frame = 0.0;
        timeline.end_frame = 100.0;
        timeline.mode = PlaybackMode::Loop;

        timeline.set_frame(100.0);
        timeline.step_forward();
        // Should wrap to near start
        assert!(timeline.current_frame() < 10.0);
    }
}
