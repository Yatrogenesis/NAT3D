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

//! Layer management system.
//!
//! Layers allow organizing objects and controlling their visibility,
//! editability, and rendering properties.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Unique identifier for a layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LayerId(pub Uuid);

impl LayerId {
    /// Create a new unique layer ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create a layer ID from an existing UUID.
    #[must_use]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for LayerId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for LayerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Layer({})", &self.0.to_string()[..8])
    }
}

/// Layer display color.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LayerColor {
    /// Red component (0-255).
    pub r: u8,
    /// Green component (0-255).
    pub g: u8,
    /// Blue component (0-255).
    pub b: u8,
}

impl LayerColor {
    /// Create a new color.
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Predefined colors for layers.
    /// Predefined red color.
    pub const RED: Self = Self::new(255, 100, 100);
    /// Predefined green color.
    pub const GREEN: Self = Self::new(100, 255, 100);
    /// Predefined blue color.
    pub const BLUE: Self = Self::new(100, 100, 255);
    /// Predefined yellow color.
    pub const YELLOW: Self = Self::new(255, 255, 100);
    /// Predefined cyan color.
    pub const CYAN: Self = Self::new(100, 255, 255);
    /// Predefined magenta color.
    pub const MAGENTA: Self = Self::new(255, 100, 255);
    /// Predefined orange color.
    pub const ORANGE: Self = Self::new(255, 180, 100);
    /// Predefined purple color.
    pub const PURPLE: Self = Self::new(180, 100, 255);
    /// Predefined gray color.
    pub const GRAY: Self = Self::new(180, 180, 180);
    /// Predefined white color.
    pub const WHITE: Self = Self::new(255, 255, 255);

    /// Get all predefined colors.
    pub const PALETTE: [Self; 10] = [
        Self::RED,
        Self::GREEN,
        Self::BLUE,
        Self::YELLOW,
        Self::CYAN,
        Self::MAGENTA,
        Self::ORANGE,
        Self::PURPLE,
        Self::GRAY,
        Self::WHITE,
    ];

    /// Convert to float RGB (0.0 - 1.0).
    #[must_use]
    pub fn to_f32(&self) -> [f32; 3] {
        [
            f32::from(self.r) / 255.0,
            f32::from(self.g) / 255.0,
            f32::from(self.b) / 255.0,
        ]
    }

    /// Create from float RGB.
    #[must_use]
    pub fn from_f32(rgb: [f32; 3]) -> Self {
        Self {
            r: (rgb[0].clamp(0.0, 1.0) * 255.0) as u8,
            g: (rgb[1].clamp(0.0, 1.0) * 255.0) as u8,
            b: (rgb[2].clamp(0.0, 1.0) * 255.0) as u8,
        }
    }
}

impl Default for LayerColor {
    fn default() -> Self {
        Self::GRAY
    }
}

/// Layer visibility and editing properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayerProperties {
    /// Visible in viewport.
    pub visible: bool,
    /// Rendered in final output.
    pub renderable: bool,
    /// Objects can be selected.
    pub selectable: bool,
    /// Objects can be modified.
    pub editable: bool,
    /// Layer is locked (cannot be modified or deleted).
    pub locked: bool,
}

impl LayerProperties {
    /// Create with all properties enabled.
    #[must_use]
    pub fn all_enabled() -> Self {
        Self {
            visible: true,
            renderable: true,
            selectable: true,
            editable: true,
            locked: false,
        }
    }

    /// Create with visibility only.
    #[must_use]
    pub fn visible_only() -> Self {
        Self {
            visible: true,
            renderable: false,
            selectable: false,
            editable: false,
            locked: false,
        }
    }

    /// Create a reference layer (visible but not editable).
    #[must_use]
    pub fn reference() -> Self {
        Self {
            visible: true,
            renderable: true,
            selectable: false,
            editable: false,
            locked: true,
        }
    }

    /// Create a hidden layer.
    #[must_use]
    pub fn hidden() -> Self {
        Self {
            visible: false,
            renderable: false,
            selectable: false,
            editable: false,
            locked: false,
        }
    }
}

impl Default for LayerProperties {
    fn default() -> Self {
        Self::all_enabled()
    }
}

/// A layer in the document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layer {
    /// Unique identifier.
    pub id: LayerId,
    /// Layer name.
    pub name: String,
    /// Display color.
    pub color: LayerColor,
    /// Layer properties.
    pub properties: LayerProperties,
    /// Parent layer (for nested layers).
    pub parent: Option<LayerId>,
    /// Order index for sorting.
    pub order: i32,
    /// Custom metadata.
    pub metadata: HashMap<String, String>,
}

