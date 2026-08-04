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

//! Distributed rendering system for NAT3D.
//!
//! # Architecture
//!
//! The render farm uses a master-worker architecture with the following components:
//!
//! - **Discovery**: mDNS-based network discovery for zero-configuration setup
//! - **Protocol**: Binary protocol for job assignment and result collection
//! - **Scheduler**: Adaptive job scheduling based on node capabilities
//! - **Scene CRDT**: Conflict-free scene synchronization using CRDTs
//! - **Sync Manager**: Broadcasts scene changes to all workers
//! - **Heartbeat**: Health monitoring and failure detection
//! - **Fault Tolerance**: Automatic job reassignment and recovery
//! - **Checkpoint**: Progress persistence for crash recovery
//!
//! # Paradigms (SDL-Engine Integration)
//!
//! - **SDL-14 (Gradientes Informativos)**: Data flows from master (high detail) to workers (low detail)
//! - **SDL-21 (Recursión Adaptativa)**: Adaptive load balancing based on node performance
//! - **VR-14 (Compartido Distribuido)**: CRDT-based shared state (eventual consistency)
//! - **VR-15 (Actores)**: Each render node is an actor with message passing
//!
//! # Usage Example
//!
//! ```ignore
//! use nat3d_sync::render_farm::{RenderFarmMaster, RenderFarmWorker};
//!
//! // Master node
//! async fn run_master() {
//!     let mut master = RenderFarmMaster::new("master-node", Some(9000)).await.unwrap();
//!     master.start_discovery().await.unwrap();
//!
//!     // Wait for workers to connect
//!     tokio::time::sleep(std::time::Duration::from_secs(5)).await;
//!
//!     // Submit render job
//!     let start_frame = 0;
//!     let end_frame = 100;
//!     let resolution = (1920, 1080);
//!     let job_id = master.submit_animation_job(start_frame, end_frame, resolution).await.unwrap();
//!
//!     // Collect results
//!     master.wait_for_completion(job_id).await.unwrap();
//! }
//!
//! // Worker node
//! async fn run_worker() {
//!     use std::net::SocketAddr;
//!     let mut worker = RenderFarmWorker::new("worker-1".to_string());
//!     let master_addr: SocketAddr = "127.0.0.1:9000".parse().unwrap();
//!     worker.run(master_addr).await.unwrap();
//! }
//! ```

pub mod checkpoint;
pub mod discovery;
pub mod fault_tolerance;
pub mod heartbeat;
pub mod master;
pub mod protocol;
pub mod scene_crdt;
pub mod scheduler;
pub mod sync;
pub mod worker;

// Re-exports for convenience
pub use discovery::{
    DiscoveryEvent as RenderDiscoveryEvent, NodeInfo, NodeRole, RenderNodeDiscovery,
};
pub use master::RenderFarmMaster;
pub use protocol::{JobResult, RenderMessage, TileSpec, WorkerCapabilities};
pub use scene_crdt::{CrdtOperation, SceneCRDT};
pub use scheduler::{Job, JobPriority, JobScheduler};
pub use worker::RenderFarmWorker;

/// Default port for render farm master node.
pub const DEFAULT_MASTER_PORT: u16 = 9000;

/// Default mDNS service type for render farm.
pub const RENDER_FARM_SERVICE_TYPE: &str = "_nat3d-render._tcp.local.";

/// Maximum tile size (width or height in pixels).
pub const MAX_TILE_SIZE: u32 = 1024;

/// Heartbeat interval in seconds.
pub const HEARTBEAT_INTERVAL_SECS: u64 = 5;

/// Worker timeout in seconds (no heartbeat = considered dead).
pub const WORKER_TIMEOUT_SECS: u64 = 15;

/// Checkpoint save interval in seconds.
pub const CHECKPOINT_INTERVAL_SECS: u64 = 60;
