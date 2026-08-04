#![allow(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    missing_docs,
    unused_imports,
    dead_code
)]
//! # NAT3D Sync
//!
//! Device synchronization and distributed rendering.
//!
//! ## Modules
//!
//! - `discovery`: mDNS device discovery (iPads, Pencils, tablets)
//! - `protocol`: Device input protocol
//! - `input`: Input handling
//! - `streaming`: Data streaming
//! - `render_farm`: Distributed rendering system (Master-Worker architecture)
//! - `scene_crdt`: Multi-user scene synchronization

pub mod discovery;
pub mod input;
pub mod protocol;
pub mod render_farm;
pub mod scene_crdt;
