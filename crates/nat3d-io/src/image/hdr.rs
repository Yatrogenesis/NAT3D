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

//! Radiance HDR format handler.
//!
//! Supports RGBE (Run-Length Encoded) HDR format.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use thiserror::Error;

/// HDR errors.
#[derive(Error, Debug)]
pub enum HdrError {
    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// Parse error.
    #[error("Parse error: {0}")]
    Parse(String),
    /// Invalid format.
    #[error("Invalid HDR format")]
    InvalidFormat,
}

/// Result type for HDR operations.
pub type HdrResult<T> = Result<T, HdrError>;

/// HDR image data (RGB as f32).
#[derive(Debug, Clone)]
pub struct HdrImage {
    /// Image width.
    pub width: u32,
    /// Image height.
    pub height: u32,
    /// RGB data (f32, interleaved).
    pub data: Vec<f32>,
}

impl HdrImage {
    /// Load HDR from file.
    pub fn load<P: AsRef<Path>>(path: P) -> HdrResult<Self> {
        let file = std::fs::File::open(path)?;
        Self::load_from_reader(BufReader::new(file))
    }

    /// Load from reader.
    pub fn load_from_reader<R: Read>(mut reader: BufReader<R>) -> HdrResult<Self> {
        // Parse header
        let mut line = String::new();
        reader.read_line(&mut line)?;

        if !line.starts_with("#?RADIANCE") && !line.starts_with("#?RGBE") {
            return Err(HdrError::InvalidFormat);
        }

        // Skip header lines until we find resolution
        let (width, height) = loop {
            line.clear();
            reader.read_line(&mut line)?;

            if line.trim().is_empty() {
                // Empty line marks end of header
                line.clear();
                reader.read_line(&mut line)?;

                // Parse resolution: "-Y height +X width"
                if let Some((h, w)) = Self::parse_resolution(&line) {
                    break (w, h);
                }
            }
        };

        // Read RGBE data
        let mut rgbe_data = vec![0u8; (width * height * 4) as usize];
        reader.read_exact(&mut rgbe_data)?;

        // Convert RGBE to RGB f32
        let rgb_data = Self::rgbe_to_rgb(&rgbe_data);

        Ok(Self {
            width,
            height,
            data: rgb_data,
        })
    }

    /// Save HDR to file.
    pub fn save<P: AsRef<Path>>(&self, path: P) -> HdrResult<()> {
        let file = std::fs::File::create(path)?;
        self.save_to_writer(file)
    }

    /// Save to writer.
    pub fn save_to_writer<W: Write>(&self, mut writer: W) -> HdrResult<()> {
        // Write header
        writeln!(writer, "#?RADIANCE")?;
        writeln!(writer, "# NAT3D HDR Export")?;
        writeln!(writer, "FORMAT=32-bit_rle_rgbe")?;
        writeln!(writer)?;
        writeln!(writer, "-Y {} +X {}", self.height, self.width)?;

        // Convert RGB to RGBE and write
        let rgbe_data = Self::rgb_to_rgbe(&self.data);
        writer.write_all(&rgbe_data)?;

        Ok(())
    }

    fn parse_resolution(line: &str) -> Option<(u32, u32)> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 4 && parts[0] == "-Y" && parts[2] == "+X" {
            let height = parts[1].parse().ok()?;
            let width = parts[3].parse().ok()?;
            Some((height, width))
        } else {
            None
        }
    }

    fn rgbe_to_rgb(rgbe: &[u8]) -> Vec<f32> {
        let mut rgb = Vec::with_capacity((rgbe.len() / 4) * 3);

        for chunk in rgbe.chunks(4) {
            if chunk.len() < 4 {
                break;
            }

            let r = chunk[0];
            let g = chunk[1];
            let b = chunk[2];
            let e = chunk[3] as i32;

            if e == 0 {
                rgb.push(0.0);
                rgb.push(0.0);
                rgb.push(0.0);
            } else {
                let scale = 2f32.powi(e - 128 - 8);
                rgb.push(r as f32 * scale);
                rgb.push(g as f32 * scale);
                rgb.push(b as f32 * scale);
            }
        }

        rgb
    }

    fn rgb_to_rgbe(rgb: &[f32]) -> Vec<u8> {
        let mut rgbe = Vec::with_capacity((rgb.len() / 3) * 4);

        for chunk in rgb.chunks(3) {
            if chunk.len() < 3 {
                break;
            }

            let r = chunk[0];
            let g = chunk[1];
            let b = chunk[2];

            let max_val = r.max(g).max(b);

            if max_val < 1e-32 {
                rgbe.extend_from_slice(&[0, 0, 0, 0]);
            } else {
                let (mantissa, exponent) = Self::frexp(max_val);
                let scale = mantissa * 256.0 / max_val;

                rgbe.push((r * scale) as u8);
                rgbe.push((g * scale) as u8);
                rgbe.push((b * scale) as u8);
                rgbe.push((exponent + 128) as u8);
            }
        }

        rgbe
    }

    fn frexp(value: f32) -> (f32, i32) {
        if value == 0.0 {
            (0.0, 0)
        } else {
            let bits = value.to_bits();
            let exponent = ((bits >> 23) & 0xFF) as i32 - 126;
            let mantissa = f32::from_bits((bits & 0x807FFFFF) | 0x3F000000);
            (mantissa, exponent)
        }
    }

    /// Adjust exposure.
    pub fn adjust_exposure(&mut self, exposure: f32) {
        let scale = 2f32.powf(exposure);
        for pixel in &mut self.data {
            *pixel *= scale;
        }
    }

    /// Tone map to LDR (simple Reinhard).
    pub fn tone_map(&self) -> Vec<u8> {
        let mut ldr = Vec::with_capacity(self.data.len());

        for &value in &self.data {
            let mapped = value / (1.0 + value);
            let byte = (mapped * 255.0).min(255.0).max(0.0) as u8;
            ldr.push(byte);
        }

        ldr
    }
}

/// Load HDR from file.
pub fn load_hdr<P: AsRef<Path>>(path: P) -> HdrResult<HdrImage> {
    HdrImage::load(path)
}

/// Save HDR to file.
pub fn save_hdr<P: AsRef<Path>>(path: P, image: &HdrImage) -> HdrResult<()> {
    image.save(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgbe_conversion() {
        let rgb = vec![1.0, 0.5, 0.25];
        let rgbe = HdrImage::rgb_to_rgbe(&rgb);
        assert_eq!(rgbe.len(), 4);

        let decoded = HdrImage::rgbe_to_rgb(&rgbe);
        assert_eq!(decoded.len(), 3);

        // Check approximate equality (RGBE is lossy)
        for i in 0..3 {
            assert!((decoded[i] - rgb[i]).abs() < 0.1);
        }
    }

    #[test]
    fn test_resolution_parsing() {
        let line = "-Y 512 +X 1024";
        let (h, w) = HdrImage::parse_resolution(line).unwrap();
        assert_eq!(h, 512);
        assert_eq!(w, 1024);
    }

    #[test]
    fn test_exposure_adjustment() {
        let mut img = HdrImage {
            width: 1,
            height: 1,
            data: vec![1.0, 0.5, 0.25],
        };

        img.adjust_exposure(1.0); // Double exposure
        assert!((img.data[0] - 2.0).abs() < 0.001);
        assert!((img.data[1] - 1.0).abs() < 0.001);
    }
}
