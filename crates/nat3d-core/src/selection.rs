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

//! Selection system for NAT3D.
//!
//! Provides selection management for objects, vertices, edges, and faces.

use crate::hierarchy::ObjectId;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Selection mode determines what type of elements can be selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum SelectionMode {
    /// Object mode - select entire objects.
    #[default]
    Object,
    /// Vertex mode - select individual vertices.
    Vertex,
    /// Edge mode - select edges.
    Edge,
    /// Face mode - select faces/polygons.
    Face,
    /// Mixed mode - allows selecting any element type.
    Mixed,
}

impl std::fmt::Display for SelectionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Object => write!(f, "Object"),
            Self::Vertex => write!(f, "Vertex"),
            Self::Edge => write!(f, "Edge"),
            Self::Face => write!(f, "Face"),
            Self::Mixed => write!(f, "Mixed"),
        }
    }
}

/// A selectable element identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SelectableId {
    /// Object identifier.
    Object(ObjectId),
    /// Vertex identifier with parent object.
    Vertex(ObjectId, usize),
    /// Edge identifier with parent object.
    Edge(ObjectId, usize),
    /// Face identifier with parent object.
    Face(ObjectId, usize),
}

impl SelectableId {
    /// Get the parent object ID.
    #[must_use]
    pub fn object_id(&self) -> ObjectId {
        match self {
            Self::Object(id) => *id,
            Self::Vertex(id, _) => *id,
            Self::Edge(id, _) => *id,
            Self::Face(id, _) => *id,
        }
    }

    /// Check if this is an object selection.
    #[must_use]
    pub fn is_object(&self) -> bool {
        matches!(self, Self::Object(_))
    }

    /// Check if this is a vertex selection.
    #[must_use]
    pub fn is_vertex(&self) -> bool {
        matches!(self, Self::Vertex(_, _))
    }

    /// Check if this is an edge selection.
    #[must_use]
    pub fn is_edge(&self) -> bool {
        matches!(self, Self::Edge(_, _))
    }

    /// Check if this is a face selection.
    #[must_use]
    pub fn is_face(&self) -> bool {
        matches!(self, Self::Face(_, _))
    }
}

/// A set of selected elements.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SelectionSet {
    /// Selected elements.
    elements: HashSet<SelectableId>,
    /// Primary/active selection (last selected).
    active: Option<SelectableId>,
}

impl SelectionSet {
    /// Create a new empty selection set.
    #[must_use]
    pub fn new() -> Self {
        Self {
            elements: HashSet::new(),
            active: None,
        }
    }

    /// Check if the selection is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Get the number of selected elements.
    #[must_use]
    pub fn len(&self) -> usize {
        self.elements.len()
    }

    /// Check if an element is selected.
    #[must_use]
    pub fn contains(&self, id: &SelectableId) -> bool {
        self.elements.contains(id)
    }

    /// Get the active (last selected) element.
    #[must_use]
    pub fn active(&self) -> Option<SelectableId> {
        self.active
    }

    /// Add an element to the selection.
    pub fn add(&mut self, id: SelectableId) {
        self.elements.insert(id);
        self.active = Some(id);
    }

    /// Remove an element from the selection.
    pub fn remove(&mut self, id: &SelectableId) {
        self.elements.remove(id);
        if self.active.as_ref() == Some(id) {
            self.active = self.elements.iter().next().copied();
        }
    }

    /// Toggle an element's selection state.
    pub fn toggle(&mut self, id: SelectableId) {
        if self.elements.contains(&id) {
            self.remove(&id);
        } else {
            self.add(id);
        }
    }

    /// Clear the selection.
    pub fn clear(&mut self) {
        self.elements.clear();
        self.active = None;
    }

    /// Set the selection to a single element.
    pub fn set(&mut self, id: SelectableId) {
        self.elements.clear();
        self.elements.insert(id);
        self.active = Some(id);
    }

    /// Set the selection to multiple elements.
    pub fn set_multiple(&mut self, ids: impl IntoIterator<Item = SelectableId>) {
        self.elements.clear();
        self.active = None;
        for id in ids {
            self.elements.insert(id);
            self.active = Some(id);
        }
    }

    /// Get an iterator over selected elements.
    pub fn iter(&self) -> impl Iterator<Item = &SelectableId> {
        self.elements.iter()
    }

