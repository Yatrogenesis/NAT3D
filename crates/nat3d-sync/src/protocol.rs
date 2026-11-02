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
        /// Independent contact ID; omitted by legacy single-touch senders.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        touch_id: Option<u32>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_input_event_decodes_without_contact_id() {
        let payload = br#"{"InputEvent":{"event_type":"move","x":1.0,"y":2.0,"pressure":null,"tilt":null,"azimuth":null,"timestamp":0}}"#;
        let event = SyncProtocol::decode(payload).unwrap();
        assert!(matches!(
            event,
            SyncMessage::InputEvent { touch_id: None, .. }
        ));
        let encoded = SyncProtocol::encode(&event).unwrap();
        assert!(!String::from_utf8(encoded).unwrap().contains("touch_id"));
    }

    #[test]
    fn separate_contact_ids_survive_wire_roundtrip() {
        for id in [10, 20] {
            let event = SyncMessage::InputEvent {
                event_type: "move".into(),
                x: 1.0,
                y: 2.0,
                pressure: None,
                tilt: None,
                azimuth: None,
                timestamp: 0,
                touch_id: Some(id),
            };
            let encoded = SyncProtocol::encode(&event).unwrap();
            let decoded = SyncProtocol::decode(&encoded).unwrap();
            assert!(
                matches!(decoded, SyncMessage::InputEvent { touch_id: Some(value), .. } if value == id)
            );
        }
    }

    #[test]
    fn decoded_contacts_drive_two_finger_zoom() {
        use crate::input::ipad::{IPadInput, ViewportTransform};
        let mut input = IPadInput::new(1600, 900);
        for (id, x) in [(10, 0.0), (20, 10.0), (20, 20.0)] {
            let event = SyncMessage::InputEvent {
                event_type: "move".into(),
                x,
                y: 0.0,
                pressure: None,
                tilt: None,
                azimuth: None,
                timestamp: 0,
                touch_id: Some(id),
            };
            let wire = SyncProtocol::encode(&event).unwrap();
            let SyncMessage::InputEvent { touch_id, x, y, .. } =
                SyncProtocol::decode(&wire).unwrap()
            else {
                panic!("expected input event");
            };
            input.handle_touch_event(touch_id.unwrap_or(0), x, y, false);
        }
        let ViewportTransform::Zoom { factor } = input.get_viewport_transform() else {
            panic!("expected two-finger zoom");
        };
        assert!((factor - 2.0).abs() < 1e-6);
    }
}
