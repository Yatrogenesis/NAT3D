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

//! Shape keys / morph targets / blend shapes.
//!
//! Vertex-level animation for facial expressions, corrective shapes, etc.

use nalgebra::Vector3;
use std::collections::HashMap;

use crate::rigging::bone::BoneId;

/// A single shape key.
#[derive(Debug, Clone)]
pub struct ShapeKey {
    /// Shape key name.
    pub name: String,
    /// Vertex offsets (delta from reference).
    pub vertices: Vec<Vector3<f64>>,
    /// Current value (0.0 to 1.0, or beyond for overshoot).
    pub value: f64,
    /// Minimum value.
    pub min_value: f64,
    /// Maximum value.
    pub max_value: f64,
    /// Is muted.
    pub mute: bool,
    /// Is relative to reference (true) or absolute (false).
    pub relative: bool,
}

impl ShapeKey {
    /// Create a new shape key.
    pub fn new(name: impl Into<String>, vertex_count: usize) -> Self {
        Self {
            name: name.into(),
            vertices: vec![Vector3::zeros(); vertex_count],
            value: 0.0,
            min_value: 0.0,
            max_value: 1.0,
            mute: false,
            relative: true,
        }
    }

    /// Set vertex offset.
    pub fn set_vertex(&mut self, index: usize, offset: Vector3<f64>) {
        if index < self.vertices.len() {
            self.vertices[index] = offset;
        }
    }

    /// Get effective value (considering mute).
    pub fn effective_value(&self) -> f64 {
        if self.mute {
            0.0
        } else {
            self.value.clamp(self.min_value, self.max_value)
        }
    }
}

/// Shape key block (collection of shape keys).
#[derive(Debug, Clone)]
pub struct ShapeKeyBlock {
    /// Reference key (basis shape).
    pub reference_key: Vec<Vector3<f64>>,
    /// All shape keys.
    shape_keys: Vec<ShapeKey>,
    /// Shape key name to index mapping.
    key_map: HashMap<String, usize>,
    /// Corrective shape keys (combination shapes).
    correctives: Vec<CorrectiveShape>,
}

impl ShapeKeyBlock {
    /// Create a new shape key block.
    pub fn new(reference_vertices: Vec<Vector3<f64>>) -> Self {
        Self {
            reference_key: reference_vertices,
            shape_keys: Vec::new(),
            key_map: HashMap::new(),
            correctives: Vec::new(),
        }
    }

    /// Add a shape key.
    pub fn add_key(&mut self, key: ShapeKey) {
        let index = self.shape_keys.len();
        self.key_map.insert(key.name.clone(), index);
        self.shape_keys.push(key);
    }

    /// Get shape key by name.
    pub fn get_key(&self, name: &str) -> Option<&ShapeKey> {
        self.key_map
            .get(name)
            .and_then(|&idx| self.shape_keys.get(idx))
    }

    /// Get mutable shape key by name.
    pub fn get_key_mut(&mut self, name: &str) -> Option<&mut ShapeKey> {
        if let Some(&idx) = self.key_map.get(name) {
            self.shape_keys.get_mut(idx)
        } else {
            None
        }
    }

    /// Set shape key value.
    pub fn set_value(&mut self, name: &str, value: f64) {
        if let Some(key) = self.get_key_mut(name) {
            key.value = value;
        }
    }

    /// Get shape key value.
    pub fn get_value(&self, name: &str) -> f64 {
        self.get_key(name)
            .map(|k| k.effective_value())
            .unwrap_or(0.0)
    }

    /// Add a corrective shape key.
    pub fn add_corrective(&mut self, corrective: CorrectiveShape) {
        self.correctives.push(corrective);
    }

    /// Evaluate all shape keys and return final vertex positions.
    pub fn evaluate(&self) -> Vec<Vector3<f64>> {
        let vertex_count = self.reference_key.len();
        let mut result = self.reference_key.clone();

        // Apply base shape keys
        for key in &self.shape_keys {
            if key.mute {
                continue;
            }

            let value = key.effective_value();
            if value.abs() < 1e-6 {
                continue;
            }

            if key.relative {
                // Relative mode: add weighted offset
                for (i, offset) in key.vertices.iter().enumerate() {
                    if i < vertex_count {
                        result[i] += offset * value;
                    }
                }
            } else {
                // Absolute mode: blend towards target
                for (i, target) in key.vertices.iter().enumerate() {
                    if i < vertex_count {
                        result[i] = result[i] + (target - result[i]) * value;
                    }
                }
            }
        }

        // Apply corrective shapes
        for corrective in &self.correctives {
            if let Some(weight) = corrective.compute_weight(&self.shape_keys, &self.key_map) {
                if weight > 1e-6 {
                    if let Some(key) = self.get_key(&corrective.shape_key) {
                        for (i, offset) in key.vertices.iter().enumerate() {
                            if i < vertex_count {
                                result[i] += offset * weight;
                            }
                        }
                    }
                }
            }
        }

        result
    }

    /// Blend between multiple shape key states.
    pub fn blend_shapes(&mut self, states: &[(&str, f64)]) {
        for &(name, value) in states {
            self.set_value(name, value);
        }
    }

    /// Get all shape key names.
    pub fn key_names(&self) -> Vec<&str> {
        self.shape_keys.iter().map(|k| k.name.as_str()).collect()
    }
}

/// Corrective shape key (activated by combination of other shapes).
#[derive(Debug, Clone)]
pub struct CorrectiveShape {
    /// Name of the corrective shape key.
    pub shape_key: String,
    /// Driving shape keys and their target values.
    pub drivers: Vec<(String, f64)>,
    /// Activation threshold.
    pub threshold: f64,
}

