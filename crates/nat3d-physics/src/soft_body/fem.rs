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

//! Finite Element Method for soft bodies.
//!
//! Implements tetrahedral FEM with Neo-Hookean material model.
//! Uses corotational formulation for large deformations.

use nalgebra::{Matrix3, Vector3};
use serde::{Deserialize, Serialize};

/// Material properties for FEM.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FemMaterial {
    /// Young's modulus (stiffness).
    pub youngs_modulus: f64,
    /// Poisson's ratio (0 = no volume change, 0.5 = incompressible).
    pub poisson_ratio: f64,
    /// Damping coefficient.
    pub damping: f64,
    /// Density.
    pub density: f64,
}

impl FemMaterial {
    /// Create a new material.
    pub fn new(youngs_modulus: f64, poisson_ratio: f64) -> Self {
        Self {
            youngs_modulus,
            poisson_ratio,
            damping: 0.01,
            density: 1000.0,
        }
    }

    /// Compute Lame parameters.
    pub fn lame_lambda(&self) -> f64 {
        (self.youngs_modulus * self.poisson_ratio)
            / ((1.0 + self.poisson_ratio) * (1.0 - 2.0 * self.poisson_ratio))
    }

    /// Compute shear modulus (Lame mu).
    pub fn lame_mu(&self) -> f64 {
        self.youngs_modulus / (2.0 * (1.0 + self.poisson_ratio))
    }
}

impl Default for FemMaterial {
    fn default() -> Self {
        Self::new(1e6, 0.3)
    }
}

/// A node in the FEM mesh.
#[derive(Debug, Clone)]
pub struct FemNode {
    /// Current position.
    pub position: Vector3<f64>,
    /// Velocity.
    pub velocity: Vector3<f64>,
    /// Accumulated force.
    pub force: Vector3<f64>,
    /// Mass.
    pub mass: f64,
    /// Rest position.
    pub rest_position: Vector3<f64>,
    /// Is this node fixed?
    pub fixed: bool,
}

impl FemNode {
    /// Create a new node.
    pub fn new(position: Vector3<f64>, mass: f64) -> Self {
        Self {
            position,
            velocity: Vector3::zeros(),
            force: Vector3::zeros(),
            mass,
            rest_position: position,
            fixed: false,
        }
    }

    /// Create a fixed node.
    pub fn fixed(position: Vector3<f64>) -> Self {
        Self {
            position,
            velocity: Vector3::zeros(),
            force: Vector3::zeros(),
            mass: f64::INFINITY,
            rest_position: position,
            fixed: true,
        }
    }
}

/// A tetrahedral element.
#[derive(Debug, Clone)]
pub struct TetraElement {
    /// Indices of the four nodes.
    pub node_indices: [usize; 4],
    /// Rest volume of the tetrahedron.
    pub rest_volume: f64,
    /// Inverse of the rest shape matrix Dm.
    pub dm_inv: Matrix3<f64>,
    /// Material properties.
    pub material: FemMaterial,
}

impl TetraElement {
    /// Create a new tetrahedral element.
    pub fn new(
        node_indices: [usize; 4],
        rest_positions: [Vector3<f64>; 4],
        material: FemMaterial,
    ) -> Self {
        // Compute rest shape matrix Dm = [x1-x0, x2-x0, x3-x0]
        let dm = Matrix3::from_columns(&[
            rest_positions[1] - rest_positions[0],
            rest_positions[2] - rest_positions[0],
            rest_positions[3] - rest_positions[0],
        ]);

        let dm_inv = dm.try_inverse().expect("Degenerate tetrahedron");

        // Compute rest volume
        let rest_volume = dm.determinant().abs() / 6.0;

        Self {
            node_indices,
            rest_volume,
            dm_inv,
            material,
        }
    }

