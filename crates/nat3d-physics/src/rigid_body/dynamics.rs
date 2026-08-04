// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Francisco Molina-Burgos, Avermex Research Division

//! High-precision Rigid Body Dynamics using Runge-Kutta 4th Order (RK4).

use nalgebra::{Matrix3, Quaternion, UnitQuaternion, Vector3};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RigidBodyProperties {
    pub mass: f64,
    pub inv_mass: f64,
    pub inertia: Matrix3<f64>,
    pub inv_inertia: Matrix3<f64>,
    pub linear_damping: f64,
    pub angular_damping: f64,
    pub is_static: bool,
    pub is_kinematic: bool,
    pub friction: f64,
    pub restitution: f64,
}

impl RigidBodyProperties {
    pub fn dynamic(mass: f64, inertia: Matrix3<f64>) -> Self {
        Self {
            mass,
            inv_mass: if mass > 0.0 { 1.0 / mass } else { 0.0 },
            inertia,
            inv_inertia: inertia.try_inverse().unwrap_or(Matrix3::zeros()),
            linear_damping: 0.01,
            angular_damping: 0.01,
            is_static: false,
            is_kinematic: false,
            friction: 0.5,
            restitution: 0.5,
        }
    }

    pub fn kinematic() -> Self {
        Self {
            mass: 0.0,
            inv_mass: 0.0,
            inertia: Matrix3::zeros(),
            inv_inertia: Matrix3::zeros(),
            linear_damping: 0.0,
            angular_damping: 0.0,
            is_static: false,
            is_kinematic: true,
            friction: 0.5,
            restitution: 0.5,
        }
    }

