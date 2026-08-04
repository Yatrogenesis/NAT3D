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

//! Math benchmarks for NAT3D.

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_interpolation(c: &mut Criterion) {
    c.bench_function("lerp_f32", |b| {
        b.iter(|| {
            let a = black_box(0.0f32);
            let b_val = black_box(1.0f32);
            let t = black_box(0.5f32);
            a + (b_val - a) * t
        })
    });
}

fn bench_matrix_ops(c: &mut Criterion) {
    use nalgebra::Matrix4;

    let m1 = Matrix4::<f32>::identity();
    let m2 = Matrix4::<f32>::identity();

    c.bench_function("matrix4_multiply", |b| {
        b.iter(|| black_box(&m1) * black_box(&m2))
    });
}

criterion_group!(benches, bench_interpolation, bench_matrix_ops);
criterion_main!(benches);
