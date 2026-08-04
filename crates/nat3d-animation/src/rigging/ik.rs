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

//! Inverse kinematics solvers.
//!
//! Implements FABRIK, CCD, and analytical IK solvers for skeletal animation.

use nalgebra::{Point3, UnitQuaternion, Vector3};

use super::armature::Armature;
use super::bone::BoneId;

/// IK solver type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IkSolverType {
    /// Forward And Backward Reaching Inverse Kinematics.
    Fabrik,
    /// Cyclic Coordinate Descent.
    Ccd,
    /// Jacobian-based solver.
    Jacobian,
    /// Two-bone analytical solver.
    TwoBone,
}

/// IK chain definition.
#[derive(Debug, Clone)]
pub struct IkChain {
    /// End effector bone.
    pub end_effector: BoneId,
    /// Chain length (number of bones).
    pub chain_length: usize,
    /// Pole target for orienting the chain.
    pub pole_target: Option<Point3<f64>>,
    /// Target position.
    pub target: Point3<f64>,
    /// Target rotation (optional).
    pub target_rotation: Option<UnitQuaternion<f64>>,
    /// Maximum iterations.
    pub max_iterations: usize,
    /// Tolerance for convergence.
    pub tolerance: f64,
    /// Solver type.
    pub solver_type: IkSolverType,
    /// Influence (0-1).
    pub influence: f64,
}

impl IkChain {
    /// Create a new IK chain.
    pub fn new(end_effector: BoneId, chain_length: usize) -> Self {
        Self {
            end_effector,
            chain_length,
            pole_target: None,
            target: Point3::origin(),
            target_rotation: None,
            max_iterations: 10,
            tolerance: 0.001,
            solver_type: IkSolverType::Fabrik,
            influence: 1.0,
        }
    }

    /// Set target position.
    pub fn with_target(mut self, target: Point3<f64>) -> Self {
        self.target = target;
        self
    }

    /// Set pole target.
    pub fn with_pole(mut self, pole: Point3<f64>) -> Self {
        self.pole_target = Some(pole);
        self
    }

    /// Set solver type.
    pub fn with_solver(mut self, solver: IkSolverType) -> Self {
        self.solver_type = solver;
        self
    }
}

/// IK solver.
#[derive(Debug)]
pub struct IkSolver {
    /// Working positions for chain joints.
    positions: Vec<Point3<f64>>,
    /// Bone lengths in chain.
    lengths: Vec<f64>,
    /// Original rotations.
    original_rotations: Vec<UnitQuaternion<f64>>,
}

impl IkSolver {
    /// Create a new IK solver.
    pub fn new() -> Self {
        Self {
            positions: Vec::new(),
            lengths: Vec::new(),
            original_rotations: Vec::new(),
        }
    }

    /// Solve IK for a chain.
    pub fn solve(&mut self, armature: &mut Armature, chain: &IkChain) {
        match chain.solver_type {
            IkSolverType::Fabrik => self.solve_fabrik(armature, chain),
            IkSolverType::Ccd => self.solve_ccd(armature, chain),
            IkSolverType::Jacobian => self.solve_jacobian(armature, chain),
            IkSolverType::TwoBone => self.solve_two_bone(armature, chain),
        }
    }

    /// FABRIK solver implementation.
    fn solve_fabrik(&mut self, armature: &mut Armature, chain: &IkChain) {
        // Get bone chain
        let bone_ids = armature.get_bone_chain(chain.end_effector, chain.chain_length);
        if bone_ids.is_empty() {
            return;
        }

        // Extract current positions and lengths
        self.extract_chain_data(armature, &bone_ids);

        let total_length: f64 = self.lengths.iter().sum();
        let root_pos = self.positions[self.positions.len() - 1];
        let target_dist = (chain.target - root_pos).magnitude();

        // Check if target is reachable
        if target_dist > total_length {
            // Stretch towards target
            self.stretch_to_target(chain.target);
        } else {
            // FABRIK iterations
            for _ in 0..chain.max_iterations {
                // Forward reaching
                self.positions[0] = chain.target;
                for i in 1..self.positions.len() {
                    let direction = (self.positions[i] - self.positions[i - 1]).normalize();
                    self.positions[i] = self.positions[i - 1] + direction * self.lengths[i - 1];
                }

                // Backward reaching
                let last_idx = self.positions.len() - 1;
                self.positions[last_idx] = root_pos;
                for i in (0..self.positions.len() - 1).rev() {
                    let direction = (self.positions[i] - self.positions[i + 1]).normalize();
                    self.positions[i] = self.positions[i + 1] + direction * self.lengths[i];
                }

                // Check convergence
                let error = (self.positions[0] - chain.target).magnitude();
                if error < chain.tolerance {
                    break;
                }
            }
        }

        // Apply pole target constraint
        if let Some(pole) = chain.pole_target {
            self.apply_pole_constraint(pole);
        }

        // Apply results back to armature
        self.apply_results(armature, &bone_ids, chain.influence);
    }

