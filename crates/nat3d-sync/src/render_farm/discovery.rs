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

//! Network discovery for render farm nodes using mDNS.
//!
//! Implements zero-configuration discovery:
//! - Masters announce presence on `_nat3d-render._tcp.local.`
//! - Workers discover masters automatically
//! - Capabilities advertised via TXT records

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use uuid::Uuid;

use super::RENDER_FARM_SERVICE_TYPE;

/// Role of a render node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeRole {
    /// Master node (schedules jobs, collects results)
    Master,
    /// Worker node (executes render jobs)
    Worker,
}

impl std::fmt::Display for NodeRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Master => write!(f, "Master"),
            Self::Worker => write!(f, "Worker"),
        }
    }
}

/// Information about a discovered render node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    /// Unique node identifier
    pub id: Uuid,
    /// Human-readable node name
    pub name: String,
    /// Node role (master or worker)
    pub role: NodeRole,
    /// Network address
    pub address: IpAddr,
    /// TCP port for job protocol
    pub port: u16,
    /// GPU name (e.g., "NVIDIA RTX 3050")
    pub gpu_name: Option<String>,
    /// VRAM in megabytes
    pub vram_mb: Option<u32>,
    /// CPU core count
    pub cpu_cores: Option<u8>,
    /// Maximum tile size (e.g., 512 for 512×512)
    pub max_tile_size: Option<u32>,
    /// NAT3D version
    pub version: Option<String>,
}

impl NodeInfo {
    /// Create a new node info.
    pub fn new(id: Uuid, name: String, role: NodeRole, address: IpAddr, port: u16) -> Self {
        Self {
            id,
            name,
            role,
            address,
            port,
            gpu_name: None,
            vram_mb: None,
            cpu_cores: None,
            max_tile_size: None,
            version: None,
        }
    }

    /// Get socket address for TCP connection.
    pub fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.address, self.port)
    }

    /// Set GPU information.
    pub fn with_gpu(mut self, gpu_name: String, vram_mb: u32) -> Self {
        self.gpu_name = Some(gpu_name);
        self.vram_mb = Some(vram_mb);
        self
    }

    /// Set CPU information.
    pub fn with_cpu(mut self, cpu_cores: u8) -> Self {
        self.cpu_cores = Some(cpu_cores);
        self
    }

    /// Set maximum tile size.
    pub fn with_max_tile_size(mut self, max_tile_size: u32) -> Self {
        self.max_tile_size = Some(max_tile_size);
        self
    }

    /// Set version.
    pub fn with_version(mut self, version: String) -> Self {
        self.version = Some(version);
        self
    }
}

/// Discovery events.
#[derive(Debug, Clone)]
pub enum DiscoveryEvent {
    /// New master node discovered
    MasterFound(NodeInfo),
    /// Master node lost
    MasterLost(Uuid),
    /// New worker node discovered
    WorkerFound(NodeInfo),
    /// Worker node lost
    WorkerLost(Uuid),
}

/// Render node discovery manager.
pub struct RenderNodeDiscovery {
    /// This node's unique ID
    node_id: Uuid,
    /// This node's role
    role: NodeRole,
    /// mDNS service daemon
    daemon: ServiceDaemon,
    /// Discovered nodes (keyed by node ID)
    discovered_nodes: Arc<Mutex<HashMap<Uuid, NodeInfo>>>,
    /// Service name (for registration)
    service_name: String,
}

impl RenderNodeDiscovery {
    /// Create a new render node discovery manager.
    pub fn new(node_id: Uuid, role: NodeRole, service_name: String) -> anyhow::Result<Self> {
        let daemon = ServiceDaemon::new()?;

        Ok(Self {
            node_id,
            role,
            daemon,
            discovered_nodes: Arc::new(Mutex::new(HashMap::new())),
            service_name,
        })
    }

