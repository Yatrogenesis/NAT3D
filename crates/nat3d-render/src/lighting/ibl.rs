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

//! Image-Based Lighting.
//!
//! Environment mapping and IBL for realistic ambient lighting.

use nalgebra::{Point2, Vector3};

/// Environment map for IBL.
#[derive(Debug, Clone)]
pub struct EnvironmentMap {
    /// HDR pixel data (RGB).
    pub pixels: Vec<Vector3<f64>>,
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// Intensity multiplier.
    pub intensity: f64,
    /// Rotation around Y axis (radians).
    pub rotation: f64,
}

impl EnvironmentMap {
    /// Create a new environment map.
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            pixels: vec![Vector3::zeros(); width * height],
            width,
            height,
            intensity: 1.0,
            rotation: 0.0,
        }
    }

    /// Create a solid color environment.
    pub fn solid_color(color: Vector3<f64>) -> Self {
        let mut env = Self::new(1, 1);
        env.pixels[0] = color;
        env
    }

    /// Create a gradient sky environment.
    pub fn gradient_sky(horizon: Vector3<f64>, zenith: Vector3<f64>, ground: Vector3<f64>) -> Self {
        let width = 256;
        let height = 128;
        let mut env = Self::new(width, height);

        for y in 0..height {
            let v = y as f64 / height as f64;
            let elevation = (1.0 - v) * std::f64::consts::PI - std::f64::consts::FRAC_PI_2;

            let color = if elevation > 0.0 {
                // Above horizon: blend horizon to zenith
                let t = elevation / std::f64::consts::FRAC_PI_2;
                horizon * (1.0 - t) + zenith * t
            } else {
                // Below horizon: blend horizon to ground
                let t = (-elevation) / std::f64::consts::FRAC_PI_2;
                horizon * (1.0 - t) + ground * t
            };

            for x in 0..width {
                env.pixels[y * width + x] = color;
            }
        }

        env
    }

    /// Sample environment at a direction.
    pub fn sample(&self, direction: Vector3<f64>) -> Vector3<f64> {
        let uv = direction_to_equirectangular(direction, self.rotation);
        self.sample_uv(uv) * self.intensity
    }

    /// Sample at UV coordinates (bilinear).
    pub fn sample_uv(&self, uv: Point2<f64>) -> Vector3<f64> {
        let u = uv.x.rem_euclid(1.0);
        let v = uv.y.clamp(0.0, 1.0);

        let x = u * (self.width - 1) as f64;
        let y = v * (self.height - 1) as f64;

        let x0 = x.floor() as usize;
        let y0 = y.floor() as usize;
        let x1 = (x0 + 1).min(self.width - 1);
        let y1 = (y0 + 1).min(self.height - 1);

        let fx = x.fract();
        let fy = y.fract();

        let c00 = self.pixels[y0 * self.width + x0];
        let c10 = self.pixels[y0 * self.width + x1];
        let c01 = self.pixels[y1 * self.width + x0];
        let c11 = self.pixels[y1 * self.width + x1];

        let c0 = c00 * (1.0 - fx) + c10 * fx;
        let c1 = c01 * (1.0 - fx) + c11 * fx;

        c0 * (1.0 - fy) + c1 * fy
    }

    /// Get pixel at coordinates.
    pub fn get_pixel(&self, x: usize, y: usize) -> Vector3<f64> {
        if x < self.width && y < self.height {
            self.pixels[y * self.width + x]
        } else {
            Vector3::zeros()
        }
    }

    /// Set pixel at coordinates.
    pub fn set_pixel(&mut self, x: usize, y: usize, color: Vector3<f64>) {
        if x < self.width && y < self.height {
            self.pixels[y * self.width + x] = color;
        }
    }
}

/// Convert direction to equirectangular UV coordinates.
pub fn direction_to_equirectangular(direction: Vector3<f64>, rotation: f64) -> Point2<f64> {
    let d = direction.normalize();

    let theta = d.z.atan2(d.x) + rotation;
    let phi = d.y.asin();

    let u = theta / std::f64::consts::TAU + 0.5;
    let v = 0.5 - phi / std::f64::consts::PI;

    Point2::new(u, v)
}

/// Convert equirectangular UV to direction.
pub fn equirectangular_to_direction(uv: Point2<f64>, rotation: f64) -> Vector3<f64> {
    let theta = (uv.x - 0.5) * std::f64::consts::TAU - rotation;
    let phi = (0.5 - uv.y) * std::f64::consts::PI;

    let cos_phi = phi.cos();

    Vector3::new(cos_phi * theta.cos(), phi.sin(), cos_phi * theta.sin())
}

/// Diffuse irradiance map (preconvolved).
#[derive(Debug, Clone)]
pub struct IrradianceMap {
    /// Irradiance data.
    pub data: Vec<Vector3<f64>>,
    /// Width.
    pub width: usize,
    /// Height.
    pub height: usize,
}

