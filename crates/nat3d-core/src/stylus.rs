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

//! Stylus input abstraction for pressure-sensitive devices.
//!
//! Provides a unified interface for stylus input from:
//! - Apple Pencil (native iOS via FFI)
//! - Wacom tablets (native driver API)
//! - Generic tablet devices (WinTab, libinput)
//! - Remote stylus over TCP (iPad as input device for desktop)
//!
//! # Architecture
//!
//! The abstraction separates data (`StylusInput`, `StylusEvent`) from providers
//! (`StylusProvider` trait). This allows:
//! - Native backends compile to iOS/Android without network code
//! - TCP backend reuses same structs for remote input
//! - Application code is provider-agnostic
//!
//! # Example
//!
//! ```ignore
//! use nat3d_core::stylus::{StylusInput, StylusProvider, StylusEvent};
//!
//! fn handle_stylus<P: StylusProvider>(provider: &mut P) {
//!     while let Some(event) = provider.poll() {
//!         match event {
//!             StylusEvent::Down(input) => start_stroke(input),
//!             StylusEvent::Move(input) => continue_stroke(input),
//!             StylusEvent::Up(input) => end_stroke(input),
//!         }
//!     }
//! }
//! ```

use std::time::{Duration, Instant};

/// Stylus input sample with pressure, tilt, and position.
///
/// All values are normalized to consistent ranges regardless of hardware:
/// - `pressure`: 0.0 (no contact) to 1.0 (max force)
/// - `tilt_altitude`: 0.0 (parallel to surface) to π/2 (perpendicular)
/// - `tilt_azimuth`: 0.0 to 2π (rotation around perpendicular axis)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StylusInput {
    /// X position in normalized device coordinates (0.0 to 1.0).
    pub x: f32,
    /// Y position in normalized device coordinates (0.0 to 1.0).
    pub y: f32,
    /// Pressure from 0.0 (hover/no contact) to 1.0 (maximum force).
    pub pressure: f32,
    /// Altitude angle in radians: 0 = parallel to surface, π/2 = perpendicular.
    pub tilt_altitude: f32,
    /// Azimuth angle in radians: rotation of tilt direction (0 to 2π).
    pub tilt_azimuth: f32,
    /// Timestamp of this sample (monotonic, relative to provider start).
    pub timestamp_ms: u64,
    /// Optional barrel button state (true = pressed).
    pub barrel_button: bool,
    /// Optional eraser mode (true = using eraser end).
    pub eraser: bool,
}

impl StylusInput {
    /// Create a new stylus input with default values.
    pub fn new(x: f32, y: f32, pressure: f32) -> Self {
        Self {
            x,
            y,
            pressure,
            tilt_altitude: std::f32::consts::FRAC_PI_2, // perpendicular default
            tilt_azimuth: 0.0,
            timestamp_ms: 0,
            barrel_button: false,
            eraser: false,
        }
    }

    /// Create input with full tilt information.
    pub fn with_tilt(mut self, altitude: f32, azimuth: f32) -> Self {
        self.tilt_altitude = altitude;
        self.tilt_azimuth = azimuth;
        self
    }

    /// Set timestamp.
    pub fn with_timestamp(mut self, timestamp_ms: u64) -> Self {
        self.timestamp_ms = timestamp_ms;
        self
    }

    /// Set barrel button state.
    pub fn with_barrel_button(mut self, pressed: bool) -> Self {
        self.barrel_button = pressed;
        self
    }

    /// Set eraser mode.
    pub fn with_eraser(mut self, eraser: bool) -> Self {
        self.eraser = eraser;
        self
    }

    /// Convert normalized coordinates to pixel coordinates.
    pub fn to_pixels(&self, width: u32, height: u32) -> (f32, f32) {
        (self.x * width as f32, self.y * height as f32)
    }

    /// Calculate brush size multiplier based on pressure (for sculpt/paint).
    pub fn pressure_size(&self, min_size: f32, max_size: f32) -> f32 {
        min_size + self.pressure * (max_size - min_size)
    }

    /// Calculate brush opacity based on pressure.
    pub fn pressure_opacity(&self, min_opacity: f32, max_opacity: f32) -> f32 {
        min_opacity + self.pressure * (max_opacity - min_opacity)
    }

    /// Get tilt as a 2D direction vector (for angled brushes).
    pub fn tilt_direction(&self) -> [f32; 2] {
        let horizontal = (std::f32::consts::FRAC_PI_2 - self.tilt_altitude).cos();
        [
            horizontal * self.tilt_azimuth.cos(),
            horizontal * self.tilt_azimuth.sin(),
        ]
    }
}

impl Default for StylusInput {
    fn default() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }
}

/// Stylus event types for stroke handling.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StylusEvent {
    /// Stylus touched surface (start of stroke).
    Down(StylusInput),
    /// Stylus moved while in contact.
    Move(StylusInput),
    /// Stylus lifted from surface (end of stroke).
    Up(StylusInput),
    /// Stylus hovering above surface (if supported).
    Hover(StylusInput),
    /// Stylus left proximity sensor range.
    ProximityOut,
}

