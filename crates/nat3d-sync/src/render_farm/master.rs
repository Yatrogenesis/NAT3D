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

//! Master node implementation - coordinates render farm.

use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time;
use uuid::Uuid;

use super::checkpoint::CheckpointManager;
use super::discovery::{NodeRole, RenderNodeDiscovery};
use super::heartbeat::HeartbeatMonitor;
use super::protocol::{JobResult, RenderClient, RenderMessage, RenderServer, WorkerCapabilities};
use super::scheduler::{JobPriority, JobScheduler};
use super::sync::SyncManager;
use super::{DEFAULT_MASTER_PORT, HEARTBEAT_INTERVAL_SECS, WORKER_TIMEOUT_SECS};

/// Render farm master node.
pub struct RenderFarmMaster {
    node_id: Uuid,
    discovery: RenderNodeDiscovery,
    scheduler: JobScheduler,
    sync_manager: SyncManager,
    heartbeat: HeartbeatMonitor,
    checkpoint: CheckpointManager,
    port: u16,
    completed_frames: HashMap<u32, Vec<JobResult>>,
    worker_clients: HashMap<Uuid, RenderClient>,
}

impl RenderFarmMaster {
    /// Create a new master node.
    pub async fn new(name: &str, port: Option<u16>) -> anyhow::Result<Self> {
        let node_id = Uuid::new_v4();
        let port = port.unwrap_or(DEFAULT_MASTER_PORT);

        let discovery = RenderNodeDiscovery::new(node_id, NodeRole::Master, name.to_string())?;

        let checkpoint_dir = std::env::temp_dir().join("nat3d_render_farm");

        Ok(Self {
            node_id,
            discovery,
            scheduler: JobScheduler::new(),
            sync_manager: SyncManager::new(node_id),
            heartbeat: HeartbeatMonitor::new(),
            checkpoint: CheckpointManager::new(checkpoint_dir),
            port,
            completed_frames: HashMap::new(),
            worker_clients: HashMap::new(),
        })
    }

    /// Start discovery and register service.
    pub async fn start_discovery(&mut self) -> anyhow::Result<()> {
        // Register this master on the network
        self.discovery.register_service(self.port, None)?;

        // Start discovering workers (spawn task to handle events)
        let _rx = self.discovery.start_discovery().await?;

        tracing::info!("Master node started on port {}", self.port);

        Ok(())
    }

    /// Submit an animation render job.
    pub async fn submit_animation_job(
        &mut self,
        start_frame: u32,
        end_frame: u32,
        resolution: (u32, u32),
    ) -> anyhow::Result<Uuid> {
        let job_id = Uuid::new_v4();

        tracing::info!(
            "Submitting animation job {} (frames {}-{}, {}x{})",
            job_id,
            start_frame,
            end_frame,
            resolution.0,
            resolution.1
        );

        // Split each frame into tiles
        for frame in start_frame..=end_frame {
            let jobs = self
                .scheduler
                .split_frame(frame, resolution, JobPriority::Normal);
            self.scheduler.submit_jobs(jobs);
        }

        Ok(job_id)
    }

    /// Run the master node (main loop).
    pub async fn run(&mut self) -> anyhow::Result<()> {
        // Start TCP server
        let server = RenderServer::bind(format!("0.0.0.0:{}", self.port).parse()?).await?;
        tracing::info!("Master listening on port {}", self.port);

        // Spawn connection handler
        let (worker_tx, mut worker_rx) = mpsc::channel(100);

        tokio::spawn(async move {
            loop {
                match server.accept().await {
                    Ok((stream, addr)) => {
                        tracing::info!("Worker connected from {}", addr);
                        let _ = worker_tx.send((stream, addr)).await;
                    }
                    Err(e) => {
                        tracing::error!("Accept error: {}", e);
                        break;
                    }
                }
            }
        });

        // Main loop
        let mut heartbeat_interval = time::interval(Duration::from_secs(HEARTBEAT_INTERVAL_SECS));
        let mut sync_interval = time::interval(Duration::from_millis(100));

        loop {
            tokio::select! {
                _ = heartbeat_interval.tick() => {
                    // Check for timed out workers
                    let timeout = Duration::from_secs(WORKER_TIMEOUT_SECS);
                    let timed_out = self.scheduler.get_timed_out_workers(timeout);
                    for worker_id in timed_out {
                        tracing::warn!("Worker {} timed out", worker_id);
                        self.scheduler.handle_worker_timeout(worker_id);
                        self.worker_clients.remove(&worker_id);
                    }
                }

                _ = sync_interval.tick() => {
                    // Scene changes are sent with job assignments (JobAssign.scene_diff)
                    // No separate broadcast needed - each worker gets ops with their next job
                }

                Some((stream, addr)) = worker_rx.recv() => {
                    // Handle new worker connection
                    tracing::info!("New worker connection from {}", addr);
                    let client = RenderClient::from_stream(stream);
                    self.handle_new_worker(client).await?;
                }
            }
        }
    }

