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

//! # Batch 28: Weight Painting
//!
//! Provides interactive vertex weight painting for skeletal deformation:
//! - Brush-based weight painting with radius and strength
//! - Weight visualization: Red (0.0) → Yellow (0.5) → Green (1.0)
//! - Bone influence display per vertex group
//! - Weight normalization (sum = 1.0 per vertex across all groups)

#![allow(clippy::all, dead_code, unused_imports)]

use crate::state::{AppState, SceneObject, VertexGroup};

// ─────────────────────────────────────────────────────────────────────────────
// Brush Settings
// ─────────────────────────────────────────────────────────────────────────────

/// Weight paint brush type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WeightBrushType {
    /// Add weight under brush.
    #[default]
    Add,
    /// Subtract weight under brush.
    Subtract,
    /// Blur/smooth weights under brush.
    Blur,
    /// Replace weight under brush with exact value.
    Replace,
}

/// Weight paint brush settings.
#[derive(Debug, Clone)]
pub struct WeightBrush {
    /// World-space radius of the brush.
    pub radius: f32,
    /// Brush strength (0.0–1.0).
    pub strength: f32,
    /// Weight value to paint (0.0–1.0).
    pub weight: f32,
    /// Falloff (0.0 = sharp, 1.0 = smooth gaussian).
    pub falloff: f32,
    /// Brush type.
    pub brush_type: WeightBrushType,
    /// Whether to normalize all groups after painting.
    pub auto_normalize: bool,
}

