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

//! Undo/Redo history system.
//!
//! Provides a command-based history system for tracking and reversing
//! changes to the document.

use crate::error::{CoreError, CoreResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use uuid::Uuid;

/// Maximum number of history entries to keep.
pub const DEFAULT_HISTORY_LIMIT: usize = 100;

/// A command that can be executed, undone, and redone.
pub trait Command: std::fmt::Debug + Send + Sync {
    /// Get the display name of this command.
    fn name(&self) -> &str;

    /// Execute the command.
    fn execute(&mut self, context: &mut dyn CommandContext) -> CoreResult<()>;

    /// Undo the command.
    fn undo(&mut self, context: &mut dyn CommandContext) -> CoreResult<()>;

    /// Check if this command can be merged with another.
    fn can_merge(&self, _other: &dyn Command) -> bool {
        false
    }

    /// Merge with another command (if `can_merge` returns true).
    fn merge(&mut self, _other: Box<dyn Command>) -> CoreResult<()> {
        Err(CoreError::NotSupported(
            "Command merging not supported".into(),
        ))
    }

    /// Get the estimated memory size of this command.
    fn memory_size(&self) -> usize {
        std::mem::size_of_val(self)
    }
}

/// Context provided to commands for execution.
pub trait CommandContext {
    /// Get a reference to the document data.
    fn document(&self) -> &dyn std::any::Any;

    /// Get a mutable reference to the document data.
    fn document_mut(&mut self) -> &mut dyn std::any::Any;
}

/// An entry in the history stack.
#[derive(Debug)]
pub struct HistoryEntry {
    /// Unique identifier for this entry.
    pub id: Uuid,
    /// The command.
    pub command: Box<dyn Command>,
    /// When the command was executed.
    pub timestamp: DateTime<Utc>,
    /// Optional description override.
    pub description: Option<String>,
}

impl HistoryEntry {
    /// Create a new history entry.
    #[must_use]
    pub fn new(command: Box<dyn Command>) -> Self {
        Self {
            id: Uuid::new_v4(),
            command,
            timestamp: Utc::now(),
            description: None,
        }
    }

    /// Create a new history entry with a description.
    pub fn with_description(command: Box<dyn Command>, description: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            command,
            timestamp: Utc::now(),
            description: Some(description.into()),
        }
    }

    /// Get the display name for this entry.
    #[must_use]
    pub fn name(&self) -> &str {
        self.description
            .as_deref()
            .unwrap_or_else(|| self.command.name())
    }
}

/// History state for serialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryState {
    /// Names of undo stack entries.
    pub undo_names: Vec<String>,
    /// Names of redo stack entries.
    pub redo_names: Vec<String>,
    /// Current position in history.
    pub position: usize,
    /// Whether history is modified since last save.
    pub is_modified: bool,
}

/// The history manager for undo/redo operations.
#[derive(Debug)]
pub struct History {
    /// Stack of commands that can be undone.
    undo_stack: VecDeque<HistoryEntry>,
    /// Stack of commands that can be redone.
    redo_stack: VecDeque<HistoryEntry>,
    /// Maximum number of entries to keep.
    limit: usize,
    /// Whether the document has been modified since the last save.
    is_modified: bool,
    /// Position of last save (for determining modification state).
    save_position: usize,
    /// Total commands executed (for save position tracking).
    total_commands: usize,
    /// Whether history recording is enabled.
    recording_enabled: bool,
    /// Nested transaction depth.
    transaction_depth: usize,
    /// Commands accumulated in current transaction.
    transaction_commands: Vec<Box<dyn Command>>,
}

impl History {
    /// Create a new history manager.
    #[must_use]
    pub fn new() -> Self {
        Self::with_limit(DEFAULT_HISTORY_LIMIT)
    }

    /// Create a new history manager with a custom limit.
    #[must_use]
    pub fn with_limit(limit: usize) -> Self {
        Self {
            undo_stack: VecDeque::new(),
            redo_stack: VecDeque::new(),
            limit,
            is_modified: false,
            save_position: 0,
            total_commands: 0,
            recording_enabled: true,
            transaction_depth: 0,
            transaction_commands: Vec::new(),
        }
    }

