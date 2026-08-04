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

//! Properties panel for editing object properties.

use nalgebra::Vector3;

/// Transform properties.
#[derive(Debug, Clone)]
pub struct Transform {
    pub position: Vector3<f64>,
    pub rotation: Vector3<f64>, // Euler angles in degrees
    pub scale: Vector3<f64>,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: Vector3::zeros(),
            rotation: Vector3::zeros(),
            scale: Vector3::new(1.0, 1.0, 1.0),
        }
    }
}

/// Object properties.
#[derive(Debug, Clone)]
pub struct ObjectProperties {
    pub id: u64,
    pub name: String,
    pub transform: Transform,
    pub visible: bool,
    pub render_visible: bool,
    pub selectable: bool,
    pub object_type: String,
    pub modifier_count: usize,
}

impl Default for ObjectProperties {
    fn default() -> Self {
        Self {
            id: 0,
            name: String::from("Object"),
            transform: Transform::default(),
            visible: true,
            render_visible: true,
            selectable: true,
            object_type: String::from("Unknown"),
            modifier_count: 0,
        }
    }
}

/// Properties panel.
#[derive(Debug, Clone)]
pub struct PropertiesPanel {
    /// Copy of properties being edited.
    edit_buffer: Option<ObjectProperties>,
    /// Transform edit mode.
    transform_expanded: bool,
    /// Visibility section expanded.
    visibility_expanded: bool,
    /// Modifiers section expanded.
    modifiers_expanded: bool,
}

impl PropertiesPanel {
    /// Create a new properties panel.
    pub fn new() -> Self {
        Self {
            edit_buffer: None,
            transform_expanded: true,
            visibility_expanded: true,
            modifiers_expanded: true,
        }
    }

    /// Show the properties panel.
    pub fn show(&mut self, ui: &mut egui::Ui, selected_object: Option<&ObjectProperties>) {
        ui.heading("Properties");

        if let Some(obj) = selected_object {
            // Initialize edit buffer if needed
            if self.edit_buffer.is_none() || self.edit_buffer.as_ref().unwrap().id != obj.id {
                self.edit_buffer = Some(obj.clone());
            }

            if self.edit_buffer.is_some() {
                let mut props = self.edit_buffer.take().unwrap();
                Self::draw_object_info_static(ui, &mut props);
                ui.separator();
                Self::draw_transform_static(ui, &mut props, self.transform_expanded);
                ui.separator();
                Self::draw_visibility_static(ui, &mut props);
                ui.separator();
                Self::draw_modifiers_static(ui, &mut props, self.modifiers_expanded);
                self.edit_buffer = Some(props);
            }
        } else {
            ui.label("No object selected");
            self.edit_buffer = None;
        }
    }

    /// Draw object info section.
    fn draw_object_info_static(ui: &mut egui::Ui, props: &mut ObjectProperties) {
        egui::CollapsingHeader::new("Object")
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut props.name);
                });
                ui.horizontal(|ui| {
                    ui.label("Type:");
                    ui.label(&props.object_type);
                });
                ui.horizontal(|ui| {
                    ui.label("ID:");
                    ui.label(format!("{}", props.id));
                });
            });
    }

    /// Draw transform section.
    fn draw_transform_static(ui: &mut egui::Ui, props: &mut ObjectProperties, expanded: bool) {
        egui::CollapsingHeader::new("Transform")
            .default_open(expanded)
            .show(ui, |ui| {
                // Position
                ui.label("Position:");
                ui.horizontal(|ui| {
                    ui.label("X:");
                    ui.add(egui::DragValue::new(&mut props.transform.position.x).speed(0.1));
                    ui.label("Y:");
                    ui.add(egui::DragValue::new(&mut props.transform.position.y).speed(0.1));
                    ui.label("Z:");
                    ui.add(egui::DragValue::new(&mut props.transform.position.z).speed(0.1));
                });

                // Rotation
                ui.label("Rotation (degrees):");
                ui.horizontal(|ui| {
                    ui.label("X:");
                    ui.add(egui::DragValue::new(&mut props.transform.rotation.x).speed(1.0));
                    ui.label("Y:");
                    ui.add(egui::DragValue::new(&mut props.transform.rotation.y).speed(1.0));
                    ui.label("Z:");
                    ui.add(egui::DragValue::new(&mut props.transform.rotation.z).speed(1.0));
                });

                // Scale
                ui.label("Scale:");
                ui.horizontal(|ui| {
                    ui.label("X:");
                    ui.add(egui::DragValue::new(&mut props.transform.scale.x).speed(0.01));
                    ui.label("Y:");
                    ui.add(egui::DragValue::new(&mut props.transform.scale.y).speed(0.01));
                    ui.label("Z:");
                    ui.add(egui::DragValue::new(&mut props.transform.scale.z).speed(0.01));
                });

                // Reset button
                if ui.button("Reset Transform").clicked() {
                    props.transform = Transform::default();
                }
            });
    }

    /// Draw visibility section.
    fn draw_visibility_static(ui: &mut egui::Ui, props: &mut ObjectProperties) {
        egui::CollapsingHeader::new("Visibility")
            .default_open(true)
            .show(ui, |ui| {
                ui.checkbox(&mut props.visible, "Visible in Viewport");
                ui.checkbox(&mut props.render_visible, "Visible in Render");
                ui.checkbox(&mut props.selectable, "Selectable");
            });
    }

    /// Draw modifiers section.
    fn draw_modifiers_static(ui: &mut egui::Ui, props: &mut ObjectProperties, expanded: bool) {
        egui::CollapsingHeader::new(format!("Modifiers ({})", props.modifier_count))
            .default_open(expanded)
            .show(ui, |ui| {
                if props.modifier_count == 0 {
                    ui.label("No modifiers");
                } else {
                    ui.label(format!("{} modifiers applied", props.modifier_count));
                }

                ui.horizontal(|ui| {
                    if ui.button("Add Modifier").clicked() {
                        // Add modifier logic
                    }
                    if ui.button("Remove All").clicked() {
                        props.modifier_count = 0;
                    }
                });
            });
    }

    /// Get edited properties.
    pub fn get_edited_properties(&self) -> Option<&ObjectProperties> {
        self.edit_buffer.as_ref()
    }

    /// Clear edit buffer.
    pub fn clear(&mut self) {
        self.edit_buffer = None;
    }
}

impl Default for PropertiesPanel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_properties_panel() {
        let panel = PropertiesPanel::new();
        assert!(panel.transform_expanded);
    }

    #[test]
    fn test_transform_default() {
        let transform = Transform::default();
        assert_eq!(transform.position, Vector3::zeros());
        assert_eq!(transform.scale, Vector3::new(1.0, 1.0, 1.0));
    }
}
