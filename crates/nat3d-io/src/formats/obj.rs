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

//! Wavefront OBJ format import/export.
//!
//! OBJ is a simple, widely-supported 3D model format that stores:
//! - Vertex positions (v)
//! - Texture coordinates (vt)
//! - Vertex normals (vn)
//! - Faces (f)
//! - Groups (g)
//! - Materials (usemtl, mtllib)

use nat3d_core::geometry::mesh::MeshData;
use nat3d_core::{Normal, Position, TexCoord};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use thiserror::Error;

/// OBJ format errors.
#[derive(Error, Debug)]
pub enum ObjError {
    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// Parse error.
    #[error("Parse error at line {line}: {message}")]
    Parse {
        /// Line number where error occurred.
        line: usize,
        /// Error message.
        message: String,
    },
    /// Invalid face index.
    #[error("Invalid face index {index} at line {line}")]
    InvalidIndex {
        /// The invalid index.
        index: i32,
        /// Line number.
        line: usize,
    },
}

/// Result type for OBJ operations.
pub type ObjResult<T> = Result<T, ObjError>;

/// OBJ file data with multiple objects/groups.
#[derive(Debug, Clone, Default)]
pub struct ObjData {
    /// Named objects/groups.
    pub objects: Vec<ObjObject>,
    /// Material library files referenced.
    pub mtl_libs: Vec<String>,
}

/// A single object or group in an OBJ file.
#[derive(Debug, Clone, Default)]
pub struct ObjObject {
    /// Object name.
    pub name: String,
    /// Groups within this object.
    pub groups: Vec<ObjGroup>,
}

/// A group within an object.
#[derive(Debug, Clone)]
pub struct ObjGroup {
    /// Group name.
    pub name: String,
    /// Material name.
    pub material: Option<String>,
    /// Mesh data.
    pub mesh: MeshData,
}

impl Default for ObjGroup {
    fn default() -> Self {
        Self {
            name: String::new(),
            material: None,
            mesh: MeshData::new("default"),
        }
    }
}

/// OBJ importer.
pub struct ObjImporter {
    /// Whether to compute normals if not provided.
    pub compute_normals: bool,
    /// Whether to triangulate faces.
    pub triangulate: bool,
}

impl Default for ObjImporter {
    fn default() -> Self {
        Self {
            compute_normals: true,
            triangulate: true,
        }
    }
}

impl ObjImporter {
    /// Create a new OBJ importer with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Import an OBJ file from a path.
    pub fn import_file<P: AsRef<Path>>(&self, path: P) -> ObjResult<ObjData> {
        let file = std::fs::File::open(path)?;
        self.import_reader(BufReader::new(file))
    }

