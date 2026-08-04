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

//! Selection tool for picking objects in the 3D viewport.

use std::collections::HashSet;

/// Selection mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectMode {
    /// Single click selection.
    Single,
    /// Box selection.
    Box,
    /// Lasso (free-form) selection.
    Lasso,
    /// Circle selection.
    Circle,
    /// Paint selection.
    Paint,
}

/// Selection modifier for combining selections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectModifier {
    /// Replace selection.
    Replace,
    /// Add to selection (Shift).
    Add,
    /// Subtract from selection (Ctrl).
    Subtract,
    /// Toggle selection (Alt).
    Toggle,
}

/// Selection state tracking.
#[derive(Debug, Clone)]
pub struct SelectionState {
    /// Currently selected object IDs.
    pub selected: HashSet<u64>,
    /// Objects hovered under mouse.
    pub hovered: Option<u64>,
    /// Selection is active (during drag).
    pub active: bool,
}

impl SelectionState {
    pub fn new() -> Self {
        Self {
            selected: HashSet::new(),
            hovered: None,
            active: false,
        }
    }

    /// Clear all selections.
    pub fn clear(&mut self) {
        self.selected.clear();
    }

    /// Add object to selection.
    pub fn add(&mut self, id: u64) {
        self.selected.insert(id);
    }

    /// Remove object from selection.
    pub fn remove(&mut self, id: u64) {
        self.selected.remove(&id);
    }

    /// Toggle object selection.
    pub fn toggle(&mut self, id: u64) {
        if self.selected.contains(&id) {
            self.selected.remove(&id);
        } else {
            self.selected.insert(id);
        }
    }

    /// Check if object is selected.
    pub fn is_selected(&self, id: u64) -> bool {
        self.selected.contains(&id)
    }

    /// Get selection count.
    pub fn count(&self) -> usize {
        self.selected.len()
    }
}

impl Default for SelectionState {
    fn default() -> Self {
        Self::new()
    }
}

/// Selection tool.
#[derive(Debug, Clone)]
pub struct SelectTool {
    /// Current selection mode.
    pub mode: SelectMode,
    /// Selection modifier.
    pub modifier: SelectModifier,
    /// Selection state.
    pub state: SelectionState,
    /// Box selection start point (screen space).
    box_start: Option<(f64, f64)>,
    /// Box selection end point (screen space).
    box_end: Option<(f64, f64)>,
    /// Lasso points (screen space).
    lasso_points: Vec<(f64, f64)>,
    /// Circle selection center.
    circle_center: Option<(f64, f64)>,
    /// Circle selection radius.
    pub circle_radius: f64,
    /// Paint selection radius.
    pub paint_radius: f64,
}

impl SelectTool {
    /// Create a new selection tool.
    pub fn new() -> Self {
        Self {
            mode: SelectMode::Single,
            modifier: SelectModifier::Replace,
            state: SelectionState::new(),
            box_start: None,
            box_end: None,
            lasso_points: Vec::new(),
            circle_center: None,
            circle_radius: 50.0,
            paint_radius: 25.0,
        }
    }

    /// Activate tool.
    pub fn activate(&mut self) {
        tracing::debug!("Selection tool activated");
    }

    /// Deactivate tool.
    pub fn deactivate(&mut self) {
        self.cancel_selection();
        tracing::debug!("Selection tool deactivated");
    }

    /// Handle click event.
    pub fn handle_click(
        &mut self,
        screen_x: f64,
        screen_y: f64,
        shift: bool,
        ctrl: bool,
        alt: bool,
    ) {
        self.modifier = if shift {
            SelectModifier::Add
        } else if ctrl {
            SelectModifier::Subtract
        } else if alt {
            SelectModifier::Toggle
        } else {
            SelectModifier::Replace
        };

        match self.mode {
            SelectMode::Single => {
                self.state.active = true;
            }
            SelectMode::Box => {
                self.box_start = Some((screen_x, screen_y));
                self.box_end = Some((screen_x, screen_y));
                self.state.active = true;
            }
            SelectMode::Lasso => {
                self.lasso_points.clear();
                self.lasso_points.push((screen_x, screen_y));
                self.state.active = true;
            }
            SelectMode::Circle => {
                self.circle_center = Some((screen_x, screen_y));
                self.state.active = true;
            }
            SelectMode::Paint => {
                self.state.active = true;
            }
        }
    }

    /// Handle drag event.
    pub fn handle_drag(&mut self, screen_x: f64, screen_y: f64) {
        if !self.state.active {
            return;
        }

        match self.mode {
            SelectMode::Box => {
                self.box_end = Some((screen_x, screen_y));
            }
            SelectMode::Lasso => {
                self.lasso_points.push((screen_x, screen_y));
            }
            SelectMode::Circle => {
                self.circle_center = Some((screen_x, screen_y));
            }
            SelectMode::Paint => {}
            SelectMode::Single => {}
        }
    }

    /// Handle release event.
    pub fn handle_release(&mut self, objects_in_selection: &[u64]) {
        if !self.state.active {
            return;
        }

        match self.modifier {
            SelectModifier::Replace => {
                self.state.selected.clear();
                for &id in objects_in_selection {
                    self.state.selected.insert(id);
                }
            }
            SelectModifier::Add => {
                for &id in objects_in_selection {
                    self.state.selected.insert(id);
                }
            }
            SelectModifier::Subtract => {
                for &id in objects_in_selection {
                    self.state.selected.remove(&id);
                }
            }
            SelectModifier::Toggle => {
                for &id in objects_in_selection {
                    self.state.toggle(id);
                }
            }
        }

        self.cancel_selection();
    }

