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

//! Adaptive job scheduler for render farm.
//!
//! Implements SDL-21 (Recursión Adaptativa): tile sizes adapt to worker capabilities.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::time::Instant;
use uuid::Uuid;

use super::protocol::{TileSpec, WorkerCapabilities};
use super::MAX_TILE_SIZE;

/// Job priority levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum JobPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Urgent = 3,
}

/// Render job.
#[derive(Debug, Clone)]
pub struct Job {
    pub id: Uuid,
    pub frame: u32,
    pub tile: TileSpec,
    pub priority: JobPriority,
    pub created_at: Instant,
}

impl Job {
    pub fn new(frame: u32, tile: TileSpec, priority: JobPriority) -> Self {
        Self {
            id: Uuid::new_v4(),
            frame,
            tile,
            priority,
            created_at: Instant::now(),
        }
    }
}

/// Wrapper for priority queue (max-heap on priority).
#[derive(Debug)]
struct PriorityJob(Job);

impl PartialEq for PriorityJob {
    fn eq(&self, other: &Self) -> bool {
        self.0.priority == other.0.priority && self.0.id == other.0.id
    }
}

impl Eq for PriorityJob {}

impl PartialOrd for PriorityJob {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PriorityJob {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority first, then older jobs first (FIFO within priority)
        self.0
            .priority
            .cmp(&other.0.priority)
            .then_with(|| other.0.created_at.cmp(&self.0.created_at))
    }
}

/// Worker state.
#[derive(Debug, Clone)]
pub struct WorkerState {
    pub id: Uuid,
    pub capabilities: WorkerCapabilities,
    pub current_job: Option<Uuid>,
    pub jobs_completed: usize,
    pub total_render_time_ms: u64,
    pub last_heartbeat: Instant,
}

impl WorkerState {
    pub fn new(id: Uuid, capabilities: WorkerCapabilities) -> Self {
        Self {
            id,
            capabilities,
            current_job: None,
            jobs_completed: 0,
            total_render_time_ms: 0,
            last_heartbeat: Instant::now(),
        }
    }

    /// Average render time per job in milliseconds
    pub fn avg_render_time_ms(&self) -> u64 {
        if self.jobs_completed == 0 {
            0
        } else {
            self.total_render_time_ms / self.jobs_completed as u64
        }
    }

    /// Is worker fast? (based on average render time)
    pub fn is_fast(&self) -> bool {
        if self.jobs_completed < 3 {
            // Assume fast until proven otherwise
            return true;
        }
        self.avg_render_time_ms() < 5000 // Less than 5 seconds per tile
    }
}

/// Job scheduler with adaptive tile splitting.
pub struct JobScheduler {
    /// Pending jobs (priority queue)
    pending_jobs: BinaryHeap<PriorityJob>,
    /// Jobs in progress (job_id → (worker_id, job))
    in_progress: HashMap<Uuid, (Uuid, Job)>,
    /// Workers (worker_id → state)
    workers: HashMap<Uuid, WorkerState>,
    /// Completed tiles per frame (frame → tiles)
    completed_tiles: HashMap<u32, Vec<TileSpec>>,
}

impl JobScheduler {
    pub fn new() -> Self {
        Self {
            pending_jobs: BinaryHeap::new(),
            in_progress: HashMap::new(),
            workers: HashMap::new(),
            completed_tiles: HashMap::new(),
        }
    }

    /// Register a new worker.
    pub fn register_worker(&mut self, worker_id: Uuid, capabilities: WorkerCapabilities) {
        let state = WorkerState::new(worker_id, capabilities);
        self.workers.insert(worker_id, state);
        tracing::info!(
            "Registered worker {} with GPU: {}, VRAM: {}MB, max tile: {}",
            worker_id,
            self.workers[&worker_id].capabilities.gpu_name,
            self.workers[&worker_id].capabilities.vram_mb,
            self.workers[&worker_id].capabilities.max_tile_size
        );
    }

    /// Unregister a worker.
    pub fn unregister_worker(&mut self, worker_id: Uuid) {
        self.workers.remove(&worker_id);
        tracing::info!("Unregistered worker {}", worker_id);
    }

    /// Get list of registered workers.
    pub fn get_workers(&self) -> Vec<Uuid> {
        self.workers.keys().copied().collect()
    }