    /// Start discovering nodes on the network.
    ///
    /// Returns a channel that emits discovery events.
    pub async fn start_discovery(&self) -> anyhow::Result<mpsc::Receiver<DiscoveryEvent>> {
        let (tx, rx) = mpsc::channel(32);

        let receiver = self.daemon.browse(RENDER_FARM_SERVICE_TYPE)?;
        let discovered_nodes = Arc::clone(&self.discovered_nodes);
        let our_node_id = self.node_id;

        tokio::spawn(async move {
            while let Ok(event) = receiver.recv_async().await {
                match event {
                    ServiceEvent::ServiceResolved(info) => {
                        if let Some(node) = Self::parse_service_info(&info) {
                            // Don't discover ourselves
                            if node.id == our_node_id {
                                continue;
                            }

                            // Insert node and create event (drop lock before await)
                            let event = {
                                discovered_nodes
                                    .lock()
                                    .unwrap()
                                    .insert(node.id, node.clone());
                                match node.role {
                                    NodeRole::Master => DiscoveryEvent::MasterFound(node),
                                    NodeRole::Worker => DiscoveryEvent::WorkerFound(node),
                                }
                            };

                            let _ = tx.send(event).await;
                        }
                    }
                    ServiceEvent::ServiceRemoved(_, fullname) => {
                        // Extract node ID from fullname
                        if let Some(node_id) = Self::extract_node_id(&fullname) {
                            // Remove node and create event (drop lock before await)
                            let event_opt = {
                                discovered_nodes
                                    .lock()
                                    .unwrap()
                                    .remove(&node_id)
                                    .map(|node| match node.role {
                                        NodeRole::Master => DiscoveryEvent::MasterLost(node_id),
                                        NodeRole::Worker => DiscoveryEvent::WorkerLost(node_id),
                                    })
                            };

                            if let Some(event) = event_opt {
                                let _ = tx.send(event).await;
                            }
                        }
                    }
                    _ => {}
                }
            }
        });

        Ok(rx)
    }

    /// Register this node as a service on the network.
    ///
    /// # Arguments
    /// * `port` - TCP port for job protocol
    /// * `capabilities` - Optional capabilities (GPU, CPU, etc.)
    pub fn register_service(
        &mut self,
        port: u16,
        capabilities: Option<NodeCapabilities>,
    ) -> anyhow::Result<()> {
        let service_name = format!("{}.{}", self.service_name, RENDER_FARM_SERVICE_TYPE);

        let mut properties = HashMap::new();
        properties.insert("id".to_string(), self.node_id.to_string());
        properties.insert("role".to_string(), format!("{:?}", self.role));
        properties.insert("version".to_string(), env!("CARGO_PKG_VERSION").to_string());

        if let Some(caps) = capabilities {
            if let Some(ref gpu) = caps.gpu_name {
                properties.insert("gpu".to_string(), gpu.clone());
            }
            if let Some(vram) = caps.vram_mb {
                properties.insert("vram_mb".to_string(), vram.to_string());
            }
            if let Some(cores) = caps.cpu_cores {
                properties.insert("cpu_cores".to_string(), cores.to_string());
            }
            if let Some(tile_size) = caps.max_tile_size {
                properties.insert("max_tile_size".to_string(), tile_size.to_string());
            }
        }

        let service_info = ServiceInfo::new(
            RENDER_FARM_SERVICE_TYPE,
            &self.service_name,
            &service_name,
            "",
            port,
            Some(properties),
        )?;

        self.daemon.register(service_info)?;

        tracing::info!(
            "Registered {} node '{}' on port {} (ID: {})",
            self.role,
            self.service_name,
            port,
            self.node_id
        );

        Ok(())
    }

    /// Unregister this node from the network.
    pub fn unregister_service(&mut self) -> anyhow::Result<()> {
        let service_name = format!("{}.{}", self.service_name, RENDER_FARM_SERVICE_TYPE);
        self.daemon.unregister(&service_name)?;

        tracing::info!("Unregistered {} node '{}'", self.role, self.service_name);

        Ok(())
    }

    /// Get all discovered nodes.
    pub fn get_discovered_nodes(&self) -> Vec<NodeInfo> {
        self.discovered_nodes
            .lock()
            .unwrap()
            .values()
            .cloned()
            .collect()
    }

    /// Get discovered masters only.
    pub fn get_masters(&self) -> Vec<NodeInfo> {
        self.discovered_nodes
            .lock()
            .unwrap()
            .values()
            .filter(|n| n.role == NodeRole::Master)
            .cloned()
            .collect()
    }

    /// Get discovered workers only.
    pub fn get_workers(&self) -> Vec<NodeInfo> {
        self.discovered_nodes
            .lock()
            .unwrap()
            .values()
            .filter(|n| n.role == NodeRole::Worker)
            .cloned()
            .collect()
    }