    /// Execute a command and add it to history.
    pub fn execute(
        &mut self,
        mut command: Box<dyn Command>,
        context: &mut dyn CommandContext,
    ) -> CoreResult<()> {
        // Execute the command
        command.execute(context)?;

        // If we're in a transaction, accumulate the command
        if self.transaction_depth > 0 {
            self.transaction_commands.push(command);
            return Ok(());
        }

        // If recording is disabled, don't add to history
        if !self.recording_enabled {
            return Ok(());
        }

        // Try to merge with the previous command
        if let Some(last) = self.undo_stack.back_mut() {
            if last.command.can_merge(&*command) {
                last.command.merge(command)?;
                last.timestamp = Utc::now();
                self.is_modified = true;
                return Ok(());
            }
        }

        // Add to undo stack
        self.undo_stack.push_back(HistoryEntry::new(command));
        self.total_commands += 1;

        // Clear redo stack
        self.redo_stack.clear();

        // Enforce limit
        while self.undo_stack.len() > self.limit {
            self.undo_stack.pop_front();
        }

        self.is_modified = self.total_commands != self.save_position;

        Ok(())
    }

    /// Undo the last command.
    pub fn undo(&mut self, context: &mut dyn CommandContext) -> CoreResult<()> {
        let mut entry = self.undo_stack.pop_back().ok_or(CoreError::NothingToUndo)?;

        entry.command.undo(context)?;

        self.redo_stack.push_back(entry);
        self.total_commands -= 1;
        self.is_modified = self.total_commands != self.save_position;

        Ok(())
    }

    /// Redo the last undone command.
    pub fn redo(&mut self, context: &mut dyn CommandContext) -> CoreResult<()> {
        let mut entry = self.redo_stack.pop_back().ok_or(CoreError::NothingToRedo)?;

        entry.command.execute(context)?;

        self.undo_stack.push_back(entry);
        self.total_commands += 1;
        self.is_modified = self.total_commands != self.save_position;

        Ok(())
    }

    /// Check if undo is available.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Check if redo is available.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Get the name of the next undo operation.
    #[must_use]
    pub fn undo_name(&self) -> Option<&str> {
        self.undo_stack.back().map(HistoryEntry::name)
    }

    /// Get the name of the next redo operation.
    #[must_use]
    pub fn redo_name(&self) -> Option<&str> {
        self.redo_stack.back().map(HistoryEntry::name)
    }

    /// Get the undo stack entries.
    pub fn undo_entries(&self) -> impl Iterator<Item = &HistoryEntry> {
        self.undo_stack.iter()
    }

    /// Get the redo stack entries.
    pub fn redo_entries(&self) -> impl Iterator<Item = &HistoryEntry> {
        self.redo_stack.iter()
    }

    /// Get the number of undo steps available.
    #[must_use]
    pub fn undo_count(&self) -> usize {
        self.undo_stack.len()
    }

    /// Get the number of redo steps available.
    #[must_use]
    pub fn redo_count(&self) -> usize {
        self.redo_stack.len()
    }

    /// Check if the document has been modified since the last save.
    #[must_use]
    pub fn is_modified(&self) -> bool {
        self.is_modified
    }

    /// Mark the current state as saved.
    pub fn mark_saved(&mut self) {
        self.save_position = self.total_commands;
        self.is_modified = false;
    }

    /// Clear all history.
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.total_commands = 0;
        self.save_position = 0;
        self.is_modified = false;
    }

    /// Enable or disable history recording.
    pub fn set_recording_enabled(&mut self, enabled: bool) {
        self.recording_enabled = enabled;
    }

    /// Check if recording is enabled.
    #[must_use]
    pub fn is_recording_enabled(&self) -> bool {
        self.recording_enabled
    }

    /// Begin a transaction (commands will be grouped).
    pub fn begin_transaction(&mut self) {
        self.transaction_depth += 1;
    }

    /// End a transaction and create a composite command.
    pub fn end_transaction(&mut self, name: impl Into<String>) -> CoreResult<()> {
        if self.transaction_depth == 0 {
            return Err(CoreError::Internal("No transaction to end".into()));
        }

        self.transaction_depth -= 1;

        if self.transaction_depth == 0 && !self.transaction_commands.is_empty() {
            let commands = std::mem::take(&mut self.transaction_commands);
            let composite = CompositeCommand::new(name.into(), commands);

            // Add composite command to history (without executing, already executed)
            self.undo_stack
                .push_back(HistoryEntry::new(Box::new(composite)));
            self.total_commands += 1;
            self.redo_stack.clear();

            while self.undo_stack.len() > self.limit {
                self.undo_stack.pop_front();
            }

            self.is_modified = true;
        }

        Ok(())
    }

    /// Cancel a transaction and undo all commands in it.
    pub fn cancel_transaction(&mut self, context: &mut dyn CommandContext) -> CoreResult<()> {
        if self.transaction_depth == 0 {
            return Err(CoreError::Internal("No transaction to cancel".into()));
        }

        self.transaction_depth = 0;

        // Undo all transaction commands in reverse order
        for mut cmd in self.transaction_commands.drain(..).rev() {
            cmd.undo(context)?;
        }

        Ok(())
    }

    /// Get the current state for serialization.
    #[must_use]
    pub fn state(&self) -> HistoryState {
        HistoryState {
            undo_names: self
                .undo_stack
                .iter()
                .map(|e| e.name().to_string())
                .collect(),
            redo_names: self
                .redo_stack
                .iter()
                .map(|e| e.name().to_string())
                .collect(),
            position: self.total_commands,
            is_modified: self.is_modified,
        }
    }

    /// Get estimated memory usage.
    #[must_use]
    pub fn memory_usage(&self) -> usize {
        let undo_size: usize = self
            .undo_stack
            .iter()
            .map(|e| e.command.memory_size())
            .sum();
        let redo_size: usize = self
            .redo_stack
            .iter()
            .map(|e| e.command.memory_size())
            .sum();
        undo_size + redo_size
    }
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

