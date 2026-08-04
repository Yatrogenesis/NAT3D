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

//! Real FBX (Filmbox) geometry extractor using fbxcel-dom.
//! Extracts meshes from binary FBX 7.4+ files.

use nalgebra;
use nat3d_core::geometry::mesh::MeshData;
use fbxcel_dom::any::AnyDocument;
use fbxcel_dom::v7400::object::TypedObjectHandle;
use fbxcel_dom::v7400::object::model::TypedModelHandle;
use anyhow::{Result, anyhow};
use std::path::Path;
use std::fs::File;
use std::io::BufReader;

pub struct FbxImporter;

impl FbxImporter {
    pub fn new() -> Self { Self }

    pub fn import<P: AsRef<Path>>(&self, path: P) -> Result<MeshData> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        
        let doc = AnyDocument::from_seekable_reader(reader)
            .map_err(|e| anyhow!("Failed to parse FBX: {}", e))?;
            
        let doc = match doc {
            AnyDocument::V7400(_, v) => v,
            _ => return Err(anyhow!("Unsupported FBX version (only 7.4+ is supported)")),
        };

        // REAL extraction loop
        for object in doc.objects() {
            if let TypedObjectHandle::Model(model) = object.get_typed() {
                if let TypedModelHandle::Mesh(mesh_handle) = model {
                    let mut mesh_data = MeshData::new(mesh_handle.name().unwrap_or("Imported_FBX"));
                    
                    let geometry = mesh_handle.geometry()
                        .map_err(|e| anyhow!("Failed to get geometry: {}", e))?;
                    
                    // Access node directly to ensure we get the arrays (API work-around)
                    let node = geometry.node();
                    
                    if let Some(vertices_node) = node.children().find(|c| c.name() == "Vertices") {
                        if let Some(attr) = vertices_node.attributes().get(0) {
                            if let Some(arr) = attr.get_arr_f64() {
                                for chunk in arr.chunks(3) {
                                    if chunk.len() == 3 {
                                        mesh_data.positions.push(nalgebra::Point3::new(chunk[0], chunk[1], chunk[2]));
                                    }
                                }
                            }
                        }
                    }
                    
                    if let Some(indices_node) = node.children().find(|c| c.name() == "PolygonVertexIndex") {
                        if let Some(attr) = indices_node.attributes().get(0) {
                            if let Some(arr) = attr.get_arr_i32() {
                                let mut face = Vec::new();
                                for &i in arr {
                                    if i < 0 {
                                        face.push((-i - 1) as usize);
                                        mesh_data.faces.push(face.clone());
                                        face.clear();
                                    } else {
                                        face.push(i as usize);
                                    }
                                }
                            }
                        }
                    }

                    if !mesh_data.positions.is_empty() {
                        return Ok(mesh_data);
                    }
                }
            }
        }
        
        Err(anyhow!("No valid mesh geometry found in FBX file"))
    }
}
