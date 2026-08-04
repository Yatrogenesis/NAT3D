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

//! PNG image format handler.
//!
//! Uses the `image` crate for PNG loading and saving.

use std::path::Path;
use thiserror::Error;

/// PNG errors.
#[derive(Error, Debug)]
pub enum PngError {
    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// Image error.
    #[error("Image error: {0}")]
    Image(#[from] image::ImageError),
}

/// Result type for PNG operations.
pub type PngResult<T> = Result<T, PngError>;

/// PNG image data.
#[derive(Debug, Clone)]
pub struct PngImage {
    /// Image width.
    pub width: u32,
    /// Image height.
    pub height: u32,
    /// Number of channels (1=gray, 2=gray+alpha, 3=RGB, 4=RGBA).
    pub channels: u8,
    /// Raw pixel data.
    pub data: Vec<u8>,
}

impl PngImage {
    /// Load PNG from file.
    pub fn load<P: AsRef<Path>>(path: P) -> PngResult<Self> {
        let img = image::open(path)?;
        let width = img.width();
        let height = img.height();

        let (data, channels) = match img {
            image::DynamicImage::ImageLuma8(_) => (img.to_luma8().into_raw(), 1),
            image::DynamicImage::ImageLumaA8(_) => (img.to_luma_alpha8().into_raw(), 2),
            image::DynamicImage::ImageRgb8(_) => (img.to_rgb8().into_raw(), 3),
            image::DynamicImage::ImageRgba8(_) => (img.to_rgba8().into_raw(), 4),
            _ => (img.to_rgba8().into_raw(), 4),
        };

        Ok(Self {
            width,
            height,
            channels,
            data,
        })
    }

    /// Save PNG to file.
    pub fn save<P: AsRef<Path>>(&self, path: P) -> PngResult<()> {
        use image::{ImageBuffer, Rgba};

        let img: ImageBuffer<Rgba<u8>, Vec<u8>> = match self.channels {
            4 => ImageBuffer::from_raw(self.width, self.height, self.data.clone()).ok_or_else(
                || {
                    PngError::Image(image::ImageError::Parameter(
                        image::error::ParameterError::from_kind(
                            image::error::ParameterErrorKind::DimensionMismatch,
                        ),
                    ))
                },
            )?,
            3 => {
                // Convert RGB to RGBA
                let mut rgba_data = Vec::with_capacity((self.width * self.height * 4) as usize);
                for chunk in self.data.chunks(3) {
                    rgba_data.push(chunk[0]);
                    rgba_data.push(chunk[1]);
                    rgba_data.push(chunk[2]);
                    rgba_data.push(255);
                }
                ImageBuffer::from_raw(self.width, self.height, rgba_data).ok_or_else(|| {
                    PngError::Image(image::ImageError::Parameter(
                        image::error::ParameterError::from_kind(
                            image::error::ParameterErrorKind::DimensionMismatch,
                        ),
                    ))
                })?
            }
            _ => {
                return Err(PngError::Image(image::ImageError::Unsupported(
                    image::error::UnsupportedError::from_format_and_kind(
                        image::error::ImageFormatHint::Unknown,
                        image::error::UnsupportedErrorKind::GenericFeature(format!(
                            "Unsupported channel count: {}",
                            self.channels
                        )),
                    ),
                )))
            }
        };

        img.save(path)?;
        Ok(())
    }

    /// Create from RGBA data.
    pub fn from_rgba(width: u32, height: u32, data: Vec<u8>) -> Self {
        Self {
            width,
            height,
            channels: 4,
            data,
        }
    }

    /// Convert to RGBA format.
    pub fn to_rgba(&self) -> Vec<u8> {
        match self.channels {
            4 => self.data.clone(),
            3 => {
                let mut rgba = Vec::with_capacity((self.width * self.height * 4) as usize);
                for chunk in self.data.chunks(3) {
                    rgba.push(chunk[0]);
                    rgba.push(chunk[1]);
                    rgba.push(chunk[2]);
                    rgba.push(255);
                }
                rgba
            }
            1 => {
                let mut rgba = Vec::with_capacity((self.width * self.height * 4) as usize);
                for &gray in &self.data {
                    rgba.push(gray);
                    rgba.push(gray);
                    rgba.push(gray);
                    rgba.push(255);
                }
                rgba
            }
            2 => {
                let mut rgba = Vec::with_capacity((self.width * self.height * 4) as usize);
                for chunk in self.data.chunks(2) {
                    let gray = chunk[0];
                    let alpha = chunk[1];
                    rgba.push(gray);
                    rgba.push(gray);
                    rgba.push(gray);
                    rgba.push(alpha);
                }
                rgba
            }
            _ => vec![],
        }
    }

    /// Resize image.
    pub fn resize(&mut self, new_width: u32, new_height: u32) {
        use image::{DynamicImage, ImageBuffer, Rgba};

        let img: DynamicImage = if self.channels == 4 {
            let buffer =
                ImageBuffer::<Rgba<u8>, _>::from_raw(self.width, self.height, self.data.clone())
                    .unwrap();
            DynamicImage::ImageRgba8(buffer)
        } else {
            let rgba = self.to_rgba();
            let buffer =
                ImageBuffer::<Rgba<u8>, _>::from_raw(self.width, self.height, rgba).unwrap();
            DynamicImage::ImageRgba8(buffer)
        };

        let resized = img.resize(new_width, new_height, image::imageops::FilterType::Lanczos3);
        self.width = new_width;
        self.height = new_height;
        self.channels = 4;
        self.data = resized.to_rgba8().into_raw();
    }
}

/// Load PNG from file.
pub fn load_png<P: AsRef<Path>>(path: P) -> PngResult<PngImage> {
    PngImage::load(path)
}

/// Save PNG to file.
pub fn save_png<P: AsRef<Path>>(path: P, image: &PngImage) -> PngResult<()> {
    image.save(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_rgba() {
        let data = vec![255u8, 0, 0, 255, 0, 255, 0, 255]; // Red and green pixels
        let img = PngImage::from_rgba(2, 1, data);

        assert_eq!(img.width, 2);
        assert_eq!(img.height, 1);
        assert_eq!(img.channels, 4);
        assert_eq!(img.data.len(), 8);
    }

    #[test]
    fn test_rgb_to_rgba_conversion() {
        let rgb_data = vec![255, 0, 0, 0, 255, 0]; // Red and green pixels
        let img = PngImage {
            width: 2,
            height: 1,
            channels: 3,
            data: rgb_data,
        };

        let rgba = img.to_rgba();
        assert_eq!(rgba.len(), 8);
        assert_eq!(rgba[0..4], [255, 0, 0, 255]); // Red with full alpha
        assert_eq!(rgba[4..8], [0, 255, 0, 255]); // Green with full alpha
    }
}
