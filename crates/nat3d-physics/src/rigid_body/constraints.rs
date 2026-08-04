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

//! Constraints for rigid bodies.
//!
//! Implements joints and constraints using sequential impulse solver.

use nalgebra::{Matrix3, Vector3};

/// Trait for rigid body constraints.
pub trait RigidConstraint {
    /// Solve the constraint by applying impulses.
    fn solve(&mut self, dt: f64);

    /// Get the indices of the bodies involved in this constraint.
    fn get_bodies(&self) -> (usize, usize);
}

/// Distance constraint maintains a fixed distance between two points on rigid bodies.
#[derive(Debug, Clone)]
pub struct DistanceConstraint {
    /// Index of body A.
    pub body_a: usize,
    /// Index of body B.
    pub body_b: usize,
    /// Anchor point on body A (local space).
    pub anchor_a: Vector3<f64>,
    /// Anchor point on body B (local space).
    pub anchor_b: Vector3<f64>,
    /// Target distance to maintain.
    pub distance: f64,
    /// Constraint stiffness (0-1).
    pub stiffness: f64,
}

impl DistanceConstraint {
    /// Create a new distance constraint.
    pub fn new(
        body_a: usize,
        body_b: usize,
        anchor_a: Vector3<f64>,
        anchor_b: Vector3<f64>,
        distance: f64,
    ) -> Self {
        Self {
            body_a,
            body_b,
            anchor_a,
            anchor_b,
            distance,
            stiffness: 1.0,
        }
    }

    /// Compute constraint error.
    pub fn compute_error(&self, pos_a: Vector3<f64>, pos_b: Vector3<f64>) -> f64 {
        let current_dist = (pos_b - pos_a).magnitude();
        current_dist - self.distance
    }
}

/// Hinge constraint (revolute joint) allows rotation around a single axis.
#[derive(Debug, Clone)]
pub struct HingeConstraint {
    /// Index of body A.
    pub body_a: usize,
    /// Index of body B.
    pub body_b: usize,
    /// Anchor point on body A (local space).
    pub anchor_a: Vector3<f64>,
    /// Anchor point on body B (local space).
    pub anchor_b: Vector3<f64>,
    /// Hinge axis on body A (local space).
    pub axis_a: Vector3<f64>,
    /// Hinge axis on body B (local space).
    pub axis_b: Vector3<f64>,
    /// Minimum angle limit (radians).
    pub min_angle: f64,
    /// Maximum angle limit (radians).
    pub max_angle: f64,
    /// Is angle limited?
    pub use_limits: bool,
}

impl HingeConstraint {
    /// Create a new hinge constraint.
    pub fn new(
        body_a: usize,
        body_b: usize,
        anchor_a: Vector3<f64>,
        anchor_b: Vector3<f64>,
        axis_a: Vector3<f64>,
        axis_b: Vector3<f64>,
    ) -> Self {
        Self {
            body_a,
            body_b,
            anchor_a,
            anchor_b,
            axis_a: axis_a.normalize(),
            axis_b: axis_b.normalize(),
            min_angle: -std::f64::consts::PI,
            max_angle: std::f64::consts::PI,
            use_limits: false,
        }
    }

    /// Set angle limits.
    pub fn set_limits(&mut self, min: f64, max: f64) {
        self.min_angle = min;
        self.max_angle = max;
        self.use_limits = true;
    }
}

/// Ball and socket joint (spherical joint) constrains position but allows free rotation.
#[derive(Debug, Clone)]
pub struct BallSocketConstraint {
    /// Index of body A.
    pub body_a: usize,
    /// Index of body B.
    pub body_b: usize,
    /// Anchor point on body A (local space).
    pub anchor_a: Vector3<f64>,
    /// Anchor point on body B (local space).
    pub anchor_b: Vector3<f64>,
    /// Constraint stiffness.
    pub stiffness: f64,
}

impl BallSocketConstraint {
    /// Create a new ball socket constraint.
    pub fn new(
        body_a: usize,
        body_b: usize,
        anchor_a: Vector3<f64>,
        anchor_b: Vector3<f64>,
    ) -> Self {
        Self {
            body_a,
            body_b,
            anchor_a,
            anchor_b,
            stiffness: 1.0,
        }
    }

