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

//! Sketch constraint system.
//!
//! Implements geometric and dimensional constraints for parametric sketching.

use nalgebra::{Point2, Vector2};
use std::collections::HashMap;

/// Constraint ID.
pub type ConstraintId = u64;

/// Entity ID for constraints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntityRef {
    /// Point entity.
    Point(usize),
    /// Line entity (start, end points).
    Line(usize, usize),
    /// Circle entity (center, radius point).
    Circle(usize),
    /// Arc entity.
    Arc(usize),
}

/// Geometric constraints.
#[derive(Debug, Clone)]
pub enum GeometricConstraint {
    /// Fix point at position.
    Fixed(EntityRef, Point2<f64>),
    /// Coincident points.
    Coincident(EntityRef, EntityRef),
    /// Horizontal line/points.
    Horizontal(EntityRef),
    /// Vertical line/points.
    Vertical(EntityRef),
    /// Parallel lines.
    Parallel(EntityRef, EntityRef),
    /// Perpendicular lines.
    Perpendicular(EntityRef, EntityRef),
    /// Tangent constraint.
    Tangent(EntityRef, EntityRef),
    /// Equal length/radius.
    Equal(EntityRef, EntityRef),
    /// Symmetric about line/point.
    Symmetric(EntityRef, EntityRef, EntityRef),
    /// Concentric circles/arcs.
    Concentric(EntityRef, EntityRef),
    /// Collinear points/lines.
    Collinear(Vec<EntityRef>),
    /// Point on entity.
    PointOn(EntityRef, EntityRef),
    /// Midpoint constraint.
    Midpoint(EntityRef, EntityRef),
}

/// Dimensional constraints.
#[derive(Debug, Clone)]
pub enum DimensionalConstraint {
    /// Distance between points.
    Distance(EntityRef, EntityRef, f64),
    /// Horizontal distance.
    HorizontalDistance(EntityRef, EntityRef, f64),
    /// Vertical distance.
    VerticalDistance(EntityRef, EntityRef, f64),
    /// Line length.
    Length(EntityRef, f64),
    /// Angle between lines.
    Angle(EntityRef, EntityRef, f64),
    /// Circle/arc radius.
    Radius(EntityRef, f64),
    /// Circle/arc diameter.
    Diameter(EntityRef, f64),
}

/// Constraint with metadata.
#[derive(Debug, Clone)]
pub struct Constraint {
    /// Unique identifier.
    pub id: ConstraintId,
    /// Constraint type.
    pub constraint_type: ConstraintType,
    /// Is constraint satisfied?
    pub is_satisfied: bool,
    /// Error magnitude.
    pub error: f64,
    /// Is driving (vs driven).
    pub is_driving: bool,
}

/// Constraint type union.
#[derive(Debug, Clone)]
pub enum ConstraintType {
    /// Geometric constraint.
    Geometric(GeometricConstraint),
    /// Dimensional constraint.
    Dimensional(DimensionalConstraint),
}

/// Constraint solver using Newton-Raphson iteration.
pub struct ConstraintSolver {
    /// Points (variables).
    pub points: Vec<Point2<f64>>,
    /// Constraints.
    pub constraints: Vec<Constraint>,
    /// Max iterations.
    pub max_iterations: usize,
    /// Convergence tolerance.
    pub tolerance: f64,
    /// Next constraint ID.
    next_id: ConstraintId,
}

impl Default for ConstraintSolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ConstraintSolver {
    /// Create a new constraint solver.
    pub fn new() -> Self {
        Self {
            points: Vec::new(),
            constraints: Vec::new(),
            max_iterations: 100,
            tolerance: 1e-10,
            next_id: 1,
        }
    }

    /// Add a point and return its index.
    pub fn add_point(&mut self, point: Point2<f64>) -> usize {
        let idx = self.points.len();
        self.points.push(point);
        idx
    }

    /// Add a geometric constraint.
    pub fn add_geometric(&mut self, constraint: GeometricConstraint) -> ConstraintId {
        let id = self.next_id;
        self.next_id += 1;

        self.constraints.push(Constraint {
            id,
            constraint_type: ConstraintType::Geometric(constraint),
            is_satisfied: false,
            error: f64::MAX,
            is_driving: true,
        });

        id
    }

