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

//! Network device discovery using mDNS/Bonjour.
//!
//! Discovers iPads, iPhones, Apple Pencils, and drawing tablets on the local network.

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// Type of discovered device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType {
    IPad,
    IPhone,
    ApplePencil,
    DrawingTablet,
    Custom,
}

impl std::fmt::Display for DeviceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IPad => write!(f, "iPad"),
            Self::IPhone => write!(f, "iPhone"),
            Self::ApplePencil => write!(f, "Apple Pencil"),
            Self::DrawingTablet => write!(f, "Drawing Tablet"),
            Self::Custom => write!(f, "Custom"),
        }
    }
}

/// Discovered device information.
#[derive(Debug, Clone)]
pub struct DiscoveredDevice {
    pub name: String,
    pub address: IpAddr,
    pub port: u16,
    pub device_type: DeviceType,
    pub capabilities: Vec<String>,
}

impl DiscoveredDevice {
    pub fn new(name: String, address: IpAddr, port: u16, device_type: DeviceType) -> Self {
        Self {
            name,
            address,
            port,
            device_type,
            capabilities: Vec::new(),
        }
    }

    pub fn with_capability(mut self, capability: String) -> Self {
        self.capabilities.push(capability);
        self
    }
}

/// Device discovery events.
#[derive(Debug, Clone)]
pub enum DiscoveryEvent {
    DeviceFound(DiscoveredDevice),
    DeviceLost(String),
}

/// Device discovery manager.
pub struct DeviceDiscovery {
    service_type: String,
    devices: Arc<Mutex<HashMap<String, DiscoveredDevice>>>,
    daemon: Option<ServiceDaemon>,
}

impl DeviceDiscovery {
    /// Create a new device discovery manager.
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            service_type: "_nat3d._tcp.local.".to_string(),
            devices: Arc::new(Mutex::new(HashMap::new())),
            daemon: None,
        })
    }

    /// Set custom service type.
    pub fn with_service_type(mut self, service_type: String) -> Self {
        self.service_type = service_type;
        self
    }

    /// Start device discovery.
    pub async fn start_discovery(&mut self) -> anyhow::Result<mpsc::Receiver<DiscoveryEvent>> {
        let (tx, rx) = mpsc::channel(32);

        let daemon = ServiceDaemon::new()?;
        let receiver = daemon.browse(&self.service_type)?;

        let devices = Arc::clone(&self.devices);

        tokio::spawn(async move {
            while let Ok(event) = receiver.recv_async().await {
                match event {
                    ServiceEvent::ServiceResolved(info) => {
                        if let Some(device) = Self::parse_service_info(&info) {
                            devices
                                .lock()
                                .unwrap()
                                .insert(device.name.clone(), device.clone());
                            let _ = tx.send(DiscoveryEvent::DeviceFound(device)).await;
                        }
                    }
                    ServiceEvent::ServiceRemoved(_, fullname) => {
                        devices.lock().unwrap().remove(&fullname);
                        let _ = tx.send(DiscoveryEvent::DeviceLost(fullname)).await;
                    }
                    _ => {}
                }
            }
        });

        self.daemon = Some(daemon);
        Ok(rx)
    }

    /// Stop device discovery.
    pub fn stop(&mut self) {
        self.daemon = None;
    }

    /// Get all discovered devices.
    pub fn get_devices(&self) -> Vec<DiscoveredDevice> {
        self.devices.lock().unwrap().values().cloned().collect()
    }

    /// Register this device as a service.
    pub fn register_service(&self, name: &str, port: u16) -> anyhow::Result<()> {
        let service_info = ServiceInfo::new(&self.service_type, name, name, "", port, None)?;

        if let Some(daemon) = &self.daemon {
            daemon.register(service_info)?;
        }

        Ok(())
    }

    fn parse_service_info(info: &ServiceInfo) -> Option<DiscoveredDevice> {
        let name = info.get_fullname().to_string();
        let addresses = info.get_addresses();
        let address = addresses.iter().next().copied()?;
        let port = info.get_port();

        let device_type = Self::detect_device_type(&name);

        let mut device = DiscoveredDevice::new(name, address, port, device_type);

        // Parse TXT records for capabilities
        for property in info.get_properties().iter() {
            let key = property.key();
            let value = property.val_str();
            device = device.with_capability(format!("{}={}", key, value));
        }

        Some(device)
    }

    fn detect_device_type(name: &str) -> DeviceType {
        let name_lower = name.to_lowercase();

        if name_lower.contains("ipad") {
            DeviceType::IPad
        } else if name_lower.contains("iphone") {
            DeviceType::IPhone
        } else if name_lower.contains("pencil") {
            DeviceType::ApplePencil
        } else if name_lower.contains("tablet") {
            DeviceType::DrawingTablet
        } else {
            DeviceType::Custom
        }
    }
}

impl Default for DeviceDiscovery {
    fn default() -> Self {
        Self::new().expect("Failed to create device discovery")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn test_device_creation() {
        let device = DiscoveredDevice::new(
            "Test iPad".to_string(),
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)),
            8080,
            DeviceType::IPad,
        );

        assert_eq!(device.name, "Test iPad");
        assert_eq!(device.port, 8080);
        assert_eq!(device.device_type, DeviceType::IPad);
    }

    #[test]
    fn test_device_with_capability() {
        let device = DiscoveredDevice::new(
            "Test".to_string(),
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)),
            8080,
            DeviceType::Custom,
        )
        .with_capability("touch".to_string())
        .with_capability("pressure".to_string());

        assert_eq!(device.capabilities.len(), 2);
    }

    #[test]
    fn test_device_type_detection() {
        assert_eq!(
            DeviceDiscovery::detect_device_type("my-ipad"),
            DeviceType::IPad
        );
        assert_eq!(
            DeviceDiscovery::detect_device_type("iPhone-12"),
            DeviceType::IPhone
        );
        assert_eq!(
            DeviceDiscovery::detect_device_type("Apple-Pencil"),
            DeviceType::ApplePencil
        );
        assert_eq!(
            DeviceDiscovery::detect_device_type("Wacom-Tablet"),
            DeviceType::DrawingTablet
        );
    }

    #[test]
    fn test_discovery_creation() {
        let discovery = DeviceDiscovery::new();
        assert!(discovery.is_ok());
    }

    #[test]
    fn test_discovery_with_service_type() {
        let discovery = DeviceDiscovery::new()
            .unwrap()
            .with_service_type("_custom._tcp.local.".to_string());

        assert_eq!(discovery.service_type, "_custom._tcp.local.");
    }
}
