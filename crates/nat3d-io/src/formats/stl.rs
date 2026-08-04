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

//! STL (Stereolithography) format import/export.
//!
//! STL is a simple format widely used for 3D printing that stores:
//! - Triangulated surfaces only
//! - Face normals
//! - No colors, materials, or texture coordinates
//!
//! Supports both ASCII and binary formats.

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use nat3d_core::geometry::mesh::MeshData;
use nat3d_core::{Normal, Position};
use std::io::{Cursor, Write};
use std::path::Path;
use thiserror::Error;

/// STL format errors.
#[derive(Error, Debug)]
pub enum StlError {
    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// Invalid header.
    #[error("Invalid STL header")]
    InvalidHeader,
    /// Parse error.
    #[error("Parse error: {0}")]
    Parse(String),
    /// Non-triangular face.
    #[error("STL requires triangular faces, found face with {0} vertices")]
    NonTriangular(usize),
}

/// Result type for STL operations.
pub type StlResult<T> = Result<T, StlError>;

/// A single triangle in an STL file.
#[derive(Debug, Clone, Copy)]
pub struct StlTriangle {
    /// Face normal.
    pub normal: [f32; 3],
    /// First vertex.
    pub v1: [f32; 3],
    /// Second vertex.
    pub v2: [f32; 3],
    /// Third vertex.
    pub v3: [f32; 3],
    /// Attribute byte count (usually 0).
    pub attribute: u16,
}

/// STL file data.
#[derive(Debug, Clone, Default)]
pub struct StlData {
    /// Name/header of the STL file.
    pub name: String,
    /// Triangles.
    pub triangles: Vec<StlTriangle>,
}

impl StlData {
    /// Convert to mesh data.
    pub fn to_mesh(&self) -> MeshData {
        let mut positions = Vec::with_capacity(self.triangles.len() * 3);
        let mut normals = Vec::with_capacity(self.triangles.len() * 3);
        let mut faces = Vec::with_capacity(self.triangles.len());

        for (i, tri) in self.triangles.iter().enumerate() {
            let base = i * 3;

            positions.push(Position::new(
                tri.v1[0] as f64,
                tri.v1[1] as f64,
                tri.v1[2] as f64,
            ));
            positions.push(Position::new(
                tri.v2[0] as f64,
                tri.v2[1] as f64,
                tri.v2[2] as f64,
            ));
            positions.push(Position::new(
                tri.v3[0] as f64,
                tri.v3[1] as f64,
                tri.v3[2] as f64,
            ));

            let n = Normal::new(
                tri.normal[0] as f64,
                tri.normal[1] as f64,
                tri.normal[2] as f64,
            );
            normals.push(n);
            normals.push(n);
            normals.push(n);

            faces.push(vec![base, base + 1, base + 2]);
        }

        MeshData {
            name: self.name.clone(),
            positions,
            normals,
            uvs: vec![],
            faces,
            material_indices: vec![],
        }
    }

    /// Create from mesh data.
    pub fn from_mesh(mesh: &MeshData) -> StlResult<Self> {
        let mut triangles = Vec::new();

        for face in &mesh.faces {
            if face.len() != 3 {
                // Triangulate if needed
                if face.len() > 3 {
                    for i in 1..face.len() - 1 {
                        let tri = Self::create_triangle(mesh, face[0], face[i], face[i + 1]);
                        triangles.push(tri);
                    }
                } else {
                    return Err(StlError::NonTriangular(face.len()));
                }
            } else {
                let tri = Self::create_triangle(mesh, face[0], face[1], face[2]);
                triangles.push(tri);
            }
        }

        Ok(StlData {
            name: mesh.name.clone(),
            triangles,
        })
    }

