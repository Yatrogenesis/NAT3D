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

//! Scene hierarchy panel (outliner) for managing scene objects.

use std::collections::{HashMap, HashSet};

/// Tree node state (expanded/collapsed).
#[derive(Debug, Clone)]
pub struct TreeState {
    expanded: HashMap<u64, bool>,
}

impl TreeState {
    pub fn new() -> Self {
        Self {
            expanded: HashMap::new(),
        }
    }

    pub fn is_expanded(&self, id: u64) -> bool {
        *self.expanded.get(&id).unwrap_or(&false)
    }

    pub fn toggle(&mut self, id: u64) {
        let current = self.is_expanded(id);
        self.expanded.insert(id, !current);
    }

    pub fn expand(&mut self, id: u64) {
        self.expanded.insert(id, true);
    }

    pub fn collapse(&mut self, id: u64) {
        self.expanded.insert(id, false);
    }
}

impl Default for TreeState {
    fn default() -> Self {
        Self::new()
    }
}

/// Scene object data for hierarchy display.
#[derive(Debug, Clone)]
pub struct SceneObject {
    pub id: u64,
    pub name: String,
    pub object_type: String,
    pub visible: bool,
    pub locked: bool,
    pub parent_id: Option<u64>,
    pub children: Vec<u64>,
}

/// Hierarchy panel.
#[derive(Debug, Clone)]
pub struct HierarchyPanel {
    /// Tree expansion state.
    pub tree_state: TreeState,
    /// Search filter text.
    pub search_filter: String,
    /// Currently selected items.
    pub selected_items: HashSet<u64>,
    /// Item being dragged (for drag-drop).
    dragging: Option<u64>,
    /// Item under drag cursor.
    drop_target: Option<u64>,
}

impl HierarchyPanel {
    /// Create a new hierarchy panel.
    pub fn new() -> Self {
        Self {
            tree_state: TreeState::new(),
            search_filter: String::new(),
            selected_items: HashSet::new(),
            dragging: None,
            drop_target: None,
        }
    }

    /// Show the hierarchy panel.
    pub fn show(&mut self, ui: &mut egui::Ui, objects: &[SceneObject]) {
        ui.heading("Scene");

        // Search bar
        ui.horizontal(|ui| {
            ui.label("Search:");
            ui.text_edit_singleline(&mut self.search_filter);
            if ui.button("Clear").clicked() {
                self.search_filter.clear();
            }
        });

        ui.separator();

        // Hierarchy tree
        egui::ScrollArea::vertical().show(ui, |ui| {
            let root_objects: Vec<_> = objects
                .iter()
                .filter(|obj| obj.parent_id.is_none())
                .collect();

            for obj in root_objects {
                self.draw_tree_node(ui, obj, objects);
            }
        });

        // Context menu (right-click)
        ui.add_space(4.0);
        let response = ui.allocate_response(egui::Vec2::ZERO, egui::Sense::click());
        response.context_menu(|ui| {
            self.context_menu(ui);
        });
    }

    /// Draw a single tree node.
    fn draw_tree_node(
        &mut self,
        ui: &mut egui::Ui,
        obj: &SceneObject,
        all_objects: &[SceneObject],
    ) {
        // Filter by search
        if !self.search_filter.is_empty()
            && !obj
                .name
                .to_lowercase()
                .contains(&self.search_filter.to_lowercase())
        {
            return;
        }

        let has_children = !obj.children.is_empty();
        let is_expanded = self.tree_state.is_expanded(obj.id);
        let is_selected = self.selected_items.contains(&obj.id);

        ui.horizontal(|ui| {
            // Expand/collapse arrow
            if has_children {
                let icon = if is_expanded { "▼" } else { "▶" };
                if ui.small_button(icon).clicked() {
                    self.tree_state.toggle(obj.id);
                }
            } else {
                ui.add_space(20.0);
            }

            // Object icon
            let icon = match obj.object_type.as_str() {
                "mesh" => "[M]",
                "camera" => "[C]",
                "light" => "[L]",
                "empty" => "[E]",
                _ => "[O]",
            };
            ui.label(icon);

            // Object name (clickable)
            let text = egui::RichText::new(&obj.name);
            let text = if is_selected {
                text.strong().color(egui::Color32::from_rgb(100, 150, 255))
            } else {
                text
            };

            let response = ui.selectable_label(is_selected, text);

            if response.clicked() {
                if ui.input(|i| i.modifiers.shift) {
                    self.selected_items.insert(obj.id);
                } else if ui.input(|i| i.modifiers.ctrl) {
                    if is_selected {
                        self.selected_items.remove(&obj.id);
                    } else {
                        self.selected_items.insert(obj.id);
                    }
                } else {
                    self.selected_items.clear();
                    self.selected_items.insert(obj.id);
                }
            }

            // Visibility toggle
            let vis_icon = if obj.visible { "vis" } else { "hid" };
            if ui.small_button(vis_icon).clicked() {
                // Visibility toggle would be handled by callback
            }

            // Lock toggle
            let lock_icon = if obj.locked { "lock" } else { "open" };
            if ui.small_button(lock_icon).clicked() {
                // Lock toggle would be handled by callback
            }
        });

        // Draw children if expanded
        if has_children && is_expanded {
            ui.indent(obj.id, |ui| {
                for &child_id in &obj.children {
                    if let Some(child) = all_objects.iter().find(|o| o.id == child_id) {
                        self.draw_tree_node(ui, child, all_objects);
                    }
                }
            });
        }
    }

    /// Show context menu.
    fn context_menu(&mut self, ui: &mut egui::Ui) {
        if ui.button("Add Object").clicked() {
            ui.close_menu();
        }
        if ui.button("Duplicate").clicked() {
            ui.close_menu();
        }
        if ui.button("Delete").clicked() {
            ui.close_menu();
        }
        ui.separator();
        if ui.button("Select Children").clicked() {
            ui.close_menu();
        }
        if ui.button("Select Parent").clicked() {
            ui.close_menu();
        }
    }

    /// Handle drag-drop operation.
    pub fn handle_drag_drop(&mut self, dragged: u64, target: u64) {
        // Re-parent logic would go here
        tracing::debug!("Drag {} to {}", dragged, target);
    }

    /// Clear selection.
    pub fn clear_selection(&mut self) {
        self.selected_items.clear();
    }

    /// Select object.
    pub fn select(&mut self, id: u64) {
        self.selected_items.clear();
        self.selected_items.insert(id);
    }

    /// Add to selection.
    pub fn add_to_selection(&mut self, id: u64) {
        self.selected_items.insert(id);
    }
}

impl Default for HierarchyPanel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tree_state() {
        let mut state = TreeState::new();
        assert!(!state.is_expanded(1));
        state.toggle(1);
        assert!(state.is_expanded(1));
        state.toggle(1);
        assert!(!state.is_expanded(1));
    }

    #[test]
    fn test_hierarchy_panel() {
        let mut panel = HierarchyPanel::new();
        panel.select(1);
        assert!(panel.selected_items.contains(&1));
        panel.clear_selection();
        assert!(panel.selected_items.is_empty());
    }
}
