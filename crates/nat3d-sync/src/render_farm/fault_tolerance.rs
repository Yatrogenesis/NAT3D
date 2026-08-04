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

//! Fault tolerance and recovery.

use std::time::Duration;
use uuid::Uuid;

use super::heartbeat::HeartbeatMonitor;
use super::scheduler::JobScheduler;

/// Fault handler integrating heartbeat monitoring and job reassignment.
pub struct FaultHandler {
    pub scheduler: JobScheduler,
    pub heartbeat: HeartbeatMonitor,
}

impl FaultHandler {
    pub fn new() -> Self {
        Self {
            scheduler: JobScheduler::new(),
            heartbeat: HeartbeatMonitor::new(),
        }
    }

    /// Handle worker failure - reassign jobs and remove worker.
    pub fn handle_worker_failure(&mut self, failed_worker: Uuid) {
        tracing::warn!("Handling failure of worker {}", failed_worker);
        self.scheduler.handle_worker_timeout(failed_worker);
        // Heartbeat will be naturally removed when worker doesn't respond
    }

    /// Check for timed out workers and handle failures.
    pub fn check_and_handle_timeouts(&mut self, timeout: Duration) -> Vec<Uuid> {
        let timed_out = self.heartbeat.check_timeouts_with_duration(timeout);

        for worker_id in &timed_out {
            self.handle_worker_failure(*worker_id);
        }

        timed_out
    }

    /// Record heartbeat from worker.
    pub fn record_heartbeat(&mut self, worker_id: Uuid) {
        self.heartbeat.record_heartbeat(worker_id);
        self.scheduler.update_heartbeat(worker_id);
    }
}

impl Default for FaultHandler {
    fn default() -> Self {
        Self::new()
    }
}
