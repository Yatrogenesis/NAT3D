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

//! Bloom effect.
//!
//! HDR bloom for glowing highlights using Gaussian blur pyramid.

use nalgebra::Vector3;

/// Bloom effect configuration.
#[derive(Debug, Clone)]
pub struct BloomConfig {
    /// Bloom threshold (minimum luminance).
    pub threshold: f64,
    /// Soft knee for threshold.
    pub soft_knee: f64,
    /// Bloom intensity.
    pub intensity: f64,
    /// Number of blur iterations.
    pub iterations: usize,
    /// Blur radius.
    pub radius: f64,
    /// Mip chain length.
    pub mip_count: usize,
}

impl Default for BloomConfig {
    fn default() -> Self {
        Self {
            threshold: 1.0,
            soft_knee: 0.5,
            intensity: 1.0,
            iterations: 3,
            radius: 1.0,
            mip_count: 5,
        }
    }
}

/// Bloom processor for HDR images.
pub struct BloomProcessor {
    /// Configuration.
    pub config: BloomConfig,
    /// Downsampled mip chain.
    mip_chain: Vec<Vec<Vector3<f64>>>,
    /// Mip dimensions.
    mip_sizes: Vec<(usize, usize)>,
}

impl BloomProcessor {
    /// Create a new bloom processor.
    pub fn new(config: BloomConfig) -> Self {
        Self {
            config,
            mip_chain: Vec::new(),
            mip_sizes: Vec::new(),
        }
    }

    /// Process an HDR image with bloom.
    pub fn process(
        &mut self,
        image: &[Vector3<f64>],
        width: usize,
        height: usize,
    ) -> Vec<Vector3<f64>> {
        // Extract bright pixels
        let bright = self.extract_bright(image);

        // Build mip chain
        self.build_mip_chain(&bright, width, height);

        // Blur each mip level
        for i in 0..self.mip_chain.len() {
            let (w, h) = self.mip_sizes[i];
            for _ in 0..self.config.iterations {
                self.blur_mip(i, w, h);
            }
        }

        // Upsample and combine
        self.upsample_combine();

        // Final composite
        let bloom = &self.mip_chain[0];
        let mut result = Vec::with_capacity(image.len());

        for i in 0..image.len() {
            let bloom_color = if i < bloom.len() {
                bloom[i]
            } else {
                Vector3::zeros()
            };
            result.push(image[i] + bloom_color * self.config.intensity);
        }

        result
    }

    /// Extract pixels above threshold.
    fn extract_bright(&self, image: &[Vector3<f64>]) -> Vec<Vector3<f64>> {
        image
            .iter()
            .map(|pixel| {
                let lum = luminance(*pixel);
                let soft = soft_threshold(lum, self.config.threshold, self.config.soft_knee);
                *pixel * soft
            })
            .collect()
    }

    /// Build downsampling mip chain.
    fn build_mip_chain(&mut self, image: &[Vector3<f64>], width: usize, height: usize) {
        self.mip_chain.clear();
        self.mip_sizes.clear();

        self.mip_chain.push(image.to_vec());
        self.mip_sizes.push((width, height));

        let mut w = width;
        let mut h = height;

        for _ in 1..self.config.mip_count {
            let prev = self.mip_chain.last().unwrap();
            let prev_w = w;
            let prev_h = h;

            w = (w / 2).max(1);
            h = (h / 2).max(1);

            let mut mip = vec![Vector3::zeros(); w * h];

            for y in 0..h {
                for x in 0..w {
                    // Sample 2x2 box from previous level
                    let px = (x * 2).min(prev_w - 1);
                    let py = (y * 2).min(prev_h - 1);

                    let mut sum = Vector3::zeros();
                    let mut count = 0.0;

                    for dy in 0..2 {
                        for dx in 0..2 {
                            let sx = (px + dx).min(prev_w - 1);
                            let sy = (py + dy).min(prev_h - 1);
                            sum += prev[sy * prev_w + sx];
                            count += 1.0;
                        }
                    }

                    mip[y * w + x] = sum / count;
                }
            }

            self.mip_chain.push(mip);
            self.mip_sizes.push((w, h));
        }
    }

    /// Apply Gaussian blur to a mip level.
    fn blur_mip(&mut self, mip_index: usize, width: usize, height: usize) {
        let radius = (self.config.radius * (1 << mip_index) as f64).max(1.0) as usize;
        let kernel = gaussian_kernel(radius);

        let mip = &self.mip_chain[mip_index];

        // Horizontal pass
        let mut temp = vec![Vector3::zeros(); width * height];
        for y in 0..height {
            for x in 0..width {
                let mut sum = Vector3::zeros();
                let mut weight_sum = 0.0;

                for (i, &weight) in kernel.iter().enumerate() {
                    let offset = i as i64 - radius as i64;
                    let sx = (x as i64 + offset).clamp(0, width as i64 - 1) as usize;
                    sum += mip[y * width + sx] * weight;
                    weight_sum += weight;
                }

                temp[y * width + x] = sum / weight_sum;
            }
        }

        // Vertical pass
        let mip = &mut self.mip_chain[mip_index];
        for y in 0..height {
            for x in 0..width {
                let mut sum = Vector3::zeros();
                let mut weight_sum = 0.0;

                for (i, &weight) in kernel.iter().enumerate() {
                    let offset = i as i64 - radius as i64;
                    let sy = (y as i64 + offset).clamp(0, height as i64 - 1) as usize;
                    sum += temp[sy * width + x] * weight;
                    weight_sum += weight;
                }

                mip[y * width + x] = sum / weight_sum;
            }
        }
    }

