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

//! DXF (Drawing Exchange Format) import/export.
//!
//! DXF is AutoCAD's text-based CAD exchange format.
//! It uses group codes (integers) followed by values.

use nat3d_core::geometry::mesh::MeshData;
use nat3d_core::Position;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use thiserror::Error;

/// DXF format errors.
#[derive(Error, Debug)]
pub enum DxfError {
    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// Parse error.
    #[error("Parse error: {0}")]
    Parse(String),
    /// Unsupported entity.
    #[error("Unsupported entity type: {0}")]
    UnsupportedEntity(String),
}

/// Result type for DXF operations.
pub type DxfResult<T> = Result<T, DxfError>;

/// DXF entity types.
#[derive(Debug, Clone)]
pub enum DxfEntity {
    /// 3D face (4 vertices).
    Face3D {
        /// Vertices.
        vertices: [[f64; 3]; 4],
    },
    /// Line.
    Line {
        /// Start point.
        start: [f64; 3],
        /// End point.
        end: [f64; 3],
    },
    /// Polyline with 3D vertices.
    Polyline {
        /// Vertices.
        vertices: Vec<[f64; 3]>,
        /// Closed flag.
        closed: bool,
    },
    /// Circle.
    Circle {
        /// Center.
        center: [f64; 3],
        /// Radius.
        radius: f64,
    },
}

/// DXF file data.
#[derive(Debug, Clone, Default)]
pub struct DxfData {
    /// Entities extracted from the file.
    pub entities: Vec<DxfEntity>,
}

impl DxfData {
    /// Create new empty DXF data.
    pub fn new() -> Self {
        Self::default()
    }

    /// Convert to mesh data.
    pub fn to_mesh(&self) -> MeshData {
        let mut positions = Vec::new();
        let mut faces = Vec::new();

        for entity in &self.entities {
            match entity {
                DxfEntity::Face3D { vertices } => {
                    let base = positions.len();
                    for &v in vertices {
                        positions.push(Position::new(v[0], v[1], v[2]));
                    }
                    // Check if it's a triangle (4th vertex same as 3rd)
                    if vertices[2] == vertices[3] {
                        faces.push(vec![base, base + 1, base + 2]);
                    } else {
                        faces.push(vec![base, base + 1, base + 2, base + 3]);
                    }
                }
                DxfEntity::Line { start, end } => {
                    let base = positions.len();
                    positions.push(Position::new(start[0], start[1], start[2]));
                    positions.push(Position::new(end[0], end[1], end[2]));
                    faces.push(vec![base, base + 1]);
                }
                DxfEntity::Polyline { vertices, .. } => {
                    if vertices.len() >= 3 {
                        let base = positions.len();
                        for &v in vertices {
                            positions.push(Position::new(v[0], v[1], v[2]));
                        }
                        // Create face from polyline
                        let face: Vec<usize> = (base..base + vertices.len()).collect();
                        faces.push(face);
                    }
                }
                _ => {} // Skip other entity types for now
            }
        }

        MeshData {
            name: "DXF_Mesh".to_string(),
            positions,
            normals: Vec::new(),
            uvs: Vec::new(),
            faces,
            material_indices: Vec::new(),
        }
    }
}

/// DXF importer.
pub struct DxfImporter;

impl Default for DxfImporter {
    fn default() -> Self {
        Self::new()
    }
}

impl DxfImporter {
    /// Create new importer.
    pub fn new() -> Self {
        Self
    }

    /// Import DXF file.
    pub fn import_file<P: AsRef<Path>>(&self, path: P) -> DxfResult<DxfData> {
        let file = std::fs::File::open(path)?;
        self.import_reader(BufReader::new(file))
    }

    /// Import from reader.
    pub fn import_reader<R: Read>(&self, reader: BufReader<R>) -> DxfResult<DxfData> {
        let mut data = DxfData::new();
        let mut lines = reader.lines();
        let mut in_entities = false;

        while let Some(Ok(line)) = lines.next() {
            let line = line.trim();

            // Check for ENTITIES section
            if line == "ENTITIES" {
                in_entities = true;
                continue;
            }

            if line == "ENDSEC" {
                in_entities = false;
                continue;
            }

            if !in_entities {
                continue;
            }

            // Parse entity
            if line == "0" {
                if let Some(Ok(entity_type)) = lines.next() {
                    match entity_type.trim() {
                        "3DFACE" => {
                            if let Some(entity) = self.parse_3dface(&mut lines)? {
                                data.entities.push(entity);
                            }
                        }
                        "LINE" => {
                            if let Some(entity) = self.parse_line(&mut lines)? {
                                data.entities.push(entity);
                            }
                        }
                        "POLYLINE" => {
                            if let Some(entity) = self.parse_polyline(&mut lines)? {
                                data.entities.push(entity);
                            }
                        }
                        "CIRCLE" => {
                            if let Some(entity) = self.parse_circle(&mut lines)? {
                                data.entities.push(entity);
                            }
                        }
                        _ => {} // Skip unknown entities
                    }
                }
            }
        }

        Ok(data)
    }

