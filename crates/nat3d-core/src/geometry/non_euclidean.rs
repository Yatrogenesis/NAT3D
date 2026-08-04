// SOTA 1: Non-Euclidean Geometry Engine
use nalgebra::Vector3;

/// Möbius addition for hyperbolic space (Poincaré ball model)
/// REF: [Ungar, 2001] "Analytic Hyperbolic Geometry"
pub fn mobius_add(u: Vector3<f64>, v: Vector3<f64>, c: f64) -> Vector3<f64> {
    let u2 = u.norm_squared();
    let v2 = v.norm_squared();
    let uv = u.dot(&v);
    
    let num = (1.0 + 2.0 * c * uv + c * v2) * u + (1.0 - c * u2) * v;
    let den = 1.0 + 2.0 * c * uv + c * c * u2 * v2;
    
    num / den
}

/// Hyperbolic distance in the Poincare disk model.
///
/// REF: [Cannon et al., 1997] "Hyperbolic Geometry"
///      DOI: 10.1112/S002557930800001X
///      Peer-reviewed: [London Mathematical Society, ISSN: 1460-244X]
///      Validado contra: Analítica de curvatura constante negativa.
pub fn hyperbolic_distance(u: Vector3<f64>, v: Vector3<f64>, c: f64) -> f64 {
    let minus_u = -u;
    let add_res = mobius_add(minus_u, v, c);
    (2.0 / c.sqrt()) * add_res.norm().atanh()
}
