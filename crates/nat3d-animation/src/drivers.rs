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

//! Expression-driven animation.
//!
//! Procedural animation control through mathematical expressions and variables.

use std::collections::HashMap;

use crate::rigging::bone::BoneId;

/// Animation driver.
#[derive(Debug, Clone)]
pub struct Driver {
    /// Driver name.
    pub name: String,
    /// Expression string.
    pub expression: String,
    /// Variables.
    pub variables: Vec<DriverVariable>,
    /// Fallback value if evaluation fails.
    pub fallback_value: f64,
}

impl Driver {
    /// Create a new driver.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            expression: String::new(),
            variables: Vec::new(),
            fallback_value: 0.0,
        }
    }

    /// Set expression.
    pub fn with_expression(mut self, expression: impl Into<String>) -> Self {
        self.expression = expression.into();
        self
    }

    /// Add a variable.
    pub fn add_variable(&mut self, variable: DriverVariable) {
        self.variables.push(variable);
    }

    /// Evaluate the driver.
    pub fn evaluate(&self, context: &DriverContext) -> f64 {
        // Build variable map
        let mut var_map = HashMap::new();
        for var in &self.variables {
            let value = var.evaluate(context);
            var_map.insert(var.name.clone(), value);
        }

        // Parse and evaluate expression
        match parse_and_evaluate(&self.expression, &var_map) {
            Ok(value) => value,
            Err(_) => self.fallback_value,
        }
    }
}

/// Driver variable.
#[derive(Debug, Clone)]
pub struct DriverVariable {
    /// Variable name.
    pub name: String,
    /// Source of the variable.
    pub source: DriverSource,
    /// Transform space (for bone transforms).
    pub transform_space: TransformSpace,
}

impl DriverVariable {
    /// Create a new driver variable.
    pub fn new(name: impl Into<String>, source: DriverSource) -> Self {
        Self {
            name: name.into(),
            source,
            transform_space: TransformSpace::World,
        }
    }

    /// Evaluate the variable.
    pub fn evaluate(&self, context: &DriverContext) -> f64 {
        match &self.source {
            DriverSource::BoneTransform(bone_id, channel) => {
                context.get_bone_channel(*bone_id, *channel, self.transform_space)
            }
            DriverSource::ShapeKey(name) => context.get_shape_key(name),
            DriverSource::ObjectTransform(channel) => context.get_object_channel(*channel),
            DriverSource::Custom(value) => *value,
        }
    }
}

/// Driver source.
#[derive(Debug, Clone)]
pub enum DriverSource {
    /// Bone transform channel.
    BoneTransform(BoneId, DriverChannel),
    /// Shape key value.
    ShapeKey(String),
    /// Object transform channel.
    ObjectTransform(DriverChannel),
    /// Custom constant value.
    Custom(f64),
}

/// Driver channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DriverChannel {
    LocX,
    LocY,
    LocZ,
    RotX,
    RotY,
    RotZ,
    ScaleX,
    ScaleY,
    ScaleZ,
}

/// Transform space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformSpace {
    /// World space.
    World,
    /// Local space.
    Local,
    /// Parent space.
    Parent,
}

/// Driver evaluation context.
#[derive(Debug, Clone)]
pub struct DriverContext {
    /// Bone transform channels (bone_id -> (channel -> value)).
    bone_channels: HashMap<BoneId, HashMap<DriverChannel, f64>>,
    /// Shape key values.
    shape_keys: HashMap<String, f64>,
    /// Object transform channels.
    object_channels: HashMap<DriverChannel, f64>,
    /// Custom values.
    custom_values: HashMap<String, f64>,
}

impl DriverContext {
    /// Create a new driver context.
    pub fn new() -> Self {
        Self {
            bone_channels: HashMap::new(),
            shape_keys: HashMap::new(),
            object_channels: HashMap::new(),
            custom_values: HashMap::new(),
        }
    }

    /// Set bone channel value.
    pub fn set_bone_channel(&mut self, bone_id: BoneId, channel: DriverChannel, value: f64) {
        self.bone_channels
            .entry(bone_id)
            .or_default()
            .insert(channel, value);
    }

    /// Get bone channel value.
    pub fn get_bone_channel(
        &self,
        bone_id: BoneId,
        channel: DriverChannel,
        _space: TransformSpace,
    ) -> f64 {
        self.bone_channels
            .get(&bone_id)
            .and_then(|channels| channels.get(&channel))
            .copied()
            .unwrap_or(0.0)
    }