    /// Cancel current selection operation.
    pub fn cancel_selection(&mut self) {
        self.state.active = false;
        self.box_start = None;
        self.box_end = None;
        self.lasso_points.clear();
        self.circle_center = None;
    }

    /// Get box selection rectangle (min_x, min_y, max_x, max_y).
    pub fn get_box_rect(&self) -> Option<(f64, f64, f64, f64)> {
        if let (Some((x1, y1)), Some((x2, y2))) = (self.box_start, self.box_end) {
            let min_x = x1.min(x2);
            let max_x = x1.max(x2);
            let min_y = y1.min(y2);
            let max_y = y1.max(y2);
            Some((min_x, min_y, max_x, max_y))
        } else {
            None
        }
    }

    /// Get lasso points.
    pub fn get_lasso_points(&self) -> &[(f64, f64)] {
        &self.lasso_points
    }

    /// Get circle selection bounds.
    pub fn get_circle_bounds(&self) -> Option<(f64, f64, f64)> {
        self.circle_center.map(|(x, y)| (x, y, self.circle_radius))
    }

    /// Test if screen point is inside box selection.
    pub fn point_in_box(&self, x: f64, y: f64) -> bool {
        if let Some((min_x, min_y, max_x, max_y)) = self.get_box_rect() {
            x >= min_x && x <= max_x && y >= min_y && y <= max_y
        } else {
            false
        }
    }

    /// Test if screen point is inside lasso selection.
    pub fn point_in_lasso(&self, x: f64, y: f64) -> bool {
        if self.lasso_points.len() < 3 {
            return false;
        }

        let mut inside = false;
        let mut j = self.lasso_points.len() - 1;

        for i in 0..self.lasso_points.len() {
            let (xi, yi) = self.lasso_points[i];
            let (xj, yj) = self.lasso_points[j];

            if ((yi > y) != (yj > y)) && (x < (xj - xi) * (y - yi) / (yj - yi) + xi) {
                inside = !inside;
            }
            j = i;
        }

        inside
    }

    /// Test if screen point is inside circle selection.
    pub fn point_in_circle(&self, x: f64, y: f64) -> bool {
        if let Some((cx, cy, r)) = self.get_circle_bounds() {
            let dx = x - cx;
            let dy = y - cy;
            dx * dx + dy * dy <= r * r
        } else {
            false
        }
    }

    /// Draw selection overlay using egui painter.
    pub fn draw_overlay(&self, painter: &egui::Painter) {
        use egui::{Color32, Pos2, Rect, Shape, Stroke};

        match self.mode {
            SelectMode::Box => {
                if let Some((min_x, min_y, max_x, max_y)) = self.get_box_rect() {
                    let rect = Rect::from_min_max(
                        Pos2::new(min_x as f32, min_y as f32),
                        Pos2::new(max_x as f32, max_y as f32),
                    );
                    painter.rect_stroke(
                        rect,
                        0.0,
                        Stroke::new(2.0, Color32::from_rgb(255, 255, 0)),
                    );
                    painter.rect_filled(
                        rect,
                        0.0,
                        Color32::from_rgba_premultiplied(255, 255, 0, 30),
                    );
                }
            }
            SelectMode::Lasso => {
                if self.lasso_points.len() >= 2 {
                    let points: Vec<Pos2> = self
                        .lasso_points
                        .iter()
                        .map(|&(x, y)| Pos2::new(x as f32, y as f32))
                        .collect();
                    painter.add(Shape::line(
                        points,
                        Stroke::new(2.0, Color32::from_rgb(255, 255, 0)),
                    ));
                }
            }
            SelectMode::Circle => {
                if let Some((cx, cy)) = self.circle_center {
                    painter.circle_stroke(
                        Pos2::new(cx as f32, cy as f32),
                        self.circle_radius as f32,
                        Stroke::new(2.0, Color32::from_rgb(255, 255, 0)),
                    );
                }
            }
            SelectMode::Paint => {}
            SelectMode::Single => {}
        }
    }
}

impl Default for SelectTool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_tool_creation() {
        let tool = SelectTool::new();
        assert_eq!(tool.mode, SelectMode::Single);
        assert_eq!(tool.state.count(), 0);
    }

    #[test]
    fn test_selection_state() {
        let mut state = SelectionState::new();
        state.add(1);
        state.add(2);
        assert_eq!(state.count(), 2);
        assert!(state.is_selected(1));
        state.toggle(1);
        assert!(!state.is_selected(1));
    }

    #[test]
    fn test_box_selection() {
        let mut tool = SelectTool::new();
        tool.mode = SelectMode::Box;
        tool.handle_click(10.0, 10.0, false, false, false);
        tool.handle_drag(100.0, 100.0);

        assert!(tool.point_in_box(50.0, 50.0));
        assert!(!tool.point_in_box(5.0, 5.0));
    }

    #[test]
    fn test_point_in_lasso() {
        let mut tool = SelectTool::new();
        tool.lasso_points = vec![(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)];
        assert!(tool.point_in_lasso(50.0, 50.0));
        assert!(!tool.point_in_lasso(150.0, 150.0));
    }
}