/// A command that groups multiple commands together.
#[derive(Debug)]
pub struct CompositeCommand {
    name: String,
    commands: Vec<Box<dyn Command>>,
}

impl CompositeCommand {
    /// Create a new composite command.
    #[must_use]
    pub fn new(name: String, commands: Vec<Box<dyn Command>>) -> Self {
        Self { name, commands }
    }
}

impl Command for CompositeCommand {
    fn name(&self) -> &str {
        &self.name
    }

    fn execute(&mut self, context: &mut dyn CommandContext) -> CoreResult<()> {
        for cmd in &mut self.commands {
            cmd.execute(context)?;
        }
        Ok(())
    }

    fn undo(&mut self, context: &mut dyn CommandContext) -> CoreResult<()> {
        for cmd in self.commands.iter_mut().rev() {
            cmd.undo(context)?;
        }
        Ok(())
    }

    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.name.len()
            + self.commands.iter().map(|c| c.memory_size()).sum::<usize>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestCommand {
        _value: i32,
        executed: bool,
    }

    impl Command for TestCommand {
        fn name(&self) -> &str {
            "Test Command"
        }

        fn execute(&mut self, _context: &mut dyn CommandContext) -> CoreResult<()> {
            self.executed = true;
            Ok(())
        }

        fn undo(&mut self, _context: &mut dyn CommandContext) -> CoreResult<()> {
            self.executed = false;
            Ok(())
        }
    }

    struct TestContext {
        data: i32,
    }

    impl CommandContext for TestContext {
        fn document(&self) -> &dyn std::any::Any {
            &self.data
        }

        fn document_mut(&mut self) -> &mut dyn std::any::Any {
            &mut self.data
        }
    }

    #[test]
    fn test_history_creation() {
        let history = History::new();
        assert!(!history.can_undo());
        assert!(!history.can_redo());
        assert!(!history.is_modified());
    }

    #[test]
    fn test_execute_and_undo() {
        let mut history = History::new();
        let mut context = TestContext { data: 0 };

        let cmd = Box::new(TestCommand {
            _value: 1,
            executed: false,
        });
        history.execute(cmd, &mut context).unwrap();

        assert!(history.can_undo());
        assert!(!history.can_redo());
        assert!(history.is_modified());

        history.undo(&mut context).unwrap();

        assert!(!history.can_undo());
        assert!(history.can_redo());
    }

    #[test]
    fn test_redo() {
        let mut history = History::new();
        let mut context = TestContext { data: 0 };

        let cmd = Box::new(TestCommand {
            _value: 1,
            executed: false,
        });
        history.execute(cmd, &mut context).unwrap();
        history.undo(&mut context).unwrap();
        history.redo(&mut context).unwrap();

        assert!(history.can_undo());
        assert!(!history.can_redo());
    }

    #[test]
    fn test_mark_saved() {
        let mut history = History::new();
        let mut context = TestContext { data: 0 };

        let cmd = Box::new(TestCommand {
            _value: 1,
            executed: false,
        });
        history.execute(cmd, &mut context).unwrap();

        assert!(history.is_modified());
        history.mark_saved();
        assert!(!history.is_modified());

        let cmd2 = Box::new(TestCommand {
            _value: 2,
            executed: false,
        });
        history.execute(cmd2, &mut context).unwrap();
        assert!(history.is_modified());

        history.undo(&mut context).unwrap();
        assert!(!history.is_modified());
    }

    #[test]
    fn test_history_limit() {
        let mut history = History::with_limit(5);
        let mut context = TestContext { data: 0 };

        for i in 0..10 {
            let cmd = Box::new(TestCommand {
                _value: i,
                executed: false,
            });
            history.execute(cmd, &mut context).unwrap();
        }

        assert_eq!(history.undo_count(), 5);
    }
}