impl Default for WeightBrush {
    fn default() -> Self {
        Self {
            radius: 0.5,
            strength: 0.5,
            weight: 1.0,
            falloff: 0.5,
            brush_type: WeightBrushType::Add,
            auto_normalize: true,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Weight Painting Engine
// ─────────────────────────────────────────────────────────────────────────────

/// Result of a weight painting stroke.
#[derive(Debug, Clone, Default)]
pub struct PaintResult {
    /// Number of vertices affected.
    pub affected: usize,
    /// Min weight in affected area before painting.
    pub min_before: f32,
    /// Max weight in affected area before painting.
    pub max_before: f32,
}

/// Weight painting engine for NAT3D.
#[derive(Debug, Clone, Default)]
pub struct WeightPaintEngine {
    /// Currently active vertex group index.
    pub active_group: usize,
    /// Brush settings.
    pub brush: WeightBrush,
    /// Whether weight painting mode is active.
    pub active: bool,
    /// Whether to show bone influences overlay.
    pub show_bone_influences: bool,
    /// Currently highlighted bone index (for influence display).
    pub highlighted_bone: Option<usize>,
}

impl WeightPaintEngine {
    /// Create a new weight paint engine.
    pub fn new() -> Self {
        Self {
            active_group: 0,
            brush: WeightBrush::default(),
            active: false,
            show_bone_influences: false,
            highlighted_bone: None,
        }
    }

    /// Paint weights on a single object at a world-space brush center.
    ///
    /// `vertices` is the list of vertex world positions for the object.
    /// `brush_center` is the 3D world position of the brush stroke.
    ///
    /// Returns a `PaintResult` with info about the operation.
    pub fn paint(
        &self,
        obj: &mut SceneObject,
        vertices: &[[f32; 3]],
        brush_center: [f32; 3],
    ) -> PaintResult {
        // Ensure the active vertex group exists
        while obj.vertex_groups.len() <= self.active_group {
            let group_name = format!("Group.{:03}", obj.vertex_groups.len());
            obj.vertex_groups.push(VertexGroup {
                name: group_name,
                weights: Vec::new(),
            });
        }

        // Ensure vertex_weights has one entry per vertex
        if obj.vertex_weights.len() < vertices.len() {
            obj.vertex_weights.resize(vertices.len(), 0.0);
        }

        let mut affected = 0;
        let mut min_before = 1.0f32;
        let mut max_before = 0.0f32;
        let radius = self.brush.radius;
        let radius_sq = radius * radius;

        // Collect existing weights by vertex index for the active group
        let group = &obj.vertex_groups[self.active_group];
        let mut weight_map: std::collections::HashMap<usize, f32> =
            group.weights.iter().map(|&(vi, w)| (vi, w)).collect();

        for (vi, pos) in vertices.iter().enumerate() {
            let dx = pos[0] - brush_center[0];
            let dy = pos[1] - brush_center[1];
            let dz = pos[2] - brush_center[2];
            let dist_sq = dx * dx + dy * dy + dz * dz;

            if dist_sq <= radius_sq {
                let dist = dist_sq.sqrt();
                let t = (dist / radius).clamp(0.0, 1.0);

                // Gaussian-like falloff: 1.0 at center → 0.0 at edge
                let falloff_factor = {
                    let smooth = self.brush.falloff;
                    if smooth < 1e-5 {
                        1.0
                    } else {
                        let k = (-t * t / (2.0 * smooth * smooth)).exp();
                        k
                    }
                };

                let influence = self.brush.strength * falloff_factor;
                let old_w = *weight_map.get(&vi).unwrap_or(&0.0);

                min_before = min_before.min(old_w);
                max_before = max_before.max(old_w);

                let new_w = match self.brush.brush_type {
                    WeightBrushType::Add => (old_w + influence * self.brush.weight).clamp(0.0, 1.0),
                    WeightBrushType::Subtract => {
                        (old_w - influence * self.brush.weight).clamp(0.0, 1.0)
                    }
                    WeightBrushType::Replace => old_w + influence * (self.brush.weight - old_w),
                    WeightBrushType::Blur => {
                        // Average with neighboring vertices (approximate: just move toward mean)
                        old_w + influence * (0.5 - old_w) * 0.5
                    }
                };

                weight_map.insert(vi, new_w);
                obj.vertex_weights[vi] = new_w;
                affected += 1;
            }
        }

        // Write back to vertex group
        obj.vertex_groups[self.active_group].weights = weight_map.into_iter().collect();
        obj.vertex_groups[self.active_group]
            .weights
            .sort_by_key(|&(vi, _)| vi);

        if self.brush.auto_normalize {
            normalize_weights(obj, vertices.len());
        }

        PaintResult {
            affected,
            min_before,
            max_before,
        }
    }

    /// Get the weight at a specific vertex index for the active vertex group.
    pub fn get_weight(&self, obj: &SceneObject, vertex_idx: usize) -> f32 {
        if let Some(group) = obj.vertex_groups.get(self.active_group) {
            for &(vi, w) in &group.weights {
                if vi == vertex_idx {
                    return w;
                }
            }
        }
        obj.vertex_weights.get(vertex_idx).copied().unwrap_or(0.0)
    }

    /// Get bone influences on a vertex (all groups with weight > threshold).
    pub fn bone_influences(
        obj: &SceneObject,
        vertex_idx: usize,
        threshold: f32,
    ) -> Vec<(String, f32)> {
        let mut influences = Vec::new();
        for group in &obj.vertex_groups {
            for &(vi, w) in &group.weights {
                if vi == vertex_idx && w > threshold {
                    influences.push((group.name.clone(), w));
                }
            }
        }
        influences.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        influences
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Weight Normalization
// ─────────────────────────────────────────────────────────────────────────────

/// Normalize vertex weights so that all vertex groups sum to 1.0 per vertex.
/// Vertices with total weight 0.0 are left at 0.0 (no normalization applied).
pub fn normalize_weights(obj: &mut SceneObject, vertex_count: usize) {
    // Collect total weight per vertex
    let mut totals = vec![0.0f32; vertex_count];
    for group in &obj.vertex_groups {
        for &(vi, w) in &group.weights {
            if vi < vertex_count {
                totals[vi] += w;
            }
        }
    }

    // Normalize all groups
    for group in &mut obj.vertex_groups {
        for (vi, w) in &mut group.weights {
            let total = totals[*vi];
            if total > 1e-6 {
                *w = (*w / total).clamp(0.0, 1.0);
            }
        }
    }

    // Also sync the flat vertex_weights array from the first group (for GPU)
    if let Some(first_group) = obj.vertex_groups.first() {
        let weight_map: std::collections::HashMap<usize, f32> =
            first_group.weights.iter().map(|&(vi, w)| (vi, w)).collect();
        for vi in 0..vertex_count {
            if vi < obj.vertex_weights.len() {
                obj.vertex_weights[vi] = *weight_map.get(&vi).unwrap_or(&0.0);
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Weight-to-Color Visualization
// ─────────────────────────────────────────────────────────────────────────────

/// Convert a weight value (0.0–1.0) to an RGBA color for viewport display.
///
/// Color ramp: Red (0.0) → Yellow (0.5) → Green (1.0)
pub fn weight_to_color(weight: f32) -> [f32; 4] {
    let w = weight.clamp(0.0, 1.0);
    if w <= 0.5 {
        // Red (1,0,0) → Yellow (1,1,0)
        let t = w * 2.0;
        [1.0, t, 0.0, 1.0]
    } else {
        // Yellow (1,1,0) → Green (0,1,0)
        let t = (w - 0.5) * 2.0;
        [1.0 - t, 1.0, 0.0, 1.0]
    }
}

/// Convert weight to egui color for UI display.
pub fn weight_to_egui_color(weight: f32) -> egui::Color32 {
    let [r, g, b, _a] = weight_to_color(weight);
    egui::Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

use eframe::egui;

// ─────────────────────────────────────────────────────────────────────────────
// Weight Paint UI Panel
// ─────────────────────────────────────────────────────────────────────────────

/// Show the weight painting side panel in the egui UI.
pub fn show_weight_paint_panel(
    ui: &mut egui::Ui,
    engine: &mut WeightPaintEngine,
    state: &mut AppState,
    vertex_count: usize,
) {
    ui.heading("Weight Paint");
    ui.separator();

    // Active mode toggle
    let active_label = if engine.active {
        "Painting ON"
    } else {
        "Start Painting"
    };
    if ui.button(active_label).clicked() {
        engine.active = !engine.active;
    }

    ui.separator();
    ui.label("Vertex Group:");

    // Vertex group selector
    if let Some(idx) = state.selected_object {
        let obj = &mut state.objects[idx];

        // Add new group
        if ui.small_button("+ Add Group").clicked() {
            let name = format!("Group.{:03}", obj.vertex_groups.len());
            obj.vertex_groups.push(VertexGroup {
                name,
                weights: Vec::new(),
            });
        }

        let mut to_remove: Option<usize> = None;
        let groups: Vec<String> = obj.vertex_groups.iter().map(|g| g.name.clone()).collect();
        for (i, name) in groups.iter().enumerate() {
            ui.horizontal(|ui| {
                let selected = engine.active_group == i;
                if ui.selectable_label(selected, name).clicked() {
                    engine.active_group = i;
                }
                if ui.small_button("Remove").clicked() {
                    to_remove = Some(i);
                }
            });
        }

        if let Some(i) = to_remove {
            obj.vertex_groups.remove(i);
            if engine.active_group >= obj.vertex_groups.len() && !obj.vertex_groups.is_empty() {
                engine.active_group = obj.vertex_groups.len() - 1;
            }
        }

        ui.separator();

        // Normalize button
        if ui.button("Normalize All Weights").clicked() {
            normalize_weights(obj, vertex_count);
        }

        // Show group weight stats
        if let Some(group) = obj.vertex_groups.get(engine.active_group) {
            ui.separator();
            ui.label(format!("Group: {}", group.name));
            ui.label(format!("Painted vertices: {}", group.weights.len()));
            if !group.weights.is_empty() {
                let min_w = group.weights.iter().map(|&(_, w)| w).fold(1.0f32, f32::min);
                let max_w = group.weights.iter().map(|&(_, w)| w).fold(0.0f32, f32::max);
                let avg_w =
                    group.weights.iter().map(|&(_, w)| w).sum::<f32>() / group.weights.len() as f32;
                ui.label(format!(
                    "Min: {:.3}  Max: {:.3}  Avg: {:.3}",
                    min_w, max_w, avg_w
                ));
            }
        }
    }

    ui.separator();

    // ── Brush settings ──────────────────────────────────────────────────────

    ui.label("Brush Type:");
    ui.horizontal_wrapped(|ui| {
        for (label, variant) in &[
            ("Add", WeightBrushType::Add),
            ("Subtract", WeightBrushType::Subtract),
            ("Blur", WeightBrushType::Blur),
            ("Replace", WeightBrushType::Replace),
        ] {
            let selected = engine.brush.brush_type == *variant;
            if ui.selectable_label(selected, *label).clicked() {
                engine.brush.brush_type = *variant;
            }
        }
    });

    ui.add(
        egui::Slider::new(&mut engine.brush.radius, 0.05..=5.0)
            .text("Radius")
            .clamping(egui::SliderClamping::Always),
    );
    ui.add(
        egui::Slider::new(&mut engine.brush.strength, 0.0..=1.0)
            .text("Strength")
            .clamping(egui::SliderClamping::Always),
    );
    ui.add(
        egui::Slider::new(&mut engine.brush.weight, 0.0..=1.0)
            .text("Weight")
            .clamping(egui::SliderClamping::Always),
    );
    ui.add(
        egui::Slider::new(&mut engine.brush.falloff, 0.01..=1.0)
            .text("Falloff")
            .clamping(egui::SliderClamping::Always),
    );
    ui.checkbox(&mut engine.brush.auto_normalize, "Auto Normalize");

    // Current weight color swatch
    ui.separator();
    let color = weight_to_egui_color(engine.brush.weight);
    ui.horizontal(|ui| {
        ui.label("Weight Color:");
        let (rect, _) = ui.allocate_exact_size(egui::vec2(40.0, 18.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 4.0, color);
    });

    ui.separator();

    // ── Bone Influence Display ──────────────────────────────────────────────
    ui.label("Bone Influence Display:");
    ui.checkbox(&mut engine.show_bone_influences, "Show Bone Influences");

    if engine.show_bone_influences {
        if let Some(idx) = state.selected_object {
            let obj = &state.objects[idx];
            if !obj.bones.is_empty() {
                ui.label(format!("{} bones in armature", obj.bones.len()));
                for (bi, bone) in obj.bones.iter().enumerate() {
                    let hi = engine.highlighted_bone == Some(bi);
                    if ui.selectable_label(hi, &bone.name).clicked() {
                        engine.highlighted_bone = if hi { None } else { Some(bi) };
                    }
                }
            } else {
                ui.label("(No bones — add an armature)");
            }
        }
    }

    ui.separator();

    // Weight ramp legend
    ui.label("Weight Ramp:");
    let (ramp_rect, _) = ui.allocate_exact_size(egui::vec2(200.0, 16.0), egui::Sense::hover());
    let painter = ui.painter();
    for i in 0..200_u32 {
        let t = i as f32 / 199.0;
        let color = weight_to_egui_color(t);
        let x = ramp_rect.min.x + i as f32;
        painter.line_segment(
            [
                egui::pos2(x, ramp_rect.min.y),
                egui::pos2(x, ramp_rect.max.y),
            ],
            egui::Stroke::new(1.0_f32, color),
        );
    }
    ui.horizontal(|ui| {
        ui.label("0.0");
        ui.add_space(80.0);
        ui.label("0.5");
        ui.add_space(80.0);
        ui.label("1.0");
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// Viewport Overlay: draw weight colors over mesh vertices
// ─────────────────────────────────────────────────────────────────────────────

/// Draw weight painting overlay in the egui painter (for 2D-projected viewport use).
///
/// `projected_vertices` is a list of `(screen_x, screen_y, weight)` tuples.
pub fn draw_weight_overlay(
    painter: &egui::Painter,
    projected_vertices: &[(f32, f32, f32)],
    radius: f32,
) {
    for &(sx, sy, weight) in projected_vertices {
        let color = weight_to_egui_color(weight);
        painter.circle_filled(egui::pos2(sx, sy), radius.max(3.0), color);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{AppState, VertexGroup};

    /// Create a minimal SceneObject for testing (no geometry, just for data fields).
    fn make_test_obj() -> crate::state::SceneObject {
        AppState::default()
            .objects
            .into_iter()
            .next()
            .expect("default scene has at least one object")
    }

    #[test]
    fn test_weight_to_color_boundaries() {
        let red = weight_to_color(0.0);
        assert!((red[0] - 1.0).abs() < 1e-5, "0.0 should be fully red");
        assert!(red[1] < 0.05, "0.0 should have no green");

        let green = weight_to_color(1.0);
        assert!(green[0] < 0.05, "1.0 should have no red");
        assert!((green[1] - 1.0).abs() < 1e-5, "1.0 should be fully green");

        let yellow = weight_to_color(0.5);
        assert!((yellow[0] - 1.0).abs() < 1e-5, "0.5 should be yellow (r=1)");
        assert!((yellow[1] - 1.0).abs() < 1e-5, "0.5 should be yellow (g=1)");
        assert!(yellow[2] < 0.05, "0.5 should have no blue");
    }

    #[test]
    fn test_weight_to_color_midpoints() {
        let quarter = weight_to_color(0.25);
        assert!((quarter[0] - 1.0).abs() < 1e-5);
        assert!((quarter[1] - 0.5).abs() < 1e-5);

        let three_quarter = weight_to_color(0.75);
        assert!((three_quarter[0] - 0.5).abs() < 1e-5);
        assert!((three_quarter[1] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_normalize_weights_sums_to_one() {
        let mut obj = make_test_obj();
        obj.vertex_groups.clear();
        obj.vertex_groups.push(VertexGroup {
            name: "GroupA".to_string(),
            weights: vec![(0, 0.8), (1, 0.4)],
        });
        obj.vertex_groups.push(VertexGroup {
            name: "GroupB".to_string(),
            weights: vec![(0, 0.4), (1, 0.8)],
        });
        obj.vertex_weights = vec![0.0; 8];

        normalize_weights(&mut obj, 8);

        // Vertex 0: 0.8 + 0.4 = 1.2 → GroupA≈0.667, GroupB≈0.333
        let ga_w0 = obj.vertex_groups[0]
            .weights
            .iter()
            .find(|&&(vi, _)| vi == 0)
            .map(|&(_, w)| w)
            .unwrap_or(0.0);
        let gb_w0 = obj.vertex_groups[1]
            .weights
            .iter()
            .find(|&&(vi, _)| vi == 0)
            .map(|&(_, w)| w)
            .unwrap_or(0.0);
        assert!(
            (ga_w0 + gb_w0 - 1.0).abs() < 1e-5,
            "Weights should sum to 1.0"
        );
    }

    #[test]
    fn test_paint_add_increases_weight() {
        let mut obj = make_test_obj();
        obj.vertex_groups.clear();
        obj.vertex_groups.push(VertexGroup {
            name: "Group.000".to_string(),
            weights: vec![],
        });
        obj.vertex_weights = vec![0.0; 8];

        let engine = WeightPaintEngine {
            active_group: 0,
            brush: WeightBrush {
                radius: 100.0, // large enough to cover all test vertices
                strength: 1.0,
                weight: 1.0,
                falloff: 0.01,
                brush_type: WeightBrushType::Add,
                auto_normalize: false,
            },
            active: true,
            ..Default::default()
        };

        let vertices = vec![
            [0.0f32, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [-1.0, -1.0, 0.0],
            [0.0, 0.5, 0.5],
            [0.5, 0.0, 0.5],
            [-0.5, 0.5, 0.0],
            [0.0, -0.5, 0.5],
        ];
        let result = engine.paint(&mut obj, &vertices, [0.0, 0.0, 0.0]);
        assert!(
            result.affected > 0,
            "At least one vertex should be affected"
        );
        assert!(
            obj.vertex_weights[0] > 0.0,
            "Vertex 0 weight should have increased"
        );
    }
}
