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

//! Scene synchronization manager - broadcasts scene changes to workers.

use super::scene_crdt::{CrdtOperation, SceneCRDT};
use uuid::Uuid;

/// Sync manager for broadcasting scene changes.
pub struct SyncManager {
    local_scene: SceneCRDT,
    pending_ops: Vec<CrdtOperation>,
}

impl SyncManager {
    pub fn new(node_id: Uuid) -> Self {
        Self {
            local_scene: SceneCRDT::new(node_id),
            pending_ops: Vec::new(),
        }
    }

    /// Broadcast pending changes to workers (call every 100ms).
    pub fn get_pending_ops(&mut self) -> Vec<CrdtOperation> {
        std::mem::take(&mut self.pending_ops)
    }

    /// Apply a local change (from master UI).
    pub fn apply_local_change(&mut self, op: CrdtOperation) {
        self.local_scene.apply_operation(op.clone());
        self.pending_ops.push(op);
    }

    /// Apply a remote change (from worker).
    pub fn apply_remote_change(&mut self, op: CrdtOperation) {
        self.local_scene.apply_operation(op);
    }

    /// Get current scene version.
    pub fn get_version(&self) -> u64 {
        self.local_scene.get_version()
    }

    /// Get scene diff since version.
    pub fn get_diff(&self, since_version: u64) -> Vec<CrdtOperation> {
        self.local_scene.diff(since_version)
    }

    /// Get reference to local scene.
    pub fn get_scene(&self) -> &SceneCRDT {
        &self.local_scene
    }
}
