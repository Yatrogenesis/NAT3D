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

//! Dark theme for NAT3D UI.
//!
//! Professional dark color scheme inspired by Blender and modern IDEs.

use egui::{style::Widgets, Color32, Rgba, Stroke, Visuals};

/// Dark theme color palette.
#[derive(Debug, Clone, Copy)]
pub struct DarkTheme;

impl DarkTheme {
    /// Background colors.
    pub const BACKGROUND: Color32 = Color32::from_rgb(32, 32, 36); // Main background
    pub const PANEL: Color32 = Color32::from_rgb(42, 42, 46); // Panel background
    pub const WINDOW: Color32 = Color32::from_rgb(38, 38, 42); // Window background
    pub const VIEWPORT: Color32 = Color32::from_rgb(28, 28, 30); // Viewport background

    /// UI element colors.
    pub const BUTTON: Color32 = Color32::from_rgb(60, 60, 64); // Button idle
    pub const BUTTON_HOVER: Color32 = Color32::from_rgb(75, 75, 80); // Button hovered
    pub const BUTTON_ACTIVE: Color32 = Color32::from_rgb(45, 120, 200); // Button active/pressed

    /// Text colors.
    pub const TEXT: Color32 = Color32::from_rgb(220, 220, 220); // Primary text
    pub const TEXT_DIM: Color32 = Color32::from_rgb(150, 150, 150); // Secondary text
    pub const TEXT_DISABLED: Color32 = Color32::from_rgb(100, 100, 100); // Disabled text

    /// Accent colors.
    pub const ACCENT: Color32 = Color32::from_rgb(45, 120, 200); // Primary accent (blue)
    pub const ACCENT_HOVER: Color32 = Color32::from_rgb(60, 140, 220); // Accent hover
    pub const SUCCESS: Color32 = Color32::from_rgb(80, 180, 80); // Success (green)
    pub const WARNING: Color32 = Color32::from_rgb(220, 160, 40); // Warning (orange)
    pub const ERROR: Color32 = Color32::from_rgb(200, 60, 60); // Error (red)

    /// Selection colors.
    pub const SELECTION: Color32 = Color32::from_rgba_premultiplied(45, 120, 200, 80); // Selection overlay
    pub const SELECTION_STRONG: Color32 = Color32::from_rgb(45, 120, 200); // Strong selection

    /// Border/separator colors.
    pub const BORDER: Color32 = Color32::from_rgb(60, 60, 64); // Border
    pub const SEPARATOR: Color32 = Color32::from_rgb(50, 50, 54); // Separator line

    /// Widget-specific colors.
    pub const SLIDER_BG: Color32 = Color32::from_rgb(50, 50, 54); // Slider background
    pub const SLIDER_HANDLE: Color32 = Color32::from_rgb(120, 120, 125); // Slider handle
    pub const CHECKBOX_BG: Color32 = Color32::from_rgb(50, 50, 54); // Checkbox background
    pub const CHECKBOX_CHECK: Color32 = Color32::from_rgb(220, 220, 220); // Checkmark

    /// Grid colors.
    pub const GRID_MAJOR: Color32 = Color32::from_rgba_premultiplied(80, 80, 80, 128); // Major grid lines
    pub const GRID_MINOR: Color32 = Color32::from_rgba_premultiplied(60, 60, 60, 80); // Minor grid lines

    /// Axis colors (RGB = XYZ).
    pub const AXIS_X: Color32 = Color32::from_rgb(220, 60, 60); // X axis (red)
    pub const AXIS_Y: Color32 = Color32::from_rgb(80, 220, 60); // Y axis (green)
    pub const AXIS_Z: Color32 = Color32::from_rgb(60, 120, 220); // Z axis (blue)

    /// Create egui Visuals for dark theme.
    pub fn visuals() -> Visuals {
        Visuals {
            dark_mode: true,
            override_text_color: None,
            widgets: Self::widgets(),
            selection: Self::selection_style(),
            hyperlink_color: Self::ACCENT,
            faint_bg_color: Self::PANEL,
            extreme_bg_color: Self::BACKGROUND,
            code_bg_color: Color32::from_rgb(45, 45, 50),
            warn_fg_color: Self::WARNING,
            error_fg_color: Self::ERROR,
            menu_rounding: 4.0.into(),
            window_rounding: 6.0.into(),
            window_highlight_topmost: true,
            window_shadow: egui::epaint::Shadow {
                offset: [4.0, 4.0].into(),
                blur: 16.0,
                spread: 0.0,
                color: Color32::from_black_alpha(100),
            },
            window_fill: Self::WINDOW,
            window_stroke: Stroke::new(1.0, Self::BORDER),
            panel_fill: Self::PANEL,
            popup_shadow: egui::epaint::Shadow {
                offset: [2.0, 2.0].into(),
                blur: 8.0,
                spread: 0.0,
                color: Color32::from_black_alpha(120),
            },
            resize_corner_size: 12.0,
            text_cursor: egui::style::TextCursorStyle {
                stroke: Stroke::new(2.0, Self::TEXT),
                ..Default::default()
            },
            clip_rect_margin: 3.0,
            button_frame: true,
            collapsing_header_frame: false,
            indent_has_left_vline: true,
            striped: false,
            slider_trailing_fill: true,
            handle_shape: egui::style::HandleShape::Circle,
            interact_cursor: None,
            image_loading_spinners: true,
            numeric_color_space: egui::style::NumericColorSpace::GammaByte,
        }
    }

