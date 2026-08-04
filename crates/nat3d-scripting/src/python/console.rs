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

//! Interactive Python console.
//!
//! Provides an embedded Python interpreter with history and output capture.

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyModule};
use std::ffi::CString;
use std::path::Path;

/// Python console with interactive capabilities.
pub struct PythonConsole {
    /// Command history
    history: Vec<String>,
    /// Output buffer
    output_buffer: Vec<String>,
    /// Python scope (variables)
    scope: Py<PyDict>,
    /// Incomplete statement buffer
    incomplete_buffer: String,
}

impl PythonConsole {
    /// Create a new Python console.
    pub fn new() -> anyhow::Result<Self> {
        Python::with_gil(|py| {
            let scope = PyDict::new(py).into();
            Ok(Self {
                history: Vec::new(),
                output_buffer: Vec::new(),
                scope,
                incomplete_buffer: String::new(),
            })
        })
    }

    /// Execute Python code.
    pub fn execute(&mut self, code: &str) -> anyhow::Result<Option<String>> {
        self.history.push(code.to_string());

        Python::with_gil(|py| {
            let scope = self.scope.bind(py);

            // Capture stdout/stderr
            let sys = PyModule::import(py, "sys")?;
            let io = PyModule::import(py, "io")?;
            let string_io = io.getattr("StringIO")?.call0()?;

            let old_stdout = sys.getattr("stdout")?;
            let old_stderr = sys.getattr("stderr")?;

            sys.setattr("stdout", &string_io)?;
            sys.setattr("stderr", &string_io)?;

            // Try to execute
            let c_code =
                CString::new(code).map_err(|e| anyhow::anyhow!("Invalid code string: {}", e))?;
            let result = py.run(&c_code, Some(scope), Some(scope));

            // Restore stdout/stderr
            sys.setattr("stdout", old_stdout)?;
            sys.setattr("stderr", old_stderr)?;

            // Get output
            let output = string_io.call_method0("getvalue")?.extract::<String>()?;
            if !output.is_empty() {
                self.output_buffer.push(output.clone());
            }

            match result {
                Ok(_) => Ok(Some(output)),
                Err(e) => {
                    let error_msg = format!("Error: {}", e);
                    self.output_buffer.push(error_msg.clone());
                    Err(anyhow::anyhow!(error_msg))
                }
            }
        })
    }

    /// Execute code from a file.
    pub fn execute_file(&mut self, path: &Path) -> anyhow::Result<String> {
        let code = std::fs::read_to_string(path)?;
        self.execute(&code)?;
        Ok(format!("Executed: {}", path.display()))
    }

    /// Get all output.
    pub fn get_output(&self) -> Vec<String> {
        self.output_buffer.clone()
    }

    /// Clear output buffer.
    pub fn clear(&mut self) {
        self.output_buffer.clear();
    }

    /// Add a variable to the Python scope (for basic types like i32).
    pub fn add_to_scope(&mut self, name: &str, value: i32) -> anyhow::Result<()> {
        Python::with_gil(|py| {
            let scope = self.scope.bind(py);
            scope.set_item(name, value)?;
            Ok(())
        })
    }

    /// Get command history.
    pub fn history(&self) -> &[String] {
        &self.history
    }

    /// Check if code is incomplete (needs more lines).
    pub fn is_incomplete(&self, code: &str) -> bool {
        // Simple heuristic: check for unclosed brackets/parens/quotes
        let mut paren_count = 0;
        let mut bracket_count = 0;
        let mut brace_count = 0;
        let mut in_string = false;
        let mut string_char = '\0';

        for ch in code.chars() {
            if in_string {
                if ch == string_char {
                    in_string = false;
                }
            } else {
                match ch {
                    '(' => paren_count += 1,
                    ')' => paren_count -= 1,
                    '[' => bracket_count += 1,
                    ']' => bracket_count -= 1,
                    '{' => brace_count += 1,
                    '}' => brace_count -= 1,
                    '"' | '\'' => {
                        in_string = true;
                        string_char = ch;
                    }
                    _ => {}
                }
            }
        }

        // Incomplete if any brackets/parens are unclosed, in a string,
        // or line ends with ':' (Python block opener like if/for/def/class)
        let ends_with_colon = code.trim().ends_with(':');
        in_string || paren_count > 0 || bracket_count > 0 || brace_count > 0 || ends_with_colon
    }

    /// Execute with multi-line support.
    pub fn execute_multiline(&mut self, line: &str) -> anyhow::Result<Option<String>> {
        self.incomplete_buffer.push_str(line);
        self.incomplete_buffer.push('\n');

        if self.is_incomplete(&self.incomplete_buffer) {
            // Need more input
            Ok(None)
        } else {
            // Complete statement
            let code = self.incomplete_buffer.clone();
            self.incomplete_buffer.clear();
            self.execute(&code)
        }
    }

    /// Clear the incomplete buffer.
    pub fn clear_incomplete(&mut self) {
        self.incomplete_buffer.clear();
    }
}

impl Default for PythonConsole {
    fn default() -> Self {
        Self::new().expect("Failed to create Python console")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_console_creation() {
        let console = PythonConsole::new();
        assert!(console.is_ok());
    }

    #[test]
    fn test_simple_execution() {
        let mut console = PythonConsole::new().unwrap();
        let result = console.execute("x = 42");
        assert!(result.is_ok());
        assert_eq!(console.history().len(), 1);
    }

    #[test]
    fn test_output_capture() {
        let mut console = PythonConsole::new().unwrap();
        console.execute("print('Hello, World!')").unwrap();
        let output = console.get_output();
        assert!(!output.is_empty());
        assert!(output[0].contains("Hello, World!"));
    }

    #[test]
    fn test_add_to_scope() {
        let mut console = PythonConsole::new().unwrap();
        console.add_to_scope("test_var", 123).unwrap();
        let result = console.execute("print(test_var)");
        assert!(result.is_ok());
    }

    #[test]
    fn test_error_handling() {
        let mut console = PythonConsole::new().unwrap();
        let result = console.execute("undefined_variable");
        assert!(result.is_err());
    }

    #[test]
    fn test_incomplete_detection() {
        let console = PythonConsole::new().unwrap();
        assert!(console.is_incomplete("if True:"));
        assert!(console.is_incomplete("def foo("));
        assert!(console.is_incomplete("[1, 2,"));
        assert!(!console.is_incomplete("x = 42"));
    }

    #[test]
    fn test_clear() {
        let mut console = PythonConsole::new().unwrap();
        console.execute("print('test')").unwrap();
        assert!(!console.get_output().is_empty());
        console.clear();
        assert!(console.get_output().is_empty());
    }

    #[test]
    fn test_history() {
        let mut console = PythonConsole::new().unwrap();
        console.execute("x = 1").unwrap();
        console.execute("y = 2").unwrap();
        assert_eq!(console.history().len(), 2);
    }
}
