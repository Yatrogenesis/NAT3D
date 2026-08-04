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

//! Active tool settings panel (context-sensitive).

/// Active tool type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTool {
    Select,
    Move,
    Rotate,
    Scale,
    Extrude,
    Bevel,
    LoopCut,
    Knife,
    Sculpt,
    None,
}

/// Tools panel showing settings for the currently active tool.
#[derive(Debug, Clone)]
pub struct ToolsPanel {
    /// Currently active tool.
    pub active_tool: ActiveTool,
}

impl ToolsPanel {
    /// Create a new tools panel.
    pub fn new() -> Self {
        Self {
            active_tool: ActiveTool::Select,
        }
    }

    /// Show the tools panel.
    pub fn show(&mut self, ui: &mut egui::Ui, active_tool: ActiveTool) {
        self.active_tool = active_tool;

        ui.heading("Tool Settings");
        ui.separator();

        match self.active_tool {
            ActiveTool::Select => self.show_select_settings(ui),
            ActiveTool::Move => self.show_move_settings(ui),
            ActiveTool::Rotate => self.show_rotate_settings(ui),
            ActiveTool::Scale => self.show_scale_settings(ui),
            ActiveTool::Extrude => self.show_extrude_settings(ui),
            ActiveTool::Bevel => self.show_bevel_settings(ui),
            ActiveTool::LoopCut => self.show_loop_cut_settings(ui),
            ActiveTool::Knife => self.show_knife_settings(ui),
            ActiveTool::Sculpt => self.show_sculpt_settings(ui),
            ActiveTool::None => {
                ui.label("No tool selected");
            }
        }
    }

    fn show_select_settings(&mut self, ui: &mut egui::Ui) {
        ui.label("Selection Mode:");
        ui.radio_value(&mut 0, 0, "Single");
        ui.radio_value(&mut 0, 1, "Box");
        ui.radio_value(&mut 0, 2, "Lasso");
        ui.radio_value(&mut 0, 3, "Circle");
        ui.radio_value(&mut 0, 4, "Paint");

        ui.separator();
        ui.label("Modifiers:");
        ui.label("• Shift: Add to selection");
        ui.label("• Ctrl: Subtract from selection");
        ui.label("• Alt: Toggle selection");
    }

    fn show_move_settings(&mut self, ui: &mut egui::Ui) {
        ui.label("Constraint Axis:");
        ui.radio_value(&mut 0, 0, "Free");
        ui.radio_value(&mut 0, 1, "X");
        ui.radio_value(&mut 0, 2, "Y");
        ui.radio_value(&mut 0, 3, "Z");

        ui.separator();
        ui.checkbox(&mut false, "Grid Snap");
        ui.add(egui::Slider::new(&mut 0.1, 0.01..=1.0).text("Snap Increment"));
    }

    fn show_rotate_settings(&mut self, ui: &mut egui::Ui) {
        ui.label("Rotation Mode:");
        ui.radio_value(&mut 0, 0, "Trackball");
        ui.radio_value(&mut 0, 1, "Free");
        ui.radio_value(&mut 0, 2, "Axis Constrained");

        ui.separator();
        ui.checkbox(&mut false, "Angle Snap");
        ui.add(egui::Slider::new(&mut 15.0, 1.0..=90.0).text("Snap Angle (deg)"));
    }

    fn show_scale_settings(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut true, "Uniform Scale");

        ui.separator();
        ui.label("Constraint Axis:");
        ui.radio_value(&mut 0, 0, "Uniform");
        ui.radio_value(&mut 0, 1, "X");
        ui.radio_value(&mut 0, 2, "Y");
        ui.radio_value(&mut 0, 3, "Z");

        ui.separator();
        ui.checkbox(&mut false, "Scale Snap");
        ui.add(egui::Slider::new(&mut 0.1, 0.01..=1.0).text("Snap Increment"));
    }

    fn show_extrude_settings(&mut self, ui: &mut egui::Ui) {
        ui.label("Extrude Mode:");
        ui.radio_value(&mut 0, 0, "Region");
        ui.radio_value(&mut 0, 1, "Individual");
        ui.radio_value(&mut 0, 2, "Along Normals");

        ui.separator();
        ui.add(egui::Slider::new(&mut 0.0, -10.0..=10.0).text("Offset"));
    }

    fn show_bevel_settings(&mut self, ui: &mut egui::Ui) {
        ui.add(egui::Slider::new(&mut 0.1, 0.0..=5.0).text("Width"));
        ui.add(egui::Slider::new(&mut 1, 1..=20).text("Segments"));

        ui.separator();
        ui.label("Profile:");
        ui.radio_value(&mut 0, 0, "Linear");
        ui.radio_value(&mut 0, 1, "Convex");
        ui.radio_value(&mut 0, 2, "Concave");
    }

    fn show_loop_cut_settings(&mut self, ui: &mut egui::Ui) {
        ui.add(egui::Slider::new(&mut 1, 1..=10).text("Number of Cuts"));
        ui.checkbox(&mut true, "Even Spacing");
        ui.add(egui::Slider::new(&mut 0.5, 0.0..=1.0).text("Position"));
    }

    fn show_knife_settings(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut false, "Snap to Vertex");
        ui.checkbox(&mut false, "Through Cut");
        ui.checkbox(&mut true, "Angle Snap");

        if true {
            ui.add(egui::Slider::new(&mut 45.0, 5.0..=90.0).text("Snap Angle"));
        }

        ui.separator();
        ui.label("Click to add cut points");
        ui.label("Double-click to apply");
    }

    fn show_sculpt_settings(&mut self, ui: &mut egui::Ui) {
        ui.label("Brush Type:");
        egui::ComboBox::from_label("")
            .selected_text("Standard")
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut 0, 0, "Standard");
                ui.selectable_value(&mut 0, 1, "Clay");
                ui.selectable_value(&mut 0, 2, "Grab");
                ui.selectable_value(&mut 0, 3, "Smooth");
                ui.selectable_value(&mut 0, 4, "Pinch");
                ui.selectable_value(&mut 0, 5, "Flatten");
            });

        ui.separator();
        ui.add(egui::Slider::new(&mut 1.0, 0.1..=10.0).text("Radius"));
        ui.add(egui::Slider::new(&mut 0.5, 0.0..=1.0).text("Strength"));

        ui.separator();
        ui.label("Falloff:");
        ui.radio_value(&mut 0, 0, "Smooth");
        ui.radio_value(&mut 0, 1, "Linear");
        ui.radio_value(&mut 0, 2, "Sharp");

        ui.separator();
        ui.label("Symmetry:");
        ui.checkbox(&mut false, "X");
        ui.checkbox(&mut false, "Y");
        ui.checkbox(&mut false, "Z");

        ui.separator();
        ui.checkbox(&mut false, "Invert");
    }

    /// Set active tool.
    pub fn set_active_tool(&mut self, tool: ActiveTool) {
        self.active_tool = tool;
    }
}

impl Default for ToolsPanel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tools_panel() {
        let panel = ToolsPanel::new();
        assert_eq!(panel.active_tool, ActiveTool::Select);
    }
}
