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

//! Binary protocol for render farm job distribution.
//!
//! Uses TCP with length-prefixed bincode serialization.

use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener as TokioTcpListener, TcpStream as TokioTcpStream};
use uuid::Uuid;

use super::scene_crdt::CrdtOperation;

/// Maximum message size (16 MB)
const MAX_MESSAGE_SIZE: u32 = 16 * 1024 * 1024;

/// Job protocol messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RenderMessage {
    // ══════════════════════════════════════════════════════════════════════
    // MASTER → WORKER MESSAGES
    // ══════════════════════════════════════════════════════════════════════
    /// Assign a render job to worker
    JobAssign {
        job_id: Uuid,
        frame: u32,
        tile: TileSpec,
        scene_diff: Vec<CrdtOperation>,
        priority: u8,
    },

    /// Heartbeat ping
    Heartbeat { timestamp: u64 },

    /// Cancel a pending job
    CancelJob { job_id: Uuid },

    /// Request worker capabilities
    RequestCapabilities,

    /// Shutdown worker
    Shutdown,

    // ══════════════════════════════════════════════════════════════════════
    // WORKER → MASTER MESSAGES
    // ══════════════════════════════════════════════════════════════════════
    /// Register worker with master
    RegisterWorker {
        worker_id: Uuid,
        capabilities: WorkerCapabilities,
    },

    /// Job completed successfully
    JobComplete { job_id: Uuid, result: JobResult },

    /// Job failed with error
    JobError { job_id: Uuid, error: String },

    /// Heartbeat acknowledgment
    HeartbeatAck { timestamp: u64 },

    /// Report worker capabilities
    ReportCapabilities {
        worker_id: Uuid,
        capabilities: WorkerCapabilities,
    },
}

/// Worker capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerCapabilities {
    /// GPU name (e.g., "NVIDIA GeForce RTX 3050")
    pub gpu_name: String,
    /// VRAM in megabytes
    pub vram_mb: u32,
    /// CPU core count
    pub cpu_cores: u8,
    /// Maximum tile size (width or height)
    pub max_tile_size: u32,
    /// NAT3D version
    pub version: String,
}

impl WorkerCapabilities {
    pub fn new(gpu_name: String, vram_mb: u32, cpu_cores: u8, max_tile_size: u32) -> Self {
        Self {
            gpu_name,
            vram_mb,
            cpu_cores,
            max_tile_size,
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

/// Tile specification for rendering.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct TileSpec {
    /// Tile X offset in pixels
    pub x: u32,
    /// Tile Y offset in pixels
    pub y: u32,
    /// Tile width in pixels
    pub width: u32,
    /// Tile height in pixels
    pub height: u32,
}

impl TileSpec {
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Total number of pixels in this tile
    pub fn pixel_count(&self) -> usize {
        (self.width * self.height) as usize
    }

    /// Expected byte size (RGBA8)
    pub fn byte_size(&self) -> usize {
        self.pixel_count() * 4
    }
}

/// Job result from worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobResult {
    /// Frame number
    pub frame: u32,
    /// Tile specification
    pub tile: TileSpec,
    /// Pixel data (RGBA8, row-major)
    pub pixels: Vec<u8>,
    /// Render time in milliseconds
    pub render_time_ms: u64,
}

impl JobResult {
    pub fn new(frame: u32, tile: TileSpec, pixels: Vec<u8>, render_time_ms: u64) -> Self {
        Self {
            frame,
            tile,
            pixels,
            render_time_ms,
        }
    }

    /// Validate pixel data size matches tile size
    pub fn validate(&self) -> Result<(), String> {
        let expected = self.tile.byte_size();
        let actual = self.pixels.len();
        if expected != actual {
            return Err(format!(
                "Pixel data size mismatch: expected {} bytes, got {}",
                expected, actual
            ));
        }
        Ok(())
    }
}

// ══════════════════════════════════════════════════════════════════════════
// SYNCHRONOUS PROTOCOL (for simple use cases)
// ══════════════════════════════════════════════════════════════════════════

/// Send a message over TCP (blocking).
pub fn send_message_sync(stream: &mut TcpStream, message: &RenderMessage) -> io::Result<()> {
    // Serialize message
    let data =
        bincode::serialize(message).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    // Check size limit
    if data.len() > MAX_MESSAGE_SIZE as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Message too large: {} bytes", data.len()),
        ));
    }

    // Send length prefix (u32 big-endian)
    let length = data.len() as u32;
    stream.write_all(&length.to_be_bytes())?;

    // Send message data
    stream.write_all(&data)?;
    stream.flush()?;

    Ok(())
}

