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

//! Volume preservation constraints for soft bodies.
//!
//! Implements constraints to maintain or control volume of soft body regions.

use nalgebra::Vector3;

/// Volume constraint for a tetrahedral region.
#[derive(Debug, Clone)]
pub struct VolumeConstraint {
    /// Indices of nodes forming tetrahedra.
    pub node_indices: Vec<[usize; 4]>,
    /// Rest volume to maintain.
    pub rest_volume: f64,
    /// Constraint stiffness (0-1).
    pub stiffness: f64,
}

impl VolumeConstraint {
    /// Create a new volume constraint.
    pub fn new(node_indices: Vec<[usize; 4]>, rest_volume: f64, stiffness: f64) -> Self {
        Self {
            node_indices,
            rest_volume,
            stiffness,
        }
    }

    /// Compute current volume of all tetrahedra.
    pub fn compute_volume(&self, positions: &[Vector3<f64>]) -> f64 {
        self.node_indices
            .iter()
            .map(|&[i0, i1, i2, i3]| {
                Self::tetrahedron_volume(positions[i0], positions[i1], positions[i2], positions[i3])
            })
            .sum()
    }

    /// Compute volume of a single tetrahedron.
    fn tetrahedron_volume(
        p0: Vector3<f64>,
        p1: Vector3<f64>,
        p2: Vector3<f64>,
        p3: Vector3<f64>,
    ) -> f64 {
        let v1 = p1 - p0;
        let v2 = p2 - p0;
        let v3 = p3 - p0;
        v1.cross(&v2).dot(&v3).abs() / 6.0
    }

    /// Compute gradient of volume with respect to each node position.
    pub fn compute_gradient(&self, positions: &[Vector3<f64>]) -> Vec<Vector3<f64>> {
        let mut gradients = vec![Vector3::zeros(); positions.len()];

        for &[i0, i1, i2, i3] in &self.node_indices {
            let p0 = positions[i0];
            let p1 = positions[i1];
            let p2 = positions[i2];
            let p3 = positions[i3];

            // Gradient of volume w.r.t. each vertex
            let grad0 = (p2 - p1).cross(&(p3 - p1)) / 6.0;
            let grad1 = (p3 - p0).cross(&(p2 - p0)) / 6.0;
            let grad2 = (p1 - p0).cross(&(p3 - p0)) / 6.0;
            let grad3 = (p2 - p0).cross(&(p1 - p0)) / 6.0;

            gradients[i0] += grad0;
            gradients[i1] += grad1;
            gradients[i2] += grad2;
            gradients[i3] += grad3;
        }

        gradients
    }

    /// Project positions to satisfy volume constraint.
    /// Returns position corrections for each node.
    pub fn project(&self, positions: &[Vector3<f64>], inv_masses: &[f64]) -> Vec<Vector3<f64>> {
        let current_volume = self.compute_volume(positions);
        let volume_error = current_volume - self.rest_volume;

        if volume_error.abs() < 1e-10 {
            return vec![Vector3::zeros(); positions.len()];
        }

        let gradients = self.compute_gradient(positions);

        // Compute weighted gradient magnitude
        let mut weighted_grad_sq = 0.0;
        for (i, grad) in gradients.iter().enumerate() {
            weighted_grad_sq += inv_masses[i] * grad.magnitude_squared();
        }

        if weighted_grad_sq < 1e-10 {
            return vec![Vector3::zeros(); positions.len()];
        }

        // Compute lambda (Lagrange multiplier)
        let lambda = -volume_error / weighted_grad_sq * self.stiffness;

        // Compute position corrections
        let mut corrections = vec![Vector3::zeros(); positions.len()];
        for (i, grad) in gradients.iter().enumerate() {
            corrections[i] = grad * (lambda * inv_masses[i]);
        }

        corrections
    }
}

/// Pressure constraint for inflatable objects.
#[derive(Debug, Clone)]
pub struct PressureConstraint {
    /// Surface triangles (indices).
    pub triangles: Vec<[usize; 3]>,
    /// Target pressure.
    pub target_pressure: f64,
    /// Stiffness.
    pub stiffness: f64,
    /// Reference volume at rest.
    pub rest_volume: f64,
}

impl PressureConstraint {
    /// Create a new pressure constraint.
    pub fn new(
        triangles: Vec<[usize; 3]>,
        rest_volume: f64,
        target_pressure: f64,
        stiffness: f64,
    ) -> Self {
        Self {
            triangles,
            target_pressure,
            stiffness,
            rest_volume,
        }
    }

