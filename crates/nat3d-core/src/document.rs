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

//! Document structure for NAT3D.
//!
//! The Document is the root container for a NAT3D project,
//! containing all scene data, meshes, materials, and settings.

use crate::error::CoreResult;
use crate::geometry::{Mesh, MeshId};
use crate::hierarchy::{Object, ObjectId, SceneGraph};
use crate::history::History;
use crate::layer::{Layer, LayerId, LayerManager};
use crate::material::{Material, MaterialId};
use crate::transform::Transform;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

/// Document file format version.
pub const DOCUMENT_VERSION: u32 = 1;

/// Unique identifier for a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DocumentId(pub Uuid);

impl DocumentId {
    /// Create a new unique document ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for DocumentId {
    fn default() -> Self {
        Self::new()
    }
}

/// Document metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMetadata {
    /// Document title.
    pub title: String,
    /// Author name.
    pub author: Option<String>,
    /// Document description.
    pub description: Option<String>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last modification timestamp.
    pub modified_at: DateTime<Utc>,
    /// Software version that created this document.
    pub software_version: String,
    /// Custom tags for organization.
    pub tags: Vec<String>,
    /// Custom metadata key-value pairs.
    pub custom: HashMap<String, String>,
}

impl DocumentMetadata {
    /// Create new metadata with a title.
    #[must_use]
    pub fn new(title: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            title: title.into(),
            author: None,
            description: None,
            created_at: now,
            modified_at: now,
            software_version: env!("CARGO_PKG_VERSION").to_string(),
            tags: Vec::new(),
            custom: HashMap::new(),
        }
    }

    /// Update the modification timestamp.
    pub fn touch(&mut self) {
        self.modified_at = Utc::now();
    }
}

/// Document rendering settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderSettings {
    /// Output resolution width.
    pub width: u32,
    /// Output resolution height.
    pub height: u32,
    /// Samples per pixel for anti-aliasing.
    pub samples: u32,
    /// Maximum ray bounces.
    pub max_bounces: u32,
    /// Background color (RGB, 0-1).
    pub background_color: [f32; 3],
    /// Whether to use environment map.
    pub use_environment: bool,
    /// Environment map path.
    pub environment_path: Option<PathBuf>,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            samples: 64,
            max_bounces: 8,
            background_color: [0.05, 0.05, 0.05],
            use_environment: false,
            environment_path: None,
        }
    }
}

/// Document viewport settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewportSettings {
    /// Grid visibility.
    pub show_grid: bool,
    /// Grid size.
    pub grid_size: f64,
    /// Grid subdivisions.
    pub grid_subdivisions: u32,
    /// Show world axes.
    pub show_axes: bool,
    /// Show wireframe overlay.
    pub show_wireframe: bool,
    /// Show vertex normals.
    pub show_normals: bool,
    /// Show bounding boxes.
    pub show_bounds: bool,
    /// Ambient occlusion in viewport.
    pub viewport_ao: bool,
}

impl Default for ViewportSettings {
    fn default() -> Self {
        Self {
            show_grid: true,
            grid_size: 1.0,
            grid_subdivisions: 10,
            show_axes: true,
            show_wireframe: false,
            show_normals: false,
            show_bounds: false,
            viewport_ao: true,
        }
    }
}

/// Unit system for the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum UnitSystem {
    /// Metric (meters).
    #[default]
    Metric,
    /// Imperial (feet).
    Imperial,
    /// Architectural (feet and inches).
    Architectural,
}

/// Document settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSettings {
    /// Unit system.
    pub unit_system: UnitSystem,
    /// Scale factor (1 unit = scale meters).
    pub unit_scale: f64,
    /// Render settings.
    pub render: RenderSettings,
    /// Viewport settings.
    pub viewport: ViewportSettings,
}

impl Default for DocumentSettings {
    fn default() -> Self {
        Self {
            unit_system: UnitSystem::Metric,
            unit_scale: 1.0,
            render: RenderSettings::default(),
            viewport: ViewportSettings::default(),
        }
    }
}

/// A NAT3D document containing all project data.
#[derive(Debug)]
pub struct Document {
    /// Unique identifier.
    pub id: DocumentId,
    /// Document metadata.
    pub metadata: DocumentMetadata,
    /// Document settings.
    pub settings: DocumentSettings,
    /// Scene graph containing all objects.
    pub scene: SceneGraph,
    /// All meshes in the document.
    meshes: HashMap<MeshId, Mesh>,
    /// All materials in the document.
    materials: HashMap<MaterialId, Material>,
    /// Layer manager.
    pub layers: LayerManager,
    /// Edit history.
    #[allow(dead_code)]
    history: History,
    /// File path (if saved).
    pub file_path: Option<PathBuf>,
}