    /// Import OBJ data from a reader.
    pub fn import_reader<R: Read>(&self, reader: BufReader<R>) -> ObjResult<ObjData> {
        let mut positions: Vec<Position> = Vec::new();
        let mut normals: Vec<Normal> = Vec::new();
        let mut uvs: Vec<TexCoord> = Vec::new();

        let mut data = ObjData::default();
        let mut current_object = ObjObject {
            name: "default".to_string(),
            groups: vec![ObjGroup::default()],
        };
        let mut current_group_idx = 0;
        let mut current_material: Option<String> = None;

        for (line_num, line_result) in reader.lines().enumerate() {
            let line = line_result?;
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let mut parts = line.split_whitespace();
            let keyword = match parts.next() {
                Some(k) => k,
                None => continue,
            };

            match keyword {
                "v" => {
                    // Vertex position
                    let coords: Vec<f64> = parts.filter_map(|s| s.parse().ok()).collect();
                    if coords.len() >= 3 {
                        positions.push(Position::new(coords[0], coords[1], coords[2]));
                    }
                }
                "vn" => {
                    // Vertex normal
                    let coords: Vec<f64> = parts.filter_map(|s| s.parse().ok()).collect();
                    if coords.len() >= 3 {
                        normals.push(Normal::new(coords[0], coords[1], coords[2]));
                    }
                }
                "vt" => {
                    // Texture coordinate
                    let coords: Vec<f64> = parts.filter_map(|s| s.parse().ok()).collect();
                    if coords.len() >= 2 {
                        uvs.push(TexCoord::new(coords[0], coords[1]));
                    }
                }
                "f" => {
                    // Face
                    let face_result = self.parse_face(
                        &parts.collect::<Vec<_>>(),
                        &positions,
                        &normals,
                        &uvs,
                        line_num + 1,
                    );

                    match face_result {
                        Ok((face_indices, face_normals, face_uvs)) => {
                            let group = &mut current_object.groups[current_group_idx];

                            // Add vertices for this face
                            let base_idx = group.mesh.positions.len();
                            for i in 0..face_indices.len() {
                                group.mesh.positions.push(positions[face_indices[i]]);
                                if !face_normals.is_empty() {
                                    group.mesh.normals.push(normals[face_normals[i]]);
                                }
                                if !face_uvs.is_empty() {
                                    group.mesh.uvs.push(uvs[face_uvs[i]]);
                                }
                            }

                            // Create face with local indices
                            let local_face: Vec<usize> =
                                (base_idx..base_idx + face_indices.len()).collect();

                            if self.triangulate && local_face.len() > 3 {
                                // Fan triangulation
                                for i in 1..local_face.len() - 1 {
                                    group.mesh.faces.push(vec![
                                        local_face[0],
                                        local_face[i],
                                        local_face[i + 1],
                                    ]);
                                }
                            } else {
                                group.mesh.faces.push(local_face);
                            }
                        }
                        Err(e) => return Err(e),
                    }
                }
                "o" => {
                    // Object name
                    if !current_object
                        .groups
                        .iter()
                        .all(|g| g.mesh.faces.is_empty())
                    {
                        data.objects.push(current_object);
                    }
                    let name = parts.collect::<Vec<_>>().join(" ");
                    current_object = ObjObject {
                        name: if name.is_empty() {
                            "object".to_string()
                        } else {
                            name
                        },
                        groups: vec![ObjGroup::default()],
                    };
                    current_group_idx = 0;
                }
                "g" => {
                    // Group
                    let name = parts.collect::<Vec<_>>().join(" ");
                    let group_name = if name.is_empty() {
                        "default".to_string()
                    } else {
                        name
                    };
                    let new_group = ObjGroup {
                        name: group_name.clone(),
                        material: current_material.clone(),
                        mesh: MeshData::new(group_name),
                    };
                    current_object.groups.push(new_group);
                    current_group_idx = current_object.groups.len() - 1;
                }
                "usemtl" => {
                    // Material
                    current_material = Some(parts.collect::<Vec<_>>().join(" "));
                    current_object.groups[current_group_idx].material = current_material.clone();
                }
                "mtllib" => {
                    // Material library
                    data.mtl_libs.push(parts.collect::<Vec<_>>().join(" "));
                }
                "s" => {
                    // Smoothing group - ignored for now
                }
                _ => {
                    // Unknown keyword - ignore
                }
            }
        }

        // Add the last object
        if !current_object
            .groups
            .iter()
            .all(|g| g.mesh.faces.is_empty())
        {
            data.objects.push(current_object);
        }

        // Compute normals if needed
        if self.compute_normals {
            for obj in &mut data.objects {
                for group in &mut obj.groups {
                    if group.mesh.normals.is_empty() && !group.mesh.positions.is_empty() {
                        group.mesh.normals = compute_normals(&group.mesh);
                    }
                }
            }
        }

        Ok(data)
    }

    fn parse_face(
        &self,
        parts: &[&str],
        positions: &[Position],
        normals: &[Normal],
        uvs: &[TexCoord],
        line_num: usize,
    ) -> ObjResult<(Vec<usize>, Vec<usize>, Vec<usize>)> {
        let mut pos_indices = Vec::new();
        let mut norm_indices = Vec::new();
        let mut uv_indices = Vec::new();

        for part in parts {
            let indices: Vec<&str> = part.split('/').collect();

            // Position index (required)
            if let Some(pos_str) = indices.first() {
                if let Ok(idx) = pos_str.parse::<i32>() {
                    let pos_idx = self.resolve_index(idx, positions.len(), line_num)?;
                    pos_indices.push(pos_idx);
                }
            }

            // Texture coordinate index (optional)
            if indices.len() > 1 && !indices[1].is_empty() {
                if let Ok(idx) = indices[1].parse::<i32>() {
                    let uv_idx = self.resolve_index(idx, uvs.len(), line_num)?;
                    uv_indices.push(uv_idx);
                }
            }

            // Normal index (optional)
            if indices.len() > 2 && !indices[2].is_empty() {
                if let Ok(idx) = indices[2].parse::<i32>() {
                    let norm_idx = self.resolve_index(idx, normals.len(), line_num)?;
                    norm_indices.push(norm_idx);
                }
            }
        }

        Ok((pos_indices, norm_indices, uv_indices))
    }

    fn resolve_index(&self, idx: i32, count: usize, line_num: usize) -> ObjResult<usize> {
        let count_i32 = count as i32;
        let resolved = if idx > 0 {
            idx - 1
        } else if idx < 0 {
            count_i32 + idx
        } else {
            return Err(ObjError::InvalidIndex {
                index: idx,
                line: line_num,
            });
        };

        if resolved < 0 || resolved >= count_i32 {
            return Err(ObjError::InvalidIndex {
                index: idx,
                line: line_num,
            });
        }

        Ok(resolved as usize)
    }
}

