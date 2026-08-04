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

//! Extrusion operations for sketch-to-mesh.

use super::path::Path2D;
use super::MeshResult;

/// Extrude a 2D path into a 3D mesh.
pub fn extrude_path(path: &Path2D, depth: f32, resolution: u32, cap: bool) -> MeshResult {
    let mut mesh = MeshResult::new();
    let points = path.tesselate(resolution);

    if points.len() < 2 {
        return mesh;
    }

    let half_depth = depth / 2.0;

    // Create front and back vertices
    let mut front_indices = Vec::new();
    let mut back_indices = Vec::new();

    for p in &points {
        // Front face (z = -half_depth)
        let idx = mesh.add_vertex([p.x, p.y, -half_depth], [0.0, 0.0, -1.0], [p.x, p.y]);
        front_indices.push(idx);

        // Back face (z = +half_depth)
        let idx = mesh.add_vertex([p.x, p.y, half_depth], [0.0, 0.0, 1.0], [p.x, p.y]);
        back_indices.push(idx);
    }

    // Create side faces
    let n = points.len();
    for i in 0..n {
        let next = (i + 1) % n;

        let p0 = &points[i];
        let p1 = &points[next];

        // Calculate side normal
        let dx = p1.x - p0.x;
        let dy = p1.y - p0.y;
        let len = (dx * dx + dy * dy).sqrt();
        let normal = if len > 0.0 {
            [dy / len, -dx / len, 0.0]
        } else {
            [1.0, 0.0, 0.0]
        };

        // Side quad vertices
        let i0 = mesh.add_vertex([p0.x, p0.y, -half_depth], normal, [0.0, 0.0]);
        let i1 = mesh.add_vertex([p1.x, p1.y, -half_depth], normal, [1.0, 0.0]);
        let i2 = mesh.add_vertex([p1.x, p1.y, half_depth], normal, [1.0, 1.0]);
        let i3 = mesh.add_vertex([p0.x, p0.y, half_depth], normal, [0.0, 1.0]);

        mesh.add_quad(i0, i1, i2, i3);
    }

    // Cap the ends if closed path
    if cap && path.is_closed() && points.len() >= 3 {
        // Triangulate front cap (simple fan)
        triangulate_cap(&mut mesh, &front_indices, [0.0, 0.0, -1.0], false);
        // Triangulate back cap
        triangulate_cap(&mut mesh, &back_indices, [0.0, 0.0, 1.0], true);
    }

    mesh
}

/// Symmetric extrude (both directions from center).
pub fn symmetric_extrude_path(path: &Path2D, depth: f32, resolution: u32, cap: bool) -> MeshResult {
    // Same as regular extrude but centered
    extrude_path(path, depth, resolution, cap)
}

/// Extrude along a path/spine.
pub fn extrude_along_path(
    profile: &Path2D,
    spine: &Path2D,
    resolution: u32,
    twist: f32,
    scale_profile: bool,
) -> MeshResult {
    let mut mesh = MeshResult::new();

    let profile_points = profile.tesselate(resolution);
    let spine_points = spine.tesselate(resolution);

    if profile_points.len() < 2 || spine_points.len() < 2 {
        return mesh;
    }

    let spine_len = spine_points.len();
    let profile_len = profile_points.len();

    // Create rings along spine
    let mut rings: Vec<Vec<u32>> = Vec::new();

    for (si, sp) in spine_points.iter().enumerate() {
        let t = si as f32 / (spine_len - 1) as f32;
        let twist_angle = twist * t * std::f32::consts::PI / 180.0;
        let scale = if scale_profile { 1.0 - t * 0.5 } else { 1.0 };

        let mut ring = Vec::new();

        for pp in &profile_points {
            // Rotate profile point by twist
            let cos_t = twist_angle.cos();
            let sin_t = twist_angle.sin();
            let rx = pp.x * cos_t - pp.y * sin_t;
            let ry = pp.x * sin_t + pp.y * cos_t;

            // Scale and position
            let x = rx * scale + sp.x;
            let y = ry * scale + sp.y;
            let z = t * 10.0; // Simple z mapping

            let idx = mesh.add_vertex([x, y, z], [0.0, 0.0, 1.0], [t, 0.0]);
            ring.push(idx);
        }

        rings.push(ring);
    }

    // Connect rings
    for ri in 0..rings.len() - 1 {
        let ring0 = &rings[ri];
        let ring1 = &rings[ri + 1];

        for pi in 0..profile_len {
            let next_pi = (pi + 1) % profile_len;

            mesh.add_quad(ring0[pi], ring0[next_pi], ring1[next_pi], ring1[pi]);
        }
    }

    mesh.recalculate_normals();
    mesh
}