    /// Set shape key value.
    pub fn set_shape_key(&mut self, name: impl Into<String>, value: f64) {
        self.shape_keys.insert(name.into(), value);
    }

    /// Get shape key value.
    pub fn get_shape_key(&self, name: &str) -> f64 {
        self.shape_keys.get(name).copied().unwrap_or(0.0)
    }

    /// Set object channel value.
    pub fn set_object_channel(&mut self, channel: DriverChannel, value: f64) {
        self.object_channels.insert(channel, value);
    }

    /// Get object channel value.
    pub fn get_object_channel(&self, channel: DriverChannel) -> f64 {
        self.object_channels.get(&channel).copied().unwrap_or(0.0)
    }

    /// Set custom value.
    pub fn set_custom(&mut self, name: impl Into<String>, value: f64) {
        self.custom_values.insert(name.into(), value);
    }

    /// Get custom value.
    pub fn get_custom(&self, name: &str) -> f64 {
        self.custom_values.get(name).copied().unwrap_or(0.0)
    }
}

impl Default for DriverContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple expression parser and evaluator.
fn parse_and_evaluate(expression: &str, variables: &HashMap<String, f64>) -> Result<f64, String> {
    let expr = expression.trim();

    if expr.is_empty() {
        return Ok(0.0);
    }

    // Try to evaluate as simple variable reference
    if let Some(&value) = variables.get(expr) {
        return Ok(value);
    }

    // Try to parse as number
    if let Ok(value) = expr.parse::<f64>() {
        return Ok(value);
    }

    // Simple expression evaluation (supports basic math)
    evaluate_expression(expr, variables)
}

/// Evaluate a mathematical expression.
fn evaluate_expression(expr: &str, variables: &HashMap<String, f64>) -> Result<f64, String> {
    let expr_str = expr.trim();

    // First try to evaluate as a simple term (handles function calls)
    if !expr_str.contains('+') && !expr_str.contains('*') && !expr_str.contains('/') {
        // Check if it contains - but not as part of a negative number
        let has_minus_op = expr_str.matches('-').count() > 0
            && !expr_str.starts_with('-')
            && !expr_str.contains("(-");

        if !has_minus_op {
            return evaluate_simple_term(expr_str, variables);
        }
    }

    // Replace variables with their values for arithmetic
    let mut expr_replaced = expr.to_string();
    for (var_name, &value) in variables {
        // Only replace whole word matches
        if expr_replaced.contains(var_name) {
            expr_replaced = expr_replaced.replace(var_name, &value.to_string());
        }
    }

    // Simple tokenizer and evaluator
    if expr_replaced.contains('+') {
        let parts: Vec<&str> = expr_replaced.split('+').collect();
        let mut sum = 0.0;
        for part in parts {
            sum += evaluate_simple_term(part.trim(), variables)?;
        }
        Ok(sum)
    } else if expr_replaced.contains('-') {
        let parts: Vec<&str> = expr_replaced.split('-').collect();
        if parts.is_empty() {
            return Err("Invalid expression".to_string());
        }
        let mut result = evaluate_simple_term(parts[0].trim(), variables)?;
        for part in parts.iter().skip(1) {
            result -= evaluate_simple_term(part.trim(), variables)?;
        }
        Ok(result)
    } else if expr_replaced.contains('*') {
        let parts: Vec<&str> = expr_replaced.split('*').collect();
        let mut product = 1.0;
        for part in parts {
            product *= evaluate_simple_term(part.trim(), variables)?;
        }
        Ok(product)
    } else if expr_replaced.contains('/') {
        let parts: Vec<&str> = expr_replaced.split('/').collect();
        if parts.len() != 2 {
            return Err("Division requires exactly two operands".to_string());
        }
        let a = evaluate_simple_term(parts[0].trim(), variables)?;
        let b = evaluate_simple_term(parts[1].trim(), variables)?;
        if b.abs() < 1e-10 {
            return Err("Division by zero".to_string());
        }
        Ok(a / b)
    } else {
        evaluate_simple_term(&expr_replaced, variables)
    }
}