impl CorrectiveShape {
    /// Create a new corrective shape.
    pub fn new(shape_key: impl Into<String>) -> Self {
        Self {
            shape_key: shape_key.into(),
            drivers: Vec::new(),
            threshold: 0.8,
        }
    }

    /// Add a driver shape key.
    pub fn add_driver(&mut self, key_name: impl Into<String>, target_value: f64) {
        self.drivers.push((key_name.into(), target_value));
    }

    /// Compute activation weight based on driver states.
    pub fn compute_weight(
        &self,
        shape_keys: &[ShapeKey],
        key_map: &HashMap<String, usize>,
    ) -> Option<f64> {
        if self.drivers.is_empty() {
            return None;
        }

        let mut total_match = 0.0;
        let mut count = 0;

        for (driver_name, target_value) in &self.drivers {
            if let Some(&idx) = key_map.get(driver_name) {
                if let Some(key) = shape_keys.get(idx) {
                    let current_value = key.effective_value();
                    let diff = (current_value - target_value).abs();
                    let match_score = (1.0 - diff).max(0.0);
                    total_match += match_score;
                    count += 1;
                }
            }
        }

        if count > 0 {
            let avg_match = total_match / count as f64;
            if avg_match >= self.threshold {
                Some(avg_match)
            } else {
                Some(0.0)
            }
        } else {
            None
        }
    }
}

/// Shape key driver (expression-based automation).
#[derive(Debug, Clone)]
pub struct ShapeKeyDriver {
    /// Target shape key name.
    pub target_key: String,
    /// Driver type.
    pub driver_type: DriverType,
    /// Expression.
    pub expression: String,
    /// Variables.
    pub variables: HashMap<String, DriverVariable>,
}

/// Driver variable source.
#[derive(Debug, Clone)]
pub enum DriverVariable {
    /// Bone transform channel.
    BoneTransform {
        bone_id: BoneId,
        channel: TransformChannel,
    },
    /// Another shape key value.
    ShapeKey(String),
    /// Custom value.
    Custom(f64),
}

/// Transform channel for drivers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformChannel {
    LocX,
    LocY,
    LocZ,
    RotX,
    RotY,
    RotZ,
    ScaleX,
    ScaleY,
    ScaleZ,
}

/// Driver type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverType {
    /// Average of variables.
    Average,
    /// Sum of variables.
    Sum,
    /// Scripted expression.
    Scripted,
    /// Minimum value.
    Min,
    /// Maximum value.
    Max,
}

impl ShapeKeyDriver {
    /// Create a new shape key driver.
    pub fn new(target_key: impl Into<String>) -> Self {
        Self {
            target_key: target_key.into(),
            driver_type: DriverType::Average,
            expression: String::new(),
            variables: HashMap::new(),
        }
    }

    /// Add a variable.
    pub fn add_variable(&mut self, name: impl Into<String>, variable: DriverVariable) {
        self.variables.insert(name.into(), variable);
    }

    /// Evaluate the driver (simplified - real implementation would parse expression).
    pub fn evaluate(&self, _context: &DriverContext) -> f64 {
        // Simplified evaluation
        match self.driver_type {
            DriverType::Average => {
                if self.variables.is_empty() {
                    return 0.0;
                }
                // Would evaluate each variable and average
                0.0
            }
            DriverType::Sum => {
                // Would sum all variable values
                0.0
            }
            DriverType::Min | DriverType::Max => {
                // Would find min/max of variables
                0.0
            }
            DriverType::Scripted => {
                // Would parse and evaluate expression
                0.0
            }
        }
    }
}

/// Driver evaluation context.
#[derive(Debug, Clone)]
pub struct DriverContext {
    /// Shape key values.
    pub shape_values: HashMap<String, f64>,
    /// Custom values.
    pub custom_values: HashMap<String, f64>,
}

impl DriverContext {
    /// Create a new driver context.
    pub fn new() -> Self {
        Self {
            shape_values: HashMap::new(),
            custom_values: HashMap::new(),
        }
    }
}

impl Default for DriverContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shape_key_creation() {
        let key = ShapeKey::new("Smile", 100);
        assert_eq!(key.name, "Smile");
        assert_eq!(key.vertices.len(), 100);
        assert_eq!(key.value, 0.0);
    }

    #[test]
    fn test_shape_key_block() {
        let reference = vec![Vector3::new(0.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0)];

        let mut block = ShapeKeyBlock::new(reference);

        let mut smile = ShapeKey::new("Smile", 2);
        smile.set_vertex(0, Vector3::new(0.0, 0.1, 0.0));
        smile.set_vertex(1, Vector3::new(0.0, 0.1, 0.0));
        block.add_key(smile);

        block.set_value("Smile", 1.0);
        let result = block.evaluate();

        assert!((result[0].y - 0.1).abs() < 1e-6);
    }

    #[test]
    fn test_shape_blending() {
        let reference = vec![Vector3::zeros(); 10];
        let mut block = ShapeKeyBlock::new(reference);

        let mut key1 = ShapeKey::new("Key1", 10);
        key1.set_vertex(0, Vector3::new(1.0, 0.0, 0.0));
        block.add_key(key1);

        let mut key2 = ShapeKey::new("Key2", 10);
        key2.set_vertex(0, Vector3::new(0.0, 1.0, 0.0));
        block.add_key(key2);

        block.blend_shapes(&[("Key1", 0.5), ("Key2", 0.5)]);
        let result = block.evaluate();

        assert!((result[0].x - 0.5).abs() < 1e-6);
        assert!((result[0].y - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_corrective_shape() {
        let mut corrective = CorrectiveShape::new("EyebrowCorrect");
        corrective.add_driver("Smile", 1.0);
        corrective.add_driver("EyeOpen", 1.0);
        corrective.threshold = 0.8;

        assert_eq!(corrective.drivers.len(), 2);
    }
}