    /// Compute deformation gradient F = Ds * Dm^-1.
    pub fn compute_deformation_gradient(&self, positions: &[Vector3<f64>]) -> Matrix3<f64> {
        let p0 = positions[self.node_indices[0]];
        let p1 = positions[self.node_indices[1]];
        let p2 = positions[self.node_indices[2]];
        let p3 = positions[self.node_indices[3]];

        // Deformed shape matrix Ds
        let ds = Matrix3::from_columns(&[p1 - p0, p2 - p0, p3 - p0]);

        ds * self.dm_inv
    }

    /// Compute first Piola-Kirchhoff stress using Neo-Hookean model.
    /// P = F * S where S is second PK stress.
    pub fn compute_stress(&self, f: &Matrix3<f64>) -> Matrix3<f64> {
        let lambda = self.material.lame_lambda();
        let mu = self.material.lame_mu();

        let j = f.determinant();

        if j <= 0.0 {
            // Handle inverted elements
            return Matrix3::zeros();
        }

        let f_t = f.transpose();
        let f_inv_t = f_t.try_inverse().unwrap_or(Matrix3::zeros());

        // Neo-Hookean: P = mu * (F - F^-T) + lambda * ln(J) * F^-T

        f.scale(mu) - f_inv_t.scale(mu) + f_inv_t.scale(lambda * j.ln())
    }

    /// Compute forces on nodes using the finite element method.
    pub fn compute_forces(&self, positions: &[Vector3<f64>]) -> [Vector3<f64>; 4] {
        let f = self.compute_deformation_gradient(positions);
        let p = self.compute_stress(&f);

        // Force = -volume * P * Dm^-T * [H0, H1, H2, H3]
        // where Hi are the shape function gradients in rest config
        let h = self.dm_inv.transpose();

        let force_scale = -self.rest_volume;

        let f0 = force_scale * p * (-h.column(0) - h.column(1) - h.column(2));
        let f1 = force_scale * p * h.column(0);
        let f2 = force_scale * p * h.column(1);
        let f3 = force_scale * p * h.column(2);

        [f0, f1, f2, f3]
    }
}

/// FEM soft body solver.
#[derive(Debug, Clone)]
pub struct FemSolver {
    /// All nodes in the mesh.
    pub nodes: Vec<FemNode>,
    /// All tetrahedral elements.
    pub elements: Vec<TetraElement>,
    /// Gravity vector.
    pub gravity: Vector3<f64>,
    /// Simulation time.
    pub time: f64,
}