/// OBJ exporter.
pub struct ObjExporter {
    /// Whether to export normals.
    pub export_normals: bool,
    /// Whether to export texture coordinates.
    pub export_uvs: bool,
    /// Decimal precision for coordinates.
    pub precision: usize,
}

impl Default for ObjExporter {
    fn default() -> Self {
        Self {
            export_normals: true,
            export_uvs: true,
            precision: 6,
        }
    }
}

impl ObjExporter {
    /// Create a new OBJ exporter with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Export OBJ data to a file.
    pub fn export_file<P: AsRef<Path>>(&self, path: P, data: &ObjData) -> ObjResult<()> {
        let file = std::fs::File::create(path)?;
        self.export_writer(file, data)
    }

    /// Export OBJ data to a writer.
    pub fn export_writer<W: Write>(&self, mut writer: W, data: &ObjData) -> ObjResult<()> {
        writeln!(writer, "# NAT3D OBJ Export")?;
        writeln!(writer)?;

        // Export material libraries
        for mtl_lib in &data.mtl_libs {
            writeln!(writer, "mtllib {}", mtl_lib)?;
        }

        let mut vertex_offset = 0usize;
        let mut normal_offset = 0usize;
        let mut uv_offset = 0usize;

        for obj in &data.objects {
            writeln!(writer, "o {}", obj.name)?;

            for group in &obj.groups {
                // Export all vertices for this group first
                for pos in &group.mesh.positions {
                    writeln!(
                        writer,
                        "v {:.prec$} {:.prec$} {:.prec$}",
                        pos.x,
                        pos.y,
                        pos.z,
                        prec = self.precision
                    )?;
                }

                // Export texture coordinates
                if self.export_uvs {
                    for uv in &group.mesh.uvs {
                        writeln!(
                            writer,
                            "vt {:.prec$} {:.prec$}",
                            uv.x,
                            uv.y,
                            prec = self.precision
                        )?;
                    }
                }

                // Export normals
                if self.export_normals {
                    for normal in &group.mesh.normals {
                        writeln!(
                            writer,
                            "vn {:.prec$} {:.prec$} {:.prec$}",
                            normal.x,
                            normal.y,
                            normal.z,
                            prec = self.precision
                        )?;
                    }
                }

                // Export group and material
                if !group.name.is_empty() && group.name != "default" {
                    writeln!(writer, "g {}", group.name)?;
                }
                if let Some(ref mat) = group.material {
                    writeln!(writer, "usemtl {}", mat)?;
                }

                // Export faces
                let has_uvs = self.export_uvs && !group.mesh.uvs.is_empty();
                let has_normals = self.export_normals && !group.mesh.normals.is_empty();

                for face in &group.mesh.faces {
                    write!(writer, "f")?;
                    for &idx in face {
                        let v_idx = vertex_offset + idx + 1;

                        if has_uvs && has_normals {
                            let vt_idx = uv_offset + idx + 1;
                            let vn_idx = normal_offset + idx + 1;
                            write!(writer, " {}/{}/{}", v_idx, vt_idx, vn_idx)?;
                        } else if has_uvs {
                            let vt_idx = uv_offset + idx + 1;
                            write!(writer, " {}/{}", v_idx, vt_idx)?;
                        } else if has_normals {
                            let vn_idx = normal_offset + idx + 1;
                            write!(writer, " {}//{}", v_idx, vn_idx)?;
                        } else {
                            write!(writer, " {}", v_idx)?;
                        }
                    }
                    writeln!(writer)?;
                }

                vertex_offset += group.mesh.positions.len();
                if has_normals {
                    normal_offset += group.mesh.normals.len();
                }
                if has_uvs {
                    uv_offset += group.mesh.uvs.len();
                }
            }
        }

        Ok(())
    }

