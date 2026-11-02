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
    /// Adds an object. Ignored by `apply_operation` if an object with the same id already exists with a timestamp greater than or equal to this one; otherwise the object is created at the origin with identity rotation, unit scale and material name `"default"`.
    AddObject {
        /// Identifier of the object.
        id: Uuid,
        /// Display name of the object.
        name: String,
        /// Object type as a free-form string.
        object_type: String,
        /// Logical timestamp of the operation, compared for last-write-wins; its unit is not defined by the code.
        timestamp: u64,
    },
    /// Removes an object. Applied only if the object exists and this timestamp is strictly greater than the object's timestamp.
    RemoveObject {
        /// Identifier of the object to remove.
        id: Uuid,
        /// Logical timestamp of the operation.
        timestamp: u64,
    },
    /// Sets position, rotation and scale of an existing object. Applied only if the object exists and the timestamp is strictly greater than the object's timestamp.
    UpdateObjectTransform {
        /// Identifier of the object to update.
        id: Uuid,
        /// New position (three components).
        position: [f32; 3],
        /// New rotation (four components; the initial value for new objects is `[0, 0, 0, 1]`).
        rotation: [f32; 4],
        /// New scale (three components).
        scale: [f32; 3],
        /// Logical timestamp of the operation.
        timestamp: u64,
    },
    /// Sets the material name of an existing object. Applied only if the object exists and the timestamp is strictly greater than the object's timestamp.
    UpdateMaterial {
        /// Identifier of the object to update.
        id: Uuid,
        /// New material name.
        material_name: String,
        /// Logical timestamp of the operation.
        timestamp: u64,
    },
    /// Creates or replaces a light. Ignored if a light with the same id exists with a timestamp greater than or equal to this one.
    UpdateLight {
        /// Identifier of the light.
        id: Uuid,
        /// Light type as a free-form string.
        light_type: String,
        /// Light intensity.
        intensity: f32,
        /// Light color (three components).
        color: [f32; 3],
        /// Logical timestamp of the operation.
        timestamp: u64,
    },
    /// Sets the scene camera. Ignored if a camera exists with a timestamp greater than or equal to this one.
    SetCamera {
        /// Camera position (three components).
        position: [f32; 3],
        /// Camera target (three components).
        target: [f32; 3],
        /// Camera field of view; the code does not define its unit.
        fov: f32,
        /// Logical timestamp of the operation.
        timestamp: u64,
    },
}

impl CrdtOperation {
    /// Returns the timestamp carried by the operation, whichever variant it is.
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
    /// Identifier of the object.
    pub id: Uuid,
    /// Display name of the object.
    pub name: String,
    /// Object type as a free-form string.
    pub object_type: String,
    /// Position (three components).
    pub position: [f32; 3],
    /// Rotation (four components).
    pub rotation: [f32; 4],
    /// Scale (three components).
    pub scale: [f32; 3],
    /// Name of the material assigned to the object.
    pub material_name: String,
    /// Timestamp of the last operation applied to this object.
    pub timestamp: u64,
}

/// Scene light (simplified).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneLight {
    /// Identifier of the light.
    pub id: Uuid,
    /// Light type as a free-form string.
    pub light_type: String,
    /// Light intensity.
    pub intensity: f32,
    /// Light color (three components).
    pub color: [f32; 3],
    /// Timestamp of the last operation applied to this light.
    pub timestamp: u64,
}

/// Scene camera (simplified).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneCamera {
    /// Camera position (three components).
    pub position: [f32; 3],
    /// Camera target (three components).
    pub target: [f32; 3],
    /// Camera field of view; the code does not define its unit.
    pub fov: f32,
    /// Timestamp of the last operation applied to the camera.
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
    /// Creates an empty scene state for the given node id, with no objects, lights or camera and version 0.
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

    /// Returns the version counter, which `apply_operation` increments by one each time an operation is not ignored.
    pub fn get_version(&self) -> u64 {
        self.version
    }

    /// Returns the objects keyed by id.
    pub fn get_objects(&self) -> &HashMap<Uuid, SceneObject> {
        &self.objects
    }

    /// Returns the lights keyed by id.
    pub fn get_lights(&self) -> &HashMap<Uuid, SceneLight> {
        &self.lights
    }

    /// Returns the camera, if one has been set.
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
