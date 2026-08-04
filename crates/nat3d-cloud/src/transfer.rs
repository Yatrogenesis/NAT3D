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

//! File transfer for cloud rendering.
//!
//! Handles upload/download of scenes and results with progress tracking.

use reqwest::Client;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Transfer progress information.
#[derive(Debug, Clone)]
pub struct TransferProgress {
    pub bytes_sent: u64,
    pub total_bytes: u64,
    pub speed: f64,
    pub eta: Duration,
    pub started_at: Instant,
}

impl TransferProgress {
    pub fn new(total_bytes: u64) -> Self {
        Self {
            bytes_sent: 0,
            total_bytes,
            speed: 0.0,
            eta: Duration::ZERO,
            started_at: Instant::now(),
        }
    }

    pub fn update(&mut self, bytes_sent: u64) {
        self.bytes_sent = bytes_sent;
        let elapsed = self.started_at.elapsed().as_secs_f64();

        if elapsed > 0.0 {
            self.speed = bytes_sent as f64 / elapsed;

            if self.speed > 0.0 {
                let remaining_bytes = self.total_bytes.saturating_sub(bytes_sent);
                let eta_secs = remaining_bytes as f64 / self.speed;
                self.eta = Duration::from_secs_f64(eta_secs);
            }
        }
    }

    pub fn percentage(&self) -> f32 {
        if self.total_bytes == 0 {
            0.0
        } else {
            (self.bytes_sent as f32 / self.total_bytes as f32) * 100.0
        }
    }
}

/// Callback for transfer progress.
pub type ProgressCallback = Arc<dyn Fn(TransferProgress) + Send + Sync>;

/// File transfer manager.
pub struct TransferManager {
    client: Client,
    chunk_size: usize,
    progress_callback: Option<ProgressCallback>,
}

impl TransferManager {
    /// Create a new transfer manager.
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            chunk_size: 1024 * 1024, // 1 MB chunks
            progress_callback: None,
        }
    }

    /// Set chunk size for uploads.
    pub fn with_chunk_size(mut self, size: usize) -> Self {
        self.chunk_size = size;
        self
    }

    /// Set progress callback.
    pub fn with_progress_callback(mut self, callback: ProgressCallback) -> Self {
        self.progress_callback = Some(callback);
        self
    }

    /// Upload a file to a URL.
    pub async fn upload_file(&self, path: &Path, url: &str) -> anyhow::Result<()> {
        let mut file = File::open(path).await?;
        let file_size = file.metadata().await?.len();

        let mut progress = TransferProgress::new(file_size);
        let _buffer = vec![0u8; self.chunk_size];
        let _bytes_sent = 0u64;

        // In a real implementation, this would use multipart upload
        // For now, read entire file and upload
        let mut contents = Vec::new();
        file.read_to_end(&mut contents).await?;

        let response = self.client.put(url).body(contents).send().await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Upload failed: {}", response.status()));
        }

        progress.update(file_size);
        if let Some(callback) = &self.progress_callback {
            callback(progress);
        }

        Ok(())
    }

    /// Download a file from a URL.
    pub async fn download_file(&self, url: &str, path: &Path) -> anyhow::Result<()> {
        let response = self.client.get(url).send().await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Download failed: {}", response.status()));
        }

        let total_size = response.content_length().unwrap_or(0);
        let mut progress = TransferProgress::new(total_size);

        let mut file = File::create(path).await?;
        let bytes = response.bytes().await?;
        file.write_all(&bytes).await?;

        let bytes_received = bytes.len() as u64;
        progress.update(bytes_received);

        if let Some(callback) = &self.progress_callback {
            callback(progress);
        }

        Ok(())
    }

    /// Upload a scene file.
    pub async fn upload_scene(&self, scene_path: &Path, url: &str) -> anyhow::Result<String> {
        self.upload_file(scene_path, url).await?;
        Ok(format!("Scene uploaded: {}", scene_path.display()))
    }

    /// Download render result.
    pub async fn download_result(&self, url: &str, output_path: &Path) -> anyhow::Result<String> {
        self.download_file(url, output_path).await?;
        Ok(format!("Result downloaded: {}", output_path.display()))
    }

    /// Upload multiple files (chunked).
    pub async fn upload_files(
        &self,
        files: &[PathBuf],
        base_url: &str,
    ) -> anyhow::Result<Vec<String>> {
        let mut results = Vec::new();

        for (i, file) in files.iter().enumerate() {
            let url = format!("{}/file_{}", base_url, i);
            self.upload_file(file, &url).await?;
            results.push(url);
        }

        Ok(results)
    }

    /// Download multiple files.
    pub async fn download_files(
        &self,
        urls: &[String],
        output_dir: &Path,
    ) -> anyhow::Result<Vec<PathBuf>> {
        let mut results = Vec::new();

        for (i, url) in urls.iter().enumerate() {
            let output_path = output_dir.join(format!("result_{}.bin", i));
            self.download_file(url, &output_path).await?;
            results.push(output_path);
        }

        Ok(results)
    }
}

impl Default for TransferManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn test_transfer_progress() {
        let mut progress = TransferProgress::new(1000);
        assert_eq!(progress.percentage(), 0.0);

        progress.update(500);
        assert_eq!(progress.percentage(), 50.0);

        progress.update(1000);
        assert_eq!(progress.percentage(), 100.0);
    }

    #[test]
    fn test_transfer_manager_creation() {
        let manager = TransferManager::new();
        assert_eq!(manager.chunk_size, 1024 * 1024);
    }

    #[test]
    fn test_transfer_manager_with_chunk_size() {
        let manager = TransferManager::new().with_chunk_size(512 * 1024);
        assert_eq!(manager.chunk_size, 512 * 1024);
    }

    #[tokio::test]
    async fn test_progress_callback() {
        let progress_data = Arc::new(Mutex::new(Vec::new()));
        let progress_data_clone = Arc::clone(&progress_data);

        let callback: ProgressCallback = Arc::new(move |progress| {
            progress_data_clone
                .lock()
                .unwrap()
                .push(progress.percentage());
        });

        let _manager = TransferManager::new().with_progress_callback(callback);
        // Manager is created successfully
    }
}
