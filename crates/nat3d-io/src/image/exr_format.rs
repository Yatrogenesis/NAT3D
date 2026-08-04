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

//! OpenEXR HDR image format handler.
//!
//! Uses the `exr` crate for high dynamic range image loading and saving.
//! Supports loading/saving multi-channel EXR files with f32 precision.

use std::path::Path;
use thiserror::Error;

/// EXR errors.
#[derive(Error, Debug)]
pub enum ExrError {
    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// EXR error.
    #[error("EXR error: {0}")]
    Exr(String),
}

/// Result type for EXR operations.
pub type ExrResult<T> = Result<T, ExrError>;

/// EXR channel data.
#[derive(Debug, Clone)]
pub struct ExrChannel {
    /// Channel name (R, G, B, A, Z, etc.).
    pub name: String,
    /// Channel data (f32 values).
    pub data: Vec<f32>,
}

/// EXR image data.
#[derive(Debug, Clone)]
pub struct ExrImage {
    /// Image width.
    pub width: u32,
    /// Image height.
    pub height: u32,
    /// Channels.
    pub channels: Vec<ExrChannel>,
}

impl ExrImage {
    /// Load EXR from file using the high-level RGBA API.
    pub fn load<P: AsRef<Path>>(path: P) -> ExrResult<Self> {
        use exr::prelude::*;

        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let img_width = Arc::new(AtomicUsize::new(0));
        let w1 = img_width.clone();
        let w2 = img_width.clone();

        let image = read_first_rgba_layer_from_file(
            path,
            move |resolution, _| {
                w1.store(resolution.width(), Ordering::Relaxed);
                vec![(0.0f32, 0.0f32, 0.0f32, 1.0f32); resolution.width() * resolution.height()]
            },
            move |pixels, pos, (r, g, b, a): (f32, f32, f32, f32)| {
                let w = w2.load(Ordering::Relaxed);
                if w > 0 {
                    let idx = pos.y() * w + pos.x();
                    if idx < pixels.len() {
                        pixels[idx] = (r, g, b, a);
                    }
                }
            },
        )
        .map_err(|e| ExrError::Exr(format!("{}", e)))?;

        let width = image.layer_data.size.width() as u32;
        let height = image.layer_data.size.height() as u32;
        let pixels = &image.layer_data.channel_data.pixels;

        let mut r_data = Vec::with_capacity(pixels.len());
        let mut g_data = Vec::with_capacity(pixels.len());
        let mut b_data = Vec::with_capacity(pixels.len());
        let mut a_data = Vec::with_capacity(pixels.len());

        for &(r, g, b, a) in pixels {
            r_data.push(r);
            g_data.push(g);
            b_data.push(b);
            a_data.push(a);
        }

        Ok(Self {
            width,
            height,
            channels: vec![
                ExrChannel {
                    name: "R".to_string(),
                    data: r_data,
                },
                ExrChannel {
                    name: "G".to_string(),
                    data: g_data,
                },
                ExrChannel {
                    name: "B".to_string(),
                    data: b_data,
                },
                ExrChannel {
                    name: "A".to_string(),
                    data: a_data,
                },
            ],
        })
    }

    /// Save EXR to file using the high-level RGBA API.
    pub fn save<P: AsRef<Path>>(&self, path: P) -> ExrResult<()> {
        use exr::prelude::*;

        let r = self.get_channel("R").map(|c| &c.data[..]);
        let g = self.get_channel("G").map(|c| &c.data[..]);
        let b = self.get_channel("B").map(|c| &c.data[..]);
        let a = self.get_channel("A").map(|c| &c.data[..]);

        let w = self.width as usize;
        let h = self.height as usize;

        write_rgba_file(path, w, h, |x, y| {
            let idx = y * w + x;
            let rv = r.map(|d| d.get(idx).copied().unwrap_or(0.0)).unwrap_or(0.0);
            let gv = g.map(|d| d.get(idx).copied().unwrap_or(0.0)).unwrap_or(0.0);
            let bv = b.map(|d| d.get(idx).copied().unwrap_or(0.0)).unwrap_or(0.0);
            let av = a.map(|d| d.get(idx).copied().unwrap_or(1.0)).unwrap_or(1.0);
            (rv, gv, bv, av)
        })
        .map_err(|e| ExrError::Exr(format!("{}", e)))?;

        Ok(())
    }