impl Document {
    /// Create a new empty document.
    #[must_use]
    pub fn new(title: impl Into<String>) -> Self {
        let mut doc = Self {
            id: DocumentId::new(),
            metadata: DocumentMetadata::new(title),
            settings: DocumentSettings::default(),
            scene: SceneGraph::new(),
            meshes: HashMap::new(),
            materials: HashMap::new(),
            layers: LayerManager::new(),
            history: History::new(),
            file_path: None,
        };

        // Add default material
        let default_material = Material::default();
        doc.materials.insert(default_material.id, default_material);

        doc
    }

    /// Create a document with a default cube.
    #[must_use]
    pub fn with_default_cube() -> Self {
        let mut doc = Self::new("Untitled");

        // Create cube mesh
        let cube = Mesh::cube(1.0);
        let mesh_id = cube.id;
        doc.meshes.insert(mesh_id, cube);

        // Create cube object
        let obj = Object::mesh("Cube", mesh_id);
        doc.scene.add_object(obj);

        doc
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Mesh Management
    // ══════════════════════════════════════════════════════════════════════════

    /// Add a mesh to the document.
    pub fn add_mesh(&mut self, mesh: Mesh) -> MeshId {
        let id = mesh.id;
        self.meshes.insert(id, mesh);
        self.metadata.touch();
        id
    }

    /// Get a mesh by ID.
    #[must_use]
    pub fn mesh(&self, id: MeshId) -> Option<&Mesh> {
        self.meshes.get(&id)
    }

    /// Get a mutable mesh by ID.
    pub fn mesh_mut(&mut self, id: MeshId) -> Option<&mut Mesh> {
        self.metadata.touch();
        self.meshes.get_mut(&id)
    }

    /// Remove a mesh from the document.
    pub fn remove_mesh(&mut self, id: MeshId) -> Option<Mesh> {
        self.metadata.touch();
        self.meshes.remove(&id)
    }

    /// Get all meshes.
    pub fn meshes(&self) -> impl Iterator<Item = &Mesh> {
        self.meshes.values()
    }

    /// Get mesh count.
    #[must_use]
    pub fn mesh_count(&self) -> usize {
        self.meshes.len()
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Material Management
    // ══════════════════════════════════════════════════════════════════════════

    /// Add a material to the document.
    pub fn add_material(&mut self, material: Material) -> MaterialId {
        let id = material.id;
        self.materials.insert(id, material);
        self.metadata.touch();
        id
    }

    /// Get a material by ID.
    #[must_use]
    pub fn material(&self, id: MaterialId) -> Option<&Material> {
        self.materials.get(&id)
    }

    /// Get a mutable material by ID.
    pub fn material_mut(&mut self, id: MaterialId) -> Option<&mut Material> {
        self.metadata.touch();
        self.materials.get_mut(&id)
    }

    /// Remove a material from the document.
    pub fn remove_material(&mut self, id: MaterialId) -> Option<Material> {
        self.metadata.touch();
        self.materials.remove(&id)
    }

    /// Get all materials.
    pub fn materials(&self) -> impl Iterator<Item = &Material> {
        self.materials.values()
    }

    /// Get material count.
    #[must_use]
    pub fn material_count(&self) -> usize {
        self.materials.len()
    }

    /// Get the default material.
    #[must_use]
    pub fn default_material(&self) -> Option<&Material> {
        self.materials.values().find(|m| m.name == "Default")
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Object Management
    // ══════════════════════════════════════════════════════════════════════════

    /// Add an object to the scene with a mesh.
    pub fn add_object(&mut self, mesh: Mesh, transform: Transform) -> ObjectId {
        let mesh_id = mesh.id;
        let name = mesh.name.clone();
        self.meshes.insert(mesh_id, mesh);

        let obj = Object::mesh(name, mesh_id).with_transform(transform);
        self.metadata.touch();
        self.scene.add_object(obj)
    }

    /// Add an empty object to the scene.
    pub fn add_empty(&mut self, name: impl Into<String>, transform: Transform) -> ObjectId {
        let obj = Object::new(name).with_transform(transform);
        self.metadata.touch();
        self.scene.add_object(obj)
    }

    /// Get an object by ID.
    #[must_use]
    pub fn object(&self, id: ObjectId) -> Option<&Object> {
        self.scene.get(id)
    }

    /// Get a mutable object by ID.
    pub fn object_mut(&mut self, id: ObjectId) -> Option<&mut Object> {
        self.metadata.touch();
        self.scene.get_mut(id)
    }

    /// Remove an object from the scene.
    pub fn remove_object(&mut self, id: ObjectId) -> CoreResult<Object> {
        self.metadata.touch();
        self.scene.remove_object(id)
    }

    /// Get all objects.
    pub fn objects(&self) -> impl Iterator<Item = &Object> {
        self.scene.objects()
    }

    /// Get object count.
    #[must_use]
    pub fn object_count(&self) -> usize {
        self.scene.object_count()
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Selection
    // ══════════════════════════════════════════════════════════════════════════

    /// Get selected objects.
    #[must_use]
    pub fn selected_objects(&self) -> Vec<ObjectId> {
        self.scene.selected()
    }

    /// Select an object.
    pub fn select_object(&mut self, id: ObjectId) {
        self.scene.select(id);
    }

    /// Deselect an object.
    pub fn deselect_object(&mut self, id: ObjectId) {
        self.scene.deselect(id);
    }

    /// Clear selection.
    pub fn clear_selection(&mut self) {
        self.scene.clear_selection();
    }

    /// Select all objects.
    pub fn select_all(&mut self) {
        self.scene.select_all();
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Document State
    // ══════════════════════════════════════════════════════════════════════════

    /// Check if the document has been modified.
    #[must_use]
    pub fn is_modified(&self) -> bool {
        self.history.is_modified()
    }

    /// Check if the document is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.scene.is_empty() && self.meshes.is_empty()
    }

    /// Get document statistics.
    #[must_use]
    pub fn statistics(&self) -> DocumentStatistics {
        let mut total_vertices = 0;
        let mut total_faces = 0;
        let mut total_triangles = 0;

        for mesh in self.meshes.values() {
            total_vertices += mesh.vertex_count();
            total_faces += mesh.face_count();
            total_triangles += mesh.triangle_count();
        }

        DocumentStatistics {
            object_count: self.scene.object_count(),
            mesh_count: self.meshes.len(),
            material_count: self.materials.len(),
            layer_count: self.layers.layer_count(),
            total_vertices,
            total_faces,
            total_triangles,
        }
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new("Untitled")
    }
}

/// Document statistics.
#[derive(Debug, Clone, Copy)]
pub struct DocumentStatistics {
    /// Number of objects in the scene.
    pub object_count: usize,
    /// Number of meshes.
    pub mesh_count: usize,
    /// Number of materials.
    pub material_count: usize,
    /// Number of layers.
    pub layer_count: usize,
    /// Total vertex count across all meshes.
    pub total_vertices: usize,
    /// Total face count across all meshes.
    pub total_faces: usize,
    /// Total triangle count (for rendering).
    pub total_triangles: usize,
}

/// Serializable document data for saving.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentData {
    /// Document format version.
    pub version: u32,
    /// Document ID.
    pub id: DocumentId,
    /// Metadata.
    pub metadata: DocumentMetadata,
    /// Settings.
    pub settings: DocumentSettings,
    /// Serialized meshes.
    pub meshes: Vec<crate::geometry::MeshData>,
    /// Serialized materials.
    pub materials: Vec<Material>,
    /// Layer data.
    pub layers: Vec<Layer>,
    /// Object data (simplified for serialization).
    pub objects: Vec<ObjectData>,
}

/// Simplified object data for serialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectData {
    /// Object ID.
    pub id: ObjectId,
    /// Object name.
    pub name: String,
    /// Transform components.
    pub transform: crate::transform::TransformComponent,
    /// Parent ID.
    pub parent: Option<ObjectId>,
    /// Mesh ID.
    pub mesh: Option<MeshId>,
    /// Material IDs.
    pub materials: Vec<MaterialId>,
    /// Layer ID.
    pub layer: Option<LayerId>,
}

impl Document {
    /// Export document to serializable data.
    #[must_use]
    pub fn to_data(&self) -> DocumentData {
        DocumentData {
            version: DOCUMENT_VERSION,
            id: self.id,
            metadata: self.metadata.clone(),
            settings: self.settings.clone(),
            meshes: self
                .meshes
                .values()
                .map(super::geometry::mesh::Mesh::to_data)
                .collect(),
            materials: self.materials.values().cloned().collect(),
            layers: self.layers.layers().cloned().collect(),
            objects: self
                .scene
                .objects()
                .map(|obj| ObjectData {
                    id: obj.id,
                    name: obj.name.clone(),
                    transform: *obj.transform.components(),
                    parent: obj.parent,
                    mesh: obj.mesh,
                    materials: obj.materials.clone(),
                    layer: obj.layer,
                })
                .collect(),
        }
    }

    /// Import document from serializable data.
    #[must_use]
    pub fn from_data(data: DocumentData) -> Self {
        let mut doc = Self {
            id: data.id,
            metadata: data.metadata,
            settings: data.settings,
            scene: SceneGraph::new(),
            meshes: HashMap::new(),
            materials: HashMap::new(),
            layers: LayerManager::new(),
            history: History::new(),
            file_path: None,
        };

        // Load meshes
        for mesh_data in data.meshes {
            let mesh = Mesh::from_data(mesh_data);
            doc.meshes.insert(mesh.id, mesh);
        }

        // Load materials
        for material in data.materials {
            doc.materials.insert(material.id, material);
        }

        // Load layers
        for layer in data.layers {
            doc.layers.add_layer(layer);
        }

        // Load objects
        for obj_data in data.objects {
            let obj = Object {
                id: obj_data.id,
                name: obj_data.name,
                object_type: if obj_data.mesh.is_some() {
                    crate::hierarchy::ObjectType::Mesh
                } else {
                    crate::hierarchy::ObjectType::Empty
                },
                transform: Transform::new(obj_data.transform),
                parent: obj_data.parent,
                children: smallvec::SmallVec::new(),
                mesh: obj_data.mesh,
                materials: obj_data.materials,
                layer: obj_data.layer,
                visibility: crate::hierarchy::ObjectVisibility::default(),
                selected: false,
                locked: false,
                user_data: HashMap::new(),
            };
            doc.scene.add_object(obj);
        }

        doc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_creation() {
        let doc = Document::new("Test Document");
        assert_eq!(doc.metadata.title, "Test Document");
        assert!(doc.is_empty());
    }

    #[test]
    fn test_document_with_default_cube() {
        let doc = Document::with_default_cube();
        assert_eq!(doc.mesh_count(), 1);
        assert_eq!(doc.object_count(), 1);
    }

    #[test]
    fn test_add_mesh() {
        let mut doc = Document::new("Test");
        let mesh = Mesh::sphere(1.0);
        let id = doc.add_mesh(mesh);

        assert!(doc.mesh(id).is_some());
        assert_eq!(doc.mesh_count(), 1);
    }

    #[test]
    fn test_add_object() {
        let mut doc = Document::new("Test");
        let mesh = Mesh::cube(1.0);
        let id = doc.add_object(mesh, Transform::identity());

        assert!(doc.object(id).is_some());
        assert_eq!(doc.object_count(), 1);
        assert_eq!(doc.mesh_count(), 1);
    }

    #[test]
    fn test_selection() {
        let mut doc = Document::new("Test");
        let mesh = Mesh::cube(1.0);
        let id = doc.add_object(mesh, Transform::identity());

        doc.select_object(id);
        assert_eq!(doc.selected_objects().len(), 1);

        doc.clear_selection();
        assert!(doc.selected_objects().is_empty());
    }

    #[test]
    fn test_document_statistics() {
        let mut doc = Document::new("Test");
        let mesh = Mesh::cube(1.0);
        doc.add_object(mesh, Transform::identity());

        let stats = doc.statistics();
        assert_eq!(stats.object_count, 1);
        assert_eq!(stats.mesh_count, 1);
        assert!(stats.total_vertices > 0);
    }

    #[test]
    fn test_document_serialization() {
        let mut doc = Document::new("Test");
        let mesh = Mesh::cube(1.0);
        doc.add_object(mesh, Transform::identity());

        let data = doc.to_data();
        let restored = Document::from_data(data);

        assert_eq!(restored.metadata.title, doc.metadata.title);
        assert_eq!(restored.mesh_count(), doc.mesh_count());
        assert_eq!(restored.object_count(), doc.object_count());
    }
}
