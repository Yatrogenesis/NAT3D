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

//! PyO3 bindings for NAT3D.

// PyO3 structs expose fields to Python via #[pyo3(get, set)] attributes.
// Documentation is provided via Python docstrings, not Rust doc comments.
#![allow(missing_docs)]

use nat3d_core::material::Color;
use nat3d_core::{Material, Mesh};
use nat3d_math::glam::{Quat, Vec3};
use nat3d_math::nalgebra::Point3;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use std::collections::HashMap;

#[pyclass]
#[derive(Debug, Clone)]
pub struct PyVector3 {
    #[pyo3(get, set)]
    pub x: f32,
    #[pyo3(get, set)]
    pub y: f32,
    #[pyo3(get, set)]
    pub z: f32,
}

#[pymethods]
impl PyVector3 {
    #[new]
    fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
    fn __repr__(&self) -> String {
        format!("Vector3({}, {}, {})", self.x, self.y, self.z)
    }
}

#[pyclass]
#[derive(Clone)]
pub struct PyMesh {
    inner: Mesh,
}

#[pymethods]
impl PyMesh {
    #[new]
    fn new() -> Self {
        Self {
            inner: Mesh::new("mesh".to_string()),
        }
    }
    fn vertex_count(&self) -> usize {
        self.inner.vertex_count()
    }
    fn face_count(&self) -> usize {
        self.inner.face_count()
    }
    fn __repr__(&self) -> String {
        format!(
            "Mesh(v={}, f={})",
            self.inner.vertex_count(),
            self.inner.face_count()
        )
    }
}

#[pyclass]
pub struct PyScene {
    objects: HashMap<String, PyMesh>,
    selected: Option<String>,
}

#[pymethods]
impl PyScene {
    #[new]
    fn new() -> Self {
        Self {
            objects: HashMap::new(),
            selected: None,
        }
    }
    fn list_objects(&self) -> Vec<String> {
        self.objects.keys().cloned().collect()
    }
}

#[pyclass]
pub struct PyMaterial {
    inner: Material,
}

#[pymethods]
impl PyMaterial {
    #[new]
    fn new(name: String) -> Self {
        Self {
            inner: Material::new(name),
        }
    }
}

#[pyclass]
pub struct PyModifier {
    name: String,
}

#[pymethods]
impl PyModifier {
    #[new]
    fn new(name: String) -> Self {
        Self { name }
    }
}

#[pyfunction]
fn create_object(obj_type: &str, name: &str) -> PyResult<String> {
    if let Some(host) = crate::GLOBAL_HOST.read().as_ref() {
        host.create_object(obj_type, name);
        Ok(format!("Created {}", name))
    } else {
        Ok("Error: No Host".to_string())
    }
}

#[pyfunction]
fn delete_object(_name: &str) -> PyResult<bool> {
    Ok(true)
}

#[pyfunction]
fn translate(_name: &str, _x: f32, _y: f32, _z: f32) -> PyResult<()> {
    Ok(())
}

#[pyfunction]
fn export(_path: &str, _format: &str) -> PyResult<()> {
    Ok(())
}

#[pymodule]
fn nat3d(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyVector3>()?;
    m.add_class::<PyMesh>()?;
    m.add_class::<PyScene>()?;
    m.add_class::<PyMaterial>()?;
    m.add_class::<PyModifier>()?;
    m.add_function(wrap_pyfunction!(create_object, m)?)?;
    m.add_function(wrap_pyfunction!(delete_object, m)?)?;
    m.add_function(wrap_pyfunction!(translate, m)?)?;
    m.add_function(wrap_pyfunction!(export, m)?)?;
    Ok(())
}
