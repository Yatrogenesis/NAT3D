/*
 * NAT3D - Next-generation Advanced Technology for 3D
 * Professional 3D Modeling, CAD, Physics Simulation and Rendering Suite
 * 
 * Copyright (C) 2023-2026 Francisco Molina <pako.molina@gmail.com>
 * 
 * This software is dual-licensed:
 * 1. Open Source: GNU Affero General Public License v3.0 or later (AGPL-3.0-or-later)
 * 2. Commercial: For commercial use, please contact <fmolina@avermex.com>
 * 
 * For research information, visit: https://research.avermex.com
 * For collaborations, contact: <pako.molina@gmail.com>
 * 
 * DOI: [PENDING]
 */

//! Sketch to 3D mesh conversion.
//!
//! Converts 2D sketches/curves into 3D polygon meshes using
//! extrusion, revolution, and lofting techniques.

pub mod extrude;
pub mod inflate;
pub mod loft;
pub mod path;
pub mod revolve;
pub mod svg;

pub use extrude::ExtrudeMesh;
pub use inflate::InflateMesh;
pub use loft::LoftMesh;
pub use path::{Path2D, PathCommand, Point2D};
pub use revolve::RevolveMesh;
pub use svg::SvgImporter;

/// Sketch to mesh conversion options.
#[derive(Debug, Clone)]
pub struct SketchToMeshOptions {
    /// Conversion method
    pub method: ConversionMethod,
    /// Resolution for curves (segments per curve)
    pub curve_resolution: u32,
    /// Depth for extrusion
    pub extrude_depth: f32,
    /// Axis for revolution (0=X, 1=Y, 2=Z)
    pub revolve_axis: u32,
    /// Angle for revolution (degrees, 360 = full)
    pub revolve_angle: f32,
    /// Segments for revolution
    pub revolve_segments: u32,
    /// Inflation amount
    pub inflate_amount: f32,
    /// Cap ends
    pub cap_ends: bool,
    /// Smooth normals
    pub smooth_normals: bool,
    /// Scale factor
    pub scale: f32,
    /// Center the result
    pub center: bool,
}

impl Default for SketchToMeshOptions {
    fn default() -> Self {
        Self {
            method: ConversionMethod::Extrude,
            curve_resolution: 32,
            extrude_depth: 1.0,
            revolve_axis: 1, // Y axis
            revolve_angle: 360.0,
            revolve_segments: 32,
            inflate_amount: 0.5,
            cap_ends: true,
            smooth_normals: true,
            scale: 1.0,
            center: true,
        }
    }
}

/// Conversion method for sketch to 3D.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversionMethod {
    /// Extrude the 2D shape along Z axis
    Extrude,
    /// Revolve around an axis
    Revolve,
    /// Loft between multiple profiles
    Loft,
    /// Inflate/balloon the shape
    Inflate,
    /// Symmetric extrude (both directions)
    SymmetricExtrude,
}

/// Result of sketch to mesh conversion.
#[derive(Debug, Clone)]
pub struct MeshResult {
    /// Vertex positions [x, y, z]
    pub vertices: Vec<[f32; 3]>,
    /// Vertex normals
    pub normals: Vec<[f32; 3]>,
    /// UV coordinates
    pub uvs: Vec<[f32; 2]>,
    /// Triangle indices
    pub indices: Vec<u32>,
    /// Bounds min
    pub bounds_min: [f32; 3],
    /// Bounds max
    pub bounds_max: [f32; 3],
}