impl StylusEvent {
    /// Get the input data if this event contains one.
    pub fn input(&self) -> Option<&StylusInput> {
        match self {
            StylusEvent::Down(i)
            | StylusEvent::Move(i)
            | StylusEvent::Up(i)
            | StylusEvent::Hover(i) => Some(i),
            StylusEvent::ProximityOut => None,
        }
    }

    /// Check if this is a contact event (Down or Move).
    pub fn is_contact(&self) -> bool {
        matches!(self, StylusEvent::Down(_) | StylusEvent::Move(_))
    }

    /// Check if this is the start of a stroke.
    pub fn is_stroke_start(&self) -> bool {
        matches!(self, StylusEvent::Down(_))
    }

    /// Check if this is the end of a stroke.
    pub fn is_stroke_end(&self) -> bool {
        matches!(self, StylusEvent::Up(_))
    }
}

/// Stylus device capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StylusCapabilities {
    /// Device supports pressure sensitivity.
    pub pressure: bool,
    /// Device supports tilt detection.
    pub tilt: bool,
    /// Device supports hover detection.
    pub hover: bool,
    /// Device has barrel button(s).
    pub barrel_button: bool,
    /// Device has eraser end.
    pub eraser: bool,
    /// Number of pressure levels (e.g., 4096 for Apple Pencil 2).
    pub pressure_levels: u16,
}

impl StylusCapabilities {
    /// Apple Pencil (1st/2nd gen) capabilities.
    pub fn apple_pencil() -> Self {
        Self {
            pressure: true,
            tilt: true,
            hover: true, // Pencil 2 with compatible iPad
            barrel_button: false,
            eraser: false,
            pressure_levels: 4096,
        }
    }

    /// Wacom Intuos Pro capabilities.
    pub fn wacom_intuos_pro() -> Self {
        Self {
            pressure: true,
            tilt: true,
            hover: true,
            barrel_button: true,
            eraser: true,
            pressure_levels: 8192,
        }
    }

    /// Basic stylus (pressure only).
    pub fn basic() -> Self {
        Self {
            pressure: true,
            tilt: false,
            hover: false,
            barrel_button: false,
            eraser: false,
            pressure_levels: 1024,
        }
    }
}

/// Trait for stylus input providers.
///
/// Implement this trait to add support for a new stylus device or input source.
/// The trait is object-safe and Send+Sync for use across threads.
pub trait StylusProvider: Send + Sync {
    /// Poll for the next stylus event.
    ///
    /// Returns `Some(event)` if an event is available, `None` otherwise.
    /// This is non-blocking; use in a loop or with async polling.
    fn poll(&mut self) -> Option<StylusEvent>;

    /// Get device capabilities.
    fn capabilities(&self) -> StylusCapabilities;

    /// Get device name/identifier.
    fn device_name(&self) -> &str;

    /// Check if device is still connected.
    fn is_connected(&self) -> bool;
}

/// Stylus stroke accumulator for drawing/sculpting.
///
/// Collects stylus samples into a stroke with smoothing and interpolation.
#[derive(Debug, Clone)]
pub struct StylusStroke {
    /// Raw input samples.
    pub samples: Vec<StylusInput>,
    /// Start time of stroke.
    pub start_time: Option<Instant>,
    /// Whether stroke is complete (Up received).
    pub completed: bool,
}

impl StylusStroke {
    /// Create a new empty stroke.
    pub fn new() -> Self {
        Self {
            samples: Vec::with_capacity(256),
            start_time: None,
            completed: false,
        }
    }

    /// Add a sample to the stroke.
    pub fn add_sample(&mut self, input: StylusInput) {
        if self.start_time.is_none() {
            self.start_time = Some(Instant::now());
        }
        self.samples.push(input);
    }

    /// Mark stroke as completed.
    pub fn complete(&mut self) {
        self.completed = true;
    }

    /// Get stroke duration.
    pub fn duration(&self) -> Option<Duration> {
        self.start_time.map(|t| t.elapsed())
    }

    /// Get average pressure of stroke.
    pub fn average_pressure(&self) -> f32 {
        if self.samples.is_empty() {
            return 0.0;
        }
        self.samples.iter().map(|s| s.pressure).sum::<f32>() / self.samples.len() as f32
    }

    /// Get stroke bounding box in normalized coordinates.
    pub fn bounds(&self) -> Option<(f32, f32, f32, f32)> {
        if self.samples.is_empty() {
            return None;
        }
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        for s in &self.samples {
            min_x = min_x.min(s.x);
            min_y = min_y.min(s.y);
            max_x = max_x.max(s.x);
            max_y = max_y.max(s.y);
        }
        Some((min_x, min_y, max_x, max_y))
    }

    /// Simplify stroke using Ramer-Douglas-Peucker algorithm.
    pub fn simplify(&self, epsilon: f32) -> Vec<StylusInput> {
        if self.samples.len() <= 2 {
            return self.samples.clone();
        }
        rdp_simplify(&self.samples, epsilon)
    }
}

impl Default for StylusStroke {
    fn default() -> Self {
        Self::new()
    }
}

