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

//! Sync protocol definition.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TileSpec {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderResult {
    pub tile_id: u32,
    pub frame: u32,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncMessage {
    Hello {
        version: String,
        device_name: String,
    },
    Ping {
        timestamp: u64,
    },
    Pong {
        timestamp: u64,
    },
    InputEvent {
        event_type: String,
        x: f32,
        y: f32,
        pressure: Option<f32>,
        tilt: Option<f32>,
        azimuth: Option<f32>,
        timestamp: u64,
    },
    PencilUpdate {
        x: f32,
        y: f32,
        force: f32,
        tilt_x: f32,
        tilt_y: f32,
        azimuth: f32,
        altitude: f32,
        in_contact: bool,
        double_tap: bool,
        timestamp: u64,
    },
    AssignTile(TileSpec, u32),
    SubmitResult(RenderResult),
    Disconnect {
        reason: String,
    },
}

pub struct SyncProtocol;
impl SyncProtocol {
    pub fn decode(data: &[u8]) -> Result<SyncMessage, serde_json::Error> {
        serde_json::from_slice(data)
    }
    pub fn encode(msg: &SyncMessage) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(msg)
    }
}
