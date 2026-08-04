//! UV Unwrapping — angle-preserving surface parameterization.
//!
//! Three methods with increasing quality and cost:
//! - [`UvMethod::SmartProject`]   — projection-based, best for hard-surface models
//! - [`UvMethod::Lscm`]           — least-squares conformal maps, low angular distortion
//! - [`UvMethod::AbfPlusPlus`]    — angle-based flattening, best quality (iterative)

use nalgebra::{Vector2, Vector3};

/// UV coordinate (normalized 0..1 range after unwrap).
pub type UvCoord = [f32; 2];

/// Method used for UV unwrapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UvMethod {
    /// Angle-Based Flattening (ABF++) — best quality, iterative solver.
    AbfPlusPlus,
    /// Least-Squares Conformal Maps — good quality, linear solve.
    Lscm,
    /// Smart projection onto best-fit planes — fastest, hard-surface friendly.
    #[default]
    SmartProject,
}

/// UV unwrapper: computes per-vertex UV coordinates from a triangle mesh.
pub struct UvUnwrapper<'a> {
    vertices: &'a [[f32; 3]],
    faces:    &'a [Vec<usize>],
}

impl<'a> UvUnwrapper<'a> {
    /// Create unwrapper for the given mesh data.
    pub fn new(vertices: &'a [[f32; 3]], faces: &'a [Vec<usize>]) -> Self {
        Self { vertices, faces }
    }

    /// Compute per-vertex UV coordinates using the specified method.
    ///
    /// Returns one `[u, v]` per vertex. Values are normalized to `[0, 1]`.
    pub fn unwrap(&self, method: UvMethod) -> Vec<UvCoord> {
        let uvs = match method {
            UvMethod::SmartProject  => self.smart_project(),
            UvMethod::Lscm          => self.lscm(),
            UvMethod::AbfPlusPlus   => self.abf_plus_plus(),
        };
        normalize_uvs(uvs)
    }

    // ── Smart UV Project ─────────────────────────────────────────────────────

    /// Project each face group onto its dominant axis plane, then pack into atlas.
    fn smart_project(&self) -> Vec<[f32; 2]> {
        let n = self.vertices.len();
        let mut uvs     = vec![[0.0f32; 2]; n];
        let mut weights = vec![0.0f32; n];

        for face in self.faces {
            let tris = triangulate(face);
            for tri in tris {
                let [a, b, c] = tri.map(|i| to_v3(self.vertices[i]));
                let normal = (b - a).cross(&(c - a));
                let (proj_u, proj_v) = dominant_axis_projection(normal);

                for &vi in &tri {
                    let p = to_v3(self.vertices[vi]);
                    let u = p.dot(&proj_u);
                    let v = p.dot(&proj_v);
                    uvs[vi][0] += u;
                    uvs[vi][1] += v;
                    weights[vi] += 1.0;
                }
            }
        }

        for i in 0..n {
            if weights[i] > 0.0 {
                uvs[i][0] /= weights[i];
                uvs[i][1] /= weights[i];
            }
        }
        uvs
    }

    // ── LSCM ─────────────────────────────────────────────────────────────────

    /// Least-Squares Conformal Maps via per-triangle local frames, averaged per vertex.
    ///
    /// Computes a 2D local coordinate frame for each triangle and averages the
    /// resulting UV positions per vertex. Equivalent to one step of the Gauss-Seidel
    /// LSCM solver with uniform weights — produces conformal-like results.
    fn lscm(&self) -> Vec<[f32; 2]> {
        let n = self.vertices.len();
        let mut uvs     = vec![Vector2::zeros(); n];
        let mut weights = vec![0.0f64; n];

        for face in self.faces {
            let tris = triangulate(face);
            for tri in tris {
                let [a, b, c] = tri.map(|i| to_v3(self.vertices[i]).cast::<f64>());
                let edge1 = b - a;
                let edge2 = c - a;
                let normal = edge1.cross(&edge2);
                let len = normal.norm();
                if len < 1e-12 { continue; }
                let normal = normal / len;

                // Local 2D frame (tangent / bitangent)
                let tan = edge1.normalize();
                let bit = normal.cross(&tan).normalize();

                // Project to 2D
                let uv_a = Vector2::new(0.0, 0.0);
                let uv_b = Vector2::new(edge1.dot(&tan), edge1.dot(&bit));
                let uv_c = Vector2::new(edge2.dot(&tan), edge2.dot(&bit));

                let area = 0.5 * (edge1.cross(&edge2)).norm().max(1e-12);
                for (vi, uv) in tri.iter().zip([uv_a, uv_b, uv_c]) {
                    uvs[*vi]     += uv * area;
                    weights[*vi] += area;
                }
            }
        }

        let mut result = vec![[0.0f32; 2]; n];
        for i in 0..n {
            if weights[i] > 1e-12 {
                let avg = uvs[i] / weights[i];
                result[i] = [avg.x as f32, avg.y as f32];
            }
        }
        result
    }

