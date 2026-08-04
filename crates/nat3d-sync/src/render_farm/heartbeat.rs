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

//! Heartbeat monitoring for failure detection.
//!
//! Production-ready implementation for worker health tracking.

use std::collections::HashMap;
use std::time::{Duration, Instant};
use uuid::Uuid;

use super::WORKER_TIMEOUT_SECS;

/// Heartbeat monitor.
pub struct HeartbeatMonitor {
    last_seen: HashMap<Uuid, Instant>,
    timeout: Duration,
}

impl HeartbeatMonitor {
    pub fn new() -> Self {
        Self {
            last_seen: HashMap::new(),
            timeout: Duration::from_secs(WORKER_TIMEOUT_SECS),
        }
    }

    pub fn record_heartbeat(&mut self, worker_id: Uuid) {
        self.last_seen.insert(worker_id, Instant::now());
    }

    pub fn check_timeouts(&self) -> Vec<Uuid> {
        self.check_timeouts_with_duration(self.timeout)
    }

    pub fn check_timeouts_with_duration(&self, timeout: Duration) -> Vec<Uuid> {
        self.last_seen
            .iter()
            .filter(|(_, last)| last.elapsed() > timeout)
            .map(|(id, _)| *id)
            .collect()
    }
}

impl Default for HeartbeatMonitor {
    fn default() -> Self {
        Self::new()
    }
}
