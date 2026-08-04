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

//! File format handlers for 3D model import/export.
//!
//! Supported formats:
//! - OBJ: Wavefront OBJ format (widely supported, simple)
//! - STL: Stereolithography (3D printing)
//! - glTF: GL Transmission Format (modern, efficient)
//! - Native: NAT3D native format (full feature support)
//!
//! Planned formats:
//! - FBX: Autodesk FBX (industry standard)
//! - STEP: ISO 10303 (CAD exchange)
//! - IGES: Initial Graphics Exchange Specification
//! - DXF: AutoCAD Drawing Exchange Format

pub mod dxf;
pub mod fbx;
pub mod gltf_format;
pub mod iges;
pub mod native;
pub mod obj;
pub mod step;
pub mod stl;

// Re-exports for convenience
pub use gltf_format::{export_gltf, import_gltf, GltfError, GltfExporter, GltfImporter, GltfScene};
pub use native::{
    export_nat, import_nat, NativeCamera, NativeError, NativeMaterial, NativeObject, NativeResult,
    NativeScene, SceneMetadata,
};
pub use obj::{
    export_mesh_obj, export_obj, import_obj, ObjData, ObjError, ObjExporter, ObjGroup, ObjImporter,
    ObjObject,
};
pub use stl::{
    export_mesh_stl, export_stl, import_stl, StlData, StlError, StlExporter, StlImporter,
};