    /// Add a dimensional constraint.
    pub fn add_dimensional(&mut self, constraint: DimensionalConstraint) -> ConstraintId {
        let id = self.next_id;
        self.next_id += 1;

        self.constraints.push(Constraint {
            id,
            constraint_type: ConstraintType::Dimensional(constraint),
            is_satisfied: false,
            error: f64::MAX,
            is_driving: true,
        });

        id
    }

    /// Remove a constraint by ID.
    pub fn remove_constraint(&mut self, id: ConstraintId) -> bool {
        if let Some(pos) = self.constraints.iter().position(|c| c.id == id) {
            self.constraints.remove(pos);
            true
        } else {
            false
        }
    }

    /// Get degrees of freedom.
    pub fn degrees_of_freedom(&self) -> i32 {
        let total_dof = (self.points.len() * 2) as i32;
        let constraint_dof: i32 = self
            .constraints
            .iter()
            .filter(|c| c.is_driving)
            .map(|c| self.constraint_dof(&c.constraint_type))
            .sum();

        total_dof - constraint_dof
    }

    /// Get DOF consumed by a constraint.
    fn constraint_dof(&self, ct: &ConstraintType) -> i32 {
        match ct {
            ConstraintType::Geometric(g) => match g {
                GeometricConstraint::Fixed(_, _) => 2,
                GeometricConstraint::Coincident(_, _) => 2,
                GeometricConstraint::Horizontal(_) => 1,
                GeometricConstraint::Vertical(_) => 1,
                GeometricConstraint::Parallel(_, _) => 1,
                GeometricConstraint::Perpendicular(_, _) => 1,
                GeometricConstraint::Tangent(_, _) => 1,
                GeometricConstraint::Equal(_, _) => 1,
                GeometricConstraint::Symmetric(_, _, _) => 2,
                GeometricConstraint::Concentric(_, _) => 2,
                GeometricConstraint::Collinear(refs) => refs.len().saturating_sub(2) as i32,
                GeometricConstraint::PointOn(_, _) => 1,
                GeometricConstraint::Midpoint(_, _) => 2,
            },
            ConstraintType::Dimensional(d) => match d {
                DimensionalConstraint::Distance(_, _, _) => 1,
                DimensionalConstraint::HorizontalDistance(_, _, _) => 1,
                DimensionalConstraint::VerticalDistance(_, _, _) => 1,
                DimensionalConstraint::Length(_, _) => 1,
                DimensionalConstraint::Angle(_, _, _) => 1,
                DimensionalConstraint::Radius(_, _) => 1,
                DimensionalConstraint::Diameter(_, _) => 1,
            },
        }
    }

    /// Solve constraints.
    pub fn solve(&mut self) -> SolverResult {
        let mut total_error = 0.0;
        let mut iterations = 0;

        // Identify fixed points - these should not be moved by other constraints
        let mut fixed_points: HashMap<usize, Point2<f64>> = HashMap::new();
        for constraint in &self.constraints {
            if let ConstraintType::Geometric(GeometricConstraint::Fixed(
                EntityRef::Point(idx),
                target,
            )) = &constraint.constraint_type
            {
                if constraint.is_driving {
                    fixed_points.insert(*idx, *target);
                }
            }
        }

        for _ in 0..self.max_iterations {
            iterations += 1;
            total_error = 0.0;

            // First, apply fixed constraints directly
            for (&idx, &target) in &fixed_points {
                if idx < self.points.len() {
                    self.points[idx] = target;
                }
            }

            // Evaluate all constraints and compute gradients
            let mut updates: HashMap<usize, Vector2<f64>> = HashMap::new();
            type ConstraintResult = (usize, f64, Vec<(usize, Vector2<f64>)>);
            let mut constraint_results: Vec<ConstraintResult> = Vec::new();

            // First pass: evaluate constraints (immutable borrow)
            for (i, constraint) in self.constraints.iter().enumerate() {
                if !constraint.is_driving {
                    continue;
                }

                let (error, gradient) = self.evaluate_constraint(&constraint.constraint_type);
                constraint_results.push((i, error, gradient));
            }

            // Second pass: update constraints (mutable borrow)
            for (i, error, gradient) in constraint_results {
                self.constraints[i].error = error;
                self.constraints[i].is_satisfied = error < self.tolerance;
                total_error += error * error;

                // Accumulate point updates (skip fixed points)
                for (point_idx, delta) in gradient {
                    if !fixed_points.contains_key(&point_idx) {
                        updates
                            .entry(point_idx)
                            .and_modify(|v| *v += delta)
                            .or_insert(delta);
                    }
                }
            }

            total_error = total_error.sqrt();

            // Check convergence
            if total_error < self.tolerance {
                break;
            }

            // Apply updates with damping
            let damping = 0.5;
            for (idx, delta) in updates {
                if idx < self.points.len() {
                    self.points[idx] += delta * damping;
                }
            }
        }

        let satisfied = self
            .constraints
            .iter()
            .filter(|c| c.is_driving)
            .all(|c| c.is_satisfied);

        SolverResult {
            success: satisfied,
            iterations,
            error: total_error,
            dof: self.degrees_of_freedom(),
        }
    }

