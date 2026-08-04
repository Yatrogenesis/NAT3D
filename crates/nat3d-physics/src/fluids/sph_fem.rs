// SOTA 3: Fully Coupled SPH-FEM Physics
use nalgebra::Vector3;

/// Evaluates the Poly6 smoothing kernel
pub fn poly6_kernel(r: Vector3<f64>, h: f64) -> f64 {
    let r2 = r.norm_squared();
    let h2 = h * h;
    if r2 >= h2 {
        0.0
    } else {
        let coeff = 315.0 / (64.0 * std::f64::consts::PI * h.powi(9));
        coeff * (h2 - r2).powi(3)
    }
}

/// Computes interaction force between fluid particle and solid node
pub fn compute_coupling_force(
    fluid_pos: Vector3<f64>, 
    solid_pos: Vector3<f64>, 
    fluid_pressure: f64, 
    solid_pressure: f64,
    h: f64
) -> Vector3<f64> {
    let r_vec = fluid_pos - solid_pos;
    let r = r_vec.norm();
    if r >= h || r < 1e-6 {
        return Vector3::zeros();
    }
    
    // Spiky kernel gradient
    let coeff = -45.0 / (std::f64::consts::PI * h.powi(6));
    let grad_w = r_vec * (coeff * (h - r).powi(2) / r);
    
    let mean_pressure = (fluid_pressure + solid_pressure) * 0.5;
    -grad_w * mean_pressure
}
