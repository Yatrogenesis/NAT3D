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

//! Path tracer implementation.
//!
//! Monte Carlo path tracing for physically-based rendering.

use super::bvh::{Bvh, BvhPrimitive};
use super::ray::Ray;
use nalgebra::{Point3, Vector3};
use rand::Rng;

/// Material types for path tracing.
#[derive(Debug, Clone)]
pub enum Material {
    /// Lambertian diffuse material.
    Diffuse {
        /// Diffuse albedo color.
        albedo: Vector3<f64>,
    },
    /// Metallic material.
    Metal {
        /// Metallic albedo color.
        albedo: Vector3<f64>,
        /// Surface roughness.
        roughness: f64,
    },
    /// Dielectric (glass) material.
    Dielectric {
        /// Index of refraction.
        ior: f64,
    },
    /// Emissive material.
    Emissive {
        /// Emission color profile.
        emission: Vector3<f64>,
    },
    /// PBR material.
    Pbr {
        /// Base color.
        base_color: Vector3<f64>,
        /// Metallic factor.
        metallic: f64,
        /// Roughness factor.
        roughness: f64,
        /// Emission color.
        emission: Vector3<f64>,
    },
}

impl Default for Material {
    fn default() -> Self {
        Material::Diffuse {
            albedo: Vector3::new(0.8, 0.8, 0.8),
        }
    }
}

impl Material {
    /// Create a diffuse material.
    pub fn diffuse(r: f64, g: f64, b: f64) -> Self {
        Material::Diffuse {
            albedo: Vector3::new(r, g, b),
        }
    }

    /// Create a metallic material.
    pub fn metal(r: f64, g: f64, b: f64, roughness: f64) -> Self {
        Material::Metal {
            albedo: Vector3::new(r, g, b),
            roughness: roughness.clamp(0.0, 1.0),
        }
    }

    /// Create a glass material.
    pub fn glass(ior: f64) -> Self {
        Material::Dielectric { ior }
    }

    /// Create an emissive material.
    pub fn emissive(r: f64, g: f64, b: f64) -> Self {
        Material::Emissive {
            emission: Vector3::new(r, g, b),
        }
    }

    /// Get emission color.
    pub fn emission(&self) -> Vector3<f64> {
        match self {
            Material::Emissive { emission } => *emission,
            Material::Pbr { emission, .. } => *emission,
            _ => Vector3::zeros(),
        }
    }
}

/// Path tracer configuration.
#[derive(Debug, Clone)]
pub struct PathTracerConfig {
    /// Maximum ray bounces.
    pub max_bounces: usize,
    /// Samples per pixel.
    pub samples_per_pixel: usize,
    /// Russian roulette start depth.
    pub rr_start_depth: usize,
    /// Russian roulette probability.
    pub rr_probability: f64,
    /// Enable next event estimation.
    pub next_event_estimation: bool,
    /// Background color.
    pub background: Vector3<f64>,
}

impl Default for PathTracerConfig {
    fn default() -> Self {
        Self {
            max_bounces: 10,
            samples_per_pixel: 64,
            rr_start_depth: 3,
            rr_probability: 0.9,
            next_event_estimation: true,
            background: Vector3::new(0.5, 0.7, 1.0),
        }
    }
}

/// Path tracer for rendering.
pub struct PathTracer<P: BvhPrimitive> {
    /// Scene BVH.
    pub bvh: Bvh<P>,
    /// Materials.
    pub materials: Vec<Material>,
    /// Configuration.
    pub config: PathTracerConfig,
}

impl<P: BvhPrimitive> PathTracer<P> {
    /// Create a new path tracer.
    pub fn new(bvh: Bvh<P>, materials: Vec<Material>, config: PathTracerConfig) -> Self {
        Self {
            bvh,
            materials,
            config,
        }
    }

    /// Trace a single ray.
    pub fn trace_ray<R: Rng>(&self, ray: &Ray, rng: &mut R) -> Vector3<f64> {
        self.trace_path(ray, 0, rng)
    }

