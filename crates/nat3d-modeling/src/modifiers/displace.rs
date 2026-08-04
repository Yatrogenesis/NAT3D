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

//! Displace modifier.
//!
//! Displaces vertices along normals using noise or texture.

use nalgebra::{Point3, Vector3};
use std::any::Any;
use super::stack::{Modifier, ModifierMesh};

/// Noise type for displacement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoiseType {
    /// Perlin noise.
    Perlin,
    /// Simplex noise.
    Simplex,
    /// Turbulence (fractional Brownian motion).
    Turbulence,
    /// Voronoi cells.
    Voronoi,
}

/// Displacement direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplaceDirection {
    /// Along vertex normals.
    Normal,
    /// Along X axis.
    X,
    /// Along Y axis.
    Y,
    /// Along Z axis.
    Z,
    /// Custom direction vector.
    Custom,
}

/// Displace modifier.
#[derive(Debug, Clone)]
pub struct DisplaceModifier {
    /// Modifier name.
    pub name: String,
    /// Whether enabled.
    pub enabled: bool,
    /// Displacement strength.
    pub strength: f64,
    /// Noise type.
    pub noise_type: NoiseType,
    /// Noise frequency/scale.
    pub frequency: f64,
    /// Number of octaves for turbulence.
    pub octaves: usize,
    /// Random seed.
    pub seed: u32,
    /// Displacement direction.
    pub direction: DisplaceDirection,
    /// Custom direction vector (used when direction = Custom).
    pub custom_direction: Vector3<f64>,
    /// Use vertex groups for weighting.
    pub vertex_group: Option<String>,
    /// Midlevel (0.5 = centered, 0.0 = all positive, 1.0 = all negative).
    pub midlevel: f64,
}

impl Default for DisplaceModifier {
    fn default() -> Self {
        Self {
            name: "Displace".to_string(),
            enabled: true,
            strength: 0.1,
            noise_type: NoiseType::Perlin,
            frequency: 1.0,
            octaves: 4,
            seed: 0,
            direction: DisplaceDirection::Normal,
            custom_direction: Vector3::y(),
            vertex_group: None,
            midlevel: 0.5,
        }
    }
}

impl DisplaceModifier {
    /// Create new displace modifier.
    pub fn new(strength: f64) -> Self {
        Self {
            strength,
            ..Default::default()
        }
    }

    /// Simple hash function for noise generation.
    fn hash(&self, x: i32, y: i32, z: i32) -> f64 {
        let n = x.wrapping_mul(374761393)
            .wrapping_add(y.wrapping_mul(668265263))
            .wrapping_add(z.wrapping_mul(1274126177))
            .wrapping_add(self.seed as i32);

        let n = (n ^ (n >> 13)).wrapping_mul(1274126177);
        let h = n ^ (n >> 16);

        (h as f64 / i32::MAX as f64).abs()
    }

    /// Smooth interpolation.
    fn smoothstep(&self, t: f64) -> f64 {
        t * t * (3.0 - 2.0 * t)
    }

    /// Linear interpolation.
    fn lerp(&self, a: f64, b: f64, t: f64) -> f64 {
        a + (b - a) * t
    }

    /// Perlin noise implementation.
    fn perlin_noise(&self, p: Point3<f64>) -> f64 {
        let x = p.x * self.frequency;
        let y = p.y * self.frequency;
        let z = p.z * self.frequency;

        let xi = x.floor() as i32;
        let yi = y.floor() as i32;
        let zi = z.floor() as i32;

        let xf = x - x.floor();
        let yf = y - y.floor();
        let zf = z - z.floor();

        let u = self.smoothstep(xf);
        let v = self.smoothstep(yf);
        let w = self.smoothstep(zf);

        // Sample corners
        let c000 = self.hash(xi, yi, zi);
        let c100 = self.hash(xi + 1, yi, zi);
        let c010 = self.hash(xi, yi + 1, zi);
        let c110 = self.hash(xi + 1, yi + 1, zi);
        let c001 = self.hash(xi, yi, zi + 1);
        let c101 = self.hash(xi + 1, yi, zi + 1);
        let c011 = self.hash(xi, yi + 1, zi + 1);
        let c111 = self.hash(xi + 1, yi + 1, zi + 1);

        // Trilinear interpolation
        let x00 = self.lerp(c000, c100, u);
        let x10 = self.lerp(c010, c110, u);
        let x01 = self.lerp(c001, c101, u);
        let x11 = self.lerp(c011, c111, u);

        let y0 = self.lerp(x00, x10, v);
        let y1 = self.lerp(x01, x11, v);

        self.lerp(y0, y1, w)
    }

    /// Turbulence (fractal noise).
    fn turbulence(&self, p: Point3<f64>) -> f64 {
        let mut sum = 0.0;
        let mut amplitude = 1.0;
        let mut frequency = 1.0;

        for _ in 0..self.octaves {
            let scaled = Point3::new(
                p.x * frequency,
                p.y * frequency,
                p.z * frequency,
            );
            sum += self.perlin_noise(scaled) * amplitude;
            amplitude *= 0.5;
            frequency *= 2.0;
        }

        sum
    }