impl IrradianceMap {
    /// Create a new irradiance map.
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            data: vec![Vector3::zeros(); width * height],
            width,
            height,
        }
    }

    /// Compute irradiance map from environment map.
    pub fn from_environment(env: &EnvironmentMap, width: usize, height: usize) -> Self {
        let mut irr = Self::new(width, height);

        let sample_count = 64;

        for y in 0..height {
            for x in 0..width {
                let u = (x as f64 + 0.5) / width as f64;
                let v = (y as f64 + 0.5) / height as f64;

                let normal = equirectangular_to_direction(Point2::new(u, v), 0.0);
                let irradiance = compute_irradiance(env, normal, sample_count);

                irr.data[y * width + x] = irradiance;
            }
        }

        irr
    }

    /// Sample irradiance at a direction.
    pub fn sample(&self, direction: Vector3<f64>) -> Vector3<f64> {
        let uv = direction_to_equirectangular(direction, 0.0);

        let x = (uv.x * self.width as f64) as usize % self.width;
        let y = (uv.y * (self.height - 1) as f64) as usize;

        self.data[y * self.width + x]
    }
}

/// Compute irradiance for a normal direction.
fn compute_irradiance(env: &EnvironmentMap, normal: Vector3<f64>, samples: usize) -> Vector3<f64> {
    // Build tangent frame
    let up = if normal.y.abs() < 0.999 {
        Vector3::new(0.0, 1.0, 0.0)
    } else {
        Vector3::new(1.0, 0.0, 0.0)
    };

    let tangent = up.cross(&normal).normalize();
    let bitangent = normal.cross(&tangent);

    let mut irradiance = Vector3::zeros();
    let delta = std::f64::consts::PI / samples as f64;

    for i in 0..samples {
        let phi = i as f64 * delta;
        for j in 0..samples {
            let theta = j as f64 * delta * 2.0;

            let sample_dir =
                Vector3::new(phi.sin() * theta.cos(), phi.sin() * theta.sin(), phi.cos());

            let world_dir =
                tangent * sample_dir.x + bitangent * sample_dir.y + normal * sample_dir.z;

            let radiance = env.sample(world_dir);
            irradiance += radiance * phi.cos() * phi.sin();
        }
    }

    irradiance * std::f64::consts::PI / (samples * samples) as f64
}

/// Prefiltered environment map for specular IBL.
#[derive(Debug, Clone)]
pub struct PrefilteredEnvMap {
    /// Mip levels (roughness levels).
    pub mips: Vec<EnvironmentMap>,
    /// Maximum roughness level.
    pub max_mip_level: usize,
}

impl PrefilteredEnvMap {
    /// Create a new prefiltered map.
    pub fn new(base_width: usize, mip_levels: usize) -> Self {
        let mut mips = Vec::with_capacity(mip_levels);

        let mut width = base_width;
        let mut height = base_width / 2;

        for _ in 0..mip_levels {
            mips.push(EnvironmentMap::new(width, height));
            width = (width / 2).max(1);
            height = (height / 2).max(1);
        }

        Self {
            mips,
            max_mip_level: mip_levels - 1,
        }
    }

    /// Sample prefiltered map at roughness level.
    pub fn sample(&self, direction: Vector3<f64>, roughness: f64) -> Vector3<f64> {
        let mip = roughness * self.max_mip_level as f64;
        let mip0 = mip.floor() as usize;
        let mip1 = (mip0 + 1).min(self.max_mip_level);
        let frac = mip.fract();

        let c0 = self.mips[mip0].sample(direction);
        let c1 = self.mips[mip1].sample(direction);

        c0 * (1.0 - frac) + c1 * frac
    }
}

/// BRDF integration LUT for split-sum approximation.
#[derive(Debug, Clone)]
pub struct BrdfLut {
    /// LUT data (scale, bias pairs).
    pub data: Vec<Point2<f64>>,
    /// LUT size.
    pub size: usize,
}

impl BrdfLut {
    /// Create a new BRDF LUT.
    pub fn new(size: usize) -> Self {
        let mut lut = Self {
            data: vec![Point2::origin(); size * size],
            size,
        };
        lut.generate();
        lut
    }

    /// Generate BRDF integration LUT.
    fn generate(&mut self) {
        for y in 0..self.size {
            for x in 0..self.size {
                let n_dot_v = (x as f64 + 0.5) / self.size as f64;
                let roughness = (y as f64 + 0.5) / self.size as f64;

                let (scale, bias) = integrate_brdf(n_dot_v, roughness, 1024);
                self.data[y * self.size + x] = Point2::new(scale, bias);
            }
        }
    }

    /// Sample LUT.
    pub fn sample(&self, n_dot_v: f64, roughness: f64) -> Point2<f64> {
        let x = (n_dot_v * (self.size - 1) as f64) as usize;
        let y = (roughness * (self.size - 1) as f64) as usize;
        self.data[y * self.size + x]
    }
}