    /// Handle new worker connection and registration.
    async fn handle_new_worker(&mut self, mut client: RenderClient) -> anyhow::Result<()> {
        // Wait for worker registration message
        match tokio::time::timeout(Duration::from_secs(5), client.recv()).await {
            Ok(Ok(RenderMessage::RegisterWorker {
                worker_id,
                capabilities,
            })) => {
                tracing::info!(
                    "Worker {} registered: {} ({} MB VRAM)",
                    worker_id,
                    capabilities.gpu_name,
                    capabilities.vram_mb
                );

                // Register with scheduler
                self.register_worker(worker_id, capabilities);

                // Store client connection
                self.worker_clients.insert(worker_id, client);

                // Try to assign first job
                if let Some(job) = self.scheduler.assign_job(worker_id) {
                    self.send_job_to_worker(worker_id, job).await?;
                }

                Ok(())
            }
            Ok(Ok(other)) => {
                tracing::warn!("Expected RegisterWorker, got {:?}", other);
                Err(anyhow::anyhow!("Invalid registration message"))
            }
            Ok(Err(e)) => {
                tracing::error!("Registration receive error: {}", e);
                Err(e.into())
            }
            Err(_) => {
                tracing::error!("Worker registration timeout");
                Err(anyhow::anyhow!("Registration timeout"))
            }
        }
    }

    /// Send job assignment to worker.
    async fn send_job_to_worker(
        &mut self,
        worker_id: Uuid,
        job: super::scheduler::Job,
    ) -> anyhow::Result<()> {
        let Some(client) = self.worker_clients.get_mut(&worker_id) else {
            return Err(anyhow::anyhow!("Worker {} not connected", worker_id));
        };

        // Get pending scene operations
        let scene_diff = self.sync_manager.get_pending_ops();

        let message = RenderMessage::JobAssign {
            job_id: job.id,
            frame: job.frame,
            tile: job.tile,
            scene_diff,
            priority: job.priority as u8,
        };

        client.send(&message).await?;
        tracing::debug!("Sent job {} to worker {}", job.id, worker_id);

        Ok(())
    }

    /// Wait for job completion (simplified).
    pub async fn wait_for_completion(&mut self, _job_id: Uuid) -> anyhow::Result<()> {
        // TODO: Implement proper completion tracking
        tracing::info!("Waiting for job completion...");
        Ok(())
    }

    /// Register a worker.
    pub fn register_worker(&mut self, worker_id: Uuid, capabilities: WorkerCapabilities) {
        self.scheduler.register_worker(worker_id, capabilities);
        self.heartbeat.record_heartbeat(worker_id);
    }

    /// Handle job completion from worker.
    pub async fn handle_job_complete(&mut self, worker_id: Uuid, job_id: Uuid, result: JobResult) {
        self.scheduler
            .handle_job_complete(job_id, result.render_time_ms);

        tracing::info!(
            "Worker {} completed job {} (frame {}) in {} ms",
            worker_id,
            job_id,
            result.frame,
            result.render_time_ms
        );

        // Store completed tile
        self.completed_frames
            .entry(result.frame)
            .or_default()
            .push(result);

        // Try to assign next job to worker
        if let Some(job) = self.scheduler.assign_job(worker_id) {
            tracing::debug!("Assigned job {} to worker {}", job.id, worker_id);
            if let Err(e) = self.send_job_to_worker(worker_id, job).await {
                tracing::error!("Failed to send job to worker {}: {}", worker_id, e);
            }
        }
    }

    pub fn get_scheduler(&self) -> &JobScheduler {
        &self.scheduler
    }

    pub fn get_scheduler_mut(&mut self) -> &mut JobScheduler {
        &mut self.scheduler
    }
}