    /// Evaluate a constraint and compute gradient.
    fn evaluate_constraint(&self, ct: &ConstraintType) -> (f64, Vec<(usize, Vector2<f64>)>) {
        match ct {
            ConstraintType::Geometric(g) => self.evaluate_geometric(g),
            ConstraintType::Dimensional(d) => self.evaluate_dimensional(d),
        }
    }

    /// Evaluate geometric constraint.
    fn evaluate_geometric(&self, g: &GeometricConstraint) -> (f64, Vec<(usize, Vector2<f64>)>) {
        match g {
            GeometricConstraint::Fixed(EntityRef::Point(idx), target) => {
                let current = self.points.get(*idx).copied().unwrap_or(*target);
                let error = (current - target).norm();
                let delta = target - current;
                (error, vec![(*idx, delta)])
            }
            GeometricConstraint::Coincident(EntityRef::Point(a), EntityRef::Point(b)) => {
                let pa = self.points.get(*a).copied().unwrap_or(Point2::origin());
                let pb = self.points.get(*b).copied().unwrap_or(Point2::origin());
                let error = (pa - pb).norm();
                let delta = (pb - pa) * 0.5;
                (error, vec![(*a, delta), (*b, -delta)])
            }
            GeometricConstraint::Horizontal(EntityRef::Line(a, b)) => {
                let pa = self.points.get(*a).copied().unwrap_or(Point2::origin());
                let pb = self.points.get(*b).copied().unwrap_or(Point2::origin());
                let error = (pa.y - pb.y).abs();
                let mid_y = (pa.y + pb.y) / 2.0;
                let delta_a = Vector2::new(0.0, mid_y - pa.y);
                let delta_b = Vector2::new(0.0, mid_y - pb.y);
                (error, vec![(*a, delta_a), (*b, delta_b)])
            }
            GeometricConstraint::Vertical(EntityRef::Line(a, b)) => {
                let pa = self.points.get(*a).copied().unwrap_or(Point2::origin());
                let pb = self.points.get(*b).copied().unwrap_or(Point2::origin());
                let error = (pa.x - pb.x).abs();
                let mid_x = (pa.x + pb.x) / 2.0;
                let delta_a = Vector2::new(mid_x - pa.x, 0.0);
                let delta_b = Vector2::new(mid_x - pb.x, 0.0);
                (error, vec![(*a, delta_a), (*b, delta_b)])
            }
            GeometricConstraint::Parallel(EntityRef::Line(a1, a2), EntityRef::Line(b1, b2)) => {
                let pa1 = self.points.get(*a1).copied().unwrap_or(Point2::origin());
                let pa2 = self.points.get(*a2).copied().unwrap_or(Point2::origin());
                let pb1 = self.points.get(*b1).copied().unwrap_or(Point2::origin());
                let pb2 = self.points.get(*b2).copied().unwrap_or(Point2::origin());

                let dir_a = (pa2 - pa1).normalize();
                let dir_b = (pb2 - pb1).normalize();

                let cross = dir_a.x * dir_b.y - dir_a.y * dir_b.x;
                let error = cross.abs();

                let angle = dir_b.y.atan2(dir_b.x) - dir_a.y.atan2(dir_a.x);
                let rot_delta = angle * 0.5;

                let len_b = (pb2 - pb1).norm();
                let center_b = Point2::new((pb1.x + pb2.x) / 2.0, (pb1.y + pb2.y) / 2.0);
                let new_dir = Vector2::new(
                    dir_b.x * rot_delta.cos() - dir_b.y * rot_delta.sin(),
                    dir_b.x * rot_delta.sin() + dir_b.y * rot_delta.cos(),
                );

                let new_b1 = center_b - new_dir * (len_b / 2.0);
                let new_b2 = center_b + new_dir * (len_b / 2.0);

                (
                    error,
                    vec![
                        (*b1, Vector2::new(new_b1.x - pb1.x, new_b1.y - pb1.y)),
                        (*b2, Vector2::new(new_b2.x - pb2.x, new_b2.y - pb2.y)),
                    ],
                )
            }
            GeometricConstraint::Perpendicular(
                EntityRef::Line(a1, a2),
                EntityRef::Line(b1, b2),
            ) => {
                let pa1 = self.points.get(*a1).copied().unwrap_or(Point2::origin());
                let pa2 = self.points.get(*a2).copied().unwrap_or(Point2::origin());
                let pb1 = self.points.get(*b1).copied().unwrap_or(Point2::origin());
                let pb2 = self.points.get(*b2).copied().unwrap_or(Point2::origin());

                let dir_a = (pa2 - pa1).normalize();
                let dir_b = (pb2 - pb1).normalize();

                let dot = dir_a.dot(&dir_b);
                let error = dot.abs();

                let target_dir = Vector2::new(-dir_a.y, dir_a.x);
                let sign = if dir_b.dot(&target_dir) >= 0.0 {
                    1.0
                } else {
                    -1.0
                };
                let target_dir = target_dir * sign;

                let len_b = (pb2 - pb1).norm();
                let center_b = Point2::new((pb1.x + pb2.x) / 2.0, (pb1.y + pb2.y) / 2.0);

                let new_b1 = center_b - target_dir * (len_b / 2.0);
                let new_b2 = center_b + target_dir * (len_b / 2.0);

                (
                    error,
                    vec![(*b1, (new_b1 - pb1) * 0.5), (*b2, (new_b2 - pb2) * 0.5)],
                )
            }
            GeometricConstraint::Equal(EntityRef::Line(a1, a2), EntityRef::Line(b1, b2)) => {
                let pa1 = self.points.get(*a1).copied().unwrap_or(Point2::origin());
                let pa2 = self.points.get(*a2).copied().unwrap_or(Point2::origin());
                let pb1 = self.points.get(*b1).copied().unwrap_or(Point2::origin());
                let pb2 = self.points.get(*b2).copied().unwrap_or(Point2::origin());

                let len_a = (pa2 - pa1).norm();
                let len_b = (pb2 - pb1).norm();
                let error = (len_a - len_b).abs();

                if len_b < 1e-10 {
                    return (error, vec![]);
                }

                let dir_b = (pb2 - pb1) / len_b;
                let delta = dir_b * (len_a - len_b) * 0.5;

                (error, vec![(*b1, -delta), (*b2, delta)])
            }
            GeometricConstraint::Midpoint(EntityRef::Point(mid), EntityRef::Line(a, b)) => {
                let pmid = self.points.get(*mid).copied().unwrap_or(Point2::origin());
                let pa = self.points.get(*a).copied().unwrap_or(Point2::origin());
                let pb = self.points.get(*b).copied().unwrap_or(Point2::origin());

                let actual_mid = Point2::new((pa.x + pb.x) / 2.0, (pa.y + pb.y) / 2.0);
                let error = (pmid - actual_mid).norm();
                let delta = actual_mid - pmid;

                (error, vec![(*mid, delta)])
            }
            GeometricConstraint::PointOn(EntityRef::Point(p), EntityRef::Line(a, b)) => {
                let pp = self.points.get(*p).copied().unwrap_or(Point2::origin());
                let pa = self.points.get(*a).copied().unwrap_or(Point2::origin());
                let pb = self.points.get(*b).copied().unwrap_or(Point2::origin());

                let line_dir = pb - pa;
                let line_len = line_dir.norm();
                if line_len < 1e-10 {
                    return (0.0, vec![]);
                }

                let line_dir = line_dir / line_len;
                let to_point = pp - pa;
                let proj = line_dir * to_point.dot(&line_dir);
                let closest = pa + proj;
                let error = (pp - closest).norm();
                let delta = closest - pp;

                (error, vec![(*p, delta)])
            }
            _ => (0.0, vec![]),
        }
    }