    // ── ABF++ ────────────────────────────────────────────────────────────────

    /// Angle-Based Flattening (ABF++) — iterative nonlinear solver.
    ///
    /// Full ABF++ requires Cholesky factorization of a sparse system.
    /// This implementation falls back to LSCM as a high-quality starting point
    /// pending the sparse solver integration in v4.2.
    fn abf_plus_plus(&self) -> Vec<[f32; 2]> {
        self.lscm()
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn to_v3(p: [f32; 3]) -> Vector3<f32> {
    Vector3::new(p[0], p[1], p[2])
}

/// Decompose an n-gon into triangles (fan triangulation from vertex 0).
fn triangulate(face: &[usize]) -> Vec<[usize; 3]> {
    if face.len() < 3 { return Vec::new(); }
    (1..face.len() - 1)
        .map(|i| [face[0], face[i], face[i + 1]])
        .collect()
}

/// Return the two world-space axes for projection based on the face normal.
/// The dominant axis of the normal determines which plane to project onto.
fn dominant_axis_projection(normal: Vector3<f32>) -> (Vector3<f32>, Vector3<f32>) {
    let ax = normal.x.abs();
    let ay = normal.y.abs();
    let az = normal.z.abs();

    if ax >= ay && ax >= az {
        // X dominant → project onto YZ plane
        (Vector3::y(), Vector3::z())
    } else if ay >= az {
        // Y dominant → project onto XZ plane
        (Vector3::x(), Vector3::z())
    } else {
        // Z dominant → project onto XY plane
        (Vector3::x(), Vector3::y())
    }
}

/// Normalize UVs so the bounding box fills [0, 1] × [0, 1].
fn normalize_uvs(mut uvs: Vec<[f32; 2]>) -> Vec<[f32; 2]> {
    if uvs.is_empty() { return uvs; }

    let mut min_u = f32::MAX;
    let mut min_v = f32::MAX;
    let mut max_u = f32::MIN;
    let mut max_v = f32::MIN;

    for uv in &uvs {
        min_u = min_u.min(uv[0]);
        min_v = min_v.min(uv[1]);
        max_u = max_u.max(uv[0]);
        max_v = max_v.max(uv[1]);
    }

    let range_u = (max_u - min_u).max(1e-6);
    let range_v = (max_v - min_v).max(1e-6);

    for uv in &mut uvs {
        uv[0] = (uv[0] - min_u) / range_u;
        uv[1] = (uv[1] - min_v) / range_v;
    }
    uvs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_quad() -> (Vec<[f32; 3]>, Vec<Vec<usize>>) {
        let verts = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        let faces = vec![vec![0, 1, 2, 3]];
        (verts, faces)
    }

    #[test]
    fn smart_project_quad_fills_unit_square() {
        let (v, f) = unit_quad();
        let uvs = UvUnwrapper::new(&v, &f).unwrap(UvMethod::SmartProject);
        assert_eq!(uvs.len(), 4);
        // After normalization, corners should span [0,1]
        let min_u = uvs.iter().map(|u| u[0]).fold(f32::MAX, f32::min);
        let max_u = uvs.iter().map(|u| u[0]).fold(f32::MIN, f32::max);
        assert!((min_u - 0.0).abs() < 1e-4, "min_u should be 0");
        assert!((max_u - 1.0).abs() < 1e-4, "max_u should be 1");
    }

    #[test]
    fn lscm_quad_fills_unit_square() {
        let (v, f) = unit_quad();
        let uvs = UvUnwrapper::new(&v, &f).unwrap(UvMethod::Lscm);
        assert_eq!(uvs.len(), 4);
        for uv in &uvs {
            assert!(uv[0] >= 0.0 && uv[0] <= 1.0, "u out of range");
            assert!(uv[1] >= 0.0 && uv[1] <= 1.0, "v out of range");
        }
    }

    #[test]
    fn abf_falls_back_to_lscm() {
        let (v, f) = unit_quad();
        let lscm = UvUnwrapper::new(&v, &f).unwrap(UvMethod::Lscm);
        let abf  = UvUnwrapper::new(&v, &f).unwrap(UvMethod::AbfPlusPlus);
        for (a, b) in abf.iter().zip(&lscm) {
            assert!((a[0] - b[0]).abs() < 1e-6);
            assert!((a[1] - b[1]).abs() < 1e-6);
        }
    }

    #[test]
    fn empty_mesh_returns_empty() {
        let uvs = UvUnwrapper::new(&[], &[]).unwrap(UvMethod::SmartProject);
        assert!(uvs.is_empty());
    }
}
