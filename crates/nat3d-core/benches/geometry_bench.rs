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

//! Geometry benchmarks for NAT3D Core.

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_bounding_box(c: &mut Criterion) {
    use nalgebra::Point3;

    c.bench_function("bounding_box_contains", |b| {
        let min = Point3::new(0.0f32, 0.0, 0.0);
        let max = Point3::new(1.0f32, 1.0, 1.0);
        let point = Point3::new(0.5f32, 0.5, 0.5);

        b.iter(|| {
            let min = black_box(&min);
            let max = black_box(&max);
            let p = black_box(&point);
            p.x >= min.x
                && p.x <= max.x
                && p.y >= min.y
                && p.y <= max.y
                && p.z >= min.z
                && p.z <= max.z
        })
    });
}

fn bench_vertex_operations(c: &mut Criterion) {
    use nalgebra::Vector3;

    c.bench_function("vertex_normal_normalize", |b| {
        let normal = Vector3::new(1.0f32, 2.0, 3.0);
        b.iter(|| black_box(&normal).normalize())
    });
}

criterion_group!(benches, bench_bounding_box, bench_vertex_operations);
criterion_main!(benches);
