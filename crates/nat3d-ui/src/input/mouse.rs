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

//! Mouse input handling for NAT3D UI.
//!
//! Provides mouse button states, position tracking, drag detection,
//! and 3D ray casting for viewport interaction.

use nalgebra::{Point3, Vector2, Vector3};

/// Mouse button identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    /// Left mouse button (primary).
    Left,
    /// Right mouse button (context menu).
    Right,
    /// Middle mouse button (camera/view manipulation).
    Middle,
    /// Back button (browser-style navigation).
    Back,
    /// Forward button (browser-style navigation).
    Forward,
}

impl MouseButton {
    /// Get all standard mouse buttons.
    pub fn all() -> &'static [MouseButton] {
        &[
            MouseButton::Left,
            MouseButton::Right,
            MouseButton::Middle,
            MouseButton::Back,
            MouseButton::Forward,
        ]
    }
}

/// Mouse button state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonState {
    /// Button is not pressed.
    Released,
    /// Button was just pressed this frame.
    Pressed,
    /// Button is held down.
    Down,
    /// Button was just released this frame.
    JustReleased,
}

impl ButtonState {
    /// Check if button is pressed (down or just pressed).
    pub fn is_pressed(&self) -> bool {
        matches!(self, ButtonState::Pressed | ButtonState::Down)
    }

    /// Check if button was just pressed this frame.
    pub fn just_pressed(&self) -> bool {
        matches!(self, ButtonState::Pressed)
    }

    /// Check if button was just released this frame.
    pub fn just_released(&self) -> bool {
        matches!(self, ButtonState::JustReleased)
    }
}

/// Mouse drag state.
#[derive(Debug, Clone)]
pub struct DragState {
    /// Whether a drag is currently active.
    pub active: bool,
    /// Button being used for drag.
    pub button: MouseButton,
    /// Start position of drag (screen coordinates).
    pub start_pos: Vector2<f32>,
    /// Current drag delta from start.
    pub delta: Vector2<f32>,
    /// Total distance dragged.
    pub total_distance: f32,
}

impl Default for DragState {
    fn default() -> Self {
        Self {
            active: false,
            button: MouseButton::Left,
            start_pos: Vector2::zeros(),
            delta: Vector2::zeros(),
            total_distance: 0.0,
        }
    }
}

/// 3D ray for viewport picking.
#[derive(Debug, Clone)]
pub struct Ray3D {
    /// Ray origin (camera position).
    pub origin: Point3<f64>,
    /// Ray direction (normalized).
    pub direction: Vector3<f64>,
}

impl Ray3D {
    /// Create a new 3D ray.
    pub fn new(origin: Point3<f64>, direction: Vector3<f64>) -> Self {
        Self {
            origin,
            direction: direction.normalize(),
        }
    }

    /// Get a point along the ray at distance t.
    pub fn point_at(&self, t: f64) -> Point3<f64> {
        self.origin + self.direction * t
    }
}

/// Mouse input state tracker.
#[derive(Debug, Clone)]
pub struct MouseState {
    /// Current mouse position (screen coordinates, pixels).
    pub position: Vector2<f32>,
    /// Previous frame mouse position.
    pub previous_position: Vector2<f32>,
    /// Mouse movement delta this frame.
    pub delta: Vector2<f32>,
    /// Scroll wheel delta this frame (vertical).
    pub scroll_delta: f32,
    /// Horizontal scroll delta (for trackpad/tilt wheel).
    pub horizontal_scroll_delta: f32,
    /// Button states for each mouse button.
    button_states: [ButtonState; 5],
    /// Current drag state.
    pub drag: DragState,
    /// Whether mouse is currently hovering over viewport.
    pub hovering_viewport: bool,
}

impl Default for MouseState {
    fn default() -> Self {
        Self {
            position: Vector2::zeros(),
            previous_position: Vector2::zeros(),
            delta: Vector2::zeros(),
            scroll_delta: 0.0,
            horizontal_scroll_delta: 0.0,
            button_states: [ButtonState::Released; 5],
            drag: DragState::default(),
            hovering_viewport: false,
        }
    }
}

impl MouseState {
    /// Create new mouse state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Update mouse state for new frame.
    pub fn update(&mut self) {
        // Update previous position
        self.previous_position = self.position;

        // Reset one-frame states
        for state in &mut self.button_states {
            *state = match *state {
                ButtonState::Pressed => ButtonState::Down,
                ButtonState::JustReleased => ButtonState::Released,
                other => other,
            };
        }

        // Reset scroll deltas
        self.scroll_delta = 0.0;
        self.horizontal_scroll_delta = 0.0;
    }

    /// Set mouse position.
    pub fn set_position(&mut self, x: f32, y: f32) {
        self.position = Vector2::new(x, y);
        self.delta = self.position - self.previous_position;
    }

    /// Set scroll delta.
    pub fn set_scroll(&mut self, vertical: f32, horizontal: f32) {
        self.scroll_delta = vertical;
        self.horizontal_scroll_delta = horizontal;
    }

    /// Press a mouse button.
    pub fn press_button(&mut self, button: MouseButton) {
        let idx = button as usize;
        if self.button_states[idx] == ButtonState::Released {
            self.button_states[idx] = ButtonState::Pressed;
        }
    }