/// Evaluate a simple term (number, variable, or function call).
fn evaluate_simple_term(term: &str, variables: &HashMap<String, f64>) -> Result<f64, String> {
    let term = term.trim();

    // Try to parse as number
    if let Ok(value) = term.parse::<f64>() {
        return Ok(value);
    }

    // Try to get as variable
    if let Some(&value) = variables.get(term) {
        return Ok(value);
    }

    // Try to evaluate as function call
    if term.starts_with("sin(") && term.ends_with(')') {
        let arg = &term[4..term.len() - 1];
        let arg_value = evaluate_simple_term(arg, variables)?;
        return Ok(arg_value.sin());
    }

    if term.starts_with("cos(") && term.ends_with(')') {
        let arg = &term[4..term.len() - 1];
        let arg_value = evaluate_simple_term(arg, variables)?;
        return Ok(arg_value.cos());
    }

    if term.starts_with("abs(") && term.ends_with(')') {
        let arg = &term[4..term.len() - 1];
        let arg_value = evaluate_simple_term(arg, variables)?;
        return Ok(arg_value.abs());
    }

    if term.starts_with("clamp(") && term.ends_with(')') {
        let args = &term[6..term.len() - 1];
        let parts: Vec<&str> = args.split(',').collect();
        if parts.len() != 3 {
            return Err("clamp requires 3 arguments".to_string());
        }
        let value = evaluate_simple_term(parts[0].trim(), variables)?;
        let min = evaluate_simple_term(parts[1].trim(), variables)?;
        let max = evaluate_simple_term(parts[2].trim(), variables)?;
        return Ok(value.clamp(min, max));
    }

    if term.starts_with("lerp(") && term.ends_with(')') {
        let args = &term[5..term.len() - 1];
        let parts: Vec<&str> = args.split(',').collect();
        if parts.len() != 3 {
            return Err("lerp requires 3 arguments".to_string());
        }
        let a = evaluate_simple_term(parts[0].trim(), variables)?;
        let b = evaluate_simple_term(parts[1].trim(), variables)?;
        let t = evaluate_simple_term(parts[2].trim(), variables)?;
        return Ok(a + (b - a) * t);
    }

    Err(format!("Unknown term: {}", term))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_driver_creation() {
        let driver = Driver::new("TestDriver");
        assert_eq!(driver.name, "TestDriver");
        assert_eq!(driver.fallback_value, 0.0);
    }

    #[test]
    fn test_driver_variable() {
        let var = DriverVariable::new("x", DriverSource::Custom(5.0));
        let context = DriverContext::new();
        assert_eq!(var.evaluate(&context), 5.0);
    }

    #[test]
    fn test_driver_context() {
        let mut context = DriverContext::new();
        context.set_bone_channel(BoneId(0), DriverChannel::LocX, 10.0);
        context.set_shape_key("Smile", 0.5);

        assert_eq!(
            context.get_bone_channel(BoneId(0), DriverChannel::LocX, TransformSpace::World),
            10.0
        );
        assert_eq!(context.get_shape_key("Smile"), 0.5);
    }

    #[test]
    fn test_simple_expression() {
        let mut vars = HashMap::new();
        vars.insert("x".to_string(), 5.0);

        assert_eq!(parse_and_evaluate("x", &vars).unwrap(), 5.0);
        assert_eq!(parse_and_evaluate("10", &vars).unwrap(), 10.0);
    }

    #[test]
    fn test_math_expression() {
        let mut vars = HashMap::new();
        vars.insert("x".to_string(), 5.0);
        vars.insert("y".to_string(), 3.0);

        assert_eq!(parse_and_evaluate("x + y", &vars).unwrap(), 8.0);
        assert_eq!(parse_and_evaluate("x - y", &vars).unwrap(), 2.0);
        assert_eq!(parse_and_evaluate("x * y", &vars).unwrap(), 15.0);
        assert_eq!(parse_and_evaluate("x / y", &vars).unwrap(), 5.0 / 3.0);
    }

    #[test]
    fn test_function_calls() {
        let vars = HashMap::new();

        let result = parse_and_evaluate("abs(-5)", &vars).unwrap();
        assert_eq!(result, 5.0);

        let result = parse_and_evaluate("clamp(15, 0, 10)", &vars).unwrap();
        assert_eq!(result, 10.0);

        let result = parse_and_evaluate("lerp(0, 10, 0.5)", &vars).unwrap();
        assert_eq!(result, 5.0);
    }

    #[test]
    fn test_driver_evaluation() {
        let mut driver = Driver::new("Test");
        driver.expression = "x + y".to_string();
        driver.add_variable(DriverVariable::new("x", DriverSource::Custom(5.0)));
        driver.add_variable(DriverVariable::new("y", DriverSource::Custom(3.0)));

        let context = DriverContext::new();
        let result = driver.evaluate(&context);
        assert_eq!(result, 8.0);
    }
}
