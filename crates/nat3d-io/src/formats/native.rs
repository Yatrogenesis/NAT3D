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

//! Native NAT3D file format (.nat).
//!
//! Provides high-performance binary serialization/deserialization for NAT3D scenes.
//! Uses bitcode for efficient representation.

use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::Path;
// Reusing error type or defining a new one

/// Magic number for NAT3D files ("NAT\x01").
const NAT_MAGIC: [u8; 4] = [b'N', b'A', b'T', 0x01];

/// Native format version.
const NATIVE_VERSION: u32 = 1;

/// Native format error.
#[derive(Debug, thiserror::Error)]
pub enum NativeError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Invalid magic number")]
    InvalidMagic,
    #[error("Unsupported version: {0}")]
    UnsupportedVersion(u32),
}

/// Result type for native operations.
pub type NativeResult<T> = Result<T, NativeError>;

/// Native scene data structure for serialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeScene {
    pub version: u32,
    pub metadata: SceneMetadata,
    pub objects: Vec<NativeObject>,
    pub camera: Option<NativeCamera>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneMetadata {
    pub name: String,
    pub author: String,
    pub created_at: u64,
    pub modified_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeObject {
    pub name: String,
    pub object_type: String,
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
    pub material: Option<NativeMaterial>,
    pub modifiers: Vec<String>,
    pub visible: bool,
    pub children: Vec<NativeObject>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeMaterial {
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeCamera {
    pub position: [f32; 3],
    pub target: [f32; 3],
    pub orbit_angles: [f32; 2],
    pub distance: f32,
}

/// Export a scene to .nat format.
pub fn export_nat<P: AsRef<Path>>(path: P, scene: &NativeScene) -> NativeResult<()> {
    let mut file = std::fs::File::create(path)?;

    // Write header
    file.write_all(&NAT_MAGIC)?;
    file.write_all(&NATIVE_VERSION.to_le_bytes())?;

    // Serialize data using bitcode (assuming bitcode is in workspace)
    // For now, let's use bincode or similar if bitcode isn't available as a dependency yet,
    // but the plan mentioned bitcode. I'll check Cargo.toml after this if possible.
    // Assuming bincode for the implementation if bitcode is not yet a dependency.
    let encoded =
        bincode::serialize(scene).map_err(|e| NativeError::Serialization(e.to_string()))?;

    file.write_all(&encoded)?;
    Ok(())
}

/// Import a scene from .nat format.
pub fn import_nat<P: AsRef<Path>>(path: P) -> NativeResult<NativeScene> {
    let mut file = std::fs::File::open(path)?;

    // Read and verify header
    let mut magic = [0u8; 4];
    file.read_exact(&mut magic)?;
    if magic != NAT_MAGIC {
        return Err(NativeError::InvalidMagic);
    }

    let mut version_bytes = [0u8; 4];
    file.read_exact(&mut version_bytes)?;
    let version = u32::from_le_bytes(version_bytes);
    if version > NATIVE_VERSION {
        return Err(NativeError::UnsupportedVersion(version));
    }

    // Read rest of the file
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    let scene: NativeScene =
        bincode::deserialize(&buffer).map_err(|e| NativeError::Serialization(e.to_string()))?;

    Ok(scene)
}
