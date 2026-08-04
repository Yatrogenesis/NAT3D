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

//! Directional lights.
//!
//! Represents infinitely distant light sources like the sun.

use nalgebra::{Point3, Vector3};

/// Directional light (sun-like).
#[derive(Debug, Clone)]
pub struct DirectionalLight {
    /// Light direction (normalized, pointing toward light source).
    pub direction: Vector3<f64>,
    /// Light color.
    pub color: Vector3<f64>,
    /// Light intensity.
    pub intensity: f64,
    /// Enable shadows.
    pub cast_shadows: bool,
    /// Shadow bias.
    pub shadow_bias: f64,
    /// Soft shadow angle (for PCSS).
    pub angular_diameter: f64,
}

impl DirectionalLight {
    /// Create a new directional light.
    pub fn new(direction: Vector3<f64>, color: Vector3<f64>, intensity: f64) -> Self {
        Self {
            direction: direction.normalize(),
            color,
            intensity,
            cast_shadows: true,
            shadow_bias: 0.001,
            angular_diameter: 0.00935, // Sun's angular diameter in radians
        }
    }

    /// Create a sun light with default settings.
    pub fn sun() -> Self {
        Self::new(
            Vector3::new(-0.5, -1.0, -0.5).normalize(),
            Vector3::new(1.0, 0.98, 0.95),
            1.0,
        )
    }

    /// Get radiance at a point (same everywhere for directional light).
    pub fn radiance(&self) -> Vector3<f64> {
        self.color * self.intensity
    }

    /// Get direction to light from a point (same everywhere).
    pub fn direction_to_light(&self) -> Vector3<f64> {
        -self.direction
    }

    /// Sample light direction for soft shadows.
    pub fn sample_direction<R: rand::Rng>(&self, rng: &mut R) -> Vector3<f64> {
        if self.angular_diameter <= 0.0 {
            return self.direction_to_light();
        }

        // Create orthonormal basis around light direction
        let w = self.direction_to_light();
        let u = if w.x.abs() > 0.9 {
            Vector3::new(0.0, 1.0, 0.0).cross(&w).normalize()
        } else {
            Vector3::new(1.0, 0.0, 0.0).cross(&w).normalize()
        };
        let v = w.cross(&u);

        // Sample within angular diameter
        let r = rng.random::<f64>().sqrt() * (self.angular_diameter / 2.0).tan();
        let theta = rng.random::<f64>() * std::f64::consts::TAU;

        let offset = u * r * theta.cos() + v * r * theta.sin();
        (w + offset).normalize()
    }

    /// Compute shadow ray.
    pub fn shadow_ray(&self, point: Point3<f64>) -> (Point3<f64>, Vector3<f64>, f64) {
        let origin = point + self.direction_to_light() * self.shadow_bias;
        (origin, self.direction_to_light(), f64::INFINITY)
    }

    /// Set light direction from Euler angles (azimuth, elevation).
    pub fn set_direction_from_angles(&mut self, azimuth: f64, elevation: f64) {
        let cos_elev = elevation.cos();
        self.direction = Vector3::new(
            cos_elev * azimuth.sin(),
            elevation.sin(),
            cos_elev * azimuth.cos(),
        )
        .normalize();
    }
}

impl Default for DirectionalLight {
    fn default() -> Self {
        Self::sun()
    }
}

/// Cascade data for cascaded shadow maps.
#[derive(Debug, Clone)]
pub struct CascadeData {
    /// View-projection matrix for this cascade.
    pub view_projection: nalgebra::Matrix4<f64>,
    /// Near plane distance.
    pub near: f64,
    /// Far plane distance.
    pub far: f64,
    /// Cascade bounds in light space.
    pub bounds_min: Point3<f64>,
    /// Cascade bounds in light space.
    pub bounds_max: Point3<f64>,
}

/// Compute cascades for cascaded shadow maps.
pub fn compute_cascades(
    light: &DirectionalLight,
    camera_view: &nalgebra::Matrix4<f64>,
    camera_proj: &nalgebra::Matrix4<f64>,
    cascade_splits: &[f64],
    near: f64,
    far: f64,
) -> Vec<CascadeData> {
    let mut cascades = Vec::with_capacity(cascade_splits.len() + 1);

    let mut prev_split = near;

    for &split in cascade_splits.iter().chain(std::iter::once(&far)) {
        let cascade = compute_cascade(light, camera_view, camera_proj, prev_split, split);
        cascades.push(cascade);
        prev_split = split;
    }

    cascades
}

fn compute_cascade(
    light: &DirectionalLight,
    _camera_view: &nalgebra::Matrix4<f64>,
    _camera_proj: &nalgebra::Matrix4<f64>,
    near: f64,
    far: f64,
) -> CascadeData {
    // Compute light-space view matrix
    let light_dir = light.direction;
    let light_up = if light_dir.y.abs() > 0.99 {
        Vector3::new(1.0, 0.0, 0.0)
    } else {
        Vector3::new(0.0, 1.0, 0.0)
    };

    let light_right = light_dir.cross(&light_up).normalize();
    let light_up = light_right.cross(&light_dir).normalize();

    let view = nalgebra::Matrix4::look_at_rh(
        &Point3::origin(),
        &Point3::new(light_dir.x, light_dir.y, light_dir.z),
        &light_up,
    );

    // Simplified cascade bounds
    let size = (far - near) * 2.0;
    let proj = nalgebra::Matrix4::new_orthographic(-size, size, -size, size, -size, size);

    CascadeData {
        view_projection: proj * view,
        near,
        far,
        bounds_min: Point3::new(-size, -size, -size),
        bounds_max: Point3::new(size, size, size),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_directional_light() {
        let light = DirectionalLight::sun();
        let radiance = light.radiance();

        assert!(radiance.x > 0.0);
        assert!(radiance.y > 0.0);
        assert!(radiance.z > 0.0);
    }

    #[test]
    fn test_direction_normalization() {
        let light = DirectionalLight::new(
            Vector3::new(10.0, 20.0, 30.0),
            Vector3::new(1.0, 1.0, 1.0),
            1.0,
        );

        assert!((light.direction.magnitude() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_direction_to_light() {
        let light = DirectionalLight::new(
            Vector3::new(0.0, -1.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
            1.0,
        );

        let dir = light.direction_to_light();
        assert!((dir.y - 1.0).abs() < 1e-10);
    }
}
