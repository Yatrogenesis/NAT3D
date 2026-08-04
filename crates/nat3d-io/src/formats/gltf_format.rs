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

//! glTF 2.0 format import/export.
//!
//! glTF (GL Transmission Format) is a modern, efficient format for 3D scenes:
//! - JSON-based structure with binary data buffers
//! - PBR materials
//! - Animations and skinning
//! - Scene hierarchy
//! - Extensions for additional features

use nat3d_core::geometry::mesh::MeshData;
use nat3d_core::{Normal, Position, TexCoord};
use serde_json::json;
use std::io::Write;
use std::path::Path;
use thiserror::Error;

/// glTF format errors.
#[derive(Error, Debug)]
pub enum GltfError {
    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// glTF library error.
    #[error("glTF error: {0}")]
    Gltf(#[from] gltf::Error),
    /// JSON error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    /// Invalid data.
    #[error("Invalid data: {0}")]
    InvalidData(String),
    /// Missing attribute.
    #[error("Missing required attribute: {0}")]
    MissingAttribute(String),
}

/// Result type for glTF operations.
pub type GltfResult<T> = Result<T, GltfError>;

/// Imported glTF scene data.
#[derive(Debug, Clone, Default)]
pub struct GltfScene {
    /// Scene name.
    pub name: String,
    /// Meshes in the scene.
    pub meshes: Vec<GltfMesh>,
    /// Materials.
    pub materials: Vec<GltfMaterial>,
}

/// A mesh from a glTF file.
#[derive(Debug, Clone, Default)]
pub struct GltfMesh {
    /// Mesh name.
    pub name: String,
    /// Primitives (sub-meshes with different materials).
    pub primitives: Vec<MeshData>,
    /// Material indices for each primitive.
    pub material_indices: Vec<Option<usize>>,
}

/// A material from a glTF file.
#[derive(Debug, Clone)]
pub struct GltfMaterial {
    /// Material name.
    pub name: String,
    /// Base color factor (RGBA).
    pub base_color: [f32; 4],
    /// Metallic factor.
    pub metallic: f32,
    /// Roughness factor.
    pub roughness: f32,
    /// Emissive factor (RGB).
    pub emissive: [f32; 3],
    /// Alpha mode.
    pub alpha_mode: AlphaMode,
    /// Alpha cutoff for mask mode.
    pub alpha_cutoff: f32,
    /// Double-sided.
    pub double_sided: bool,
}

impl Default for GltfMaterial {
    fn default() -> Self {
        Self {
            name: String::new(),
            base_color: [1.0, 1.0, 1.0, 1.0],
            metallic: 0.0,
            roughness: 1.0,
            emissive: [0.0, 0.0, 0.0],
            alpha_mode: AlphaMode::Opaque,
            alpha_cutoff: 0.5,
            double_sided: false,
        }
    }
}

/// Alpha blending mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlphaMode {
    /// Fully opaque.
    #[default]
    Opaque,
    /// Binary transparency (cutoff).
    Mask,
    /// Alpha blending.
    Blend,
}

/// glTF importer.
pub struct GltfImporter {
    /// Whether to compute normals if not provided.
    pub compute_normals: bool,
    /// Whether to include all scenes (true) or just the default scene (false).
    pub all_scenes: bool,
}

impl Default for GltfImporter {
    fn default() -> Self {
        Self {
            compute_normals: true,
            all_scenes: false,
        }
    }
}

impl GltfImporter {
    /// Create a new glTF importer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Import a glTF file from a path.
    pub fn import_file<P: AsRef<Path>>(&self, path: P) -> GltfResult<GltfScene> {
        let (document, buffers, _images) = gltf::import(path)?;
        self.import_document(&document, &buffers)
    }

    /// Import from glTF document and buffers.
    fn import_document(
        &self,
        document: &gltf::Document,
        buffers: &[gltf::buffer::Data],
    ) -> GltfResult<GltfScene> {
        let mut scene = GltfScene::default();

        // Import materials
        for material in document.materials() {
            scene.materials.push(self.import_material(&material));
        }

        // Import meshes
        for mesh in document.meshes() {
            scene.meshes.push(self.import_mesh(&mesh, buffers)?);
        }

        // Get scene name
        if let Some(gltf_scene) = document.default_scene() {
            scene.name = gltf_scene.name().unwrap_or("Scene").to_string();
        }

        Ok(scene)
    }

