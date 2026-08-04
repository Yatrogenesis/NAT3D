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

//! Console and logging system for NAT3D.

use std::collections::VecDeque;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// Log level.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

#[allow(dead_code)]
impl LogLevel {
    /// Get color for log level.
    pub fn color(&self) -> [u8; 3] {
        match self {
            Self::Debug => [128, 128, 128],
            Self::Info => [200, 200, 200],
            Self::Warning => [255, 200, 100],
            Self::Error => [255, 100, 100],
        }
    }

    /// Get prefix for log level.
    pub fn prefix(&self) -> &'static str {
        match self {
            Self::Debug => "[DEBUG]",
            Self::Info => "[INFO]",
            Self::Warning => "[WARN]",
            Self::Error => "[ERROR]",
        }
    }
}

/// A log entry.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level: LogLevel,
    pub message: String,
    pub source: Option<String>,
    pub timestamp: f64,
    pub count: u32,
}

#[allow(dead_code)]
impl LogEntry {
    /// Create a new log entry.
    pub fn new(level: LogLevel, message: &str) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);

        Self {
            level,
            message: message.to_string(),
            source: None,
            timestamp,
            count: 1,
        }
    }

    /// Create with source.
    pub fn with_source(mut self, source: &str) -> Self {
        self.source = Some(source.to_string());
        self
    }

    /// Format timestamp.
    pub fn format_time(&self) -> String {
        let secs = self.timestamp as u64;
        let hours = (secs / 3600) % 24;
        let mins = (secs / 60) % 60;
        let secs = secs % 60;
        format!("{:02}:{:02}:{:02}", hours, mins, secs)
    }
}

/// Console with log history and command execution.
#[allow(dead_code)]
pub struct Console {
    entries: VecDeque<LogEntry>,
    max_entries: usize,
    min_level: LogLevel,
    pub is_open: bool,
    command_history: Vec<String>,
    history_index: usize,
    current_input: String,
    pub auto_scroll: bool,
    pub collapse_duplicates: bool,
    filter_text: String,
    start_time: Instant,
}

#[allow(dead_code)]
impl Console {
    /// Create a new console.
    pub fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            max_entries: 1000,
            min_level: LogLevel::Debug,
            is_open: false,
            command_history: Vec::new(),
            history_index: 0,
            current_input: String::new(),
            auto_scroll: true,
            collapse_duplicates: true,
            filter_text: String::new(),
            start_time: Instant::now(),
        }
    }

    /// Log a message.
    pub fn log(&mut self, level: LogLevel, message: &str) {
        if level < self.min_level {
            return;
        }

        // Check for duplicate (collapse)
        if self.collapse_duplicates {
            if let Some(last) = self.entries.back_mut() {
                if last.message == message && last.level == level {
                    last.count += 1;
                    return;
                }
            }
        }

        let entry = LogEntry::new(level, message);
        self.entries.push_back(entry);

        // Trim old entries
        while self.entries.len() > self.max_entries {
            self.entries.pop_front();
        }
    }

    /// Log debug message.
    pub fn debug(&mut self, message: &str) {
        self.log(LogLevel::Debug, message);
    }

    /// Log info message.
    pub fn info(&mut self, message: &str) {
        self.log(LogLevel::Info, message);
    }

    /// Log warning message.
    pub fn warn(&mut self, message: &str) {
        self.log(LogLevel::Warning, message);
    }

    /// Log error message.
    pub fn error(&mut self, message: &str) {
        self.log(LogLevel::Error, message);
    }

    /// Clear all logs.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Get filtered entries.
    pub fn filtered_entries(&self) -> impl Iterator<Item = &LogEntry> {
        self.entries.iter().filter(|e| {
            if !self.filter_text.is_empty() {
                e.message
                    .to_lowercase()
                    .contains(&self.filter_text.to_lowercase())
            } else {
                true
            }
        })
    }

    /// Get all entries.
    pub fn entries(&self) -> impl Iterator<Item = &LogEntry> {
        self.entries.iter()
    }

    /// Entry count.
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Set minimum log level.
    pub fn set_min_level(&mut self, level: LogLevel) {
        self.min_level = level;
    }

    /// Get filter text.
    pub fn filter(&self) -> &str {
        &self.filter_text
    }

    /// Set filter text.
    pub fn set_filter(&mut self, filter: &str) {
        self.filter_text = filter.to_string();
    }

    /// Execute a command.
    pub fn execute(&mut self, command: &str) -> String {
        let command = command.trim();
        if command.is_empty() {
            return String::new();
        }

        // Add to history
        self.command_history.push(command.to_string());
        self.history_index = self.command_history.len();

        // Log the command
        self.info(&format!("> {}", command));

        // Parse and execute
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return String::new();
        }

        let result = match parts[0] {
            "help" => self.cmd_help(),
            "clear" => {
                self.clear();
                "Console cleared".to_string()
            }
            "version" => format!("NAT3D v{}", env!("CARGO_PKG_VERSION")),
            "time" => format!("Uptime: {:.1}s", self.start_time.elapsed().as_secs_f32()),
            "echo" => parts[1..].join(" "),
            "log" => {
                if parts.len() >= 3 {
                    let level = match parts[1] {
                        "debug" => LogLevel::Debug,
                        "info" => LogLevel::Info,
                        "warn" => LogLevel::Warning,
                        "error" => LogLevel::Error,
                        _ => LogLevel::Info,
                    };
                    self.log(level, &parts[2..].join(" "));
                    "Logged".to_string()
                } else {
                    "Usage: log <level> <message>".to_string()
                }
            }
            "stats" => format!(
                "Log entries: {}\nHistory: {} commands",
                self.entries.len(),
                self.command_history.len()
            ),
            _ => format!(
                "Unknown command: {}. Type 'help' for available commands.",
                parts[0]
            ),
        };

        if !result.is_empty() {
            self.info(&result);
        }

        result
    }

    fn cmd_help(&self) -> String {
        r#"Available commands:
  help     - Show this help
  clear    - Clear console
  version  - Show version
  time     - Show uptime
  echo     - Echo text
  log      - Log message (log <level> <message>)
  stats    - Show console stats"#
            .to_string()
    }

    /// Navigate history up.
    pub fn history_up(&mut self) -> Option<&str> {
        if self.history_index > 0 {
            self.history_index -= 1;
            Some(&self.command_history[self.history_index])
        } else {
            None
        }
    }

    /// Navigate history down.
    pub fn history_down(&mut self) -> Option<&str> {
        if self.history_index < self.command_history.len() {
            self.history_index += 1;
            if self.history_index < self.command_history.len() {
                Some(&self.command_history[self.history_index])
            } else {
                Some("")
            }
        } else {
            None
        }
    }

    /// Get/set current input.
    pub fn input(&self) -> &str {
        &self.current_input
    }

    pub fn set_input(&mut self, input: &str) {
        self.current_input = input.to_string();
    }

    /// Toggle console.
    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
    }

    /// Count by level.
    pub fn count_by_level(&self, level: LogLevel) -> usize {
        self.entries.iter().filter(|e| e.level == level).count()
    }

    /// Error count.
    pub fn error_count(&self) -> usize {
        self.count_by_level(LogLevel::Error)
    }

    /// Warning count.
    pub fn warning_count(&self) -> usize {
        self.count_by_level(LogLevel::Warning)
    }
}

