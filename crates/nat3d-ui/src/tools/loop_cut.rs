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

//! Loop cut tool for creating edge loops on meshes.

/// Loop cut tool.
#[derive(Debug, Clone)]
pub struct LoopCutTool {
    /// Number of cuts to make.
    pub cuts: usize,
    /// Use even spacing between cuts.
    pub even_spacing: bool,
    /// Edge currently under mouse cursor.
    pub edge_under_mouse: Option<u64>,
    /// Preview cut position (0-1 along edge loop).
    pub cut_position: f64,
    /// Is currently active.
    pub active: bool,
    /// Cut has been confirmed.
    pub confirmed: bool,
}

impl LoopCutTool {
    /// Create a new loop cut tool.
    pub fn new() -> Self {
        Self {
            cuts: 1,
            even_spacing: true,
            edge_under_mouse: None,
            cut_position: 0.5,
            active: false,
            confirmed: false,
        }
    }

    /// Activate tool.
    pub fn activate(&mut self) {
        tracing::debug!("Loop cut tool activated");
        self.active = true;
        self.confirmed = false;
        self.cuts = 1;
    }

    /// Deactivate tool.
    pub fn deactivate(&mut self) {
        self.active = false;
        self.edge_under_mouse = None;
        self.confirmed = false;
        tracing::debug!("Loop cut tool deactivated");
    }

    /// Handle mouse hover to preview cut location.
    pub fn handle_hover(&mut self, edge_id: Option<u64>, position: f64) {
        self.edge_under_mouse = edge_id;
        self.cut_position = position.clamp(0.0, 1.0);
    }

    /// Handle click to confirm cut.
    pub fn handle_click(&mut self) -> Option<LoopCutResult> {
        if !self.confirmed {
            // First click: confirm the cut location
            self.confirmed = true;
            None
        } else {
            // Second click: apply the cut
            if let Some(edge_id) = self.edge_under_mouse {
                Some(LoopCutResult {
                    edge_id,
                    cuts: self.cuts,
                    positions: self.calculate_cut_positions(),
                    even_spacing: self.even_spacing,
                })
            } else {
                None
            }
        }
    }

    /// Calculate positions for multiple cuts.
    fn calculate_cut_positions(&self) -> Vec<f64> {
        if self.cuts == 0 {
            return Vec::new();
        }

        if self.cuts == 1 {
            return vec![self.cut_position];
        }

        if self.even_spacing {
            // Evenly spaced cuts
            let mut positions = Vec::new();
            for i in 0..self.cuts {
                let t = (i + 1) as f64 / (self.cuts + 1) as f64;
                positions.push(t);
            }
            positions
        } else {
            // Cuts centered around selected position
            let mut positions = Vec::new();
            let spacing = 1.0 / (self.cuts + 1) as f64;
            for i in 0..self.cuts {
                let offset = (i as f64 - (self.cuts - 1) as f64 / 2.0) * spacing;
                let pos = (self.cut_position + offset).clamp(0.0, 1.0);
                positions.push(pos);
            }
            positions
        }
    }

    /// Increase number of cuts.
    pub fn increase_cuts(&mut self) {
        if self.cuts < 100 {
            self.cuts += 1;
        }
    }

    /// Decrease number of cuts.
    pub fn decrease_cuts(&mut self) {
        if self.cuts > 1 {
            self.cuts -= 1;
        }
    }

    /// Toggle even spacing.
    pub fn toggle_spacing(&mut self) {
        self.even_spacing = !self.even_spacing;
    }

    /// Cancel operation.
    pub fn cancel(&mut self) {
        self.confirmed = false;
        self.edge_under_mouse = None;
    }
}

/// Result of loop cut operation.
#[derive(Debug, Clone)]
pub struct LoopCutResult {
    /// Edge ID where cut was initiated.
    pub edge_id: u64,
    /// Number of cuts.
    pub cuts: usize,
    /// Positions for each cut (0-1).
    pub positions: Vec<f64>,
    /// Even spacing flag.
    pub even_spacing: bool,
}

impl Default for LoopCutTool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loop_cut_creation() {
        let tool = LoopCutTool::new();
        assert_eq!(tool.cuts, 1);
        assert!(tool.even_spacing);
    }

    #[test]
    fn test_cut_positions_single() {
        let tool = LoopCutTool::new();
        let positions = tool.calculate_cut_positions();
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0], 0.5);
    }

    #[test]
    fn test_cut_positions_multiple() {
        let mut tool = LoopCutTool::new();
        tool.cuts = 3;
        let positions = tool.calculate_cut_positions();
        assert_eq!(positions.len(), 3);
        assert_eq!(positions[0], 0.25);
        assert_eq!(positions[1], 0.5);
        assert_eq!(positions[2], 0.75);
    }

    #[test]
    fn test_increase_decrease_cuts() {
        let mut tool = LoopCutTool::new();
        tool.increase_cuts();
        assert_eq!(tool.cuts, 2);
        tool.decrease_cuts();
        assert_eq!(tool.cuts, 1);
        tool.decrease_cuts();
        assert_eq!(tool.cuts, 1); // Should not go below 1
    }
}