    /// Split a frame into jobs based on worker capabilities.
    ///
    /// Implements SDL-21 (Recursión Adaptativa): tile size adapts to GPU power.
    pub fn split_frame(
        &self,
        frame: u32,
        resolution: (u32, u32),
        priority: JobPriority,
    ) -> Vec<Job> {
        let (width, height) = resolution;

        // Determine tile size based on available workers
        let tile_size = self.determine_tile_size();

        let mut jobs = Vec::new();

        for y in (0..height).step_by(tile_size as usize) {
            for x in (0..width).step_by(tile_size as usize) {
                let tile_width = (tile_size).min(width - x);
                let tile_height = (tile_size).min(height - y);

                let tile = TileSpec::new(x, y, tile_width, tile_height);
                let job = Job::new(frame, tile, priority);
                jobs.push(job);
            }
        }

        tracing::info!(
            "Split frame {} ({}x{}) into {} jobs with tile size {}",
            frame,
            width,
            height,
            jobs.len(),
            tile_size
        );

        jobs
    }

    /// Determine optimal tile size based on worker capabilities (adaptive).
    fn determine_tile_size(&self) -> u32 {
        if self.workers.is_empty() {
            return 256; // Default
        }

        // Get average max_tile_size from all workers
        let avg_max_tile: u32 = self
            .workers
            .values()
            .map(|w| w.capabilities.max_tile_size)
            .sum::<u32>()
            / self.workers.len() as u32;

        // Get average VRAM
        let avg_vram: u32 = self
            .workers
            .values()
            .map(|w| w.capabilities.vram_mb)
            .sum::<u32>()
            / self.workers.len() as u32;

        // Adaptive tile size based on VRAM:
        // - High VRAM (>8GB): 1024×1024
        // - Mid VRAM (4-8GB): 512×512
        // - Low VRAM (<4GB): 256×256
        let adaptive_size = if avg_vram >= 8000 {
            1024
        } else if avg_vram >= 4000 {
            512
        } else {
            256
        };

        // Use minimum of adaptive size and worker max_tile_size
        adaptive_size.min(avg_max_tile).min(MAX_TILE_SIZE)
    }

    /// Submit jobs to queue.
    pub fn submit_jobs(&mut self, jobs: Vec<Job>) {
        for job in jobs {
            self.pending_jobs.push(PriorityJob(job));
        }
    }

    /// Assign a job to a worker.
    ///
    /// Returns the highest-priority pending job that matches worker capabilities.
    pub fn assign_job(&mut self, worker_id: Uuid) -> Option<Job> {
        let worker = self.workers.get(&worker_id)?;

        // Check if worker already has a job
        if worker.current_job.is_some() {
            return None;
        }

        // Find highest-priority job that fits worker's max_tile_size
        let max_tile_size = worker.capabilities.max_tile_size;

        // Drain heap to temporary vec to search
        let mut temp = Vec::new();
        let mut found_job = None;

        while let Some(PriorityJob(job)) = self.pending_jobs.pop() {
            let tile_fits = job.tile.width <= max_tile_size && job.tile.height <= max_tile_size;

            if found_job.is_none() && tile_fits {
                found_job = Some(job);
            } else {
                temp.push(job);
            }
        }

        // Put unassigned jobs back
        for job in temp {
            self.pending_jobs.push(PriorityJob(job));
        }

        // Assign job if found
        if let Some(job) = found_job {
            let job_id = job.id;
            self.in_progress.insert(job_id, (worker_id, job.clone()));
            self.workers.get_mut(&worker_id).unwrap().current_job = Some(job_id);
            Some(job)
        } else {
            None
        }
    }

    /// Handle job completion.
    pub fn handle_job_complete(&mut self, job_id: Uuid, render_time_ms: u64) {
        if let Some((worker_id, job)) = self.in_progress.remove(&job_id) {
            // Update worker stats
            if let Some(worker) = self.workers.get_mut(&worker_id) {
                worker.current_job = None;
                worker.jobs_completed += 1;
                worker.total_render_time_ms += render_time_ms;
            }

            // Track completed tile
            self.completed_tiles
                .entry(job.frame)
                .or_default()
                .push(job.tile);

            tracing::debug!(
                "Job {} completed by worker {} in {}ms",
                job_id,
                worker_id,
                render_time_ms
            );
        }
    }

    /// Handle job failure (requeue).
    pub fn handle_job_failure(&mut self, job_id: Uuid) {
        if let Some((worker_id, job)) = self.in_progress.remove(&job_id) {
            // Clear worker's current job
            if let Some(worker) = self.workers.get_mut(&worker_id) {
                worker.current_job = None;
            }

            // Requeue job
            self.pending_jobs.push(PriorityJob(job.clone()));

            tracing::warn!("Job {} failed, requeued", job_id);
        }
    }

    /// Handle worker timeout (reassign its jobs).
    pub fn handle_worker_timeout(&mut self, worker_id: Uuid) {
        // Find all jobs assigned to this worker
        let mut jobs_to_requeue = Vec::new();

        for (job_id, (wid, job)) in &self.in_progress {
            if *wid == worker_id {
                jobs_to_requeue.push((*job_id, job.clone()));
            }
        }

        // Store count before consuming vector
        let requeued_count = jobs_to_requeue.len();

        // Requeue jobs
        for (job_id, job) in jobs_to_requeue {
            self.in_progress.remove(&job_id);
            self.pending_jobs.push(PriorityJob(job));
        }

        // Remove worker
        self.workers.remove(&worker_id);

        tracing::warn!(
            "Worker {} timed out, {} jobs requeued",
            worker_id,
            requeued_count
        );
    }