    fn trace_path<R: Rng>(&self, ray: &Ray, depth: usize, rng: &mut R) -> Vector3<f64> {
        // Russian roulette termination
        if depth >= self.config.rr_start_depth
            && rng.random::<f64>() > self.config.rr_probability
        {
            return Vector3::zeros();
        }

        if depth >= self.config.max_bounces {
            return Vector3::zeros();
        }

        // Find intersection
        let Some((prim_idx, t)) = self.bvh.intersect(ray) else {
            return self.config.background;
        };

        // Get material
        let material = self.materials.get(prim_idx).cloned().unwrap_or_default();

        // Get hit point and normal (simplified - real implementation would use intersection data)
        let hit_point = ray.at(t);

        // Compute normal from primitive (placeholder)
        let normal = Vector3::new(0.0, 1.0, 0.0); // Would be computed from primitive

        // Start with emission
        let mut color = material.emission();

        // Scatter ray based on material
        match &material {
            Material::Diffuse { albedo } => {
                let scattered_dir = random_hemisphere(normal, rng);
                let scattered = Ray::new(hit_point + normal * 1e-4, scattered_dir);
                let incoming = self.trace_path(&scattered, depth + 1, rng);
                let cos_theta = scattered_dir.dot(&normal).max(0.0);
                color += albedo.component_mul(&incoming) * cos_theta * 2.0;
            }
            Material::Metal { albedo, roughness } => {
                let reflected = reflect(ray.direction, normal);
                let fuzz = random_in_unit_sphere(rng) * *roughness;
                let scattered_dir = (reflected + fuzz).normalize();
                if scattered_dir.dot(&normal) > 0.0 {
                    let scattered = Ray::new(hit_point + normal * 1e-4, scattered_dir);
                    let incoming = self.trace_path(&scattered, depth + 1, rng);
                    color += albedo.component_mul(&incoming);
                }
            }
            Material::Dielectric { ior } => {
                let (outward_normal, ni_over_nt, cosine) = if ray.direction.dot(&normal) > 0.0 {
                    (-normal, *ior, ior * ray.direction.dot(&normal))
                } else {
                    (normal, 1.0 / ior, -ray.direction.dot(&normal))
                };

                let reflect_prob = schlick(cosine, *ior);

                let scattered_dir = if rng.random::<f64>() < reflect_prob {
                    reflect(ray.direction, outward_normal)
                } else if let Some(refracted) = refract(ray.direction, outward_normal, ni_over_nt) {
                    refracted
                } else {
                    reflect(ray.direction, outward_normal)
                };

                let offset = if scattered_dir.dot(&normal) > 0.0 {
                    normal
                } else {
                    -normal
                } * 1e-4;
                let scattered = Ray::new(hit_point + offset, scattered_dir);
                let incoming = self.trace_path(&scattered, depth + 1, rng);
                color += incoming;
            }
            Material::Emissive { .. } => {
                // Already added emission above
            }
            Material::Pbr {
                base_color,
                metallic,
                roughness,
                ..
            } => {
                // Simplified PBR: blend between diffuse and metallic
                if rng.random::<f64>() < *metallic {
                    // Metallic reflection
                    let reflected = reflect(ray.direction, normal);
                    let fuzz = random_in_unit_sphere(rng) * *roughness;
                    let scattered_dir = (reflected + fuzz).normalize();
                    if scattered_dir.dot(&normal) > 0.0 {
                        let scattered = Ray::new(hit_point + normal * 1e-4, scattered_dir);
                        let incoming = self.trace_path(&scattered, depth + 1, rng);
                        color += base_color.component_mul(&incoming);
                    }
                } else {
                    // Diffuse
                    let scattered_dir = random_hemisphere(normal, rng);
                    let scattered = Ray::new(hit_point + normal * 1e-4, scattered_dir);
                    let incoming = self.trace_path(&scattered, depth + 1, rng);
                    let cos_theta = scattered_dir.dot(&normal).max(0.0);
                    color += base_color.component_mul(&incoming) * cos_theta * 2.0;
                }
            }
        }

        // Apply Russian roulette correction
        if depth >= self.config.rr_start_depth {
            color /= self.config.rr_probability;
        }

        color
    }
}

/// Reflect vector around normal.
fn reflect(v: Vector3<f64>, n: Vector3<f64>) -> Vector3<f64> {
    v - 2.0 * v.dot(&n) * n
}

/// Refract vector through surface. Returns None for total internal reflection.
fn refract(v: Vector3<f64>, n: Vector3<f64>, ni_over_nt: f64) -> Option<Vector3<f64>> {
    let uv = v.normalize();
    let dt = uv.dot(&n);
    let discriminant = 1.0 - ni_over_nt * ni_over_nt * (1.0 - dt * dt);

    if discriminant > 0.0 {
        Some(ni_over_nt * (uv - n * dt) - n * discriminant.sqrt())
    } else {
        None
    }
}

/// Schlick approximation for Fresnel.
fn schlick(cosine: f64, ior: f64) -> f64 {
    let r0 = ((1.0 - ior) / (1.0 + ior)).powi(2);
    r0 + (1.0 - r0) * (1.0 - cosine).powi(5)
}

/// Random direction in hemisphere.
fn random_hemisphere<R: Rng>(normal: Vector3<f64>, rng: &mut R) -> Vector3<f64> {
    let dir = random_in_unit_sphere(rng).normalize();
    if dir.dot(&normal) > 0.0 {
        dir
    } else {
        -dir
    }
}