impl Layer {
    /// Create a new layer.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: LayerId::new(),
            name: name.into(),
            color: LayerColor::default(),
            properties: LayerProperties::default(),
            parent: None,
            order: 0,
            metadata: HashMap::new(),
        }
    }

    /// Create the default layer.
    #[must_use]
    pub fn default_layer() -> Self {
        Self {
            id: LayerId::new(),
            name: "Default".into(),
            color: LayerColor::GRAY,
            properties: LayerProperties::all_enabled(),
            parent: None,
            order: 0,
            metadata: HashMap::new(),
        }
    }

    /// Builder method to set color.
    #[must_use]
    pub fn with_color(mut self, color: LayerColor) -> Self {
        self.color = color;
        self
    }

    /// Builder method to set properties.
    #[must_use]
    pub fn with_properties(mut self, properties: LayerProperties) -> Self {
        self.properties = properties;
        self
    }

    /// Builder method to set parent.
    #[must_use]
    pub fn with_parent(mut self, parent: LayerId) -> Self {
        self.parent = Some(parent);
        self
    }

    /// Builder method to set order.
    #[must_use]
    pub fn with_order(mut self, order: i32) -> Self {
        self.order = order;
        self
    }

    /// Check if objects on this layer can be interacted with.
    #[must_use]
    pub fn is_interactive(&self) -> bool {
        self.properties.visible && self.properties.selectable && !self.properties.locked
    }

    /// Check if objects on this layer can be edited.
    #[must_use]
    pub fn is_editable(&self) -> bool {
        self.properties.editable && !self.properties.locked
    }
}

impl Default for Layer {
    fn default() -> Self {
        Self::default_layer()
    }
}

/// Layer manager for organizing and querying layers.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LayerManager {
    /// All layers indexed by ID.
    layers: HashMap<LayerId, Layer>,
    /// The default layer ID.
    default_layer: Option<LayerId>,
    /// The active/current layer ID.
    active_layer: Option<LayerId>,
    /// Layer ordering.
    order: Vec<LayerId>,
}

impl LayerManager {
    /// Create a new layer manager with a default layer.
    #[must_use]
    pub fn new() -> Self {
        let default_layer = Layer::default_layer();
        let default_id = default_layer.id;

        let mut manager = Self {
            layers: HashMap::new(),
            default_layer: Some(default_id),
            active_layer: Some(default_id),
            order: vec![default_id],
        };

        manager.layers.insert(default_id, default_layer);
        manager
    }

    /// Get the number of layers.
    #[must_use]
    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    /// Check if a layer exists.
    #[must_use]
    pub fn contains(&self, id: LayerId) -> bool {
        self.layers.contains_key(&id)
    }

    /// Get a layer by ID.
    #[must_use]
    pub fn get(&self, id: LayerId) -> Option<&Layer> {
        self.layers.get(&id)
    }

    /// Get a mutable layer by ID.
    pub fn get_mut(&mut self, id: LayerId) -> Option<&mut Layer> {
        self.layers.get_mut(&id)
    }

    /// Get all layers.
    pub fn layers(&self) -> impl Iterator<Item = &Layer> {
        self.layers.values()
    }

    /// Get layers in order.
    pub fn layers_ordered(&self) -> impl Iterator<Item = &Layer> {
        self.order.iter().filter_map(|id| self.layers.get(id))
    }

    /// Get the default layer.
    #[must_use]
    pub fn default_layer(&self) -> Option<&Layer> {
        self.default_layer.and_then(|id| self.layers.get(&id))
    }

    /// Get the default layer ID.
    #[must_use]
    pub fn default_layer_id(&self) -> Option<LayerId> {
        self.default_layer
    }

    /// Get the active layer.
    #[must_use]
    pub fn active_layer(&self) -> Option<&Layer> {
        self.active_layer.and_then(|id| self.layers.get(&id))
    }

    /// Get the active layer ID.
    #[must_use]
    pub fn active_layer_id(&self) -> Option<LayerId> {
        self.active_layer
    }

    /// Set the active layer.
    pub fn set_active_layer(&mut self, id: LayerId) -> bool {
        if self.layers.contains_key(&id) {
            self.active_layer = Some(id);
            true
        } else {
            false
        }
    }

    /// Add a new layer.
    pub fn add_layer(&mut self, layer: Layer) -> LayerId {
        let id = layer.id;
        let order = layer.order;

        self.layers.insert(id, layer);

        // Insert in order
        let pos = self
            .order
            .iter()
            .position(|&lid| self.layers.get(&lid).map_or(true, |l| l.order > order))
            .unwrap_or(self.order.len());
        self.order.insert(pos, id);

        id
    }

    /// Create and add a new layer with a name.
    pub fn create_layer(&mut self, name: impl Into<String>) -> LayerId {
        let order = self.layers.len() as i32;
        let color_idx = self.layers.len() % LayerColor::PALETTE.len();
        let layer = Layer::new(name)
            .with_color(LayerColor::PALETTE[color_idx])
            .with_order(order);
        self.add_layer(layer)
    }