    fn import_material(&self, material: &gltf::Material) -> GltfMaterial {
        let pbr = material.pbr_metallic_roughness();

        GltfMaterial {
            name: material.name().unwrap_or("Material").to_string(),
            base_color: pbr.base_color_factor(),
            metallic: pbr.metallic_factor(),
            roughness: pbr.roughness_factor(),
            emissive: material.emissive_factor(),
            alpha_mode: match material.alpha_mode() {
                gltf::material::AlphaMode::Opaque => AlphaMode::Opaque,
                gltf::material::AlphaMode::Mask => AlphaMode::Mask,
                gltf::material::AlphaMode::Blend => AlphaMode::Blend,
            },
            alpha_cutoff: material.alpha_cutoff().unwrap_or(0.5),
            double_sided: material.double_sided(),
        }
    }

    fn import_mesh(
        &self,
        mesh: &gltf::Mesh,
        buffers: &[gltf::buffer::Data],
    ) -> GltfResult<GltfMesh> {
        let mut gltf_mesh = GltfMesh {
            name: mesh.name().unwrap_or("Mesh").to_string(),
            primitives: Vec::new(),
            material_indices: Vec::new(),
        };

        for primitive in mesh.primitives() {
            let mesh_data = self.import_primitive(&primitive, buffers)?;
            gltf_mesh.primitives.push(mesh_data);
            gltf_mesh
                .material_indices
                .push(primitive.material().index());
        }

        Ok(gltf_mesh)
    }

    fn import_primitive(
        &self,
        primitive: &gltf::Primitive,
        buffers: &[gltf::buffer::Data],
    ) -> GltfResult<MeshData> {
        let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

        // Read positions (required)
        let positions: Vec<Position> = reader
            .read_positions()
            .ok_or_else(|| GltfError::MissingAttribute("POSITION".to_string()))?
            .map(|p| Position::new(p[0] as f64, p[1] as f64, p[2] as f64))
            .collect();

        // Read normals (optional)
        let normals: Vec<Normal> = if let Some(iter) = reader.read_normals() {
            iter.map(|n| Normal::new(n[0] as f64, n[1] as f64, n[2] as f64))
                .collect()
        } else {
            vec![]
        };

        // Read texture coordinates (optional)
        let uvs: Vec<TexCoord> = if let Some(iter) = reader.read_tex_coords(0) {
            iter.into_f32()
                .map(|uv| TexCoord::new(uv[0] as f64, uv[1] as f64))
                .collect()
        } else {
            vec![]
        };

        // Read indices
        let faces = if let Some(indices) = reader.read_indices() {
            let indices: Vec<u32> = indices.into_u32().collect();

            match primitive.mode() {
                gltf::mesh::Mode::Triangles => indices
                    .chunks(3)
                    .map(|chunk| vec![chunk[0] as usize, chunk[1] as usize, chunk[2] as usize])
                    .collect(),
                gltf::mesh::Mode::TriangleStrip => {
                    let mut faces = Vec::new();
                    for i in 0..indices.len().saturating_sub(2) {
                        if i % 2 == 0 {
                            faces.push(vec![
                                indices[i] as usize,
                                indices[i + 1] as usize,
                                indices[i + 2] as usize,
                            ]);
                        } else {
                            faces.push(vec![
                                indices[i] as usize,
                                indices[i + 2] as usize,
                                indices[i + 1] as usize,
                            ]);
                        }
                    }
                    faces
                }
                gltf::mesh::Mode::TriangleFan => {
                    let mut faces = Vec::new();
                    for i in 1..indices.len().saturating_sub(1) {
                        faces.push(vec![
                            indices[0] as usize,
                            indices[i] as usize,
                            indices[i + 1] as usize,
                        ]);
                    }
                    faces
                }
                _ => {
                    return Err(GltfError::InvalidData(format!(
                        "Unsupported primitive mode: {:?}",
                        primitive.mode()
                    )));
                }
            }
        } else {
            // No indices - create faces from vertex order
            match primitive.mode() {
                gltf::mesh::Mode::Triangles => (0..positions.len())
                    .collect::<Vec<_>>()
                    .chunks(3)
                    .map(|chunk| chunk.to_vec())
                    .collect(),
                _ => {
                    return Err(GltfError::InvalidData(
                        "Non-indexed non-triangle primitives not supported".to_string(),
                    ));
                }
            }
        };

        let mut mesh_data = MeshData {
            name: String::new(),
            positions,
            normals,
            uvs,
            faces,
            material_indices: vec![],
        };

        // Compute normals if needed
        if mesh_data.normals.is_empty() && self.compute_normals {
            mesh_data.normals = compute_normals(&mesh_data);
        }

        Ok(mesh_data)
    }
}

/// glTF exporter.
pub struct GltfExporter {
    /// Whether to export as binary GLB (true) or JSON glTF (false).
    pub binary: bool,
}

impl Default for GltfExporter {
    fn default() -> Self {
        Self { binary: true }
    }
}

