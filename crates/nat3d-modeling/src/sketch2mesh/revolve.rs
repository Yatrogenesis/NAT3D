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

//! Revolution operations for sketch-to-mesh.

use super::path::{Path2D, Point2D};
use super::MeshResult;
use std::f32::consts::PI;

/// Revolve a 2D path around an axis to create a 3D mesh.
pub fn revolve_path(
    path: &Path2D,
    axis: u32,
    angle_degrees: f32,
    segments: u32,
    resolution: u32,
) -> MeshResult {
    let mut mesh = MeshResult::new();
    let points = path.tesselate(resolution);

    if points.len() < 2 || segments < 3 {
        return mesh;
    }

    let angle_rad = angle_degrees * PI / 180.0;
    let angle_step = angle_rad / segments as f32;
    let is_full = (angle_degrees - 360.0).abs() < 0.001;

    // Create rings by rotating profile
    let mut rings: Vec<Vec<u32>> = Vec::new();

    let actual_segments = if is_full { segments } else { segments + 1 };

    for si in 0..actual_segments {
        let theta = angle_step * si as f32;
        let cos_t = theta.cos();
        let sin_t = theta.sin();

        let mut ring = Vec::new();

        for p in &points {
            let (x, y, z) = match axis {
                0 => {
                    // Revolve around X axis: Y becomes radius, rotate in YZ plane
                    let r = p.y;
                    (p.x, r * cos_t, r * sin_t)
                }
                1 => {
                    // Revolve around Y axis: X becomes radius, rotate in XZ plane
                    let r = p.x;
                    (r * cos_t, p.y, r * sin_t)
                }
                _ => {
                    // Revolve around Z axis: X becomes radius, rotate in XY plane
                    let r = p.x;
                    (r * cos_t, r * sin_t, p.y)
                }
            };

            // Calculate normal (pointing outward from axis)
            let normal = match axis {
                0 => {
                    let len = (y * y + z * z).sqrt();
                    if len > 0.0 {
                        [0.0, y / len, z / len]
                    } else {
                        [0.0, 1.0, 0.0]
                    }
                }
                1 => {
                    let len = (x * x + z * z).sqrt();
                    if len > 0.0 {
                        [x / len, 0.0, z / len]
                    } else {
                        [1.0, 0.0, 0.0]
                    }
                }
                _ => {
                    let len = (x * x + y * y).sqrt();
                    if len > 0.0 {
                        [x / len, y / len, 0.0]
                    } else {
                        [1.0, 0.0, 0.0]
                    }
                }
            };

            let u = si as f32 / segments as f32;
            let v = (p.y + 1.0) / 2.0; // Normalized

            let idx = mesh.add_vertex([x, y, z], normal, [u, v]);
            ring.push(idx);
        }

        rings.push(ring);
    }

    // Connect rings
    let ring_count = rings.len();
    let point_count = points.len();

    for ri in 0..ring_count {
        let next_ri = if is_full {
            (ri + 1) % ring_count
        } else {
            ri + 1
        };
        if next_ri >= ring_count {
            continue;
        }

        let ring0 = &rings[ri];
        let ring1 = &rings[next_ri];

        for pi in 0..point_count - 1 {
            mesh.add_quad(ring0[pi], ring0[pi + 1], ring1[pi + 1], ring1[pi]);
        }

        // Close the profile if it's closed
        if path.is_closed() {
            mesh.add_quad(
                ring0[point_count - 1],
                ring0[0],
                ring1[0],
                ring1[point_count - 1],
            );
        }
    }

    // Cap ends if not full revolution
    if !is_full {
        // Start cap
        cap_revolution(&mut mesh, &rings[0], axis, false);
        // End cap
        cap_revolution(&mut mesh, &rings[ring_count - 1], axis, true);
    }

    mesh
}

fn cap_revolution(mesh: &mut MeshResult, ring: &[u32], _axis: u32, reverse: bool) {
    if ring.len() < 3 {
        return;
    }

    // Simple fan triangulation
    for i in 1..ring.len() - 1 {
        if reverse {
            mesh.add_triangle(ring[0], ring[i + 1], ring[i]);
        } else {
            mesh.add_triangle(ring[0], ring[i], ring[i + 1]);
        }
    }
}

/// Revolve mesh generator with advanced options.
pub struct RevolveMesh {
    pub axis: u32,
    pub angle: f32,
    pub segments: u32,
    pub resolution: u32,
    pub offset: f32,
}

impl Default for RevolveMesh {
    fn default() -> Self {
        Self {
            axis: 1, // Y axis
            angle: 360.0,
            segments: 32,
            resolution: 16,
            offset: 0.0,
        }
    }
}

