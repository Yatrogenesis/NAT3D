// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Francisco Molina-Burgos, Avermex Research Division

//! TCP-based stylus input receiver.
//!
//! Enables using an iPad (or any device) as a remote stylus input source
//! for desktop NAT3D. The remote device runs a companion app that sends
//! stylus events over TCP.
//!
//! # Protocol
//!
//! Binary protocol over TCP (port 9001 default):
//!
//! ```text
//! Header (1 byte):
//!   0x01 = Down
//!   0x02 = Move
//!   0x03 = Up
//!   0x04 = Hover
//!   0x05 = ProximityOut
//!
//! Payload (for 0x01-0x04):
//!   x: f32 (4 bytes, little-endian)
//!   y: f32 (4 bytes)
//!   pressure: f32 (4 bytes)
//!   altitude: f32 (4 bytes)
//!   azimuth: f32 (4 bytes)
//!   timestamp_ms: u64 (8 bytes)
//!   flags: u8 (1 byte) - bit 0: barrel_button, bit 1: eraser
//!
//! Total: 1 + 29 = 30 bytes per event
//! ```
//!
//! # Usage
//!
//! ```ignore
//! use nat3d_sync::input::tcp_stylus::TcpStylusReceiver;
//! use nat3d_core::stylus::StylusProvider;
//!
//! let mut receiver = TcpStylusReceiver::bind("0.0.0.0:9001").unwrap();
//!
//! // In event loop
//! while let Some(event) = receiver.poll() {
//!     handle_stylus_event(event);
//! }
//! ```

use nat3d_core::stylus::{StylusCapabilities, StylusEvent, StylusInput, StylusProvider};
use std::collections::VecDeque;
use std::io::{self, Read};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Default port for TCP stylus receiver.
pub const DEFAULT_STYLUS_PORT: u16 = 9001;

/// Packet header bytes.
const HEADER_DOWN: u8 = 0x01;
const HEADER_MOVE: u8 = 0x02;
const HEADER_UP: u8 = 0x03;
const HEADER_HOVER: u8 = 0x04;
const HEADER_PROXIMITY_OUT: u8 = 0x05;

/// Payload size (without header).
const PAYLOAD_SIZE: usize = 29;

/// TCP-based stylus input receiver.
///
/// Listens for stylus events from remote devices over TCP.
/// Implements `StylusProvider` for seamless integration.
pub struct TcpStylusReceiver {
    events: Arc<Mutex<VecDeque<StylusEvent>>>,
    connected: Arc<Mutex<bool>>,
    device_name: Arc<Mutex<String>>,
    _listener_thread: Option<thread::JoinHandle<()>>,
}

impl TcpStylusReceiver {
    /// Create a new TCP stylus receiver bound to the given address.
    ///
    /// # Arguments
    /// * `addr` - Address to bind to (e.g., "0.0.0.0:9001")
    ///
    /// # Returns
    /// Receiver instance, or IO error if binding fails.
    pub fn bind(addr: &str) -> io::Result<Self> {
        let listener = TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;

        let events = Arc::new(Mutex::new(VecDeque::with_capacity(256)));
        let connected = Arc::new(Mutex::new(false));
        let device_name = Arc::new(Mutex::new("TCP Stylus".to_string()));

        let events_clone = events.clone();
        let connected_clone = connected.clone();
        let device_name_clone = device_name.clone();

        let handle = thread::spawn(move || {
            Self::listener_loop(listener, events_clone, connected_clone, device_name_clone);
        });

        Ok(Self {
            events,
            connected,
            device_name,
            _listener_thread: Some(handle),
        })
    }

    /// Bind to the default port (9001).
    pub fn bind_default() -> io::Result<Self> {
        Self::bind(&format!("0.0.0.0:{}", DEFAULT_STYLUS_PORT))
    }

    fn listener_loop(
        listener: TcpListener,
        events: Arc<Mutex<VecDeque<StylusEvent>>>,
        connected: Arc<Mutex<bool>>,
        device_name: Arc<Mutex<String>>,
    ) {
        loop {
            match listener.accept() {
                Ok((stream, addr)) => {
                    if let Ok(mut name) = device_name.lock() {
                        *name = format!("iPad @ {}", addr);
                    }
                    if let Ok(mut c) = connected.lock() {
                        *c = true;
                    }

                    Self::handle_client(stream, events.clone());

                    if let Ok(mut c) = connected.lock() {
                        *c = false;
                    }
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(50));
                }
                Err(_) => {
                    thread::sleep(Duration::from_millis(100));
                }
            }
        }
    }

    fn handle_client(mut stream: TcpStream, events: Arc<Mutex<VecDeque<StylusEvent>>>) {
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));

        let mut header = [0u8; 1];
        let mut payload = [0u8; PAYLOAD_SIZE];

        loop {
            match stream.read_exact(&mut header) {
                Ok(_) => {
                    let event = match header[0] {
                        HEADER_PROXIMITY_OUT => Some(StylusEvent::ProximityOut),
                        HEADER_DOWN | HEADER_MOVE | HEADER_UP | HEADER_HOVER => {
                            if stream.read_exact(&mut payload).is_ok() {
                                Some(Self::parse_payload(header[0], &payload))
                            } else {
                                break;
                            }
                        }
                        _ => None,
                    };

                    if let Some(e) = event {
                        if let Ok(mut ev) = events.lock() {
                            if ev.len() >= 256 {
                                ev.pop_front();
                            }
                            ev.push_back(e);
                        }
                    }
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(1));
                }
                Err(_) => break,
            }
        }
    }

    fn parse_payload(header: u8, data: &[u8]) -> StylusEvent {
        let x = f32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let y = f32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let pressure = f32::from_le_bytes([data[8], data[9], data[10], data[11]]);
        let altitude = f32::from_le_bytes([data[12], data[13], data[14], data[15]]);
        let azimuth = f32::from_le_bytes([data[16], data[17], data[18], data[19]]);
        let timestamp_ms = u64::from_le_bytes([
            data[20], data[21], data[22], data[23], data[24], data[25], data[26], data[27],
        ]);
        let flags = data[28];

        let input = StylusInput::new(x, y, pressure)
            .with_tilt(altitude, azimuth)
            .with_timestamp(timestamp_ms)
            .with_barrel_button(flags & 0x01 != 0)
            .with_eraser(flags & 0x02 != 0);

        match header {
            HEADER_DOWN => StylusEvent::Down(input),
            HEADER_MOVE => StylusEvent::Move(input),
            HEADER_UP => StylusEvent::Up(input),
            HEADER_HOVER => StylusEvent::Hover(input),
            _ => StylusEvent::ProximityOut,
        }
    }
}