    /// Check if frame is complete (all tiles rendered).
    pub fn is_frame_complete(&self, frame: u32, resolution: (u32, u32)) -> bool {
        let (width, height) = resolution;
        let tile_size = self.determine_tile_size();

        let expected_tiles = width.div_ceil(tile_size) * height.div_ceil(tile_size);

        if let Some(completed) = self.completed_tiles.get(&frame) {
            completed.len() >= expected_tiles as usize
        } else {
            false
        }
    }

    /// Get completed tiles for a frame.
    pub fn get_completed_tiles(&self, frame: u32) -> Option<&Vec<TileSpec>> {
        self.completed_tiles.get(&frame)
    }

    /// Get pending job count.
    pub fn pending_count(&self) -> usize {
        self.pending_jobs.len()
    }

    /// Get in-progress job count.
    pub fn in_progress_count(&self) -> usize {
        self.in_progress.len()
    }

    /// Update worker heartbeat.
    pub fn update_heartbeat(&mut self, worker_id: Uuid) {
        if let Some(worker) = self.workers.get_mut(&worker_id) {
            worker.last_heartbeat = Instant::now();
        }
    }

    /// Get workers that haven't sent heartbeat in timeout duration.
    pub fn get_timed_out_workers(&self, timeout: std::time::Duration) -> Vec<Uuid> {
        self.workers
            .iter()
            .filter(|(_, worker)| worker.last_heartbeat.elapsed() > timeout)
            .map(|(id, _)| *id)
            .collect()
    }
}