/// Integrate BRDF for a given NdotV and roughness.
fn integrate_brdf(n_dot_v: f64, roughness: f64, samples: usize) -> (f64, f64) {
    let v = Vector3::new((1.0 - n_dot_v * n_dot_v).sqrt(), 0.0, n_dot_v);
    let n = Vector3::new(0.0, 0.0, 1.0);

    let mut a = 0.0;
    let mut b = 0.0;

    let _a2 = roughness * roughness * roughness * roughness;

    for i in 0..samples {
        let xi = hammersley(i, samples);
        let h = importance_sample_ggx(xi, n, roughness);
        let l = (2.0 * v.dot(&h) * h - v).normalize();

        let n_dot_l = l.z.max(0.0);
        let n_dot_h = h.z.max(0.0);
        let v_dot_h = v.dot(&h).max(0.0);

        if n_dot_l > 0.0 {
            let g = geometry_smith(n, v, l, roughness);
            let g_vis = g * v_dot_h / (n_dot_h * n_dot_v);
            let fc = (1.0 - v_dot_h).powi(5);

            a += (1.0 - fc) * g_vis;
            b += fc * g_vis;
        }
    }

    (a / samples as f64, b / samples as f64)
}

/// Hammersley low-discrepancy sequence.
fn hammersley(i: usize, n: usize) -> Point2<f64> {
    let mut bits = i as u32;
    bits = bits.rotate_right(16);
    bits = ((bits & 0x55555555) << 1) | ((bits & 0xAAAAAAAA) >> 1);
    bits = ((bits & 0x33333333) << 2) | ((bits & 0xCCCCCCCC) >> 2);
    bits = ((bits & 0x0F0F0F0F) << 4) | ((bits & 0xF0F0F0F0) >> 4);
    bits = ((bits & 0x00FF00FF) << 8) | ((bits & 0xFF00FF00) >> 8);

    Point2::new(i as f64 / n as f64, bits as f64 * 2.3283064365386963e-10)
}

/// Importance sample GGX distribution.
fn importance_sample_ggx(xi: Point2<f64>, n: Vector3<f64>, roughness: f64) -> Vector3<f64> {
    let a = roughness * roughness;

    let phi = 2.0 * std::f64::consts::PI * xi.x;
    let cos_theta = ((1.0 - xi.y) / (1.0 + (a * a - 1.0) * xi.y)).sqrt();
    let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();

    let h = Vector3::new(phi.cos() * sin_theta, phi.sin() * sin_theta, cos_theta);

    // Transform to world space
    let up = if n.z.abs() < 0.999 {
        Vector3::new(0.0, 0.0, 1.0)
    } else {
        Vector3::new(1.0, 0.0, 0.0)
    };

    let tangent = up.cross(&n).normalize();
    let bitangent = n.cross(&tangent);

    (tangent * h.x + bitangent * h.y + n * h.z).normalize()
}

/// Smith geometry function.
fn geometry_smith(n: Vector3<f64>, v: Vector3<f64>, l: Vector3<f64>, roughness: f64) -> f64 {
    geometry_schlick_ggx(n.dot(&v).max(0.0), roughness)
        * geometry_schlick_ggx(n.dot(&l).max(0.0), roughness)
}

fn geometry_schlick_ggx(n_dot_v: f64, roughness: f64) -> f64 {
    let k = roughness * roughness / 2.0;
    n_dot_v / (n_dot_v * (1.0 - k) + k)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_direction_conversion() {
        let dir = Vector3::new(1.0, 0.0, 0.0);
        let uv = direction_to_equirectangular(dir, 0.0);
        let dir2 = equirectangular_to_direction(uv, 0.0);

        assert!((dir.x - dir2.x).abs() < 1e-6);
        assert!((dir.y - dir2.y).abs() < 1e-6);
        assert!((dir.z - dir2.z).abs() < 1e-6);
    }

    #[test]
    fn test_gradient_sky() {
        let sky = EnvironmentMap::gradient_sky(
            Vector3::new(0.8, 0.9, 1.0),
            Vector3::new(0.2, 0.4, 0.8),
            Vector3::new(0.3, 0.2, 0.1),
        );

        // Sample up direction (should be closer to zenith)
        let up_color = sky.sample(Vector3::new(0.0, 1.0, 0.0));
        assert!(up_color.z > up_color.x); // More blue

        // Sample down direction (should be closer to ground)
        let down_color = sky.sample(Vector3::new(0.0, -1.0, 0.0));
        assert!(down_color.x > down_color.z); // More red/brown
    }

    #[test]
    fn test_hammersley() {
        let p = hammersley(0, 16);
        assert!((p.x - 0.0).abs() < 1e-10);

        let p = hammersley(8, 16);
        assert!((p.x - 0.5).abs() < 1e-10);
    }
}