impl FemSolver {
    /// Create a new FEM solver.
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            elements: Vec::new(),
            gravity: Vector3::new(0.0, -9.81, 0.0),
            time: 0.0,
        }
    }

    /// Add a node to the system.
    pub fn add_node(&mut self, node: FemNode) -> usize {
        let idx = self.nodes.len();
        self.nodes.push(node);
        idx
    }

    /// Add an element to the system.
    pub fn add_element(&mut self, element: TetraElement) {
        self.elements.push(element);
    }

    /// Compute element forces and assemble into global force vector.
    fn assemble_forces(&mut self) {
        // Clear forces
        for node in &mut self.nodes {
            node.force = Vector3::zeros();
        }

        // Get node positions
        let positions: Vec<_> = self.nodes.iter().map(|n| n.position).collect();

        // Compute and accumulate element forces
        for element in &self.elements {
            let forces = element.compute_forces(&positions);

            for (i, &node_idx) in element.node_indices.iter().enumerate() {
                self.nodes[node_idx].force += forces[i];
            }
        }

        // Add gravity
        for node in &mut self.nodes {
            if !node.fixed {
                node.force += self.gravity * node.mass;
            }
        }
    }

    /// Integrate using semi-implicit Euler.
    fn integrate(&mut self, dt: f64) {
        for node in &mut self.nodes {
            if node.fixed {
                continue;
            }

            let inv_mass = if node.mass > 0.0 {
                1.0 / node.mass
            } else {
                0.0
            };

            // v(t+dt) = v(t) + a(t) * dt
            let acceleration = node.force * inv_mass;
            node.velocity += acceleration * dt;

            // Apply damping
            let material = self
                .elements
                .first()
                .map(|e| e.material)
                .unwrap_or_default();
            node.velocity *= 1.0 - material.damping;

            // x(t+dt) = x(t) + v(t+dt) * dt
            node.position += node.velocity * dt;
        }
    }

    /// Step the simulation forward.
    pub fn step(&mut self, dt: f64) {
        self.assemble_forces();
        self.integrate(dt);
        self.time += dt;
    }

    /// Create a single tetrahedron FEM system.
    pub fn create_tetrahedron(vertices: [Vector3<f64>; 4], material: FemMaterial) -> Self {
        let mut solver = Self::new();

        // Compute mass for each vertex
        let positions = vertices;
        let dm = Matrix3::from_columns(&[
            positions[1] - positions[0],
            positions[2] - positions[0],
            positions[3] - positions[0],
        ]);
        let volume = dm.determinant().abs() / 6.0;
        let total_mass = material.density * volume;
        let node_mass = total_mass / 4.0;

        // Add nodes
        for pos in &vertices {
            let node = FemNode::new(*pos, node_mass);
            solver.add_node(node);
        }

        // Add element
        let element = TetraElement::new([0, 1, 2, 3], vertices, material);
        solver.add_element(element);

        solver
    }

    /// Create a cube subdivided into tetrahedra.
    pub fn create_cube(
        center: Vector3<f64>,
        size: f64,
        subdivisions: usize,
        material: FemMaterial,
    ) -> Self {
        let mut solver = Self::new();

        let h = size / (subdivisions as f64);
        let half_size = size / 2.0;

        // Create nodes
        let mut node_map = std::collections::HashMap::new();
        let _nx = subdivisions + 1;
        let _ny = subdivisions + 1;
        let _nz = subdivisions + 1;

        for k in 0..=subdivisions {
            for j in 0..=subdivisions {
                for i in 0..=subdivisions {
                    let pos = center
                        + Vector3::new(
                            i as f64 * h - half_size,
                            j as f64 * h - half_size,
                            k as f64 * h - half_size,
                        );

                    let node = FemNode::new(pos, 1.0); // Mass will be computed later
                    let idx = solver.add_node(node);
                    node_map.insert((i, j, k), idx);
                }
            }
        }

        // Create tetrahedra (5 per cube cell)
        for k in 0..subdivisions {
            for j in 0..subdivisions {
                for i in 0..subdivisions {
                    let v000 = node_map[&(i, j, k)];
                    let v100 = node_map[&(i + 1, j, k)];
                    let v010 = node_map[&(i, j + 1, k)];
                    let v110 = node_map[&(i + 1, j + 1, k)];
                    let v001 = node_map[&(i, j, k + 1)];
                    let v101 = node_map[&(i + 1, j, k + 1)];
                    let v011 = node_map[&(i, j + 1, k + 1)];
                    let v111 = node_map[&(i + 1, j + 1, k + 1)];

                    // Subdivide cube into 5 tetrahedra
                    let tets = [
                        [v000, v100, v110, v101],
                        [v000, v110, v010, v011],
                        [v000, v101, v001, v011],
                        [v110, v101, v111, v011],
                        [v000, v110, v101, v011],
                    ];

                    for tet in &tets {
                        let positions = [
                            solver.nodes[tet[0]].position,
                            solver.nodes[tet[1]].position,
                            solver.nodes[tet[2]].position,
                            solver.nodes[tet[3]].position,
                        ];

                        let element = TetraElement::new(*tet, positions, material);
                        solver.add_element(element);
                    }
                }
            }
        }

        // Compute masses from elements
        let mut masses = vec![0.0; solver.nodes.len()];
        for element in &solver.elements {
            let tet_mass = material.density * element.rest_volume;
            let node_mass = tet_mass / 4.0;
            for &idx in &element.node_indices {
                masses[idx] += node_mass;
            }
        }

        for (i, node) in solver.nodes.iter_mut().enumerate() {
            node.mass = masses[i];
        }

        solver
    }

    /// Get total kinetic energy.
    pub fn kinetic_energy(&self) -> f64 {
        self.nodes
            .iter()
            .filter(|n| !n.fixed)
            .map(|n| 0.5 * n.mass * n.velocity.magnitude_squared())
            .sum()
    }

    /// Get total elastic potential energy.
    pub fn potential_energy(&self) -> f64 {
        let positions: Vec<_> = self.nodes.iter().map(|n| n.position).collect();

        self.elements
            .iter()
            .map(|element| {
                let f = element.compute_deformation_gradient(&positions);
                let j = f.determinant();

                if j <= 0.0 {
                    return 0.0;
                }

                let lambda = element.material.lame_lambda();
                let mu = element.material.lame_mu();

                // Neo-Hookean energy density
                let i1 = (f.transpose() * f).trace();
                let ln_j = j.ln();

                let energy = 0.5 * mu * (i1 - 3.0) - mu * ln_j + 0.5 * lambda * ln_j * ln_j;

                energy * element.rest_volume
            })
            .sum()
    }
}

