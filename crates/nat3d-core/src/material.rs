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

//! Material system for NAT3D.
//!
//! Provides PBR (Physically Based Rendering) materials with
//! support for textures and various material properties.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

/// Unique identifier for a material.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MaterialId(pub Uuid);

impl MaterialId {
    /// Create a new unique material ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create a material ID from an existing UUID.
    #[must_use]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for MaterialId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for MaterialId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Material({})", &self.0.to_string()[..8])
    }
}

/// RGBA color value.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Color {
    /// Red component (0.0 - 1.0).
    pub r: f32,
    /// Green component (0.0 - 1.0).
    pub g: f32,
    /// Blue component (0.0 - 1.0).
    pub b: f32,
    /// Alpha component (0.0 - 1.0).
    pub a: f32,
}

impl Color {
    /// Create a new color.
    #[must_use]
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Create an opaque color.
    #[must_use]
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self::new(r, g, b, 1.0)
    }

    /// Create from sRGB bytes.
    #[must_use]
    pub fn from_srgb(r: u8, g: u8, b: u8) -> Self {
        Self::rgb(
            srgb_to_linear(f32::from(r) / 255.0),
            srgb_to_linear(f32::from(g) / 255.0),
            srgb_to_linear(f32::from(b) / 255.0),
        )
    }

    /// Create from hex string (e.g., "#FF5500" or "FF5500").
    #[must_use]
    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.trim_start_matches('#');
        if hex.len() != 6 {
            return None;
        }

        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;

        Some(Self::from_srgb(r, g, b))
    }

    /// Predefined colors.
    /// Predefined white color.
    pub const WHITE: Self = Self::rgb(1.0, 1.0, 1.0);
    /// Predefined black color.
    pub const BLACK: Self = Self::rgb(0.0, 0.0, 0.0);
    /// Predefined red color.
    pub const RED: Self = Self::rgb(1.0, 0.0, 0.0);
    /// Predefined green color.
    pub const GREEN: Self = Self::rgb(0.0, 1.0, 0.0);
    /// Predefined blue color.
    pub const BLUE: Self = Self::rgb(0.0, 0.0, 1.0);
    /// Predefined gray color.
    pub const GRAY: Self = Self::rgb(0.5, 0.5, 0.5);

    /// Convert to array.
    #[must_use]
    pub fn to_array(&self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }

    /// Convert to RGB array.
    #[must_use]
    pub fn to_rgb(&self) -> [f32; 3] {
        [self.r, self.g, self.b]
    }

    /// Linear interpolation.
    #[must_use]
    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        Self {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
            a: self.a + (other.a - self.a) * t,
        }
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::WHITE
    }
}

/// Convert sRGB to linear color space.
#[must_use]
fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Texture reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextureRef {
    /// Texture file path.
    pub path: PathBuf,
    /// UV channel index.
    pub uv_channel: u32,
    /// Texture tiling.
    pub tiling: [f32; 2],
    /// Texture offset.
    pub offset: [f32; 2],
}

impl TextureRef {
    /// Create a new texture reference.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            uv_channel: 0,
            tiling: [1.0, 1.0],
            offset: [0.0, 0.0],
        }
    }

    /// Builder method to set UV channel.
    #[must_use]
    pub fn with_uv_channel(mut self, channel: u32) -> Self {
        self.uv_channel = channel;
        self
    }

    /// Builder method to set tiling.
    #[must_use]
    pub fn with_tiling(mut self, u: f32, v: f32) -> Self {
        self.tiling = [u, v];
        self
    }

    /// Builder method to set offset.
    #[must_use]
    pub fn with_offset(mut self, u: f32, v: f32) -> Self {
        self.offset = [u, v];
        self
    }
}

/// Material blend mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BlendMode {
    /// Fully opaque.
    #[default]
    Opaque,
    /// Binary transparency (alpha cutoff).
    Cutout,
    /// Smooth transparency blending.
    Transparent,
    /// Additive blending.
    Additive,
}

