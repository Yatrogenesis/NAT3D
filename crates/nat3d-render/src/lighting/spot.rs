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

//! Spot lights.
//!
//! Directional lights with cone falloff for focused illumination.

use nalgebra::{Point3, Vector3};

/// Spot light with cone falloff.
#[derive(Debug, Clone)]
pub struct SpotLight {
    /// Light position.
    pub position: Point3<f64>,
    /// Light direction (normalized).
    pub direction: Vector3<f64>,
    /// Light color.
    pub color: Vector3<f64>,
    /// Light intensity.
    pub intensity: f64,
    /// Inner cone angle (full intensity, radians).
    pub inner_angle: f64,
    /// Outer cone angle (falloff to zero, radians).
    pub outer_angle: f64,
    /// Maximum range.
    pub range: f64,
    /// Enable shadows.
    pub cast_shadows: bool,
    /// Shadow bias.
    pub shadow_bias: f64,
    /// Attenuation constant.
    pub attenuation_constant: f64,
    /// Attenuation linear.
    pub attenuation_linear: f64,
    /// Attenuation quadratic.
    pub attenuation_quadratic: f64,
}

impl SpotLight {
    /// Create a new spot light.
    pub fn new(
        position: Point3<f64>,
        direction: Vector3<f64>,
        color: Vector3<f64>,
        intensity: f64,
        inner_angle: f64,
        outer_angle: f64,
    ) -> Self {
        Self {
            position,
            direction: direction.normalize(),
            color,
            intensity,
            inner_angle,
            outer_angle,
            range: 100.0,
            cast_shadows: true,
            shadow_bias: 0.001,
            attenuation_constant: 1.0,
            attenuation_linear: 0.09,
            attenuation_quadratic: 0.032,
        }
    }

    /// Create a spot light from degrees.
    pub fn from_degrees(
        position: Point3<f64>,
        direction: Vector3<f64>,
        color: Vector3<f64>,
        intensity: f64,
        inner_degrees: f64,
        outer_degrees: f64,
    ) -> Self {
        Self::new(
            position,
            direction,
            color,
            intensity,
            inner_degrees.to_radians(),
            outer_degrees.to_radians(),
        )
    }

    /// Get direction from point to light.
    pub fn direction_to_light(&self, point: Point3<f64>) -> Vector3<f64> {
        (self.position - point).normalize()
    }

    /// Get distance from point to light.
    pub fn distance(&self, point: Point3<f64>) -> f64 {
        (self.position - point).magnitude()
    }