    /// Voronoi noise (cellular).
    fn voronoi_noise(&self, p: Point3<f64>) -> f64 {
        let xi = (p.x * self.frequency).floor() as i32;
        let yi = (p.y * self.frequency).floor() as i32;
        let zi = (p.z * self.frequency).floor() as i32;

        let mut min_dist = f64::MAX;

        // Check neighboring cells
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let cx = xi + dx;
                    let cy = yi + dy;
                    let cz = zi + dz;

                    // Random point in cell
                    let px = cx as f64 + self.hash(cx, cy, cz);
                    let py = cy as f64 + self.hash(cy, cz, cx);
                    let pz = cz as f64 + self.hash(cz, cx, cy);

                    let dx = (p.x * self.frequency) - px;
                    let dy = (p.y * self.frequency) - py;
                    let dz = (p.z * self.frequency) - pz;

                    let dist = (dx * dx + dy * dy + dz * dz).sqrt();
                    min_dist = min_dist.min(dist);
                }
            }
        }

        min_dist
    }

    /// Sample noise at position.
    fn sample_noise(&self, p: Point3<f64>) -> f64 {
        let value = match self.noise_type {
            NoiseType::Perlin => self.perlin_noise(p),
            NoiseType::Simplex => self.perlin_noise(p), // Simplified
            NoiseType::Turbulence => self.turbulence(p),
            NoiseType::Voronoi => self.voronoi_noise(p),
        };

        // Apply midlevel
        (value - self.midlevel) * 2.0
    }

    /// Get displacement direction for a vertex.
    fn get_direction(&self, mesh: &ModifierMesh, vertex_idx: usize) -> Vector3<f64> {
        match self.direction {
            DisplaceDirection::Normal => {
                if vertex_idx < mesh.normals.len() {
                    mesh.normals[vertex_idx]
                } else {
                    Vector3::y()
                }
            }
            DisplaceDirection::X => Vector3::x(),
            DisplaceDirection::Y => Vector3::y(),
            DisplaceDirection::Z => Vector3::z(),
            DisplaceDirection::Custom => self.custom_direction.normalize(),
        }
    }

    /// Get vertex weight from vertex group.
    fn get_vertex_weight(&self, mesh: &ModifierMesh, vertex_idx: usize) -> f64 {
        if let Some(ref group_name) = self.vertex_group {
            if let Some(weights) = mesh.vertex_groups.get(group_name) {
                for &(vi, weight) in weights {
                    if vi == vertex_idx {
                        return weight;
                    }
                }
            }
        }
        1.0
    }
}

impl Modifier for DisplaceModifier {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_id(&self) -> &'static str {
        "DisplaceModifier"
    }

    fn apply(&self, mesh: &ModifierMesh) -> ModifierMesh {
        if mesh.positions.is_empty() || self.strength.abs() < 1e-10 {
            return mesh.clone();
        }

        let mut result = mesh.clone();

        // Displace each vertex
        for i in 0..result.positions.len() {
            let pos = result.positions[i];
            let noise = self.sample_noise(pos);
            let direction = self.get_direction(mesh, i);
            let weight = self.get_vertex_weight(mesh, i);

            let displacement = direction * (noise * self.strength * weight);
            result.positions[i] = pos + displacement;
        }

        // Recompute normals after displacement
        result.compute_normals();
        result
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn clone_box(&self) -> Box<dyn Modifier> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_displace_basic() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.5, 1.0, 0.0),
            ],
            vec![vec![0, 1, 2]],
        );

        let modifier = DisplaceModifier::new(0.1);
        let result = modifier.apply(&mesh);

        assert_eq!(result.positions.len(), 3);
        // Vertices should be displaced (not exact original positions)
        assert!(result.positions.iter().any(|p| (*p - mesh.positions[0]).magnitude() > 1e-6));
    }

    #[test]
    fn test_displace_noise_types() {
        let mesh = ModifierMesh::from_geometry(
            vec![Point3::new(1.0, 1.0, 1.0)],
            vec![],
        );

        // Test Perlin
        let mut modifier = DisplaceModifier::new(0.1);
        modifier.noise_type = NoiseType::Perlin;
        let result = modifier.apply(&mesh);
        assert_eq!(result.positions.len(), 1);

        // Test Turbulence
        modifier.noise_type = NoiseType::Turbulence;
        let result = modifier.apply(&mesh);
        assert_eq!(result.positions.len(), 1);

        // Test Voronoi
        modifier.noise_type = NoiseType::Voronoi;
        let result = modifier.apply(&mesh);
        assert_eq!(result.positions.len(), 1);
    }

    #[test]
    fn test_displace_directions() {
        let mesh = ModifierMesh::from_geometry(
            vec![Point3::new(0.0, 0.0, 0.0)],
            vec![],
        );

        // Test X direction
        let mut modifier = DisplaceModifier::new(1.0);
        modifier.direction = DisplaceDirection::X;
        modifier.frequency = 0.1; // Low frequency for predictable result
        let result = modifier.apply(&mesh);
        assert_eq!(result.positions.len(), 1);

        // Test Y direction
        modifier.direction = DisplaceDirection::Y;
        let result = modifier.apply(&mesh);
        assert_eq!(result.positions.len(), 1);

        // Test Z direction
        modifier.direction = DisplaceDirection::Z;
        let result = modifier.apply(&mesh);
        assert_eq!(result.positions.len(), 1);
    }
}
