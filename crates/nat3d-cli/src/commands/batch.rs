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

// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Francisco Molina-Burgos, Avermex Research Division

//! Batch processing command for multiple files.

use anyhow::{Result, Context};
use std::path::{Path, PathBuf};
use std::fs;
use nat3d_io::{import_obj, import_stl, export_obj, export_mesh_obj, export_mesh_stl};
use nat3d_core::geometry::mesh::MeshData;

/// Batch processing operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchOperation {
    /// Convert between formats.
    Convert,
    /// Optimize meshes (reduce vertices, clean up).
    Optimize,
    /// Validate file integrity.
    Validate,
    /// Generate thumbnails/previews.
    Preview,
}

/// Batch process files.
pub fn process_batch(
    input_pattern: &str,
    output_dir: &Path,
    operation: BatchOperation,
) -> Result<()> {
    println!("NAT3D Batch Processor");
    println!("=====================\n");
    println!("Operation: {:?}", operation);
    println!("Pattern:   {}", input_pattern);
    println!("Output:    {}\n", output_dir.display());

    // Create output directory
    fs::create_dir_all(output_dir)
        .with_context(|| format!("Failed to create output directory: {}", output_dir.display()))?;

    // Find matching files
    let files = find_files(input_pattern)?;

    if files.is_empty() {
        anyhow::bail!("No files match pattern: {}", input_pattern);
    }

    println!("Found {} files to process\n", files.len());

    // Process each file
    let mut success_count = 0;
    let mut error_count = 0;

    for file in &files {
        print!("Processing {}... ", file.file_name().unwrap_or_default().to_string_lossy());

        match process_single_file(file, output_dir, operation) {
            Ok(_) => {
                println!("OK");
                success_count += 1;
            }
            Err(e) => {
                println!("ERROR: {}", e);
                error_count += 1;
            }
        }
    }

    println!("\nCompleted:");
    println!("  Success: {}", success_count);
    println!("  Errors:  {}", error_count);

    if error_count > 0 {
        anyhow::bail!("{} files failed to process", error_count);
    }

    Ok(())
}

/// Find files matching glob pattern.
fn find_files(pattern: &str) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    // Simple pattern matching: *.ext or path/*.ext
    let path = Path::new(pattern);

    if let Some(parent) = path.parent() {
        let file_name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("*");

        let search_dir = if parent.as_os_str().is_empty() {
            Path::new(".")
        } else {
            parent
        };

        if search_dir.is_dir() {
            for entry in fs::read_dir(search_dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_file() {
                    if file_name == "*" || file_name.contains('*') {
                        // Wildcard matching
                        if let Some(ext) = file_name.strip_prefix("*.") {
                            if path.extension().and_then(|e| e.to_str()) == Some(ext) {
                                files.push(path);
                            }
                        }
                    } else if path.file_name() == Some(std::ffi::OsStr::new(file_name)) {
                        files.push(path);
                    }
                }
            }
        }
    }

    files.sort();
    Ok(files)
}

/// Process a single file.
fn process_single_file(
    input: &Path,
    output_dir: &Path,
    operation: BatchOperation,
) -> Result<()> {
    match operation {
        BatchOperation::Convert => convert_file(input, output_dir),
        BatchOperation::Optimize => optimize_file(input, output_dir),
        BatchOperation::Validate => validate_file(input),
        BatchOperation::Preview => generate_preview(input, output_dir),
    }
}

/// Convert file format.
fn convert_file(input: &Path, output_dir: &Path) -> Result<()> {
    let ext = input.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    // Load mesh
    let (vertices, faces) = match ext.as_str() {
        "obj" => {
            let obj_data = import_obj(input)?;
            let mut all_verts = Vec::new();
            let mut all_faces = Vec::new();

            for object in &obj_data.objects {
                for group in &object.groups {
                    for pos in &group.mesh.positions {
                        all_verts.push([pos.x, pos.y, pos.z]);
                    }
                    for face in &group.mesh.faces {
                        all_faces.push(face.clone());
                    }
                }
            }
            (all_verts, all_faces)
        }
        "stl" => {
            let stl_data = import_stl(input)?;
            let mut verts = Vec::new();
            let mut faces = Vec::new();

            for tri in &stl_data.triangles {
                let base = verts.len();
                verts.push([tri.v1[0] as f64, tri.v1[1] as f64, tri.v1[2] as f64]);
                verts.push([tri.v2[0] as f64, tri.v2[1] as f64, tri.v2[2] as f64]);
                verts.push([tri.v3[0] as f64, tri.v3[1] as f64, tri.v3[2] as f64]);
                faces.push(vec![base, base + 1, base + 2]);
            }
            (verts, faces)
        }
        _ => anyhow::bail!("Unsupported input format: {}", ext),
    };

    // Convert to MeshData
    let mut mesh = MeshData::new(input.file_stem().unwrap_or_default().to_string_lossy().as_ref());
    mesh.positions = vertices.iter().map(|v| nat3d_core::Position::new(v[0], v[1], v[2])).collect();
    mesh.faces = faces;

    // Determine output format (toggle OBJ <-> STL)
    let output_ext = match ext.as_str() {
        "obj" => "stl",
        "stl" => "obj",
        _ => "obj",
    };

    let output_file = output_dir.join(format!(
        "{}.{}",
        input.file_stem().unwrap_or_default().to_string_lossy(),
        output_ext
    ));

    // Export
    match output_ext {
        "obj" => export_mesh_obj(&output_file, &mesh, &mesh.name)?,
        "stl" => export_mesh_stl(&output_file, &mesh)?,
        _ => anyhow::bail!("Unsupported output format: {}", output_ext),
    }

    Ok(())
}