/// Extrude mesh generator.
pub struct ExtrudeMesh {
    pub depth: f32,
    pub resolution: u32,
    pub cap_start: bool,
    pub cap_end: bool,
    pub taper: f32,
    pub twist: f32,
}

impl Default for ExtrudeMesh {
    fn default() -> Self {
        Self {
            depth: 1.0,
            resolution: 32,
            cap_start: true,
            cap_end: true,
            taper: 0.0,
            twist: 0.0,
        }
    }
}

impl ExtrudeMesh {
    /// Create new extrude mesh generator.
    pub fn new(depth: f32) -> Self {
        Self {
            depth,
            ..Default::default()
        }
    }

    /// Set taper (0 = no taper, 1 = point).
    pub fn with_taper(mut self, taper: f32) -> Self {
        self.taper = taper.clamp(0.0, 1.0);
        self
    }

    /// Set twist in degrees.
    pub fn with_twist(mut self, twist: f32) -> Self {
        self.twist = twist;
        self
    }

    /// Generate mesh from path.
    pub fn generate(&self, path: &Path2D) -> MeshResult {
        if self.taper.abs() < 0.001 && self.twist.abs() < 0.001 {
            // Simple extrude
            extrude_path(
                path,
                self.depth,
                self.resolution,
                self.cap_start && self.cap_end,
            )
        } else {
            // Tapered/twisted extrude
            self.generate_tapered(path)
        }
    }

    fn generate_tapered(&self, path: &Path2D) -> MeshResult {
        let mut mesh = MeshResult::new();
        let points = path.tesselate(self.resolution);

        if points.len() < 2 {
            return mesh;
        }

        let segments = 16; // Z segments
        let half_depth = self.depth / 2.0;

        // Create rings
        let mut rings: Vec<Vec<u32>> = Vec::new();

        for si in 0..=segments {
            let t = si as f32 / segments as f32;
            let z = -half_depth + t * self.depth;
            let scale = 1.0 - self.taper * t;
            let twist_angle = self.twist * t * std::f32::consts::PI / 180.0;

            let mut ring = Vec::new();

            for p in &points {
                let cos_t = twist_angle.cos();
                let sin_t = twist_angle.sin();
                let rx = p.x * cos_t - p.y * sin_t;
                let ry = p.x * sin_t + p.y * cos_t;

                let x = rx * scale;
                let y = ry * scale;

                let idx = mesh.add_vertex([x, y, z], [0.0, 0.0, 1.0], [t, 0.0]);
                ring.push(idx);
            }

            rings.push(ring);
        }

        // Connect rings
        let n = points.len();
        for ri in 0..segments {
            let ring0 = &rings[ri];
            let ring1 = &rings[ri + 1];

            for pi in 0..n {
                let next_pi = (pi + 1) % n;
                mesh.add_quad(ring0[pi], ring0[next_pi], ring1[next_pi], ring1[pi]);
            }
        }

        // Caps
        if self.cap_start && path.is_closed() {
            triangulate_cap(&mut mesh, &rings[0], [0.0, 0.0, -1.0], false);
        }
        if self.cap_end && path.is_closed() && self.taper < 0.99 {
            triangulate_cap(&mut mesh, &rings[segments], [0.0, 0.0, 1.0], true);
        }

        mesh.recalculate_normals();
        mesh
    }
}

/// Triangulate a cap using ear clipping (simplified fan).
fn triangulate_cap(mesh: &mut MeshResult, indices: &[u32], _normal: [f32; 3], reverse: bool) {
    if indices.len() < 3 {
        return;
    }

    // Simple fan triangulation (works for convex shapes)
    let n = indices.len();
    for i in 1..n - 1 {
        if reverse {
            mesh.add_triangle(indices[0], indices[i + 1], indices[i]);
        } else {
            mesh.add_triangle(indices[0], indices[i], indices[i + 1]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extrude_rectangle() {
        let path = Path2D::rectangle(-0.5, -0.5, 1.0, 1.0);
        let mesh = extrude_path(&path, 1.0, 1, true);

        assert!(mesh.vertex_count() > 0);
        assert!(mesh.triangle_count() > 0);
    }

    #[test]
    fn test_extrude_circle() {
        let path = Path2D::circle(0.0, 0.0, 1.0);
        let mesh = extrude_path(&path, 2.0, 16, true);

        assert!(mesh.vertex_count() > 0);
    }
}
