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

//! Move/Translate tool for repositioning objects.

use nalgebra::{Point3, Vector3};

/// Constraint axis for movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintAxis {
    /// No constraint (free movement).
    None,
    /// Constrain to X axis.
    X,
    /// Constrain to Y axis.
    Y,
    /// Constrain to Z axis.
    Z,
    /// Constrain to XY plane.
    XY,
    /// Constrain to XZ plane.
    XZ,
    /// Constrain to YZ plane.
    YZ,
    /// Free 3D movement.
    Free,
}

/// Move/Translate tool.
#[derive(Debug, Clone)]
pub struct MoveTool {
    /// Current constraint axis.
    pub constraint_axis: ConstraintAxis,
    /// Snap increment for grid snapping.
    pub snap_increment: f64,
    /// Enable grid snapping.
    pub use_grid_snap: bool,
    /// Drag start position (world space).
    drag_start: Option<Point3<f64>>,
    /// Initial object position.
    initial_position: Option<Vector3<f64>>,
    /// Is currently dragging.
    pub dragging: bool,
}

impl MoveTool {
    /// Create a new move tool.
    pub fn new() -> Self {
        Self {
            constraint_axis: ConstraintAxis::Free,
            snap_increment: 0.1,
            use_grid_snap: false,
            drag_start: None,
            initial_position: None,
            dragging: false,
        }
    }

    /// Activate tool.
    pub fn activate(&mut self) {
        tracing::debug!("Move tool activated");
        self.constraint_axis = ConstraintAxis::Free;
    }

    /// Deactivate tool.
    pub fn deactivate(&mut self) {
        self.dragging = false;
        self.drag_start = None;
        self.initial_position = None;
        tracing::debug!("Move tool deactivated");
    }

    /// Begin drag operation.
    pub fn begin_drag(&mut self, start_pos: Point3<f64>, object_pos: Vector3<f64>) {
        self.dragging = true;
        self.drag_start = Some(start_pos);
        self.initial_position = Some(object_pos);
    }

    /// Handle drag movement.
    pub fn handle_drag(&mut self, current_pos: Point3<f64>) -> Option<Vector3<f64>> {
        if !self.dragging {
            return None;
        }

        let start = self.drag_start?;
        let initial = self.initial_position?;

        let delta = self.get_delta(start, current_pos);
        let new_pos = initial + delta;

        Some(new_pos)
    }

    /// End drag operation.
    pub fn end_drag(&mut self) {
        self.dragging = false;
        self.drag_start = None;
        self.initial_position = None;
    }

    /// Get movement delta based on constraint.
    pub fn get_delta(&self, start: Point3<f64>, current: Point3<f64>) -> Vector3<f64> {
        let mut delta = current - start;

        // Apply axis constraint
        delta = match self.constraint_axis {
            ConstraintAxis::None | ConstraintAxis::Free => delta,
            ConstraintAxis::X => Vector3::new(delta.x, 0.0, 0.0),
            ConstraintAxis::Y => Vector3::new(0.0, delta.y, 0.0),
            ConstraintAxis::Z => Vector3::new(0.0, 0.0, delta.z),
            ConstraintAxis::XY => Vector3::new(delta.x, delta.y, 0.0),
            ConstraintAxis::XZ => Vector3::new(delta.x, 0.0, delta.z),
            ConstraintAxis::YZ => Vector3::new(0.0, delta.y, delta.z),
        };

        // Apply snapping
        if self.use_grid_snap {
            delta = self.snap_vector(delta);
        }

        delta
    }

    /// Snap vector to grid.
    fn snap_vector(&self, v: Vector3<f64>) -> Vector3<f64> {
        let snap = |x: f64| (x / self.snap_increment).round() * self.snap_increment;
        Vector3::new(snap(v.x), snap(v.y), snap(v.z))
    }

    /// Apply transform to position.
    pub fn apply_transform(&self, position: Vector3<f64>, delta: Vector3<f64>) -> Vector3<f64> {
        position + delta
    }

    /// Set constraint axis from keyboard input.
    pub fn set_constraint_from_key(&mut self, key: char) {
        self.constraint_axis = match key {
            'x' | 'X' => ConstraintAxis::X,
            'y' | 'Y' => ConstraintAxis::Y,
            'z' | 'Z' => ConstraintAxis::Z,
            _ => ConstraintAxis::Free,
        };
    }

    /// Toggle grid snapping.
    pub fn toggle_snap(&mut self) {
        self.use_grid_snap = !self.use_grid_snap;
    }
}

impl Default for MoveTool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_move_tool_creation() {
        let tool = MoveTool::new();
        assert_eq!(tool.constraint_axis, ConstraintAxis::Free);
        assert!(!tool.dragging);
    }

    #[test]
    fn test_constraint_axis() {
        let tool = MoveTool::new();
        let start = Point3::new(0.0, 0.0, 0.0);
        let current = Point3::new(1.0, 2.0, 3.0);

        let mut tool_x = tool.clone();
        tool_x.constraint_axis = ConstraintAxis::X;
        let delta_x = tool_x.get_delta(start, current);
        assert_eq!(delta_x, Vector3::new(1.0, 0.0, 0.0));

        let mut tool_y = tool.clone();
        tool_y.constraint_axis = ConstraintAxis::Y;
        let delta_y = tool_y.get_delta(start, current);
        assert_eq!(delta_y, Vector3::new(0.0, 2.0, 0.0));
    }

    #[test]
    fn test_snapping() {
        let mut tool = MoveTool::new();
        tool.use_grid_snap = true;
        tool.snap_increment = 0.5;

        let v = Vector3::new(1.3, 2.7, 3.1);
        let snapped = tool.snap_vector(v);
        assert_eq!(snapped, Vector3::new(1.5, 2.5, 3.0));
    }
}
