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

//! Area lights.
//!
//! Physically-based area light sources for soft shadows and realistic illumination.

use nalgebra::{Point3, Vector3};
use rand::Rng;

/// Shape of an area light.
#[derive(Debug, Clone)]
pub enum AreaLightShape {
    /// Rectangle defined by two edge vectors.
    Rectangle {
        /// First edge vector (local x).
        edge_u: Vector3<f64>,
        /// Second edge vector (local y).
        edge_v: Vector3<f64>,
    },
    /// Disk with radius.
    /// Disk with radius.
    Disk {
        /// Radius of the disk.
        radius: f64,
    },
    /// Sphere with radius.
    Sphere {
        /// Radius of the sphere.
        radius: f64,
    },
}

/// Area light for soft shadows and realistic illumination.
#[derive(Debug, Clone)]
pub struct AreaLight {
    /// Light center position.
    pub position: Point3<f64>,
    /// Light normal direction.
    pub normal: Vector3<f64>,
    /// Light color.
    pub color: Vector3<f64>,
    /// Light intensity (radiant flux in watts).
    pub intensity: f64,
    /// Light shape.
    pub shape: AreaLightShape,
    /// Two-sided emission.
    pub two_sided: bool,
    /// Enable shadows.
    pub cast_shadows: bool,
}

impl AreaLight {
    /// Create a rectangular area light.
    pub fn rectangle(
        position: Point3<f64>,
        edge_u: Vector3<f64>,
        edge_v: Vector3<f64>,
        color: Vector3<f64>,
        intensity: f64,
    ) -> Self {
        let normal = edge_u.cross(&edge_v).normalize();
        Self {
            position,
            normal,
            color,
            intensity,
            shape: AreaLightShape::Rectangle { edge_u, edge_v },
            two_sided: false,
            cast_shadows: true,
        }
    }

    /// Create a disk area light.
    pub fn disk(
        position: Point3<f64>,
        normal: Vector3<f64>,
        radius: f64,
        color: Vector3<f64>,
        intensity: f64,
    ) -> Self {
        Self {
            position,
            normal: normal.normalize(),
            color,
            intensity,
            shape: AreaLightShape::Disk { radius },
            two_sided: false,
            cast_shadows: true,
        }
    }

    /// Create a spherical area light.
    pub fn sphere(position: Point3<f64>, radius: f64, color: Vector3<f64>, intensity: f64) -> Self {
        Self {
            position,
            normal: Vector3::new(0.0, 1.0, 0.0), // Not used for spheres
            color,
            intensity,
            shape: AreaLightShape::Sphere { radius },
            two_sided: true, // Spheres always emit in all directions
            cast_shadows: true,
        }
    }

    /// Get the surface area of the light.
    pub fn area(&self) -> f64 {
        match &self.shape {
            AreaLightShape::Rectangle { edge_u, edge_v } => edge_u.cross(edge_v).magnitude(),
            AreaLightShape::Disk { radius } => std::f64::consts::PI * radius * radius,
            AreaLightShape::Sphere { radius } => 4.0 * std::f64::consts::PI * radius * radius,
        }
    }

    /// Get radiance (power per unit area per steradian).
    pub fn radiance(&self) -> Vector3<f64> {
        self.color * self.intensity / (self.area() * std::f64::consts::PI)
    }

    /// Sample a point on the light surface.
    pub fn sample_point<R: Rng>(&self, rng: &mut R) -> (Point3<f64>, Vector3<f64>, f64) {
        match &self.shape {
            AreaLightShape::Rectangle { edge_u, edge_v } => {
                let u = rng.random::<f64>() - 0.5;
                let v = rng.random::<f64>() - 0.5;
                let point = self.position + edge_u * u + edge_v * v;
                let pdf = 1.0 / self.area();
                (point, self.normal, pdf)
            }
            AreaLightShape::Disk { radius } => {
                let r = rng.random::<f64>().sqrt() * radius;
                let theta = rng.random::<f64>() * std::f64::consts::TAU;

                // Build local frame
                let (tangent, bitangent) = build_tangent_frame(self.normal);

                let offset = tangent * (r * theta.cos()) + bitangent * (r * theta.sin());
                let point = self.position + offset;
                let pdf = 1.0 / self.area();
                (point, self.normal, pdf)
            }
            AreaLightShape::Sphere { radius } => {
                let u = rng.random::<f64>();
                let v = rng.random::<f64>();

                let theta = 2.0 * std::f64::consts::PI * u;
                let phi = (2.0 * v - 1.0).acos();

                let normal =
                    Vector3::new(phi.sin() * theta.cos(), phi.sin() * theta.sin(), phi.cos());

                let point = self.position + normal * *radius;
                let pdf = 1.0 / self.area();
                (point, normal, pdf)
            }
        }
    }

