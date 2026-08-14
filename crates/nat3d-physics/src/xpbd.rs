// SPDX-License-Identifier: AGPL-3.0-or-later
// REF: [Macklin, Muller and Chentanez, 2016] "XPBD: Position-Based Simulation
//      of Compliant Constrained Dynamics", Proceedings of the 9th International
//      Conference on Motion in Games (MIG '16)
//      DOI: 10.1145/2994258.2994272

use nalgebra::{Point3, Vector3};

pub struct XpbdConstraint {
    pub particle_a: usize,
    pub particle_b: usize,
    pub rest_length: f64,
    pub compliance: f64, // alpha en el paper
}

pub struct XpbdSolver {
    pub positions: Vec<Point3<f64>>,
    pub velocities: Vec<Vector3<f64>>,
    pub inv_masses: Vec<f64>,
    pub constraints: Vec<XpbdConstraint>,
}

impl XpbdSolver {
    pub fn new() -> Self {
        Self {
            positions: Vec::new(),
            velocities: Vec::new(),
            inv_masses: Vec::new(),
            constraints: Vec::new(),
        }
    }

    pub fn step(&mut self, dt: f64, substeps: usize) {
        if substeps == 0 {
            return;
        }
        let h = dt / (substeps as f64);

        for _ in 0..substeps {
            let prev_positions = self.positions.clone();

            // 1. Integrar velocidades con fuerzas externas y predecir
            for i in 0..self.positions.len() {
                if self.inv_masses[i] > 0.0 {
                    self.velocities[i] += Vector3::new(0.0, -9.81, 0.0) * h;
                    self.positions[i] += self.velocities[i] * h;
                }
            }

            // 2. Resolver constraints
            for c in &self.constraints {
                let w1 = self.inv_masses[c.particle_a];
                let w2 = self.inv_masses[c.particle_b];
                let w_sum = w1 + w2;
                if w_sum <= 0.0 {
                    continue;
                }

                let dir = self.positions[c.particle_a] - self.positions[c.particle_b];
                let len = dir.norm();
                if len < 1e-6 {
                    continue;
                }

                let n = dir / len;
                let constraint_eval = len - c.rest_length;
                let alpha = c.compliance / (h * h);
                let d_lambda = -constraint_eval / (w_sum + alpha);
                let p = n * d_lambda;

                if w1 > 0.0 {
                    self.positions[c.particle_a] += p * w1;
                }
                if w2 > 0.0 {
                    self.positions[c.particle_b] -= p * w2;
                }
            }

            // 3. Actualizar velocidades
            for i in 0..self.positions.len() {
                if self.inv_masses[i] > 0.0 {
                    self.velocities[i] = (self.positions[i] - prev_positions[i]) / h;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xpbd_distance_constraint_converges() {
        let mut solver = XpbdSolver::new();
        solver.positions.push(Point3::new(0.0, 10.0, 0.0));
        solver.positions.push(Point3::new(0.0, 9.0, 0.0));
        solver.velocities.push(Vector3::zeros());
        solver.velocities.push(Vector3::zeros());
        solver.inv_masses.push(0.0); // Fixed
        solver.inv_masses.push(1.0); // Dynamic

        let rest_length = 1.0;
        solver.constraints.push(XpbdConstraint {
            particle_a: 0,
            particle_b: 1,
            rest_length,
            compliance: 0.0,
        });

        for _ in 0..100 {
            solver.step(0.016, 10);
        }
        let dist = (solver.positions[0] - solver.positions[1]).norm();
        assert!((dist - rest_length).abs() < 0.01);
    }
}
