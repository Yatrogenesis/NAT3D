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

//! Error types for nat3d-core.
//!
//! This module defines the error types used throughout the core crate,
//! providing detailed error information for all operations.

use std::path::PathBuf;
use thiserror::Error;
use uuid::Uuid;

/// Result type alias for core operations.
pub type CoreResult<T> = Result<T, CoreError>;

/// Errors that can occur in core operations.
#[derive(Error, Debug)]
pub enum CoreError {
    // ══════════════════════════════════════════════════════════════════════════
    // Geometry Errors
    // ══════════════════════════════════════════════════════════════════════════
    /// Invalid vertex index in mesh operation.
    #[error("Invalid vertex index {index} in mesh with {count} vertices")]
    InvalidVertexIndex {
        /// The invalid index that was provided.
        index: usize,
        /// The total number of vertices in the mesh.
        count: usize,
    },

    /// Invalid edge index in mesh operation.
    #[error("Invalid edge index {index} in mesh with {count} edges")]
    InvalidEdgeIndex {
        /// The invalid index that was provided.
        index: usize,
        /// The total number of edges in the mesh.
        count: usize,
    },

    /// Invalid face index in mesh operation.
    #[error("Invalid face index {index} in mesh with {count} faces")]
    InvalidFaceIndex {
        /// The invalid index that was provided.
        index: usize,
        /// The total number of faces in the mesh.
        count: usize,
    },

    /// Degenerate geometry detected (e.g., zero-area face, coincident vertices).
    #[error("Degenerate geometry: {description}")]
    DegenerateGeometry {
        /// Description of the degenerate condition.
        description: String,
    },

    /// Invalid mesh topology.
    #[error("Invalid mesh topology: {description}")]
    InvalidTopology {
        /// Description of the topology error.
        description: String,
    },

    /// Empty mesh when non-empty was expected.
    #[error("Mesh is empty but operation requires at least {required}")]
    EmptyMesh {
        /// What was required (e.g., "one vertex", "three vertices for a face").
        required: String,
    },

    // ══════════════════════════════════════════════════════════════════════════
    // Document Errors
    // ══════════════════════════════════════════════════════════════════════════
    /// Object not found in document.
    #[error("Object with ID {0} not found in document")]
    ObjectNotFound(Uuid),

    /// Layer not found in document.
    #[error("Layer with ID {0} not found in document")]
    LayerNotFound(Uuid),

    /// Material not found in document.
    #[error("Material with ID {0} not found in document")]
    MaterialNotFound(Uuid),

    /// Mesh not found in document.
    #[error("Mesh with ID {0} not found in document")]
    MeshNotFound(Uuid),

    /// Attempted to delete a protected object (e.g., default layer).
    #[error("Cannot delete protected object: {name}")]
    ProtectedObject {
        /// Name of the protected object.
        name: String,
    },

    /// Circular dependency detected in scene graph.
    #[error("Circular dependency detected: {path}")]
    CircularDependency {
        /// Path showing the circular dependency.
        path: String,
    },

    // ══════════════════════════════════════════════════════════════════════════
    // History Errors
    // ══════════════════════════════════════════════════════════════════════════
    /// No operations to undo.
    #[error("Nothing to undo")]
    NothingToUndo,

    /// No operations to redo.
    #[error("Nothing to redo")]
    NothingToRedo,

    /// Command execution failed.
    #[error("Command execution failed: {description}")]
    CommandFailed {
        /// Description of the failure.
        description: String,
    },

    // ══════════════════════════════════════════════════════════════════════════
    // Transform Errors
    // ══════════════════════════════════════════════════════════════════════════
    /// Matrix is not invertible.
    #[error("Transform matrix is singular and cannot be inverted")]
    SingularMatrix,

    /// Invalid scale (zero or negative when not allowed).
    #[error("Invalid scale value: {0}")]
    InvalidScale(f64),

    // ══════════════════════════════════════════════════════════════════════════
    // Selection Errors
    // ══════════════════════════════════════════════════════════════════════════
    /// Selection mode mismatch.
    #[error("Selection mode mismatch: expected {expected}, got {actual}")]
    SelectionModeMismatch {
        /// Expected selection mode.
        expected: String,
        /// Actual selection mode.
        actual: String,
    },

    // ══════════════════════════════════════════════════════════════════════════
    // I/O Errors
    // ══════════════════════════════════════════════════════════════════════════
    /// File I/O error.
    #[error("I/O error for path {path}: {source}")]
    IoError {
        /// Path that caused the error.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// Serialization error.
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Deserialization error.
    #[error("Deserialization error: {0}")]
    DeserializationError(String),

    // ══════════════════════════════════════════════════════════════════════════
    // General Errors
    // ══════════════════════════════════════════════════════════════════════════
    /// Invalid parameter value.
    #[error("Invalid parameter '{name}': {reason}")]
    InvalidParameter {
        /// Name of the parameter.
        name: String,
        /// Reason why it's invalid.
        reason: String,
    },

    /// Operation not supported.
    #[error("Operation not supported: {0}")]
    NotSupported(String),

    /// Internal error (should not happen in normal operation).
    #[error("Internal error: {0}")]
    Internal(String),
}

impl CoreError {
    /// Create an invalid vertex index error.
    #[must_use]
    pub fn invalid_vertex(index: usize, count: usize) -> Self {
        Self::InvalidVertexIndex { index, count }
    }

    /// Create an invalid edge index error.
    #[must_use]
    pub fn invalid_edge(index: usize, count: usize) -> Self {
        Self::InvalidEdgeIndex { index, count }
    }

    /// Create an invalid face index error.
    #[must_use]
    pub fn invalid_face(index: usize, count: usize) -> Self {
        Self::InvalidFaceIndex { index, count }
    }

    /// Create a degenerate geometry error.
    pub fn degenerate<S: Into<String>>(description: S) -> Self {
        Self::DegenerateGeometry {
            description: description.into(),
        }
    }

    /// Create an invalid topology error.
    pub fn invalid_topology<S: Into<String>>(description: S) -> Self {
        Self::InvalidTopology {
            description: description.into(),
        }
    }

    /// Create an I/O error.
    pub fn io_error(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::IoError {
            path: path.into(),
            source,
        }
    }

    /// Create an invalid parameter error.
    pub fn invalid_param<N: Into<String>, R: Into<String>>(name: N, reason: R) -> Self {
        Self::InvalidParameter {
            name: name.into(),
            reason: reason.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = CoreError::invalid_vertex(10, 5);
        assert!(err.to_string().contains("10"));
        assert!(err.to_string().contains("5"));
    }

    #[test]
    fn test_error_creation() {
        let err = CoreError::degenerate("zero-area face");
        assert!(err.to_string().contains("zero-area"));

        let err = CoreError::invalid_param("scale", "must be positive");
        assert!(err.to_string().contains("scale"));
        assert!(err.to_string().contains("positive"));
    }
}