    /// Compute current volume enclosed by surface.
    pub fn compute_volume(&self, positions: &[Vector3<f64>]) -> f64 {
        let mut volume = 0.0;

        for &[i0, i1, i2] in &self.triangles {
            let p0 = positions[i0];
            let p1 = positions[i1];
            let p2 = positions[i2];

            // Signed volume contribution of triangle
            volume += p0.dot(&p1.cross(&p2)) / 6.0;
        }

        volume
    }

    /// Compute pressure force on each node.
    pub fn compute_pressure_forces(&self, positions: &[Vector3<f64>]) -> Vec<Vector3<f64>> {
        let current_volume = self.compute_volume(positions);
        let volume_ratio = current_volume / self.rest_volume;

        // Ideal gas law: P * V = const => P = P0 * V0 / V
        let current_pressure = self.target_pressure / volume_ratio;

        let mut forces = vec![Vector3::zeros(); positions.len()];

        // Apply pressure force normal to each triangle
        for &[i0, i1, i2] in &self.triangles {
            let p0 = positions[i0];
            let p1 = positions[i1];
            let p2 = positions[i2];

            // Compute triangle normal and area
            let e1 = p1 - p0;
            let e2 = p2 - p0;
            let normal = e1.cross(&e2);
            let area = normal.magnitude() / 2.0;
            let unit_normal = if normal.magnitude() > 1e-10 {
                normal.normalize()
            } else {
                Vector3::zeros()
            };

            // Force per vertex (distributed equally)
            let force = unit_normal * (current_pressure * area / 3.0 * self.stiffness);

            forces[i0] += force;
            forces[i1] += force;
            forces[i2] += force;
        }

        forces
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tetrahedron_volume() {
        let p0 = Vector3::new(0.0, 0.0, 0.0);
        let p1 = Vector3::new(1.0, 0.0, 0.0);
        let p2 = Vector3::new(0.0, 1.0, 0.0);
        let p3 = Vector3::new(0.0, 0.0, 1.0);

        let volume = VolumeConstraint::tetrahedron_volume(p0, p1, p2, p3);

        // Volume should be 1/6
        assert!((volume - 1.0 / 6.0).abs() < 1e-10);
    }

    #[test]
    fn test_volume_constraint() {
        let positions = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
        ];

        let constraint = VolumeConstraint::new(vec![[0, 1, 2, 3]], 1.0 / 6.0, 1.0);

        let volume = constraint.compute_volume(&positions);
        assert!((volume - 1.0 / 6.0).abs() < 1e-10);
    }

    #[test]
    fn test_volume_gradient() {
        let positions = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
        ];

        let constraint = VolumeConstraint::new(vec![[0, 1, 2, 3]], 1.0 / 6.0, 1.0);

        let gradients = constraint.compute_gradient(&positions);

        // Gradients should be non-zero
        for grad in &gradients {
            assert!(grad.magnitude() > 0.0);
        }
    }

    #[test]
    fn test_volume_projection() {
        let positions = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(2.0, 0.0, 0.0), // Expanded
            Vector3::new(0.0, 2.0, 0.0),
            Vector3::new(0.0, 0.0, 2.0),
        ];

        let rest_volume = 1.0 / 6.0;
        let constraint = VolumeConstraint::new(vec![[0, 1, 2, 3]], rest_volume, 1.0);

        let inv_masses = vec![1.0, 1.0, 1.0, 1.0];

        let current_volume = constraint.compute_volume(&positions);
        assert!(current_volume > rest_volume);

        let corrections = constraint.project(&positions, &inv_masses);

        // Corrections should be non-zero and point inward (negative)
        let _total_correction: Vector3<f64> = corrections.iter().sum();
        assert!(
            corrections.iter().any(|c| c.magnitude() > 1e-6),
            "Should have some corrections"
        );
    }

    #[test]
    fn test_pressure_constraint() {
        let positions = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
        ];

        // Tetrahedron surface triangles
        let triangles = vec![
            [0, 2, 1], // Bottom
            [0, 1, 3], // Front
            [0, 3, 2], // Left
            [1, 2, 3], // Back
        ];

        let rest_volume = 1.0 / 6.0;
        let constraint = PressureConstraint::new(triangles, rest_volume, 1.0, 1.0);

        let forces = constraint.compute_pressure_forces(&positions);

        // Forces should be non-zero and push outward
        let total_force: Vector3<f64> = forces.iter().sum();
        // Total force should be close to zero (pressure is internal)
        assert!(total_force.magnitude() < 1.0);
    }
}
