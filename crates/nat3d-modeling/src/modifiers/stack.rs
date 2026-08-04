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

//! Modifier stack system.
//!
//! Implements a non-destructive modifier system where modifiers
//! are applied in sequence to generate the final mesh.

use nalgebra::{Point3, Vector3};
use std::any::Any;
use std::collections::HashMap;

/// Unique modifier ID.
pub type ModifierId = u64;

/// Trait for mesh modifiers.
pub trait Modifier: Send + Sync {
    /// Get modifier name.
    fn name(&self) -> &str;

    /// Get modifier type ID.
    fn type_id(&self) -> &'static str;

    /// Apply modifier to mesh.
    fn apply(&self, mesh: &ModifierMesh) -> ModifierMesh;

    /// Check if modifier is enabled.
    fn is_enabled(&self) -> bool;

    /// Enable/disable modifier.
    fn set_enabled(&mut self, enabled: bool);

    /// Clone as boxed trait object.
    fn clone_box(&self) -> Box<dyn Modifier>;

    /// Get as Any for downcasting.
    fn as_any(&self) -> &dyn Any;

    /// Get as mutable Any for downcasting.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl Clone for Box<dyn Modifier> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// Mesh data for modifiers.
#[derive(Debug, Clone)]
pub struct ModifierMesh {
    /// Vertex positions.
    pub positions: Vec<Point3<f64>>,
    /// Vertex normals.
    pub normals: Vec<Vector3<f64>>,
    /// Face indices (polygons).
    pub faces: Vec<Vec<usize>>,
    /// UV coordinates.
    pub uvs: Vec<(f64, f64)>,
    /// Vertex groups.
    pub vertex_groups: HashMap<String, Vec<(usize, f64)>>,
    /// Custom attributes.
    pub attributes: HashMap<String, Vec<f64>>,
}

impl Default for ModifierMesh {
    fn default() -> Self {
        Self::new()
    }
}

impl ModifierMesh {
    /// Create empty mesh.
    pub fn new() -> Self {
        Self {
            positions: Vec::new(),
            normals: Vec::new(),
            faces: Vec::new(),
            uvs: Vec::new(),
            vertex_groups: HashMap::new(),
            attributes: HashMap::new(),
        }
    }

    /// Create from positions and faces.
    pub fn from_geometry(positions: Vec<Point3<f64>>, faces: Vec<Vec<usize>>) -> Self {
        let mut mesh = Self {
            positions,
            normals: Vec::new(),
            faces,
            uvs: Vec::new(),
            vertex_groups: HashMap::new(),
            attributes: HashMap::new(),
        };
        mesh.compute_normals();
        mesh
    }

    /// Compute vertex normals.
    pub fn compute_normals(&mut self) {
        self.normals = vec![Vector3::zeros(); self.positions.len()];

        for face in &self.faces {
            if face.len() < 3 {
                continue;
            }

            // Calculate face normal
            let v0 = self.positions[face[0]];
            let v1 = self.positions[face[1]];
            let v2 = self.positions[face[2]];

            let face_normal = (v1 - v0).cross(&(v2 - v0));

            // Accumulate to vertex normals
            for &vi in face {
                if vi < self.normals.len() {
                    self.normals[vi] += face_normal;
                }
            }
        }

        // Normalize
        for n in &mut self.normals {
            let len = n.magnitude();
            if len > 1e-10 {
                *n /= len;
            } else {
                *n = Vector3::y();
            }
        }
    }

    /// Add vertex.
    pub fn add_vertex(&mut self, position: Point3<f64>) -> usize {
        let idx = self.positions.len();
        self.positions.push(position);
        self.normals.push(Vector3::y());
        idx
    }

    /// Add face.
    pub fn add_face(&mut self, vertices: Vec<usize>) {
        self.faces.push(vertices);
    }

    /// Get vertex count.
    pub fn vertex_count(&self) -> usize {
        self.positions.len()
    }

    /// Get face count.
    pub fn face_count(&self) -> usize {
        self.faces.len()
    }

    /// Clone mesh geometry.
    pub fn clone_geometry(&self) -> Self {
        Self {
            positions: self.positions.clone(),
            normals: self.normals.clone(),
            faces: self.faces.clone(),
            uvs: self.uvs.clone(),
            vertex_groups: self.vertex_groups.clone(),
            attributes: self.attributes.clone(),
        }
    }