    /// Compute position error.
    pub fn compute_error(&self, pos_a: Vector3<f64>, pos_b: Vector3<f64>) -> Vector3<f64> {
        pos_b - pos_a
    }
}

/// Slider constraint (prismatic joint) allows translation along an axis.
#[derive(Debug, Clone)]
pub struct SliderConstraint {
    /// Index of body A.
    pub body_a: usize,
    /// Index of body B.
    pub body_b: usize,
    /// Anchor point on body A (local space).
    pub anchor_a: Vector3<f64>,
    /// Anchor point on body B (local space).
    pub anchor_b: Vector3<f64>,
    /// Slider axis (local space of body A).
    pub axis: Vector3<f64>,
    /// Minimum translation limit.
    pub min_limit: f64,
    /// Maximum translation limit.
    pub max_limit: f64,
    /// Use limits?
    pub use_limits: bool,
}

impl SliderConstraint {
    /// Create a new slider constraint.
    pub fn new(
        body_a: usize,
        body_b: usize,
        anchor_a: Vector3<f64>,
        anchor_b: Vector3<f64>,
        axis: Vector3<f64>,
    ) -> Self {
        Self {
            body_a,
            body_b,
            anchor_a,
            anchor_b,
            axis: axis.normalize(),
            min_limit: -1.0,
            max_limit: 1.0,
            use_limits: false,
        }
    }

    /// Set translation limits.
    pub fn set_limits(&mut self, min: f64, max: f64) {
        self.min_limit = min;
        self.max_limit = max;
        self.use_limits = true;
    }

    /// Compute current translation along axis.
    pub fn compute_translation(&self, pos_a: Vector3<f64>, pos_b: Vector3<f64>) -> f64 {
        let delta = pos_b - pos_a;
        delta.dot(&self.axis)
    }
}

/// Fixed constraint welds two bodies together.
#[derive(Debug, Clone)]
pub struct FixedConstraint {
    /// Index of body A.
    pub body_a: usize,
    /// Index of body B.
    pub body_b: usize,
    /// Relative offset (from A to B) in world space at creation.
    pub relative_offset: Vector3<f64>,
    /// Relative rotation at creation.
    pub relative_rotation: Matrix3<f64>,
}

impl FixedConstraint {
    /// Create a new fixed constraint.
    pub fn new(
        body_a: usize,
        body_b: usize,
        pos_a: Vector3<f64>,
        pos_b: Vector3<f64>,
        rot_a: Matrix3<f64>,
        rot_b: Matrix3<f64>,
    ) -> Self {
        let relative_offset = rot_a.transpose() * (pos_b - pos_a);
        let relative_rotation = rot_a.transpose() * rot_b;

        Self {
            body_a,
            body_b,
            relative_offset,
            relative_rotation,
        }
    }

    /// Compute position error.
    pub fn compute_position_error(
        &self,
        pos_a: Vector3<f64>,
        pos_b: Vector3<f64>,
        rot_a: Matrix3<f64>,
    ) -> Vector3<f64> {
        let target_pos_b = pos_a + rot_a * self.relative_offset;
        pos_b - target_pos_b
    }

    /// Compute rotation error.
    pub fn compute_rotation_error(&self, rot_a: Matrix3<f64>, rot_b: Matrix3<f64>) -> Vector3<f64> {
        let target_rot_b = rot_a * self.relative_rotation;
        let error_rot = rot_b * target_rot_b.transpose();

        // Extract rotation axis (simplified)
        let trace = error_rot.trace();
        let angle = ((trace - 1.0) / 2.0).clamp(-1.0, 1.0).acos();

        if angle.abs() < 1e-6 {
            return Vector3::zeros();
        }

        let axis = Vector3::new(
            error_rot[(2, 1)] - error_rot[(1, 2)],
            error_rot[(0, 2)] - error_rot[(2, 0)],
            error_rot[(1, 0)] - error_rot[(0, 1)],
        );

        if axis.magnitude() < 1e-10 {
            Vector3::zeros()
        } else {
            axis.normalize() * angle
        }
    }
}