/// Receive a message from TCP (blocking).
pub fn recv_message_sync(stream: &mut TcpStream) -> io::Result<RenderMessage> {
    // Read length prefix (u32 big-endian)
    let mut length_bytes = [0u8; 4];
    stream.read_exact(&mut length_bytes)?;
    let length = u32::from_be_bytes(length_bytes);

    // Check size limit
    if length > MAX_MESSAGE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Message too large: {} bytes", length),
        ));
    }

    // Read message data
    let mut data = vec![0u8; length as usize];
    stream.read_exact(&mut data)?;

    // Deserialize message
    let message =
        bincode::deserialize(&data).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    Ok(message)
}

// ══════════════════════════════════════════════════════════════════════════
// ASYNCHRONOUS PROTOCOL (for tokio)
// ══════════════════════════════════════════════════════════════════════════

/// Send a message over TCP (async).
pub async fn send_message(stream: &mut TokioTcpStream, message: &RenderMessage) -> io::Result<()> {
    // Serialize message
    let data =
        bincode::serialize(message).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    // Check size limit
    if data.len() > MAX_MESSAGE_SIZE as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Message too large: {} bytes", data.len()),
        ));
    }

    // Send length prefix (u32 big-endian)
    let length = data.len() as u32;
    stream.write_all(&length.to_be_bytes()).await?;

    // Send message data
    stream.write_all(&data).await?;
    stream.flush().await?;

    Ok(())
}

/// Receive a message from TCP (async).
pub async fn recv_message(stream: &mut TokioTcpStream) -> io::Result<RenderMessage> {
    // Read length prefix (u32 big-endian)
    let mut length_bytes = [0u8; 4];
    stream.read_exact(&mut length_bytes).await?;
    let length = u32::from_be_bytes(length_bytes);

    // Check size limit
    if length > MAX_MESSAGE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Message too large: {} bytes", length),
        ));
    }

    // Read message data
    let mut data = vec![0u8; length as usize];
    stream.read_exact(&mut data).await?;

    // Deserialize message
    let message =
        bincode::deserialize(&data).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    Ok(message)
}

// ══════════════════════════════════════════════════════════════════════════
// CONNECTION HELPERS
// ══════════════════════════════════════════════════════════════════════════

/// TCP server listener wrapper.
pub struct RenderServer {
    listener: TokioTcpListener,
}

impl RenderServer {
    /// Bind server to address.
    pub async fn bind(addr: SocketAddr) -> io::Result<Self> {
        let listener = TokioTcpListener::bind(addr).await?;
        tracing::info!("Render server listening on {}", addr);
        Ok(Self { listener })
    }

    /// Accept incoming connection.
    pub async fn accept(&self) -> io::Result<(TokioTcpStream, SocketAddr)> {
        self.listener.accept().await
    }

    /// Get local address.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }
}

/// TCP client connection wrapper.
pub struct RenderClient {
    stream: TokioTcpStream,
}

impl RenderClient {
    /// Create from existing stream.
    pub fn from_stream(stream: TokioTcpStream) -> Self {
        Self { stream }
    }

    /// Connect to server.
    pub async fn connect(addr: SocketAddr) -> io::Result<Self> {
        let stream = TokioTcpStream::connect(addr).await?;
        tracing::info!("Connected to render server at {}", addr);
        Ok(Self { stream })
    }

    /// Send message.
    pub async fn send(&mut self, message: &RenderMessage) -> io::Result<()> {
        send_message(&mut self.stream, message).await
    }

    /// Receive message.
    pub async fn recv(&mut self) -> io::Result<RenderMessage> {
        recv_message(&mut self.stream).await
    }

    /// Get peer address.
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.stream.peer_addr()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tile_spec_creation() {
        let tile = TileSpec::new(0, 0, 512, 512);
        assert_eq!(tile.x, 0);
        assert_eq!(tile.y, 0);
        assert_eq!(tile.width, 512);
        assert_eq!(tile.height, 512);
    }

