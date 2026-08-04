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

//! Scene graph and object hierarchy.
//!
//! Provides the scene graph structure for organizing 3D objects
//! in a parent-child hierarchy with transform inheritance.

use crate::error::{CoreError, CoreResult};
use crate::geometry::MeshId;
use crate::layer::LayerId;
use crate::material::MaterialId;
use crate::transform::Transform;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::collections::HashMap;
use uuid::Uuid;

/// Unique identifier for an object in the scene.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ObjectId(pub Uuid);

impl ObjectId {
    /// Create a new unique object ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create an object ID from an existing UUID.
    #[must_use]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for ObjectId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ObjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Object({})", &self.0.to_string()[..8])
    }
}

/// Type of object in the scene.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ObjectType {
    /// Empty object (just a transform).
    #[default]
    Empty,
    /// Mesh object.
    Mesh,
    /// Light object.
    Light,
    /// Camera object.
    Camera,
    /// Group/container object.
    Group,
    /// Armature/skeleton.
    Armature,
    /// Curve object.
    Curve,
    /// NURBS surface.
    NurbsSurface,
}

/// Object visibility flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectVisibility {
    /// Visible in viewport.
    pub viewport: bool,
    /// Visible in render.
    pub render: bool,
    /// Selectable in viewport.
    pub selectable: bool,
}

impl Default for ObjectVisibility {
    fn default() -> Self {
        Self {
            viewport: true,
            render: true,
            selectable: true,
        }
    }
}

impl ObjectVisibility {
    /// Create with all flags set.
    #[must_use]
    pub fn all() -> Self {
        Self::default()
    }

    /// Create with all flags unset.
    #[must_use]
    pub fn none() -> Self {
        Self {
            viewport: false,
            render: false,
            selectable: false,
        }
    }

    /// Create hidden visibility (not visible but still selectable).
    #[must_use]
    pub fn hidden() -> Self {
        Self {
            viewport: false,
            render: false,
            selectable: true,
        }
    }
}

/// An object in the scene graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Object {
    /// Unique identifier.
    pub id: ObjectId,
    /// Object name.
    pub name: String,
    /// Object type.
    pub object_type: ObjectType,
    /// Local transform (relative to parent).
    pub transform: Transform,
    /// Parent object ID (None for root objects).
    pub parent: Option<ObjectId>,
    /// Child object IDs.
    pub children: SmallVec<[ObjectId; 4]>,
    /// Associated mesh ID (for mesh objects).
    pub mesh: Option<MeshId>,
    /// Associated material IDs.
    pub materials: Vec<MaterialId>,
    /// Layer assignment.
    pub layer: Option<LayerId>,
    /// Visibility settings.
    pub visibility: ObjectVisibility,
    /// Whether the object is selected.
    pub selected: bool,
    /// Whether the object is locked (cannot be modified).
    pub locked: bool,
    /// Custom user data.
    pub user_data: HashMap<String, String>,
}

impl Object {
    /// Create a new empty object.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: ObjectId::new(),
            name: name.into(),
            object_type: ObjectType::Empty,
            transform: Transform::identity(),
            parent: None,
            children: SmallVec::new(),
            mesh: None,
            materials: Vec::new(),
            layer: None,
            visibility: ObjectVisibility::default(),
            selected: false,
            locked: false,
            user_data: HashMap::new(),
        }
    }

    /// Create a new mesh object.
    #[must_use]
    pub fn mesh(name: impl Into<String>, mesh_id: MeshId) -> Self {
        Self {
            mesh: Some(mesh_id),
            object_type: ObjectType::Mesh,
            ..Self::new(name)
        }
    }

    /// Create a new group object.
    #[must_use]
    pub fn group(name: impl Into<String>) -> Self {
        Self {
            object_type: ObjectType::Group,
            ..Self::new(name)
        }
    }

    /// Builder method to set transform.
    #[must_use]
    pub fn with_transform(mut self, transform: Transform) -> Self {
        self.transform = transform;
        self
    }

    /// Builder method to set parent.
    #[must_use]
    pub fn with_parent(mut self, parent: ObjectId) -> Self {
        self.parent = Some(parent);
        self
    }

    /// Builder method to set layer.
    #[must_use]
    pub fn with_layer(mut self, layer: LayerId) -> Self {
        self.layer = Some(layer);
        self
    }

    /// Check if this object is a root object (no parent).
    #[must_use]
    pub fn is_root(&self) -> bool {
        self.parent.is_none()
    }

    /// Check if this object has children.
    #[must_use]
    pub fn has_children(&self) -> bool {
        !self.children.is_empty()
    }

    /// Get the number of direct children.
    #[must_use]
    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    /// Check if this object is an ancestor of another.
    #[must_use]
    pub fn is_ancestor_of(&self, other_id: ObjectId, graph: &SceneGraph) -> bool {
        graph.is_ancestor(self.id, other_id)
    }
}

