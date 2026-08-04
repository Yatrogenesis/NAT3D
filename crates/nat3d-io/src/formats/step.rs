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

//! STEP/STP (ISO 10303) format import.
//!
//! STEP is a complex CAD exchange format using EXPRESS schema.
//! This implementation provides basic entity parsing.

use nat3d_core::geometry::mesh::MeshData;
use nat3d_core::Position;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use thiserror::Error;

/// STEP format errors.
#[derive(Error, Debug)]
pub enum StepError {
    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// Parse error.
    #[error("Parse error: {0}")]
    Parse(String),
}

/// Result type for STEP operations.
pub type StepResult<T> = Result<T, StepError>;

/// STEP entity.
#[derive(Debug, Clone)]
pub struct StepEntity {
    /// Entity ID (e.g., #123).
    pub id: usize,
    /// Entity type (e.g., CARTESIAN_POINT).
    pub entity_type: String,
    /// Parameters.
    pub parameters: Vec<String>,
}

/// STEP file data.
#[derive(Debug, Clone, Default)]
pub struct StepData {
    /// Entities indexed by ID.
    pub entities: HashMap<usize, StepEntity>,
    /// Points extracted.
    pub points: Vec<[f64; 3]>,
}

impl StepData {
    /// Create new empty STEP data.
    pub fn new() -> Self {
        Self::default()
    }

    /// Convert to mesh data (simplified).
    pub fn to_mesh(&self) -> MeshData {
        let mut positions = Vec::new();

        for point in &self.points {
            positions.push(Position::new(point[0], point[1], point[2]));
        }

        MeshData {
            name: "STEP_Mesh".to_string(),
            positions,
            normals: Vec::new(),
            uvs: Vec::new(),
            faces: Vec::new(),
            material_indices: Vec::new(),
        }
    }
}

/// STEP importer.
pub struct StepImporter;

impl Default for StepImporter {
    fn default() -> Self {
        Self::new()
    }
}

impl StepImporter {
    /// Create new importer.
    pub fn new() -> Self {
        Self
    }

    /// Import STEP file.
    pub fn import_file<P: AsRef<Path>>(&self, path: P) -> StepResult<StepData> {
        let file = std::fs::File::open(path)?;
        self.import_reader(BufReader::new(file))
    }

    /// Import from reader.
    pub fn import_reader<R: Read>(&self, reader: BufReader<R>) -> StepResult<StepData> {
        let mut data = StepData::new();
        let mut in_data_section = false;
        let mut current_line = String::new();

        for line in reader.lines() {
            let line = line?;
            let trimmed = line.trim();

            if trimmed == "DATA;" {
                in_data_section = true;
                continue;
            }

            if trimmed == "ENDSEC;" {
                in_data_section = false;
                continue;
            }

            if !in_data_section {
                continue;
            }

            // STEP entities can span multiple lines
            current_line.push_str(trimmed);

            if trimmed.ends_with(';') {
                // Complete entity
                if let Some(entity) = self.parse_entity(&current_line)? {
                    // Extract points
                    if entity.entity_type == "CARTESIAN_POINT" {
                        if let Some(point) = self.extract_point(&entity) {
                            data.points.push(point);
                        }
                    }
                    data.entities.insert(entity.id, entity);
                }
                current_line.clear();
            }
        }

        Ok(data)
    }

    fn parse_entity(&self, line: &str) -> StepResult<Option<StepEntity>> {
        // Format: #ID = TYPE(params);
        if !line.starts_with('#') {
            return Ok(None);
        }

        let parts: Vec<&str> = line.splitn(2, '=').collect();
        if parts.len() != 2 {
            return Ok(None);
        }

        let id_str = parts[0].trim_start_matches('#').trim();
        let id = id_str
            .parse::<usize>()
            .map_err(|e| StepError::Parse(format!("Invalid entity ID: {}", e)))?;

        let rest = parts[1].trim();
        let type_end = rest.find('(').unwrap_or(rest.len());
        let entity_type = rest[..type_end].trim().to_string();

        // Extract parameters
        let mut parameters = Vec::new();
        if let Some(start) = rest.find('(') {
            if let Some(end) = rest.rfind(')') {
                let param_str = &rest[start + 1..end];
                parameters = self.parse_parameters(param_str);
            }
        }

        Ok(Some(StepEntity {
            id,
            entity_type,
            parameters,
        }))
    }

    fn parse_parameters(&self, param_str: &str) -> Vec<String> {
        let mut params = Vec::new();
        let mut current = String::new();
        let mut depth = 0;
        let mut in_string = false;

        for ch in param_str.chars() {
            match ch {
                '(' if !in_string => {
                    depth += 1;
                    current.push(ch);
                }
                ')' if !in_string => {
                    depth -= 1;
                    current.push(ch);
                }
                '\'' => {
                    in_string = !in_string;
                    current.push(ch);
                }
                ',' if depth == 0 && !in_string => {
                    params.push(current.trim().to_string());
                    current.clear();
                }
                _ => {
                    current.push(ch);
                }
            }
        }

        if !current.is_empty() {
            params.push(current.trim().to_string());
        }

        params
    }

    fn extract_point(&self, entity: &StepEntity) -> Option<[f64; 3]> {
        // CARTESIAN_POINT('name', (x, y, z))
        if entity.parameters.len() < 2 {
            return None;
        }

        let coord_str = &entity.parameters[1];
        let coords = coord_str.trim_start_matches('(').trim_end_matches(')');
        let values: Vec<f64> = coords
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();

        if values.len() >= 3 {
            Some([values[0], values[1], values[2]])
        } else {
            None
        }
    }
}

/// Import STEP file.
pub fn import_step<P: AsRef<Path>>(path: P) -> StepResult<StepData> {
    StepImporter::new().import_file(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_parsing() {
        let importer = StepImporter::new();
        let line = "#123 = CARTESIAN_POINT('origin', (0.0, 0.0, 0.0));";
        let entity = importer.parse_entity(line).unwrap().unwrap();

        assert_eq!(entity.id, 123);
        assert_eq!(entity.entity_type, "CARTESIAN_POINT");
        assert_eq!(entity.parameters.len(), 2);
    }

    #[test]
    fn test_point_extraction() {
        let importer = StepImporter::new();
        let entity = StepEntity {
            id: 1,
            entity_type: "CARTESIAN_POINT".to_string(),
            parameters: vec!["'p1'".to_string(), "(1.0, 2.0, 3.0)".to_string()],
        };

        let point = importer.extract_point(&entity).unwrap();
        assert_eq!(point, [1.0, 2.0, 3.0]);
    }
}