impl Default for Console {
    fn default() -> Self {
        Self::new()
    }
}

/// Global console macro helper.
#[macro_export]
macro_rules! console_log {
    ($console:expr, $level:expr, $($arg:tt)*) => {
        $console.log($level, &format!($($arg)*))
    };
}

/// Performance profiler.
#[allow(dead_code)]
pub struct Profiler {
    samples: VecDeque<f32>,
    max_samples: usize,
    current_frame_start: Instant,
}

#[allow(dead_code)]
impl Profiler {
    /// Create a new profiler.
    pub fn new() -> Self {
        Self {
            samples: VecDeque::new(),
            max_samples: 120,
            current_frame_start: Instant::now(),
        }
    }

    /// Start a new frame.
    pub fn start_frame(&mut self) {
        self.current_frame_start = Instant::now();
    }

    /// End frame and record sample.
    pub fn end_frame(&mut self) {
        let elapsed = self.current_frame_start.elapsed().as_secs_f32() * 1000.0;
        self.samples.push_back(elapsed);
        while self.samples.len() > self.max_samples {
            self.samples.pop_front();
        }
    }

    /// Get average frame time.
    pub fn average_ms(&self) -> f32 {
        if self.samples.is_empty() {
            return 0.0;
        }
        self.samples.iter().sum::<f32>() / self.samples.len() as f32
    }

    /// Get FPS.
    pub fn fps(&self) -> f32 {
        let avg = self.average_ms();
        if avg > 0.0 {
            1000.0 / avg
        } else {
            0.0
        }
    }

    /// Get min frame time.
    pub fn min_ms(&self) -> f32 {
        self.samples.iter().copied().fold(f32::MAX, f32::min)
    }

    /// Get max frame time.
    pub fn max_ms(&self) -> f32 {
        self.samples.iter().copied().fold(f32::MIN, f32::max)
    }

    /// Get samples for graphing.
    pub fn samples(&self) -> &VecDeque<f32> {
        &self.samples
    }

    /// Get sample count.
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }
}

impl Default for Profiler {
    fn default() -> Self {
        Self::new()
    }
}

/// Memory tracker.
#[allow(dead_code)]
pub struct MemoryTracker {
    allocations: usize,
    total_allocated: usize,
    peak_allocated: usize,
}

#[allow(dead_code)]
impl MemoryTracker {
    /// Create a new memory tracker.
    pub fn new() -> Self {
        Self {
            allocations: 0,
            total_allocated: 0,
            peak_allocated: 0,
        }
    }

    /// Record allocation.
    pub fn allocate(&mut self, size: usize) {
        self.allocations += 1;
        self.total_allocated += size;
        self.peak_allocated = self.peak_allocated.max(self.total_allocated);
    }

    /// Record deallocation.
    pub fn deallocate(&mut self, size: usize) {
        self.total_allocated = self.total_allocated.saturating_sub(size);
    }

    /// Get current allocation.
    pub fn current(&self) -> usize {
        self.total_allocated
    }

    /// Get peak allocation.
    pub fn peak(&self) -> usize {
        self.peak_allocated
    }

    /// Get allocation count.
    pub fn allocation_count(&self) -> usize {
        self.allocations
    }

    /// Format bytes.
    pub fn format_bytes(bytes: usize) -> String {
        if bytes >= 1024 * 1024 * 1024 {
            format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
        } else if bytes >= 1024 * 1024 {
            format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
        } else if bytes >= 1024 {
            format!("{:.2} KB", bytes as f64 / 1024.0)
        } else {
            format!("{} B", bytes)
        }
    }
}

impl Default for MemoryTracker {
    fn default() -> Self {
        Self::new()
    }
}
