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

//! Application startup and initialization.
//!
//! Handles initial setup, splash screen, workspace restoration,
//! and system capability detection.

use std::path::PathBuf;
use std::time::Instant;

/// System capabilities detected at startup.
#[derive(Debug, Clone)]
pub struct SystemCapabilities {
    /// GPU name.
    pub gpu_name: String,
    /// GPU vendor.
    pub gpu_vendor: String,
    /// GPU backend (Vulkan, DX12, Metal).
    pub gpu_backend: String,
    /// Available VRAM in bytes.
    pub vram_bytes: u64,
    /// Available system RAM in bytes.
    pub ram_bytes: u64,
    /// Number of CPU cores.
    pub cpu_cores: usize,
    /// Whether ray tracing hardware is available.
    pub raytracing_hw: bool,
    /// Maximum texture size.
    pub max_texture_size: u32,
    /// Whether compute shaders are supported.
    pub compute_shaders: bool,
    /// Whether GPU is suitable for real-time rendering.
    pub gpu_suitable: bool,
}

impl Default for SystemCapabilities {
    fn default() -> Self {
        Self {
            gpu_name: "Unknown".to_string(),
            gpu_vendor: "Unknown".to_string(),
            gpu_backend: "Unknown".to_string(),
            vram_bytes: 0,
            ram_bytes: 0,
            cpu_cores: 1,
            raytracing_hw: false,
            max_texture_size: 4096,
            compute_shaders: false,
            gpu_suitable: false,
        }
    }
}

impl SystemCapabilities {
    /// Detect system capabilities.
    pub fn detect() -> Self {
        let cpu_cores = std::thread::available_parallelism()
            .map(|p| p.get())
            .unwrap_or(1);

        Self {
            cpu_cores,
            max_texture_size: 8192,
            compute_shaders: true,
            gpu_suitable: true,
            ..Default::default()
        }
    }

    /// Check minimum requirements.
    pub fn meets_minimum_requirements(&self) -> bool {
        self.gpu_suitable && self.cpu_cores >= 2
    }

    /// Get capability summary string.
    pub fn summary(&self) -> String {
        format!(
            "GPU: {} ({}) | VRAM: {} MB | RAM: {} MB | Cores: {} | RT: {}",
            self.gpu_name,
            self.gpu_backend,
            self.vram_bytes / (1024 * 1024),
            self.ram_bytes / (1024 * 1024),
            self.cpu_cores,
            if self.raytracing_hw { "Yes" } else { "No" }
        )
    }
}

/// Startup configuration.
#[derive(Debug, Clone)]
pub struct StartupConfig {
    /// Whether to show splash screen.
    pub show_splash: bool,
    /// Whether to restore last workspace.
    pub restore_workspace: bool,
    /// Whether to load plugins.
    pub load_plugins: bool,
    /// Whether to check for updates.
    pub check_updates: bool,
    /// File to open on startup (from command line).
    pub open_file: Option<PathBuf>,
    /// Whether to run in headless mode.
    pub headless: bool,
    /// Log level.
    pub log_level: LogLevel,
}

/// Log level for startup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    /// Only errors.
    Error,
    /// Warnings and errors.
    Warn,
    /// Info, warnings, and errors.
    Info,
    /// Debug and above.
    Debug,
    /// Everything.
    Trace,
}

impl Default for StartupConfig {
    fn default() -> Self {
        Self {
            show_splash: true,
            restore_workspace: true,
            load_plugins: true,
            check_updates: false,
            open_file: None,
            headless: false,
            log_level: LogLevel::Info,
        }
    }
}

/// Startup phase tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupPhase {
    /// Detecting system capabilities.
    DetectingSystem,
    /// Initializing GPU.
    InitializingGpu,
    /// Loading configuration.
    LoadingConfig,
    /// Loading plugins.
    LoadingPlugins,
    /// Restoring workspace.
    RestoringWorkspace,
    /// Opening file.
    OpeningFile,
    /// Ready.
    Ready,
}

impl StartupPhase {
    /// Get human-readable description.
    pub fn description(&self) -> &'static str {
        match self {
            Self::DetectingSystem => "Detecting system capabilities...",
            Self::InitializingGpu => "Initializing GPU...",
            Self::LoadingConfig => "Loading configuration...",
            Self::LoadingPlugins => "Loading plugins...",
            Self::RestoringWorkspace => "Restoring workspace...",
            Self::OpeningFile => "Opening file...",
            Self::Ready => "Ready",
        }
    }

    /// Get progress (0.0 - 1.0).
    pub fn progress(&self) -> f32 {
        match self {
            Self::DetectingSystem => 0.1,
            Self::InitializingGpu => 0.3,
            Self::LoadingConfig => 0.5,
            Self::LoadingPlugins => 0.6,
            Self::RestoringWorkspace => 0.8,
            Self::OpeningFile => 0.9,
            Self::Ready => 1.0,
        }
    }
}

