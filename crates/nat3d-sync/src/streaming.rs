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

//! Viewport streaming for remote viewing.
//!
//! Streams viewport frames to connected devices with compression.

use std::sync::{Arc, Mutex};

/// Stream configuration.
#[derive(Debug, Clone)]
pub struct StreamConfig {
    pub resolution: (u32, u32),
    pub fps: u32,
    pub quality: u8,
    pub codec: StreamCodec,
}

impl StreamConfig {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            resolution: (width, height),
            fps: 30,
            quality: 80,
            codec: StreamCodec::Rle,
        }
    }

    pub fn with_fps(mut self, fps: u32) -> Self {
        self.fps = fps;
        self
    }

    pub fn with_quality(mut self, quality: u8) -> Self {
        self.quality = quality.clamp(1, 100);
        self
    }

    pub fn with_codec(mut self, codec: StreamCodec) -> Self {
        self.codec = codec;
        self
    }
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self::new(1920, 1080)
    }
}

/// Stream codec types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamCodec {
    Raw,
    Rle,
    Delta,
}

/// Viewport streamer.
pub struct ViewportStreamer {
    config: StreamConfig,
    running: Arc<Mutex<bool>>,
    frame_count: Arc<Mutex<u64>>,
    last_frame: Arc<Mutex<Option<Vec<u8>>>>,
}

impl ViewportStreamer {
    /// Create a new viewport streamer.
    pub fn new(config: StreamConfig) -> Self {
        Self {
            config,
            running: Arc::new(Mutex::new(false)),
            frame_count: Arc::new(Mutex::new(0)),
            last_frame: Arc::new(Mutex::new(None)),
        }
    }

    /// Start streaming.
    pub fn start_stream(&self) -> anyhow::Result<()> {
        *self.running.lock().unwrap() = true;
        tracing::info!(
            "Started streaming at {}x{} @ {} fps",
            self.config.resolution.0,
            self.config.resolution.1,
            self.config.fps
        );
        Ok(())
    }

    /// Stop streaming.
    pub fn stop_stream(&self) {
        *self.running.lock().unwrap() = false;
        tracing::info!("Stopped streaming");
    }

    /// Send a frame.
    pub fn send_frame(&self, pixels: &[u8]) -> anyhow::Result<Vec<u8>> {
        if !*self.running.lock().unwrap() {
            return Err(anyhow::anyhow!("Streamer is not running"));
        }

        let compressed = self.compress_frame(pixels)?;

        // Store frame for delta encoding
        *self.last_frame.lock().unwrap() = Some(pixels.to_vec());

        // Increment frame count
        *self.frame_count.lock().unwrap() += 1;

        Ok(compressed)
    }

    /// Set stream quality (1-100).
    pub fn set_quality(&mut self, quality: u8) {
        self.config.quality = quality.clamp(1, 100);
        tracing::info!("Stream quality set to {}", self.config.quality);
    }

    /// Get current frame count.
    pub fn frame_count(&self) -> u64 {
        *self.frame_count.lock().unwrap()
    }

    /// Check if streaming is active.
    pub fn is_running(&self) -> bool {
        *self.running.lock().unwrap()
    }

    fn compress_frame(&self, pixels: &[u8]) -> anyhow::Result<Vec<u8>> {
        match self.config.codec {
            StreamCodec::Raw => Ok(pixels.to_vec()),
            StreamCodec::Rle => self.rle_encode(pixels),
            StreamCodec::Delta => self.delta_encode(pixels),
        }
    }

    fn rle_encode(&self, data: &[u8]) -> anyhow::Result<Vec<u8>> {
        let mut encoded = Vec::new();

        let mut i = 0;
        while i < data.len() {
            let byte = data[i];
            let mut count = 1u8;

            while (i + count as usize) < data.len()
                && data[i + count as usize] == byte
                && count < 255
            {
                count += 1;
            }

            encoded.push(count);
            encoded.push(byte);
            i += count as usize;
        }

        Ok(encoded)
    }

    fn delta_encode(&self, data: &[u8]) -> anyhow::Result<Vec<u8>> {
        let last_frame = self.last_frame.lock().unwrap();

        if let Some(ref last) = *last_frame {
            if last.len() == data.len() {
                // Encode only differences
                let mut delta = Vec::new();

                for (i, (&new, &old)) in data.iter().zip(last.iter()).enumerate() {
                    if new != old {
                        delta.extend_from_slice(&(i as u32).to_be_bytes());
                        delta.push(new);
                    }
                }

                return Ok(delta);
            }
        }

        // Fall back to RLE if no previous frame
        self.rle_encode(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_config_creation() {
        let config = StreamConfig::new(1920, 1080);
        assert_eq!(config.resolution, (1920, 1080));
        assert_eq!(config.fps, 30);
        assert_eq!(config.quality, 80);
    }

    #[test]
    fn test_stream_config_builder() {
        let config = StreamConfig::new(1920, 1080)
            .with_fps(60)
            .with_quality(90)
            .with_codec(StreamCodec::Delta);

        assert_eq!(config.fps, 60);
        assert_eq!(config.quality, 90);
        assert_eq!(config.codec, StreamCodec::Delta);
    }

    #[test]
    fn test_viewport_streamer_creation() {
        let config = StreamConfig::default();
        let streamer = ViewportStreamer::new(config);
        assert!(!streamer.is_running());
    }

    #[test]
    fn test_start_stop_stream() {
        let config = StreamConfig::default();
        let streamer = ViewportStreamer::new(config);

        streamer.start_stream().unwrap();
        assert!(streamer.is_running());

        streamer.stop_stream();
        assert!(!streamer.is_running());
    }

    #[test]
    fn test_send_frame() {
        let config = StreamConfig::default();
        let streamer = ViewportStreamer::new(config);

        streamer.start_stream().unwrap();

        let pixels = vec![255u8; 1920 * 1080 * 4];
        let result = streamer.send_frame(&pixels);

        assert!(result.is_ok());
        assert_eq!(streamer.frame_count(), 1);
    }

    #[test]
    fn test_rle_encoding() {
        let config = StreamConfig::default().with_codec(StreamCodec::Rle);
        let streamer = ViewportStreamer::new(config);

        streamer.start_stream().unwrap();

        // Simple pattern: 100 red pixels
        let pixels = vec![255u8; 100];
        let compressed = streamer.send_frame(&pixels).unwrap();

        // RLE should be much smaller
        assert!(compressed.len() < pixels.len());
    }

    #[test]
    fn test_frame_count() {
        let config = StreamConfig::default();
        let streamer = ViewportStreamer::new(config);

        streamer.start_stream().unwrap();

        let pixels = vec![0u8; 100];

        for _ in 0..5 {
            streamer.send_frame(&pixels).unwrap();
        }

        assert_eq!(streamer.frame_count(), 5);
    }
}
