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

//! Subdivision modifier.

use super::stack::{Modifier, ModifierMesh};
use crate::polygon::subdivision::{CatmullClark, SubdivisionMesh, SubdivisionSettings};
use std::any::Any;

/// Subdivision modifier.
#[derive(Debug, Clone)]
pub struct SubdivisionModifier {
    pub name: String,
    pub enabled: bool,
    pub levels: u32,
}

impl Default for SubdivisionModifier {
    fn default() -> Self {
        Self {
            name: "Subdivision".into(),
            enabled: true,
            levels: 1,
        }
    }
}

impl Modifier for SubdivisionModifier {
    fn name(&self) -> &str {
        &self.name
    }
    fn type_id(&self) -> &'static str {
        "Subdivision"
    }

    fn apply(&self, mesh: &ModifierMesh) -> ModifierMesh {
        if self.levels == 0 || !self.enabled {
            return mesh.clone();
        }

        // Convert ModifierMesh to SubdivisionMesh
        let mut sub_mesh = SubdivisionMesh::new();
        for p in &mesh.positions {
            sub_mesh.add_vertex(*p);
        }
        for face in &mesh.faces {
            sub_mesh.add_face(face.clone());
        }

        let settings = SubdivisionSettings {
            levels: self.levels as usize,
            ..Default::default()
        };

        let result_sub = CatmullClark::subdivide(&sub_mesh, &settings);

        // Convert back to ModifierMesh
        let mut result = ModifierMesh::new();
        result.positions = result_sub.positions;
        result.faces = result_sub.faces;
        result.normals = result_sub.normals;
        // UVs and other attributes could be interpolated here if needed

        result
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
