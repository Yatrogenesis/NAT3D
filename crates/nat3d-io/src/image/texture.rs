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

//! Texture loading and management system.

use crate::image::{exr_format::*, hdr::*, png::*};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use thiserror::Error;

/// Texture format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextureFormat {
    /// PNG.
    Png,
    /// OpenEXR.
    Exr,
    /// Radiance HDR.
    Hdr,
    /// JPEG.
    Jpg,
    /// Targa.
    Tga,
}

impl TextureFormat {
    /// Detect format from file extension.
    pub fn from_extension(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?;
        match ext.to_lowercase().as_str() {
            "png" => Some(Self::Png),
            "exr" => Some(Self::Exr),
            "hdr" => Some(Self::Hdr),
            "jpg" | "jpeg" => Some(Self::Jpg),
            "tga" => Some(Self::Tga),
            _ => None,
        }
    }
}

/// Texture errors.
#[derive(Error, Debug)]
pub enum TextureError {
    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// Unsupported format.
    #[error("Unsupported texture format")]
    UnsupportedFormat,
    /// PNG error.
    #[error("PNG error: {0}")]
    Png(#[from] PngError),
    /// EXR error.
    #[error("EXR error: {0}")]
    Exr(#[from] ExrError),
    /// HDR error.
    #[error("HDR error: {0}")]
    Hdr(#[from] HdrError),
}

/// Result type for texture operations.
pub type TextureResult<T> = Result<T, TextureError>;

/// Texture data.
#[derive(Debug, Clone)]
pub struct Texture {
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
    /// Format.
    pub format: TextureFormat,
    /// RGBA data (u8, 4 channels).
    pub data: Vec<u8>,
    /// Mipmaps (optional).
    pub mipmaps: Vec<Vec<u8>>,
}

impl Texture {
    /// Load texture from file (auto-detect format).
    pub fn load<P: AsRef<Path>>(path: P) -> TextureResult<Self> {
        let path = path.as_ref();
        let format = TextureFormat::from_extension(path).ok_or(TextureError::UnsupportedFormat)?;

        match format {
            TextureFormat::Png => {
                let png = PngImage::load(path)?;
                let data = png.to_rgba();
                Ok(Self {
                    width: png.width,
                    height: png.height,
                    format,
                    data,
                    mipmaps: Vec::new(),
                })
            }
            TextureFormat::Exr => {
                let exr = ExrImage::load(path)?;
                let rgb_f32 = exr.to_rgb_f32().ok_or(TextureError::UnsupportedFormat)?;

                // Convert f32 to u8 (simple tone map)
                let mut data = Vec::with_capacity(exr.width as usize * exr.height as usize * 4);
                for chunk in rgb_f32.chunks(3) {
                    for &val in chunk {
                        let byte = (val.min(1.0).max(0.0) * 255.0) as u8;
                        data.push(byte);
                    }
                    data.push(255); // Alpha
                }

                Ok(Self {
                    width: exr.width,
                    height: exr.height,
                    format,
                    data,
                    mipmaps: Vec::new(),
                })
            }
            TextureFormat::Hdr => {
                let hdr = HdrImage::load(path)?;
                let ldr = hdr.tone_map();

                // Convert RGB to RGBA
                let mut data = Vec::with_capacity((hdr.width * hdr.height * 4) as usize);
                for chunk in ldr.chunks(3) {
                    data.extend_from_slice(chunk);
                    data.push(255);
                }

                Ok(Self {
                    width: hdr.width,
                    height: hdr.height,
                    format,
                    data,
                    mipmaps: Vec::new(),
                })
            }
            TextureFormat::Jpg | TextureFormat::Tga => {
                // Use image crate for these
                let img = image::open(path).map_err(|_| TextureError::UnsupportedFormat)?;
                let rgba = img.to_rgba8();

                Ok(Self {
                    width: rgba.width(),
                    height: rgba.height(),
                    format,
                    data: rgba.into_raw(),
                    mipmaps: Vec::new(),
                })
            }
        }
    }

    /// Generate mipmaps.
    pub fn generate_mipmaps(&mut self) {
        use image::{DynamicImage, ImageBuffer, Rgba};

        self.mipmaps.clear();

        let mut current_width = self.width;
        let mut current_height = self.height;
        let mut current_data = self.data.clone();

        while current_width > 1 && current_height > 1 {
            // Downscale by half
            let new_width = (current_width / 2).max(1);
            let new_height = (current_height / 2).max(1);

            let img =
                ImageBuffer::<Rgba<u8>, _>::from_raw(current_width, current_height, current_data)
                    .unwrap();

            let resized = DynamicImage::ImageRgba8(img).resize(
                new_width,
                new_height,
                image::imageops::FilterType::Lanczos3,
            );

            current_data = resized.to_rgba8().into_raw();
            self.mipmaps.push(current_data.clone());

            current_width = new_width;
            current_height = new_height;
        }
    }

    /// Resize texture.
    pub fn resize(&mut self, new_width: u32, new_height: u32) {
        use image::{DynamicImage, ImageBuffer, Rgba};

        let img = ImageBuffer::<Rgba<u8>, _>::from_raw(self.width, self.height, self.data.clone())
            .unwrap();

        let resized = DynamicImage::ImageRgba8(img).resize(
            new_width,
            new_height,
            image::imageops::FilterType::Lanczos3,
        );

        self.width = new_width;
        self.height = new_height;
        self.data = resized.to_rgba8().into_raw();
        self.mipmaps.clear(); // Invalidate mipmaps
    }
}

/// Texture cache for avoiding duplicate loads.
#[derive(Default)]
pub struct TextureCache {
    /// Cached textures.
    cache: Arc<Mutex<HashMap<String, Arc<Texture>>>>,
}

impl TextureCache {
    /// Create new cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Load texture (uses cache).
    pub fn load<P: AsRef<Path>>(&self, path: P) -> TextureResult<Arc<Texture>> {
        let path_str = path.as_ref().to_string_lossy().to_string();

        let mut cache = self.cache.lock().unwrap();

        if let Some(texture) = cache.get(&path_str) {
            Ok(Arc::clone(texture))
        } else {
            let texture = Arc::new(Texture::load(path)?);
            cache.insert(path_str, Arc::clone(&texture));
            Ok(texture)
        }
    }

    /// Clear cache.
    pub fn clear(&self) {
        let mut cache = self.cache.lock().unwrap();
        cache.clear();
    }

    /// Get cache size.
    pub fn len(&self) -> usize {
        let cache = self.cache.lock().unwrap();
        cache.len()
    }

    /// Check if cache is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Load texture from file.
pub fn load_texture<P: AsRef<Path>>(path: P) -> TextureResult<Texture> {
    Texture::load(path)
}

/// Resize texture.
pub fn resize_texture(texture: &mut Texture, width: u32, height: u32) {
    texture.resize(width, height);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_detection() {
        assert_eq!(
            TextureFormat::from_extension(Path::new("test.png")),
            Some(TextureFormat::Png)
        );
        assert_eq!(
            TextureFormat::from_extension(Path::new("test.EXR")),
            Some(TextureFormat::Exr)
        );
        assert_eq!(
            TextureFormat::from_extension(Path::new("test.unknown")),
            None
        );
    }

    #[test]
    fn test_cache() {
        let cache = TextureCache::new();
        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());

        cache.clear();
        assert_eq!(cache.len(), 0);
    }
}
