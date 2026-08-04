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

//! Boolean modifier.

use super::stack::{Modifier, ModifierMesh};
pub use crate::polygon::boolean::BooleanOp;
use std::any::Any;

/// Boolean modifier.
#[derive(Debug, Clone)]
pub struct BooleanModifier {
    pub name: String,
    pub enabled: bool,
    pub op: BooleanOp,
    /// The operand mesh to perform the operation with.
    pub operand: Option<ModifierMesh>,
}

impl Default for BooleanModifier {
    fn default() -> Self {
        Self {
            name: "Boolean".into(),
            enabled: true,
            op: BooleanOp::Union,
            operand: None,
        }
    }
}

impl Modifier for BooleanModifier {
    fn name(&self) -> &str {
        &self.name
    }
    fn type_id(&self) -> &'static str {
        "Boolean"
    }

    fn apply(&self, mesh: &ModifierMesh) -> ModifierMesh {
        if !self.enabled || self.operand.is_none() {
            return mesh.clone();
        }

        let operand = self.operand.as_ref().unwrap();

        // Convert ModifierMesh to BooleanMesh
        let mesh_a = crate::polygon::boolean::BooleanMesh::from_mesh(&mesh.positions, &mesh.faces);
        let mesh_b =
            crate::polygon::boolean::BooleanMesh::from_mesh(&operand.positions, &operand.faces);

        // Perform operation
        let result_bool = crate::polygon::boolean::BooleanMesh::apply(self.op, &mesh_a, &mesh_b);

        // Convert back to ModifierMesh
        let (positions, faces) = result_bool.to_mesh();
        ModifierMesh::from_geometry(positions, faces)
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }
    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
    fn clone_box(&self) -> Box<dyn Modifier> {
        Box::new(self.clone())
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
