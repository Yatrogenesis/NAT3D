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

//! Boundary conditions for fluid simulation.

use nalgebra::Vector3;

/// Boundary condition type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryType {
    /// No-slip boundary (velocity = 0 at boundary).
    NoSlip,
    /// Free-slip boundary (tangential velocity allowed).
    FreeSlip,
    /// Outflow boundary (extrapolation).
    Outflow,
    /// Inflow boundary (specified velocity).
    Inflow,
    /// Periodic boundary.
    Periodic,
}

/// Boundary condition.
#[derive(Debug, Clone)]
pub struct BoundaryCondition {
    /// Type of boundary.
    pub boundary_type: BoundaryType,
    /// Inflow velocity (for Inflow boundary).
    pub velocity: Vector3<f64>,
}

impl Default for BoundaryCondition {
    fn default() -> Self {
        Self {
            boundary_type: BoundaryType::NoSlip,
            velocity: Vector3::zeros(),
        }
    }
}

impl BoundaryCondition {
    /// Create no-slip boundary.
    pub fn no_slip() -> Self {
        Self {
            boundary_type: BoundaryType::NoSlip,
            ..Default::default()
        }
    }

    /// Create free-slip boundary.
    pub fn free_slip() -> Self {
        Self {
            boundary_type: BoundaryType::FreeSlip,
            ..Default::default()
        }
    }

    /// Create outflow boundary.
    pub fn outflow() -> Self {
        Self {
            boundary_type: BoundaryType::Outflow,
            ..Default::default()
        }
    }

    /// Create inflow boundary with specified velocity.
    pub fn inflow(velocity: Vector3<f64>) -> Self {
        Self {
            boundary_type: BoundaryType::Inflow,
            velocity,
        }
    }

    /// Apply boundary condition to velocity field.
    pub fn apply(&self, velocity: &mut Vector3<f64>, normal: &Vector3<f64>) {
        match self.boundary_type {
            BoundaryType::NoSlip => {
                // Zero velocity at boundary
                *velocity = Vector3::zeros();
            }
            BoundaryType::FreeSlip => {
                // Zero normal component, preserve tangential
                let normal_vel = velocity.dot(normal) * normal;
                *velocity -= normal_vel;
            }
            BoundaryType::Outflow => {
                // Extrapolate (no change needed, handled by solver)
            }
            BoundaryType::Inflow => {
                // Set to specified inflow velocity
                *velocity = self.velocity;
            }
            BoundaryType::Periodic => {
                // Handled by grid indexing, no modification needed
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_slip() {
        let bc = BoundaryCondition::no_slip();
        let mut vel = Vector3::new(1.0, 2.0, 3.0);
        let normal = Vector3::new(0.0, 1.0, 0.0);
        bc.apply(&mut vel, &normal);
        assert_eq!(vel, Vector3::zeros());
    }

    #[test]
    fn test_free_slip() {
        let bc = BoundaryCondition::free_slip();
        let mut vel = Vector3::new(1.0, 2.0, 3.0);
        let normal = Vector3::new(0.0, 1.0, 0.0);
        bc.apply(&mut vel, &normal);
        // Should remove normal component (y=2.0)
        assert!((vel.x - 1.0).abs() < 1e-10);
        assert!(vel.y.abs() < 1e-10);
        assert!((vel.z - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_inflow() {
        let bc = BoundaryCondition::inflow(Vector3::new(5.0, 0.0, 0.0));
        let mut vel = Vector3::new(1.0, 2.0, 3.0);
        let normal = Vector3::new(1.0, 0.0, 0.0);
        bc.apply(&mut vel, &normal);
        assert_eq!(vel, Vector3::new(5.0, 0.0, 0.0));
    }
}