    fn create_triangle(mesh: &MeshData, i0: usize, i1: usize, i2: usize) -> StlTriangle {
        let v1 = mesh.positions[i0];
        let v2 = mesh.positions[i1];
        let v3 = mesh.positions[i2];

        // Compute face normal
        let e1 = v2 - v1;
        let e2 = v3 - v1;
        let mut normal = e1.cross(&e2);
        let len = normal.norm();
        if len > 1e-10 {
            normal /= len;
        }

        StlTriangle {
            normal: [normal.x as f32, normal.y as f32, normal.z as f32],
            v1: [v1.x as f32, v1.y as f32, v1.z as f32],
            v2: [v2.x as f32, v2.y as f32, v2.z as f32],
            v3: [v3.x as f32, v3.y as f32, v3.z as f32],
            attribute: 0,
        }
    }
}

/// STL importer.
pub struct StlImporter {
    /// Whether to merge duplicate vertices.
    pub merge_vertices: bool,
    /// Merge threshold for vertices.
    pub merge_threshold: f64,
}

impl Default for StlImporter {
    fn default() -> Self {
        Self {
            merge_vertices: true,
            merge_threshold: 1e-6,
        }
    }
}

impl StlImporter {
    /// Create a new STL importer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Import an STL file from a path.
    pub fn import_file<P: AsRef<Path>>(&self, path: P) -> StlResult<StlData> {
        let data = std::fs::read(path)?;
        self.import_bytes(&data)
    }

    /// Import STL data from bytes.
    pub fn import_bytes(&self, data: &[u8]) -> StlResult<StlData> {
        // Check if ASCII or binary
        if self.is_ascii(data) {
            self.import_ascii(data)
        } else {
            self.import_binary(data)
        }
    }

    fn is_ascii(&self, data: &[u8]) -> bool {
        // ASCII STL files start with "solid"
        if data.len() < 5 {
            return false;
        }

        // Check for "solid" keyword
        let header = &data[..5];
        if header != b"solid" {
            return false;
        }

        // Binary files can also start with "solid" in the header
        // Check for "facet" or "endsolid" to confirm ASCII
        let text = String::from_utf8_lossy(data);
        text.contains("facet") || text.contains("endsolid")
    }

    fn import_ascii(&self, data: &[u8]) -> StlResult<StlData> {
        let text = String::from_utf8_lossy(data);
        let mut lines = text.lines();

        // Parse header
        let header_line = lines.next().unwrap_or("solid");
        let name = header_line
            .strip_prefix("solid")
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        let mut triangles = Vec::new();
        let mut current_normal = [0.0f32; 3];
        let mut vertices: Vec<[f32; 3]> = Vec::new();

        for line in lines {
            let line = line.trim();

            if line.starts_with("facet normal") {
                let parts: Vec<f32> = line
                    .strip_prefix("facet normal")
                    .unwrap_or("")
                    .split_whitespace()
                    .filter_map(|s| s.parse().ok())
                    .collect();

                if parts.len() >= 3 {
                    current_normal = [parts[0], parts[1], parts[2]];
                }
            } else if line.starts_with("vertex") {
                let parts: Vec<f32> = line
                    .strip_prefix("vertex")
                    .unwrap_or("")
                    .split_whitespace()
                    .filter_map(|s| s.parse().ok())
                    .collect();

                if parts.len() >= 3 {
                    vertices.push([parts[0], parts[1], parts[2]]);
                }
            } else if line.starts_with("endfacet") {
                if vertices.len() == 3 {
                    triangles.push(StlTriangle {
                        normal: current_normal,
                        v1: vertices[0],
                        v2: vertices[1],
                        v3: vertices[2],
                        attribute: 0,
                    });
                }
                vertices.clear();
            }
        }

        Ok(StlData { name, triangles })
    }

