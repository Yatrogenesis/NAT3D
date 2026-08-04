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

//! Image sequence handler for rendering output.

use std::path::{Path, PathBuf};
use thiserror::Error;

/// Sequence errors.
#[derive(Error, Debug)]
pub enum SequenceError {
    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// Invalid frame number.
    #[error("Invalid frame number: {0}")]
    InvalidFrame(i32),
}

/// Result type for sequence operations.
pub type SequenceResult<T> = Result<T, SequenceError>;

/// Image sequence configuration.
#[derive(Debug, Clone)]
pub struct ImageSequence {
    /// Base path (e.g., "/path/to/render_").
    pub base_path: PathBuf,
    /// Frame range (start, end).
    pub frame_range: (i32, i32),
    /// Format extension (png, exr, etc.).
    pub format: String,
    /// Frame number padding (e.g., 4 for "0001").
    pub padding: usize,
}

impl ImageSequence {
    /// Create a new image sequence.
    pub fn new<P: AsRef<Path>>(base_path: P, start_frame: i32, end_frame: i32) -> Self {
        Self {
            base_path: base_path.as_ref().to_path_buf(),
            frame_range: (start_frame, end_frame),
            format: "png".to_string(),
            padding: 4,
        }
    }

    /// Set format extension.
    pub fn with_format(mut self, format: &str) -> Self {
        self.format = format.to_string();
        self
    }

    /// Set padding.
    pub fn with_padding(mut self, padding: usize) -> Self {
        self.padding = padding;
        self
    }

    /// Get path for a specific frame.
    pub fn get_frame_path(&self, frame_number: i32) -> SequenceResult<PathBuf> {
        if frame_number < self.frame_range.0 || frame_number > self.frame_range.1 {
            return Err(SequenceError::InvalidFrame(frame_number));
        }

        let frame_str = format!("{:0width$}", frame_number, width = self.padding);
        let filename = format!(
            "{}{}.{}",
            self.base_path.to_string_lossy(),
            frame_str,
            self.format
        );

        Ok(PathBuf::from(filename))
    }

    /// Save a frame.
    pub fn save_frame(&self, frame_number: i32, image_data: &[u8]) -> SequenceResult<()> {
        let path = self.get_frame_path(frame_number)?;
        std::fs::write(path, image_data)?;
        Ok(())
    }

    /// Get all frame paths in the sequence.
    pub fn get_all_frame_paths(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        for frame in self.frame_range.0..=self.frame_range.1 {
            if let Ok(path) = self.get_frame_path(frame) {
                paths.push(path);
            }
        }
        paths
    }

    /// Render sequence with callback.
    ///
    /// The callback receives the frame number and should return the image data.
    pub fn render_sequence<F>(&self, mut callback: F) -> SequenceResult<()>
    where
        F: FnMut(i32) -> Vec<u8>,
    {
        for frame in self.frame_range.0..=self.frame_range.1 {
            let image_data = callback(frame);
            self.save_frame(frame, &image_data)?;
        }
        Ok(())
    }

    /// Check if a frame exists.
    pub fn frame_exists(&self, frame_number: i32) -> bool {
        if let Ok(path) = self.get_frame_path(frame_number) {
            path.exists()
        } else {
            false
        }
    }

    /// Get frame count.
    pub fn frame_count(&self) -> usize {
        (self.frame_range.1 - self.frame_range.0 + 1).max(0) as usize
    }

    /// Get start frame.
    pub fn start_frame(&self) -> i32 {
        self.frame_range.0
    }

    /// Get end frame.
    pub fn end_frame(&self) -> i32 {
        self.frame_range.1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sequence_creation() {
        let seq = ImageSequence::new("/tmp/render_", 1, 10)
            .with_format("exr")
            .with_padding(5);

        assert_eq!(seq.frame_count(), 10);
        assert_eq!(seq.start_frame(), 1);
        assert_eq!(seq.end_frame(), 10);
        assert_eq!(seq.format, "exr");
        assert_eq!(seq.padding, 5);
    }

    #[test]
    fn test_frame_path() {
        let seq = ImageSequence::new("/tmp/render_", 1, 10);

        let path = seq.get_frame_path(5).unwrap();
        assert_eq!(path.to_string_lossy(), "/tmp/render_0005.png");
    }

    #[test]
    fn test_invalid_frame() {
        let seq = ImageSequence::new("/tmp/render_", 1, 10);

        let result = seq.get_frame_path(0);
        assert!(result.is_err());

        let result = seq.get_frame_path(11);
        assert!(result.is_err());
    }

    #[test]
    fn test_all_frame_paths() {
        let seq = ImageSequence::new("/tmp/render_", 1, 3);

        let paths = seq.get_all_frame_paths();
        assert_eq!(paths.len(), 3);
        assert_eq!(paths[0].to_string_lossy(), "/tmp/render_0001.png");
        assert_eq!(paths[2].to_string_lossy(), "/tmp/render_0003.png");
    }
}