impl Default for FemSolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_material_creation() {
        let mat = FemMaterial::new(1e6, 0.3);
        assert_eq!(mat.youngs_modulus, 1e6);
        assert_eq!(mat.poisson_ratio, 0.3);

        let lambda = mat.lame_lambda();
        let mu = mat.lame_mu();
        assert!(lambda > 0.0);
        assert!(mu > 0.0);
    }

    #[test]
    fn test_node_creation() {
        let node = FemNode::new(Vector3::new(1.0, 2.0, 3.0), 1.5);
        assert_eq!(node.position, Vector3::new(1.0, 2.0, 3.0));
        assert_eq!(node.mass, 1.5);
        assert!(!node.fixed);
    }

    #[test]
    fn test_tetrahedron_creation() {
        let vertices = [
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
        ];

        let material = FemMaterial::default();
        let tet = TetraElement::new([0, 1, 2, 3], vertices, material);

        // Volume of this tetrahedron is 1/6
        assert!((tet.rest_volume - 1.0 / 6.0).abs() < 1e-10);
    }

    #[test]
    fn test_deformation_gradient() {
        let rest_vertices = [
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
        ];

        let material = FemMaterial::default();
        let tet = TetraElement::new([0, 1, 2, 3], rest_vertices, material);

        // Test identity deformation (no deformation)
        let f = tet.compute_deformation_gradient(&rest_vertices);
        assert!((f - Matrix3::identity()).norm() < 1e-10);
    }

    #[test]
    fn test_solver_creation() {
        let solver = FemSolver::new();
        assert_eq!(solver.nodes.len(), 0);
        assert_eq!(solver.elements.len(), 0);
        assert_eq!(solver.time, 0.0);
    }

    #[test]
    fn test_single_tetrahedron() {
        let vertices = [
            Vector3::new(0.0, 1.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(-1.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
        ];

        let material = FemMaterial::new(1e5, 0.3);
        let mut solver = FemSolver::create_tetrahedron(vertices, material);

        // Fix bottom nodes
        solver.nodes[1].fixed = true;
        solver.nodes[2].fixed = true;
        solver.nodes[3].fixed = true;

        let initial_height = solver.nodes[0].position.y;

        // Step simulation
        for _ in 0..10 {
            solver.step(0.001);
        }

        let final_height = solver.nodes[0].position.y;

        // Top node should fall
        assert!(final_height < initial_height);
    }

    #[test]
    fn test_material_response() {
        let vertices = [
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
        ];

        let material = FemMaterial::new(1e6, 0.3);
        let tet = TetraElement::new([0, 1, 2, 3], vertices, material);

        // Compress the tetrahedron
        let deformed = [
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(0.8, 0.0, 0.0),
            Vector3::new(0.0, 0.8, 0.0),
            Vector3::new(0.0, 0.0, 0.8),
        ];

        let forces = tet.compute_forces(&deformed);

        // Compressed tetrahedron should push outward
        // Node 0 should be pushed toward negative direction
        assert!(forces[0].norm() > 0.0);
    }
}
