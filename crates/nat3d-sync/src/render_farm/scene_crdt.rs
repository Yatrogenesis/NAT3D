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

//! Conflict-free scene synchronization using CRDTs.
//!
//! Implements VR-14 (Compartido Distribuido): eventual consistency.
//!
//! NOTE: This is a simplified Last-Write-Wins (LWW) implementation.
//! For production, use full CRDT library with version vectors.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// CRDT operation for scene changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CrdtOperation {
    AddObject {
        id: Uuid,
        name: String,
        object_type: String,
        timestamp: u64,
    },
    RemoveObject {
        id: Uuid,
        timestamp: u64,
    },
    UpdateObjectTransform {
        id: Uuid,
        position: [f32; 3],
        rotation: [f32; 4],
        scale: [f32; 3],
        timestamp: u64,
    },
    UpdateMaterial {
        id: Uuid,
        material_name: String,
        timestamp: u64,
    },
    UpdateLight {
        id: Uuid,
        light_type: String,
        intensity: f32,
        color: [f32; 3],
        timestamp: u64,
    },
    SetCamera {
        position: [f32; 3],
        target: [f32; 3],
        fov: f32,
        timestamp: u64,
    },
}

impl CrdtOperation {
    pub fn timestamp(&self) -> u64 {
        match self {
            Self::AddObject { timestamp, .. } => *timestamp,
            Self::RemoveObject { timestamp, .. } => *timestamp,
            Self::UpdateObjectTransform { timestamp, .. } => *timestamp,
            Self::UpdateMaterial { timestamp, .. } => *timestamp,
            Self::UpdateLight { timestamp, .. } => *timestamp,
            Self::SetCamera { timestamp, .. } => *timestamp,
        }
    }
}

/// Scene object (simplified).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneObject {
    pub id: Uuid,
    pub name: String,
    pub object_type: String,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    pub material_name: String,
    pub timestamp: u64,
}

/// Scene light (simplified).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneLight {
    pub id: Uuid,
    pub light_type: String,
    pub intensity: f32,
    pub color: [f32; 3],
    pub timestamp: u64,
}

/// Scene camera (simplified).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneCamera {
    pub position: [f32; 3],
    pub target: [f32; 3],
    pub fov: f32,
    pub timestamp: u64,
}

/// Scene CRDT state (Last-Write-Wins).
pub struct SceneCRDT {
    node_id: Uuid,
    objects: HashMap<Uuid, SceneObject>,
    lights: HashMap<Uuid, SceneLight>,
    camera: Option<SceneCamera>,
    version: u64,
}

impl SceneCRDT {
    pub fn new(node_id: Uuid) -> Self {
        Self {
            node_id,
            objects: HashMap::new(),
            lights: HashMap::new(),
            camera: None,
            version: 0,
        }
    }