    /// Parse mDNS service info into NodeInfo.
    fn parse_service_info(info: &ServiceInfo) -> Option<NodeInfo> {
        let addresses = info.get_addresses();
        let address = addresses.iter().next().copied()?;
        let port = info.get_port();

        // Extract properties from TXT records
        let properties = info.get_properties();

        let id_str = properties.get_property_val_str("id")?;
        let id = Uuid::parse_str(id_str).ok()?;

        let role_str = properties.get_property_val_str("role")?;
        let role = match role_str {
            "Master" => NodeRole::Master,
            "Worker" => NodeRole::Worker,
            _ => return None,
        };

        let name = info.get_hostname().to_string();

        let mut node = NodeInfo::new(id, name, role, address, port);

        if let Some(gpu) = properties.get_property_val_str("gpu") {
            if let Some(vram_str) = properties.get_property_val_str("vram_mb") {
                if let Ok(vram) = vram_str.parse() {
                    node = node.with_gpu(gpu.to_string(), vram);
                }
            }
        }

        if let Some(cores_str) = properties.get_property_val_str("cpu_cores") {
            if let Ok(cores) = cores_str.parse() {
                node = node.with_cpu(cores);
            }
        }

        if let Some(tile_str) = properties.get_property_val_str("max_tile_size") {
            if let Ok(tile_size) = tile_str.parse() {
                node = node.with_max_tile_size(tile_size);
            }
        }

        if let Some(version) = properties.get_property_val_str("version") {
            node = node.with_version(version.to_string());
        }

        Some(node)
    }

    /// Extract node ID from mDNS fullname.
    fn extract_node_id(fullname: &str) -> Option<Uuid> {
        // Fullname format: "service-name._nat3d-render._tcp.local."
        // Extract service-name part and look for UUID pattern
        fullname
            .split('.')
            .next()
            .and_then(|name| Uuid::parse_str(name).ok())
    }
}

/// Node capabilities for service registration.
#[derive(Debug, Clone)]
pub struct NodeCapabilities {
    pub gpu_name: Option<String>,
    pub vram_mb: Option<u32>,
    pub cpu_cores: Option<u8>,
    pub max_tile_size: Option<u32>,
}

impl NodeCapabilities {
    pub fn new() -> Self {
        Self {
            gpu_name: None,
            vram_mb: None,
            cpu_cores: None,
            max_tile_size: None,
        }
    }

    pub fn with_gpu(mut self, gpu_name: String, vram_mb: u32) -> Self {
        self.gpu_name = Some(gpu_name);
        self.vram_mb = Some(vram_mb);
        self
    }

    pub fn with_cpu(mut self, cpu_cores: u8) -> Self {
        self.cpu_cores = Some(cpu_cores);
        self
    }

    pub fn with_max_tile_size(mut self, max_tile_size: u32) -> Self {
        self.max_tile_size = Some(max_tile_size);
        self
    }
}

impl Default for NodeCapabilities {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn test_node_info_creation() {
        let id = Uuid::new_v4();
        let node = NodeInfo::new(
            id,
            "test-master".to_string(),
            NodeRole::Master,
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)),
            9000,
        );

        assert_eq!(node.id, id);
        assert_eq!(node.name, "test-master");
        assert_eq!(node.role, NodeRole::Master);
        assert_eq!(node.port, 9000);
    }

    #[test]
    fn test_node_info_with_capabilities() {
        let id = Uuid::new_v4();
        let node = NodeInfo::new(
            id,
            "test-worker".to_string(),
            NodeRole::Worker,
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 101)),
            0,
        )
        .with_gpu("NVIDIA RTX 3050".to_string(), 4096)
        .with_cpu(8)
        .with_max_tile_size(512);

        assert_eq!(node.gpu_name, Some("NVIDIA RTX 3050".to_string()));
        assert_eq!(node.vram_mb, Some(4096));
        assert_eq!(node.cpu_cores, Some(8));
        assert_eq!(node.max_tile_size, Some(512));
    }

    #[test]
    fn test_socket_addr() {
        let id = Uuid::new_v4();
        let node = NodeInfo::new(
            id,
            "test".to_string(),
            NodeRole::Worker,
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)),
            9000,
        );

        let addr = node.socket_addr();
        assert_eq!(addr.ip(), IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)));
        assert_eq!(addr.port(), 9000);
    }

    #[test]
    fn test_node_role_display() {
        assert_eq!(format!("{}", NodeRole::Master), "Master");
        assert_eq!(format!("{}", NodeRole::Worker), "Worker");
    }

    #[test]
    fn test_node_capabilities_builder() {
        let caps = NodeCapabilities::new()
            .with_gpu("Test GPU".to_string(), 8192)
            .with_cpu(16)
            .with_max_tile_size(1024);

        assert_eq!(caps.gpu_name, Some("Test GPU".to_string()));
        assert_eq!(caps.vram_mb, Some(8192));
        assert_eq!(caps.cpu_cores, Some(16));
        assert_eq!(caps.max_tile_size, Some(1024));
    }

    #[test]
    fn test_discovery_creation() {
        let id = Uuid::new_v4();
        let discovery = RenderNodeDiscovery::new(id, NodeRole::Master, "test-master".to_string());

        assert!(discovery.is_ok());
        let discovery = discovery.unwrap();
        assert_eq!(discovery.node_id, id);
        assert_eq!(discovery.role, NodeRole::Master);
    }
}
