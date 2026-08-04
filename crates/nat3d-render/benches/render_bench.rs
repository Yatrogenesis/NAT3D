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

//! Rendering benchmarks for NAT3D.

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_frustum_culling(c: &mut Criterion) {
    c.bench_function("frustum_aabb_test", |b| {
        b.iter(|| {
            // Placeholder for frustum culling
            let box_min = black_box([0.0f32, 0.0, 0.0]);
            let box_max = black_box([1.0f32, 1.0, 1.0]);
            let frustum_near = black_box(-1.0f32);
            let frustum_far = black_box(100.0f32);

            box_max[2] >= frustum_near && box_min[2] <= frustum_far
        })
    });
}

fn bench_vertex_transform(c: &mut Criterion) {
    use nalgebra::{Matrix4, Vector4};

    c.bench_function("vertex_mvp_transform", |b| {
        let mvp = Matrix4::<f32>::identity();
        let vertex = Vector4::new(1.0f32, 2.0, 3.0, 1.0);

        b.iter(|| black_box(&mvp) * black_box(&vertex))
    });
}

criterion_group!(benches, bench_frustum_culling, bench_vertex_transform);
criterion_main!(benches);