impl StylusProvider for TcpStylusReceiver {
    fn poll(&mut self) -> Option<StylusEvent> {
        if let Ok(mut ev) = self.events.lock() {
            ev.pop_front()
        } else {
            None
        }
    }

    fn capabilities(&self) -> StylusCapabilities {
        StylusCapabilities::apple_pencil()
    }

    fn device_name(&self) -> &str {
        "TCP Stylus"
    }

    fn is_connected(&self) -> bool {
        if let Ok(c) = self.connected.lock() {
            *c
        } else {
            false
        }
    }
}

/// Serialize a stylus event for transmission.
///
/// Use this on the sender side (iPad app) to encode events.
pub fn serialize_event(event: &StylusEvent) -> Vec<u8> {
    match event {
        StylusEvent::ProximityOut => vec![HEADER_PROXIMITY_OUT],
        StylusEvent::Down(input) => serialize_input(HEADER_DOWN, input),
        StylusEvent::Move(input) => serialize_input(HEADER_MOVE, input),
        StylusEvent::Up(input) => serialize_input(HEADER_UP, input),
        StylusEvent::Hover(input) => serialize_input(HEADER_HOVER, input),
    }
}

fn serialize_input(header: u8, input: &StylusInput) -> Vec<u8> {
    let mut buf = Vec::with_capacity(30);
    buf.push(header);
    buf.extend_from_slice(&input.x.to_le_bytes());
    buf.extend_from_slice(&input.y.to_le_bytes());
    buf.extend_from_slice(&input.pressure.to_le_bytes());
    buf.extend_from_slice(&input.tilt_altitude.to_le_bytes());
    buf.extend_from_slice(&input.tilt_azimuth.to_le_bytes());
    buf.extend_from_slice(&input.timestamp_ms.to_le_bytes());

    let mut flags = 0u8;
    if input.barrel_button {
        flags |= 0x01;
    }
    if input.eraser {
        flags |= 0x02;
    }
    buf.push(flags);

    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_deserialize_roundtrip() {
        let input = StylusInput::new(0.5, 0.75, 0.9)
            .with_tilt(1.2, 0.8)
            .with_timestamp(12345)
            .with_barrel_button(true);

        let serialized = serialize_event(&StylusEvent::Down(input));
        assert_eq!(serialized.len(), 30);
        assert_eq!(serialized[0], HEADER_DOWN);

        let parsed = TcpStylusReceiver::parse_payload(HEADER_DOWN, &serialized[1..]);
        if let StylusEvent::Down(parsed_input) = parsed {
            assert!((parsed_input.x - 0.5).abs() < 0.001);
            assert!((parsed_input.y - 0.75).abs() < 0.001);
            assert!((parsed_input.pressure - 0.9).abs() < 0.001);
            assert!((parsed_input.tilt_altitude - 1.2).abs() < 0.001);
            assert!(parsed_input.barrel_button);
        } else {
            panic!("Expected Down event");
        }
    }

    #[test]
    fn test_proximity_out_serialization() {
        let serialized = serialize_event(&StylusEvent::ProximityOut);
        assert_eq!(serialized.len(), 1);
        assert_eq!(serialized[0], HEADER_PROXIMITY_OUT);
    }

    #[test]
    fn test_all_event_types() {
        let input = StylusInput::new(0.1, 0.2, 0.3);

        for (event, expected_header) in [
            (StylusEvent::Down(input), HEADER_DOWN),
            (StylusEvent::Move(input), HEADER_MOVE),
            (StylusEvent::Up(input), HEADER_UP),
            (StylusEvent::Hover(input), HEADER_HOVER),
        ] {
            let serialized = serialize_event(&event);
            assert_eq!(serialized[0], expected_header);
        }
    }

    #[test]
    fn test_flags_encoding() {
        let input_barrel = StylusInput::new(0.0, 0.0, 0.0).with_barrel_button(true);
        let input_eraser = StylusInput::new(0.0, 0.0, 0.0).with_eraser(true);
        let input_both = StylusInput::new(0.0, 0.0, 0.0)
            .with_barrel_button(true)
            .with_eraser(true);

        let ser_barrel = serialize_event(&StylusEvent::Down(input_barrel));
        let ser_eraser = serialize_event(&StylusEvent::Down(input_eraser));
        let ser_both = serialize_event(&StylusEvent::Down(input_both));

        assert_eq!(ser_barrel[29], 0x01);
        assert_eq!(ser_eraser[29], 0x02);
        assert_eq!(ser_both[29], 0x03);
    }
}