    /// CCD solver implementation.
    fn solve_ccd(&mut self, armature: &mut Armature, chain: &IkChain) {
        let bone_ids = armature.get_bone_chain(chain.end_effector, chain.chain_length);
        if bone_ids.is_empty() {
            return;
        }

        self.extract_chain_data(armature, &bone_ids);

        for _ in 0..chain.max_iterations {
            // Iterate from root to end
            for i in (1..self.positions.len()).rev() {
                let end_effector = self.positions[0];
                let joint = self.positions[i];

                let to_end = (end_effector - joint).normalize();
                let to_target = (chain.target - joint).normalize();

                // Compute rotation
                if let Some(rotation) = UnitQuaternion::rotation_between(&to_end, &to_target) {
                    // Apply rotation to all joints from this one to the end
                    for j in 0..i {
                        let relative = self.positions[j] - joint;
                        let rotated = rotation * relative;
                        self.positions[j] = joint + rotated;
                    }
                }
            }

            // Check convergence
            let error = (self.positions[0] - chain.target).magnitude();
            if error < chain.tolerance {
                break;
            }
        }

        if let Some(pole) = chain.pole_target {
            self.apply_pole_constraint(pole);
        }

        self.apply_results(armature, &bone_ids, chain.influence);
    }

    /// Jacobian-based solver (simplified).
    fn solve_jacobian(&mut self, armature: &mut Armature, chain: &IkChain) {
        let bone_ids = armature.get_bone_chain(chain.end_effector, chain.chain_length);
        if bone_ids.is_empty() {
            return;
        }

        self.extract_chain_data(armature, &bone_ids);

        let damping = 0.5;

        for _ in 0..chain.max_iterations {
            let end_effector = self.positions[0];
            let error = chain.target - end_effector;

            if error.magnitude() < chain.tolerance {
                break;
            }

            // Compute Jacobian and update each joint
            for i in (1..self.positions.len()).rev() {
                let joint = self.positions[i];
                let axis = Vector3::new(0.0, 0.0, 1.0); // Simplified - assume Z rotation

                // Jacobian column for this joint
                let to_end = end_effector - joint;
                let jacobian_col = axis.cross(&to_end);

                // Compute delta angle using damped least squares
                let j_dot_e = jacobian_col.dot(&error);
                let j_dot_j = jacobian_col.dot(&jacobian_col);
                let delta_angle = (j_dot_e) / (j_dot_j + damping * damping);

                // Apply rotation
                let rotation = UnitQuaternion::from_axis_angle(
                    &nalgebra::Unit::new_normalize(axis),
                    delta_angle.clamp(-0.1, 0.1),
                );

                for j in 0..i {
                    let relative = self.positions[j] - joint;
                    let rotated = rotation * relative;
                    self.positions[j] = joint + rotated;
                }
            }
        }

        self.apply_results(armature, &bone_ids, chain.influence);
    }

