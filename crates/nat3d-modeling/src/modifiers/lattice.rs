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

//! Lattice (Free Form Deformation) modifier.
//!
//! Implements FFD using a 3D control point lattice.

use nalgebra::{Point3, Vector3};
use std::any::Any;
use super::stack::{Modifier, ModifierMesh};

/// Lattice resolution configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LatticeResolution {
    /// U resolution.
    pub u: usize,
    /// V resolution.
    pub v: usize,
    /// W resolution.
    pub w: usize,
}

impl Default for LatticeResolution {
    fn default() -> Self {
        Self { u: 2, v: 2, w: 2 }
    }
}

impl LatticeResolution {
    /// Create new resolution.
    pub fn new(u: usize, v: usize, w: usize) -> Self {
        Self {
            u: u.max(2),
            v: v.max(2),
            w: w.max(2),
        }
    }

    /// Get total control point count.
    pub fn count(&self) -> usize {
        self.u * self.v * self.w
    }

    /// Get linear index from 3D coordinates.
    pub fn index(&self, i: usize, j: usize, k: usize) -> usize {
        k * (self.u * self.v) + j * self.u + i
    }
}

/// Falloff function for deformation influence.
#[derive(Debug, Clone, Copy, PartialEq)]
#[derive(Default)]
pub enum Falloff {
    /// Linear falloff.
    #[default]
    Linear,
    /// Smooth falloff (smoothstep).
    Smooth,
    /// Spherical falloff.
    Sphere,
    /// No falloff (constant).
    Constant,
}


/// Lattice modifier.
#[derive(Debug, Clone)]
pub struct LatticeModifier {
    /// Modifier name.
    pub name: String,
    /// Whether enabled.
    pub enabled: bool,
    /// Lattice resolution.
    pub resolution: LatticeResolution,
    /// Control point displacements (offset from grid position).
    pub control_points: Vec<Vector3<f64>>,
    /// Falloff type.
    pub falloff: Falloff,
    /// Bounds of the lattice (min, max).
    pub bounds: (Point3<f64>, Point3<f64>),
}

impl Default for LatticeModifier {
    fn default() -> Self {
        let resolution = LatticeResolution::default();
        Self {
            name: "Lattice".to_string(),
            enabled: true,
            resolution,
            control_points: vec![Vector3::zeros(); resolution.count()],
            falloff: Falloff::default(),
            bounds: (Point3::new(-1.0, -1.0, -1.0), Point3::new(1.0, 1.0, 1.0)),
        }
    }
}

impl LatticeModifier {
    /// Create new lattice modifier with resolution.
    pub fn new(u: usize, v: usize, w: usize) -> Self {
        let resolution = LatticeResolution::new(u, v, w);
        Self {
            resolution,
            control_points: vec![Vector3::zeros(); resolution.count()],
            ..Default::default()
        }
    }

    /// Create lattice from mesh bounds.
    pub fn from_mesh(mesh: &ModifierMesh, u: usize, v: usize, w: usize) -> Self {
        let bounds = mesh.bounds();
        let resolution = LatticeResolution::new(u, v, w);

        Self {
            resolution,
            control_points: vec![Vector3::zeros(); resolution.count()],
            bounds,
            ..Default::default()
        }
    }

    /// Set control point displacement.
    pub fn set_control_point(&mut self, i: usize, j: usize, k: usize, displacement: Vector3<f64>) {
        let idx = self.resolution.index(i, j, k);
        if idx < self.control_points.len() {
            self.control_points[idx] = displacement;
        }
    }

    /// Get control point position (grid position + displacement).
    fn get_control_point_pos(&self, i: usize, j: usize, k: usize) -> Point3<f64> {
        let idx = self.resolution.index(i, j, k);

        let (min, max) = self.bounds;

        // Base grid position
        let u_step = if self.resolution.u > 1 {
            (max.x - min.x) / (self.resolution.u - 1) as f64
        } else {
            0.0
        };

        let v_step = if self.resolution.v > 1 {
            (max.y - min.y) / (self.resolution.v - 1) as f64
        } else {
            0.0
        };

        let w_step = if self.resolution.w > 1 {
            (max.z - min.z) / (self.resolution.w - 1) as f64
        } else {
            0.0
        };

        let base = Point3::new(
            min.x + i as f64 * u_step,
            min.y + j as f64 * v_step,
            min.z + k as f64 * w_step,
        );

        // Add displacement
        Point3::from(base.coords + self.control_points[idx])
    }