/// Ramer-Douglas-Peucker line simplification.
fn rdp_simplify(points: &[StylusInput], epsilon: f32) -> Vec<StylusInput> {
    if points.len() <= 2 {
        return points.to_vec();
    }

    let first = &points[0];
    let last = &points[points.len() - 1];

    let mut max_dist = 0.0f32;
    let mut max_idx = 0;

    for (i, p) in points.iter().enumerate().skip(1).take(points.len() - 2) {
        let dist = perpendicular_distance(p, first, last);
        if dist > max_dist {
            max_dist = dist;
            max_idx = i;
        }
    }

    if max_dist > epsilon {
        let mut left = rdp_simplify(&points[..=max_idx], epsilon);
        let right = rdp_simplify(&points[max_idx..], epsilon);
        left.pop(); // remove duplicate point
        left.extend(right);
        left
    } else {
        vec![first.clone(), last.clone()]
    }
}

fn perpendicular_distance(
    p: &StylusInput,
    line_start: &StylusInput,
    line_end: &StylusInput,
) -> f32 {
    let dx = line_end.x - line_start.x;
    let dy = line_end.y - line_start.y;
    let len_sq = dx * dx + dy * dy;

    if len_sq < 1e-10 {
        let dx = p.x - line_start.x;
        let dy = p.y - line_start.y;
        return (dx * dx + dy * dy).sqrt();
    }

    let t = ((p.x - line_start.x) * dx + (p.y - line_start.y) * dy) / len_sq;
    let t = t.clamp(0.0, 1.0);

    let proj_x = line_start.x + t * dx;
    let proj_y = line_start.y + t * dy;

    let dx = p.x - proj_x;
    let dy = p.y - proj_y;
    (dx * dx + dy * dy).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stylus_input_creation() {
        let input = StylusInput::new(0.5, 0.5, 0.8);
        assert_eq!(input.x, 0.5);
        assert_eq!(input.y, 0.5);
        assert_eq!(input.pressure, 0.8);
    }

    #[test]
    fn stylus_input_with_tilt() {
        let input = StylusInput::new(0.5, 0.5, 1.0).with_tilt(0.7, 1.5);
        assert_eq!(input.tilt_altitude, 0.7);
        assert_eq!(input.tilt_azimuth, 1.5);
    }

    #[test]
    fn stylus_input_to_pixels() {
        let input = StylusInput::new(0.5, 0.25, 1.0);
        let (px, py) = input.to_pixels(1920, 1080);
        assert_eq!(px, 960.0);
        assert_eq!(py, 270.0);
    }

    #[test]
    fn pressure_size_interpolation() {
        let input = StylusInput::new(0.0, 0.0, 0.5);
        let size = input.pressure_size(1.0, 10.0);
        assert!((size - 5.5).abs() < 0.001);
    }

    #[test]
    fn stroke_accumulation() {
        let mut stroke = StylusStroke::new();
        stroke.add_sample(StylusInput::new(0.0, 0.0, 0.5));
        stroke.add_sample(StylusInput::new(0.5, 0.5, 0.7));
        stroke.add_sample(StylusInput::new(1.0, 1.0, 0.9));

        assert_eq!(stroke.samples.len(), 3);
        assert!((stroke.average_pressure() - 0.7).abs() < 0.001);
    }

    #[test]
    fn stroke_bounds() {
        let mut stroke = StylusStroke::new();
        stroke.add_sample(StylusInput::new(0.2, 0.3, 1.0));
        stroke.add_sample(StylusInput::new(0.8, 0.1, 1.0));
        stroke.add_sample(StylusInput::new(0.5, 0.9, 1.0));

        let bounds = stroke.bounds().unwrap();
        assert_eq!(bounds, (0.2, 0.1, 0.8, 0.9));
    }

    #[test]
    fn event_is_contact() {
        let input = StylusInput::default();
        assert!(StylusEvent::Down(input).is_contact());
        assert!(StylusEvent::Move(input).is_contact());
        assert!(!StylusEvent::Up(input).is_contact());
        assert!(!StylusEvent::Hover(input).is_contact());
    }

    #[test]
    fn capabilities_presets() {
        let pencil = StylusCapabilities::apple_pencil();
        assert!(pencil.pressure);
        assert!(pencil.tilt);
        assert_eq!(pencil.pressure_levels, 4096);

        let wacom = StylusCapabilities::wacom_intuos_pro();
        assert!(wacom.eraser);
        assert_eq!(wacom.pressure_levels, 8192);
    }

    #[test]
    fn rdp_simplify_basic() {
        let stroke = StylusStroke {
            samples: vec![
                StylusInput::new(0.0, 0.0, 1.0),
                StylusInput::new(0.1, 0.001, 1.0), // nearly on line
                StylusInput::new(0.2, 0.002, 1.0), // nearly on line
                StylusInput::new(0.3, 0.0, 1.0),
            ],
            start_time: None,
            completed: true,
        };

        let simplified = stroke.simplify(0.01);
        assert!(simplified.len() < stroke.samples.len());
    }
}