    /// Two-bone analytical solver.
    fn solve_two_bone(&mut self, armature: &mut Armature, chain: &IkChain) {
        let bone_ids = armature.get_bone_chain(chain.end_effector, 2.min(chain.chain_length));
        if bone_ids.len() < 2 {
            return;
        }

        self.extract_chain_data(armature, &bone_ids);

        if self.positions.len() < 3 {
            return;
        }

        let root = self.positions[2];
        let _mid = self.positions[1];
        let _end = self.positions[0];
        let target = chain.target;

        let len_a = self.lengths[1];
        let len_b = self.lengths[0];
        let len_c = (target - root).magnitude();

        // Clamp to reachable range
        let len_c = len_c.clamp((len_a - len_b).abs() + 0.001, len_a + len_b - 0.001);

        // Law of cosines for joint angles
        let cos_angle_a = ((len_a * len_a + len_c * len_c - len_b * len_b) / (2.0 * len_a * len_c))
            .clamp(-1.0, 1.0);
        let cos_angle_b = ((len_a * len_a + len_b * len_b - len_c * len_c) / (2.0 * len_a * len_b))
            .clamp(-1.0, 1.0);

        let angle_a = cos_angle_a.acos();
        let _angle_b = cos_angle_b.acos();

        // Direction from root to target
        let dir_to_target = (target - root).normalize();

        // Compute pole plane normal
        let pole_dir = if let Some(pole) = chain.pole_target {
            let to_pole = pole - root;
            let proj = dir_to_target * to_pole.dot(&dir_to_target);
            (to_pole - proj).normalize()
        } else {
            // Default pole direction
            let up = Vector3::new(0.0, 1.0, 0.0);
            let proj = dir_to_target * up.dot(&dir_to_target);
            (up - proj)
                .try_normalize(1e-6)
                .unwrap_or(Vector3::new(1.0, 0.0, 0.0))
        };

        // Compute new mid position
        let rotation = UnitQuaternion::from_axis_angle(
            &nalgebra::Unit::new_normalize(dir_to_target.cross(&pole_dir)),
            angle_a,
        );
        let new_mid = root + rotation * (dir_to_target * len_a);

        // Update positions
        self.positions[2] = root;
        self.positions[1] = new_mid;
        self.positions[0] = target;

        self.apply_results(armature, &bone_ids, chain.influence);
    }

    /// Extract chain positions and lengths from armature.
    fn extract_chain_data(&mut self, armature: &Armature, bone_ids: &[BoneId]) {
        self.positions.clear();
        self.lengths.clear();
        self.original_rotations.clear();

        // Ensure armature is updated
        // Note: In real usage, armature.update() should be called before solving

        for &bone_id in bone_ids {
            if let Some(bone) = armature.get_bone(bone_id) {
                // Get world position from bone
                if let Some(world_transform) = armature.world_transform(bone_id) {
                    let pos = Point3::new(
                        world_transform[(0, 3)],
                        world_transform[(1, 3)],
                        world_transform[(2, 3)],
                    );
                    self.positions.push(pos);
                    self.original_rotations.push(bone.pose.rotation);
                }
            }
        }

        // Compute lengths between consecutive joints
        for i in 0..self.positions.len().saturating_sub(1) {
            let length = (self.positions[i] - self.positions[i + 1]).magnitude();
            self.lengths.push(length);
        }
    }

    /// Stretch chain towards unreachable target.
    fn stretch_to_target(&mut self, target: Point3<f64>) {
        if self.positions.is_empty() {
            return;
        }

        let root = self.positions[self.positions.len() - 1];
        let direction = (target - root).normalize();

        let mut current = root;
        for i in (0..self.positions.len() - 1).rev() {
            current += direction * self.lengths[i];
            self.positions[i] = current;
        }
    }

    /// Apply pole target constraint.
    fn apply_pole_constraint(&mut self, pole: Point3<f64>) {
        if self.positions.len() < 3 {
            return;
        }

        // For each joint except root and end
        for i in 1..self.positions.len() - 1 {
            let prev = self.positions[i + 1];
            let curr = self.positions[i];
            let next = self.positions[i - 1];

            // Chain direction
            let chain_dir = (next - prev).normalize();

            // Project current position onto chain axis
            let to_curr = curr - prev;
            let proj_length = to_curr.dot(&chain_dir);
            let proj_point = prev + chain_dir * proj_length;

            // Direction from projected point to pole
            let to_pole = pole - proj_point;
            let pole_dir = (to_pole - chain_dir * to_pole.dot(&chain_dir)).normalize();

            // Current bend direction
            let curr_bend = (curr - proj_point).normalize();

            // Rotate around chain axis to align with pole
            if let Some(rotation) = UnitQuaternion::rotation_between(&curr_bend, &pole_dir) {
                let new_pos = proj_point + rotation * (curr - proj_point);
                self.positions[i] = new_pos;
            }
        }
    }

