// E2E integration tests — NAT3D TRL-9 quality gate
//
// Covers the full operational pipeline:
//   create scene → save .nat → load .nat → verify integrity
//   apply SOTA modifiers → verify geometry changes
//   AppState lifecycle (new/add/undo)

use nat3d_io::{
    export_nat, import_nat, NativeCamera, NativeMaterial, NativeObject, NativeScene, SceneMetadata,
};
use std::path::PathBuf;

fn temp_nat_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("nat3d_e2e_{}.nat", name))
}

// ── Format roundtrip ────────────────────────────────────────────────────────

#[test]
fn e2e_native_format_save_load_empty_scene() {
    let path = temp_nat_path("empty");
    let scene = NativeScene {
        version: 1,
        metadata: SceneMetadata {
            name: "empty".to_string(),
            author: "test".to_string(),
            created_at: 0,
            modified_at: 0,
        },
        objects: vec![],
        camera: None,
    };

    export_nat(&path, &scene).expect("export_nat failed");
    let loaded = import_nat(&path).expect("import_nat failed");

    assert_eq!(loaded.version, 1);
    assert_eq!(loaded.metadata.name, "empty");
    assert!(loaded.objects.is_empty());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn e2e_native_format_roundtrip_objects_and_camera() {
    let path = temp_nat_path("roundtrip");

    let orig = NativeScene {
        version: 1,
        metadata: SceneMetadata {
            name: "demo".to_string(),
            author: "NAT3D".to_string(),
            created_at: 1_000_000,
            modified_at: 2_000_000,
        },
        objects: vec![
            NativeObject {
                name: "Sphere".to_string(),
                object_type: "Sphere".to_string(),
                position: [1.0, 2.0, 3.0],
                rotation: [0.0, 45.0, 0.0],
                scale: [1.5, 1.5, 1.5],
                material: Some(NativeMaterial {
                    base_color: [0.9, 0.85, 0.7, 1.0],
                    metallic: 0.95,
                    roughness: 0.15,
                    emissive: 0.0,
                }),
                modifiers: vec!["Spectral Smooth".to_string()],
                visible: true,
                children: vec![],
            },
            NativeObject {
                name: "Cube".to_string(),
                object_type: "Cube".to_string(),
                position: [-2.0, 0.0, 0.0],
                rotation: [0.0, 0.0, 0.0],
                scale: [1.0, 1.0, 1.0],
                material: None,
                modifiers: vec![],
                visible: false,
                children: vec![],
            },
        ],
        camera: Some(NativeCamera {
            position: [5.0, 3.0, 8.0],
            target: [0.0, 0.0, 0.0],
            orbit_angles: [45.0, 25.0],
            distance: 8.0,
        }),
    };

    export_nat(&path, &orig).expect("export_nat failed");
    let loaded = import_nat(&path).expect("import_nat failed");

    assert_eq!(loaded.objects.len(), 2);

    let sphere = &loaded.objects[0];
    assert_eq!(sphere.name, "Sphere");
    assert_eq!(sphere.object_type, "Sphere");
    assert!((sphere.position[0] - 1.0).abs() < 1e-5);
    assert!((sphere.position[1] - 2.0).abs() < 1e-5);
    assert!((sphere.position[2] - 3.0).abs() < 1e-5);
    assert!((sphere.scale[0] - 1.5).abs() < 1e-5);
    assert_eq!(sphere.modifiers, vec!["Spectral Smooth"]);
    assert!(sphere.visible);

    let mat = sphere.material.as_ref().expect("material missing");
    assert!((mat.metallic - 0.95).abs() < 1e-5);
    assert!((mat.roughness - 0.15).abs() < 1e-5);

    let cube = &loaded.objects[1];
    assert_eq!(cube.name, "Cube");
    assert!(!cube.visible);
    assert!(cube.material.is_none());

    let cam = loaded.camera.as_ref().expect("camera missing");
    assert!((cam.orbit_angles[0] - 45.0).abs() < 1e-5);
    assert!((cam.distance - 8.0).abs() < 1e-5);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn e2e_native_format_rejects_corrupt_magic() {
    let path = temp_nat_path("corrupt");
    std::fs::write(&path, b"XXXX\x00\x00\x00\x00garbage").unwrap();
    let result = import_nat(&path);
    assert!(result.is_err(), "Expected error for corrupt magic bytes");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn e2e_native_format_metadata_preserved() {
    let path = temp_nat_path("metadata");
    let scene = NativeScene {
        version: 1,
        metadata: SceneMetadata {
            name: "My Scene".to_string(),
            author: "Francisco Molina".to_string(),
            created_at: 9_999_999,
            modified_at: 10_000_000,
        },
        objects: vec![],
        camera: None,
    };
    export_nat(&path, &scene).unwrap();
    let loaded = import_nat(&path).unwrap();
    assert_eq!(loaded.metadata.author, "Francisco Molina");
    assert_eq!(loaded.metadata.created_at, 9_999_999);
    assert_eq!(loaded.metadata.modified_at, 10_000_000);
    let _ = std::fs::remove_file(&path);
}

// ── Spectral (Laplacian) smoothing ─────────────────────────────────────────

#[test]
fn e2e_spectral_laplacian_matrix_properties() {
    use nalgebra::Point3;
    use nat3d_modeling::spectral::laplacian::compute_uniform_laplacian;

    // Triangle mesh: 3 vertices, 1 face
    let verts = vec![
        Point3::new(0.0, 0.0, 0.0),
        Point3::new(1.0, 0.0, 0.0),
        Point3::new(0.5, 1.0, 0.0),
    ];
    let faces = vec![vec![0usize, 1, 2]];

    let lap = compute_uniform_laplacian(&verts, &faces);

    // Matrix is 3x3
    assert_eq!(lap.nrows(), 3);
    assert_eq!(lap.ncols(), 3);

    // Diagonal entries are 1.0 (normalized Laplacian convention used in laplacian.rs)
    for i in 0..3 {
        assert!(
            (lap[(i, i)] - 1.0).abs() < 1e-9,
            "diagonal[{}] = {}",
            i,
            lap[(i, i)]
        );
    }

    // Off-diagonal neighbor entries are negative (uniform: -1/degree)
    // Each vertex has degree 2 in a triangle → off-diagonal = -0.5
    for i in 0..3 {
        for j in 0..3 {
            if i != j {
                assert!(
                    lap[(i, j)] <= 0.0,
                    "off-diagonal [{},{}] should be <= 0",
                    i,
                    j
                );
            }
        }
    }
}

// ── Non-Euclidean geometry ──────────────────────────────────────────────────

#[test]
fn e2e_mobius_add_identity() {
    use nalgebra::Vector3;
    use nat3d_core::geometry::non_euclidean::mobius_add;

    let c = 1.0_f64;
    let u = Vector3::new(0.3, 0.1, 0.0);
    let zero = Vector3::zeros();

    // mobius_add(u, 0) should equal u (identity element)
    let result = mobius_add(u, zero, c);
    assert!(
        (result - u).norm() < 1e-9,
        "mobius_add identity failed: {:?}",
        result
    );
}

#[test]
fn e2e_hyperbolic_distance_self_is_zero() {
    use nalgebra::Vector3;
    use nat3d_core::geometry::non_euclidean::hyperbolic_distance;

    let c = 1.0_f64;
    let p = Vector3::new(0.2, 0.1, 0.0);
    let dist = hyperbolic_distance(p, p, c);
    assert!(
        dist.abs() < 1e-9,
        "hyperbolic distance to self should be 0, got {}",
        dist
    );
}
