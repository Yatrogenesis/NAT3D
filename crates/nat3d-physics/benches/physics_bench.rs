// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Francisco Molina-Burgos, Avermex Research Division

//! Physics benchmarks for NAT3D — P6 TVCG Paper Experiments.
//!
//! Benchmarks:
//!   B1 — Tumbling dumbbell: quaternion drift & energy error (RK4 vs Symplectic Euler)
//!   B2 — Spinning top: gyroscopic precession accuracy
//!   B3 — XPBD chain: constraint residual vs iteration count
//!   B4 — Scene performance: physics step time for N rigid bodies

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use nalgebra::{Matrix3, UnitQuaternion, Vector3};
use nat3d_physics::rigid_body::dynamics::{RigidBody, RigidBodyProperties};

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Symplectic Euler integrator for angular motion — baseline comparison.
/// Matches the Bullet/Blender default (semi-implicit Euler).
fn step_symplectic_euler(
    orientation: &mut UnitQuaternion<f64>,
    angular_velocity: &mut Vector3<f64>,
    torque: Vector3<f64>,
    inv_inertia: &Matrix3<f64>,
    dt: f64,
) {
    // Update angular velocity first (explicit)
    let alpha =
        inv_inertia * (torque - angular_velocity.cross(&(*inv_inertia * *angular_velocity)));
    *angular_velocity += alpha * dt;

    // Integrate orientation with updated velocity (semi-implicit)
    let w_quat = nalgebra::Quaternion::new(
        0.0,
        angular_velocity.x,
        angular_velocity.y,
        angular_velocity.z,
    );
    let q_raw = orientation.quaternion();
    let dq = nalgebra::Quaternion::new(
        -0.5 * (w_quat.i * q_raw.i + w_quat.j * q_raw.j + w_quat.k * q_raw.k),
        0.5 * (w_quat.i * q_raw.w + w_quat.k * q_raw.j - w_quat.j * q_raw.k),
        0.5 * (w_quat.j * q_raw.w + w_quat.i * q_raw.k - w_quat.k * q_raw.i),
        0.5 * (w_quat.k * q_raw.w + w_quat.j * q_raw.i - w_quat.i * q_raw.j),
    );
    let sum = nalgebra::Quaternion::new(
        q_raw.w + dq.w * dt,
        q_raw.i + dq.i * dt,
        q_raw.j + dq.j * dt,
        q_raw.k + dq.k * dt,
    );
    *orientation = UnitQuaternion::from_quaternion(sum);
}

/// Build a dumbbell body: two unit masses separated by 2 m on the X axis.
/// Inertia tensor: I_xx = 0, I_yy = I_zz = 2 * m * r^2 = 2 kg·m²
fn dumbbell_body() -> RigidBody {
    let inertia = Matrix3::from_diagonal(&Vector3::new(0.01, 2.0, 2.0));
    let props = RigidBodyProperties::dynamic(2.0, inertia);
    let mut body = RigidBody::new(1, props);
    // Initial angular velocity: fast tumble on X, small perturbation on Y
    body.state.angular_velocity = Vector3::new(5.0, 0.1, 0.0);
    body
}

/// Build an oblate symmetric top: I1 = I2 = 1, I3 = 0.3 (oblate).
fn spinning_top_body() -> RigidBody {
    let inertia = Matrix3::from_diagonal(&Vector3::new(1.0, 1.0, 0.3));
    let props = RigidBodyProperties::dynamic(1.0, inertia);
    let mut body = RigidBody::new(2, props);
    // Fast spin around symmetry axis (Z), small tilt
    body.state.angular_velocity = Vector3::new(0.1, 0.0, 10.0);
    body
}

/// Kinetic energy from angular velocity and inertia tensor.
fn kinetic_energy_angular(inertia: &Matrix3<f64>, w: &Vector3<f64>) -> f64 {
    0.5 * w.dot(&(inertia * w))
}

// ── Benchmark B1: Tumbling Dumbbell ─────────────────────────────────────────

/// B1a — RK4 dumbbell at dt=1/60 for 10 seconds (600 steps).
fn bench_b1_rk4_dt60(c: &mut Criterion) {
    c.bench_function("B1_dumbbell_rk4_dt60", |b| {
        b.iter(|| {
            let mut body = dumbbell_body();
            let gravity = Vector3::zeros(); // torque-free precession
            let dt = 1.0 / 60.0;
            let steps = 600;
            for _ in 0..steps {
                body.integrate(black_box(dt), gravity);
            }
            // Return quaternion norm deviation (should be ≈ 0)
            black_box(body.state.orientation.quaternion().norm() - 1.0)
        })
    });
}

/// B1b — RK4 dumbbell at dt=1/240 for 10 seconds (2400 steps).
fn bench_b1_rk4_dt240(c: &mut Criterion) {
    c.bench_function("B1_dumbbell_rk4_dt240", |b| {
        b.iter(|| {
            let mut body = dumbbell_body();
            let gravity = Vector3::zeros();
            let dt = 1.0 / 240.0;
            for _ in 0..2400 {
                body.integrate(black_box(dt), gravity);
            }
            black_box(body.state.orientation.quaternion().norm() - 1.0)
        })
    });
}

