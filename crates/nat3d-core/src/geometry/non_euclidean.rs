// Non-Euclidean geometry primitives (Poincaré ball model).
use nalgebra::Vector3;

/// Möbius addition in the Poincaré ball model of curvature -c.
///
/// REF: [Ungar, 2001] "Hyperbolic Geometry of Gyrovector Spaces", chapter 3 of
///      "Beyond the Einstein Addition Law and its Gyroscopic Thomas Precession"
///      DOI: 10.1007/978-94-010-9122-0_4
///      Publisher: Springer, Fundamental Theories of Physics vol. 117
pub fn mobius_add(u: Vector3<f64>, v: Vector3<f64>, c: f64) -> Vector3<f64> {
    let u2 = u.norm_squared();
    let v2 = v.norm_squared();
    let uv = u.dot(&v);

    let num = (1.0 + 2.0 * c * uv + c * v2) * u + (1.0 - c * u2) * v;
    let den = 1.0 + 2.0 * c * uv + c * c * u2 * v2;

    num / den
}

/// Geodesic distance in the Poincaré ball model of curvature -c.
///
/// d_c(u, v) = (2 / sqrt(c)) * artanh( sqrt(c) * || (-u) (+)_c v || )
///
/// REF: [Cannon, Floyd, Kenyon and Parry, 1997] "Hyperbolic Geometry",
///      in "Flavors of Geometry", MSRI Publications vol. 31, pp. 59-115
///      DOI: 10.1017/9781009701853.003
///
/// Verified against the independent closed form
/// d_c(u, v) = (1 / sqrt(c)) * arcosh( 1 + 2c||u - v||^2 /
///             ((1 - c||u||^2)(1 - c||v||^2)) )
/// for c in {0.25, 0.5, 1, 2, 4}; both agree to within 1e-9. See the unit test
/// below, which encodes that comparison.
pub fn hyperbolic_distance(u: Vector3<f64>, v: Vector3<f64>, c: f64) -> f64 {
    let minus_u = -u;
    let add_res = mobius_add(minus_u, v, c);
    (2.0 / c.sqrt()) * (c.sqrt() * add_res.norm()).atanh()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Independent closed form used as the reference implementation.
    fn reference_distance(u: Vector3<f64>, v: Vector3<f64>, c: f64) -> f64 {
        let num = 2.0 * c * (u - v).norm_squared();
        let den = (1.0 - c * u.norm_squared()) * (1.0 - c * v.norm_squared());
        (1.0 / c.sqrt()) * (1.0 + num / den).acosh()
    }

    #[test]
    fn distance_matches_closed_form_across_curvatures() {
        let u = Vector3::new(0.10, 0.20, -0.05);
        let v = Vector3::new(-0.30, 0.15, 0.25);
        for &c in &[0.25_f64, 0.5, 1.0, 2.0, 4.0] {
            let got = hyperbolic_distance(u, v, c);
            let want = reference_distance(u, v, c);
            assert!(
                (got - want).abs() < 1e-9,
                "c={c}: got {got}, reference {want}"
            );
        }
    }

    #[test]
    fn distance_to_self_is_zero() {
        let u = Vector3::new(0.2, -0.1, 0.05);
        assert!(hyperbolic_distance(u, u, 1.0).abs() < 1e-12);
    }

    #[test]
    fn distance_is_symmetric() {
        let u = Vector3::new(0.10, 0.20, -0.05);
        let v = Vector3::new(-0.30, 0.15, 0.25);
        for &c in &[0.5_f64, 1.0, 2.0] {
            let a = hyperbolic_distance(u, v, c);
            let b = hyperbolic_distance(v, u, c);
            assert!((a - b).abs() < 1e-12, "c={c}: {a} vs {b}");
        }
    }

    #[test]
    fn mobius_add_zero_is_identity() {
        let u = Vector3::new(0.3, -0.2, 0.1);
        let zero = Vector3::zeros();
        assert!((mobius_add(zero, u, 1.0) - u).norm() < 1e-12);
        assert!((mobius_add(u, zero, 1.0) - u).norm() < 1e-12);
    }
}