/// Startup manager.
pub struct StartupManager {
    /// Configuration.
    config: StartupConfig,
    /// Current phase.
    phase: StartupPhase,
    /// System capabilities.
    capabilities: Option<SystemCapabilities>,
    /// Start time.
    start_time: Instant,
    /// Errors encountered.
    errors: Vec<String>,
    /// Warnings encountered.
    warnings: Vec<String>,
}

impl StartupManager {
    /// Create new startup manager.
    pub fn new(config: StartupConfig) -> Self {
        Self {
            config,
            phase: StartupPhase::DetectingSystem,
            capabilities: None,
            start_time: Instant::now(),
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Run startup sequence.
    pub fn run(&mut self) -> Result<(), Vec<String>> {
        // Phase 1: Detect system
        self.phase = StartupPhase::DetectingSystem;
        let caps = SystemCapabilities::detect();
        if !caps.meets_minimum_requirements() {
            self.warnings
                .push("System may not meet minimum requirements".to_string());
        }
        self.capabilities = Some(caps);

        // Phase 2: Initialize GPU
        self.phase = StartupPhase::InitializingGpu;

        // Phase 3: Load config
        self.phase = StartupPhase::LoadingConfig;
        self.load_config();

        // Phase 4: Load plugins
        if self.config.load_plugins {
            self.phase = StartupPhase::LoadingPlugins;
            self.load_plugins();
        }

        // Phase 5: Restore workspace
        if self.config.restore_workspace {
            self.phase = StartupPhase::RestoringWorkspace;
            self.restore_workspace();
        }

        // Phase 6: Open file
        if self.config.open_file.is_some() {
            self.phase = StartupPhase::OpeningFile;
            self.open_startup_file();
        }

        self.phase = StartupPhase::Ready;

        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(self.errors.clone())
        }
    }

    /// Get current phase.
    pub fn phase(&self) -> StartupPhase {
        self.phase
    }

    /// Get system capabilities.
    pub fn capabilities(&self) -> Option<&SystemCapabilities> {
        self.capabilities.as_ref()
    }

    /// Get elapsed time since startup.
    pub fn elapsed_ms(&self) -> u128 {
        self.start_time.elapsed().as_millis()
    }

    /// Get warnings.
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    fn load_config(&mut self) {
        let config_path = Self::config_path();
        if !config_path.exists() {
            self.warnings
                .push("No configuration file found, using defaults".to_string());
        }
    }

    fn load_plugins(&mut self) {
        let plugin_dir = Self::plugin_dir();
        if !plugin_dir.exists() {
            return;
        }
    }

    fn restore_workspace(&mut self) {
        let workspace_path = Self::workspace_path();
        if !workspace_path.exists() {
            return;
        }
    }

    fn open_startup_file(&mut self) {
        if let Some(ref path) = self.config.open_file {
            if !path.exists() {
                self.errors
                    .push(format!("File not found: {}", path.display()));
            }
        }
    }

    /// Get configuration directory.
    pub fn config_dir() -> PathBuf {
        dirs_or_default("nat3d")
    }

    /// Get config file path.
    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.json")
    }

    /// Get plugin directory.
    pub fn plugin_dir() -> PathBuf {
        Self::config_dir().join("plugins")
    }

    /// Get workspace save path.
    pub fn workspace_path() -> PathBuf {
        Self::config_dir().join("workspace.json")
    }

    /// Get recent files path.
    pub fn recent_files_path() -> PathBuf {
        Self::config_dir().join("recent_files.json")
    }

    /// Get autosave directory.
    pub fn autosave_dir() -> PathBuf {
        Self::config_dir().join("autosave")
    }
}

/// Get platform-appropriate config directory, or fallback.
fn dirs_or_default(app_name: &str) -> PathBuf {
    if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
        return PathBuf::from(home).join(format!(".{}", app_name));
    }
    PathBuf::from(format!(".{}", app_name))
}

