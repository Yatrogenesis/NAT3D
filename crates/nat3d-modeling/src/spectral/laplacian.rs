// SOTA 2: Spectral Mesh Processing
use nalgebra::{DMatrix, Point3};

/// Computes the uniform Laplacian matrix for a mesh
/// REF: [Sorkine, 2006] "Differential Representations for Mesh Processing"
pub fn compute_uniform_laplacian(vertices: &[Point3<f64>], faces: &[Vec<usize>]) -> DMatrix<f64> {
    let n = vertices.len();
    let mut laplacian = DMatrix::from_element(n, n, 0.0);
    
    let mut degrees = vec![0.0; n];
    
    for face in faces {
        let flen = face.len();
        for i in 0..flen {
            let v1 = face[i];
            let v2 = face[(i + 1) % flen];
            
            laplacian[(v1, v2)] = -1.0;
            laplacian[(v2, v1)] = -1.0;
            degrees[v1] += 1.0;
            degrees[v2] += 1.0;
        }
    }
    
    for i in 0..n {
        if degrees[i] > 0.0 {
            for j in 0..n {
                if i != j && laplacian[(i, j)] != 0.0 {
                    laplacian[(i, j)] /= degrees[i];
                }
            }
            laplacian[(i, i)] = 1.0;
        }
    }
    
    laplacian
}