impl Default for JobScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_capabilities(vram_mb: u32, max_tile_size: u32) -> WorkerCapabilities {
        WorkerCapabilities::new("Test GPU".to_string(), vram_mb, 8, max_tile_size)
    }

    #[test]
    fn test_job_creation() {
        let tile = TileSpec::new(0, 0, 512, 512);
        let job = Job::new(0, tile, JobPriority::Normal);

        assert_eq!(job.frame, 0);
        assert_eq!(job.tile, tile);
        assert_eq!(job.priority, JobPriority::Normal);
    }

    #[test]
    fn test_scheduler_creation() {
        let scheduler = JobScheduler::new();
        assert_eq!(scheduler.pending_count(), 0);
        assert_eq!(scheduler.in_progress_count(), 0);
    }

    #[test]
    fn test_register_worker() {
        let mut scheduler = JobScheduler::new();
        let worker_id = Uuid::new_v4();
        let caps = create_test_capabilities(4096, 512);

        scheduler.register_worker(worker_id, caps);

        assert_eq!(scheduler.get_workers().len(), 1);
    }

    #[test]
    fn test_unregister_worker() {
        let mut scheduler = JobScheduler::new();
        let worker_id = Uuid::new_v4();
        let caps = create_test_capabilities(4096, 512);

        scheduler.register_worker(worker_id, caps);
        scheduler.unregister_worker(worker_id);

        assert_eq!(scheduler.get_workers().len(), 0);
    }

    #[test]
    fn test_split_frame_basic() {
        let scheduler = JobScheduler::new();
        let jobs = scheduler.split_frame(0, (1920, 1080), JobPriority::Normal);

        // With no workers, uses default tile size 256
        // 1920/256 = 8 tiles wide, 1080/256 = 5 tiles high = 40 tiles
        assert!(jobs.len() > 0);
    }

    #[test]
    fn test_split_frame_with_workers() {
        let mut scheduler = JobScheduler::new();

        // Register worker with high VRAM (should use 1024 tiles)
        let worker_id = Uuid::new_v4();
        let caps = create_test_capabilities(8192, 1024);
        scheduler.register_worker(worker_id, caps);

        let jobs = scheduler.split_frame(0, (2048, 2048), JobPriority::Normal);

        // With 8GB VRAM, should use 1024×1024 tiles
        // 2048/1024 = 2×2 = 4 tiles
        assert_eq!(jobs.len(), 4);
    }

    #[test]
    fn test_adaptive_tile_size() {
        let mut scheduler = JobScheduler::new();

        // Low VRAM worker
        let worker1 = Uuid::new_v4();
        let caps1 = create_test_capabilities(2048, 256);
        scheduler.register_worker(worker1, caps1);

        let tile_size = scheduler.determine_tile_size();
        assert_eq!(tile_size, 256); // Should use 256 for low VRAM

        // Add high VRAM worker (average should increase)
        let worker2 = Uuid::new_v4();
        let caps2 = create_test_capabilities(8192, 1024);
        scheduler.register_worker(worker2, caps2);

        let tile_size = scheduler.determine_tile_size();
        // Average VRAM = (2048 + 8192) / 2 = 5120 → should be 512
        assert_eq!(tile_size, 512);
    }

    #[test]
    fn test_submit_and_assign_job() {
        let mut scheduler = JobScheduler::new();

        // Register worker
        let worker_id = Uuid::new_v4();
        let caps = create_test_capabilities(4096, 512);
        scheduler.register_worker(worker_id, caps);

        // Submit jobs
        let jobs = scheduler.split_frame(0, (1024, 1024), JobPriority::Normal);
        scheduler.submit_jobs(jobs);

        // Assign job
        let job = scheduler.assign_job(worker_id);
        assert!(job.is_some());
        assert_eq!(scheduler.in_progress_count(), 1);
        assert!(scheduler.pending_count() > 0);
    }

    #[test]
    fn test_job_priority_ordering() {
        let mut scheduler = JobScheduler::new();

        // Submit jobs with different priorities
        let tile = TileSpec::new(0, 0, 512, 512);
        let jobs = vec![
            Job::new(0, tile, JobPriority::Low),
            Job::new(1, tile, JobPriority::Urgent),
            Job::new(2, tile, JobPriority::Normal),
            Job::new(3, tile, JobPriority::High),
        ];
        scheduler.submit_jobs(jobs);

        // Register worker
        let worker_id = Uuid::new_v4();
        let caps = create_test_capabilities(4096, 512);
        scheduler.register_worker(worker_id, caps);

        // Assign should get Urgent first
        let job1 = scheduler.assign_job(worker_id).unwrap();
        assert_eq!(job1.frame, 1); // Urgent

        scheduler.handle_job_complete(job1.id, 1000);

        let job2 = scheduler.assign_job(worker_id).unwrap();
        assert_eq!(job2.frame, 3); // High
    }

    #[test]
    fn test_job_completion() {
        let mut scheduler = JobScheduler::new();

        let worker_id = Uuid::new_v4();
        let caps = create_test_capabilities(4096, 512);
        scheduler.register_worker(worker_id, caps);

        let jobs = scheduler.split_frame(0, (512, 512), JobPriority::Normal);
        scheduler.submit_jobs(jobs);

        let job = scheduler.assign_job(worker_id).unwrap();
        let job_id = job.id;

        scheduler.handle_job_complete(job_id, 1000);

        assert_eq!(scheduler.in_progress_count(), 0);
        assert_eq!(scheduler.workers[&worker_id].jobs_completed, 1);
    }

    #[test]
    fn test_job_failure_requeue() {
        let mut scheduler = JobScheduler::new();

        let worker_id = Uuid::new_v4();
        let caps = create_test_capabilities(4096, 512);
        scheduler.register_worker(worker_id, caps);

        let jobs = scheduler.split_frame(0, (512, 512), JobPriority::Normal);
        let initial_count = jobs.len();
        scheduler.submit_jobs(jobs);

        let job = scheduler.assign_job(worker_id).unwrap();
        let job_id = job.id;

        scheduler.handle_job_failure(job_id);

        // Job should be requeued
        assert_eq!(scheduler.pending_count(), initial_count);
        assert_eq!(scheduler.in_progress_count(), 0);
    }

    #[test]
    fn test_worker_timeout() {
        let mut scheduler = JobScheduler::new();

        let worker_id = Uuid::new_v4();
        let caps = create_test_capabilities(4096, 512);
        scheduler.register_worker(worker_id, caps);

        let jobs = scheduler.split_frame(0, (512, 512), JobPriority::Normal);
        let initial_count = jobs.len();
        scheduler.submit_jobs(jobs);

        let _ = scheduler.assign_job(worker_id);

        scheduler.handle_worker_timeout(worker_id);

        // Worker removed, job requeued
        assert_eq!(scheduler.get_workers().len(), 0);
        assert_eq!(scheduler.pending_count(), initial_count);
    }

    #[test]
    fn test_frame_completion_detection() {
        let mut scheduler = JobScheduler::new();

        let worker_id = Uuid::new_v4();
        let caps = create_test_capabilities(4096, 512);
        scheduler.register_worker(worker_id, caps);

        let resolution = (512, 512);
        let jobs = scheduler.split_frame(0, resolution, JobPriority::Normal);
        let job_count = jobs.len();
        scheduler.submit_jobs(jobs);

        // Complete all jobs
        for _ in 0..job_count {
            if let Some(job) = scheduler.assign_job(worker_id) {
                scheduler.handle_job_complete(job.id, 1000);
            }
        }

        assert!(scheduler.is_frame_complete(0, resolution));
    }
}