    /// Get channel by name.
    pub fn get_channel(&self, name: &str) -> Option<&ExrChannel> {
        self.channels.iter().find(|c| c.name == name)
    }

    /// Get RGB as interleaved f32 data.
    pub fn to_rgb_f32(&self) -> Option<Vec<f32>> {
        let r = self.get_channel("R")?.data.as_slice();
        let g = self.get_channel("G")?.data.as_slice();
        let b = self.get_channel("B")?.data.as_slice();

        let mut rgb = Vec::with_capacity(r.len() * 3);
        for i in 0..r.len() {
            rgb.push(r[i]);
            rgb.push(g[i]);
            rgb.push(b[i]);
        }

        Some(rgb)
    }

    /// Create from RGB f32 data.
    pub fn from_rgb_f32(width: u32, height: u32, data: &[f32]) -> ExrResult<Self> {
        if data.len() != (width * height * 3) as usize {
            return Err(ExrError::Exr("Invalid data length".to_string()));
        }

        let pixel_count = (width * height) as usize;
        let mut r_data = Vec::with_capacity(pixel_count);
        let mut g_data = Vec::with_capacity(pixel_count);
        let mut b_data = Vec::with_capacity(pixel_count);

        for chunk in data.chunks(3) {
            r_data.push(chunk[0]);
            g_data.push(chunk[1]);
            b_data.push(chunk[2]);
        }

        Ok(Self {
            width,
            height,
            channels: vec![
                ExrChannel {
                    name: "R".to_string(),
                    data: r_data,
                },
                ExrChannel {
                    name: "G".to_string(),
                    data: g_data,
                },
                ExrChannel {
                    name: "B".to_string(),
                    data: b_data,
                },
            ],
        })
    }
}

/// Load EXR from file.
pub fn load_exr<P: AsRef<Path>>(path: P) -> ExrResult<ExrImage> {
    ExrImage::load(path)
}

/// Save EXR to file.
pub fn save_exr<P: AsRef<Path>>(path: P, image: &ExrImage) -> ExrResult<()> {
    image.save(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_rgb() {
        let rgb_data = vec![1.0, 0.5, 0.25, 0.75, 1.0, 0.5];
        let img = ExrImage::from_rgb_f32(2, 1, &rgb_data).unwrap();

        assert_eq!(img.width, 2);
        assert_eq!(img.height, 1);
        assert_eq!(img.channels.len(), 3);

        let r_channel = img.get_channel("R").unwrap();
        assert_eq!(r_channel.data, vec![1.0, 0.75]);
    }

    #[test]
    fn test_invalid_data_length() {
        let rgb_data = vec![1.0, 0.5]; // Too short
        let result = ExrImage::from_rgb_f32(2, 1, &rgb_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_channel() {
        let img = ExrImage::from_rgb_f32(1, 1, &[0.5, 0.6, 0.7]).unwrap();
        assert!(img.get_channel("R").is_some());
        assert!(img.get_channel("G").is_some());
        assert!(img.get_channel("B").is_some());
        assert!(img.get_channel("Z").is_none());
    }

    #[test]
    fn test_to_rgb_f32() {
        let img = ExrImage::from_rgb_f32(2, 1, &[1.0, 0.5, 0.25, 0.75, 1.0, 0.5]).unwrap();
        let rgb = img.to_rgb_f32().unwrap();
        assert_eq!(rgb, vec![1.0, 0.5, 0.25, 0.75, 1.0, 0.5]);
    }
}
