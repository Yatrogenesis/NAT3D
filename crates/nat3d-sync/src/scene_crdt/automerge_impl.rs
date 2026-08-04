// SOTA 10: Multi-User CRDT Synchronization using Automerge
use automerge::{transaction::Transactable, AutoCommit, ObjType, ReadDoc, ROOT};

/// Manages a distributed CRDT scene graph
pub struct SceneCrdt {
    doc: AutoCommit,
}

impl SceneCrdt {
    /// Create a new CRDT scene graph.
    pub fn new() -> Self {
        let mut doc = AutoCommit::new();
        // Initialize root layout
        doc.put_object(ROOT, "scene", ObjType::Map).unwrap();
        Self { doc }
    }

    /// Update object position in the CRDT doc.
    pub fn update_object_position(&mut self, obj_id: &str, x: f64, y: f64, z: f64) {
        if let Ok(Some((_, scene_obj_id))) = self.doc.get(ROOT, "scene") {
            if let Ok(obj_map_id) = self.doc.put_object(&scene_obj_id, obj_id, ObjType::Map) {
                self.doc.put(&obj_map_id, "x", x).unwrap();
                self.doc.put(&obj_map_id, "y", y).unwrap();
                self.doc.put(&obj_map_id, "z", z).unwrap();
            }
        }
    }

    /// Serialize the document state.
    pub fn get_save_data(&mut self) -> Vec<u8> {
        self.doc.save()
    }
}

impl Default for SceneCrdt {
    fn default() -> Self {
        Self::new()
    }
}
