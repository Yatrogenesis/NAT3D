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

//! Cloud rendering client.
//!
//! Manages connections to cloud rendering services and job submission.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Cloud service provider type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudProvider {
    /// Amazon Web Services
    Aws,
    /// Google Cloud Platform
    Gcp,
    /// Microsoft Azure
    Azure,
    /// Custom/self-hosted
    Custom,
}

impl std::fmt::Display for CloudProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Aws => write!(f, "AWS"),
            Self::Gcp => write!(f, "GCP"),
            Self::Azure => write!(f, "Azure"),
            Self::Custom => write!(f, "Custom"),
        }
    }
}

/// Cloud client configuration.
#[derive(Debug, Clone)]
pub struct CloudConfig {
    /// Cloud provider
    pub provider: CloudProvider,
    /// API endpoint URL
    pub endpoint: String,
    /// API key or token
    pub api_key: String,
    /// Region
    pub region: String,
    /// Connection timeout
    pub timeout: Duration,
    /// Max retries
    pub max_retries: u32,
    /// Instance type for rendering
    pub instance_type: String,
    /// Enable GPU instances
    pub gpu_enabled: bool,
}

impl Default for CloudConfig {
    fn default() -> Self {
        Self {
            provider: CloudProvider::Custom,
            endpoint: "https://api.nat3d.cloud".to_string(),
            api_key: String::new(),
            region: "us-east-1".to_string(),
            timeout: Duration::from_secs(30),
            max_retries: 3,
            instance_type: "standard".to_string(),
            gpu_enabled: true,
        }
    }
}

/// Connection status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Error,
}

/// Cloud client for managing render jobs.
pub struct CloudClient {
    config: CloudConfig,
    status: ConnectionStatus,
    session_id: Option<String>,
    last_ping: Option<Instant>,
    jobs: HashMap<String, super::job::RenderJob>,
}

impl CloudClient {
    /// Create a new cloud client.
    pub fn new(config: CloudConfig) -> Self {
        Self {
            config,
            status: ConnectionStatus::Disconnected,
            session_id: None,
            last_ping: None,
            jobs: HashMap::new(),
        }
    }

    /// Get current status.
    pub fn status(&self) -> ConnectionStatus {
        self.status
    }

    /// Check if connected.
    pub fn is_connected(&self) -> bool {
        self.status == ConnectionStatus::Connected
    }

    /// Get provider.
    pub fn provider(&self) -> CloudProvider {
        self.config.provider
    }

    /// Connect to the cloud service.
    pub async fn connect(&mut self) -> Result<(), CloudError> {
        self.status = ConnectionStatus::Connecting;

        // Validate config
        if self.config.api_key.is_empty() {
            self.status = ConnectionStatus::Error;
            return Err(CloudError::AuthenticationFailed(
                "API key required".to_string(),
            ));
        }

        // Simulate connection
        self.status = ConnectionStatus::Connected;
        self.session_id = Some(generate_session_id());
        self.last_ping = Some(Instant::now());

        Ok(())
    }

    /// Disconnect from the cloud service.
    pub async fn disconnect(&mut self) {
        self.status = ConnectionStatus::Disconnected;
        self.session_id = None;
    }

    /// Submit a render job.
    pub async fn submit_job(&mut self, job: super::job::RenderJob) -> Result<String, CloudError> {
        if !self.is_connected() {
            return Err(CloudError::NotConnected);
        }

        let job_id = job.id.clone();
        self.jobs.insert(job_id.clone(), job);
        Ok(job_id)
    }

    /// Get job status.
    pub fn get_job(&self, job_id: &str) -> Option<&super::job::RenderJob> {
        self.jobs.get(job_id)
    }

    /// Get all jobs.
    pub fn jobs(&self) -> impl Iterator<Item = &super::job::RenderJob> {
        self.jobs.values()
    }

    /// Cancel a job.
    pub async fn cancel_job(&mut self, job_id: &str) -> Result<(), CloudError> {
        if !self.is_connected() {
            return Err(CloudError::NotConnected);
        }

        if let Some(job) = self.jobs.get_mut(job_id) {
            job.status = super::job::JobStatus::Cancelled;
            Ok(())
        } else {
            Err(CloudError::JobNotFound(job_id.to_string()))
        }
    }