    pub fn static_body() -> Self {
        Self {
            mass: 0.0,
            inv_mass: 0.0,
            inertia: Matrix3::zeros(),
            inv_inertia: Matrix3::zeros(),
            linear_damping: 0.0,
            angular_damping: 0.0,
            is_static: true,
            is_kinematic: false,
            friction: 0.5,
            restitution: 0.5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RigidBodyState {
    pub position: Vector3<f64>,
    pub orientation: UnitQuaternion<f64>,
    pub linear_velocity: Vector3<f64>,
    pub angular_velocity: Vector3<f64>,
    pub force: Vector3<f64>,
    pub torque: Vector3<f64>,
}

impl Default for RigidBodyState {
    fn default() -> Self {
        Self {
            position: Vector3::zeros(),
            orientation: UnitQuaternion::identity(),
            linear_velocity: Vector3::zeros(),
            angular_velocity: Vector3::zeros(),
            force: Vector3::zeros(),
            torque: Vector3::zeros(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RigidBody {
    pub id: u64,
    pub properties: RigidBodyProperties,
    pub state: RigidBodyState,
    pub prev_state: RigidBodyState,
    pub sleep_timer: f64,
    pub sleeping: bool,
}

impl RigidBody {
    pub fn new(id: u64, properties: RigidBodyProperties) -> Self {
        Self {
            id,
            properties,
            state: RigidBodyState::default(),
            prev_state: RigidBodyState::default(),
            sleep_timer: 0.0,
            sleeping: false,
        }
    }

    pub fn apply_force(&mut self, force: Vector3<f64>) {
        self.state.force += force;
    }

    pub fn apply_torque(&mut self, torque: Vector3<f64>) {
        self.state.torque += torque;
    }

    pub fn clear_forces(&mut self) {
        self.state.force = Vector3::zeros();
        self.state.torque = Vector3::zeros();
    }

    pub fn world_inv_inertia(&self, orientation: &UnitQuaternion<f64>) -> Matrix3<f64> {
        let r = orientation.to_rotation_matrix();
        let r_mat = r.matrix();
        r_mat * self.properties.inv_inertia * r_mat.transpose()
    }

    pub fn integrate(&mut self, dt: f64, gravity: Vector3<f64>) {
        if self.properties.is_static || self.properties.is_kinematic {
            self.clear_forces();
            return;
        }

        self.prev_state = self.state.clone();

        let mass = self.properties.mass;
        let inv_mass = self.properties.inv_mass;

        // --- LINEAR RK4 ---
        let lin_derivatives = |vel: Vector3<f64>| -> (Vector3<f64>, Vector3<f64>) {
            (vel, (self.state.force + (gravity * mass)) * inv_mass)
        };

        let vel = self.state.linear_velocity;

        let (kx1, kv1) = lin_derivatives(vel);
        let (kx2, kv2) = lin_derivatives(vel + kv1 * (dt * 0.5));
        let (kx3, kv3) = lin_derivatives(vel + kv2 * (dt * 0.5));
        let (kx4, kv4) = lin_derivatives(vel + kv3 * dt);

        self.state.position += (kx1 + kx2 * 2.0 + kx3 * 2.0 + kx4) * (dt / 6.0);
        self.state.linear_velocity += (kv1 + kv2 * 2.0 + kv3 * 2.0 + kv4) * (dt / 6.0);
        self.state.linear_velocity *= (1.0 - self.properties.linear_damping).powf(dt);

        // --- ANGULAR RK4 ---
        let ang_deriv =
            |q: UnitQuaternion<f64>, w: Vector3<f64>| -> (Quaternion<f64>, Vector3<f64>) {
                // Recalculate world inverse inertia for this orientation
                let r = q.to_rotation_matrix();
                let r_mat = r.matrix();
                let world_inv_inertia = r_mat * self.properties.inv_inertia * r_mat.transpose();

                // dq/dt = 0.5 * q * (0, w)
                let w_quat = Quaternion::new(0.0, w.x, w.y, w.z);
                let q_raw = q.quaternion();
                let dq = Quaternion::new(
                    -0.5 * (w_quat.i * q_raw.i + w_quat.j * q_raw.j + w_quat.k * q_raw.k),
                    0.5 * (w_quat.i * q_raw.w + w_quat.k * q_raw.j - w_quat.j * q_raw.k),
                    0.5 * (w_quat.j * q_raw.w + w_quat.i * q_raw.k - w_quat.k * q_raw.i),
                    0.5 * (w_quat.k * q_raw.w + w_quat.j * q_raw.i - w_quat.i * q_raw.j),
                );

                // dw/dt = I^-1 * (torque - w x (I * w))
                let world_inertia = r_mat * self.properties.inertia * r_mat.transpose();
                let i_w = world_inertia * w;
                let dw = world_inv_inertia * (self.state.torque - w.cross(&i_w));

                (dq, dw)
            };

        let q = self.state.orientation;
        let w = self.state.angular_velocity;

        let add_q = |base: UnitQuaternion<f64>, delta: Quaternion<f64>| -> UnitQuaternion<f64> {
            let b = base.quaternion();
            let sum = Quaternion::new(b.w + delta.w, b.i + delta.i, b.j + delta.j, b.k + delta.k);
            UnitQuaternion::from_quaternion(sum)
        };

        let (kq1, kw1) = ang_deriv(q, w);
        let (kq2, kw2) = ang_deriv(add_q(q, kq1 * (dt * 0.5)), w + kw1 * (dt * 0.5));
        let (kq3, kw3) = ang_deriv(add_q(q, kq2 * (dt * 0.5)), w + kw2 * (dt * 0.5));
        let (kq4, kw4) = ang_deriv(add_q(q, kq3 * dt), w + kw3 * dt);

        self.state.angular_velocity += (kw1 + kw2 * 2.0 + kw3 * 2.0 + kw4) * (dt / 6.0);
        let dq = (kq1 + kq2 * 2.0 + kq3 * 2.0 + kq4) * (dt / 6.0);
        self.state.orientation = add_q(self.state.orientation, dq);

        self.state.angular_velocity *= (1.0 - self.properties.angular_damping).powf(dt);

        self.clear_forces();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra::Vector3;

    fn run_trajectory() -> Vector3<f64> {
        let props = RigidBodyProperties::dynamic(1.0, Matrix3::identity());
        let mut body = RigidBody::new(1, props);
        body.state.position = Vector3::new(0.0, 10.0, 0.0);
        let gravity = Vector3::new(0.0, -9.81, 0.0);
        let dt = 0.01;
        for _ in 0..100 {
            body.apply_force(Vector3::zeros());
            body.integrate(dt, gravity);
        }
        body.state.position
    }

    #[test]
    fn rk4_linear_deterministic_30_runs() {
        let expected = run_trajectory();
        for _ in 0..30 {
            assert_eq!(run_trajectory(), expected);
        }
    }

    #[test]
    fn rk4_linear_free_fall_accuracy() {
        let pos = run_trajectory();
        let analytical_y = 10.0 - 0.5 * 9.81 * 1.0_f64.powi(2);
        assert!((pos.y - analytical_y).abs() < 0.1);
    }

    #[test]
    fn rk4_angular_simple_rotation() {
        let mut props = RigidBodyProperties::dynamic(1.0, Matrix3::identity());
        props.angular_damping = 0.0;
        let mut body = RigidBody::new(2, props);
        body.apply_torque(Vector3::new(1.0, 0.0, 0.0));
        let dt = 0.1;
        body.integrate(dt, Vector3::zeros());
        assert!((body.state.angular_velocity.x - 0.1).abs() < 1e-6);
        assert!(body.state.orientation.angle() > 0.0);
    }
}
