// SOTA 6: Quantum Tensor Network Simulator
use nalgebra::DMatrix;

/// Performs a simple tensor contraction (Matrix multiplication in 2D)
/// Represents gate application in a quantum state vector
pub fn apply_quantum_gate(state: &DMatrix<f64>, gate: &DMatrix<f64>) -> DMatrix<f64> {
    gate * state
}

/// Computes the probability of measuring state |0> for a single qubit
pub fn probability_zero(state: &DMatrix<f64>) -> f64 {
    // Assuming state is a column vector [alpha, beta]^T
    let alpha = state[(0, 0)];
    alpha * alpha
}