    /// Apply a CRDT operation (idempotent).
    pub fn apply_operation(&mut self, op: CrdtOperation) {
        match op {
            CrdtOperation::AddObject {
                id,
                name,
                object_type,
                timestamp,
            } => {
                // LWW: only apply if timestamp is newer
                if let Some(existing) = self.objects.get(&id) {
                    if timestamp <= existing.timestamp {
                        return; // Older operation, ignore
                    }
                }

                let obj = SceneObject {
                    id,
                    name,
                    object_type,
                    position: [0.0, 0.0, 0.0],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                    scale: [1.0, 1.0, 1.0],
                    material_name: "default".to_string(),
                    timestamp,
                };
                self.objects.insert(id, obj);
            }

            CrdtOperation::RemoveObject { id, timestamp } => {
                if let Some(existing) = self.objects.get(&id) {
                    if timestamp > existing.timestamp {
                        self.objects.remove(&id);
                    }
                }
            }

            CrdtOperation::UpdateObjectTransform {
                id,
                position,
                rotation,
                scale,
                timestamp,
            } => {
                if let Some(obj) = self.objects.get_mut(&id) {
                    if timestamp > obj.timestamp {
                        obj.position = position;
                        obj.rotation = rotation;
                        obj.scale = scale;
                        obj.timestamp = timestamp;
                    }
                }
            }

            CrdtOperation::UpdateMaterial {
                id,
                material_name,
                timestamp,
            } => {
                if let Some(obj) = self.objects.get_mut(&id) {
                    if timestamp > obj.timestamp {
                        obj.material_name = material_name;
                        obj.timestamp = timestamp;
                    }
                }
            }

            CrdtOperation::UpdateLight {
                id,
                light_type,
                intensity,
                color,
                timestamp,
            } => {
                if let Some(existing) = self.lights.get(&id) {
                    if timestamp <= existing.timestamp {
                        return;
                    }
                }

                let light = SceneLight {
                    id,
                    light_type,
                    intensity,
                    color,
                    timestamp,
                };
                self.lights.insert(id, light);
            }

            CrdtOperation::SetCamera {
                position,
                target,
                fov,
                timestamp,
            } => {
                if let Some(existing) = &self.camera {
                    if timestamp <= existing.timestamp {
                        return;
                    }
                }

                self.camera = Some(SceneCamera {
                    position,
                    target,
                    fov,
                    timestamp,
                });
            }
        }

        self.version += 1;
    }

    /// Compute diff since a version.
    pub fn diff(&self, since_version: u64) -> Vec<CrdtOperation> {
        // Simplified: return all operations if version changed
        // In production, maintain operation log
        if since_version < self.version {
            // Return current state as operations
            let mut ops = Vec::new();

            for obj in self.objects.values() {
                ops.push(CrdtOperation::AddObject {
                    id: obj.id,
                    name: obj.name.clone(),
                    object_type: obj.object_type.clone(),
                    timestamp: obj.timestamp,
                });
            }

            for light in self.lights.values() {
                ops.push(CrdtOperation::UpdateLight {
                    id: light.id,
                    light_type: light.light_type.clone(),
                    intensity: light.intensity,
                    color: light.color,
                    timestamp: light.timestamp,
                });
            }

            if let Some(camera) = &self.camera {
                ops.push(CrdtOperation::SetCamera {
                    position: camera.position,
                    target: camera.target,
                    fov: camera.fov,
                    timestamp: camera.timestamp,
                });
            }

            ops
        } else {
            Vec::new()
        }
    }

    /// Merge another CRDT state.
    pub fn merge(&mut self, other: &SceneCRDT) {
        // Apply all operations from other
        for obj in other.objects.values() {
            let op = CrdtOperation::AddObject {
                id: obj.id,
                name: obj.name.clone(),
                object_type: obj.object_type.clone(),
                timestamp: obj.timestamp,
            };
            self.apply_operation(op);
        }

        for light in other.lights.values() {
            let op = CrdtOperation::UpdateLight {
                id: light.id,
                light_type: light.light_type.clone(),
                intensity: light.intensity,
                color: light.color,
                timestamp: light.timestamp,
            };
            self.apply_operation(op);
        }

        if let Some(camera) = &other.camera {
            let op = CrdtOperation::SetCamera {
                position: camera.position,
                target: camera.target,
                fov: camera.fov,
                timestamp: camera.timestamp,
            };
            self.apply_operation(op);
        }
    }

    pub fn get_version(&self) -> u64 {
        self.version
    }

    pub fn get_objects(&self) -> &HashMap<Uuid, SceneObject> {
        &self.objects
    }

    pub fn get_lights(&self) -> &HashMap<Uuid, SceneLight> {
        &self.lights
    }