impl GltfExporter {
    /// Create a new glTF exporter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Export a mesh to glTF format.
    pub fn export_mesh<P: AsRef<Path>>(
        &self,
        path: P,
        mesh: &MeshData,
        name: &str,
    ) -> GltfResult<()> {
        // Build binary buffer with vertex and index data
        let mut buffer_data = Vec::new();

        // Write positions
        let positions_offset = buffer_data.len();
        for pos in &mesh.positions {
            buffer_data.extend_from_slice(&(pos.x as f32).to_le_bytes());
            buffer_data.extend_from_slice(&(pos.y as f32).to_le_bytes());
            buffer_data.extend_from_slice(&(pos.z as f32).to_le_bytes());
        }
        let positions_length = buffer_data.len() - positions_offset;

        // Write normals
        let normals_offset = buffer_data.len();
        for normal in &mesh.normals {
            buffer_data.extend_from_slice(&(normal.x as f32).to_le_bytes());
            buffer_data.extend_from_slice(&(normal.y as f32).to_le_bytes());
            buffer_data.extend_from_slice(&(normal.z as f32).to_le_bytes());
        }
        let normals_length = buffer_data.len() - normals_offset;

        // Write UVs
        let uvs_offset = buffer_data.len();
        for uv in &mesh.uvs {
            buffer_data.extend_from_slice(&(uv.x as f32).to_le_bytes());
            buffer_data.extend_from_slice(&(uv.y as f32).to_le_bytes());
        }
        let uvs_length = buffer_data.len() - uvs_offset;

        // Write indices (triangulated)
        let indices_offset = buffer_data.len();
        let mut index_count = 0u32;
        for face in &mesh.faces {
            if face.len() >= 3 {
                for i in 1..face.len() - 1 {
                    buffer_data.extend_from_slice(&(face[0] as u32).to_le_bytes());
                    buffer_data.extend_from_slice(&(face[i] as u32).to_le_bytes());
                    buffer_data.extend_from_slice(&(face[i + 1] as u32).to_le_bytes());
                    index_count += 3;
                }
            }
        }
        let indices_length = buffer_data.len() - indices_offset;

        // Compute bounding box
        let (min, max) = compute_bounds(&mesh.positions);

        // Build buffer views and accessors
        let mut buffer_views = Vec::new();
        let mut accessors = Vec::new();
        let mut attributes = serde_json::Map::new();

        // Positions
        buffer_views.push(json!({
            "buffer": 0,
            "byteOffset": positions_offset,
            "byteLength": positions_length,
            "target": 34962
        }));
        accessors.push(json!({
            "bufferView": 0,
            "componentType": 5126,
            "count": mesh.positions.len(),
            "type": "VEC3",
            "min": [min[0], min[1], min[2]],
            "max": [max[0], max[1], max[2]]
        }));
        attributes.insert("POSITION".to_string(), json!(0));

        let mut accessor_idx = 1;

        // Normals
        if !mesh.normals.is_empty() {
            buffer_views.push(json!({
                "buffer": 0,
                "byteOffset": normals_offset,
                "byteLength": normals_length,
                "target": 34962
            }));
            accessors.push(json!({
                "bufferView": accessor_idx,
                "componentType": 5126,
                "count": mesh.normals.len(),
                "type": "VEC3"
            }));
            attributes.insert("NORMAL".to_string(), json!(accessor_idx));
            accessor_idx += 1;
        }

        // UVs
        if !mesh.uvs.is_empty() {
            buffer_views.push(json!({
                "buffer": 0,
                "byteOffset": uvs_offset,
                "byteLength": uvs_length,
                "target": 34962
            }));
            accessors.push(json!({
                "bufferView": accessor_idx,
                "componentType": 5126,
                "count": mesh.uvs.len(),
                "type": "VEC2"
            }));
            attributes.insert("TEXCOORD_0".to_string(), json!(accessor_idx));
            accessor_idx += 1;
        }

        // Indices
        let indices_accessor = if index_count > 0 {
            buffer_views.push(json!({
                "buffer": 0,
                "byteOffset": indices_offset,
                "byteLength": indices_length,
                "target": 34963
            }));
            accessors.push(json!({
                "bufferView": accessor_idx,
                "componentType": 5125,
                "count": index_count,
                "type": "SCALAR"
            }));
            Some(accessor_idx)
        } else {
            None
        };

        // Build primitive
        let mut primitive = json!({
            "attributes": attributes,
            "mode": 4
        });
        if let Some(idx) = indices_accessor {
            primitive["indices"] = json!(idx);
        }

        // Build glTF JSON
        let gltf_json = json!({
            "asset": {
                "version": "2.0",
                "generator": "NAT3D"
            },
            "scene": 0,
            "scenes": [{
                "name": name,
                "nodes": [0]
            }],
            "nodes": [{
                "name": name,
                "mesh": 0
            }],
            "meshes": [{
                "name": name,
                "primitives": [primitive]
            }],
            "accessors": accessors,
            "bufferViews": buffer_views,
            "buffers": [{
                "byteLength": buffer_data.len()
            }]
        });

        if self.binary {
            // Write GLB
            let json_bytes = serde_json::to_vec(&gltf_json)?;
            let json_len = json_bytes.len();
            let json_padding = (4 - (json_len % 4)) % 4;

            let bin_len = buffer_data.len();
            let bin_padding = (4 - (bin_len % 4)) % 4;

            let total_len = 12 + 8 + json_len + json_padding + 8 + bin_len + bin_padding;

            let mut file = std::fs::File::create(path)?;

            // GLB header
            file.write_all(b"glTF")?;
            file.write_all(&2u32.to_le_bytes())?;
            file.write_all(&(total_len as u32).to_le_bytes())?;

            // JSON chunk
            file.write_all(&((json_len + json_padding) as u32).to_le_bytes())?;
            file.write_all(&0x4E4F534Au32.to_le_bytes())?; // "JSON"
            file.write_all(&json_bytes)?;
            file.write_all(&vec![0x20u8; json_padding])?;

            // BIN chunk
            file.write_all(&((bin_len + bin_padding) as u32).to_le_bytes())?;
            file.write_all(&0x004E4942u32.to_le_bytes())?; // "BIN\0"
            file.write_all(&buffer_data)?;
            file.write_all(&vec![0u8; bin_padding])?;
        } else {
            // Write glTF JSON (with external buffer)
            let mut gltf_with_uri = gltf_json.clone();
            gltf_with_uri["buffers"][0]["uri"] = json!(format!("{}.bin", name));

            let json_str = serde_json::to_string_pretty(&gltf_with_uri)?;
            std::fs::write(&path, json_str)?;

            // Write binary buffer
            let bin_path = path.as_ref().with_extension("bin");
            std::fs::write(bin_path, &buffer_data)?;
        }

        Ok(())
    }
}