    /// Get available instance types.
    pub async fn list_instance_types(&self) -> Result<Vec<InstanceType>, CloudError> {
        if !self.is_connected() {
            return Err(CloudError::NotConnected);
        }

        Ok(vec![
            InstanceType {
                id: "cpu-small".to_string(),
                name: "CPU Small".to_string(),
                cores: 4,
                memory_gb: 16,
                gpu: None,
                cost_per_hour: 0.10,
            },
            InstanceType {
                id: "cpu-medium".to_string(),
                name: "CPU Medium".to_string(),
                cores: 8,
                memory_gb: 32,
                gpu: None,
                cost_per_hour: 0.20,
            },
            InstanceType {
                id: "cpu-large".to_string(),
                name: "CPU Large".to_string(),
                cores: 32,
                memory_gb: 128,
                gpu: None,
                cost_per_hour: 0.80,
            },
            InstanceType {
                id: "gpu-rtx3080".to_string(),
                name: "GPU RTX 3080".to_string(),
                cores: 8,
                memory_gb: 32,
                gpu: Some(GpuInfo {
                    name: "RTX 3080".to_string(),
                    vram_gb: 10,
                    compute_units: 8704,
                }),
                cost_per_hour: 0.50,
            },
            InstanceType {
                id: "gpu-a100".to_string(),
                name: "GPU A100".to_string(),
                cores: 16,
                memory_gb: 64,
                gpu: Some(GpuInfo {
                    name: "A100".to_string(),
                    vram_gb: 40,
                    compute_units: 6912,
                }),
                cost_per_hour: 2.00,
            },
        ])
    }

    /// Get usage statistics.
    pub async fn get_usage(&self) -> Result<UsageStats, CloudError> {
        if !self.is_connected() {
            return Err(CloudError::NotConnected);
        }

        Ok(UsageStats {
            total_jobs: self.jobs.len() as u64,
            completed_jobs: self
                .jobs
                .values()
                .filter(|j| j.status == super::job::JobStatus::Completed)
                .count() as u64,
            total_render_time: Duration::from_secs(3600),
            total_cost: 12.50,
            storage_used_gb: 5.2,
        })
    }

    /// Ping the server.
    pub async fn ping(&mut self) -> Result<Duration, CloudError> {
        if !self.is_connected() {
            return Err(CloudError::NotConnected);
        }

        let start = Instant::now();
        // Simulate network latency
        let latency = Duration::from_millis(50);
        self.last_ping = Some(start);
        Ok(latency)
    }
}

/// Instance type specification.
#[derive(Debug, Clone)]
pub struct InstanceType {
    pub id: String,
    pub name: String,
    pub cores: u32,
    pub memory_gb: u32,
    pub gpu: Option<GpuInfo>,
    pub cost_per_hour: f64,
}

/// GPU information.
#[derive(Debug, Clone)]
pub struct GpuInfo {
    pub name: String,
    pub vram_gb: u32,
    pub compute_units: u32,
}

/// Usage statistics.
#[derive(Debug, Clone)]
pub struct UsageStats {
    pub total_jobs: u64,
    pub completed_jobs: u64,
    pub total_render_time: Duration,
    pub total_cost: f64,
    pub storage_used_gb: f64,
}

/// Cloud client errors.
#[derive(Debug, Clone)]
pub enum CloudError {
    NotConnected,
    ConnectionFailed(String),
    AuthenticationFailed(String),
    JobNotFound(String),
    QuotaExceeded,
    NetworkError(String),
    InvalidConfiguration(String),
    ServerError(String),
}

impl std::fmt::Display for CloudError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotConnected => write!(f, "Not connected to cloud service"),
            Self::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),
            Self::AuthenticationFailed(msg) => write!(f, "Authentication failed: {}", msg),
            Self::JobNotFound(id) => write!(f, "Job not found: {}", id),
            Self::QuotaExceeded => write!(f, "Quota exceeded"),
            Self::NetworkError(msg) => write!(f, "Network error: {}", msg),
            Self::InvalidConfiguration(msg) => write!(f, "Invalid configuration: {}", msg),
            Self::ServerError(msg) => write!(f, "Server error: {}", msg),
        }
    }
}

impl std::error::Error for CloudError {}

fn generate_session_id() -> String {
    use std::time::SystemTime;
    let time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("session-{:x}", time)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cloud_config_default() {
        let config = CloudConfig::default();
        assert_eq!(config.provider, CloudProvider::Custom);
        assert!(config.gpu_enabled);
    }

    #[test]
    fn test_client_creation() {
        let client = CloudClient::new(CloudConfig::default());
        assert_eq!(client.status(), ConnectionStatus::Disconnected);
        assert!(!client.is_connected());
    }
}