impl RevolveMesh {
    /// Create a new revolve mesh generator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set revolution axis (0=X, 1=Y, 2=Z).
    pub fn with_axis(mut self, axis: u32) -> Self {
        self.axis = axis.min(2);
        self
    }

    /// Set revolution angle in degrees.
    pub fn with_angle(mut self, angle: f32) -> Self {
        self.angle = angle;
        self
    }

    /// Set number of segments.
    pub fn with_segments(mut self, segments: u32) -> Self {
        self.segments = segments.max(3);
        self
    }

    /// Set curve resolution.
    pub fn with_resolution(mut self, resolution: u32) -> Self {
        self.resolution = resolution.max(1);
        self
    }

    /// Set offset from axis.
    pub fn with_offset(mut self, offset: f32) -> Self {
        self.offset = offset;
        self
    }

    /// Generate mesh from path.
    pub fn generate(&self, path: &Path2D) -> MeshResult {
        let mut path = path.clone();

        // Apply offset if needed
        if self.offset.abs() > 0.001 {
            path.transform(1.0, 0.0, Point2D::new(self.offset, 0.0));
        }

        revolve_path(&path, self.axis, self.angle, self.segments, self.resolution)
    }
}

/// Create common lathe shapes.
pub mod shapes {
    use super::*;

    /// Create a sphere by revolving a semicircle.
    pub fn sphere(radius: f32, segments: u32, rings: u32) -> MeshResult {
        let mut path = Path2D::new();

        // Create semicircle profile
        let angle_step = PI / rings as f32;
        path.move_to(0.0, -radius);

        for i in 1..=rings {
            let angle = -PI / 2.0 + angle_step * i as f32;
            let x = radius * angle.cos();
            let y = radius * angle.sin();
            path.line_to(x, y);
        }

        RevolveMesh::new()
            .with_axis(1)
            .with_segments(segments)
            .with_resolution(1)
            .generate(&path)
    }

    /// Create a torus.
    pub fn torus(major_radius: f32, minor_radius: f32, segments: u32, rings: u32) -> MeshResult {
        let circle = Path2D::circle(major_radius, 0.0, minor_radius);

        RevolveMesh::new()
            .with_axis(1)
            .with_segments(segments)
            .with_resolution(rings)
            .generate(&circle)
    }

    /// Create a vase shape.
    pub fn vase(height: f32, segments: u32) -> MeshResult {
        let mut path = Path2D::new();

        // Vase profile
        path.move_to(0.3, 0.0);
        path.cubic_to(0.8, 0.2, 0.6, 0.4, 0.4, 0.5);
        path.cubic_to(0.3, 0.6, 0.35, 0.7, 0.5, 0.8);
        path.cubic_to(0.6, 0.9, 0.55, 1.0, 0.4, 1.0);
        path.line_to(0.0, 1.0);

        // Scale to height
        let mut result = RevolveMesh::new()
            .with_axis(1)
            .with_segments(segments)
            .generate(&path);

        result.scale(height);
        result
    }

    /// Create a wine glass shape.
    pub fn wine_glass(height: f32, segments: u32) -> MeshResult {
        let mut path = Path2D::new();

        // Glass profile
        path.move_to(0.0, 0.0);
        path.line_to(0.3, 0.0); // Base
        path.line_to(0.3, 0.02);
        path.line_to(0.05, 0.05); // Stem start
        path.line_to(0.05, 0.4); // Stem
        path.cubic_to(0.05, 0.45, 0.1, 0.5, 0.3, 0.55); // Bowl curve
        path.cubic_to(0.4, 0.6, 0.45, 0.8, 0.4, 1.0); // Bowl top
        path.line_to(0.35, 1.0); // Rim
        path.cubic_to(0.4, 0.82, 0.35, 0.62, 0.25, 0.57);
        path.cubic_to(0.1, 0.52, 0.08, 0.48, 0.08, 0.4);
        path.line_to(0.08, 0.08);
        path.line_to(0.0, 0.05);

        let mut result = RevolveMesh::new()
            .with_axis(1)
            .with_segments(segments)
            .generate(&path);

        result.scale(height);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_revolve_circle() {
        let path = Path2D::circle(2.0, 0.0, 0.5);
        let mesh = revolve_path(&path, 1, 360.0, 16, 8);

        assert!(mesh.vertex_count() > 0);
        assert!(mesh.triangle_count() > 0);
    }

    #[test]
    fn test_sphere() {
        let mesh = shapes::sphere(1.0, 16, 8);
        assert!(mesh.vertex_count() > 0);
    }

    #[test]
    fn test_torus() {
        let mesh = shapes::torus(2.0, 0.5, 24, 12);
        assert!(mesh.vertex_count() > 0);
    }
}
