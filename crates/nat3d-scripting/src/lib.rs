#![allow(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    missing_docs,
    unused_imports,
    dead_code,
    clippy::duplicated_attributes
)]
//! # NAT3D Scripting
//! Python bindings and Rust API.

use lazy_static::lazy_static;
use parking_lot::RwLock;
use std::sync::Arc;

pub mod ai;
pub mod macros;
pub mod python;
pub mod rust_api;

/// Interface for the host application to handle scripting requests.
pub trait ScriptingHost: Send + Sync {
    /// Create a new object in the scene.
    fn create_object(&self, obj_type: &str, name: &str);
    /// Delete an object from the scene.
    fn delete_object(&self, name: &str);
    /// Translate an object.
    fn translate_object(&self, name: &str, x: f32, y: f32, z: f32);
}

lazy_static! {
    /// Global scripting host instance.
    pub static ref GLOBAL_HOST: RwLock<Option<Arc<dyn ScriptingHost>>> = RwLock::new(None);
}
