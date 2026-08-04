// SOTA 5: Differentiable Rendering Engine
use nalgebra::Vector3;

/// Computes the forward lighting and the adjoint (gradients) with respect to inputs
/// Useful for Inverse Rendering via Gradient Descent
pub fn diff_lambertian(
    normal: Vector3<f64>,
    light_dir: Vector3<f64>,
    albedo: Vector3<f64>,
    grad_output: Vector3<f64>,
) -> (Vector3<f64>, Vector3<f64>, Vector3<f64>) {
    let n_dot_l = normal.dot(&light_dir).max(0.0);
    let color = albedo * n_dot_l;

    // Gradients
    let mut grad_albedo = Vector3::zeros();
    let mut grad_normal = Vector3::zeros();

    if n_dot_l > 0.0 {
        grad_albedo = grad_output * n_dot_l;
        // dot(n, l) derivative wrt n is l
        let d_ndotl_dn = light_dir;

        let grad_color_wrt_ndotl = albedo.dot(&grad_output);
        grad_normal = d_ndotl_dn * grad_color_wrt_ndotl;
    }

    (color, grad_albedo, grad_normal)
}
