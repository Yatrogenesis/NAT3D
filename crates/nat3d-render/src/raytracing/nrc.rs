// SOTA 4: Neural Radiance Caching (NRC) Forward Pass
use nalgebra::{Vector3, DMatrix, DVector};

/// A simple Multi-Layer Perceptron (MLP) for Neural Radiance
pub struct RadianceMLP {
    /// Weight matrix for the first layer.
    pub w1: DMatrix<f64>,
    /// Bias vector for the first layer.
    pub b1: DVector<f64>,
    /// Weight matrix for the second layer.
    pub w2: DMatrix<f64>,
    /// Bias vector for the second layer.
    pub b2: DVector<f64>,
    /// Weight matrix for the third layer.
    pub w3: DMatrix<f64>,
    /// Bias vector for the third layer.
    pub b3: DVector<f64>,
}

impl RadianceMLP {
    /// Creates a new RadianceMLP with random weights (initialized to 0.1).
    pub fn new_random(input_dim: usize, hidden: usize, output_dim: usize) -> Self {
        Self {
            w1: DMatrix::from_element(hidden, input_dim, 0.1),
            b1: DVector::from_element(hidden, 0.0),
            w2: DMatrix::from_element(hidden, hidden, 0.1),
            b2: DVector::from_element(hidden, 0.0),
            w3: DMatrix::from_element(output_dim, hidden, 0.1),
            b3: DVector::from_element(output_dim, 0.0),
        }
    }

    /// ReLU activation
    fn relu(mut v: DVector<f64>) -> DVector<f64> {
        for val in v.iter_mut() { *val = val.max(0.0); }
        v
    }
    
    /// Predicts radiance RGB from a 5D input (pos 3D + dir 2D)
    pub fn predict(&self, input: DVector<f64>) -> Vector3<f64> {
        let a1 = Self::relu(&self.w1 * &input + &self.b1);
        let a2 = Self::relu(&self.w2 * &a1 + &self.b2);
        let out = &self.w3 * &a2 + &self.b3;
        // Sigmoid to bound output to [0, 1]
        Vector3::new(
            1.0 / (1.0 + (-out[0]).exp()),
            1.0 / (1.0 + (-out[1]).exp()),
            1.0 / (1.0 + (-out[2]).exp()),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mlp_prediction() {
        let mlp = RadianceMLP::new_random(5, 64, 3);
        let out = mlp.predict(DVector::zeros(5));
        assert!(out.x > 0.0 && out.x < 1.0);
    }
}