/// PBR material properties.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialProperties {
    /// Base color (albedo).
    pub base_color: Color,
    /// Base color texture.
    pub base_color_texture: Option<TextureRef>,

    /// Metallic factor (0.0 = dielectric, 1.0 = metal).
    pub metallic: f32,
    /// Roughness factor (0.0 = smooth, 1.0 = rough).
    pub roughness: f32,
    /// Metallic-roughness texture (G=roughness, B=metallic).
    pub metallic_roughness_texture: Option<TextureRef>,

    /// Normal map texture.
    pub normal_texture: Option<TextureRef>,
    /// Normal map strength.
    pub normal_scale: f32,

    /// Ambient occlusion texture.
    pub ao_texture: Option<TextureRef>,
    /// Ambient occlusion strength.
    pub ao_strength: f32,

    /// Emissive color.
    pub emissive: Color,
    /// Emissive strength multiplier.
    pub emissive_strength: f32,
    /// Emissive texture.
    pub emissive_texture: Option<TextureRef>,

    /// Index of refraction (for transparent materials).
    pub ior: f32,
    /// Transmission (for transparent materials, 0.0 = opaque, 1.0 = fully transmissive).
    pub transmission: f32,

    /// Blend mode.
    pub blend_mode: BlendMode,
    /// Alpha cutoff for cutout mode.
    pub alpha_cutoff: f32,

    /// Whether to render both sides.
    pub double_sided: bool,
}

impl MaterialProperties {
    /// Create default PBR properties.
    #[must_use]
    pub fn new() -> Self {
        Self {
            base_color: Color::WHITE,
            base_color_texture: None,
            metallic: 0.0,
            roughness: 0.5,
            metallic_roughness_texture: None,
            normal_texture: None,
            normal_scale: 1.0,
            ao_texture: None,
            ao_strength: 1.0,
            emissive: Color::BLACK,
            emissive_strength: 1.0,
            emissive_texture: None,
            ior: 1.5,
            transmission: 0.0,
            blend_mode: BlendMode::Opaque,
            alpha_cutoff: 0.5,
            double_sided: false,
        }
    }

    /// Create metallic material properties.
    #[must_use]
    pub fn metallic(base_color: Color, roughness: f32) -> Self {
        Self {
            base_color,
            metallic: 1.0,
            roughness,
            ..Self::new()
        }
    }

    /// Create dielectric (non-metal) material properties.
    #[must_use]
    pub fn dielectric(base_color: Color, roughness: f32) -> Self {
        Self {
            base_color,
            metallic: 0.0,
            roughness,
            ..Self::new()
        }
    }

    /// Create glass-like material properties.
    #[must_use]
    pub fn glass(tint: Color, ior: f32) -> Self {
        Self {
            base_color: tint,
            metallic: 0.0,
            roughness: 0.0,
            ior,
            transmission: 1.0,
            blend_mode: BlendMode::Transparent,
            ..Self::new()
        }
    }

    /// Create emissive material properties.
    #[must_use]
    pub fn emissive(color: Color, strength: f32) -> Self {
        Self {
            base_color: Color::BLACK,
            emissive: color,
            emissive_strength: strength,
            ..Self::new()
        }
    }
}

impl Default for MaterialProperties {
    fn default() -> Self {
        Self::new()
    }
}

/// A material definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Material {
    /// Unique identifier.
    pub id: MaterialId,
    /// Material name.
    pub name: String,
    /// PBR properties.
    pub properties: MaterialProperties,
    /// Custom metadata.
    pub metadata: HashMap<String, String>,
}

impl Material {
    /// Create a new material with a name.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: MaterialId::new(),
            name: name.into(),
            properties: MaterialProperties::new(),
            metadata: HashMap::new(),
        }
    }

    /// Create a new material with properties.
    #[must_use]
    pub fn with_properties(name: impl Into<String>, properties: MaterialProperties) -> Self {
        Self {
            id: MaterialId::new(),
            name: name.into(),
            properties,
            metadata: HashMap::new(),
        }
    }

    /// Create a simple colored material.
    #[must_use]
    pub fn colored(name: impl Into<String>, color: Color) -> Self {
        Self::with_properties(
            name,
            MaterialProperties {
                base_color: color,
                ..MaterialProperties::new()
            },
        )
    }

    /// Builder method to set base color.
    #[must_use]
    pub fn with_base_color(mut self, color: Color) -> Self {
        self.properties.base_color = color;
        self
    }

    /// Builder method to set metallic.
    #[must_use]
    pub fn with_metallic(mut self, metallic: f32) -> Self {
        self.properties.metallic = metallic.clamp(0.0, 1.0);
        self
    }

    /// Builder method to set roughness.
    #[must_use]
    pub fn with_roughness(mut self, roughness: f32) -> Self {
        self.properties.roughness = roughness.clamp(0.0, 1.0);
        self
    }

    /// Builder method to set emissive.
    #[must_use]
    pub fn with_emissive(mut self, color: Color, strength: f32) -> Self {
        self.properties.emissive = color;
        self.properties.emissive_strength = strength;
        self
    }

    /// Check if this material uses any textures.
    #[must_use]
    pub fn has_textures(&self) -> bool {
        self.properties.base_color_texture.is_some()
            || self.properties.metallic_roughness_texture.is_some()
            || self.properties.normal_texture.is_some()
            || self.properties.ao_texture.is_some()
            || self.properties.emissive_texture.is_some()
    }

    /// Get all texture paths used by this material.
    #[must_use]
    pub fn texture_paths(&self) -> Vec<&PathBuf> {
        let mut paths = Vec::new();

        if let Some(ref tex) = self.properties.base_color_texture {
            paths.push(&tex.path);
        }
        if let Some(ref tex) = self.properties.metallic_roughness_texture {
            paths.push(&tex.path);
        }
        if let Some(ref tex) = self.properties.normal_texture {
            paths.push(&tex.path);
        }
        if let Some(ref tex) = self.properties.ao_texture {
            paths.push(&tex.path);
        }
        if let Some(ref tex) = self.properties.emissive_texture {
            paths.push(&tex.path);
        }

        paths
    }
}