    /// Merge with another mesh.
    pub fn merge(&mut self, other: &ModifierMesh) {
        let offset = self.positions.len();

        self.positions.extend(&other.positions);
        self.normals.extend(&other.normals);

        for face in &other.faces {
            let new_face: Vec<usize> = face.iter().map(|&i| i + offset).collect();
            self.faces.push(new_face);
        }
    }

    /// Triangulate all faces.
    pub fn triangulate(&mut self) {
        let mut new_faces = Vec::new();

        for face in &self.faces {
            if face.len() < 3 {
                continue;
            }

            // Fan triangulation
            for i in 1..face.len() - 1 {
                new_faces.push(vec![face[0], face[i], face[i + 1]]);
            }
        }

        self.faces = new_faces;
    }

    /// Calculate bounding box.
    pub fn bounds(&self) -> (Point3<f64>, Point3<f64>) {
        let mut min = Point3::new(f64::MAX, f64::MAX, f64::MAX);
        let mut max = Point3::new(f64::MIN, f64::MIN, f64::MIN);

        for p in &self.positions {
            min.x = min.x.min(p.x);
            min.y = min.y.min(p.y);
            min.z = min.z.min(p.z);
            max.x = max.x.max(p.x);
            max.y = max.y.max(p.y);
            max.z = max.z.max(p.z);
        }

        (min, max)
    }

    /// Create from nat3d-core Mesh.
    pub fn from_mesh(mesh: &nat3d_core::geometry::Mesh) -> Self {
        let mut m = Self::new();

        for v in &mesh.vertices {
            let p = v.data.position;
            m.add_vertex(Point3::new(p.x, p.y, p.z));
        }

        for face in &mesh.faces {
            m.add_face(face.vertices.to_vec());
        }

        m.compute_normals();
        m
    }

    /// Convert back to nat3d-core Mesh.
    pub fn to_mesh(&self, name: impl Into<String>) -> nat3d_core::geometry::Mesh {
        use nat3d_core::geometry::{Mesh, Normal, Position, VertexData};

        let mut mesh = Mesh::new(name);

        for (i, p) in self.positions.iter().enumerate() {
            let n = self.normals.get(i).copied().unwrap_or_else(Vector3::y);
            let _ = mesh.add_vertex(VertexData::with_normal(
                Position::new(p.x, p.y, p.z),
                Normal::new(n.x, n.y, n.z),
            ));
        }

        for face_indices in &self.faces {
            let _ = mesh.add_ngon(face_indices);
        }

        mesh
    }
}

/// Modifier stack.
#[derive(Clone)]
pub struct ModifierStack {
    /// Modifiers in order.
    modifiers: Vec<(ModifierId, Box<dyn Modifier>)>,
    /// Next modifier ID.
    next_id: ModifierId,
}

impl Default for ModifierStack {
    fn default() -> Self {
        Self::new()
    }
}

impl ModifierStack {
    /// Create new modifier stack.
    pub fn new() -> Self {
        Self {
            modifiers: Vec::new(),
            next_id: 1,
        }
    }

    /// Add modifier to stack.
    pub fn add(&mut self, modifier: Box<dyn Modifier>) -> ModifierId {
        let id = self.next_id;
        self.next_id += 1;
        self.modifiers.push((id, modifier));
        id
    }

    /// Insert modifier at index.
    pub fn insert(&mut self, index: usize, modifier: Box<dyn Modifier>) -> ModifierId {
        let id = self.next_id;
        self.next_id += 1;
        let idx = index.min(self.modifiers.len());
        self.modifiers.insert(idx, (id, modifier));
        id
    }

    /// Remove modifier by ID.
    pub fn remove(&mut self, id: ModifierId) -> Option<Box<dyn Modifier>> {
        if let Some(pos) = self.modifiers.iter().position(|(mid, _)| *mid == id) {
            Some(self.modifiers.remove(pos).1)
        } else {
            None
        }
    }

    /// Move modifier up in stack.
    pub fn move_up(&mut self, id: ModifierId) -> bool {
        if let Some(pos) = self.modifiers.iter().position(|(mid, _)| *mid == id) {
            if pos > 0 {
                self.modifiers.swap(pos, pos - 1);
                return true;
            }
        }
        false
    }

    /// Move modifier down in stack.
    pub fn move_down(&mut self, id: ModifierId) -> bool {
        if let Some(pos) = self.modifiers.iter().position(|(mid, _)| *mid == id) {
            if pos < self.modifiers.len() - 1 {
                self.modifiers.swap(pos, pos + 1);
                return true;
            }
        }
        false
    }