    /// Compute distance attenuation.
    pub fn distance_attenuation(&self, distance: f64) -> f64 {
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

    /// Compute angular attenuation (cone falloff).
    pub fn angular_attenuation(&self, point: Point3<f64>) -> f64 {
        let to_point = (point - self.position).normalize();
        let cos_angle = to_point.dot(&self.direction);
        let angle = cos_angle.acos();

        if angle < self.inner_angle {
            1.0
        } else if angle < self.outer_angle {
            // Smooth falloff between inner and outer
            let t = (angle - self.inner_angle) / (self.outer_angle - self.inner_angle);
            1.0 - smoothstep(t)
        } else {
            0.0
        }
    }

    /// Get radiance at a point.
    pub fn radiance(&self, point: Point3<f64>) -> Vector3<f64> {
        let dist = self.distance(point);
        let dist_att = self.distance_attenuation(dist);
        let angle_att = self.angular_attenuation(point);
        self.color * self.intensity * dist_att * angle_att
    }

    /// Check if point is potentially illuminated.
    pub fn potentially_illuminates(&self, point: Point3<f64>) -> bool {
        if self.distance(point) > self.range {
            return false;
        }

        let to_point = (point - self.position).normalize();
        let cos_angle = to_point.dot(&self.direction);
        cos_angle.acos() < self.outer_angle
    }

    /// Compute shadow ray.
    pub fn shadow_ray(&self, point: Point3<f64>) -> (Point3<f64>, Vector3<f64>, f64) {
        let dir = self.direction_to_light(point);
        let dist = self.distance(point);
        let origin = point + dir * self.shadow_bias;
        (origin, dir, dist - self.shadow_bias * 2.0)
    }

    /// Get view matrix for shadow mapping.
    pub fn view_matrix(&self) -> nalgebra::Matrix4<f64> {
        let up = if self.direction.y.abs() > 0.99 {
            Vector3::new(1.0, 0.0, 0.0)
        } else {
            Vector3::new(0.0, 1.0, 0.0)
        };

        nalgebra::Matrix4::look_at_rh(&self.position, &(self.position + self.direction), &up)
    }

    /// Get projection matrix for shadow mapping.
    pub fn projection_matrix(&self, near: f64) -> nalgebra::Matrix4<f64> {
        nalgebra::Matrix4::new_perspective(1.0, self.outer_angle * 2.0, near, self.range)
    }
}

impl Default for SpotLight {
    fn default() -> Self {
        Self::from_degrees(
            Point3::new(0.0, 5.0, 0.0),
            Vector3::new(0.0, -1.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
            100.0,
            30.0,
            45.0,
        )
    }
}

/// Smoothstep interpolation.
fn smoothstep(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// IES light profile for realistic light distribution.
#[derive(Debug, Clone)]
pub struct IesProfile {
    /// Vertical angles (radians).
    pub vertical_angles: Vec<f64>,
    /// Horizontal angles (radians).
    pub horizontal_angles: Vec<f64>,
    /// Candela values.
    pub candela_values: Vec<Vec<f64>>,
    /// Maximum candela value for normalization.
    pub max_candela: f64,
}

impl IesProfile {
    /// Create an empty profile.
    pub fn new() -> Self {
        Self {
            vertical_angles: Vec::new(),
            horizontal_angles: Vec::new(),
            candela_values: Vec::new(),
            max_candela: 1.0,
        }
    }

    /// Sample profile at given angles.
    pub fn sample(&self, vertical: f64, horizontal: f64) -> f64 {
        if self.candela_values.is_empty() {
            return 1.0;
        }

        // Find vertical index
        let v_idx = self.find_index(&self.vertical_angles, vertical);
        let h_idx = self.find_index(&self.horizontal_angles, horizontal);

        // Bilinear interpolation
        let v0 = v_idx.0.min(self.candela_values.len() - 1);
        let v1 = (v_idx.0 + 1).min(self.candela_values.len() - 1);

        if self.candela_values[v0].is_empty() {
            return 1.0;
        }

        let h0 = h_idx.0.min(self.candela_values[v0].len() - 1);
        let h1 = (h_idx.0 + 1).min(self.candela_values[v0].len() - 1);

        let c00 = self.candela_values[v0][h0];
        let c01 = self.candela_values[v0][h1];
        let c10 = self.candela_values[v1][h0];
        let c11 = self.candela_values[v1][h1];

        let c0 = c00 + (c01 - c00) * h_idx.1;
        let c1 = c10 + (c11 - c10) * h_idx.1;
        let c = c0 + (c1 - c0) * v_idx.1;

        c / self.max_candela
    }

    fn find_index(&self, angles: &[f64], angle: f64) -> (usize, f64) {
        if angles.is_empty() {
            return (0, 0.0);
        }

        for i in 0..angles.len() - 1 {
            if angle >= angles[i] && angle <= angles[i + 1] {
                let t = (angle - angles[i]) / (angles[i + 1] - angles[i]);
                return (i, t);
            }
        }

        (angles.len() - 1, 0.0)
    }
}

impl Default for IesProfile {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spot_light_creation() {
        let light = SpotLight::from_degrees(
            Point3::new(0.0, 5.0, 0.0),
            Vector3::new(0.0, -1.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
            100.0,
            30.0,
            45.0,
        );

        assert!((light.direction.y - (-1.0)).abs() < 1e-10);
    }

    #[test]
    fn test_cone_attenuation() {
        let light = SpotLight::from_degrees(
            Point3::new(0.0, 5.0, 0.0),
            Vector3::new(0.0, -1.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
            100.0,
            30.0,
            45.0,
        );

        // Point directly below (inside inner cone)
        let center = Point3::new(0.0, 0.0, 0.0);
        let att_center = light.angular_attenuation(center);
        assert!((att_center - 1.0).abs() < 1e-10);

        // Point outside outer cone
        let outside = Point3::new(10.0, 0.0, 0.0);
        let att_outside = light.angular_attenuation(outside);
        assert!(att_outside < 0.01);
    }

    #[test]
    fn test_potentially_illuminates() {
        let light = SpotLight::from_degrees(
            Point3::new(0.0, 5.0, 0.0),
            Vector3::new(0.0, -1.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
            100.0,
            30.0,
            45.0,
        );

        assert!(light.potentially_illuminates(Point3::new(0.0, 0.0, 0.0)));
        assert!(!light.potentially_illuminates(Point3::new(100.0, 0.0, 0.0)));
    }
}
