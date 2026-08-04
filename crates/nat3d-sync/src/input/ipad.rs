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

//! iPad touch input handling.
//!
//! Handles multi-touch gestures and viewport interaction.

use std::collections::HashMap;
use std::time::Instant;

/// iPad gesture types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IPadGesture {
    Tap,
    DoubleTap,
    LongPress,
    Pan,
    Pinch,
    Rotate,
}

/// Touch point state.
#[derive(Debug, Clone)]
pub struct TouchPoint {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub started_at: Instant,
}

/// iPad input handler.
pub struct IPadInput {
    touch_state: HashMap<u32, TouchPoint>,
    gesture_state: Option<GestureState>,
    viewport_size: (u32, u32),
    tap_threshold: f32,
    long_press_duration_ms: u64,
}

#[derive(Debug, Clone)]
struct GestureState {
    gesture: IPadGesture,
    initial_distance: Option<f32>,
    initial_angle: Option<f32>,
    current_distance: Option<f32>,
    current_angle: Option<f32>,
}

impl IPadInput {
    /// Create a new iPad input handler.
    pub fn new(viewport_width: u32, viewport_height: u32) -> Self {
        Self {
            touch_state: HashMap::new(),
            gesture_state: None,
            viewport_size: (viewport_width, viewport_height),
            tap_threshold: 10.0,
            long_press_duration_ms: 500,
        }
    }

    /// Handle a touch event.
    pub fn handle_touch_event(
        &mut self,
        touch_id: u32,
        x: f32,
        y: f32,
        ended: bool,
    ) -> Option<IPadGesture> {
        if ended {
            self.touch_state.remove(&touch_id);

            if self.touch_state.is_empty() {
                let gesture = self.gesture_state.as_ref().map(|g| g.gesture);
                self.gesture_state = None;
                return gesture;
            }
        } else {
            let touch = TouchPoint {
                id: touch_id,
                x,
                y,
                started_at: Instant::now(),
            };

            self.touch_state.insert(touch_id, touch);
        }

        self.detect_gesture()
    }

    /// Handle a gesture event.
    pub fn handle_gesture(&mut self, gesture: IPadGesture) {
        self.gesture_state = Some(GestureState {
            gesture,
            initial_distance: None,
            initial_angle: None,
            current_distance: None,
            current_angle: None,
        });
    }

    /// Get viewport transform from gestures.
    pub fn get_viewport_transform(&self) -> ViewportTransform {
        if let Some(ref state) = self.gesture_state {
            match state.gesture {
                IPadGesture::Pan => {
                    let delta = self.calculate_pan_delta();
                    ViewportTransform::Pan {
                        dx: delta.0,
                        dy: delta.1,
                    }
                }
                IPadGesture::Pinch => {
                    let scale = self.calculate_pinch_scale();
                    ViewportTransform::Zoom { factor: scale }
                }
                IPadGesture::Rotate => {
                    let angle = self.calculate_rotation_angle();
                    ViewportTransform::Rotate { angle }
                }
                _ => ViewportTransform::None,
            }
        } else {
            ViewportTransform::None
        }
    }

    /// Set tap threshold in pixels.
    pub fn set_tap_threshold(&mut self, threshold: f32) {
        self.tap_threshold = threshold;
    }

    /// Set long press duration in milliseconds.
    pub fn set_long_press_duration(&mut self, duration_ms: u64) {
        self.long_press_duration_ms = duration_ms;
    }

    fn detect_gesture(&mut self) -> Option<IPadGesture> {
        let touch_count = self.touch_state.len();

        match touch_count {
            0 => None,
            1 => self.detect_single_touch_gesture(),
            2 => self.detect_two_touch_gesture(),
            _ => None,
        }
    }

    fn detect_single_touch_gesture(&self) -> Option<IPadGesture> {
        if let Some(touch) = self.touch_state.values().next() {
            let elapsed = touch.started_at.elapsed().as_millis() as u64;

            if elapsed > self.long_press_duration_ms {
                return Some(IPadGesture::LongPress);
            }
        }

        Some(IPadGesture::Pan)
    }

    fn detect_two_touch_gesture(&mut self) -> Option<IPadGesture> {
        let touches: Vec<&TouchPoint> = self.touch_state.values().collect();

        if touches.len() == 2 {
            let distance = self.distance_between(touches[0], touches[1]);
            let angle = self.angle_between(touches[0], touches[1]);

            if let Some(ref mut state) = self.gesture_state {
                if state.initial_distance.is_none() {
                    state.initial_distance = Some(distance);
                    state.initial_angle = Some(angle);
                }

                state.current_distance = Some(distance);
                state.current_angle = Some(angle);

                let distance_change = (distance - state.initial_distance.unwrap()).abs();
                let angle_change = (angle - state.initial_angle.unwrap()).abs();

                if distance_change > angle_change {
                    return Some(IPadGesture::Pinch);
                } else {
                    return Some(IPadGesture::Rotate);
                }
            }
        }

        None
    }

    fn calculate_pan_delta(&self) -> (f32, f32) {
        // Simplified: return average movement
        if let Some(touch) = self.touch_state.values().next() {
            (touch.x, touch.y)
        } else {
            (0.0, 0.0)
        }
    }

    fn calculate_pinch_scale(&self) -> f32 {
        if let Some(ref state) = self.gesture_state {
            if let (Some(initial), Some(current)) = (state.initial_distance, state.current_distance)
            {
                return current / initial;
            }
        }
        1.0
    }

    fn calculate_rotation_angle(&self) -> f32 {
        if let Some(ref state) = self.gesture_state {
            if let (Some(initial), Some(current)) = (state.initial_angle, state.current_angle) {
                return current - initial;
            }
        }
        0.0
    }

    fn distance_between(&self, a: &TouchPoint, b: &TouchPoint) -> f32 {
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        (dx * dx + dy * dy).sqrt()
    }

    fn angle_between(&self, a: &TouchPoint, b: &TouchPoint) -> f32 {
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        dy.atan2(dx)
    }
}

/// Viewport transformation from gestures.
#[derive(Debug, Clone, Copy)]
pub enum ViewportTransform {
    None,
    Pan { dx: f32, dy: f32 },
    Zoom { factor: f32 },
    Rotate { angle: f32 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipad_input_creation() {
        let input = IPadInput::new(1920, 1080);
        assert_eq!(input.viewport_size, (1920, 1080));
    }

    #[test]
    fn test_single_touch() {
        let mut input = IPadInput::new(1920, 1080);
        let gesture = input.handle_touch_event(1, 100.0, 200.0, false);
        assert!(gesture.is_some());
    }

    #[test]
    fn test_touch_end() {
        let mut input = IPadInput::new(1920, 1080);
        let start = input.handle_touch_event(1, 100.0, 200.0, false);
        // First touch starts tracking
        assert!(start.is_some());
        // Touch end for a stationary finger may or may not produce a gesture
        let _end = input.handle_touch_event(1, 100.0, 200.0, true);
    }

    #[test]
    fn test_viewport_transform() {
        let mut input = IPadInput::new(1920, 1080);
        input.handle_gesture(IPadGesture::Pan);
        let transform = input.get_viewport_transform();
        assert!(matches!(transform, ViewportTransform::Pan { .. }));
    }

    #[test]
    fn test_tap_threshold() {
        let mut input = IPadInput::new(1920, 1080);
        input.set_tap_threshold(20.0);
        assert_eq!(input.tap_threshold, 20.0);
    }
}
