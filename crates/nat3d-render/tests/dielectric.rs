use nalgebra::{Point3, Vector3};
use nat3d_render::raytracing::bvh::{Bvh, BvhParams, Triangle};
use nat3d_render::raytracing::pathtracer::{Material, PathTracer, PathTracerConfig};
use nat3d_render::raytracing::ray::Ray;
use rand::{rngs::StdRng, SeedableRng};

#[test]
fn ray_exiting_glass_above_critical_angle_cannot_reach_outside_light() {
    let plane = |z, index| {
        Triangle::new(
            Point3::new(-100.0, -100.0, z),
            Point3::new(100.0, -100.0, z),
            Point3::new(0.0, 100.0, z),
            index,
        )
    };
    let bvh = Bvh::build(vec![plane(0.0, 0), plane(1.0, 1)], BvhParams::default());
    let tracer = PathTracer::new(
        bvh,
        vec![Material::glass(1.5), Material::emissive(1.0, 1.0, 1.0)],
        PathTracerConfig {
            max_bounces: 4,
            rr_start_depth: 5,
            background: Vector3::zeros(),
            ..Default::default()
        },
    );
    // From inside glass toward the +Z outward normal at 60 degrees:
    // eta=1.5 and sin(theta)=sqrt(3)/2 require total internal reflection.
    let ray = Ray::new(
        Point3::new(0.0, 0.0, -1.0),
        Vector3::new(3.0_f64.sqrt() / 2.0, 0.0, 0.5),
    );
    for seed in 0..16 {
        let color = tracer.trace_ray(&ray, &mut StdRng::seed_from_u64(seed));
        assert_eq!(
            color,
            Vector3::zeros(),
            "seed {seed}: light leaked through glass"
        );
    }
}
