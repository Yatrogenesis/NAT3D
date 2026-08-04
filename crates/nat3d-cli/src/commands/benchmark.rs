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

//! Benchmark command for performance testing.

use anyhow::Result;
use std::time::Instant;
use nat3d_core::geometry::mesh::Mesh;
use nat3d_render::raytracing::pathtracer::{PathTracer, PathTracerConfig, Material};
use nat3d_render::raytracing::bvh::{Bvh, BvhParams, Triangle};
use nalgebra::{Point3, Vector3};

/// Run benchmark suite.
pub fn run_benchmark(suite: &str) -> Result<()> {
    println!("NAT3D Benchmark Suite");
    println!("=====================\n");

    match suite {
        "all" => {
            run_geometry_benchmark()?;
            run_modifier_benchmark()?;
            run_raytracing_benchmark()?;
        }
        "geometry" => run_geometry_benchmark()?,
        "modifiers" => run_modifier_benchmark()?,
        "raytracing" => run_raytracing_benchmark()?,
        _ => anyhow::bail!("Unknown benchmark suite: {}", suite),
    }

    Ok(())
}

/// Benchmark geometry primitive generation.
fn run_geometry_benchmark() -> Result<()> {
    println!("Geometry Primitives Benchmark");
    println!("-----------------------------");

    // Cube generation
    let start = Instant::now();
    for _ in 0..1000 {
        let _ = Mesh::cube(1.0);
    }
    let duration = start.elapsed();
    println!("Cube (1K):       {:?} ({:.0} ops/sec)", duration, 1000.0 / duration.as_secs_f64());

    // Sphere generation
    let start = Instant::now();
    for _ in 0..1000 {
        let _ = Mesh::sphere(1.0);
    }
    let duration = start.elapsed();
    println!("Sphere (1K):     {:?} ({:.0} ops/sec)", duration, 1000.0 / duration.as_secs_f64());

    // Cylinder generation
    let start = Instant::now();
    for _ in 0..1000 {
        let _ = Mesh::cylinder(1.0, 2.0);
    }
    let duration = start.elapsed();
    println!("Cylinder (1K):   {:?} ({:.0} ops/sec)\n", duration, 1000.0 / duration.as_secs_f64());

    Ok(())
}

/// Benchmark modifier stack performance.
fn run_modifier_benchmark() -> Result<()> {
    println!("Modifier Stack Benchmark");
    println!("------------------------");

    let mesh = Mesh::sphere(1.0);

    // Mesh operations benchmark (simple iteration)
    let start = Instant::now();
    for _ in 0..100 {
        let _ = mesh.vertex_count();
        let _ = mesh.face_count();
    }
    let duration = start.elapsed();
    println!("Mesh ops (100):  {:?} ({:.0} ops/sec)\n", duration, 100.0 / duration.as_secs_f64());

    Ok(())
}

/// Benchmark ray tracing performance.
fn run_raytracing_benchmark() -> Result<()> {
    println!("Ray Tracing Benchmark");
    println!("---------------------");

    // Build test scene
    let mut triangles = Vec::new();
    let mesh = Mesh::sphere(1.0);

    // Extract positions
    let positions = mesh.positions();

    for face in &mesh.faces {
        if face.vertices.len() >= 3 {
            let v0 = positions[face.vertices[0]];
            let v1 = positions[face.vertices[1]];
            let v2 = positions[face.vertices[2]];
            triangles.push(Triangle::new(
                Point3::new(v0.x, v0.y, v0.z),
                Point3::new(v1.x, v1.y, v1.z),
                Point3::new(v2.x, v2.y, v2.z),
                0,
            ));
        }
    }

    // BVH construction
    let start = Instant::now();
    let bvh = Bvh::build(triangles.clone(), BvhParams::default());
    let build_time = start.elapsed();
    println!("BVH Build:       {:?} ({} triangles)", build_time, triangles.len());

    // Ray intersection
    let materials = vec![Material::diffuse(0.8, 0.8, 0.8)];
    let config = PathTracerConfig {
        max_bounces: 3,
        samples_per_pixel: 1,
        ..Default::default()
    };
    let path_tracer = PathTracer::new(bvh, materials, config.clone());

    let start = Instant::now();
    let mut intersections = 0;
    for y in 0..128 {
        for x in 0..128 {
            let u = x as f64 / 128.0;
            let v = y as f64 / 128.0;
            let origin = Point3::new(0.0, 0.0, 5.0);
            let direction = Vector3::new(
                (u - 0.5) * 2.0,
                (v - 0.5) * 2.0,
                -1.0,
            ).normalize();

            if path_tracer.bvh.intersect_any(&nat3d_render::raytracing::ray::Ray::new(origin, direction)) {
                intersections += 1;
            }
        }
    }
    let ray_time = start.elapsed();
    let total_rays = 128 * 128;
    println!("Ray Intersect:   {:?} ({} rays, {:.0} Mrays/sec)",
             ray_time, total_rays, total_rays as f64 / ray_time.as_secs_f64() / 1_000_000.0);
    println!("Hit rate:        {:.1}%\n", intersections as f64 / total_rays as f64 * 100.0);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_geometry_benchmark() {
        assert!(run_geometry_benchmark().is_ok());
    }

    #[test]
    fn test_modifier_benchmark() {
        assert!(run_modifier_benchmark().is_ok());
    }

    #[test]
    fn test_raytracing_benchmark() {
        assert!(run_raytracing_benchmark().is_ok());
    }

    #[test]
    fn test_benchmark_suites() {
        assert!(run_benchmark("geometry").is_ok());
        assert!(run_benchmark("modifiers").is_ok());
        assert!(run_benchmark("raytracing").is_ok());
        assert!(run_benchmark("unknown").is_err());
    }
}