/// Optimize mesh file.
fn optimize_file(input: &Path, output_dir: &Path) -> Result<()> {
    let ext = input.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    // Load mesh
    let mut obj_data = match ext.as_str() {
        "obj" => import_obj(input)?,
        _ => anyhow::bail!("Optimize only supports OBJ files currently"),
    };

    // Optimization: Merge duplicate vertices
    let mut merged_count = 0;
    for object in &mut obj_data.objects {
        for group in &mut object.groups {
            let positions = &mut group.mesh.positions;
            if positions.is_empty() { continue; }

            let threshold = 0.0001;
            let mut vertex_map: std::collections::HashMap<(i64, i64, i64), usize> =
                std::collections::HashMap::new();
            let mut new_positions = Vec::new();
            let mut remap = Vec::new();

            // Build map of quantized positions
            for pos in positions.iter() {
                let quantized = (
                    (pos.x / threshold).round() as i64,
                    (pos.y / threshold).round() as i64,
                    (pos.z / threshold).round() as i64,
                );

                if let Some(&existing_idx) = vertex_map.get(&quantized) {
                    remap.push(existing_idx);
                    merged_count += 1;
                } else {
                    let new_idx = new_positions.len();
                    vertex_map.insert(quantized, new_idx);
                    new_positions.push(*pos);
                    remap.push(new_idx);
                }
            }

            // Update faces with remapped indices
            for face in &mut group.mesh.faces {
                for idx in face.iter_mut() {
                    if *idx < remap.len() {
                        *idx = remap[*idx];
                    }
                }
            }
            *positions = new_positions;
        }
    }

    // Save optimized mesh
    let output_file = output_dir.join(input.file_name().unwrap_or_default());
    export_obj(&output_file, &obj_data)?;

    println!("Optimized: {} (merged {} duplicate vertices)", input.display(), merged_count);
    Ok(())
}

/// Validate file integrity.
fn validate_file(input: &Path) -> Result<()> {
    let ext = input.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "obj" => {
            let _obj_data = import_obj(input)?;
            Ok(())
        }
        "stl" => {
            let _triangles = import_stl(input)?;
            Ok(())
        }
        _ => anyhow::bail!("Unsupported format for validation: {}", ext),
    }
}

/// Generate preview/thumbnail.
fn generate_preview(input: &Path, output_dir: &Path) -> Result<()> {
    use std::io::Write;

    let ext = input.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    // Load mesh to get basic info
    let (vertex_count, face_count, bbox_str) = match ext.as_str() {
        "obj" => {
            let obj_data = import_obj(input)?;
            let mut vert_count = 0;
            let mut face_count = 0;
            let mut min = [f64::MAX, f64::MAX, f64::MAX];
            let mut max = [f64::MIN, f64::MIN, f64::MIN];

            for object in &obj_data.objects {
                for group in &object.groups {
                    vert_count += group.mesh.positions.len();
                    face_count += group.mesh.faces.len();
                    for pos in &group.mesh.positions {
                        min[0] = min[0].min(pos.x);
                        min[1] = min[1].min(pos.y);
                        min[2] = min[2].min(pos.z);
                        max[0] = max[0].max(pos.x);
                        max[1] = max[1].max(pos.y);
                        max[2] = max[2].max(pos.z);
                    }
                }
            }
            let bbox = format!("[{:.2},{:.2},{:.2}] to [{:.2},{:.2},{:.2}]",
                min[0], min[1], min[2], max[0], max[1], max[2]);
            (vert_count, face_count, bbox)
        }
        "stl" => {
            let stl_data = import_stl(input)?;
            let face_count = stl_data.triangles.len();
            let vert_count = face_count * 3;
            let bbox = "[bounds not computed for STL]".to_string();
            (vert_count, face_count, bbox)
        }
        _ => anyhow::bail!("Preview only supports OBJ and STL files"),
    };

    // Generate text-based preview (can be enhanced with actual rendering later)
    let preview_name = input.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");
    let preview_file = output_dir.join(format!("{}_preview.txt", preview_name));

    let mut file = fs::File::create(&preview_file)?;
    writeln!(file, "Mesh Preview: {}", input.display())?;
    writeln!(file, "============================================")?;
    writeln!(file, "Format: {}", ext.to_uppercase())?;
    writeln!(file, "Vertices: {}", vertex_count)?;
    writeln!(file, "Faces: {}", face_count)?;
    writeln!(file, "Bounding Box: {}", bbox_str)?;
    writeln!(file, "Complexity: {}", if face_count < 1000 { "Low" }
             else if face_count < 10000 { "Medium" }
             else if face_count < 100000 { "High" }
             else { "Very High" })?;

    println!("Preview generated: {}", preview_file.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_files() {
        // Test finding files in current directory
        let files = find_files("Cargo.toml");
        assert!(files.is_ok());
    }

    #[test]
    fn test_batch_operation() {
        assert_eq!(BatchOperation::Convert, BatchOperation::Convert);
        assert_ne!(BatchOperation::Convert, BatchOperation::Optimize);
    }
}
