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

//! Point lights.
//!
//! Omnidirectional light sources with distance attenuation.

use nalgebra::{Point3, Vector3};

/// Point light (omnidirectional).
#[derive(Debug, Clone)]
pub struct PointLight {
    /// Light position in world space.
    pub position: Point3<f64>,
    /// Light color.
    pub color: Vector3<f64>,
    /// Light intensity (luminous power in lumens for PBR).
    pub intensity: f64,
    /// Light radius (for soft shadows and area approximation).
    pub radius: f64,
    /// Maximum range (for optimization).
    pub range: f64,
    /// Enable shadows.
    pub cast_shadows: bool,
    /// Shadow bias.
    pub shadow_bias: f64,
    /// Attenuation: constant factor.
    pub attenuation_constant: f64,
    /// Attenuation: linear factor.
    pub attenuation_linear: f64,
    /// Attenuation: quadratic factor.
    pub attenuation_quadratic: f64,
}

impl PointLight {
    /// Create a new point light.
    pub fn new(position: Point3<f64>, color: Vector3<f64>, intensity: f64) -> Self {
        Self {
            position,
            color,
            intensity,
            radius: 0.1,
            range: 100.0,
            cast_shadows: true,
            shadow_bias: 0.001,
            attenuation_constant: 1.0,
            attenuation_linear: 0.09,
            attenuation_quadratic: 0.032,
        }
    }

    /// Create with physical falloff (inverse square).
    pub fn with_physical_falloff(
        position: Point3<f64>,
        color: Vector3<f64>,
        intensity: f64,
    ) -> Self {
        Self {
            position,
            color,
            intensity,
            radius: 0.1,
            range: 100.0,
            cast_shadows: true,
            shadow_bias: 0.001,
            attenuation_constant: 0.0,
            attenuation_linear: 0.0,
            attenuation_quadratic: 1.0,
        }
    }

    /// Get direction from point to light.
    pub fn direction_to_light(&self, point: Point3<f64>) -> Vector3<f64> {
        (self.position - point).normalize()
    }

    /// Get distance from point to light.
    pub fn distance(&self, point: Point3<f64>) -> f64 {
        (self.position - point).magnitude()
    }

    /// Compute attenuation at a given distance.
    pub fn attenuation(&self, distance: f64) -> f64 {
        if distance > self.range {
            return 0.0;
        }

        let att = self.attenuation_constant
            + self.attenuation_linear * distance
            + self.attenuation_quadratic * distance * distance;

        if att > 0.0 {
            1.0 / att
        } else {
            0.0
        }
    }

    /// Get radiance at a point.
    pub fn radiance(&self, point: Point3<f64>) -> Vector3<f64> {
        let dist = self.distance(point);
        self.color * self.intensity * self.attenuation(dist)
    }

    /// Sample light position for soft shadows.
    pub fn sample_position<R: rand::Rng>(&self, rng: &mut R) -> Point3<f64> {
        if self.radius <= 0.0 {
            return self.position;
        }

        // Sample uniformly on sphere surface
        let u = rng.random::<f64>();
        let v = rng.random::<f64>();

        let theta = 2.0 * std::f64::consts::PI * u;
        let phi = (2.0 * v - 1.0).acos();

        let offset = Vector3::new(
            self.radius * phi.sin() * theta.cos(),
            self.radius * phi.sin() * theta.sin(),
            self.radius * phi.cos(),
        );

        self.position + offset
    }

    /// Compute shadow ray from point to light.
    pub fn shadow_ray(&self, point: Point3<f64>) -> (Point3<f64>, Vector3<f64>, f64) {
        let dir = self.direction_to_light(point);
        let dist = self.distance(point);
        let origin = point + dir * self.shadow_bias;
        (origin, dir, dist - self.shadow_bias * 2.0)
    }

    /// Check if point is in range.
    pub fn in_range(&self, point: Point3<f64>) -> bool {
        self.distance(point) <= self.range
    }

    /// Set physical attenuation with range.
    pub fn set_physical_range(&mut self, range: f64) {
        self.range = range;
        self.attenuation_constant = 0.0;
        self.attenuation_linear = 0.0;
        self.attenuation_quadratic = 1.0;
    }

    /// Calculate range from desired minimum intensity.
    pub fn calculate_range(intensity: f64, min_intensity: f64) -> f64 {
        (intensity / min_intensity).sqrt()
    }
}

impl Default for PointLight {
    fn default() -> Self {
        Self::new(
            Point3::new(0.0, 3.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
            10.0,
        )
    }
}

/// Point light for shadow mapping (6 cube faces).
#[derive(Debug, Clone)]
pub struct PointLightShadow {
    /// The light.
    pub light: PointLight,
    /// View-projection matrices for each cube face.
    pub face_matrices: [nalgebra::Matrix4<f64>; 6],
    /// Shadow map near plane.
    pub near: f64,
    /// Shadow map far plane.
    pub far: f64,
}

impl PointLightShadow {
    /// Create shadow data for a point light.
    pub fn new(light: PointLight, near: f64, far: f64) -> Self {
        let proj = nalgebra::Matrix4::new_perspective(1.0, std::f64::consts::FRAC_PI_2, near, far);

        let face_dirs = [
            (Vector3::x(), -Vector3::y()),  // +X
            (-Vector3::x(), -Vector3::y()), // -X
            (Vector3::y(), Vector3::z()),   // +Y
            (-Vector3::y(), -Vector3::z()), // -Y
            (Vector3::z(), -Vector3::y()),  // +Z
            (-Vector3::z(), -Vector3::y()), // -Z
        ];

        let mut face_matrices = [nalgebra::Matrix4::identity(); 6];

        for (i, (dir, up)) in face_dirs.iter().enumerate() {
            let target = light.position + *dir;
            let view = nalgebra::Matrix4::look_at_rh(&light.position, &target, up);
            face_matrices[i] = proj * view;
        }

        Self {
            light,
            face_matrices,
            near,
            far,
        }
    }

    /// Get view-projection matrix for a face.
    pub fn face_matrix(&self, face: usize) -> &nalgebra::Matrix4<f64> {
        &self.face_matrices[face.min(5)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_light() {
        let light = PointLight::new(
            Point3::new(0.0, 5.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
            100.0,
        );

        let point = Point3::new(0.0, 0.0, 0.0);
        let dir = light.direction_to_light(point);

        assert!((dir.y - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_attenuation() {
        let light =
            PointLight::with_physical_falloff(Point3::origin(), Vector3::new(1.0, 1.0, 1.0), 100.0);

        let att_1 = light.attenuation(1.0);
        let att_2 = light.attenuation(2.0);

        // Inverse square law: at 2x distance, 1/4 intensity
        assert!((att_2 - att_1 / 4.0).abs() < 1e-10);
    }

    #[test]
    fn test_range() {
        let light = PointLight::new(Point3::origin(), Vector3::new(1.0, 1.0, 1.0), 100.0);

        assert!(light.in_range(Point3::new(50.0, 0.0, 0.0)));
        assert!(!light.in_range(Point3::new(150.0, 0.0, 0.0)));
    }
}
