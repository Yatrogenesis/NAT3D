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

//! Materials panel for PBR material editing.

/// PBR material properties.
#[derive(Debug, Clone)]
pub struct Material {
    pub id: u64,
    pub name: String,
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub emission: [f32; 3],
    pub emission_strength: f32,
    pub normal_map: Option<String>,
    pub ao_map: Option<String>,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            id: 0,
            name: String::from("Material"),
            base_color: [0.8, 0.8, 0.8, 1.0],
            metallic: 0.0,
            roughness: 0.5,
            emission: [0.0, 0.0, 0.0],
            emission_strength: 1.0,
            normal_map: None,
            ao_map: None,
        }
    }
}

/// Materials panel.
#[derive(Debug, Clone)]
pub struct MaterialsPanel {
    /// List of available materials.
    pub material_list: Vec<Material>,
    /// Currently selected material index.
    pub selected_material: Option<usize>,
    /// Preview needs update flag.
    pub preview_needs_update: bool,
    /// Edit buffer for current material.
    edit_buffer: Option<Material>,
}

impl MaterialsPanel {
    /// Create a new materials panel.
    pub fn new() -> Self {
        Self {
            material_list: vec![Material::default()],
            selected_material: None,
            preview_needs_update: false,
            edit_buffer: None,
        }
    }

    /// Show the materials panel.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.heading("Materials");

        // Material list
        egui::CollapsingHeader::new(format!("Materials ({})", self.material_list.len()))
            .default_open(true)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(150.0)
                    .show(ui, |ui| {
                        for (idx, mat) in self.material_list.iter().enumerate() {
                            let is_selected = self.selected_material == Some(idx);
                            if ui.selectable_label(is_selected, &mat.name).clicked() {
                                self.selected_material = Some(idx);
                                self.edit_buffer = Some(mat.clone());
                            }
                        }
                    });

                ui.horizontal(|ui| {
                    if ui.button("New").clicked() {
                        let mut new_mat = Material::default();
                        new_mat.id = self.material_list.len() as u64;
                        new_mat.name = format!("Material_{}", new_mat.id);
                        self.material_list.push(new_mat);
                    }
                    if ui.button("Duplicate").clicked() && self.selected_material.is_some() {
                        if let Some(idx) = self.selected_material {
                            let mut dup = self.material_list[idx].clone();
                            dup.id = self.material_list.len() as u64;
                            dup.name = format!("{}_copy", dup.name);
                            self.material_list.push(dup);
                        }
                    }
                    if ui.button("Delete").clicked() && self.selected_material.is_some() {
                        if let Some(idx) = self.selected_material {
                            self.material_list.remove(idx);
                            self.selected_material = None;
                            self.edit_buffer = None;
                        }
                    }
                });
            });

        ui.separator();

        // Material editor
        if self.edit_buffer.is_some() {
            let mut mat = self.edit_buffer.take().unwrap();
            let mut needs_update = self.preview_needs_update;
            Self::draw_material_editor_static(ui, &mut mat, &mut needs_update);
            self.preview_needs_update = needs_update;
            self.edit_buffer = Some(mat);
        } else {
            ui.label("No material selected");
        }
    }

    /// Draw material editor section (static to avoid borrow issues).
    fn draw_material_editor_static(ui: &mut egui::Ui, mat: &mut Material, needs_update: &mut bool) {
        egui::CollapsingHeader::new("Material Properties")
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    if ui.text_edit_singleline(&mut mat.name).changed() {
                        *needs_update = true;
                    }
                });

                ui.separator();

                ui.label("Base Color:");
                let mut color = [mat.base_color[0], mat.base_color[1], mat.base_color[2]];
                if ui.color_edit_button_rgb(&mut color).changed() {
                    mat.base_color = [color[0], color[1], color[2], mat.base_color[3]];
                    *needs_update = true;
                }

                ui.horizontal(|ui| {
                    ui.label("Alpha:");
                    if ui
                        .add(egui::Slider::new(&mut mat.base_color[3], 0.0..=1.0))
                        .changed()
                    {
                        *needs_update = true;
                    }
                });

                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("Metallic:");
                    if ui
                        .add(egui::Slider::new(&mut mat.metallic, 0.0..=1.0))
                        .changed()
                    {
                        *needs_update = true;
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Roughness:");
                    if ui
                        .add(egui::Slider::new(&mut mat.roughness, 0.0..=1.0))
                        .changed()
                    {
                        *needs_update = true;
                    }
                });

                ui.separator();

                ui.label("Emission:");
                let mut emission = mat.emission;
                if ui.color_edit_button_rgb(&mut emission).changed() {
                    mat.emission = emission;
                    *needs_update = true;
                }

                ui.horizontal(|ui| {
                    ui.label("Strength:");
                    if ui
                        .add(egui::Slider::new(&mut mat.emission_strength, 0.0..=10.0))
                        .changed()
                    {
                        *needs_update = true;
                    }
                });

                ui.separator();

                ui.label("Textures:");
                ui.horizontal(|ui| {
                    ui.label("Normal Map:");
                    if mat.normal_map.is_some() {
                        ui.label(mat.normal_map.as_ref().unwrap());
                        if ui.button("Clear").clicked() {
                            mat.normal_map = None;
                            *needs_update = true;
                        }
                    } else if ui.button("Load...").clicked() {
                        // File dialog would open here
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("AO Map:");
                    if mat.ao_map.is_some() {
                        ui.label(mat.ao_map.as_ref().unwrap());
                        if ui.button("Clear").clicked() {
                            mat.ao_map = None;
                            *needs_update = true;
                        }
                    } else if ui.button("Load...").clicked() {
                        // File dialog would open here
                    }
                });
            });

        ui.separator();
        ui.label("Preview:");
        ui.label("[Material preview sphere would render here]");
    }

    /// Get edited material.
    pub fn get_edited_material(&self) -> Option<&Material> {
        self.edit_buffer.as_ref()
    }

    /// Add material.
    pub fn add_material(&mut self, material: Material) {
        self.material_list.push(material);
    }
}

impl Default for MaterialsPanel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_materials_panel() {
        let panel = MaterialsPanel::new();
        assert_eq!(panel.material_list.len(), 1);
    }

    #[test]
    fn test_material_default() {
        let mat = Material::default();
        assert_eq!(mat.metallic, 0.0);
        assert_eq!(mat.roughness, 0.5);
    }
}