impl Default for Material {
    fn default() -> Self {
        Self::new("Default")
    }
}

/// Preset materials for quick access.
pub mod presets {
    use super::{Color, Material, MaterialProperties};

    /// Standard gray material.
    #[must_use]
    pub fn standard_gray() -> Material {
        Material::with_properties(
            "Standard Gray",
            MaterialProperties::dielectric(Color::rgb(0.5, 0.5, 0.5), 0.5),
        )
    }

    /// Polished metal material.
    #[must_use]
    pub fn polished_metal() -> Material {
        Material::with_properties(
            "Polished Metal",
            MaterialProperties::metallic(Color::rgb(0.9, 0.9, 0.9), 0.1),
        )
    }

    /// Brushed metal material.
    #[must_use]
    pub fn brushed_metal() -> Material {
        Material::with_properties(
            "Brushed Metal",
            MaterialProperties::metallic(Color::rgb(0.7, 0.7, 0.7), 0.4),
        )
    }

    /// Gold material.
    #[must_use]
    pub fn gold() -> Material {
        Material::with_properties(
            "Gold",
            MaterialProperties::metallic(Color::rgb(1.0, 0.766, 0.336), 0.3),
        )
    }

    /// Copper material.
    #[must_use]
    pub fn copper() -> Material {
        Material::with_properties(
            "Copper",
            MaterialProperties::metallic(Color::rgb(0.955, 0.638, 0.538), 0.3),
        )
    }

    /// Plastic material.
    #[must_use]
    pub fn plastic(color: Color) -> Material {
        Material::with_properties("Plastic", MaterialProperties::dielectric(color, 0.3))
    }

    /// Rubber material.
    #[must_use]
    pub fn rubber(color: Color) -> Material {
        Material::with_properties("Rubber", MaterialProperties::dielectric(color, 0.9))
    }

    /// Glass material.
    #[must_use]
    pub fn glass() -> Material {
        Material::with_properties(
            "Glass",
            MaterialProperties::glass(Color::rgb(1.0, 1.0, 1.0), 1.5),
        )
    }

    /// Tinted glass material.
    #[must_use]
    pub fn tinted_glass(tint: Color) -> Material {
        Material::with_properties("Tinted Glass", MaterialProperties::glass(tint, 1.5))
    }

    /// Emissive material.
    #[must_use]
    pub fn emissive(color: Color, strength: f32) -> Material {
        Material::with_properties("Emissive", MaterialProperties::emissive(color, strength))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_material_creation() {
        let mat = Material::new("Test");
        assert_eq!(mat.name, "Test");
        assert!(!mat.has_textures());
    }

    #[test]
    fn test_color_from_hex() {
        let color = Color::from_hex("#FF5500").unwrap();
        assert!(color.r > 0.9);
        assert!(color.g > 0.0 && color.g < 0.5);
        assert!(color.b < 0.01);
    }

    #[test]
    fn test_material_builder() {
        let mat = Material::new("Test")
            .with_base_color(Color::RED)
            .with_metallic(1.0)
            .with_roughness(0.3);

        assert_eq!(mat.properties.base_color.r, 1.0);
        assert_eq!(mat.properties.metallic, 1.0);
        assert_eq!(mat.properties.roughness, 0.3);
    }

    #[test]
    fn test_presets() {
        let gold = presets::gold();
        assert_eq!(gold.properties.metallic, 1.0);

        let glass = presets::glass();
        assert_eq!(glass.properties.transmission, 1.0);
    }

    #[test]
    fn test_texture_paths() {
        let mut mat = Material::new("Textured");
        mat.properties.base_color_texture = Some(TextureRef::new("albedo.png"));
        mat.properties.normal_texture = Some(TextureRef::new("normal.png"));

        assert!(mat.has_textures());
        assert_eq!(mat.texture_paths().len(), 2);
    }
}