    fn import_binary(&self, data: &[u8]) -> StlResult<StlData> {
        if data.len() < 84 {
            return Err(StlError::InvalidHeader);
        }

        // Read header (80 bytes)
        let header = &data[..80];
        let name = String::from_utf8_lossy(header)
            .trim_matches('\0')
            .trim()
            .to_string();

        // Read triangle count
        let mut cursor = Cursor::new(&data[80..]);
        let triangle_count = cursor.read_u32::<LittleEndian>()? as usize;

        // Validate file size
        let expected_size = 84 + triangle_count * 50;
        if data.len() < expected_size {
            return Err(StlError::Parse(format!(
                "File too small: expected {} bytes, got {}",
                expected_size,
                data.len()
            )));
        }

        let mut triangles = Vec::with_capacity(triangle_count);

        for _ in 0..triangle_count {
            let normal = [
                cursor.read_f32::<LittleEndian>()?,
                cursor.read_f32::<LittleEndian>()?,
                cursor.read_f32::<LittleEndian>()?,
            ];

            let v1 = [
                cursor.read_f32::<LittleEndian>()?,
                cursor.read_f32::<LittleEndian>()?,
                cursor.read_f32::<LittleEndian>()?,
            ];

            let v2 = [
                cursor.read_f32::<LittleEndian>()?,
                cursor.read_f32::<LittleEndian>()?,
                cursor.read_f32::<LittleEndian>()?,
            ];

            let v3 = [
                cursor.read_f32::<LittleEndian>()?,
                cursor.read_f32::<LittleEndian>()?,
                cursor.read_f32::<LittleEndian>()?,
            ];

            let attribute = cursor.read_u16::<LittleEndian>()?;

            triangles.push(StlTriangle {
                normal,
                v1,
                v2,
                v3,
                attribute,
            });
        }

        Ok(StlData { name, triangles })
    }
}

/// STL exporter.
pub struct StlExporter {
    /// Whether to export as binary (true) or ASCII (false).
    pub binary: bool,
}

impl Default for StlExporter {
    fn default() -> Self {
        Self { binary: true }
    }
}

impl StlExporter {
    /// Create a new STL exporter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an ASCII exporter.
    pub fn ascii() -> Self {
        Self { binary: false }
    }

    /// Create a binary exporter.
    pub fn binary() -> Self {
        Self { binary: true }
    }

    /// Export STL data to a file.
    pub fn export_file<P: AsRef<Path>>(&self, path: P, data: &StlData) -> StlResult<()> {
        let file = std::fs::File::create(path)?;
        self.export_writer(file, data)
    }

    /// Export STL data to a writer.
    pub fn export_writer<W: Write>(&self, writer: W, data: &StlData) -> StlResult<()> {
        if self.binary {
            self.export_binary(writer, data)
        } else {
            self.export_ascii(writer, data)
        }
    }

    /// Export a mesh to STL.
    pub fn export_mesh<W: Write>(&self, writer: W, mesh: &MeshData) -> StlResult<()> {
        let stl_data = StlData::from_mesh(mesh)?;
        self.export_writer(writer, &stl_data)
    }

    fn export_ascii<W: Write>(&self, mut writer: W, data: &StlData) -> StlResult<()> {
        writeln!(writer, "solid {}", data.name)?;

        for tri in &data.triangles {
            writeln!(
                writer,
                "  facet normal {} {} {}",
                tri.normal[0], tri.normal[1], tri.normal[2]
            )?;
            writeln!(writer, "    outer loop")?;
            writeln!(
                writer,
                "      vertex {} {} {}",
                tri.v1[0], tri.v1[1], tri.v1[2]
            )?;
            writeln!(
                writer,
                "      vertex {} {} {}",
                tri.v2[0], tri.v2[1], tri.v2[2]
            )?;
            writeln!(
                writer,
                "      vertex {} {} {}",
                tri.v3[0], tri.v3[1], tri.v3[2]
            )?;
            writeln!(writer, "    endloop")?;
            writeln!(writer, "  endfacet")?;
        }

        writeln!(writer, "endsolid {}", data.name)?;
        Ok(())
    }