    #[test]
    fn test_tile_spec_pixel_count() {
        let tile = TileSpec::new(0, 0, 512, 512);
        assert_eq!(tile.pixel_count(), 512 * 512);
        assert_eq!(tile.byte_size(), 512 * 512 * 4);
    }

    #[test]
    fn test_worker_capabilities() {
        let caps = WorkerCapabilities::new("NVIDIA RTX 3050".to_string(), 4096, 8, 512);

        assert_eq!(caps.gpu_name, "NVIDIA RTX 3050");
        assert_eq!(caps.vram_mb, 4096);
        assert_eq!(caps.cpu_cores, 8);
        assert_eq!(caps.max_tile_size, 512);
    }

    #[test]
    fn test_job_result_validation() {
        let tile = TileSpec::new(0, 0, 64, 64);
        let pixels = vec![0u8; 64 * 64 * 4]; // Correct size
        let result = JobResult::new(0, tile, pixels, 100);
        assert!(result.validate().is_ok());
    }

    #[test]
    fn test_job_result_validation_wrong_size() {
        let tile = TileSpec::new(0, 0, 64, 64);
        let pixels = vec![0u8; 100]; // Wrong size
        let result = JobResult::new(0, tile, pixels, 100);
        assert!(result.validate().is_err());
    }

    #[test]
    fn test_message_serialization() {
        let message = RenderMessage::Heartbeat { timestamp: 12345 };
        let data = bincode::serialize(&message).unwrap();
        let deserialized: RenderMessage = bincode::deserialize(&data).unwrap();

        match deserialized {
            RenderMessage::Heartbeat { timestamp } => assert_eq!(timestamp, 12345),
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_job_assign_serialization() {
        let tile = TileSpec::new(0, 0, 512, 512);
        let message = RenderMessage::JobAssign {
            job_id: Uuid::new_v4(),
            frame: 42,
            tile,
            scene_diff: vec![],
            priority: 100,
        };

        let data = bincode::serialize(&message).unwrap();
        let deserialized: RenderMessage = bincode::deserialize(&data).unwrap();

        match deserialized {
            RenderMessage::JobAssign { frame, tile: t, .. } => {
                assert_eq!(frame, 42);
                assert_eq!(t, tile);
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[tokio::test]
    async fn test_async_send_recv() {
        // Start server
        let server = RenderServer::bind("127.0.0.1:0".parse().unwrap())
            .await
            .unwrap();
        let addr = server.local_addr().unwrap();

        // Spawn server task
        tokio::spawn(async move {
            let (mut stream, _) = server.accept().await.unwrap();
            let msg = recv_message(&mut stream).await.unwrap();

            // Echo back
            send_message(&mut stream, &msg).await.unwrap();
        });

        // Give server time to start
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Connect client
        let mut client = RenderClient::connect(addr).await.unwrap();

        // Send message
        let sent = RenderMessage::Heartbeat { timestamp: 99999 };
        client.send(&sent).await.unwrap();

        // Receive echo
        let received = client.recv().await.unwrap();

        match received {
            RenderMessage::Heartbeat { timestamp } => assert_eq!(timestamp, 99999),
            _ => panic!("Wrong message type"),
        }
    }

    #[tokio::test]
    async fn test_large_message() {
        // Start server
        let server = RenderServer::bind("127.0.0.1:0".parse().unwrap())
            .await
            .unwrap();
        let addr = server.local_addr().unwrap();

        // Spawn server task
        tokio::spawn(async move {
            let (mut stream, _) = server.accept().await.unwrap();
            let msg = recv_message(&mut stream).await.unwrap();
            send_message(&mut stream, &msg).await.unwrap();
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Connect client
        let mut client = RenderClient::connect(addr).await.unwrap();

        // Send large message (1 MB tile)
        let tile = TileSpec::new(0, 0, 512, 512);
        let pixels = vec![128u8; 512 * 512 * 4]; // 1 MB
        let result = JobResult::new(0, tile, pixels.clone(), 1000);
        let sent = RenderMessage::JobComplete {
            job_id: Uuid::new_v4(),
            result,
        };

        client.send(&sent).await.unwrap();
        let received = client.recv().await.unwrap();

        match received {
            RenderMessage::JobComplete { result, .. } => {
                assert_eq!(result.pixels.len(), pixels.len());
                assert_eq!(result.render_time_ms, 1000);
            }
            _ => panic!("Wrong message type"),
        }
    }
}