    /// Apply solved positions back to armature.
    fn apply_results(&self, armature: &mut Armature, bone_ids: &[BoneId], influence: f64) {
        if self.positions.len() < 2 || bone_ids.len() != self.positions.len() - 1 {
            return;
        }

        // Compute rotations from positions
        for (i, &bone_id) in bone_ids.iter().enumerate() {
            if i + 1 >= self.positions.len() {
                break;
            }

            let joint = self.positions[i + 1];
            let end = self.positions[i];
            let direction = (end - joint).normalize();

            // Compute rotation to point bone at child
            if let Some(bone) = armature.get_bone(bone_id) {
                let bone_dir = bone.direction();
                if let Some(rotation) = UnitQuaternion::rotation_between(&bone_dir, &direction) {
                    if let Some(bone_mut) = armature.get_bone_mut(bone_id) {
                        let new_rotation = rotation * bone_mut.pose.rotation;
                        bone_mut.pose.rotation =
                            bone_mut.pose.rotation.slerp(&new_rotation, influence);
                    }
                }
            }
        }
    }
}

impl Default for IkSolver {
    fn default() -> Self {
        Self::new()
    }
}

/// IK constraint configuration.
#[derive(Debug, Clone)]
pub struct IkConstraint {
    /// Joint angle limits (min, max) for each axis.
    pub angle_limits: Option<AngleLimits>,
    /// Joint stiffness (resistance to movement).
    pub stiffness: f64,
    /// Joint weight in optimization.
    pub weight: f64,
}

/// Angle limits for a joint.
#[derive(Debug, Clone, Copy)]
pub struct AngleLimits {
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
    pub min_z: f64,
    pub max_z: f64,
}

impl Default for AngleLimits {
    fn default() -> Self {
        let pi = std::f64::consts::PI;
        Self {
            min_x: -pi,
            max_x: pi,
            min_y: -pi,
            max_y: pi,
            min_z: -pi,
            max_z: pi,
        }
    }
}

impl AngleLimits {
    /// Create knee-like limits (single axis, limited range).
    pub fn knee() -> Self {
        Self {
            min_x: 0.0,
            max_x: std::f64::consts::PI * 0.9,
            min_y: -0.1,
            max_y: 0.1,
            min_z: -0.1,
            max_z: 0.1,
        }
    }

    /// Create elbow-like limits.
    pub fn elbow() -> Self {
        Self {
            min_x: -std::f64::consts::PI * 0.9,
            max_x: 0.0,
            min_y: -0.1,
            max_y: 0.1,
            min_z: -0.1,
            max_z: 0.1,
        }
    }

    /// Create ball-joint limits.
    pub fn ball_joint(angle: f64) -> Self {
        Self {
            min_x: -angle,
            max_x: angle,
            min_y: -angle,
            max_y: angle,
            min_z: -angle,
            max_z: angle,
        }
    }

    /// Clamp euler angles to limits.
    pub fn clamp(&self, roll: f64, pitch: f64, yaw: f64) -> (f64, f64, f64) {
        (
            roll.clamp(self.min_x, self.max_x),
            pitch.clamp(self.min_y, self.max_y),
            yaw.clamp(self.min_z, self.max_z),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rigging::armature::ArmatureBuilder;

    #[test]
    fn test_ik_chain_creation() {
        let chain = IkChain::new(BoneId(0), 3)
            .with_target(Point3::new(1.0, 0.0, 0.0))
            .with_solver(IkSolverType::Fabrik);

        assert_eq!(chain.chain_length, 3);
        assert_eq!(chain.solver_type, IkSolverType::Fabrik);
    }

    #[test]
    fn test_ik_solver_creation() {
        let solver = IkSolver::new();
        assert!(solver.positions.is_empty());
    }

    #[test]
    fn test_angle_limits() {
        let knee = AngleLimits::knee();
        let (r, p, _y) = knee.clamp(1.0, 0.5, 0.5);
        assert!(r >= knee.min_x && r <= knee.max_x);
        assert!(p >= knee.min_y && p <= knee.max_y);
    }

    #[test]
    fn test_two_bone_ik() {
        let mut armature = ArmatureBuilder::new("Arm")
            .add_root(
                "Shoulder",
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
            )
            .add_child("Elbow", Point3::new(2.0, 0.0, 0.0))
            .add_child("Wrist", Point3::new(3.0, 0.0, 0.0))
            .build();

        armature.update();

        let chain = IkChain::new(BoneId(2), 2)
            .with_target(Point3::new(1.5, 1.0, 0.0))
            .with_solver(IkSolverType::TwoBone);

        let mut solver = IkSolver::new();
        solver.solve(&mut armature, &chain);
    }
}