    /// Widget styles.
    fn widgets() -> Widgets {
        Widgets {
            noninteractive: egui::style::WidgetVisuals {
                bg_fill: Self::BACKGROUND,
                weak_bg_fill: Self::PANEL,
                bg_stroke: Stroke::new(1.0, Self::BORDER),
                fg_stroke: Stroke::new(1.0, Self::TEXT_DIM),
                rounding: 4.0.into(),
                expansion: 0.0,
            },
            inactive: egui::style::WidgetVisuals {
                bg_fill: Self::BUTTON,
                weak_bg_fill: Self::PANEL,
                bg_stroke: Stroke::new(1.0, Self::BORDER),
                fg_stroke: Stroke::new(1.0, Self::TEXT),
                rounding: 4.0.into(),
                expansion: 0.0,
            },
            hovered: egui::style::WidgetVisuals {
                bg_fill: Self::BUTTON_HOVER,
                weak_bg_fill: Color32::from_rgb(55, 55, 60),
                bg_stroke: Stroke::new(1.0, Self::ACCENT),
                fg_stroke: Stroke::new(1.5, Self::TEXT),
                rounding: 4.0.into(),
                expansion: 1.0,
            },
            active: egui::style::WidgetVisuals {
                bg_fill: Self::BUTTON_ACTIVE,
                weak_bg_fill: Self::ACCENT,
                bg_stroke: Stroke::new(1.0, Self::ACCENT_HOVER),
                fg_stroke: Stroke::new(2.0, Color32::WHITE),
                rounding: 4.0.into(),
                expansion: 1.0,
            },
            open: egui::style::WidgetVisuals {
                bg_fill: Self::PANEL,
                weak_bg_fill: Self::BACKGROUND,
                bg_stroke: Stroke::new(1.0, Self::ACCENT),
                fg_stroke: Stroke::new(1.0, Self::TEXT),
                rounding: 4.0.into(),
                expansion: 0.0,
            },
        }
    }

    /// Selection style.
    fn selection_style() -> egui::style::Selection {
        egui::style::Selection {
            bg_fill: Self::SELECTION,
            stroke: Stroke::new(1.0, Self::SELECTION_STRONG),
        }
    }

    /// Get color for axis (0=X, 1=Y, 2=Z).
    pub fn axis_color(axis: usize) -> Color32 {
        match axis {
            0 => Self::AXIS_X,
            1 => Self::AXIS_Y,
            2 => Self::AXIS_Z,
            _ => Self::TEXT_DIM,
        }
    }

    /// Get color as RGBA float array.
    pub fn to_rgba_f32(color: Color32) -> [f32; 4] {
        let rgba = Rgba::from(color);
        [rgba.r(), rgba.g(), rgba.b(), rgba.a()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_values() {
        // Ensure background is darker than panel
        assert!(DarkTheme::BACKGROUND.r() < DarkTheme::PANEL.r());

        // Ensure text is bright
        assert!(DarkTheme::TEXT.r() > 200);

        // Ensure accent is blue-ish
        assert!(DarkTheme::ACCENT.b() > DarkTheme::ACCENT.r());
    }

    #[test]
    fn test_axis_colors() {
        let x = DarkTheme::axis_color(0);
        let y = DarkTheme::axis_color(1);
        let z = DarkTheme::axis_color(2);

        // X should be reddest
        assert!(x.r() > x.g() && x.r() > x.b());
        // Y should be greenest
        assert!(y.g() > y.r() && y.g() > y.b());
        // Z should be bluest
        assert!(z.b() > z.r() && z.b() > z.g());
    }

    #[test]
    fn test_rgba_conversion() {
        let rgba = DarkTheme::to_rgba_f32(Color32::WHITE);
        assert!((rgba[0] - 1.0).abs() < 0.01);
        assert!((rgba[1] - 1.0).abs() < 0.01);
        assert!((rgba[2] - 1.0).abs() < 0.01);
        assert!((rgba[3] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_visuals_creation() {
        let visuals = DarkTheme::visuals();
        assert!(visuals.dark_mode);
        assert!(visuals.window_rounding.nw > 0.0);
    }
}
