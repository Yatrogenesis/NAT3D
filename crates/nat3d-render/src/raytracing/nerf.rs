// SOTA 7: NeRF (Neural Radiance Fields) Ray Marching
use nalgebra::Vector3;

/// A single sample point along a ray in a neural volume.
pub struct VolumeSample {
    /// RGB color at this point.
    pub color: Vector3<f64>,
    /// Volumetric density (opacity) at this point.
    pub density: f64,
}

/// Accumulates radiance along a ray using volumetric rendering equations
pub fn accumulate_radiance(samples: &[VolumeSample], step_size: f64) -> Vector3<f64> {
    let mut c_out = Vector3::zeros();
    let mut transmittance = 1.0;

    for sample in samples {
        let alpha = 1.0 - (-sample.density * step_size).exp();
        let weight = alpha * transmittance;
        c_out += sample.color * weight;
        transmittance *= 1.0 - alpha;

        if transmittance < 1e-4 {
            break; // Early ray termination
        }
    }

    c_out
}
