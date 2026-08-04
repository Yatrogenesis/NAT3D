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

//! Scripting macros.
//!
//! Provides convenient macros for defining script commands and Python bindings.

/// Define a script command.
///
/// This macro simplifies the creation of script commands by automatically
/// implementing the `ScriptCommand` trait.
///
/// # Example
///
/// ```ignore
/// script_command! {
///     name: "my_command",
///     description: "Does something",
///     args: [f32, f32],
///     execute: |args| {
///         let x = args[0].downcast_ref::<f32>().unwrap();
///         let y = args[1].downcast_ref::<f32>().unwrap();
///         Ok(Box::new(x + y))
///     }
/// }
/// ```
#[macro_export]
macro_rules! script_command {
    (
        name: $name:expr,
        description: $desc:expr,
        args: [$($arg_type:ty),*],
        execute: $exec:expr
    ) => {{
        struct GeneratedCommand;

        impl $crate::rust_api::prelude::ScriptCommand for GeneratedCommand {
            fn name(&self) -> &str {
                $name
            }

            fn description(&self) -> &str {
                $desc
            }

            fn execute(
                &self,
                args: &[Box<dyn std::any::Any>],
            ) -> $crate::rust_api::prelude::ScriptResult<Box<dyn std::any::Any>> {
                ($exec)(args)
            }
        }

        GeneratedCommand
    }};
}

/// Expose a Rust function to Python.
///
/// This macro generates PyO3 wrapper code for a Rust function.
///
/// # Example
///
/// ```ignore
/// expose_to_python! {
///     fn add(a: f32, b: f32) -> f32 {
///         a + b
///     }
/// }
/// ```
#[macro_export]
macro_rules! expose_to_python {
    (
        fn $name:ident($($arg:ident: $arg_ty:ty),*) -> $ret:ty $body:block
    ) => {
        #[pyo3::pyfunction]
        fn $name($($arg: $arg_ty),*) -> $ret $body
    };
}

/// Helper macro for error handling in scripts.
///
/// # Example
///
/// ```ignore
/// script_try! {
///     some_fallible_operation()?
/// }
/// ```
#[macro_export]
macro_rules! script_try {
    ($expr:expr) => {
        match $expr {
            Ok(val) => val,
            Err(e) => {
                return Err($crate::rust_api::prelude::ScriptError::ExecutionFailed(
                    e.to_string(),
                ))
            }
        }
    };
}

/// Create a script result.
#[macro_export]
macro_rules! script_ok {
    ($expr:expr) => {
        Ok(Box::new($expr) as Box<dyn std::any::Any>)
    };
}

/// Create a script error.
#[macro_export]
macro_rules! script_err {
    ($msg:expr) => {
        Err($crate::rust_api::prelude::ScriptError::ExecutionFailed(
            $msg.to_string(),
        ))
    };
}

#[cfg(test)]
mod tests {
    use crate::rust_api::prelude::*;

    #[test]
    fn test_script_command_macro() {
        let _cmd = script_command! {
            name: "test_command",
            description: "A test command",
            args: [f32],
            execute: |args: &[Box<dyn std::any::Any>]| {
                if args.is_empty() {
                    return Err(ScriptError::InvalidArguments("Expected 1 argument".to_string()));
                }
                Ok(Box::new(42) as Box<dyn std::any::Any>)
            }
        };

        assert_eq!(_cmd.name(), "test_command");
        assert_eq!(_cmd.description(), "A test command");
    }

    #[test]
    fn test_script_ok_macro() {
        let result: Result<Box<dyn std::any::Any>, ScriptError> = script_ok!(42);
        assert!(result.is_ok());
    }

    #[test]
    fn test_script_err_macro() {
        let result: Result<Box<dyn std::any::Any>, ScriptError> = script_err!("test error");
        assert!(result.is_err());
        match result {
            Err(ScriptError::ExecutionFailed(msg)) => assert_eq!(msg, "test error"),
            _ => panic!("Wrong error type"),
        }
    }
}
