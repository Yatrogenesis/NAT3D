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

//! Noise generation for procedural content.
//!
//! Provides various noise algorithms commonly used in 3D graphics
//! for textures, terrain, and effects.

/// Permutation table for noise generation.
const PERM: [u8; 512] = {
    let p: [u8; 256] = [
        151, 160, 137, 91, 90, 15, 131, 13, 201, 95, 96, 53, 194, 233, 7, 225, 140, 36, 103, 30,
        69, 142, 8, 99, 37, 240, 21, 10, 23, 190, 6, 148, 247, 120, 234, 75, 0, 26, 197, 62, 94,
        252, 219, 203, 117, 35, 11, 32, 57, 177, 33, 88, 237, 149, 56, 87, 174, 20, 125, 136, 171,
        168, 68, 175, 74, 165, 71, 134, 139, 48, 27, 166, 77, 146, 158, 231, 83, 111, 229, 122, 60,
        211, 133, 230, 220, 105, 92, 41, 55, 46, 245, 40, 244, 102, 143, 54, 65, 25, 63, 161, 1,
        216, 80, 73, 209, 76, 132, 187, 208, 89, 18, 169, 200, 196, 135, 130, 116, 188, 159, 86,
        164, 100, 109, 198, 173, 186, 3, 64, 52, 217, 226, 250, 124, 123, 5, 202, 38, 147, 118,
        126, 255, 82, 85, 212, 207, 206, 59, 227, 47, 16, 58, 17, 182, 189, 28, 42, 223, 183, 170,
        213, 119, 248, 152, 2, 44, 154, 163, 70, 221, 153, 101, 155, 167, 43, 172, 9, 129, 22, 39,
        253, 19, 98, 108, 110, 79, 113, 224, 232, 178, 185, 112, 104, 218, 246, 97, 228, 251, 34,
        242, 193, 238, 210, 144, 12, 191, 179, 162, 241, 81, 51, 145, 235, 249, 14, 239, 107, 49,
        192, 214, 31, 181, 199, 106, 157, 184, 84, 204, 176, 115, 121, 50, 45, 127, 4, 150, 254,
        138, 236, 205, 93, 222, 114, 67, 29, 24, 72, 243, 141, 128, 195, 78, 66, 215, 61, 156, 180,
    ];
    let mut result = [0u8; 512];
    let mut i = 0;
    while i < 256 {
        result[i] = p[i];
        result[i + 256] = p[i];
        i += 1;
    }
    result
};

/// Fade function for Perlin noise.
#[inline]
fn fade(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// Gradient function for Perlin noise.
#[inline]
fn grad(hash: u8, x: f64, y: f64, z: f64) -> f64 {
    let h = hash & 15;
    let u = if h < 8 { x } else { y };
    let v = if h < 4 {
        y
    } else if h == 12 || h == 14 {
        x
    } else {
        z
    };
    (if h & 1 == 0 { u } else { -u }) + (if h & 2 == 0 { v } else { -v })
}

/// 3D Perlin noise.
///
/// Returns a value in the range [-1, 1].
#[must_use]
pub fn perlin_3d(x: f64, y: f64, z: f64) -> f64 {
    let xi = (x.floor() as i32 & 255) as usize;
    let yi = (y.floor() as i32 & 255) as usize;
    let zi = (z.floor() as i32 & 255) as usize;

    let xf = x - x.floor();
    let yf = y - y.floor();
    let zf = z - z.floor();

    let u = fade(xf);
    let v = fade(yf);
    let w = fade(zf);

    let aaa = PERM[PERM[PERM[xi] as usize + yi] as usize + zi];
    let aba = PERM[PERM[PERM[xi] as usize + yi + 1] as usize + zi];
    let aab = PERM[PERM[PERM[xi] as usize + yi] as usize + zi + 1];
    let abb = PERM[PERM[PERM[xi] as usize + yi + 1] as usize + zi + 1];
    let baa = PERM[PERM[PERM[xi + 1] as usize + yi] as usize + zi];
    let bba = PERM[PERM[PERM[xi + 1] as usize + yi + 1] as usize + zi];
    let bab = PERM[PERM[PERM[xi + 1] as usize + yi] as usize + zi + 1];
    let bbb = PERM[PERM[PERM[xi + 1] as usize + yi + 1] as usize + zi + 1];

    let x1 = lerp(grad(aaa, xf, yf, zf), grad(baa, xf - 1.0, yf, zf), u);
    let x2 = lerp(
        grad(aba, xf, yf - 1.0, zf),
        grad(bba, xf - 1.0, yf - 1.0, zf),
        u,
    );
    let y1 = lerp(x1, x2, v);

    let x1 = lerp(
        grad(aab, xf, yf, zf - 1.0),
        grad(bab, xf - 1.0, yf, zf - 1.0),
        u,
    );
    let x2 = lerp(
        grad(abb, xf, yf - 1.0, zf - 1.0),
        grad(bbb, xf - 1.0, yf - 1.0, zf - 1.0),
        u,
    );
    let y2 = lerp(x1, x2, v);

    lerp(y1, y2, w)
}

#[inline]
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + t * (b - a)
}

