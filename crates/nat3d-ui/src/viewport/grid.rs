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

//! Infinite grid rendering for 3D viewport.
//!
//! Provides visual reference planes with adaptive subdivision.

/// Grid plane orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridPlane {
    /// XZ plane (ground, Y-up).
    XZ,
    /// XY plane (front, Z-up).
    XY,
    /// YZ plane (side, X-up).
    YZ,
}

/// Grid rendering configuration.
#[derive(Debug, Clone)]
pub struct GridConfig {
    /// Which plane to render grid on.
    pub plane: GridPlane,
    /// Grid spacing (units between major lines).
    pub spacing: f32,
    /// Number of subdivision levels.
    pub subdivisions: u32,
    /// Grid extent (half-size from center).
    pub extent: f32,
    /// Major line color (RGBA).
    pub major_color: [f32; 4],
    /// Minor line color (RGBA).
    pub minor_color: [f32; 4],
    /// Axis line color (RGBA).
    pub axis_color: [f32; 4],
    /// Line width.
    pub line_width: f32,
    /// Fade distance (grid fades out beyond this distance).
    pub fade_distance: f32,
}

impl Default for GridConfig {
    fn default() -> Self {
        Self {
            plane: GridPlane::XZ,
            spacing: 1.0,
            subdivisions: 10,
            extent: 100.0,
            major_color: [0.5, 0.5, 0.5, 0.8],
            minor_color: [0.3, 0.3, 0.3, 0.4],
            axis_color: [0.8, 0.8, 0.8, 1.0],
            line_width: 1.0,
            fade_distance: 50.0,
        }
    }
}

impl GridConfig {
    /// Create new grid configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set grid plane.
    pub fn with_plane(mut self, plane: GridPlane) -> Self {
        self.plane = plane;
        self
    }

    /// Set grid spacing.
    pub fn with_spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing.max(0.01);
        self
    }

    /// Set grid extent.
    pub fn with_extent(mut self, extent: f32) -> Self {
        self.extent = extent.max(1.0);
        self
    }

    /// Generate grid lines for rendering.
    ///
    /// Returns pairs of (start, end) points for each line.
    pub fn generate_lines(&self) -> Vec<([f32; 3], [f32; 3])> {
        let mut lines = Vec::new();
        let half_extent = self.extent;
        let spacing = self.spacing;

        match self.plane {
            GridPlane::XZ => {
                // Lines parallel to X axis (varying Z)
                let mut z = -half_extent;
                while z <= half_extent {
                    lines.push(([-half_extent, 0.0, z], [half_extent, 0.0, z]));
                    z += spacing;
                }

                // Lines parallel to Z axis (varying X)
                let mut x = -half_extent;
                while x <= half_extent {
                    lines.push(([x, 0.0, -half_extent], [x, 0.0, half_extent]));
                    x += spacing;
                }
            }
            GridPlane::XY => {
                // Lines parallel to X axis (varying Y)
                let mut y = -half_extent;
                while y <= half_extent {
                    lines.push(([-half_extent, y, 0.0], [half_extent, y, 0.0]));
                    y += spacing;
                }

                // Lines parallel to Y axis (varying X)
                let mut x = -half_extent;
                while x <= half_extent {
                    lines.push(([x, -half_extent, 0.0], [x, half_extent, 0.0]));
                    x += spacing;
                }
            }
            GridPlane::YZ => {
                // Lines parallel to Y axis (varying Z)
                let mut z = -half_extent;
                while z <= half_extent {
                    lines.push(([0.0, -half_extent, z], [0.0, half_extent, z]));
                    z += spacing;
                }

                // Lines parallel to Z axis (varying Y)
                let mut y = -half_extent;
                while y <= half_extent {
                    lines.push(([0.0, y, -half_extent], [0.0, y, half_extent]));
                    y += spacing;
                }
            }
        }

        lines
    }

    /// Calculate fade alpha for distance from camera.
    pub fn fade_alpha(&self, distance: f32) -> f32 {
        if distance < self.fade_distance {
            1.0
        } else {
            let fade_range = self.extent - self.fade_distance;
            if fade_range > 0.0 {
                ((self.extent - distance) / fade_range).max(0.0)
            } else {
                0.0
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_generation() {
        let grid = GridConfig::new().with_spacing(10.0).with_extent(20.0);

        let lines = grid.generate_lines();

        // Should have lines for each grid intersection
        // 5 lines in each direction (at -20, -10, 0, 10, 20)
        assert!(lines.len() == 10);
    }

    #[test]
    fn test_fade_alpha() {
        let grid = GridConfig {
            fade_distance: 40.0,
            extent: 50.0,
            ..Default::default()
        };

        assert_eq!(grid.fade_alpha(30.0), 1.0); // Before fade
        assert!(grid.fade_alpha(45.0) < 1.0); // In fade range
        assert_eq!(grid.fade_alpha(50.0), 0.0); // At extent
    }
}
