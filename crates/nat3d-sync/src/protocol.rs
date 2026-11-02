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

/// Rectangular tile of the output image, given as an origin and a size.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TileSpec {
    /// X coordinate of the tile origin, as stored in the field (the code attaches no unit).
    pub x: u32,
    /// Y coordinate of the tile origin, as stored in the field (the code attaches no unit).
    pub y: u32,
    /// Tile width.
    pub width: u32,
    /// Tile height.
    pub height: u32,
}

/// Result of rendering one tile, as carried by [`SyncMessage::SubmitResult`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderResult {
    /// Identifier of the tile this result belongs to.
    pub tile_id: u32,
    /// Frame number the result belongs to.
    pub frame: u32,
    /// Raw result bytes; the code in this module does not interpret their layout.
    pub data: Vec<u8>,
}

/// Messages exchanged between devices by the sync protocol.
///
/// The type derives serde `Serialize`/`Deserialize` and is encoded with [`SyncProtocol::encode`] as JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncMessage {
    /// Greeting message carrying the sender's identity.
    Hello {
        /// Protocol/application version string reported by the sender.
        version: String,
        /// Human-readable name of the sending device.
        device_name: String,
    },
    /// Latency probe request.
    Ping {
        /// Sender timestamp carried by the probe; this module does not define its unit.
        timestamp: u64,
    },
    /// Reply to a [`SyncMessage::Ping`].
    Pong {
        /// Timestamp carried by the reply; this module does not define its unit.
        timestamp: u64,
    },
    /// Single input event (touch/pointer) from a remote device.
    InputEvent {
        /// Event kind as a free-form string. The application treats `"up"`, `"end"` and `"cancel"` as the end of a contact and handles any other value as a continuing contact.
        event_type: String,
        /// X position of the event, as sent by the device.
        x: f32,
        /// Y position of the event, as sent by the device.
        y: f32,
        /// Optional pressure value; `None` when the sender did not provide one.
        pressure: Option<f32>,
        /// Optional tilt value; `None` when the sender did not provide one.
        tilt: Option<f32>,
        /// Optional azimuth value; `None` when the sender did not provide one.
        azimuth: Option<f32>,
        /// Timestamp carried by the event; this module does not define its unit.
        timestamp: u64,
        /// Independent contact ID; omitted by legacy single-touch senders.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        touch_id: Option<u32>,
    },
    /// Stylus (pencil) state update from a remote device.
    PencilUpdate {
        /// X position of the stylus, as sent by the device.
        x: f32,
        /// Y position of the stylus, as sent by the device.
        y: f32,
        /// Stylus force. The application passes it to the pencil handler as the pressure value.
        force: f32,
        /// Stylus tilt along the X axis. The application combines `tilt_x` and `tilt_y` into an altitude angle as `max(PI/2 - hypot(tilt_x, tilt_y), 0)`.
        tilt_x: f32,
        /// Stylus tilt along the Y axis; see `tilt_x`.
        tilt_y: f32,
        /// Stylus azimuth angle. The application passes it to the pencil handler as the azimuth angle.
        azimuth: f32,
        /// Stylus altitude as sent by the device; the application's pencil handling shown in this repository derives altitude from `tilt_x`/`tilt_y` instead of reading this field.
        altitude: f32,
        /// Whether the stylus is in contact with the surface; the application only processes the update when this is `true`.
        in_contact: bool,
        /// Double-tap flag as sent by the device.
        double_tap: bool,
        /// Timestamp carried by the update; this module does not define its unit.
        timestamp: u64,
    },
    /// Assigns a tile to render (first field) for a frame (second field); the worker uses the second value as the frame number.
    AssignTile(TileSpec, u32),
    /// Submits the result of rendering a tile.
    SubmitResult(RenderResult),
    /// Announces that the sender is disconnecting.
    Disconnect {
        /// Free-form reason string for the disconnect.
        reason: String,
    },
}

/// Namespace type for encoding and decoding [`SyncMessage`] values; it has no fields.
pub struct SyncProtocol;
impl SyncProtocol {
    /// Decodes a [`SyncMessage`] from JSON bytes using `serde_json::from_slice`.
    pub fn decode(data: &[u8]) -> Result<SyncMessage, serde_json::Error> {
        serde_json::from_slice(data)
    }
    /// Encodes a [`SyncMessage`] as JSON bytes using `serde_json::to_vec`.
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
