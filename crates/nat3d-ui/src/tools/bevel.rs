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

//! Bevel tool for edge/vertex beveling.

use nalgebra::Point3;

/// Bevel profile shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BevelProfile {
    /// Linear profile.
    Linear,
    /// Convex profile (arc out).
    Convex,
    /// Concave profile (arc in).
    Concave,
    /// Custom profile.
    Custom,
}

/// Bevel tool.
#[derive(Debug, Clone)]
pub struct BevelTool {
    /// Bevel width.
    pub width: f64,
    /// Number of segments.
    pub segments: usize,
    /// Bevel profile shape.
    pub profile: BevelProfile,
    /// Profile factor (0-1).
    pub profile_factor: f64,
    /// Is currently active.
    pub active: bool,
    /// Drag start position.
    drag_start: Option<Point3<f64>>,
    /// Initial width.
    initial_width: f64,
}

impl BevelTool {
    /// Create a new bevel tool.
    pub fn new() -> Self {
        Self {
            width: 0.1,
            segments: 1,
            profile: BevelProfile::Linear,
            profile_factor: 0.5,
            active: false,
            drag_start: None,
            initial_width: 0.1,
        }
    }

    /// Activate tool.
    pub fn activate(&mut self) {
        tracing::debug!("Bevel tool activated");
        self.active = true;
    }

    /// Deactivate tool.
    pub fn deactivate(&mut self) {
        self.active = false;
        self.drag_start = None;
        tracing::debug!("Bevel tool deactivated");
    }

    /// Begin drag operation.
    pub fn begin_drag(&mut self, start_pos: Point3<f64>) {
        self.drag_start = Some(start_pos);
        self.initial_width = self.width;
    }

    /// Handle drag to adjust bevel width.
    pub fn handle_drag(&mut self, current_pos: Point3<f64>) {
        if let Some(start) = self.drag_start {
            let delta = (current_pos - start).magnitude();
            self.width = (self.initial_width + delta).max(0.0);
        }
    }

    /// Apply bevel to edges.
    pub fn apply_bevel(&self, edges: &[u64]) -> BevelResult {
        BevelResult {
            edge_ids: edges.to_vec(),
            width: self.width,
            segments: self.segments,
            profile: self.profile,
            profile_factor: self.profile_factor,
        }
    }

    /// Increase segments.
    pub fn increase_segments(&mut self) {
        if self.segments < 100 {
            self.segments += 1;
        }
    }

    /// Decrease segments.
    pub fn decrease_segments(&mut self) {
        if self.segments > 1 {
            self.segments -= 1;
        }
    }

    /// Set profile.
    pub fn set_profile(&mut self, profile: BevelProfile) {
        self.profile = profile;
    }

    /// End drag operation.
    pub fn end_drag(&mut self) {
        self.drag_start = None;
    }

    /// Cancel bevel.
    pub fn cancel(&mut self) {
        self.width = self.initial_width;
        self.drag_start = None;
    }
}

/// Result of bevel operation.
#[derive(Debug, Clone)]
pub struct BevelResult {
    /// Edge IDs that were beveled.
    pub edge_ids: Vec<u64>,
    /// Bevel width.
    pub width: f64,
    /// Number of segments.
    pub segments: usize,
    /// Profile shape.
    pub profile: BevelProfile,
    /// Profile factor.
    pub profile_factor: f64,
}

impl Default for BevelTool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bevel_tool_creation() {
        let tool = BevelTool::new();
        assert_eq!(tool.segments, 1);
        assert!(!tool.active);
    }

    #[test]
    fn test_segments() {
        let mut tool = BevelTool::new();
        tool.increase_segments();
        assert_eq!(tool.segments, 2);
        tool.decrease_segments();
        assert_eq!(tool.segments, 1);
        tool.decrease_segments();
        assert_eq!(tool.segments, 1); // Should not go below 1
    }

    #[test]
    fn test_apply_bevel() {
        let tool = BevelTool::new();
        let edges = vec![1, 2, 3];
        let result = tool.apply_bevel(&edges);
        assert_eq!(result.edge_ids.len(), 3);
        assert_eq!(result.width, tool.width);
    }
}