    /// Release a mouse button.
    pub fn release_button(&mut self, button: MouseButton) {
        let idx = button as usize;
        if self.button_states[idx].is_pressed() {
            self.button_states[idx] = ButtonState::JustReleased;

            // End drag if this button was dragging
            if self.drag.active && self.drag.button == button {
                self.drag.active = false;
            }
        }
    }

    /// Get button state.
    pub fn button(&self, button: MouseButton) -> ButtonState {
        self.button_states[button as usize]
    }

    /// Check if button is currently pressed.
    pub fn is_button_pressed(&self, button: MouseButton) -> bool {
        self.button(button).is_pressed()
    }

    /// Check if button was just pressed this frame.
    pub fn button_just_pressed(&self, button: MouseButton) -> bool {
        self.button(button).just_pressed()
    }

    /// Check if button was just released this frame.
    pub fn button_just_released(&self, button: MouseButton) -> bool {
        self.button(button).just_released()
    }

    /// Start a drag operation.
    pub fn start_drag(&mut self, button: MouseButton) {
        self.drag = DragState {
            active: true,
            button,
            start_pos: self.position,
            delta: Vector2::zeros(),
            total_distance: 0.0,
        };
    }

    /// Update drag state (call after position update).
    pub fn update_drag(&mut self) {
        if self.drag.active {
            self.drag.delta = self.position - self.drag.start_pos;
            self.drag.total_distance = self.drag.delta.norm();
        }
    }

    /// Cast a ray from mouse position into 3D viewport.
    ///
    /// # Arguments
    /// * `camera_pos` - Camera position in world space
    /// * `view_matrix` - View matrix (world to camera space)
    /// * `projection_matrix` - Projection matrix (camera to clip space)
    /// * `viewport_size` - Viewport size in pixels (width, height)
    ///
    /// # Returns
    /// Ray in world space coordinates
    pub fn cast_ray_3d(
        &self,
        camera_pos: Point3<f64>,
        view_matrix: &nalgebra::Matrix4<f64>,
        projection_matrix: &nalgebra::Matrix4<f64>,
        viewport_size: (f32, f32),
    ) -> Ray3D {
        // Convert screen coordinates to normalized device coordinates (NDC)
        let ndc_x = (2.0 * self.position.x / viewport_size.0 - 1.0) as f64;
        let ndc_y = (1.0 - 2.0 * self.position.y / viewport_size.1) as f64; // Y is inverted

        // Ray in clip space
        let ray_clip = nalgebra::Vector4::new(ndc_x, ndc_y, -1.0, 1.0);

        // Ray in camera space (inverse projection)
        let inv_proj = projection_matrix
            .try_inverse()
            .unwrap_or(*projection_matrix);
        let ray_camera = inv_proj * ray_clip;
        let ray_camera = nalgebra::Vector4::new(
            ray_camera.x,
            ray_camera.y,
            -1.0,
            0.0, // direction, not position
        );

        // Ray in world space (inverse view)
        let inv_view = view_matrix.try_inverse().unwrap_or(*view_matrix);
        let ray_world = inv_view * ray_camera;

        let direction = Vector3::new(ray_world.x, ray_world.y, ray_world.z);

        Ray3D::new(camera_pos, direction)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_button_state_transitions() {
        let mut mouse = MouseState::new();

        // Initial state
        assert_eq!(mouse.button(MouseButton::Left), ButtonState::Released);

        // Press button
        mouse.press_button(MouseButton::Left);
        assert_eq!(mouse.button(MouseButton::Left), ButtonState::Pressed);
        assert!(mouse.button_just_pressed(MouseButton::Left));

        // Update frame - should transition to Down
        mouse.update();
        assert_eq!(mouse.button(MouseButton::Left), ButtonState::Down);
        assert!(mouse.is_button_pressed(MouseButton::Left));

        // Release button
        mouse.release_button(MouseButton::Left);
        assert_eq!(mouse.button(MouseButton::Left), ButtonState::JustReleased);
        assert!(mouse.button_just_released(MouseButton::Left));

        // Update frame - should transition to Released
        mouse.update();
        assert_eq!(mouse.button(MouseButton::Left), ButtonState::Released);
    }

    #[test]
    fn test_drag_operation() {
        let mut mouse = MouseState::new();

        mouse.set_position(100.0, 100.0);
        mouse.start_drag(MouseButton::Left);

        assert!(mouse.drag.active);
        assert_eq!(mouse.drag.button, MouseButton::Left);
        assert_eq!(mouse.drag.start_pos, Vector2::new(100.0, 100.0));

        mouse.set_position(150.0, 120.0);
        mouse.update_drag();

        assert_eq!(mouse.drag.delta, Vector2::new(50.0, 20.0));
        assert!((mouse.drag.total_distance - 53.85).abs() < 0.1);
    }

    #[test]
    fn test_position_delta() {
        let mut mouse = MouseState::new();

        mouse.set_position(100.0, 100.0);
        mouse.update();

        mouse.set_position(110.0, 95.0);

        assert_eq!(mouse.delta, Vector2::new(10.0, -5.0));
        assert_eq!(mouse.position, Vector2::new(110.0, 95.0));
        assert_eq!(mouse.previous_position, Vector2::new(100.0, 100.0));
    }
}