/// Recent files list.
#[derive(Debug, Clone, Default)]
pub struct RecentFiles {
    /// List of recent file paths.
    files: Vec<RecentFile>,
    /// Maximum number of entries.
    max_entries: usize,
}

/// A recent file entry.
#[derive(Debug, Clone)]
pub struct RecentFile {
    /// File path.
    pub path: PathBuf,
    /// Last opened timestamp (unix seconds).
    pub last_opened: u64,
    /// Thumbnail data (optional).
    pub thumbnail: Option<String>,
}

impl RecentFiles {
    /// Create with default max entries.
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            max_entries: 20,
        }
    }

    /// Add a file to recent list.
    pub fn add(&mut self, path: PathBuf) {
        self.files.retain(|f| f.path != path);
        self.files.insert(
            0,
            RecentFile {
                path,
                last_opened: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                thumbnail: None,
            },
        );
        self.files.truncate(self.max_entries);
    }

    /// Get recent files.
    pub fn files(&self) -> &[RecentFile] {
        &self.files
    }

    /// Clear all recent files.
    pub fn clear(&mut self) {
        self.files.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_capabilities() {
        let caps = SystemCapabilities::detect();
        assert!(caps.cpu_cores >= 1);
        assert!(!caps.summary().is_empty());
    }

    #[test]
    fn test_startup_phases() {
        assert_eq!(StartupPhase::DetectingSystem.progress(), 0.1);
        assert_eq!(StartupPhase::Ready.progress(), 1.0);
        assert!(!StartupPhase::LoadingConfig.description().is_empty());
    }

    #[test]
    fn test_startup_manager() {
        let config = StartupConfig::default();
        let mut manager = StartupManager::new(config);
        let result = manager.run();
        assert!(result.is_ok());
        assert_eq!(manager.phase(), StartupPhase::Ready);
    }

    #[test]
    fn test_recent_files() {
        let mut recent = RecentFiles::new();
        recent.add(PathBuf::from("test1.nat"));
        recent.add(PathBuf::from("test2.nat"));
        assert_eq!(recent.files().len(), 2);
        assert_eq!(recent.files()[0].path, PathBuf::from("test2.nat"));

        recent.add(PathBuf::from("test1.nat"));
        assert_eq!(recent.files().len(), 2);
        assert_eq!(recent.files()[0].path, PathBuf::from("test1.nat"));
    }

    #[test]
    fn test_config_paths() {
        let config_dir = StartupManager::config_dir();
        assert!(!config_dir.as_os_str().is_empty());
    }
}

// ── Crash reporter (C-4) ────────────────────────────────────────────────────

/// Install a global panic hook that writes a crash report to %APPDATA%\NAT3D\.
///
/// Call once at the very start of `main()` — before logging or GPU init — so
/// that panics during startup are also captured.
///
/// Writes two files:
///   `crash_latest.log`  — always overwritten, for quick inspection
///   `crash_history.log` — append-only, accumulates across runs
pub fn setup_crash_reporter() {
    std::panic::set_hook(Box::new(|panic_info| {
        let msg = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic payload".to_string()
        };

        let location = panic_info
            .location()
            .map(|loc| format!("{}:{}", loc.file(), loc.line()))
            .unwrap_or_else(|| "unknown location".to_string());

        let thread = std::thread::current();
        let thread_name = thread.name().unwrap_or("<unnamed>");

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let report = format!(
            "NAT3D Crash Report\n\
             ==================\n\
             Version  : {ver}\n\
             Timestamp: {ts}\n\
             Thread   : {thread}\n\
             Location : {loc}\n\
             Message  : {msg}\n\
             \n\
             If this error persists, share this file with support:\n\
             pako.molina@gmail.com | github.com/Yatrogenesis/NAT3D\n",
            ver = env!("CARGO_PKG_VERSION"),
            ts = now,
            thread = thread_name,
            loc = location,
            msg = msg,
        );

        let log_dir = std::env::var("APPDATA")
            .map(|d| std::path::PathBuf::from(d).join("NAT3D"))
            .unwrap_or_else(|_| std::path::PathBuf::from(".nat3d"));

        if std::fs::create_dir_all(&log_dir).is_ok() {
            // Overwrite latest
            let _ = std::fs::write(log_dir.join("crash_latest.log"), &report);
            // Append to history (separator between entries)
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_dir.join("crash_history.log"))
            {
                let _ = writeln!(f, "{}\n---\n", report);
            }
        }

        // Also print to stderr so CI/terminal logs capture it
        eprintln!("{}", report);
    }));
}
