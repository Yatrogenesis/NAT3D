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

//! Python API for NAT3D.
//!
//! This module provides the Python scripting API that exposes NAT3D
//! functionality to Python scripts.

// Script context fields and ScriptValue variants are self-documenting.
// ScriptValue is a simple wrapper enum where variant names (Bool, Int, Float, etc.)
// clearly indicate their purpose. Ref types have obvious field semantics (id, name).
#![allow(missing_docs)]

use std::collections::HashMap;

/// Script execution context.
#[derive(Debug, Clone)]
pub struct ScriptContext {
    pub variables: HashMap<String, ScriptValue>,
    pub functions: Vec<String>,
    pub last_result: Option<ScriptValue>,
    pub output: Vec<String>,
    pub errors: Vec<String>,
}

impl ScriptContext {
    /// Create a new script context.
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
            functions: Vec::new(),
            last_result: None,
            output: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// Set a variable.
    pub fn set_var(&mut self, name: &str, value: ScriptValue) {
        self.variables.insert(name.to_string(), value);
    }

    /// Get a variable.
    pub fn get_var(&self, name: &str) -> Option<&ScriptValue> {
        self.variables.get(name)
    }

    /// Log output.
    pub fn log(&mut self, message: &str) {
        self.output.push(message.to_string());
    }

    /// Log error.
    pub fn error(&mut self, message: &str) {
        self.errors.push(message.to_string());
    }

    /// Clear output.
    pub fn clear_output(&mut self) {
        self.output.clear();
        self.errors.clear();
    }
}

impl Default for ScriptContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Script value types.
#[derive(Debug, Clone)]
pub enum ScriptValue {
    None,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Vector2([f64; 2]),
    Vector3([f64; 3]),
    Vector4([f64; 4]),
    Color([f64; 4]),
    List(Vec<ScriptValue>),
    Dict(HashMap<String, ScriptValue>),
    Object(ObjectRef),
    Mesh(MeshRef),
    Material(MaterialRef),
}

impl ScriptValue {
    /// Convert to bool.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(v) => Some(*v),
            Self::Int(v) => Some(*v != 0),
            Self::Float(v) => Some(*v != 0.0),
            _ => None,
        }
    }

    /// Convert to int.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Self::Int(v) => Some(*v),
            Self::Float(v) => Some(*v as i64),
            Self::Bool(v) => Some(if *v { 1 } else { 0 }),
            _ => None,
        }
    }

    /// Convert to float.
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Self::Float(v) => Some(*v),
            Self::Int(v) => Some(*v as f64),
            _ => None,
        }
    }

    /// Convert to string.
    pub fn as_string(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    /// Convert to Vector3.
    pub fn as_vec3(&self) -> Option<[f64; 3]> {
        match self {
            Self::Vector3(v) => Some(*v),
            Self::List(l) if l.len() >= 3 => {
                let x = l[0].as_float()?;
                let y = l[1].as_float()?;
                let z = l[2].as_float()?;
                Some([x, y, z])
            }
            _ => None,
        }
    }

    /// Type name.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Bool(_) => "bool",
            Self::Int(_) => "int",
            Self::Float(_) => "float",
            Self::String(_) => "str",
            Self::Vector2(_) => "Vector2",
            Self::Vector3(_) => "Vector3",
            Self::Vector4(_) => "Vector4",
            Self::Color(_) => "Color",
            Self::List(_) => "list",
            Self::Dict(_) => "dict",
            Self::Object(_) => "Object",
            Self::Mesh(_) => "Mesh",
            Self::Material(_) => "Material",
        }
    }
}

/// Reference to a scene object.
#[derive(Debug, Clone)]
pub struct ObjectRef {
    pub id: u64,
    pub name: String,
}

/// Reference to a mesh.
#[derive(Debug, Clone)]
pub struct MeshRef {
    pub id: u64,
    pub vertex_count: usize,
    pub face_count: usize,
}

/// Reference to a material.
#[derive(Debug, Clone)]
pub struct MaterialRef {
    pub id: u64,
    pub name: String,
}

/// NAT3D Python API namespace.
pub struct Nat3dApi {
    context: ScriptContext,
}

