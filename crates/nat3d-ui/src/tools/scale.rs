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

//! Scale tool for resizing objects.

use nalgebra::{Point3, Vector3};

/// Constraint axis for scaling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaleAxis {
    /// No constraint.
    None,
    /// Scale along X axis.
    X,
    /// Scale along Y axis.
    Y,
    /// Scale along Z axis.
    Z,
    /// Uniform scaling.
    Uniform,
}

/// Scale tool.
#[derive(Debug, Clone)]
pub struct ScaleTool {
    /// Constraint axis.
    pub constraint_axis: ScaleAxis,
    /// Uniform scaling flag.
    pub uniform: bool,
    /// Snap increment for scale snapping.
    pub snap_increment: f64,
    /// Enable scale snapping.
    pub use_snap: bool,
    /// Center point for scaling.
    pub center_point: Point3<f64>,
    /// Drag start distance from center.
    drag_start_distance: Option<f64>,
    /// Initial scale.
    initial_scale: Option<Vector3<f64>>,
    /// Is currently dragging.
    pub dragging: bool,
}

impl ScaleTool {
    /// Create a new scale tool.
    pub fn new() -> Self {
        Self {
            constraint_axis: ScaleAxis::Uniform,
            uniform: true,
            snap_increment: 0.1,
            use_snap: false,
            center_point: Point3::origin(),
            drag_start_distance: None,
            initial_scale: None,
            dragging: false,
        }
    }

    /// Activate tool.
    pub fn activate(&mut self) {
        tracing::debug!("Scale tool activated");
        self.uniform = true;
    }

    /// Deactivate tool.
    pub fn deactivate(&mut self) {
        self.dragging = false;
        self.drag_start_distance = None;
        self.initial_scale = None;
        tracing::debug!("Scale tool deactivated");
    }

    /// Begin drag operation.
    pub fn begin_drag(
        &mut self,
        pivot: Point3<f64>,
        mouse_pos: Point3<f64>,
        initial_scale: Vector3<f64>,
    ) {
        self.dragging = true;
        self.center_point = pivot;
        self.drag_start_distance = Some((mouse_pos - pivot).magnitude());
        self.initial_scale = Some(initial_scale);
    }

    /// Handle drag for scaling.
    pub fn handle_drag(&mut self, current_pos: Point3<f64>) -> Option<Vector3<f64>> {
        if !self.dragging {
            return None;
        }

        let start_dist = self.drag_start_distance?;
        let initial = self.initial_scale?;

        let current_dist = (current_pos - self.center_point).magnitude();

        if start_dist < 1e-6 {
            return Some(initial);
        }

        let scale_factor = current_dist / start_dist;
        let scale = self.calculate_scale(scale_factor, initial);

        Some(scale)
    }

    /// Calculate scale vector based on constraint.
    fn calculate_scale(&self, factor: f64, initial: Vector3<f64>) -> Vector3<f64> {
        let mut factor = factor;

        // Apply snapping
        if self.use_snap {
            factor = (factor / self.snap_increment).round() * self.snap_increment;
        }

        // Apply constraint
        let scale = if self.uniform {
            Vector3::new(factor, factor, factor)
        } else {
            match self.constraint_axis {
                ScaleAxis::None | ScaleAxis::Uniform => Vector3::new(factor, factor, factor),
                ScaleAxis::X => Vector3::new(factor, 1.0, 1.0),
                ScaleAxis::Y => Vector3::new(1.0, factor, 1.0),
                ScaleAxis::Z => Vector3::new(1.0, 1.0, factor),
            }
        };

        Vector3::new(
            initial.x * scale.x,
            initial.y * scale.y,
            initial.z * scale.z,
        )
    }

    /// Apply scale to current scale value.
    pub fn apply_scale(&self, current: Vector3<f64>, delta: Vector3<f64>) -> Vector3<f64> {
        Vector3::new(
            current.x * delta.x,
            current.y * delta.y,
            current.z * delta.z,
        )
    }

    /// Set constraint axis from keyboard.
    pub fn set_constraint_from_key(&mut self, key: char) {
        self.constraint_axis = match key {
            'x' | 'X' => ScaleAxis::X,
            'y' | 'Y' => ScaleAxis::Y,
            'z' | 'Z' => ScaleAxis::Z,
            _ => ScaleAxis::Uniform,
        };
        self.uniform = self.constraint_axis == ScaleAxis::Uniform;
    }

    /// Toggle uniform scaling.
    pub fn toggle_uniform(&mut self) {
        self.uniform = !self.uniform;
    }

    /// End drag operation.
    pub fn end_drag(&mut self) {
        self.dragging = false;
        self.drag_start_distance = None;
        self.initial_scale = None;
    }
}

impl Default for ScaleTool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scale_tool_creation() {
        let tool = ScaleTool::new();
        assert!(tool.uniform);
        assert!(!tool.dragging);
    }

    #[test]
    fn test_uniform_scale() {
        let tool = ScaleTool::new();
        let initial = Vector3::new(1.0, 1.0, 1.0);
        let scale = tool.calculate_scale(2.0, initial);
        assert_eq!(scale, Vector3::new(2.0, 2.0, 2.0));
    }

    #[test]
    fn test_axis_scale() {
        let mut tool = ScaleTool::new();
        tool.uniform = false;
        tool.constraint_axis = ScaleAxis::X;
        let initial = Vector3::new(1.0, 1.0, 1.0);
        let scale = tool.calculate_scale(2.0, initial);
        assert_eq!(scale, Vector3::new(2.0, 1.0, 1.0));
    }
}