/// Sequential impulse constraint solver.
pub struct ConstraintSolver {
    /// Distance constraints.
    pub distance_constraints: Vec<DistanceConstraint>,
    /// Hinge constraints.
    pub hinge_constraints: Vec<HingeConstraint>,
    /// Ball socket constraints.
    pub ball_socket_constraints: Vec<BallSocketConstraint>,
    /// Slider constraints.
    pub slider_constraints: Vec<SliderConstraint>,
    /// Fixed constraints.
    pub fixed_constraints: Vec<FixedConstraint>,
    /// Number of solver iterations.
    pub iterations: usize,
}

impl ConstraintSolver {
    /// Create a new constraint solver.
    pub fn new() -> Self {
        Self {
            distance_constraints: Vec::new(),
            hinge_constraints: Vec::new(),
            ball_socket_constraints: Vec::new(),
            slider_constraints: Vec::new(),
            fixed_constraints: Vec::new(),
            iterations: 10,
        }
    }

    /// Add a distance constraint.
    pub fn add_distance_constraint(&mut self, constraint: DistanceConstraint) {
        self.distance_constraints.push(constraint);
    }

    /// Add a hinge constraint.
    pub fn add_hinge_constraint(&mut self, constraint: HingeConstraint) {
        self.hinge_constraints.push(constraint);
    }

    /// Add a ball socket constraint.
    pub fn add_ball_socket_constraint(&mut self, constraint: BallSocketConstraint) {
        self.ball_socket_constraints.push(constraint);
    }

    /// Add a slider constraint.
    pub fn add_slider_constraint(&mut self, constraint: SliderConstraint) {
        self.slider_constraints.push(constraint);
    }

    /// Add a fixed constraint.
    pub fn add_fixed_constraint(&mut self, constraint: FixedConstraint) {
        self.fixed_constraints.push(constraint);
    }

    /// Clear all constraints.
    pub fn clear(&mut self) {
        self.distance_constraints.clear();
        self.hinge_constraints.clear();
        self.ball_socket_constraints.clear();
        self.slider_constraints.clear();
        self.fixed_constraints.clear();
    }

    /// Get total number of constraints.
    pub fn constraint_count(&self) -> usize {
        self.distance_constraints.len()
            + self.hinge_constraints.len()
            + self.ball_socket_constraints.len()
            + self.slider_constraints.len()
            + self.fixed_constraints.len()
    }
}

impl Default for ConstraintSolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distance_constraint() {
        let constraint = DistanceConstraint::new(0, 1, Vector3::zeros(), Vector3::zeros(), 2.0);

        let pos_a = Vector3::new(0.0, 0.0, 0.0);
        let pos_b = Vector3::new(3.0, 0.0, 0.0);

        let error = constraint.compute_error(pos_a, pos_b);
        assert_eq!(error, 1.0); // Distance is 3, target is 2, error is 1
    }

    #[test]
    fn test_ball_socket_constraint() {
        let constraint = BallSocketConstraint::new(0, 1, Vector3::zeros(), Vector3::zeros());

        let pos_a = Vector3::new(0.0, 0.0, 0.0);
        let pos_b = Vector3::new(1.0, 2.0, 3.0);

        let error = constraint.compute_error(pos_a, pos_b);
        assert_eq!(error, Vector3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn test_solver_creation() {
        let solver = ConstraintSolver::new();
        assert_eq!(solver.constraint_count(), 0);
    }

    #[test]
    fn test_add_constraints() {
        let mut solver = ConstraintSolver::new();

        solver.add_distance_constraint(DistanceConstraint::new(
            0,
            1,
            Vector3::zeros(),
            Vector3::zeros(),
            1.0,
        ));

        solver.add_ball_socket_constraint(BallSocketConstraint::new(
            1,
            2,
            Vector3::zeros(),
            Vector3::zeros(),
        ));

        assert_eq!(solver.constraint_count(), 2);
    }
}
