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

//! Knife tool for creating custom cuts in meshes.

use nalgebra::{Point3, Vector3};

/// Knife tool.
#[derive(Debug, Clone)]
pub struct KnifeTool {
    /// Points along the cut path.
    pub cut_points: Vec<Point3<f64>>,
    /// Snap to existing vertices.
    pub snap_to_vertex: bool,
    /// Cut through entire mesh.
    pub through_cut: bool,
    /// Angle snap in degrees.
    pub angle_snap: Option<f64>,
    /// Is currently active.
    pub active: bool,
    /// Cut has been started.
    pub cutting: bool,
}

impl KnifeTool {
    /// Create a new knife tool.
    pub fn new() -> Self {
        Self {
            cut_points: Vec::new(),
            snap_to_vertex: false,
            through_cut: false,
            angle_snap: Some(45.0),
            active: false,
            cutting: false,
        }
    }

    /// Activate tool.
    pub fn activate(&mut self) {
        tracing::debug!("Knife tool activated");
        self.active = true;
        self.cut_points.clear();
        self.cutting = false;
    }

    /// Deactivate tool.
    pub fn deactivate(&mut self) {
        self.active = false;
        self.cut_points.clear();
        self.cutting = false;
        tracing::debug!("Knife tool deactivated");
    }

    /// Handle click to add cut point.
    pub fn handle_click(&mut self, point: Point3<f64>) {
        if !self.cutting {
            self.cutting = true;
        }

        // Apply angle snapping if enabled
        let snapped_point = if let Some(snap_angle) = self.angle_snap {
            if let Some(last_point) = self.cut_points.last() {
                self.snap_to_angle(*last_point, point, snap_angle)
            } else {
                point
            }
        } else {
            point
        };

        self.cut_points.push(snapped_point);
    }

    /// Snap point to angle relative to last point.
    fn snap_to_angle(
        &self,
        last: Point3<f64>,
        current: Point3<f64>,
        angle_deg: f64,
    ) -> Point3<f64> {
        let delta = current - last;
        let distance = delta.magnitude();

        if distance < 1e-6 {
            return current;
        }

        let angle_rad = angle_deg.to_radians();
        let current_angle = delta.y.atan2(delta.x);

        // Snap to nearest angle increment
        let snapped_angle = (current_angle / angle_rad).round() * angle_rad;

        let snapped_delta = Vector3::new(
            snapped_angle.cos() * distance,
            snapped_angle.sin() * distance,
            delta.z,
        );

        last + snapped_delta
    }

    /// Handle double-click to finish and apply cut.
    pub fn handle_double_click(&mut self) -> Option<KnifeResult> {
        if self.cut_points.len() < 2 {
            return None;
        }

        let result = Some(KnifeResult {
            points: self.cut_points.clone(),
            snap_to_vertex: self.snap_to_vertex,
            through_cut: self.through_cut,
        });

        self.cut_points.clear();
        self.cutting = false;

        result
    }

    /// Get preview of current cut line.
    pub fn get_preview_line(&self) -> Option<(&[Point3<f64>], bool)> {
        if self.cut_points.is_empty() {
            None
        } else {
            Some((&self.cut_points, self.cutting))
        }
    }

    /// Toggle vertex snapping.
    pub fn toggle_vertex_snap(&mut self) {
        self.snap_to_vertex = !self.snap_to_vertex;
    }

    /// Toggle through-cut mode.
    pub fn toggle_through_cut(&mut self) {
        self.through_cut = !self.through_cut;
    }

    /// Toggle angle snapping.
    pub fn toggle_angle_snap(&mut self) {
        self.angle_snap = if self.angle_snap.is_some() {
            None
        } else {
            Some(45.0)
        };
    }

    /// Cancel current cut.
    pub fn cancel(&mut self) {
        self.cut_points.clear();
        self.cutting = false;
    }
}

/// Result of knife cut operation.
#[derive(Debug, Clone)]
pub struct KnifeResult {
    /// Points along the cut path.
    pub points: Vec<Point3<f64>>,
    /// Snap to vertices flag.
    pub snap_to_vertex: bool,
    /// Through-cut flag.
    pub through_cut: bool,
}

impl Default for KnifeTool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_knife_tool_creation() {
        let tool = KnifeTool::new();
        assert!(tool.cut_points.is_empty());
        assert!(!tool.active);
    }

    #[test]
    fn test_add_cut_points() {
        let mut tool = KnifeTool::new();
        tool.activate();

        tool.handle_click(Point3::new(0.0, 0.0, 0.0));
        assert_eq!(tool.cut_points.len(), 1);

        tool.handle_click(Point3::new(1.0, 1.0, 0.0));
        assert_eq!(tool.cut_points.len(), 2);
    }

    #[test]
    fn test_cancel() {
        let mut tool = KnifeTool::new();
        tool.activate();
        tool.handle_click(Point3::new(0.0, 0.0, 0.0));
        tool.cancel();
        assert!(tool.cut_points.is_empty());
        assert!(!tool.cutting);
    }
}
