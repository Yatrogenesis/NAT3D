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

//! Rust scripting API prelude.
//!
//! This module provides a convenient prelude for Rust-based scripting in NAT3D.

// ScriptError variants use #[error(...)] attributes from thiserror which provide
// documentation via the error message. Adding doc comments would be redundant.
#![allow(missing_docs)]

use std::any::Any;
use std::collections::HashMap;

// Re-export commonly used types
pub use nat3d_core::prelude::*;
pub use nat3d_math::prelude::*;

/// Result type for scripting operations.
pub type ScriptResult<T> = Result<T, ScriptError>;

/// Script execution errors.
#[derive(Debug, thiserror::Error)]
pub enum ScriptError {
    #[error("Command not found: {0}")]
    CommandNotFound(String),

    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),

    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Undo not available")]
    UndoNotAvailable,

    #[error("Core error: {0}")]
    Core(#[from] nat3d_core::CoreError),
}

/// Script command trait.
///
/// Implement this trait to create custom commands that can be registered
/// with the command registry.
pub trait ScriptCommand: Send + Sync {
    /// Get the command name.
    fn name(&self) -> &str;

    /// Execute the command with arguments.
    fn execute(&self, args: &[Box<dyn Any>]) -> ScriptResult<Box<dyn Any>>;

    /// Undo the command (optional).
    fn undo(&self) -> ScriptResult<()> {
        Err(ScriptError::UndoNotAvailable)
    }

    /// Get command description.
    fn description(&self) -> &str {
        ""
    }

    /// Get expected argument types.
    fn argument_types(&self) -> Vec<&str> {
        Vec::new()
    }
}

/// Command registry for managing script commands.
pub struct CommandRegistry {
    commands: HashMap<String, Box<dyn ScriptCommand>>,
}

impl CommandRegistry {
    /// Create a new command registry.
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
        }
    }

    /// Register a command.
    pub fn register<C: ScriptCommand + 'static>(&mut self, command: C) {
        let name = command.name().to_string();
        self.commands.insert(name, Box::new(command));
    }

    /// Execute a command by name.
    pub fn execute(&self, name: &str, args: &[Box<dyn Any>]) -> ScriptResult<Box<dyn Any>> {
        let command = self
            .commands
            .get(name)
            .ok_or_else(|| ScriptError::CommandNotFound(name.to_string()))?;

        command.execute(args)
    }

    /// List all registered commands.
    pub fn list_commands(&self) -> Vec<&str> {
        self.commands.keys().map(|s| s.as_str()).collect()
    }

    /// Get command by name.
    pub fn get_command(&self, name: &str) -> Option<&dyn ScriptCommand> {
        self.commands.get(name).map(|c| &**c)
    }

    /// Check if a command is registered.
    pub fn has_command(&self, name: &str) -> bool {
        self.commands.contains_key(name)
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Example command: Create a cube.
pub struct CreateCubeCommand;

impl ScriptCommand for CreateCubeCommand {
    fn name(&self) -> &str {
        "create_cube"
    }

    fn execute(&self, args: &[Box<dyn Any>]) -> ScriptResult<Box<dyn Any>> {
        let size = if args.is_empty() {
            1.0
        } else {
            args[0]
                .downcast_ref::<f32>()
                .copied()
                .ok_or_else(|| ScriptError::InvalidArguments("Expected f32 for size".to_string()))?
        };

        let mesh = Mesh::cube(size as f64);
        Ok(Box::new(mesh))
    }

    fn description(&self) -> &str {
        "Create a cube mesh"
    }

    fn argument_types(&self) -> Vec<&str> {
        vec!["f32"]
    }
}

/// Example command: Translate mesh.
pub struct TranslateMeshCommand;

impl ScriptCommand for TranslateMeshCommand {
    fn name(&self) -> &str {
        "translate_mesh"
    }

    fn execute(&self, args: &[Box<dyn Any>]) -> ScriptResult<Box<dyn Any>> {
        if args.len() < 2 {
            return Err(ScriptError::InvalidArguments(
                "translate_mesh requires (Mesh, [f64; 3])".to_string(),
            ));
        }

        let mesh = args[0]
            .downcast_ref::<Mesh>()
            .ok_or_else(|| ScriptError::InvalidArguments("First arg must be Mesh".to_string()))?;

        let offset = args[1].downcast_ref::<[f64; 3]>().ok_or_else(|| {
            ScriptError::InvalidArguments("Second arg must be [f64; 3]".to_string())
        })?;

        let mut translated = mesh.clone();
        let vertex_count = translated.vertex_count();
        for i in 0..vertex_count {
            if let Ok(v) = translated.vertex(i) {
                let pos = v.position();
                let new_pos =
                    Position::new(pos.x + offset[0], pos.y + offset[1], pos.z + offset[2]);
                let _ = translated.set_vertex_position(i, new_pos);
            }
        }
        Ok(Box::new(translated))
    }

    fn description(&self) -> &str {
        "Translate a mesh by an offset vector"
    }

    fn argument_types(&self) -> Vec<&str> {
        vec!["Mesh", "[f64; 3]"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_registry_creation() {
        let registry = CommandRegistry::new();
        assert_eq!(registry.list_commands().len(), 0);
    }

    #[test]
    fn test_register_command() {
        let mut registry = CommandRegistry::new();
        registry.register(CreateCubeCommand);

        assert!(registry.has_command("create_cube"));
        assert_eq!(registry.list_commands().len(), 1);
    }

    #[test]
    fn test_execute_command() {
        let mut registry = CommandRegistry::new();
        registry.register(CreateCubeCommand);

        let size: Box<dyn Any> = Box::new(2.0f32);
        let result = registry.execute("create_cube", &[size]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_nonexistent_command() {
        let registry = CommandRegistry::new();
        let result = registry.execute("nonexistent", &[]);
        assert!(matches!(result, Err(ScriptError::CommandNotFound(_))));
    }

    #[test]
    fn test_get_command() {
        let mut registry = CommandRegistry::new();
        registry.register(CreateCubeCommand);

        let cmd = registry.get_command("create_cube");
        assert!(cmd.is_some());
        assert_eq!(cmd.unwrap().name(), "create_cube");
    }

    #[test]
    fn test_command_description() {
        let cmd = CreateCubeCommand;
        assert!(!cmd.description().is_empty());
        assert_eq!(cmd.argument_types().len(), 1);
    }
}