    /// Export a single mesh to OBJ format.
    pub fn export_mesh<W: Write>(
        &self,
        mut writer: W,
        mesh: &MeshData,
        name: &str,
    ) -> ObjResult<()> {
        writeln!(writer, "# NAT3D OBJ Export")?;
        writeln!(writer, "o {}", name)?;

        // Export vertices
        for pos in &mesh.positions {
            writeln!(
                writer,
                "v {:.prec$} {:.prec$} {:.prec$}",
                pos.x,
                pos.y,
                pos.z,
                prec = self.precision
            )?;
        }

        // Export texture coordinates
        if self.export_uvs {
            for uv in &mesh.uvs {
                writeln!(
                    writer,
                    "vt {:.prec$} {:.prec$}",
                    uv.x,
                    uv.y,
                    prec = self.precision
                )?;
            }
        }

        // Export normals
        if self.export_normals {
            for normal in &mesh.normals {
                writeln!(
                    writer,
                    "vn {:.prec$} {:.prec$} {:.prec$}",
                    normal.x,
                    normal.y,
                    normal.z,
                    prec = self.precision
                )?;
            }
        }

        // Export faces
        let has_uvs = self.export_uvs && !mesh.uvs.is_empty();
        let has_normals = self.export_normals && !mesh.normals.is_empty();

        for face in &mesh.faces {
            write!(writer, "f")?;
            for &idx in face {
                let v_idx = idx + 1;

                if has_uvs && has_normals {
                    write!(writer, " {}/{}/{}", v_idx, v_idx, v_idx)?;
                } else if has_uvs {
                    write!(writer, " {}/{}", v_idx, v_idx)?;
                } else if has_normals {
                    write!(writer, " {}//{}", v_idx, v_idx)?;
                } else {
                    write!(writer, " {}", v_idx)?;
                }
            }
            writeln!(writer)?;
        }

        Ok(())
    }
}

/// Compute vertex normals from face data using area-weighted averaging.
fn compute_normals(mesh: &MeshData) -> Vec<Normal> {
    let mut normals = vec![Normal::new(0.0, 0.0, 0.0); mesh.positions.len()];

    // Accumulate face normals weighted by area
    for face in &mesh.faces {
        if face.len() < 3 {
            continue;
        }

        // Compute face normal (using first 3 vertices)
        let p0 = mesh.positions[face[0]];
        let p1 = mesh.positions[face[1]];
        let p2 = mesh.positions[face[2]];

        let v1 = p1 - p0;
        let v2 = p2 - p0;
        let face_normal = v1.cross(&v2);

        // Add to vertex normals
        for &idx in face {
            normals[idx] += face_normal;
        }
    }

    // Normalize
    for normal in &mut normals {
        let len = normal.norm();
        if len > 1e-10 {
            *normal /= len;
        } else {
            *normal = Normal::new(0.0, 1.0, 0.0);
        }
    }

    normals
}

/// Helper functions for quick import/export.
pub fn import_obj<P: AsRef<Path>>(path: P) -> ObjResult<ObjData> {
    ObjImporter::new().import_file(path)
}

/// Export mesh data to an OBJ file.
pub fn export_obj<P: AsRef<Path>>(path: P, data: &ObjData) -> ObjResult<()> {
    ObjExporter::new().export_file(path, data)
}

/// Export a single mesh to an OBJ file.
pub fn export_mesh_obj<P: AsRef<Path>>(path: P, mesh: &MeshData, name: &str) -> ObjResult<()> {
    let file = std::fs::File::create(path)?;
    ObjExporter::new().export_mesh(file, mesh, name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_cube() {
        let obj_data = r#"
# Simple cube
v 0 0 0
v 1 0 0
v 1 1 0
v 0 1 0
v 0 0 1
v 1 0 1
v 1 1 1
v 0 1 1

f 1 2 3 4
f 5 6 7 8
f 1 2 6 5
f 2 3 7 6
f 3 4 8 7
f 4 1 5 8
"#;
        let reader = BufReader::new(obj_data.as_bytes());
        let result = ObjImporter::new().import_reader(reader);
        assert!(result.is_ok());

        let data = result.unwrap();
        assert_eq!(data.objects.len(), 1);
        assert!(!data.objects[0].groups[0].mesh.faces.is_empty());
    }

    #[test]
    fn test_negative_indices() {
        let obj_data = r#"
v 0 0 0
v 1 0 0
v 1 1 0
f -3 -2 -1
"#;
        let reader = BufReader::new(obj_data.as_bytes());
        let result = ObjImporter::new().import_reader(reader);
        assert!(result.is_ok());
    }

    #[test]
    fn test_export_roundtrip() {
        let mesh = MeshData {
            name: "test".to_string(),
            positions: vec![
                Position::new(0.0, 0.0, 0.0),
                Position::new(1.0, 0.0, 0.0),
                Position::new(0.5, 1.0, 0.0),
            ],
            normals: vec![
                Normal::new(0.0, 0.0, 1.0),
                Normal::new(0.0, 0.0, 1.0),
                Normal::new(0.0, 0.0, 1.0),
            ],
            uvs: vec![],
            faces: vec![vec![0, 1, 2]],
            material_indices: vec![],
        };

        let mut output = Vec::new();
        ObjExporter::new()
            .export_mesh(&mut output, &mesh, "triangle")
            .unwrap();

        let obj_str = String::from_utf8(output).unwrap();
        assert!(obj_str.contains("v 0.000000 0.000000 0.000000"));
        assert!(obj_str.contains("f 1//1 2//2 3//3"));
    }
}
