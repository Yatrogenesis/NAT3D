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

//! Extrude tool for face/edge extrusion.

use nalgebra::Point3;

/// Extrude mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtrudeMode {
    /// Extrude entire regions together.
    Region,
    /// Extrude individual faces separately.
    Individual,
    /// Extrude along normals.
    AlongNormals,
}

/// Extrude tool.
#[derive(Debug, Clone)]
pub struct ExtrudeTool {
    /// Current extrude mode.
    pub extrude_mode: ExtrudeMode,
    /// Extrusion offset distance.
    pub offset: f64,
    /// Is currently extruding.
    pub active: bool,
    /// Drag start position.
    drag_start: Option<Point3<f64>>,
    /// Initial offset value.
    initial_offset: f64,
}

impl ExtrudeTool {
    /// Create a new extrude tool.
    pub fn new() -> Self {
        Self {
            extrude_mode: ExtrudeMode::Region,
            offset: 0.0,
            active: false,
            drag_start: None,
            initial_offset: 0.0,
        }
    }

    /// Activate tool.
    pub fn activate(&mut self) {
        tracing::debug!("Extrude tool activated");
        self.active = true;
        self.offset = 0.0;
    }

    /// Deactivate tool.
    pub fn deactivate(&mut self) {
        self.active = false;
        self.drag_start = None;
        tracing::debug!("Extrude tool deactivated");
    }

    /// Begin drag operation.
    pub fn begin_drag(&mut self, start_pos: Point3<f64>) {
        self.drag_start = Some(start_pos);
        self.initial_offset = self.offset;
    }

    /// Handle drag to adjust extrusion.
    pub fn handle_drag(&mut self, current_pos: Point3<f64>) {
        if let Some(start) = self.drag_start {
            let delta = (current_pos - start).magnitude();
            self.offset = self.initial_offset + delta;
        }
    }

    /// Apply extrusion.
    pub fn apply_extrude(&mut self, faces: &[u64]) -> Vec<ExtrudeResult> {
        let mut results = Vec::new();

        match self.extrude_mode {
            ExtrudeMode::Region => {
                // Extrude all faces together as one region
                results.push(ExtrudeResult {
                    face_ids: faces.to_vec(),
                    offset: self.offset,
                    mode: ExtrudeMode::Region,
                });
            }
            ExtrudeMode::Individual => {
                // Extrude each face individually
                for &face_id in faces {
                    results.push(ExtrudeResult {
                        face_ids: vec![face_id],
                        offset: self.offset,
                        mode: ExtrudeMode::Individual,
                    });
                }
            }
            ExtrudeMode::AlongNormals => {
                // Extrude along face normals
                for &face_id in faces {
                    results.push(ExtrudeResult {
                        face_ids: vec![face_id],
                        offset: self.offset,
                        mode: ExtrudeMode::AlongNormals,
                    });
                }
            }
        }

        results
    }

    /// Set extrude mode.
    pub fn set_mode(&mut self, mode: ExtrudeMode) {
        self.extrude_mode = mode;
    }

    /// End drag operation.
    pub fn end_drag(&mut self) {
        self.drag_start = None;
    }

    /// Cancel extrusion.
    pub fn cancel(&mut self) {
        self.offset = 0.0;
        self.drag_start = None;
    }
}

/// Result of extrusion operation.
#[derive(Debug, Clone)]
pub struct ExtrudeResult {
    /// Face IDs that were extruded.
    pub face_ids: Vec<u64>,
    /// Extrusion offset.
    pub offset: f64,
    /// Extrusion mode used.
    pub mode: ExtrudeMode,
}

impl Default for ExtrudeTool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extrude_tool_creation() {
        let tool = ExtrudeTool::new();
        assert_eq!(tool.extrude_mode, ExtrudeMode::Region);
        assert!(!tool.active);
    }

    #[test]
    fn test_region_extrude() {
        let mut tool = ExtrudeTool::new();
        tool.activate();
        tool.offset = 1.0;

        let faces = vec![1, 2, 3];
        let results = tool.apply_extrude(&faces);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].face_ids.len(), 3);
    }

    #[test]
    fn test_individual_extrude() {
        let mut tool = ExtrudeTool::new();
        tool.extrude_mode = ExtrudeMode::Individual;
        tool.activate();
        tool.offset = 1.0;

        let faces = vec![1, 2, 3];
        let results = tool.apply_extrude(&faces);

        assert_eq!(results.len(), 3);
    }
}
