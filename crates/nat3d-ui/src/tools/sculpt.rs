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

//! Sculpting tools for organic modeling.

use nalgebra::{Point3, Vector3};

/// Brush type for sculpting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrushType {
    /// Standard brush (push/pull).
    Standard,
    /// Clay brush (build up).
    Clay,
    /// Grab brush (move vertices).
    Grab,
    /// Smooth brush (average positions).
    Smooth,
    /// Pinch brush (contract).
    Pinch,
    /// Flatten brush.
    Flatten,
    /// Fill brush (raise to plane).
    Fill,
    /// Scrape brush (lower to plane).
    Scrape,
    /// Inflate brush (push along normals).
    Inflate,
    /// Crease brush (sharp details).
    Crease,
}

/// Brush falloff curve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrushFalloff {
    /// Smooth falloff.
    Smooth,
    /// Linear falloff.
    Linear,
    /// Sharp falloff.
    Sharp,
    /// Constant (no falloff).
    Constant,
}

/// Symmetry axes for mirrored sculpting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SymmetryAxes {
    pub x: bool,
    pub y: bool,
    pub z: bool,
}

impl SymmetryAxes {
    pub fn none() -> Self {
        Self {
            x: false,
            y: false,
            z: false,
        }
    }

    pub fn x_only() -> Self {
        Self {
            x: true,
            y: false,
            z: false,
        }
    }
}

/// Sculpt tool.
#[derive(Debug, Clone)]
pub struct SculptTool {
    /// Current brush type.
    pub brush_type: BrushType,
    /// Brush radius.
    pub radius: f64,
    /// Brush strength (0-1).
    pub strength: f64,
    /// Brush falloff curve.
    pub falloff: BrushFalloff,
    /// Symmetry axes.
    pub symmetry: SymmetryAxes,
    /// Invert brush effect.
    pub invert: bool,
    /// Is currently sculpting (stroke active).
    pub active: bool,
    /// Stroke points for smooth interpolation.
    stroke_points: Vec<Point3<f64>>,
}

impl SculptTool {
    /// Create a new sculpt tool.
    pub fn new() -> Self {
        Self {
            brush_type: BrushType::Standard,
            radius: 1.0,
            strength: 0.5,
            falloff: BrushFalloff::Smooth,
            symmetry: SymmetryAxes::none(),
            invert: false,
            active: false,
            stroke_points: Vec::new(),
        }
    }

    /// Activate tool.
    pub fn activate(&mut self) {
        tracing::debug!("Sculpt tool activated");
    }

    /// Deactivate tool.
    pub fn deactivate(&mut self) {
        self.active = false;
        self.stroke_points.clear();
        tracing::debug!("Sculpt tool deactivated");
    }

    /// Begin sculpt stroke.
    pub fn begin_stroke(&mut self, position: Point3<f64>) {
        self.active = true;
        self.stroke_points.clear();
        self.stroke_points.push(position);
    }

    /// Handle stroke movement.
    pub fn handle_stroke(&mut self, position: Point3<f64>) -> Option<SculptStroke> {
        if !self.active {
            return None;
        }

        self.stroke_points.push(position);

        Some(SculptStroke {
            position,
            brush_type: self.brush_type,
            radius: self.radius,
            strength: if self.invert {
                -self.strength
            } else {
                self.strength
            },
            falloff: self.falloff,
            symmetry: self.symmetry,
        })
    }

    /// End sculpt stroke.
    pub fn end_stroke(&mut self) {
        self.active = false;
        self.stroke_points.clear();
    }