    /// Upsample and combine mip levels.
    fn upsample_combine(&mut self) {
        for i in (0..self.mip_chain.len() - 1).rev() {
            let (w, h) = self.mip_sizes[i];
            let (sw, sh) = self.mip_sizes[i + 1];

            let smaller = self.mip_chain[i + 1].clone();

            for y in 0..h {
                for x in 0..w {
                    // Bilinear sample from smaller mip
                    let sx = x as f64 * sw as f64 / w as f64;
                    let sy = y as f64 * sh as f64 / h as f64;

                    let x0 = sx.floor() as usize;
                    let y0 = sy.floor() as usize;
                    let x1 = (x0 + 1).min(sw - 1);
                    let y1 = (y0 + 1).min(sh - 1);

                    let fx = sx.fract();
                    let fy = sy.fract();

                    let c00 = smaller[y0 * sw + x0];
                    let c10 = smaller[y0 * sw + x1];
                    let c01 = smaller[y1 * sw + x0];
                    let c11 = smaller[y1 * sw + x1];

                    let c0 = c00 * (1.0 - fx) + c10 * fx;
                    let c1 = c01 * (1.0 - fx) + c11 * fx;
                    let upsampled = c0 * (1.0 - fy) + c1 * fy;

                    // Add to current level
                    self.mip_chain[i][y * w + x] += upsampled;
                }
            }
        }
    }
}

/// Compute luminance of a color.
fn luminance(color: Vector3<f64>) -> f64 {
    0.2126 * color.x + 0.7152 * color.y + 0.0722 * color.z
}

/// Soft threshold for smoother bloom.
fn soft_threshold(value: f64, threshold: f64, knee: f64) -> f64 {
    let soft = value - threshold + knee;
    let soft = soft.clamp(0.0, 2.0 * knee);
    let soft = soft * soft / (4.0 * knee + 1e-5);

    (soft.max(value - threshold)) / value.max(1e-5)
}

/// Generate 1D Gaussian kernel.
fn gaussian_kernel(radius: usize) -> Vec<f64> {
    let size = radius * 2 + 1;
    let sigma = radius as f64 / 3.0;
    let mut kernel = Vec::with_capacity(size);

    for i in 0..size {
        let x = i as f64 - radius as f64;
        let g = (-x * x / (2.0 * sigma * sigma)).exp();
        kernel.push(g);
    }

    // Normalize
    let sum: f64 = kernel.iter().sum();
    for k in &mut kernel {
        *k /= sum;
    }

    kernel
}

/// Lens flare configuration.
#[derive(Debug, Clone)]
pub struct LensFlareConfig {
    /// Number of ghosts.
    pub ghost_count: usize,
    /// Ghost spacing.
    pub ghost_spacing: f64,
    /// Ghost intensity.
    pub ghost_intensity: f64,
    /// Halo intensity.
    pub halo_intensity: f64,
    /// Halo width.
    pub halo_width: f64,
    /// Chromatic aberration strength.
    pub chromatic_aberration: f64,
}

impl Default for LensFlareConfig {
    fn default() -> Self {
        Self {
            ghost_count: 8,
            ghost_spacing: 0.15,
            ghost_intensity: 0.1,
            halo_intensity: 0.2,
            halo_width: 0.5,
            chromatic_aberration: 0.02,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_luminance() {
        let white = Vector3::new(1.0, 1.0, 1.0);
        let lum = luminance(white);
        assert!((lum - 1.0).abs() < 1e-6);

        let red = Vector3::new(1.0, 0.0, 0.0);
        let lum = luminance(red);
        assert!((lum - 0.2126).abs() < 1e-6);
    }

    #[test]
    fn test_soft_threshold() {
        // Value below threshold
        let t = soft_threshold(0.5, 1.0, 0.5);
        assert!(t < 0.1);

        // Value well above threshold
        let t = soft_threshold(2.0, 1.0, 0.5);
        assert!(t > 0.4);
    }

    #[test]
    fn test_gaussian_kernel() {
        let kernel = gaussian_kernel(3);
        assert_eq!(kernel.len(), 7);

        // Should sum to 1
        let sum: f64 = kernel.iter().sum();
        assert!((sum - 1.0).abs() < 1e-6);

        // Should be symmetric
        assert!((kernel[0] - kernel[6]).abs() < 1e-10);
        assert!((kernel[1] - kernel[5]).abs() < 1e-10);

        // Center should be highest
        assert!(kernel[3] > kernel[0]);
    }

    #[test]
    fn test_bloom_processor() {
        let config = BloomConfig::default();
        let mut processor = BloomProcessor::new(config);

        let image = vec![
            Vector3::new(2.0, 2.0, 2.0), // Bright
            Vector3::new(0.5, 0.5, 0.5), // Dark
            Vector3::new(1.5, 1.5, 1.5), // Medium
            Vector3::new(0.2, 0.2, 0.2), // Dark
        ];

        let result = processor.process(&image, 2, 2);
        assert_eq!(result.len(), 4);
    }
}