/// Compute bounding box of positions.
fn compute_bounds(positions: &[Position]) -> ([f64; 3], [f64; 3]) {
    if positions.is_empty() {
        return ([0.0; 3], [0.0; 3]);
    }

    let mut min = [f64::MAX; 3];
    let mut max = [f64::MIN; 3];

    for pos in positions {
        min[0] = min[0].min(pos.x);
        min[1] = min[1].min(pos.y);
        min[2] = min[2].min(pos.z);
        max[0] = max[0].max(pos.x);
        max[1] = max[1].max(pos.y);
        max[2] = max[2].max(pos.z);
    }

    (min, max)
}

/// Compute vertex normals from face data.
fn compute_normals(mesh: &MeshData) -> Vec<Normal> {
    let mut normals = vec![Normal::new(0.0, 0.0, 0.0); mesh.positions.len()];

    for face in &mesh.faces {
        if face.len() < 3 {
            continue;
        }

        let p0 = mesh.positions[face[0]];
        let p1 = mesh.positions[face[1]];
        let p2 = mesh.positions[face[2]];

        let v1 = p1 - p0;
        let v2 = p2 - p0;
        let face_normal = v1.cross(&v2);

        for &idx in face {
            normals[idx] += face_normal;
        }
    }

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

/// Import a glTF file.
pub fn import_gltf<P: AsRef<Path>>(path: P) -> GltfResult<GltfScene> {
    GltfImporter::new().import_file(path)
}

/// Export a mesh to glTF/GLB format.
pub fn export_gltf<P: AsRef<Path>>(path: P, mesh: &MeshData, name: &str) -> GltfResult<()> {
    GltfExporter::new().export_mesh(path, mesh, name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_bounds() {
        let positions = vec![
            Position::new(-1.0, -2.0, -3.0),
            Position::new(1.0, 2.0, 3.0),
            Position::new(0.0, 0.0, 0.0),
        ];

        let (min, max) = compute_bounds(&positions);
        assert_eq!(min, [-1.0, -2.0, -3.0]);
        assert_eq!(max, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_compute_normals() {
        let mesh = MeshData {
            name: "test".to_string(),
            positions: vec![
                Position::new(0.0, 0.0, 0.0),
                Position::new(1.0, 0.0, 0.0),
                Position::new(0.0, 1.0, 0.0),
            ],
            normals: vec![],
            uvs: vec![],
            faces: vec![vec![0, 1, 2]],
            material_indices: vec![],
        };

        let normals = compute_normals(&mesh);
        assert_eq!(normals.len(), 3);
        // Normal should point in +Z direction
        assert!((normals[0].z - 1.0).abs() < 1e-6);
    }
}
