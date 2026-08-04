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

//! Checkpoint system for progress persistence.

use std::collections::HashMap;
use std::path::PathBuf;

use super::protocol::JobResult;

/// Checkpoint manager for saving/loading render progress.
pub struct CheckpointManager {
    checkpoint_dir: PathBuf,
    checkpoints: HashMap<u32, Vec<JobResult>>,
}

impl CheckpointManager {
    pub fn new(checkpoint_dir: PathBuf) -> Self {
        // Create checkpoint directory if it doesn't exist
        std::fs::create_dir_all(&checkpoint_dir).ok();

        Self {
            checkpoint_dir,
            checkpoints: HashMap::new(),
        }
    }

    /// Save completed tiles for a frame (in-memory for now).
    pub async fn save_frame(&mut self, frame: u32, tiles: &[JobResult]) -> anyhow::Result<()> {
        // In-memory checkpoint
        self.checkpoints.insert(frame, tiles.to_vec());

        // Optionally persist to disk
        let path = self
            .checkpoint_dir
            .join(format!("frame_{:05}.checkpoint", frame));
        let data = bincode::serialize(tiles)?;
        tokio::fs::write(path, data).await?;

        tracing::debug!(
            "Saved checkpoint for frame {} ({} tiles)",
            frame,
            tiles.len()
        );
        Ok(())
    }

    /// Load tiles from checkpoint.
    pub async fn load_frame(&self, frame: u32) -> anyhow::Result<Vec<JobResult>> {
        // Try in-memory first
        if let Some(tiles) = self.checkpoints.get(&frame) {
            return Ok(tiles.clone());
        }

        // Load from disk
        let path = self
            .checkpoint_dir
            .join(format!("frame_{:05}.checkpoint", frame));
        let data = tokio::fs::read(path).await?;
        let tiles: Vec<JobResult> = bincode::deserialize(&data)?;

        tracing::debug!(
            "Loaded checkpoint for frame {} ({} tiles)",
            frame,
            tiles.len()
        );
        Ok(tiles)
    }

    /// Check if frame has checkpoint.
    pub fn has_checkpoint(&self, frame: u32) -> bool {
        if self.checkpoints.contains_key(&frame) {
            return true;
        }

        let path = self
            .checkpoint_dir
            .join(format!("frame_{:05}.checkpoint", frame));
        path.exists()
    }

    /// Clear checkpoint for frame.
    pub async fn clear_frame(&mut self, frame: u32) -> anyhow::Result<()> {
        self.checkpoints.remove(&frame);

        let path = self
            .checkpoint_dir
            .join(format!("frame_{:05}.checkpoint", frame));
        if path.exists() {
            tokio::fs::remove_file(path).await?;
        }

        Ok(())
    }
}