    /// Remove a layer.
    pub fn remove_layer(&mut self, id: LayerId) -> Option<Layer> {
        // Cannot remove the default layer
        if Some(id) == self.default_layer {
            return None;
        }

        if let Some(layer) = self.layers.remove(&id) {
            self.order.retain(|&lid| lid != id);

            // If removing the active layer, switch to default
            if self.active_layer == Some(id) {
                self.active_layer = self.default_layer;
            }

            Some(layer)
        } else {
            None
        }
    }

    /// Find layers by name.
    #[must_use]
    pub fn find_by_name(&self, name: &str) -> Vec<LayerId> {
        self.layers
            .iter()
            .filter(|(_, layer)| layer.name == name)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Get visible layers.
    #[must_use]
    pub fn visible_layers(&self) -> Vec<LayerId> {
        self.layers
            .iter()
            .filter(|(_, layer)| layer.properties.visible)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Get renderable layers.
    #[must_use]
    pub fn renderable_layers(&self) -> Vec<LayerId> {
        self.layers
            .iter()
            .filter(|(_, layer)| layer.properties.renderable)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Get editable layers.
    #[must_use]
    pub fn editable_layers(&self) -> Vec<LayerId> {
        self.layers
            .iter()
            .filter(|(_, layer)| layer.is_editable())
            .map(|(id, _)| *id)
            .collect()
    }

    /// Move a layer to a new position in the order.
    pub fn reorder_layer(&mut self, id: LayerId, new_position: usize) {
        if let Some(current_pos) = self.order.iter().position(|&lid| lid == id) {
            self.order.remove(current_pos);
            let new_pos = new_position.min(self.order.len());
            self.order.insert(new_pos, id);

            // Update order values
            for (i, &lid) in self.order.iter().enumerate() {
                if let Some(layer) = self.layers.get_mut(&lid) {
                    layer.order = i as i32;
                }
            }
        }
    }

    /// Toggle layer visibility.
    pub fn toggle_visibility(&mut self, id: LayerId) -> bool {
        if let Some(layer) = self.layers.get_mut(&id) {
            layer.properties.visible = !layer.properties.visible;
            layer.properties.visible
        } else {
            false
        }
    }

    /// Toggle layer lock.
    pub fn toggle_lock(&mut self, id: LayerId) -> bool {
        if let Some(layer) = self.layers.get_mut(&id) {
            // Cannot lock the default layer
            if Some(id) != self.default_layer {
                layer.properties.locked = !layer.properties.locked;
            }
            layer.properties.locked
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layer_creation() {
        let layer = Layer::new("Test Layer");
        assert_eq!(layer.name, "Test Layer");
        assert!(layer.properties.visible);
    }

    #[test]
    fn test_layer_manager_creation() {
        let manager = LayerManager::new();
        assert_eq!(manager.layer_count(), 1);
        assert!(manager.default_layer().is_some());
    }

    #[test]
    fn test_add_layer() {
        let mut manager = LayerManager::new();
        let id = manager.create_layer("New Layer");

        assert_eq!(manager.layer_count(), 2);
        assert!(manager.get(id).is_some());
        assert_eq!(manager.get(id).unwrap().name, "New Layer");
    }

    #[test]
    fn test_cannot_remove_default() {
        let mut manager = LayerManager::new();
        let default_id = manager.default_layer_id().unwrap();

        let result = manager.remove_layer(default_id);
        assert!(result.is_none());
        assert_eq!(manager.layer_count(), 1);
    }

    #[test]
    fn test_remove_layer() {
        let mut manager = LayerManager::new();
        let id = manager.create_layer("Removable");

        let removed = manager.remove_layer(id);
        assert!(removed.is_some());
        assert_eq!(manager.layer_count(), 1);
    }

    #[test]
    fn test_active_layer() {
        let mut manager = LayerManager::new();
        let id = manager.create_layer("New Layer");

        manager.set_active_layer(id);
        assert_eq!(manager.active_layer_id(), Some(id));
    }

    #[test]
    fn test_layer_visibility() {
        let mut manager = LayerManager::new();
        let id = manager.create_layer("Test");

        assert!(manager.get(id).unwrap().properties.visible);
        manager.toggle_visibility(id);
        assert!(!manager.get(id).unwrap().properties.visible);
    }

    #[test]
    fn test_layer_color() {
        let color = LayerColor::new(128, 64, 32);
        let f32_color = color.to_f32();

        assert!((f32_color[0] - 128.0 / 255.0).abs() < 0.01);
        assert!((f32_color[1] - 64.0 / 255.0).abs() < 0.01);
        assert!((f32_color[2] - 32.0 / 255.0).abs() < 0.01);
    }
}