    fn parse_3dface<R: BufRead>(
        &self,
        lines: &mut std::io::Lines<R>,
    ) -> DxfResult<Option<DxfEntity>> {
        let mut vertices = [[0.0, 0.0, 0.0]; 4];
        while let Some(Ok(line)) = lines.next() {
            let code: i32 = line.trim().parse().unwrap_or(-1);

            if code == 0 {
                break; // Next entity
            }

            if let Some(Ok(value_line)) = lines.next() {
                let value = value_line.trim();

                // Group codes for 3DFACE:
                // 10, 20, 30 = first corner
                // 11, 21, 31 = second corner
                // 12, 22, 32 = third corner
                // 13, 23, 33 = fourth corner
                match code {
                    10..=13 => {
                        let idx = (code - 10) as usize;
                        vertices[idx][0] = value.parse().unwrap_or(0.0);
                    }
                    20..=23 => {
                        let idx = (code - 20) as usize;
                        vertices[idx][1] = value.parse().unwrap_or(0.0);
                    }
                    30..=33 => {
                        let idx = (code - 30) as usize;
                        vertices[idx][2] = value.parse().unwrap_or(0.0);
                    }
                    _ => {}
                }
            }
        }

        Ok(Some(DxfEntity::Face3D { vertices }))
    }

    fn parse_line<R: BufRead>(
        &self,
        lines: &mut std::io::Lines<R>,
    ) -> DxfResult<Option<DxfEntity>> {
        let mut start = [0.0, 0.0, 0.0];
        let mut end = [0.0, 0.0, 0.0];

        while let Some(Ok(line)) = lines.next() {
            let code: i32 = line.trim().parse().unwrap_or(-1);

            if code == 0 {
                break;
            }

            if let Some(Ok(value_line)) = lines.next() {
                let value = value_line.trim();

                match code {
                    10 => start[0] = value.parse().unwrap_or(0.0),
                    20 => start[1] = value.parse().unwrap_or(0.0),
                    30 => start[2] = value.parse().unwrap_or(0.0),
                    11 => end[0] = value.parse().unwrap_or(0.0),
                    21 => end[1] = value.parse().unwrap_or(0.0),
                    31 => end[2] = value.parse().unwrap_or(0.0),
                    _ => {}
                }
            }
        }

        Ok(Some(DxfEntity::Line { start, end }))
    }

    fn parse_polyline<R: BufRead>(
        &self,
        lines: &mut std::io::Lines<R>,
    ) -> DxfResult<Option<DxfEntity>> {
        let mut vertices = Vec::new();
        let closed = false;

        // Read VERTEX entities until SEQEND
        while let Some(Ok(line)) = lines.next() {
            let code: i32 = line.trim().parse().unwrap_or(-1);

            if code == 0 {
                if let Some(Ok(entity_type)) = lines.next() {
                    if entity_type.trim() == "SEQEND" {
                        break;
                    }
                    if entity_type.trim() == "VERTEX" {
                        let mut vertex = [0.0, 0.0, 0.0];

                        while let Some(Ok(v_line)) = lines.next() {
                            let v_code: i32 = v_line.trim().parse().unwrap_or(-1);

                            if v_code == 0 {
                                break;
                            }

                            if let Some(Ok(v_value)) = lines.next() {
                                let val = v_value.trim();
                                match v_code {
                                    10 => vertex[0] = val.parse().unwrap_or(0.0),
                                    20 => vertex[1] = val.parse().unwrap_or(0.0),
                                    30 => vertex[2] = val.parse().unwrap_or(0.0),
                                    _ => {}
                                }
                            }
                        }
                        vertices.push(vertex);
                    }
                }
            }
        }

        Ok(Some(DxfEntity::Polyline { vertices, closed }))
    }

