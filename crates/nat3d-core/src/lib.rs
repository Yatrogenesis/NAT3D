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

//! # NAT3D Core
//!
//! Core data structures and types for the NAT3D 3D modeling, CAD, and simulation suite.

#![warn(missing_docs)]
#![allow(clippy::all)]
#![allow(clippy::pedantic)]
#![allow(clippy::nursery)]

pub mod document;
pub mod error;
pub mod geometry;
pub mod hierarchy;
pub mod history;
pub mod layer;
pub mod material;
pub mod selection;
pub mod stylus;
pub mod transform;

pub use document::{Document, DocumentMetadata};
pub use error::{CoreError, CoreResult};
pub use geometry::{
    primitives::{Primitive, PrimitiveParams},
    BoundingBox, Edge, EdgeId, Face, FaceId, Mesh, MeshId, Normal, Position, TexCoord, Vertex,
    VertexId,
};
pub use hierarchy::{Object, ObjectId, SceneGraph};
pub use history::{Command, History, HistoryEntry};
pub use layer::{Layer, LayerId, LayerProperties};
pub use material::{Material, MaterialId, MaterialProperties};
pub use selection::{Selection, SelectionMode, SelectionSet};
pub use stylus::{StylusCapabilities, StylusEvent, StylusInput, StylusProvider, StylusStroke};
pub use transform::{Transform, TransformComponent};

/// The prelude module provides convenient imports for common types.
pub mod prelude {
    pub use crate::{
        BoundingBox, Command, CoreError, CoreResult, Document, DocumentMetadata, Edge, EdgeId,
        Face, FaceId, History, HistoryEntry, Layer, LayerId, LayerProperties, Material, MaterialId,
        MaterialProperties, Mesh, MeshId, Normal, Object, ObjectId, Position, SceneGraph,
        Selection, SelectionMode, SelectionSet, StylusCapabilities, StylusEvent, StylusInput,
        StylusProvider, StylusStroke, TexCoord, Transform, TransformComponent, Vertex, VertexId,
    };
}
