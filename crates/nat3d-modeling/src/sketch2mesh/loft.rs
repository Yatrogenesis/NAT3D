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

//! Loft operations for sketch-to-mesh.

use super::path::Path2D;
use super::MeshResult;

/// Loft between multiple 2D profiles.
pub fn loft_paths(profiles: &[Path2D], resolution: u32, closed: bool) -> MeshResult {
    let mut mesh = MeshResult::new();

    if profiles.len() < 2 {
        return mesh;
    }

    // Tesselate all profiles
    let rings: Vec<Vec<_>> = profiles.iter().map(|p| p.tesselate(resolution)).collect();

    // Ensure all rings have same point count (resample if needed)
    let target_count = rings.iter().map(|r| r.len()).max().unwrap_or(0);
    if target_count < 2 {
        return mesh;
    }

    // Create vertex rings
    let mut vertex_rings: Vec<Vec<u32>> = Vec::new();

    for (ri, ring) in rings.iter().enumerate() {
        let t = ri as f32 / (profiles.len() - 1) as f32;
        let z = t * 10.0; // Spread along Z

        let mut vertex_ring = Vec::new();

        for (pi, p) in ring.iter().enumerate() {
            let u = pi as f32 / ring.len() as f32;
            let idx = mesh.add_vertex([p.x, p.y, z], [0.0, 0.0, 1.0], [u, t]);
            vertex_ring.push(idx);
        }

        vertex_rings.push(vertex_ring);
    }

    // Connect rings
    for ri in 0..vertex_rings.len() - 1 {
        let ring0 = &vertex_rings[ri];
        let ring1 = &vertex_rings[ri + 1];

        let count0 = ring0.len();
        let count1 = ring1.len();

        // Handle different point counts with interpolation
        if count0 == count1 {
            for pi in 0..count0 {
                let next = (pi + 1) % count0;
                mesh.add_quad(ring0[pi], ring0[next], ring1[next], ring1[pi]);
            }
        } else {
            // Simple connection for different counts
            for pi in 0..count0.min(count1) {
                let next0 = (pi + 1) % count0;
                let next1 = (pi + 1) % count1;
                mesh.add_quad(ring0[pi], ring0[next0], ring1[next1], ring1[pi]);
            }
        }
    }

    // Cap ends if closed
    if closed {
        // Start cap
        triangulate_fan(&mut mesh, &vertex_rings[0], false);
        // End cap
        triangulate_fan(&mut mesh, vertex_rings.last().unwrap(), true);
    }

    mesh.recalculate_normals();
    mesh
}

/// Loft with guide rails.
pub fn loft_with_rails(profiles: &[Path2D], rails: &[Path2D], resolution: u32) -> MeshResult {
    let mut mesh = MeshResult::new();

    if profiles.len() < 2 || rails.is_empty() {
        return loft_paths(profiles, resolution, true);
    }

    // Tesselate rails to get positions along the loft
    let rail_points: Vec<Vec<_>> = rails.iter().map(|r| r.tesselate(resolution)).collect();

    // Get the number of steps from the first rail
    let steps = rail_points[0].len();
    if steps < 2 {
        return loft_paths(profiles, resolution, true);
    }

    // Interpolate profiles along rails
    let profile_points: Vec<Vec<_>> = profiles.iter().map(|p| p.tesselate(resolution)).collect();

    let points_per_profile = profile_points[0].len();

    // Create vertex grid
    let mut vertex_rings: Vec<Vec<u32>> = Vec::new();

    #[allow(clippy::needless_range_loop)]
    for si in 0..steps {
        let t = si as f32 / (steps - 1) as f32;

        // Interpolate profile
        let profile_idx = ((profiles.len() - 1) as f32 * t) as usize;
        let profile_t = (profiles.len() - 1) as f32 * t - profile_idx as f32;

        let mut ring = Vec::new();

        for pi in 0..points_per_profile {
            // Get interpolated point from profiles
            let p0 = &profile_points[profile_idx][pi % profile_points[profile_idx].len()];
            let p1 = if profile_idx + 1 < profiles.len() {
                &profile_points[profile_idx + 1][pi % profile_points[profile_idx + 1].len()]
            } else {
                p0
            };

            let x = p0.x + (p1.x - p0.x) * profile_t;
            let y = p0.y + (p1.y - p0.y) * profile_t;

            // Get position from rail
            let rail_pos = &rail_points[0][si];
            let final_x = x + rail_pos.x;
            let final_y = y;
            let final_z = rail_pos.y; // Use rail Y as Z

            let u = pi as f32 / points_per_profile as f32;
            let v = t;

            let idx = mesh.add_vertex([final_x, final_y, final_z], [0.0, 0.0, 1.0], [u, v]);
            ring.push(idx);
        }

        vertex_rings.push(ring);
    }

    // Connect rings
    for ri in 0..vertex_rings.len() - 1 {
        let ring0 = &vertex_rings[ri];
        let ring1 = &vertex_rings[ri + 1];

        for pi in 0..points_per_profile {
            let next = (pi + 1) % points_per_profile;
            mesh.add_quad(ring0[pi], ring0[next], ring1[next], ring1[pi]);
        }
    }

    mesh.recalculate_normals();
    mesh
}

fn triangulate_fan(mesh: &mut MeshResult, ring: &[u32], reverse: bool) {
    if ring.len() < 3 {
        return;
    }

    for i in 1..ring.len() - 1 {
        if reverse {
            mesh.add_triangle(ring[0], ring[i + 1], ring[i]);
        } else {
            mesh.add_triangle(ring[0], ring[i], ring[i + 1]);
        }
    }
}

/// Loft mesh generator with options.
pub struct LoftMesh {
    pub resolution: u32,
    pub closed: bool,
    pub smooth: bool,
}

impl Default for LoftMesh {
    fn default() -> Self {
        Self {
            resolution: 16,
            closed: true,
            smooth: true,
        }
    }
}

impl LoftMesh {
    /// Create new loft mesh generator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Generate loft from profiles.
    pub fn generate(&self, profiles: &[Path2D]) -> MeshResult {
        loft_paths(profiles, self.resolution, self.closed)
    }

    /// Generate loft with guide rails.
    pub fn generate_with_rails(&self, profiles: &[Path2D], rails: &[Path2D]) -> MeshResult {
        loft_with_rails(profiles, rails, self.resolution)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loft_rectangles() {
        let profiles = vec![
            Path2D::rectangle(-1.0, -1.0, 2.0, 2.0),
            Path2D::rectangle(-0.5, -0.5, 1.0, 1.0),
        ];
        let mesh = loft_paths(&profiles, 4, true);
        assert!(mesh.vertex_count() > 0);
    }

    #[test]
    fn test_loft_circle_to_square() {
        let profiles = vec![
            Path2D::circle(0.0, 0.0, 1.0),
            Path2D::rectangle(-0.5, -0.5, 1.0, 1.0),
        ];
        let mesh = loft_paths(&profiles, 16, true);
        assert!(mesh.vertex_count() > 0);
    }
}