    /// Apply brush effect to a vertex.
    pub fn apply_brush(
        &self,
        vertex_pos: Point3<f64>,
        brush_center: Point3<f64>,
        normal: Vector3<f64>,
    ) -> Vector3<f64> {
        let to_vertex = vertex_pos - brush_center;
        let distance = to_vertex.magnitude();

        if distance > self.radius {
            return Vector3::zeros(); // Outside brush radius
        }

        // Calculate falloff
        let falloff_factor = self.calculate_falloff(distance / self.radius);
        let effective_strength = self.strength * falloff_factor;

        // Apply brush type effect
        match self.brush_type {
            BrushType::Standard => {
                // Push along normal
                normal * effective_strength
            }
            BrushType::Clay => {
                // Build up
                normal * effective_strength.abs()
            }
            BrushType::Grab => {
                // Move towards brush
                to_vertex.normalize() * effective_strength
            }
            BrushType::Smooth => {
                // Smoothing handled externally (requires neighbors)
                Vector3::zeros()
            }
            BrushType::Pinch => {
                // Contract towards center
                -to_vertex.normalize() * effective_strength
            }
            BrushType::Flatten => {
                // Flatten handled externally (requires plane)
                Vector3::zeros()
            }
            BrushType::Fill => {
                // Raise to plane
                normal * effective_strength.abs()
            }
            BrushType::Scrape => {
                // Lower to plane
                -normal * effective_strength.abs()
            }
            BrushType::Inflate => {
                // Push along normal (always outward)
                normal * effective_strength.abs()
            }
            BrushType::Crease => {
                // Sharp push
                normal * effective_strength * 2.0
            }
        }
    }

    /// Calculate falloff based on distance (0-1).
    fn calculate_falloff(&self, t: f64) -> f64 {
        match self.falloff {
            BrushFalloff::Smooth => {
                // Smooth cubic
                let t = t.clamp(0.0, 1.0);
                let t2 = t * t;
                let t3 = t2 * t;
                1.0 - (3.0 * t2 - 2.0 * t3)
            }
            BrushFalloff::Linear => 1.0 - t.clamp(0.0, 1.0),
            BrushFalloff::Sharp => {
                // Quadratic
                let t = t.clamp(0.0, 1.0);
                1.0 - (t * t)
            }
            BrushFalloff::Constant => {
                if t <= 1.0 {
                    1.0
                } else {
                    0.0
                }
            }
        }
    }

    /// Toggle invert.
    pub fn toggle_invert(&mut self) {
        self.invert = !self.invert;
    }

    /// Set brush type.
    pub fn set_brush_type(&mut self, brush_type: BrushType) {
        self.brush_type = brush_type;
    }

    /// Toggle symmetry axis.
    pub fn toggle_symmetry_x(&mut self) {
        self.symmetry.x = !self.symmetry.x;
    }

    pub fn toggle_symmetry_y(&mut self) {
        self.symmetry.y = !self.symmetry.y;
    }

    pub fn toggle_symmetry_z(&mut self) {
        self.symmetry.z = !self.symmetry.z;
    }
}

/// Sculpt stroke data.
#[derive(Debug, Clone, Copy)]
pub struct SculptStroke {
    /// Stroke position.
    pub position: Point3<f64>,
    /// Brush type.
    pub brush_type: BrushType,
    /// Brush radius.
    pub radius: f64,
    /// Brush strength.
    pub strength: f64,
    /// Falloff curve.
    pub falloff: BrushFalloff,
    /// Symmetry axes.
    pub symmetry: SymmetryAxes,
}

impl Default for SculptTool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sculpt_tool_creation() {
        let tool = SculptTool::new();
        assert_eq!(tool.brush_type, BrushType::Standard);
        assert!(!tool.active);
    }

    #[test]
    fn test_falloff_smooth() {
        let tool = SculptTool::new();
        let f0 = tool.calculate_falloff(0.0);
        let f1 = tool.calculate_falloff(1.0);
        assert!((f0 - 1.0).abs() < 1e-6);
        assert!(f1.abs() < 1e-6);
    }

    #[test]
    fn test_falloff_constant() {
        let mut tool = SculptTool::new();
        tool.falloff = BrushFalloff::Constant;
        assert_eq!(tool.calculate_falloff(0.5), 1.0);
        assert_eq!(tool.calculate_falloff(1.5), 0.0);
    }

    #[test]
    fn test_stroke() {
        let mut tool = SculptTool::new();
        tool.begin_stroke(Point3::new(0.0, 0.0, 0.0));
        assert!(tool.active);
        tool.end_stroke();
        assert!(!tool.active);
    }
}