/// 2D Perlin noise.
#[must_use]
pub fn perlin_2d(x: f64, y: f64) -> f64 {
    perlin_3d(x, y, 0.0)
}

/// Fractal Brownian Motion (fBm) using Perlin noise.
///
/// # Arguments
/// * `x`, `y`, `z` - Coordinates
/// * `octaves` - Number of noise layers
/// * `persistence` - Amplitude multiplier per octave (typically 0.5)
/// * `lacunarity` - Frequency multiplier per octave (typically 2.0)
#[must_use]
pub fn fbm_3d(x: f64, y: f64, z: f64, octaves: u32, persistence: f64, lacunarity: f64) -> f64 {
    let mut total = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut max_value = 0.0;

    for _ in 0..octaves {
        total += perlin_3d(x * frequency, y * frequency, z * frequency) * amplitude;
        max_value += amplitude;
        amplitude *= persistence;
        frequency *= lacunarity;
    }

    total / max_value
}

/// 2D fBm.
#[must_use]
pub fn fbm_2d(x: f64, y: f64, octaves: u32, persistence: f64, lacunarity: f64) -> f64 {
    fbm_3d(x, y, 0.0, octaves, persistence, lacunarity)
}

/// Turbulence noise (absolute value of fBm).
#[must_use]
pub fn turbulence_3d(
    x: f64,
    y: f64,
    z: f64,
    octaves: u32,
    persistence: f64,
    lacunarity: f64,
) -> f64 {
    let mut total = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut max_value = 0.0;

    for _ in 0..octaves {
        total += perlin_3d(x * frequency, y * frequency, z * frequency).abs() * amplitude;
        max_value += amplitude;
        amplitude *= persistence;
        frequency *= lacunarity;
    }

    total / max_value
}

/// Ridged multifractal noise.
#[must_use]
pub fn ridged_3d(x: f64, y: f64, z: f64, octaves: u32, persistence: f64, lacunarity: f64) -> f64 {
    let mut total = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut weight = 1.0;

    for _ in 0..octaves {
        let signal = 1.0 - perlin_3d(x * frequency, y * frequency, z * frequency).abs();
        let signal = signal * signal * weight;
        weight = (signal * 2.0).clamp(0.0, 1.0);
        total += signal * amplitude;
        amplitude *= persistence;
        frequency *= lacunarity;
    }

    total
}

/// Worley (cellular) noise - returns distance to nearest point.
#[must_use]
pub fn worley_3d(x: f64, y: f64, z: f64) -> f64 {
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let zi = z.floor() as i32;

    let mut min_dist = f64::MAX;

    for dx in -1..=1 {
        for dy in -1..=1 {
            for dz in -1..=1 {
                let cx = xi + dx;
                let cy = yi + dy;
                let cz = zi + dz;

                // Hash-based random point in cell
                let hash = ((cx * 127) ^ (cy * 269) ^ (cz * 419)) as u32;
                let px = cx as f64 + (hash as f64 / u32::MAX as f64);
                let hash = hash.wrapping_mul(16807);
                let py = cy as f64 + (hash as f64 / u32::MAX as f64);
                let hash = hash.wrapping_mul(16807);
                let pz = cz as f64 + (hash as f64 / u32::MAX as f64);

                let dist = (x - px).powi(2) + (y - py).powi(2) + (z - pz).powi(2);
                min_dist = min_dist.min(dist);
            }
        }
    }

    min_dist.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perlin_range() {
        for i in 0..100 {
            let x = i as f64 * 0.1;
            let y = i as f64 * 0.17;
            let z = i as f64 * 0.23;
            let value = perlin_3d(x, y, z);
            assert!(
                (-1.0..=1.0).contains(&value),
                "Perlin noise out of range: {}",
                value
            );
        }
    }

    #[test]
    fn test_perlin_continuity() {
        let v1 = perlin_3d(1.0, 1.0, 1.0);
        let v2 = perlin_3d(1.001, 1.0, 1.0);
        assert!((v1 - v2).abs() < 0.1, "Perlin noise not continuous");
    }

    #[test]
    fn test_fbm() {
        let value = fbm_3d(0.5, 0.5, 0.5, 4, 0.5, 2.0);
        assert!((-1.0..=1.0).contains(&value));
    }

    #[test]
    fn test_worley() {
        let value = worley_3d(0.5, 0.5, 0.5);
        assert!(value >= 0.0);
    }
}