    /// Get all selected objects.
    #[must_use]
    pub fn objects(&self) -> Vec<ObjectId> {
        self.elements
            .iter()
            .filter_map(|id| {
                if let SelectableId::Object(obj_id) = id {
                    Some(*obj_id)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get all selected vertices for an object.
    #[must_use]
    pub fn vertices_for_object(&self, object_id: ObjectId) -> Vec<usize> {
        self.elements
            .iter()
            .filter_map(|id| {
                if let SelectableId::Vertex(obj_id, idx) = id {
                    if *obj_id == object_id {
                        return Some(*idx);
                    }
                }
                None
            })
            .collect()
    }

    /// Get all selected edges for an object.
    #[must_use]
    pub fn edges_for_object(&self, object_id: ObjectId) -> Vec<usize> {
        self.elements
            .iter()
            .filter_map(|id| {
                if let SelectableId::Edge(obj_id, idx) = id {
                    if *obj_id == object_id {
                        return Some(*idx);
                    }
                }
                None
            })
            .collect()
    }

    /// Get all selected faces for an object.
    #[must_use]
    pub fn faces_for_object(&self, object_id: ObjectId) -> Vec<usize> {
        self.elements
            .iter()
            .filter_map(|id| {
                if let SelectableId::Face(obj_id, idx) = id {
                    if *obj_id == object_id {
                        return Some(*idx);
                    }
                }
                None
            })
            .collect()
    }

    /// Get all unique objects that have selected elements.
    #[must_use]
    pub fn affected_objects(&self) -> Vec<ObjectId> {
        let mut objects: Vec<ObjectId> =
            self.elements.iter().map(SelectableId::object_id).collect();
        objects.sort();
        objects.dedup();
        objects
    }
}

impl IntoIterator for SelectionSet {
    type Item = SelectableId;
    type IntoIter = std::collections::hash_set::IntoIter<SelectableId>;

    fn into_iter(self) -> Self::IntoIter {
        self.elements.into_iter()
    }
}

impl<'a> IntoIterator for &'a SelectionSet {
    type Item = &'a SelectableId;
    type IntoIter = std::collections::hash_set::Iter<'a, SelectableId>;

    fn into_iter(self) -> Self::IntoIter {
        self.elements.iter()
    }
}

/// The main selection manager.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Selection {
    /// Current selection mode.
    mode: SelectionMode,
    /// Current selection set.
    selection: SelectionSet,
    /// Previous selection (for undo).
    previous: Option<SelectionSet>,
}

impl Selection {
    /// Create a new selection manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            mode: SelectionMode::Object,
            selection: SelectionSet::new(),
            previous: None,
        }
    }

    /// Get the current selection mode.
    #[must_use]
    pub fn mode(&self) -> SelectionMode {
        self.mode
    }

    /// Set the selection mode.
    pub fn set_mode(&mut self, mode: SelectionMode) {
        if self.mode != mode {
            // Clear selection when changing modes (except to/from Mixed)
            if self.mode != SelectionMode::Mixed && mode != SelectionMode::Mixed {
                self.save_previous();
                self.selection.clear();
            }
            self.mode = mode;
        }
    }

    /// Get the current selection set.
    #[must_use]
    pub fn selection(&self) -> &SelectionSet {
        &self.selection
    }

    /// Get a mutable reference to the selection set.
    pub fn selection_mut(&mut self) -> &mut SelectionSet {
        &mut self.selection
    }

    /// Check if anything is selected.
    #[must_use]
    pub fn has_selection(&self) -> bool {
        !self.selection.is_empty()
    }

    /// Get the number of selected elements.
    #[must_use]
    pub fn count(&self) -> usize {
        self.selection.len()
    }

    /// Select an object.
    pub fn select_object(&mut self, id: ObjectId) {
        self.save_previous();
        self.selection.add(SelectableId::Object(id));
    }

    /// Select a vertex.
    pub fn select_vertex(&mut self, object_id: ObjectId, vertex_index: usize) {
        self.save_previous();
        self.selection
            .add(SelectableId::Vertex(object_id, vertex_index));
    }

    /// Select an edge.
    pub fn select_edge(&mut self, object_id: ObjectId, edge_index: usize) {
        self.save_previous();
        self.selection
            .add(SelectableId::Edge(object_id, edge_index));
    }

    /// Select a face.
    pub fn select_face(&mut self, object_id: ObjectId, face_index: usize) {
        self.save_previous();
        self.selection
            .add(SelectableId::Face(object_id, face_index));
    }

    /// Deselect an element.
    pub fn deselect(&mut self, id: &SelectableId) {
        self.save_previous();
        self.selection.remove(id);
    }

    /// Toggle selection of an element.
    pub fn toggle(&mut self, id: SelectableId) {
        self.save_previous();
        self.selection.toggle(id);
    }

    /// Clear the selection.
    pub fn clear(&mut self) {
        if !self.selection.is_empty() {
            self.save_previous();
            self.selection.clear();
        }
    }

    /// Set selection to a single element, clearing previous selection.
    pub fn set(&mut self, id: SelectableId) {
        self.save_previous();
        self.selection.set(id);
    }

    /// Restore the previous selection.
    pub fn restore_previous(&mut self) {
        if let Some(prev) = self.previous.take() {
            let current = std::mem::replace(&mut self.selection, prev);
            self.previous = Some(current);
        }
    }

    /// Save the current selection as previous.
    fn save_previous(&mut self) {
        self.previous = Some(self.selection.clone());
    }

    /// Select all elements of the current mode type.
    pub fn select_all_of_mode(&mut self, ids: impl IntoIterator<Item = SelectableId>) {
        self.save_previous();
        for id in ids {
            let matches_mode = matches!(
                (&self.mode, &id),
                (SelectionMode::Object, SelectableId::Object(_))
                    | (SelectionMode::Vertex, SelectableId::Vertex(_, _))
                    | (SelectionMode::Edge, SelectableId::Edge(_, _))
                    | (SelectionMode::Face, SelectableId::Face(_, _))
                    | (SelectionMode::Mixed, _)
            );

            if matches_mode {
                self.selection.add(id);
            }
        }
    }

    /// Invert the selection among a set of elements.
    pub fn invert(&mut self, all_ids: impl IntoIterator<Item = SelectableId>) {
        self.save_previous();
        let current: HashSet<_> = self.selection.iter().copied().collect();
        self.selection.clear();

        for id in all_ids {
            if !current.contains(&id) {
                self.selection.add(id);
            }
        }
    }

    /// Grow the selection to include connected elements.
    #[must_use]
    pub fn grow_selection(
        &self,
        get_connected: impl Fn(&SelectableId) -> Vec<SelectableId>,
    ) -> SelectionSet {
        let mut new_selection = self.selection.clone();

        for id in self.selection.iter() {
            for connected in get_connected(id) {
                new_selection.add(connected);
            }
        }

        new_selection
    }

    /// Shrink the selection by removing boundary elements.
    #[must_use]
    pub fn shrink_selection(&self, is_boundary: impl Fn(&SelectableId) -> bool) -> SelectionSet {
        let mut new_selection = SelectionSet::new();

        for id in self.selection.iter() {
            if !is_boundary(id) {
                new_selection.add(*id);
            }
        }

        new_selection
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn test_object_id() -> ObjectId {
        ObjectId(Uuid::new_v4())
    }

    #[test]
    fn test_selection_set_basic() {
        let mut sel = SelectionSet::new();
        assert!(sel.is_empty());

        let obj_id = test_object_id();
        sel.add(SelectableId::Object(obj_id));

        assert!(!sel.is_empty());
        assert_eq!(sel.len(), 1);
        assert!(sel.contains(&SelectableId::Object(obj_id)));
    }

    #[test]
    fn test_selection_toggle() {
        let mut sel = SelectionSet::new();
        let obj_id = test_object_id();
        let id = SelectableId::Object(obj_id);

        sel.toggle(id);
        assert!(sel.contains(&id));

        sel.toggle(id);
        assert!(!sel.contains(&id));
    }

    #[test]
    fn test_selection_modes() {
        let mut selection = Selection::new();
        assert_eq!(selection.mode(), SelectionMode::Object);

        selection.set_mode(SelectionMode::Vertex);
        assert_eq!(selection.mode(), SelectionMode::Vertex);
    }

    #[test]
    fn test_selection_by_type() {
        let mut sel = SelectionSet::new();
        let obj_id = test_object_id();

        sel.add(SelectableId::Object(obj_id));
        sel.add(SelectableId::Vertex(obj_id, 0));
        sel.add(SelectableId::Vertex(obj_id, 1));
        sel.add(SelectableId::Edge(obj_id, 0));
        sel.add(SelectableId::Face(obj_id, 0));

        assert_eq!(sel.objects().len(), 1);
        assert_eq!(sel.vertices_for_object(obj_id).len(), 2);
        assert_eq!(sel.edges_for_object(obj_id).len(), 1);
        assert_eq!(sel.faces_for_object(obj_id).len(), 1);
    }

    #[test]
    fn test_active_selection() {
        let mut sel = SelectionSet::new();
        let obj1 = test_object_id();
        let obj2 = test_object_id();

        sel.add(SelectableId::Object(obj1));
        assert_eq!(sel.active(), Some(SelectableId::Object(obj1)));

        sel.add(SelectableId::Object(obj2));
        assert_eq!(sel.active(), Some(SelectableId::Object(obj2)));
    }

    #[test]
    fn test_restore_previous() {
        let mut selection = Selection::new();
        let obj1 = test_object_id();
        let obj2 = test_object_id();

        selection.select_object(obj1);
        selection.select_object(obj2);
        selection.clear();

        assert!(!selection.has_selection());

        selection.restore_previous();
        assert!(selection.has_selection());
    }

    #[test]
    fn test_affected_objects() {
        let mut sel = SelectionSet::new();
        let obj1 = test_object_id();
        let obj2 = test_object_id();

        sel.add(SelectableId::Vertex(obj1, 0));
        sel.add(SelectableId::Vertex(obj1, 1));
        sel.add(SelectableId::Face(obj2, 0));

        let affected = sel.affected_objects();
        assert_eq!(affected.len(), 2);
    }
}