/// B1c — Symplectic Euler dumbbell at dt=1/60 for 10 seconds (baseline).
fn bench_b1_euler_dt60(c: &mut Criterion) {
    let inertia = Matrix3::from_diagonal(&Vector3::new(0.01_f64, 2.0, 2.0));
    let inv_inertia = inertia.try_inverse().unwrap();

    c.bench_function("B1_dumbbell_euler_dt60", |b| {
        b.iter(|| {
            let mut orientation = UnitQuaternion::identity();
            let mut angular_velocity = Vector3::new(5.0_f64, 0.1, 0.0);
            let dt = 1.0 / 60.0;
            for _ in 0..600 {
                step_symplectic_euler(
                    black_box(&mut orientation),
                    black_box(&mut angular_velocity),
                    Vector3::zeros(),
                    &inv_inertia,
                    black_box(dt),
                );
            }
            black_box(orientation.quaternion().norm() - 1.0)
        })
    });
}

// ── Benchmark B2: Spinning Top ───────────────────────────────────────────────

/// B2 — RK4 spinning top: 5-second simulation at dt=1/240.
/// Measure angular velocity magnitude stability (should be conserved in torque-free precession).
fn bench_b2_spinning_top(c: &mut Criterion) {
    c.bench_function("B2_spinning_top_rk4_dt240", |b| {
        b.iter(|| {
            let mut body = spinning_top_body();
            let gravity = Vector3::zeros();
            let dt = 1.0 / 240.0;
            let steps = 1200; // 5 seconds
            for _ in 0..steps {
                body.integrate(black_box(dt), gravity);
            }
            // Angular momentum magnitude should be conserved
            let inertia = Matrix3::from_diagonal(&Vector3::new(1.0_f64, 1.0, 0.3));
            black_box(kinetic_energy_angular(
                &inertia,
                &body.state.angular_velocity,
            ))
        })
    });
}

// ── Benchmark B3: Scene Performance (N bodies) ───────────────────────────────

/// B4 — Physics step time for scenes of N rigid bodies.
fn bench_b4_scene_n_bodies(c: &mut Criterion) {
    let mut group = c.benchmark_group("B4_scene_physics_step");

    for &n in &[1_usize, 10, 50, 100, 200] {
        group.bench_with_input(BenchmarkId::new("rk4_bodies", n), &n, |b, &n| {
            // Create N dynamic bodies with sphere inertia (2/5 * m * r^2)
            let r = 0.5_f64;
            let m = 1.0_f64;
            let i = 0.4 * m * r * r;
            let inertia = Matrix3::from_diagonal(&Vector3::new(i, i, i));
            let props = RigidBodyProperties::dynamic(m, inertia);

            let mut bodies: Vec<RigidBody> = (0..n)
                .map(|id| {
                    let mut body = RigidBody::new(id as u64, props.clone());
                    body.state.angular_velocity = Vector3::new(1.0, 0.5, 0.25);
                    body
                })
                .collect();

            let gravity = Vector3::new(0.0, -9.81, 0.0);
            let dt = 1.0 / 240.0;

            b.iter(|| {
                for body in bodies.iter_mut() {
                    body.apply_force(Vector3::new(0.0, -9.81 * body.properties.mass, 0.0));
                    body.integrate(black_box(dt), black_box(gravity));
                }
            });
        });
    }

    group.finish();
}

/// B1d — Energy drift comparison: RK4 vs Symplectic Euler at dt=1/60, 10 seconds.
/// This measures raw integration throughput.
fn bench_b1_energy_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("B1_energy_drift");
    let inertia = Matrix3::from_diagonal(&Vector3::new(0.01_f64, 2.0, 2.0));
    let inv_inertia = inertia.try_inverse().unwrap();

    group.bench_function("rk4_600steps", |b| {
        b.iter(|| {
            let mut body = dumbbell_body();
            for _ in 0..600 {
                body.integrate(black_box(1.0 / 60.0), Vector3::zeros());
            }
            black_box(kinetic_energy_angular(
                &inertia,
                &body.state.angular_velocity,
            ))
        })
    });

    group.bench_function("euler_600steps", |b| {
        b.iter(|| {
            let mut orientation = UnitQuaternion::identity();
            let mut w = Vector3::new(5.0_f64, 0.1, 0.0);
            for _ in 0..600 {
                step_symplectic_euler(
                    &mut orientation,
                    &mut w,
                    Vector3::zeros(),
                    &inv_inertia,
                    1.0 / 60.0,
                );
            }
            black_box(kinetic_energy_angular(&inertia, &w))
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_b1_rk4_dt60,
    bench_b1_rk4_dt240,
    bench_b1_euler_dt60,
    bench_b2_spinning_top,
    bench_b4_scene_n_bodies,
    bench_b1_energy_comparison,
);
criterion_main!(benches);