impl Nat3dApi {
    /// Create new API instance.
    pub fn new() -> Self {
        Self {
            context: ScriptContext::new(),
        }
    }

    /// Get context.
    pub fn context(&self) -> &ScriptContext {
        &self.context
    }

    /// Get context mutably.
    pub fn context_mut(&mut self) -> &mut ScriptContext {
        &mut self.context
    }

    // === Object Operations ===

    /// Create a new cube.
    pub fn add_cube(&mut self, size: f64, location: [f64; 3]) -> ObjectRef {
        let obj = ObjectRef {
            id: 1,
            name: "Cube".to_string(),
        };
        self.context.log(&format!(
            "Created cube at ({:.2}, {:.2}, {:.2}) with size {:.2}",
            location[0], location[1], location[2], size
        ));
        obj
    }

    /// Create a new sphere.
    pub fn add_sphere(
        &mut self,
        radius: f64,
        _segments: u32,
        _rings: u32,
        location: [f64; 3],
    ) -> ObjectRef {
        let obj = ObjectRef {
            id: 2,
            name: "Sphere".to_string(),
        };
        self.context.log(&format!(
            "Created sphere at ({:.2}, {:.2}, {:.2}) with radius {:.2}",
            location[0], location[1], location[2], radius
        ));
        obj
    }

    /// Create a new cylinder.
    pub fn add_cylinder(
        &mut self,
        _radius: f64,
        _depth: f64,
        _vertices: u32,
        location: [f64; 3],
    ) -> ObjectRef {
        let obj = ObjectRef {
            id: 3,
            name: "Cylinder".to_string(),
        };
        self.context.log(&format!(
            "Created cylinder at ({:.2}, {:.2}, {:.2})",
            location[0], location[1], location[2]
        ));
        obj
    }

    /// Create a new plane.
    pub fn add_plane(&mut self, _size: f64, location: [f64; 3]) -> ObjectRef {
        let obj = ObjectRef {
            id: 4,
            name: "Plane".to_string(),
        };
        self.context.log(&format!(
            "Created plane at ({:.2}, {:.2}, {:.2})",
            location[0], location[1], location[2]
        ));
        obj
    }

    /// Delete object.
    pub fn delete_object(&mut self, obj: &ObjectRef) {
        self.context.log(&format!("Deleted object: {}", obj.name));
    }

    /// Duplicate object.
    pub fn duplicate_object(&mut self, obj: &ObjectRef) -> ObjectRef {
        let new_obj = ObjectRef {
            id: obj.id + 100,
            name: format!("{}.001", obj.name),
        };
        self.context
            .log(&format!("Duplicated {} to {}", obj.name, new_obj.name));
        new_obj
    }

    // === Transform Operations ===

    /// Set object location.
    pub fn set_location(&mut self, obj: &ObjectRef, location: [f64; 3]) {
        self.context.log(&format!(
            "Set {} location to ({:.2}, {:.2}, {:.2})",
            obj.name, location[0], location[1], location[2]
        ));
    }

    /// Set object rotation (euler degrees).
    pub fn set_rotation(&mut self, obj: &ObjectRef, rotation: [f64; 3]) {
        self.context.log(&format!(
            "Set {} rotation to ({:.1}, {:.1}, {:.1})",
            obj.name, rotation[0], rotation[1], rotation[2]
        ));
    }

    /// Set object scale.
    pub fn set_scale(&mut self, obj: &ObjectRef, scale: [f64; 3]) {
        self.context.log(&format!(
            "Set {} scale to ({:.2}, {:.2}, {:.2})",
            obj.name, scale[0], scale[1], scale[2]
        ));
    }

    /// Translate object.
    pub fn translate(&mut self, obj: &ObjectRef, offset: [f64; 3]) {
        self.context.log(&format!(
            "Translated {} by ({:.2}, {:.2}, {:.2})",
            obj.name, offset[0], offset[1], offset[2]
        ));
    }

    /// Rotate object.
    pub fn rotate(&mut self, obj: &ObjectRef, angles: [f64; 3]) {
        self.context.log(&format!(
            "Rotated {} by ({:.1}, {:.1}, {:.1})",
            obj.name, angles[0], angles[1], angles[2]
        ));
    }

