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

//! Provider configuration management.
//!
//! Handles configuration and credentials for cloud providers.

use crate::client::CloudProvider;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Provider credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCredentials {
    /// API key, if set.
    pub api_key: Option<String>,
    /// Secret, if set; serialized together with the other fields when a `ConfigFile` is saved.
    pub secret: Option<String>,
    /// Region, if set.
    pub region: Option<String>,
    /// Endpoint, if set.
    pub endpoint: Option<String>,
    /// Additional named values, keyed by name.
    pub additional: HashMap<String, String>,
}

impl ProviderCredentials {
    /// Creates credentials with every optional field `None` and an empty additional map.
    pub fn new() -> Self {
        Self {
            api_key: None,
            secret: None,
            region: None,
            endpoint: None,
            additional: HashMap::new(),
        }
    }

    /// Sets the API key and returns the credentials.
    pub fn with_api_key(mut self, key: String) -> Self {
        self.api_key = Some(key);
        self
    }

    /// Sets the secret and returns the credentials.
    pub fn with_secret(mut self, secret: String) -> Self {
        self.secret = Some(secret);
        self
    }

    /// Sets the region and returns the credentials.
    pub fn with_region(mut self, region: String) -> Self {
        self.region = Some(region);
        self
    }

    /// Sets the endpoint and returns the credentials.
    pub fn with_endpoint(mut self, endpoint: String) -> Self {
        self.endpoint = Some(endpoint);
        self
    }

    /// Inserts or replaces a named entry in `additional`.
    pub fn add_field(&mut self, key: String, value: String) {
        self.additional.insert(key, value);
    }
}

impl Default for ProviderCredentials {
    fn default() -> Self {
        Self::new()
    }
}

/// Provider configuration trait.
pub trait ProviderConfig {
    /// Get the provider name.
    fn name(&self) -> &str;

    /// Validate the configuration.
    fn validate(&self) -> Result<(), String>;

    /// Create a client with this configuration.
    fn create_client(&self) -> anyhow::Result<Box<dyn std::any::Any>>;
}

/// Configuration file for persistent storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigFile {
    /// Version string of the file; `ConfigFile::new` sets it to `"1.0"`.
    pub version: String,
    /// Credentials per provider, keyed by the provider name used by `set_provider` (`aws`, `gcp`, `azure`, `custom`).
    pub providers: HashMap<String, ProviderCredentials>,
}

impl ConfigFile {
    /// Creates a file structure with version `"1.0"` and no providers.
    pub fn new() -> Self {
        Self {
            version: "1.0".to_string(),
            providers: HashMap::new(),
        }
    }

    /// Load configuration from file.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: ConfigFile = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// Save configuration to file.
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let content = serde_json::to_string_pretty(&self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Add or update provider credentials.
    pub fn set_provider(&mut self, provider: CloudProvider, credentials: ProviderCredentials) {
        let name = match provider {
            CloudProvider::Aws => "aws",
            CloudProvider::Gcp => "gcp",
            CloudProvider::Azure => "azure",
            CloudProvider::Custom => "custom",
        };
        self.providers.insert(name.to_string(), credentials);
    }

    /// Get provider credentials.
    pub fn get_provider(&self, provider: CloudProvider) -> Option<&ProviderCredentials> {
        let name = match provider {
            CloudProvider::Aws => "aws",
            CloudProvider::Gcp => "gcp",
            CloudProvider::Azure => "azure",
            CloudProvider::Custom => "custom",
        };
        self.providers.get(name)
    }

    /// Remove provider credentials.
    pub fn remove_provider(&mut self, provider: CloudProvider) {
        let name = match provider {
            CloudProvider::Aws => "aws",
            CloudProvider::Gcp => "gcp",
            CloudProvider::Azure => "azure",
            CloudProvider::Custom => "custom",
        };
        self.providers.remove(name);
    }
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credentials_creation() {
        let creds = ProviderCredentials::new()
            .with_api_key("test_key".to_string())
            .with_secret("test_secret".to_string())
            .with_region("us-east-1".to_string());

        assert_eq!(creds.api_key, Some("test_key".to_string()));
        assert_eq!(creds.secret, Some("test_secret".to_string()));
        assert_eq!(creds.region, Some("us-east-1".to_string()));
    }

    #[test]
    fn test_credentials_additional_fields() {
        let mut creds = ProviderCredentials::new();
        creds.add_field("custom_field".to_string(), "custom_value".to_string());

        assert_eq!(
            creds.additional.get("custom_field"),
            Some(&"custom_value".to_string())
        );
    }

    #[test]
    fn test_config_file_creation() {
        let config = ConfigFile::new();
        assert_eq!(config.version, "1.0");
        assert!(config.providers.is_empty());
    }

    #[test]
    fn test_config_file_set_get_provider() {
        let mut config = ConfigFile::new();
        let creds = ProviderCredentials::new().with_api_key("test_key".to_string());

        config.set_provider(CloudProvider::Aws, creds);

        let retrieved = config.get_provider(CloudProvider::Aws);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().api_key, Some("test_key".to_string()));
    }

    #[test]
    fn test_config_file_remove_provider() {
        let mut config = ConfigFile::new();
        let creds = ProviderCredentials::new();

        config.set_provider(CloudProvider::Aws, creds);
        assert!(config.get_provider(CloudProvider::Aws).is_some());

        config.remove_provider(CloudProvider::Aws);
        assert!(config.get_provider(CloudProvider::Aws).is_none());
    }
}