    fn export_binary<W: Write>(&self, mut writer: W, data: &StlData) -> StlResult<()> {
        // Write header (80 bytes)
        let mut header = [0u8; 80];
        let name_bytes = data.name.as_bytes();
        let copy_len = name_bytes.len().min(80);
        header[..copy_len].copy_from_slice(&name_bytes[..copy_len]);
        writer.write_all(&header)?;

        // Write triangle count
        writer.write_u32::<LittleEndian>(data.triangles.len() as u32)?;

        // Write triangles
        for tri in &data.triangles {
            writer.write_f32::<LittleEndian>(tri.normal[0])?;
            writer.write_f32::<LittleEndian>(tri.normal[1])?;
            writer.write_f32::<LittleEndian>(tri.normal[2])?;

            writer.write_f32::<LittleEndian>(tri.v1[0])?;
            writer.write_f32::<LittleEndian>(tri.v1[1])?;
            writer.write_f32::<LittleEndian>(tri.v1[2])?;

            writer.write_f32::<LittleEndian>(tri.v2[0])?;
            writer.write_f32::<LittleEndian>(tri.v2[1])?;
            writer.write_f32::<LittleEndian>(tri.v2[2])?;

            writer.write_f32::<LittleEndian>(tri.v3[0])?;
            writer.write_f32::<LittleEndian>(tri.v3[1])?;
            writer.write_f32::<LittleEndian>(tri.v3[2])?;

            writer.write_u16::<LittleEndian>(tri.attribute)?;
        }

        Ok(())
    }
}

/// Import an STL file.
pub fn import_stl<P: AsRef<Path>>(path: P) -> StlResult<StlData> {
    StlImporter::new().import_file(path)
}

/// Export STL data to a file.
pub fn export_stl<P: AsRef<Path>>(path: P, data: &StlData) -> StlResult<()> {
    StlExporter::new().export_file(path, data)
}

/// Export a mesh to an STL file.
pub fn export_mesh_stl<P: AsRef<Path>>(path: P, mesh: &MeshData) -> StlResult<()> {
    let file = std::fs::File::create(path)?;
    StlExporter::new().export_mesh(file, mesh)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ascii_import() {
        let stl_data = r#"solid test
  facet normal 0 0 1
    outer loop
      vertex 0 0 0
      vertex 1 0 0
      vertex 0.5 1 0
    endloop
  endfacet
endsolid test
"#;
        let result = StlImporter::new().import_bytes(stl_data.as_bytes());
        assert!(result.is_ok());

        let data = result.unwrap();
        assert_eq!(data.triangles.len(), 1);
        assert_eq!(data.name, "test");
    }

    #[test]
    fn test_binary_roundtrip() {
        let original = StlData {
            name: "test".to_string(),
            triangles: vec![StlTriangle {
                normal: [0.0, 0.0, 1.0],
                v1: [0.0, 0.0, 0.0],
                v2: [1.0, 0.0, 0.0],
                v3: [0.5, 1.0, 0.0],
                attribute: 0,
            }],
        };

        let mut buffer = Vec::new();
        StlExporter::binary()
            .export_writer(&mut buffer, &original)
            .unwrap();

        let imported = StlImporter::new().import_bytes(&buffer).unwrap();
        assert_eq!(imported.triangles.len(), 1);
        assert!((imported.triangles[0].v1[0] - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_mesh_conversion() {
        let mesh = MeshData {
            name: "triangle".to_string(),
            positions: vec![
                Position::new(0.0, 0.0, 0.0),
                Position::new(1.0, 0.0, 0.0),
                Position::new(0.5, 1.0, 0.0),
            ],
            normals: vec![],
            uvs: vec![],
            faces: vec![vec![0, 1, 2]],
            material_indices: vec![],
        };

        let stl = StlData::from_mesh(&mesh).unwrap();
        assert_eq!(stl.triangles.len(), 1);

        let back = stl.to_mesh();
        assert_eq!(back.faces.len(), 1);
        assert_eq!(back.positions.len(), 3);
    }
}