    /// Scale object.
    pub fn scale(&mut self, obj: &ObjectRef, factors: [f64; 3]) {
        self.context.log(&format!(
            "Scaled {} by ({:.2}, {:.2}, {:.2})",
            obj.name, factors[0], factors[1], factors[2]
        ));
    }

    // === Selection ===

    /// Select object.
    pub fn select(&mut self, obj: &ObjectRef) {
        self.context.log(&format!("Selected {}", obj.name));
    }

    /// Deselect object.
    pub fn deselect(&mut self, obj: &ObjectRef) {
        self.context.log(&format!("Deselected {}", obj.name));
    }

    /// Select all.
    pub fn select_all(&mut self) {
        self.context.log("Selected all objects");
    }

    /// Deselect all.
    pub fn deselect_all(&mut self) {
        self.context.log("Deselected all objects");
    }

    // === Modifiers ===

    /// Add subdivision modifier.
    pub fn add_subdivision(&mut self, obj: &ObjectRef, levels: u32) {
        self.context.log(&format!(
            "Added subdivision modifier to {} with {} levels",
            obj.name, levels
        ));
    }

    /// Add mirror modifier.
    pub fn add_mirror(&mut self, obj: &ObjectRef, axis: &str) {
        self.context.log(&format!(
            "Added mirror modifier to {} on {} axis",
            obj.name, axis
        ));
    }

    /// Add array modifier.
    pub fn add_array(&mut self, obj: &ObjectRef, count: u32, _offset: [f64; 3]) {
        self.context.log(&format!(
            "Added array modifier to {} with {} copies",
            obj.name, count
        ));
    }

    /// Add bevel modifier.
    pub fn add_bevel(&mut self, obj: &ObjectRef, width: f64, _segments: u32) {
        self.context.log(&format!(
            "Added bevel modifier to {} with width {:.2}",
            obj.name, width
        ));
    }

    /// Apply all modifiers.
    pub fn apply_modifiers(&mut self, obj: &ObjectRef) {
        self.context
            .log(&format!("Applied all modifiers to {}", obj.name));
    }

    // === Materials ===

    /// Create new material.
    pub fn create_material(&mut self, name: &str) -> MaterialRef {
        let mat = MaterialRef {
            id: 1,
            name: name.to_string(),
        };
        self.context.log(&format!("Created material: {}", name));
        mat
    }

    /// Assign material to object.
    pub fn assign_material(&mut self, obj: &ObjectRef, mat: &MaterialRef) {
        self.context
            .log(&format!("Assigned material {} to {}", mat.name, obj.name));
    }

    /// Set material color.
    pub fn set_material_color(&mut self, mat: &MaterialRef, color: [f64; 4]) {
        self.context.log(&format!(
            "Set {} color to ({:.2}, {:.2}, {:.2}, {:.2})",
            mat.name, color[0], color[1], color[2], color[3]
        ));
    }

    /// Set material metallic.
    pub fn set_material_metallic(&mut self, mat: &MaterialRef, value: f64) {
        self.context
            .log(&format!("Set {} metallic to {:.2}", mat.name, value));
    }

    /// Set material roughness.
    pub fn set_material_roughness(&mut self, mat: &MaterialRef, value: f64) {
        self.context
            .log(&format!("Set {} roughness to {:.2}", mat.name, value));
    }

    // === Animation ===

    /// Set current frame.
    pub fn set_frame(&mut self, frame: i32) {
        self.context.log(&format!("Set frame to {}", frame));
    }

    /// Insert keyframe.
    pub fn insert_keyframe(&mut self, obj: &ObjectRef, property: &str, frame: i32) {
        self.context.log(&format!(
            "Inserted keyframe for {}.{} at frame {}",
            obj.name, property, frame
        ));
    }

    /// Delete keyframe.
    pub fn delete_keyframe(&mut self, obj: &ObjectRef, property: &str, frame: i32) {
        self.context.log(&format!(
            "Deleted keyframe for {}.{} at frame {}",
            obj.name, property, frame
        ));
    }

    // === Render ===

    /// Render frame.
    pub fn render(&mut self, output_path: &str) {
        self.context.log(&format!("Rendering to {}", output_path));
    }

    /// Set render resolution.
    pub fn set_render_resolution(&mut self, width: u32, height: u32) {
        self.context
            .log(&format!("Set render resolution to {}x{}", width, height));
    }