    pub fn get_camera(&self) -> Option<&SceneCamera> {
        self.camera.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crdt_creation() {
        let node_id = Uuid::new_v4();
        let crdt = SceneCRDT::new(node_id);
        assert_eq!(crdt.get_version(), 0);
    }

    #[test]
    fn test_add_object() {
        let mut crdt = SceneCRDT::new(Uuid::new_v4());
        let obj_id = Uuid::new_v4();

        let op = CrdtOperation::AddObject {
            id: obj_id,
            name: "Cube".to_string(),
            object_type: "mesh".to_string(),
            timestamp: 1000,
        };

        crdt.apply_operation(op);
        assert_eq!(crdt.get_objects().len(), 1);
        assert_eq!(crdt.get_version(), 1);
    }

    #[test]
    fn test_lww_newer_wins() {
        let mut crdt = SceneCRDT::new(Uuid::new_v4());
        let obj_id = Uuid::new_v4();

        // Add object with timestamp 1000
        crdt.apply_operation(CrdtOperation::AddObject {
            id: obj_id,
            name: "Old".to_string(),
            object_type: "mesh".to_string(),
            timestamp: 1000,
        });

        // Try to add with older timestamp (should be ignored)
        crdt.apply_operation(CrdtOperation::AddObject {
            id: obj_id,
            name: "Older".to_string(),
            object_type: "mesh".to_string(),
            timestamp: 500,
        });

        assert_eq!(crdt.get_objects()[&obj_id].name, "Old");

        // Add with newer timestamp (should replace)
        crdt.apply_operation(CrdtOperation::AddObject {
            id: obj_id,
            name: "New".to_string(),
            object_type: "mesh".to_string(),
            timestamp: 2000,
        });

        assert_eq!(crdt.get_objects()[&obj_id].name, "New");
    }

    #[test]
    fn test_remove_object() {
        let mut crdt = SceneCRDT::new(Uuid::new_v4());
        let obj_id = Uuid::new_v4();

        crdt.apply_operation(CrdtOperation::AddObject {
            id: obj_id,
            name: "Cube".to_string(),
            object_type: "mesh".to_string(),
            timestamp: 1000,
        });

        crdt.apply_operation(CrdtOperation::RemoveObject {
            id: obj_id,
            timestamp: 2000,
        });

        assert_eq!(crdt.get_objects().len(), 0);
    }

    #[test]
    fn test_idempotency() {
        let mut crdt = SceneCRDT::new(Uuid::new_v4());
        let obj_id = Uuid::new_v4();

        let op = CrdtOperation::AddObject {
            id: obj_id,
            name: "Cube".to_string(),
            object_type: "mesh".to_string(),
            timestamp: 1000,
        };

        // Apply same operation twice
        crdt.apply_operation(op.clone());
        crdt.apply_operation(op);

        // Should only have one object (idempotent)
        assert_eq!(crdt.get_objects().len(), 1);
    }

    #[test]
    fn test_merge() {
        let mut crdt1 = SceneCRDT::new(Uuid::new_v4());
        let mut crdt2 = SceneCRDT::new(Uuid::new_v4());

        let obj_id = Uuid::new_v4();

        crdt1.apply_operation(CrdtOperation::AddObject {
            id: obj_id,
            name: "Cube1".to_string(),
            object_type: "mesh".to_string(),
            timestamp: 1000,
        });

        crdt2.apply_operation(CrdtOperation::AddObject {
            id: obj_id,
            name: "Cube2".to_string(),
            object_type: "mesh".to_string(),
            timestamp: 2000,
        });

        crdt1.merge(&crdt2);

        // Newer timestamp should win
        assert_eq!(crdt1.get_objects()[&obj_id].name, "Cube2");
    }

    #[test]
    fn test_diff() {
        let mut crdt = SceneCRDT::new(Uuid::new_v4());
        let obj_id = Uuid::new_v4();

        let v0 = crdt.get_version();

        crdt.apply_operation(CrdtOperation::AddObject {
            id: obj_id,
            name: "Cube".to_string(),
            object_type: "mesh".to_string(),
            timestamp: 1000,
        });

        let diff = crdt.diff(v0);
        assert!(diff.len() > 0);
    }
}