/// Random point in unit sphere.
fn random_in_unit_sphere<R: Rng>(rng: &mut R) -> Vector3<f64> {
    loop {
        let p = Vector3::new(
            rng.random::<f64>() * 2.0 - 1.0,
            rng.random::<f64>() * 2.0 - 1.0,
            rng.random::<f64>() * 2.0 - 1.0,
        );
        if p.magnitude_squared() < 1.0 {
            return p;
        }
    }
}

/// Camera for generating rays.
#[derive(Debug, Clone)]
pub struct Camera {
    /// Camera position.
    pub origin: Point3<f64>,
    /// Lower left corner of image plane.
    pub lower_left: Point3<f64>,
    /// Horizontal extent.
    pub horizontal: Vector3<f64>,
    /// Vertical extent.
    pub vertical: Vector3<f64>,
    /// Lens radius for DOF.
    pub lens_radius: f64,
    /// Camera forward direction.
    pub w: Vector3<f64>,
    /// Camera right direction.
    pub u: Vector3<f64>,
    /// Camera up direction.
    pub v: Vector3<f64>,
}

impl Camera {
    /// Create a new camera.
    pub fn new(
        look_from: Point3<f64>,
        look_at: Point3<f64>,
        vup: Vector3<f64>,
        vfov: f64,
        aspect: f64,
        aperture: f64,
        focus_dist: f64,
    ) -> Self {
        let theta = vfov.to_radians();
        let half_height = (theta / 2.0).tan();
        let half_width = aspect * half_height;

        let w = (look_from - look_at).normalize();
        let u = vup.cross(&w).normalize();
        let v = w.cross(&u);

        let lower_left =
            look_from - half_width * focus_dist * u - half_height * focus_dist * v - focus_dist * w;

        Self {
            origin: look_from,
            lower_left,
            horizontal: 2.0 * half_width * focus_dist * u,
            vertical: 2.0 * half_height * focus_dist * v,
            lens_radius: aperture / 2.0,
            w,
            u,
            v,
        }
    }

    /// Generate a ray for given uv coordinates (0-1 range).
    pub fn get_ray<R: Rng>(&self, s: f64, t: f64, rng: &mut R) -> Ray {
        let rd = random_in_disk(rng) * self.lens_radius;
        let offset = self.u * rd.x + self.v * rd.y;

        let direction = self.lower_left.coords + s * self.horizontal + t * self.vertical
            - self.origin.coords
            - offset;

        Ray::new(self.origin + offset, direction)
    }
}

fn random_in_disk<R: Rng>(rng: &mut R) -> Vector3<f64> {
    loop {
        let p = Vector3::new(
            rng.random::<f64>() * 2.0 - 1.0,
            rng.random::<f64>() * 2.0 - 1.0,
            0.0,
        );
        if p.magnitude_squared() < 1.0 {
            return p;
        }
    }
}

/// Render image using path tracing.
pub fn render_image<P: BvhPrimitive + Sync, R: Rng + Clone + Send>(
    tracer: &PathTracer<P>,
    camera: &Camera,
    width: usize,
    height: usize,
    rng: &mut R,
) -> Vec<Vector3<f64>> {
    let mut image = vec![Vector3::zeros(); width * height];
    let samples = tracer.config.samples_per_pixel;

    for y in 0..height {
        for x in 0..width {
            let mut color = Vector3::zeros();

            for _ in 0..samples {
                let u = (x as f64 + rng.random::<f64>()) / width as f64;
                let v = (y as f64 + rng.random::<f64>()) / height as f64;

                let ray = camera.get_ray(u, v, rng);
                color += tracer.trace_ray(&ray, rng);
            }

            image[y * width + x] = color / samples as f64;
        }
    }

    image
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reflect() {
        let v = Vector3::new(1.0, -1.0, 0.0).normalize();
        let n = Vector3::new(0.0, 1.0, 0.0);
        let r = reflect(v, n);

        assert!((r.x - v.x).abs() < 1e-10);
        assert!((r.y + v.y).abs() < 1e-10);
    }

    #[test]
    fn test_schlick() {
        // At perpendicular incidence
        let r = schlick(1.0, 1.5);
        assert!(r < 0.1); // Low reflection

        // At grazing angle
        let r = schlick(0.0, 1.5);
        assert!((r - 1.0).abs() < 0.01); // High reflection
    }

    #[test]
    fn test_camera() {
        let camera = Camera::new(
            Point3::new(0.0, 0.0, 1.0),
            Point3::new(0.0, 0.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
            90.0,
            1.0,
            0.0,
            1.0,
        );

        let mut rng = rand::rng();
        let ray = camera.get_ray(0.5, 0.5, &mut rng);

        // Center ray should point forward
        assert!(ray.direction.z < 0.0);
    }
}
