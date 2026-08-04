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

//! Rotate tool for rotating objects around axes.

use nalgebra::{UnitQuaternion, Vector3};

/// Rotation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotationMode {
    /// Free trackball rotation.
    Trackball,
    /// Free rotation.
    Free,
    /// Constrained to specific axis.
    AxisConstrained,
}

/// Constraint axis for rotation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotationAxis {
    /// No constraint.
    None,
    /// Rotate around X axis.
    X,
    /// Rotate around Y axis.
    Y,
    /// Rotate around Z axis.
    Z,
}

/// Rotate tool.
#[derive(Debug, Clone)]
pub struct RotateTool {
    /// Current rotation mode.
    pub rotation_mode: RotationMode,
    /// Constraint axis.
    pub constraint_axis: RotationAxis,
    /// Snap angle in degrees.
    pub snap_angle: f64,
    /// Enable angle snapping.
    pub use_angle_snap: bool,
    /// Drag start position (screen space for trackball).
    drag_start: Option<(f64, f64)>,
    /// Initial rotation.
    initial_rotation: Option<UnitQuaternion<f64>>,
    /// Is currently dragging.
    pub dragging: bool,
    /// Accumulated rotation angle.
    accumulated_angle: f64,
}

impl RotateTool {
    /// Create a new rotate tool.
    pub fn new() -> Self {
        Self {
            rotation_mode: RotationMode::Trackball,
            constraint_axis: RotationAxis::None,
            snap_angle: 15.0,
            use_angle_snap: false,
            drag_start: None,
            initial_rotation: None,
            dragging: false,
            accumulated_angle: 0.0,
        }
    }

    /// Activate tool.
    pub fn activate(&mut self) {
        tracing::debug!("Rotate tool activated");
    }

    /// Deactivate tool.
    pub fn deactivate(&mut self) {
        self.dragging = false;
        self.drag_start = None;
        self.initial_rotation = None;
        tracing::debug!("Rotate tool deactivated");
    }

    /// Begin drag operation.
    pub fn begin_drag(&mut self, screen_pos: (f64, f64), rotation: UnitQuaternion<f64>) {
        self.dragging = true;
        self.drag_start = Some(screen_pos);
        self.initial_rotation = Some(rotation);
        self.accumulated_angle = 0.0;
    }

    /// Handle drag for rotation.
    pub fn handle_drag(
        &mut self,
        current_pos: (f64, f64),
        viewport_size: (f64, f64),
    ) -> Option<UnitQuaternion<f64>> {
        if !self.dragging {
            return None;
        }

        let start = self.drag_start?;
        let initial = self.initial_rotation?;

        let rotation = match self.rotation_mode {
            RotationMode::Trackball => self.trackball_rotation(start, current_pos, viewport_size),
            RotationMode::Free => self.free_rotation(start, current_pos),
            RotationMode::AxisConstrained => self.axis_rotation(start, current_pos),
        };

        Some(rotation * initial)
    }

    /// Trackball rotation calculation.
    fn trackball_rotation(
        &self,
        start: (f64, f64),
        current: (f64, f64),
        viewport_size: (f64, f64),
    ) -> UnitQuaternion<f64> {
        let (vw, vh) = viewport_size;
        let (sx, sy) = start;
        let (cx, cy) = current;

        // Normalize to [-1, 1]
        let sx_norm = (sx / vw) * 2.0 - 1.0;
        let sy_norm = 1.0 - (sy / vh) * 2.0;
        let cx_norm = (cx / vw) * 2.0 - 1.0;
        let cy_norm = 1.0 - (cy / vh) * 2.0;

        let p1 = self.project_to_sphere(sx_norm, sy_norm);
        let p2 = self.project_to_sphere(cx_norm, cy_norm);

        let axis = p1.cross(&p2);
        if axis.magnitude() < 1e-6 {
            return UnitQuaternion::identity();
        }

        let angle = p1.dot(&p2).clamp(-1.0, 1.0).acos();
        UnitQuaternion::from_axis_angle(&nalgebra::Unit::new_normalize(axis), angle)
    }

    /// Project screen point to sphere for trackball.
    fn project_to_sphere(&self, x: f64, y: f64) -> Vector3<f64> {
        let r = 1.0;
        let d = (x * x + y * y).sqrt();

        if d < r * std::f64::consts::FRAC_1_SQRT_2 {
            // Inside sphere
            Vector3::new(x, y, (r * r - d * d).sqrt())
        } else {
            // Outside sphere, project to edge
            let t = r / std::f64::consts::SQRT_2;
            Vector3::new(x, y, t * t / d)
        }
    }

    /// Free rotation.
    fn free_rotation(&self, start: (f64, f64), current: (f64, f64)) -> UnitQuaternion<f64> {
        let dx = current.0 - start.0;
        let dy = current.1 - start.1;

        let angle_x = dy * 0.01;
        let angle_y = dx * 0.01;

        let rot_x = UnitQuaternion::from_axis_angle(&Vector3::x_axis(), angle_x);
        let rot_y = UnitQuaternion::from_axis_angle(&Vector3::y_axis(), angle_y);

        rot_y * rot_x
    }

    /// Axis-constrained rotation.
    fn axis_rotation(&mut self, start: (f64, f64), current: (f64, f64)) -> UnitQuaternion<f64> {
        let dx = current.0 - start.0;
        let dy = current.1 - start.1;

        let mut angle = ((dx * dx + dy * dy).sqrt()) * 0.01;
        if dx < 0.0 {
            angle = -angle;
        }

        self.accumulated_angle += angle;

        if self.use_angle_snap {
            let snap_rad = self.snap_angle.to_radians();
            angle = (self.accumulated_angle / snap_rad).round() * snap_rad;
        }

        let axis = match self.constraint_axis {
            RotationAxis::X => Vector3::x_axis(),
            RotationAxis::Y => Vector3::y_axis(),
            RotationAxis::Z => Vector3::z_axis(),
            RotationAxis::None => Vector3::y_axis(),
        };

        UnitQuaternion::from_axis_angle(&axis, angle)
    }

    /// Apply rotation.
    pub fn apply_rotation(
        &self,
        rotation: UnitQuaternion<f64>,
        delta: UnitQuaternion<f64>,
    ) -> UnitQuaternion<f64> {
        delta * rotation
    }

    /// Set constraint axis from keyboard.
    pub fn set_constraint_from_key(&mut self, key: char) {
        self.constraint_axis = match key {
            'x' | 'X' => RotationAxis::X,
            'y' | 'Y' => RotationAxis::Y,
            'z' | 'Z' => RotationAxis::Z,
            _ => RotationAxis::None,
        };
        if self.constraint_axis != RotationAxis::None {
            self.rotation_mode = RotationMode::AxisConstrained;
        }
    }

    /// End drag operation.
    pub fn end_drag(&mut self) {
        self.dragging = false;
        self.drag_start = None;
        self.initial_rotation = None;
        self.accumulated_angle = 0.0;
    }
}

impl Default for RotateTool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rotate_tool_creation() {
        let tool = RotateTool::new();
        assert_eq!(tool.rotation_mode, RotationMode::Trackball);
        assert!(!tool.dragging);
    }

    #[test]
    fn test_project_to_sphere() {
        let tool = RotateTool::new();
        let p = tool.project_to_sphere(0.0, 0.0);
        assert!((p.magnitude() - 1.0).abs() < 1e-6);
    }
}