    /// Evaluate dimensional constraint.
    fn evaluate_dimensional(&self, d: &DimensionalConstraint) -> (f64, Vec<(usize, Vector2<f64>)>) {
        match d {
            DimensionalConstraint::Distance(EntityRef::Point(a), EntityRef::Point(b), target) => {
                let pa = self.points.get(*a).copied().unwrap_or(Point2::origin());
                let pb = self.points.get(*b).copied().unwrap_or(Point2::origin());
                let current = (pa - pb).norm();
                let error = (current - target).abs();

                if current < 1e-10 {
                    return (error, vec![]);
                }

                let dir = (pb - pa) / current;
                let delta = dir * (target - current) * 0.5;

                (error, vec![(*a, -delta), (*b, delta)])
            }
            DimensionalConstraint::HorizontalDistance(
                EntityRef::Point(a),
                EntityRef::Point(b),
                target,
            ) => {
                let pa = self.points.get(*a).copied().unwrap_or(Point2::origin());
                let pb = self.points.get(*b).copied().unwrap_or(Point2::origin());
                let current = (pb.x - pa.x).abs();
                let error = (current - target).abs();

                let sign = if pb.x >= pa.x { 1.0 } else { -1.0 };
                let delta_x = (target - current) * sign * 0.5;

                (
                    error,
                    vec![
                        (*a, Vector2::new(-delta_x, 0.0)),
                        (*b, Vector2::new(delta_x, 0.0)),
                    ],
                )
            }
            DimensionalConstraint::VerticalDistance(
                EntityRef::Point(a),
                EntityRef::Point(b),
                target,
            ) => {
                let pa = self.points.get(*a).copied().unwrap_or(Point2::origin());
                let pb = self.points.get(*b).copied().unwrap_or(Point2::origin());
                let current = (pb.y - pa.y).abs();
                let error = (current - target).abs();

                let sign = if pb.y >= pa.y { 1.0 } else { -1.0 };
                let delta_y = (target - current) * sign * 0.5;

                (
                    error,
                    vec![
                        (*a, Vector2::new(0.0, -delta_y)),
                        (*b, Vector2::new(0.0, delta_y)),
                    ],
                )
            }
            DimensionalConstraint::Length(EntityRef::Line(a, b), target) => {
                let pa = self.points.get(*a).copied().unwrap_or(Point2::origin());
                let pb = self.points.get(*b).copied().unwrap_or(Point2::origin());
                let current = (pa - pb).norm();
                let error = (current - target).abs();

                if current < 1e-10 {
                    return (error, vec![]);
                }

                let dir = (pb - pa) / current;
                let delta = dir * (target - current) * 0.5;

                (error, vec![(*a, -delta), (*b, delta)])
            }
            DimensionalConstraint::Angle(
                EntityRef::Line(a1, a2),
                EntityRef::Line(b1, b2),
                target,
            ) => {
                let pa1 = self.points.get(*a1).copied().unwrap_or(Point2::origin());
                let pa2 = self.points.get(*a2).copied().unwrap_or(Point2::origin());
                let pb1 = self.points.get(*b1).copied().unwrap_or(Point2::origin());
                let pb2 = self.points.get(*b2).copied().unwrap_or(Point2::origin());

                let dir_a = (pa2 - pa1).normalize();
                let dir_b = (pb2 - pb1).normalize();

                let current_angle = (dir_a.y.atan2(dir_a.x) - dir_b.y.atan2(dir_b.x)).abs();
                let error = (current_angle - target).abs();

                let angle_diff = target - current_angle;
                let len_b = (pb2 - pb1).norm();
                let center_b = Point2::new((pb1.x + pb2.x) / 2.0, (pb1.y + pb2.y) / 2.0);

                let new_angle = dir_b.y.atan2(dir_b.x) + angle_diff * 0.5;
                let new_dir = Vector2::new(new_angle.cos(), new_angle.sin());

                let new_b1 = center_b - new_dir * (len_b / 2.0);
                let new_b2 = center_b + new_dir * (len_b / 2.0);

                (error, vec![(*b1, new_b1 - pb1), (*b2, new_b2 - pb2)])
            }
            DimensionalConstraint::Radius(EntityRef::Circle(center), target) => {
                // For simplicity, we store radius in a separate data structure
                // This is a placeholder
                let _ = (center, target);
                (0.0, vec![])
            }
            DimensionalConstraint::Diameter(EntityRef::Circle(center), target) => {
                let _ = (center, target);
                (0.0, vec![])
            }
            _ => (0.0, vec![]),
        }
    }