    fn parse_circle<R: BufRead>(
        &self,
        lines: &mut std::io::Lines<R>,
    ) -> DxfResult<Option<DxfEntity>> {
        let mut center = [0.0, 0.0, 0.0];
        let mut radius = 0.0;

        while let Some(Ok(line)) = lines.next() {
            let code: i32 = line.trim().parse().unwrap_or(-1);

            if code == 0 {
                break;
            }

            if let Some(Ok(value_line)) = lines.next() {
                let value = value_line.trim();

                match code {
                    10 => center[0] = value.parse().unwrap_or(0.0),
                    20 => center[1] = value.parse().unwrap_or(0.0),
                    30 => center[2] = value.parse().unwrap_or(0.0),
                    40 => radius = value.parse().unwrap_or(0.0),
                    _ => {}
                }
            }
        }

        Ok(Some(DxfEntity::Circle { center, radius }))
    }
}

/// DXF exporter.
pub struct DxfExporter;

impl Default for DxfExporter {
    fn default() -> Self {
        Self::new()
    }
}

impl DxfExporter {
    /// Create new exporter.
    pub fn new() -> Self {
        Self
    }

    /// Export to file.
    pub fn export_file<P: AsRef<Path>>(&self, path: P, data: &DxfData) -> DxfResult<()> {
        let file = std::fs::File::create(path)?;
        self.export_writer(file, data)
    }

    /// Export to writer.
    pub fn export_writer<W: Write>(&self, mut writer: W, data: &DxfData) -> DxfResult<()> {
        // Write header
        writeln!(writer, "0")?;
        writeln!(writer, "SECTION")?;
        writeln!(writer, "2")?;
        writeln!(writer, "HEADER")?;
        writeln!(writer, "0")?;
        writeln!(writer, "ENDSEC")?;

        // Write entities
        writeln!(writer, "0")?;
        writeln!(writer, "SECTION")?;
        writeln!(writer, "2")?;
        writeln!(writer, "ENTITIES")?;

        for entity in &data.entities {
            match entity {
                DxfEntity::Face3D { vertices } => {
                    writeln!(writer, "0")?;
                    writeln!(writer, "3DFACE")?;
                    for (i, v) in vertices.iter().enumerate() {
                        writeln!(writer, "{}", 10 + i)?;
                        writeln!(writer, "{}", v[0])?;
                        writeln!(writer, "{}", 20 + i)?;
                        writeln!(writer, "{}", v[1])?;
                        writeln!(writer, "{}", 30 + i)?;
                        writeln!(writer, "{}", v[2])?;
                    }
                }
                DxfEntity::Line { start, end } => {
                    writeln!(writer, "0")?;
                    writeln!(writer, "LINE")?;
                    writeln!(writer, "10")?;
                    writeln!(writer, "{}", start[0])?;
                    writeln!(writer, "20")?;
                    writeln!(writer, "{}", start[1])?;
                    writeln!(writer, "30")?;
                    writeln!(writer, "{}", start[2])?;
                    writeln!(writer, "11")?;
                    writeln!(writer, "{}", end[0])?;
                    writeln!(writer, "21")?;
                    writeln!(writer, "{}", end[1])?;
                    writeln!(writer, "31")?;
                    writeln!(writer, "{}", end[2])?;
                }
                _ => {} // Skip other types
            }
        }

        writeln!(writer, "0")?;
        writeln!(writer, "ENDSEC")?;
        writeln!(writer, "0")?;
        writeln!(writer, "EOF")?;

        Ok(())
    }
}

/// Import DXF file.
pub fn import_dxf<P: AsRef<Path>>(path: P) -> DxfResult<DxfData> {
    DxfImporter::new().import_file(path)
}

/// Export DXF file.
pub fn export_dxf<P: AsRef<Path>>(path: P, data: &DxfData) -> DxfResult<()> {
    DxfExporter::new().export_file(path, data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dxf_entity_types() {
        let face = DxfEntity::Face3D {
            vertices: [[0.0, 0.0, 0.0]; 4],
        };

        match face {
            DxfEntity::Face3D { .. } => {}
            _ => panic!("Wrong type"),
        }
    }

    #[test]
    fn test_export() {
        let mut data = DxfData::new();
        data.entities.push(DxfEntity::Line {
            start: [0.0, 0.0, 0.0],
            end: [1.0, 1.0, 1.0],
        });

        let mut output = Vec::new();
        DxfExporter::new()
            .export_writer(&mut output, &data)
            .unwrap();

        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("LINE"));
        assert!(result.contains("ENTITIES"));
    }
}