/// The scene graph containing all objects and their relationships.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SceneGraph {
    /// All objects indexed by ID.
    objects: HashMap<ObjectId, Object>,
    /// Root object IDs (objects with no parent).
    roots: Vec<ObjectId>,
    /// Cached world transforms.
    #[serde(skip)]
    world_transforms: HashMap<ObjectId, Transform>,
}

impl SceneGraph {
    /// Create a new empty scene graph.
    #[must_use]
    pub fn new() -> Self {
        Self {
            objects: HashMap::new(),
            roots: Vec::new(),
            world_transforms: HashMap::new(),
        }
    }

    /// Get the number of objects in the scene.
    #[must_use]
    pub fn object_count(&self) -> usize {
        self.objects.len()
    }

    /// Check if the scene is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    /// Get the root object IDs.
    #[must_use]
    pub fn roots(&self) -> &[ObjectId] {
        &self.roots
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Object Management
    // ══════════════════════════════════════════════════════════════════════════

    /// Add an object to the scene.
    pub fn add_object(&mut self, object: Object) -> ObjectId {
        let id = object.id;

        // Handle parent relationship
        if let Some(parent_id) = object.parent {
            if let Some(parent) = self.objects.get_mut(&parent_id) {
                if !parent.children.contains(&id) {
                    parent.children.push(id);
                }
            }
        } else {
            // No parent, this is a root object
            if !self.roots.contains(&id) {
                self.roots.push(id);
            }
        }

        self.objects.insert(id, object);
        self.invalidate_world_transform(id);
        id
    }

    /// Remove an object from the scene.
    pub fn remove_object(&mut self, id: ObjectId) -> CoreResult<Object> {
        let object = self
            .objects
            .remove(&id)
            .ok_or(CoreError::ObjectNotFound(id.0))?;

        // Remove from parent's children
        if let Some(parent_id) = object.parent {
            if let Some(parent) = self.objects.get_mut(&parent_id) {
                parent.children.retain(|c| *c != id);
            }
        }

        // Remove from roots
        self.roots.retain(|r| *r != id);

        // Reparent children to scene root
        for child_id in &object.children {
            if let Some(child) = self.objects.get_mut(child_id) {
                child.parent = None;
                if !self.roots.contains(child_id) {
                    self.roots.push(*child_id);
                }
            }
        }

        self.world_transforms.remove(&id);
        Ok(object)
    }

    /// Get an object by ID.
    #[must_use]
    pub fn get(&self, id: ObjectId) -> Option<&Object> {
        self.objects.get(&id)
    }

    /// Get a mutable object by ID.
    pub fn get_mut(&mut self, id: ObjectId) -> Option<&mut Object> {
        self.invalidate_world_transform(id);
        self.objects.get_mut(&id)
    }

    /// Check if an object exists.
    #[must_use]
    pub fn contains(&self, id: ObjectId) -> bool {
        self.objects.contains_key(&id)
    }

    /// Get an iterator over all objects.
    pub fn objects(&self) -> impl Iterator<Item = &Object> {
        self.objects.values()
    }

    /// Get an iterator over all object IDs.
    pub fn object_ids(&self) -> impl Iterator<Item = ObjectId> + '_ {
        self.objects.keys().copied()
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Hierarchy Operations
    // ══════════════════════════════════════════════════════════════════════════

    /// Set the parent of an object.
    pub fn set_parent(
        &mut self,
        child_id: ObjectId,
        parent_id: Option<ObjectId>,
    ) -> CoreResult<()> {
        // Check for circular dependency
        if let Some(pid) = parent_id {
            if self.is_ancestor(child_id, pid) {
                return Err(CoreError::CircularDependency {
                    path: format!("{child_id} -> {pid}"),
                });
            }
        }

        // Get current parent
        let old_parent = self
            .get(child_id)
            .ok_or(CoreError::ObjectNotFound(child_id.0))?
            .parent;

        // Remove from old parent
        if let Some(old_pid) = old_parent {
            if let Some(old_parent_obj) = self.objects.get_mut(&old_pid) {
                old_parent_obj.children.retain(|c| *c != child_id);
            }
        } else {
            self.roots.retain(|r| *r != child_id);
        }

        // Add to new parent
        if let Some(new_pid) = parent_id {
            if let Some(new_parent) = self.objects.get_mut(&new_pid) {
                if !new_parent.children.contains(&child_id) {
                    new_parent.children.push(child_id);
                }
            }
        } else if !self.roots.contains(&child_id) {
            self.roots.push(child_id);
        }

        // Update child's parent reference
        if let Some(child) = self.objects.get_mut(&child_id) {
            child.parent = parent_id;
        }

        self.invalidate_world_transform(child_id);
        Ok(())
    }

    /// Check if `ancestor_id` is an ancestor of `descendant_id`.
    #[must_use]
    pub fn is_ancestor(&self, ancestor_id: ObjectId, descendant_id: ObjectId) -> bool {
        let mut current = Some(descendant_id);
        while let Some(id) = current {
            if id == ancestor_id {
                return true;
            }
            current = self.get(id).and_then(|obj| obj.parent);
        }
        false
    }

    /// Get all ancestors of an object (from parent to root).
    #[must_use]
    pub fn ancestors(&self, id: ObjectId) -> Vec<ObjectId> {
        let mut result = Vec::new();
        let mut current = self.get(id).and_then(|obj| obj.parent);
        while let Some(pid) = current {
            result.push(pid);
            current = self.get(pid).and_then(|obj| obj.parent);
        }
        result
    }

    /// Get all descendants of an object (depth-first).
    #[must_use]
    pub fn descendants(&self, id: ObjectId) -> Vec<ObjectId> {
        let mut result = Vec::new();
        let mut stack = vec![id];

        while let Some(current) = stack.pop() {
            if current != id {
                result.push(current);
            }
            if let Some(obj) = self.get(current) {
                for child in obj.children.iter().rev() {
                    stack.push(*child);
                }
            }
        }

        result
    }

    /// Get the depth of an object in the hierarchy (0 for roots).
    #[must_use]
    pub fn depth(&self, id: ObjectId) -> usize {
        self.ancestors(id).len()
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Transform Operations
    // ══════════════════════════════════════════════════════════════════════════

    /// Get the world transform of an object.
    pub fn world_transform(&mut self, id: ObjectId) -> Option<Transform> {
        if let Some(cached) = self.world_transforms.get(&id) {
            return Some(cached.clone());
        }

        let obj = self.objects.get(&id)?;
        let local = obj.transform.clone();

        let world = if let Some(parent_id) = obj.parent {
            let mut parent_world = self.world_transform(parent_id)?;
            parent_world.combine(&mut local.clone())
        } else {
            local
        };

        self.world_transforms.insert(id, world.clone());
        Some(world)
    }

    /// Invalidate cached world transforms for an object and its descendants.
    fn invalidate_world_transform(&mut self, id: ObjectId) {
        let mut stack = vec![id];
        while let Some(current) = stack.pop() {
            self.world_transforms.remove(&current);
            if let Some(obj) = self.objects.get(&current) {
                for child in &obj.children {
                    stack.push(*child);
                }
            }
        }
    }

    /// Invalidate all cached world transforms.
    pub fn invalidate_all_transforms(&mut self) {
        self.world_transforms.clear();
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Selection Operations
    // ══════════════════════════════════════════════════════════════════════════

    /// Get all selected objects.
    #[must_use]
    pub fn selected(&self) -> Vec<ObjectId> {
        self.objects
            .iter()
            .filter(|(_, obj)| obj.selected)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Select an object.
    pub fn select(&mut self, id: ObjectId) {
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.selected = true;
        }
    }

    /// Deselect an object.
    pub fn deselect(&mut self, id: ObjectId) {
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.selected = false;
        }
    }

    /// Clear all selection.
    pub fn clear_selection(&mut self) {
        for obj in self.objects.values_mut() {
            obj.selected = false;
        }
    }

    /// Select all objects.
    pub fn select_all(&mut self) {
        for obj in self.objects.values_mut() {
            if obj.visibility.selectable {
                obj.selected = true;
            }
        }
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Query Operations
    // ══════════════════════════════════════════════════════════════════════════

    /// Find objects by name.
    #[must_use]
    pub fn find_by_name(&self, name: &str) -> Vec<ObjectId> {
        self.objects
            .iter()
            .filter(|(_, obj)| obj.name == name)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Find objects by type.
    #[must_use]
    pub fn find_by_type(&self, object_type: ObjectType) -> Vec<ObjectId> {
        self.objects
            .iter()
            .filter(|(_, obj)| obj.object_type == object_type)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Find objects in a layer.
    #[must_use]
    pub fn find_in_layer(&self, layer: LayerId) -> Vec<ObjectId> {
        self.objects
            .iter()
            .filter(|(_, obj)| obj.layer == Some(layer))
            .map(|(id, _)| *id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_creation() {
        let obj = Object::new("Test");
        assert_eq!(obj.name, "Test");
        assert!(obj.is_root());
        assert!(!obj.has_children());
    }

    #[test]
    fn test_scene_graph_add() {
        let mut graph = SceneGraph::new();
        let obj = Object::new("Test");
        let id = graph.add_object(obj);

        assert!(graph.contains(id));
        assert_eq!(graph.object_count(), 1);
        assert_eq!(graph.roots().len(), 1);
    }

    #[test]
    fn test_parenting() {
        let mut graph = SceneGraph::new();

        let parent = Object::new("Parent");
        let parent_id = graph.add_object(parent);

        let child = Object::new("Child").with_parent(parent_id);
        let child_id = graph.add_object(child);

        assert_eq!(graph.roots().len(), 1);
        assert!(graph.get(parent_id).unwrap().children.contains(&child_id));
        assert_eq!(graph.get(child_id).unwrap().parent, Some(parent_id));
    }

    #[test]
    fn test_reparenting() {
        let mut graph = SceneGraph::new();

        let parent1 = Object::new("Parent1");
        let parent1_id = graph.add_object(parent1);

        let parent2 = Object::new("Parent2");
        let parent2_id = graph.add_object(parent2);

        let child = Object::new("Child").with_parent(parent1_id);
        let child_id = graph.add_object(child);

        graph.set_parent(child_id, Some(parent2_id)).unwrap();

        assert!(!graph.get(parent1_id).unwrap().children.contains(&child_id));
        assert!(graph.get(parent2_id).unwrap().children.contains(&child_id));
    }

    #[test]
    fn test_circular_dependency_prevention() {
        let mut graph = SceneGraph::new();

        let parent = Object::new("Parent");
        let parent_id = graph.add_object(parent);

        let child = Object::new("Child").with_parent(parent_id);
        let child_id = graph.add_object(child);

        // Trying to make parent a child of child should fail
        let result = graph.set_parent(parent_id, Some(child_id));
        assert!(result.is_err());
    }

    #[test]
    fn test_ancestors_and_descendants() {
        let mut graph = SceneGraph::new();

        let root = Object::new("Root");
        let root_id = graph.add_object(root);

        let child = Object::new("Child").with_parent(root_id);
        let child_id = graph.add_object(child);

        let grandchild = Object::new("Grandchild").with_parent(child_id);
        let grandchild_id = graph.add_object(grandchild);

        let ancestors = graph.ancestors(grandchild_id);
        assert_eq!(ancestors, vec![child_id, root_id]);

        let descendants = graph.descendants(root_id);
        assert!(descendants.contains(&child_id));
        assert!(descendants.contains(&grandchild_id));
    }

    #[test]
    fn test_selection() {
        let mut graph = SceneGraph::new();

        let obj1 = Object::new("Obj1");
        let id1 = graph.add_object(obj1);

        let obj2 = Object::new("Obj2");
        let id2 = graph.add_object(obj2);

        graph.select(id1);
        assert!(graph.get(id1).unwrap().selected);
        assert!(!graph.get(id2).unwrap().selected);

        let selected = graph.selected();
        assert_eq!(selected.len(), 1);
        assert!(selected.contains(&id1));

        graph.clear_selection();
        assert!(graph.selected().is_empty());
    }
}
