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

//! IGES (Initial Graphics Exchange Specification) format import.
//!
//! IGES is a vendor-neutral CAD exchange format using fixed 80-column records.

use nat3d_core::geometry::mesh::MeshData;
use nat3d_core::Position;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use thiserror::Error;

/// IGES format errors.
#[derive(Error, Debug)]
pub enum IgesError {
    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// Parse error.
    #[error("Parse error: {0}")]
    Parse(String),
}

/// Result type for IGES operations.
pub type IgesResult<T> = Result<T, IgesError>;

/// IGES entity types (subset).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IgesEntityType {
    /// Point (type 116).
    Point = 116,
    /// Line (type 110).
    Line = 110,
    /// NURBS Curve (type 126).
    NurbsCurve = 126,
    /// NURBS Surface (type 128).
    NurbsSurface = 128,
    /// Color (type 314).
    Color = 314,
    /// Unknown type.
    Unknown = 0,
}

impl From<i32> for IgesEntityType {
    fn from(value: i32) -> Self {
        match value {
            110 => Self::Line,
            116 => Self::Point,
            126 => Self::NurbsCurve,
            128 => Self::NurbsSurface,
            314 => Self::Color,
            _ => Self::Unknown,
        }
    }
}

/// IGES entity.
#[derive(Debug, Clone)]
pub struct IgesEntity {
    /// Entity type number.
    pub entity_type: IgesEntityType,
    /// Parameter data pointer.
    pub parameter_pointer: usize,
    /// Structure (0 for simple entities).
    pub structure: i32,
    /// Line font pattern.
    pub line_font: i32,
    /// Level.
    pub level: i32,
    /// View.
    pub view: i32,
    /// Transformation matrix.
    pub transformation: i32,
    /// Label display.
    pub label: i32,
    /// Status.
    pub status: [i32; 4],
    /// Sequence number.
    pub sequence: usize,
}

/// IGES file data.
#[derive(Debug, Clone, Default)]
pub struct IgesData {
    /// Entities from directory section.
    pub entities: Vec<IgesEntity>,
    /// Points extracted.
    pub points: Vec<[f64; 3]>,
    /// Lines extracted (start, end pairs).
    pub lines: Vec<([f64; 3], [f64; 3])>,
}

impl IgesData {
    /// Create new empty IGES data.
    pub fn new() -> Self {
        Self::default()
    }

    /// Convert to mesh data.
    pub fn to_mesh(&self) -> MeshData {
        let mut positions = Vec::new();
        let mut faces = Vec::new();

        for point in &self.points {
            positions.push(Position::new(point[0], point[1], point[2]));
        }

        for (start, end) in &self.lines {
            let base = positions.len();
            positions.push(Position::new(start[0], start[1], start[2]));
            positions.push(Position::new(end[0], end[1], end[2]));
            faces.push(vec![base, base + 1]);
        }

        MeshData {
            name: "IGES_Mesh".to_string(),
            positions,
            normals: Vec::new(),
            uvs: Vec::new(),
            faces,
            material_indices: Vec::new(),
        }
    }
}

/// IGES importer.
pub struct IgesImporter;

impl Default for IgesImporter {
    fn default() -> Self {
        Self::new()
    }
}

impl IgesImporter {
    /// Create new importer.
    pub fn new() -> Self {
        Self
    }

    /// Import IGES file.
    pub fn import_file<P: AsRef<Path>>(&self, path: P) -> IgesResult<IgesData> {
        let file = std::fs::File::open(path)?;
        self.import_reader(BufReader::new(file))
    }

    /// Import from reader.
    pub fn import_reader<R: Read>(&self, reader: BufReader<R>) -> IgesResult<IgesData> {
        let mut data = IgesData::new();
        let lines: Vec<String> = reader.lines().collect::<Result<_, _>>()?;

        // IGES file has 5 sections: Start, Global, Directory, Parameter, Terminate
        // Each line is 80 characters with section letter in column 73

        let mut directory_lines = Vec::new();
        let mut parameter_lines = Vec::new();

        for line in &lines {
            if line.len() < 73 {
                continue;
            }

            let section = line.chars().nth(72).unwrap_or(' ');
            match section {
                'D' => directory_lines.push(line.clone()),
                'P' => parameter_lines.push(line.clone()),
                _ => {}
            }
        }

        // Parse directory section (2 lines per entity)
        for chunk in directory_lines.chunks(2) {
            if chunk.len() == 2 {
                if let Some(entity) = self.parse_directory_entry(&chunk[0], &chunk[1])? {
                    data.entities.push(entity);
                }
            }
        }

        // Parse parameter data (simplified - would need full implementation)
        // For now, we just extract entity types

        Ok(data)
    }

    fn parse_directory_entry(&self, line1: &str, _line2: &str) -> IgesResult<Option<IgesEntity>> {
        // Directory entry format (fixed columns):
        // Cols 1-8: Entity type number
        // Cols 9-16: Parameter data pointer
        // Cols 17-24: Structure
        // ... and more fields

        let entity_type_str = line1
            .get(0..8)
            .ok_or_else(|| IgesError::Parse("Short line".to_string()))?;
        let entity_type_num: i32 = entity_type_str
            .trim()
            .parse()
            .map_err(|e| IgesError::Parse(format!("Invalid entity type: {}", e)))?;

        let param_pointer_str = line1
            .get(8..16)
            .ok_or_else(|| IgesError::Parse("Short line".to_string()))?;
        let parameter_pointer: usize = param_pointer_str.trim().parse().unwrap_or(0);

        let entity = IgesEntity {
            entity_type: IgesEntityType::from(entity_type_num),
            parameter_pointer,
            structure: 0,
            line_font: 0,
            level: 0,
            view: 0,
            transformation: 0,
            label: 0,
            status: [0; 4],
            sequence: 0,
        };

        Ok(Some(entity))
    }
}

/// Import IGES file.
pub fn import_iges<P: AsRef<Path>>(path: P) -> IgesResult<IgesData> {
    IgesImporter::new().import_file(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_type_conversion() {
        assert_eq!(IgesEntityType::from(110), IgesEntityType::Line);
        assert_eq!(IgesEntityType::from(116), IgesEntityType::Point);
        assert_eq!(IgesEntityType::from(999), IgesEntityType::Unknown);
    }

    #[test]
    fn test_directory_parsing() {
        let importer = IgesImporter::new();
        let line1 = "     110       1       0       0       0       0       000000001D      1";
        let line2 = "     110       0       0       1       0                        0D      2";

        let entity = importer
            .parse_directory_entry(line1, line2)
            .unwrap()
            .unwrap();
        assert_eq!(entity.entity_type, IgesEntityType::Line);
        assert_eq!(entity.parameter_pointer, 1);
    }
}
