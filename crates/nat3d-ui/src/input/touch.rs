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

//! Multi-touch gesture handling for touchscreens and trackpads.

use nalgebra::Vector2;
use std::collections::HashMap;

/// Touch point identifier.
pub type TouchId = u64;

/// Individual touch point.
#[derive(Debug, Clone)]
pub struct TouchPoint {
    /// Unique touch identifier.
    pub id: TouchId,
    /// Current position (screen coordinates).
    pub position: Vector2<f32>,
    /// Previous position.
    pub previous_position: Vector2<f32>,
    /// Pressure (0.0-1.0, if supported).
    pub pressure: f32,
}

/// Touch gesture type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GestureType {
    /// No gesture.
    None,
    /// Single-finger tap.
    Tap,
    /// Two-finger pinch (zoom in/out).
    Pinch,
    /// Two-finger rotation.
    Rotate,
    /// Two-finger pan.
    Pan,
    /// Three-finger swipe.
    Swipe,
}

/// Touch gesture state.
#[derive(Debug, Clone)]
pub struct Gesture {
    /// Type of gesture.
    pub gesture_type: GestureType,
    /// Pinch scale factor (1.0 = no change, >1.0 = zoom in, <1.0 = zoom out).
    pub pinch_scale: f32,
    /// Rotation angle delta (radians).
    pub rotation_delta: f32,
    /// Pan delta (pixels).
    pub pan_delta: Vector2<f32>,
    /// Swipe direction (normalized).
    pub swipe_direction: Vector2<f32>,
}

impl Default for Gesture {
    fn default() -> Self {
        Self {
            gesture_type: GestureType::None,
            pinch_scale: 1.0,
            rotation_delta: 0.0,
            pan_delta: Vector2::zeros(),
            swipe_direction: Vector2::zeros(),
        }
    }
}

/// Multi-touch state tracker.
#[derive(Debug, Clone)]
pub struct TouchState {
    /// Active touch points.
    pub touches: HashMap<TouchId, TouchPoint>,
    /// Current gesture.
    pub gesture: Gesture,
    /// Previous two-touch distance (for pinch detection).
    prev_pinch_distance: f32,
    /// Previous two-touch angle (for rotation detection).
    prev_rotation_angle: f32,
}

impl Default for TouchState {
    fn default() -> Self {
        Self {
            touches: HashMap::new(),
            gesture: Gesture::default(),
            prev_pinch_distance: 0.0,
            prev_rotation_angle: 0.0,
        }
    }
}

impl TouchState {
    /// Create new touch state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add or update a touch point.
    pub fn touch_down(&mut self, id: TouchId, x: f32, y: f32, pressure: f32) {
        let position = Vector2::new(x, y);
        if let Some(touch) = self.touches.get_mut(&id) {
            touch.previous_position = touch.position;
            touch.position = position;
            touch.pressure = pressure.clamp(0.0, 1.0);
        } else {
            self.touches.insert(
                id,
                TouchPoint {
                    id,
                    position,
                    previous_position: position,
                    pressure: pressure.clamp(0.0, 1.0),
                },
            );
        }
    }

    /// Remove a touch point.
    pub fn touch_up(&mut self, id: TouchId) {
        self.touches.remove(&id);
        if self.touches.is_empty() {
            self.gesture = Gesture::default();
        }
    }

    /// Update gesture detection.
    pub fn update_gestures(&mut self) {
        let touch_count = self.touches.len();

        match touch_count {
            0 => {
                self.gesture = Gesture::default();
            }
            1 => {
                // Single touch - could be tap or pan
                self.gesture.gesture_type = GestureType::Tap;
            }
            2 => {
                // Two touches - detect pinch, rotate, or pan
                let touches: Vec<&TouchPoint> = self.touches.values().collect();
                let p1 = touches[0].position;
                let p2 = touches[1].position;

                let current_distance = (p2 - p1).norm();
                let current_angle = (p2.y - p1.y).atan2(p2.x - p1.x);

                if self.prev_pinch_distance > 0.0 {
                    // Pinch gesture
                    self.gesture.pinch_scale = current_distance / self.prev_pinch_distance;
                    if (self.gesture.pinch_scale - 1.0).abs() > 0.01 {
                        self.gesture.gesture_type = GestureType::Pinch;
                    }

                    // Rotation gesture
                    self.gesture.rotation_delta = current_angle - self.prev_rotation_angle;
                    if self.gesture.rotation_delta.abs() > 0.05 {
                        self.gesture.gesture_type = GestureType::Rotate;
                    }
                }

                self.prev_pinch_distance = current_distance;
                self.prev_rotation_angle = current_angle;

                // Pan gesture (average movement)
                let delta = (touches[0].position - touches[0].previous_position
                    + touches[1].position
                    - touches[1].previous_position)
                    * 0.5;
                if delta.norm() > 1.0 {
                    self.gesture.gesture_type = GestureType::Pan;
                    self.gesture.pan_delta = delta;
                }
            }
            3 => {
                // Three touches - swipe gesture
                let avg_delta: Vector2<f32> = self
                    .touches
                    .values()
                    .map(|t| t.position - t.previous_position)
                    .sum::<Vector2<f32>>()
                    / 3.0;

                if avg_delta.norm() > 10.0 {
                    self.gesture.gesture_type = GestureType::Swipe;
                    self.gesture.swipe_direction = avg_delta.normalize();
                }
            }
            _ => {
                // More than 3 touches - no specific gesture
                self.gesture.gesture_type = GestureType::None;
            }
        }
    }

    /// Get number of active touches.
    pub fn touch_count(&self) -> usize {
        self.touches.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_touch() {
        let mut touch = TouchState::new();
        touch.touch_down(1, 100.0, 100.0, 0.5);
        assert_eq!(touch.touch_count(), 1);

        touch.touch_up(1);
        assert_eq!(touch.touch_count(), 0);
    }

    #[test]
    fn test_pinch_gesture() {
        let mut touch = TouchState::new();

        // Start with two touches 100 pixels apart
        touch.touch_down(1, 100.0, 100.0, 1.0);
        touch.touch_down(2, 200.0, 100.0, 1.0);
        touch.update_gestures();

        // Move touches to 200 pixels apart (zoom in)
        touch.touch_down(1, 50.0, 100.0, 1.0);
        touch.touch_down(2, 250.0, 100.0, 1.0);
        touch.update_gestures();

        assert_eq!(touch.gesture.gesture_type, GestureType::Pinch);
        assert!(touch.gesture.pinch_scale > 1.5);
    }
}