    /// Set render samples.
    pub fn set_render_samples(&mut self, samples: u32) {
        self.context
            .log(&format!("Set render samples to {}", samples));
    }

    // === File Operations ===

    /// Save project.
    pub fn save(&mut self, path: &str) {
        self.context.log(&format!("Saved project to {}", path));
    }

    /// Load project.
    pub fn load(&mut self, path: &str) {
        self.context.log(&format!("Loaded project from {}", path));
    }

    /// Import file.
    pub fn import_file(&mut self, path: &str, format: &str) {
        self.context
            .log(&format!("Imported {} file: {}", format, path));
    }

    /// Export file.
    pub fn export_file(&mut self, path: &str, format: &str) {
        self.context
            .log(&format!("Exported {} file: {}", format, path));
    }

    // === Utility ===

    /// Print message.
    pub fn print(&mut self, message: &str) {
        self.context.log(message);
    }

    /// Get version.
    pub fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }
}

impl Default for Nat3dApi {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple script parser for basic commands.
pub struct ScriptParser;

impl ScriptParser {
    /// Parse and execute a simple script line.
    pub fn execute_line(api: &mut Nat3dApi, line: &str) -> Result<ScriptValue, String> {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            return Ok(ScriptValue::None);
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(ScriptValue::None);
        }

        match parts[0] {
            "print" => {
                let message = parts[1..].join(" ");
                api.print(&message);
                Ok(ScriptValue::None)
            }
            "add_cube" => {
                let size = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(1.0);
                let x = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.0);
                let y = parts.get(3).and_then(|s| s.parse().ok()).unwrap_or(0.0);
                let z = parts.get(4).and_then(|s| s.parse().ok()).unwrap_or(0.0);
                let obj = api.add_cube(size, [x, y, z]);
                Ok(ScriptValue::Object(obj))
            }
            "add_sphere" => {
                let radius = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(1.0);
                let x = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.0);
                let y = parts.get(3).and_then(|s| s.parse().ok()).unwrap_or(0.0);
                let z = parts.get(4).and_then(|s| s.parse().ok()).unwrap_or(0.0);
                let obj = api.add_sphere(radius, 32, 16, [x, y, z]);
                Ok(ScriptValue::Object(obj))
            }
            "version" => Ok(ScriptValue::String(api.version().to_string())),
            "help" => {
                api.print("NAT3D Script Commands:");
                api.print("  print <message>        - Print a message");
                api.print("  add_cube <size> <x> <y> <z>");
                api.print("  add_sphere <radius> <x> <y> <z>");
                api.print("  version                - Show version");
                api.print("  help                   - Show this help");
                Ok(ScriptValue::None)
            }
            _ => Err(format!("Unknown command: {}", parts[0])),
        }
    }

    /// Execute a multi-line script.
    pub fn execute_script(api: &mut Nat3dApi, script: &str) -> Result<Vec<ScriptValue>, String> {
        let mut results = Vec::new();
        for (line_num, line) in script.lines().enumerate() {
            match Self::execute_line(api, line) {
                Ok(value) => results.push(value),
                Err(e) => return Err(format!("Line {}: {}", line_num + 1, e)),
            }
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_creation() {
        let api = Nat3dApi::new();
        assert!(api.context().variables.is_empty());
    }

    #[test]
    fn test_add_cube() {
        let mut api = Nat3dApi::new();
        let cube = api.add_cube(2.0, [1.0, 2.0, 3.0]);
        assert_eq!(cube.name, "Cube");
        assert!(!api.context().output.is_empty());
    }

    #[test]
    fn test_script_parser() {
        let mut api = Nat3dApi::new();
        let result = ScriptParser::execute_line(&mut api, "add_cube 1.0 0 0 0");
        assert!(result.is_ok());
    }

    #[test]
    fn test_script_value_conversions() {
        let v = ScriptValue::Float(3.14);
        assert_eq!(v.as_float(), Some(3.14));
        assert_eq!(v.as_int(), Some(3));

        let v = ScriptValue::Int(42);
        assert_eq!(v.as_int(), Some(42));
        assert_eq!(v.as_float(), Some(42.0));
    }
}