    /// Get modifier by ID.
    pub fn get(&self, id: ModifierId) -> Option<&dyn Modifier> {
        self.modifiers
            .iter()
            .find(|(mid, _)| *mid == id)
            .map(|(_, m)| m.as_ref())
    }

    /// Get mutable modifier by ID.
    pub fn get_mut(&mut self, id: ModifierId) -> Option<&mut Box<dyn Modifier>> {
        for (mid, modifier) in &mut self.modifiers {
            if *mid == id {
                return Some(modifier);
            }
        }
        None
    }

    /// Apply all modifiers to mesh.
    pub fn apply(&self, input: &ModifierMesh) -> ModifierMesh {
        let mut result = input.clone();

        for (_, modifier) in &self.modifiers {
            if modifier.is_enabled() {
                result = modifier.apply(&result);
            }
        }

        result
    }

    /// Apply modifiers up to (not including) specified ID.
    pub fn apply_until(&self, input: &ModifierMesh, until_id: ModifierId) -> ModifierMesh {
        let mut result = input.clone();

        for (id, modifier) in &self.modifiers {
            if *id == until_id {
                break;
            }
            if modifier.is_enabled() {
                result = modifier.apply(&result);
            }
        }

        result
    }

    /// Get number of modifiers.
    pub fn len(&self) -> usize {
        self.modifiers.len()
    }

    /// Check if stack is empty.
    pub fn is_empty(&self) -> bool {
        self.modifiers.is_empty()
    }

    /// Iterate over modifiers.
    pub fn iter(&self) -> impl Iterator<Item = (ModifierId, &dyn Modifier)> {
        self.modifiers.iter().map(|(id, m)| (*id, m.as_ref()))
    }

    /// Clear all modifiers.
    pub fn clear(&mut self) {
        self.modifiers.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Simple test modifier
    #[derive(Clone)]
    struct TestModifier {
        name: String,
        enabled: bool,
        scale: f64,
    }

    impl Modifier for TestModifier {
        fn name(&self) -> &str {
            &self.name
        }
        fn type_id(&self) -> &'static str {
            "TestModifier"
        }

        fn apply(&self, mesh: &ModifierMesh) -> ModifierMesh {
            let mut result = mesh.clone();
            for p in &mut result.positions {
                *p = Point3::new(p.x * self.scale, p.y * self.scale, p.z * self.scale);
            }
            result
        }

        fn is_enabled(&self) -> bool {
            self.enabled
        }
        fn set_enabled(&mut self, enabled: bool) {
            self.enabled = enabled;
        }
        fn clone_box(&self) -> Box<dyn Modifier> {
            Box::new(self.clone())
        }
        fn as_any(&self) -> &dyn Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }

    #[test]
    fn test_modifier_stack() {
        let mut stack = ModifierStack::new();

        let m1 = TestModifier {
            name: "Scale2x".into(),
            enabled: true,
            scale: 2.0,
        };

        let m2 = TestModifier {
            name: "Scale3x".into(),
            enabled: true,
            scale: 3.0,
        };

        let id1 = stack.add(Box::new(m1));
        let _id2 = stack.add(Box::new(m2));

        assert_eq!(stack.len(), 2);

        let mesh = ModifierMesh::from_geometry(vec![Point3::new(1.0, 0.0, 0.0)], vec![]);

        let result = stack.apply(&mesh);

        // 1.0 * 2.0 * 3.0 = 6.0
        assert!((result.positions[0].x - 6.0).abs() < 1e-10);

        // Remove first modifier
        stack.remove(id1);
        assert_eq!(stack.len(), 1);

        let result2 = stack.apply(&mesh);
        // 1.0 * 3.0 = 3.0
        assert!((result2.positions[0].x - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_modifier_mesh() {
        let mut mesh = ModifierMesh::new();

        mesh.add_vertex(Point3::new(0.0, 0.0, 0.0));
        mesh.add_vertex(Point3::new(1.0, 0.0, 0.0));
        mesh.add_vertex(Point3::new(1.0, 1.0, 0.0));
        mesh.add_vertex(Point3::new(0.0, 1.0, 0.0));

        mesh.add_face(vec![0, 1, 2, 3]);

        assert_eq!(mesh.vertex_count(), 4);
        assert_eq!(mesh.face_count(), 1);

        mesh.triangulate();
        assert_eq!(mesh.face_count(), 2);
    }
}