    /// Check if sketch is fully constrained.
    pub fn is_fully_constrained(&self) -> bool {
        self.degrees_of_freedom() == 0
    }

    /// Check if sketch is over-constrained.
    pub fn is_over_constrained(&self) -> bool {
        self.degrees_of_freedom() < 0
    }

    /// Get constraint by ID.
    pub fn get_constraint(&self, id: ConstraintId) -> Option<&Constraint> {
        self.constraints.iter().find(|c| c.id == id)
    }

    /// Toggle constraint driving status.
    pub fn set_driving(&mut self, id: ConstraintId, driving: bool) {
        if let Some(c) = self.constraints.iter_mut().find(|c| c.id == id) {
            c.is_driving = driving;
        }
    }

    /// Get all unsatisfied constraints.
    pub fn unsatisfied_constraints(&self) -> Vec<ConstraintId> {
        self.constraints
            .iter()
            .filter(|c| c.is_driving && !c.is_satisfied)
            .map(|c| c.id)
            .collect()
    }
}

/// Solver result.
#[derive(Debug, Clone)]
pub struct SolverResult {
    /// Whether solving succeeded.
    pub success: bool,
    /// Number of iterations used.
    pub iterations: usize,
    /// Final error.
    pub error: f64,
    /// Degrees of freedom remaining.
    pub dof: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_constraint() {
        let mut solver = ConstraintSolver::new();

        let p0 = solver.add_point(Point2::new(0.0, 0.0));
        let target = Point2::new(5.0, 3.0);

        solver.add_geometric(GeometricConstraint::Fixed(EntityRef::Point(p0), target));

        let result = solver.solve();

        assert!(result.success);
        assert!((solver.points[p0] - target).norm() < 1e-6);
    }