    /// Sample the light for direct lighting calculation.
    pub fn sample<R: Rng>(
        &self,
        point: Point3<f64>,
        rng: &mut R,
    ) -> Option<(Vector3<f64>, Vector3<f64>, f64, f64)> {
        let (light_point, light_normal, pdf_area) = self.sample_point(rng);

        let to_light = light_point - point;
        let distance = to_light.magnitude();
        let direction = to_light / distance;

        // Check if light is facing the point
        let cos_light = (-direction).dot(&light_normal);
        if !self.two_sided && cos_light <= 0.0 {
            return None;
        }

        // Convert area PDF to solid angle PDF
        let pdf = pdf_area * distance * distance / cos_light.abs();

        Some((direction, self.radiance(), pdf, distance))
    }

    /// Get the PDF for a given direction.
    pub fn pdf(&self, point: Point3<f64>, _direction: Vector3<f64>) -> f64 {
        // Would need to implement proper intersection test
        // For now, approximate
        let to_center = self.position - point;
        let distance = to_center.magnitude();

        // Approximate solid angle
        let solid_angle = self.area() / (distance * distance);

        if solid_angle > 0.0 {
            1.0 / solid_angle
        } else {
            0.0
        }
    }
}

/// Build a tangent frame from a normal.
fn build_tangent_frame(normal: Vector3<f64>) -> (Vector3<f64>, Vector3<f64>) {
    let tangent = if normal.x.abs() > 0.9 {
        Vector3::new(0.0, 1.0, 0.0)
    } else {
        Vector3::new(1.0, 0.0, 0.0)
    };

    let bitangent = normal.cross(&tangent).normalize();
    let tangent = bitangent.cross(&normal).normalize();

    (tangent, bitangent)
}

/// Linearly Transformed Cosines for area light shading.
#[derive(Debug, Clone)]
pub struct LtcMatrix {
    /// LTC matrix for GGX.
    pub m: nalgebra::Matrix3<f64>,
    /// Inverse of the matrix.
    pub m_inv: nalgebra::Matrix3<f64>,
    /// Amplitude (for energy conservation).
    pub amplitude: f64,
}

impl LtcMatrix {
    /// Create LTC matrix for given roughness and view angle.
    pub fn new(roughness: f64, cos_theta: f64) -> Self {
        // Simplified LTC - full implementation would use precomputed tables
        let a = roughness * roughness;

        // Approximate LTC matrix elements
        let m11 = 1.0 / (1.0 + a * (1.0 - cos_theta));
        let m22 = 1.0;
        let m13 = -a * (1.0 - cos_theta) * m11;

        let m = nalgebra::Matrix3::new(m11, 0.0, m13, 0.0, m22, 0.0, 0.0, 0.0, 1.0);

        let m_inv = m.try_inverse().unwrap_or(nalgebra::Matrix3::identity());

        Self {
            m,
            m_inv,
            amplitude: m11 * m22,
        }
    }

    /// Transform a direction.
    pub fn transform(&self, direction: Vector3<f64>) -> Vector3<f64> {
        (self.m * direction).normalize()
    }

    /// Inverse transform a direction.
    pub fn inverse_transform(&self, direction: Vector3<f64>) -> Vector3<f64> {
        (self.m_inv * direction).normalize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rectangle_area() {
        let light = AreaLight::rectangle(
            Point3::origin(),
            Vector3::new(2.0, 0.0, 0.0),
            Vector3::new(0.0, 2.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
            100.0,
        );

        assert!((light.area() - 4.0).abs() < 1e-10);
    }

    #[test]
    fn test_disk_area() {
        let light = AreaLight::disk(
            Point3::origin(),
            Vector3::new(0.0, 1.0, 0.0),
            1.0,
            Vector3::new(1.0, 1.0, 1.0),
            100.0,
        );

        assert!((light.area() - std::f64::consts::PI).abs() < 1e-10);
    }

    #[test]
    fn test_sphere_area() {
        let light = AreaLight::sphere(Point3::origin(), 1.0, Vector3::new(1.0, 1.0, 1.0), 100.0);

        assert!((light.area() - 4.0 * std::f64::consts::PI).abs() < 1e-10);
    }

    #[test]
    fn test_sample_point() {
        let light = AreaLight::rectangle(
            Point3::origin(),
            Vector3::new(2.0, 0.0, 0.0),
            Vector3::new(0.0, 2.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
            100.0,
        );

        let mut rng = rand::rng();
        let (point, normal, pdf) = light.sample_point(&mut rng);

        // Point should be within rectangle bounds
        assert!(point.x.abs() <= 1.0);
        assert!(point.y.abs() <= 1.0);

        // Normal should point up
        assert!((normal.z - 1.0).abs() < 1e-10);

        // PDF should be 1/area
        assert!((pdf - 0.25).abs() < 1e-10);
    }
}