    /// Convert world position to UVW coordinates (0-1 range).
    fn world_to_uvw(&self, pos: Point3<f64>) -> (f64, f64, f64) {
        let (min, max) = self.bounds;

        let u = if (max.x - min.x).abs() > 1e-10 {
            (pos.x - min.x) / (max.x - min.x)
        } else {
            0.0
        };

        let v = if (max.y - min.y).abs() > 1e-10 {
            (pos.y - min.y) / (max.y - min.y)
        } else {
            0.0
        };

        let w = if (max.z - min.z).abs() > 1e-10 {
            (pos.z - min.z) / (max.z - min.z)
        } else {
            0.0
        };

        (u, v, w)
    }

    /// Bernstein basis function.
    fn bernstein(&self, i: usize, n: usize, t: f64) -> f64 {
        fn binomial(n: usize, k: usize) -> f64 {
            if k > n {
                return 0.0;
            }
            let mut result = 1.0;
            for i in 0..k {
                result *= (n - i) as f64 / (i + 1) as f64;
            }
            result
        }

        binomial(n - 1, i) * t.powi(i as i32) * (1.0 - t).powi((n - 1 - i) as i32)
    }

    /// Evaluate deformed position using trivariate Bernstein basis.
    fn evaluate_deformation(&self, pos: Point3<f64>) -> Point3<f64> {
        let (u, v, w) = self.world_to_uvw(pos);

        // Clamp to bounds
        let u = u.clamp(0.0, 1.0);
        let v = v.clamp(0.0, 1.0);
        let w = w.clamp(0.0, 1.0);

        let mut result = Vector3::zeros();

        // Trivariate Bernstein polynomial
        for k in 0..self.resolution.w {
            let bw = self.bernstein(k, self.resolution.w, w);
            for j in 0..self.resolution.v {
                let bv = self.bernstein(j, self.resolution.v, v);
                for i in 0..self.resolution.u {
                    let bu = self.bernstein(i, self.resolution.u, u);

                    let control_pos = self.get_control_point_pos(i, j, k);
                    let weight = bu * bv * bw;

                    result += control_pos.coords * weight;
                }
            }
        }

        // Apply falloff
        let influence = match self.falloff {
            Falloff::Linear => 1.0,
            Falloff::Smooth => {
                // Smoothstep based on distance from bounds center
                let center = Point3::from((self.bounds.0.coords + self.bounds.1.coords) / 2.0);
                let dist = (pos - center).magnitude();
                let max_dist = (self.bounds.1 - self.bounds.0).magnitude() / 2.0;
                if max_dist > 0.0 {
                    let t = (dist / max_dist).clamp(0.0, 1.0);
                    let smooth = t * t * (3.0 - 2.0 * t);
                    1.0 - smooth
                } else {
                    1.0
                }
            }
            Falloff::Sphere => {
                let center = Point3::from((self.bounds.0.coords + self.bounds.1.coords) / 2.0);
                let dist = (pos - center).magnitude();
                let radius = (self.bounds.1 - self.bounds.0).magnitude() / 2.0;
                if radius > 0.0 {
                    (1.0 - (dist / radius).min(1.0)).max(0.0)
                } else {
                    1.0
                }
            }
            Falloff::Constant => 1.0,
        };

        let deformed = Point3::from(result);
        Point3::from(pos.coords + (deformed.coords - pos.coords) * influence)
    }
}

impl Modifier for LatticeModifier {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_id(&self) -> &'static str {
        "LatticeModifier"
    }

    fn apply(&self, mesh: &ModifierMesh) -> ModifierMesh {
        let mut result = mesh.clone();

        // Deform each vertex
        for pos in &mut result.positions {
            *pos = self.evaluate_deformation(*pos);
        }

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
    fn test_lattice_basic() {
        let mesh = ModifierMesh::from_geometry(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
            ],
            vec![vec![0, 1, 2]],
        );

        let modifier = LatticeModifier::new(2, 2, 2);
        let result = modifier.apply(&mesh);

        // Mesh should be unchanged with zero displacements
        assert_eq!(result.positions.len(), mesh.positions.len());
    }

    #[test]
    fn test_lattice_deformation() {
        let mesh = ModifierMesh::from_geometry(
            vec![Point3::new(0.5, 0.5, 0.5)],
            vec![],
        );

        let mut modifier = LatticeModifier::new(2, 2, 2);
        modifier.bounds = (Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 1.0, 1.0));

        // Displace a corner control point
        modifier.set_control_point(1, 1, 1, Vector3::new(0.5, 0.5, 0.5));

        let result = modifier.apply(&mesh);

        // Center point should be affected
        assert!(result.positions[0] != mesh.positions[0]);
    }

    #[test]
    fn test_lattice_resolution() {
        let res = LatticeResolution::new(3, 4, 5);

        assert_eq!(res.u, 3);
        assert_eq!(res.v, 4);
        assert_eq!(res.w, 5);
        assert_eq!(res.count(), 60);

        let idx = res.index(1, 2, 3);
        assert_eq!(idx, 3 * 12 + 2 * 3 + 1);
    }
}