impl MeshResult {
    /// Create empty mesh result.
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            indices: Vec::new(),
            bounds_min: [f32::MAX; 3],
            bounds_max: [f32::MIN; 3],
        }
    }

    /// Add a vertex and return its index.
    pub fn add_vertex(&mut self, pos: [f32; 3], normal: [f32; 3], uv: [f32; 2]) -> u32 {
        let idx = self.vertices.len() as u32;
        self.vertices.push(pos);
        self.normals.push(normal);
        self.uvs.push(uv);

        // Update bounds
        for (i, &v) in pos.iter().enumerate().take(3) {
            self.bounds_min[i] = self.bounds_min[i].min(v);
            self.bounds_max[i] = self.bounds_max[i].max(v);
        }

        idx
    }

    /// Add a triangle.
    pub fn add_triangle(&mut self, i0: u32, i1: u32, i2: u32) {
        self.indices.push(i0);
        self.indices.push(i1);
        self.indices.push(i2);
    }

    /// Add a quad (as two triangles).
    pub fn add_quad(&mut self, i0: u32, i1: u32, i2: u32, i3: u32) {
        self.add_triangle(i0, i1, i2);
        self.add_triangle(i0, i2, i3);
    }

    /// Get vertex count.
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Get triangle count.
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Center the mesh at origin.
    pub fn center(&mut self) {
        let center = [
            (self.bounds_min[0] + self.bounds_max[0]) / 2.0,
            (self.bounds_min[1] + self.bounds_max[1]) / 2.0,
            (self.bounds_min[2] + self.bounds_max[2]) / 2.0,
        ];

        for v in &mut self.vertices {
            v[0] -= center[0];
            v[1] -= center[1];
            v[2] -= center[2];
        }

        let half_size = [
            (self.bounds_max[0] - self.bounds_min[0]) / 2.0,
            (self.bounds_max[1] - self.bounds_min[1]) / 2.0,
            (self.bounds_max[2] - self.bounds_min[2]) / 2.0,
        ];

        self.bounds_min = [-half_size[0], -half_size[1], -half_size[2]];
        self.bounds_max = half_size;
    }

    /// Scale the mesh.
    pub fn scale(&mut self, factor: f32) {
        for v in &mut self.vertices {
            v[0] *= factor;
            v[1] *= factor;
            v[2] *= factor;
        }
        for i in 0..3 {
            self.bounds_min[i] *= factor;
            self.bounds_max[i] *= factor;
        }
    }

    /// Recalculate normals from triangles.
    pub fn recalculate_normals(&mut self) {
        // Reset normals
        for n in &mut self.normals {
            *n = [0.0, 0.0, 0.0];
        }

        // Accumulate face normals
        for i in (0..self.indices.len()).step_by(3) {
            let i0 = self.indices[i] as usize;
            let i1 = self.indices[i + 1] as usize;
            let i2 = self.indices[i + 2] as usize;

            let v0 = self.vertices[i0];
            let v1 = self.vertices[i1];
            let v2 = self.vertices[i2];

            let e1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
            let e2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];

            let n = [
                e1[1] * e2[2] - e1[2] * e2[1],
                e1[2] * e2[0] - e1[0] * e2[2],
                e1[0] * e2[1] - e1[1] * e2[0],
            ];

            for &idx in &[i0, i1, i2] {
                self.normals[idx][0] += n[0];
                self.normals[idx][1] += n[1];
                self.normals[idx][2] += n[2];
            }
        }

        // Normalize
        for n in &mut self.normals {
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if len > 0.0 {
                n[0] /= len;
                n[1] /= len;
                n[2] /= len;
            }
        }
    }
}

impl Default for MeshResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert a 2D path to a 3D mesh.
pub fn sketch_to_mesh(path: &Path2D, options: &SketchToMeshOptions) -> MeshResult {
    let mut result = match options.method {
        ConversionMethod::Extrude => extrude::extrude_path(
            path,
            options.extrude_depth,
            options.curve_resolution,
            options.cap_ends,
        ),
        ConversionMethod::Revolve => revolve::revolve_path(
            path,
            options.revolve_axis,
            options.revolve_angle,
            options.revolve_segments,
            options.curve_resolution,
        ),
        ConversionMethod::SymmetricExtrude => extrude::symmetric_extrude_path(
            path,
            options.extrude_depth,
            options.curve_resolution,
            options.cap_ends,
        ),
        ConversionMethod::Inflate => {
            inflate::inflate_path(path, options.inflate_amount, options.curve_resolution)
        }
        ConversionMethod::Loft => {
            // Loft requires multiple profiles, for single path use extrude
            extrude::extrude_path(
                path,
                options.extrude_depth,
                options.curve_resolution,
                options.cap_ends,
            )
        }
    };

    if options.smooth_normals {
        result.recalculate_normals();
    }

    if options.center {
        result.center();
    }

    if (options.scale - 1.0).abs() > 0.0001 {
        result.scale(options.scale);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mesh_result() {
        let mut mesh = MeshResult::new();
        let i0 = mesh.add_vertex([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0]);
        let i1 = mesh.add_vertex([1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0]);
        let i2 = mesh.add_vertex([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0]);
        mesh.add_triangle(i0, i1, i2);

        assert_eq!(mesh.vertex_count(), 3);
        assert_eq!(mesh.triangle_count(), 1);
    }
}