    #[test]
    fn test_coincident_constraint() {
        let mut solver = ConstraintSolver::new();

        let p0 = solver.add_point(Point2::new(0.0, 0.0));
        let p1 = solver.add_point(Point2::new(10.0, 10.0));

        solver.add_geometric(GeometricConstraint::Coincident(
            EntityRef::Point(p0),
            EntityRef::Point(p1),
        ));

        let result = solver.solve();

        assert!(result.success);
        assert!((solver.points[p0] - solver.points[p1]).norm() < 1e-6);
    }

    #[test]
    fn test_horizontal_constraint() {
        let mut solver = ConstraintSolver::new();

        let p0 = solver.add_point(Point2::new(0.0, 0.0));
        let p1 = solver.add_point(Point2::new(10.0, 5.0));

        solver.add_geometric(GeometricConstraint::Horizontal(EntityRef::Line(p0, p1)));

        let result = solver.solve();

        assert!(result.success);
        assert!((solver.points[p0].y - solver.points[p1].y).abs() < 1e-6);
    }

    #[test]
    fn test_distance_constraint() {
        let mut solver = ConstraintSolver::new();

        let p0 = solver.add_point(Point2::new(0.0, 0.0));
        let p1 = solver.add_point(Point2::new(1.0, 0.0));
        let target_dist = 5.0;

        solver.add_geometric(GeometricConstraint::Fixed(
            EntityRef::Point(p0),
            Point2::origin(),
        ));
        solver.add_dimensional(DimensionalConstraint::Distance(
            EntityRef::Point(p0),
            EntityRef::Point(p1),
            target_dist,
        ));

        let result = solver.solve();

        assert!(result.success);
        let actual_dist = (solver.points[p1] - solver.points[p0]).norm();
        assert!((actual_dist - target_dist).abs() < 1e-6);
    }

    #[test]
    fn test_degrees_of_freedom() {
        let mut solver = ConstraintSolver::new();

        let p0 = solver.add_point(Point2::new(0.0, 0.0));
        let p1 = solver.add_point(Point2::new(10.0, 10.0));

        assert_eq!(solver.degrees_of_freedom(), 4);

        solver.add_geometric(GeometricConstraint::Fixed(
            EntityRef::Point(p0),
            Point2::origin(),
        ));
        assert_eq!(solver.degrees_of_freedom(), 2);

        solver.add_geometric(GeometricConstraint::Horizontal(EntityRef::Line(p0, p1)));
        assert_eq!(solver.degrees_of_freedom(), 1);

        solver.add_dimensional(DimensionalConstraint::Distance(
            EntityRef::Point(p0),
            EntityRef::Point(p1),
            5.0,
        ));
        assert_eq!(solver.degrees_of_freedom(), 0);
        assert!(solver.is_fully_constrained());
    }
}
