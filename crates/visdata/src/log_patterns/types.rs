// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Type definitions for log pattern extraction
//!
//! These types are compatible with o2_enterprise::log_patterns output format
//! and the frontend Pattern components.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Response from pattern extraction
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PatternExtractionResponse {
    /// Extracted patterns
    pub patterns: Vec<Pattern>,
    /// Extraction statistics
    pub statistics: Statistics,
}

/// A single extracted log pattern
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pattern {
    /// Unique pattern identifier
    pub pattern_id: String,

    /// Pattern template with SDR type placeholders
    /// e.g., "User <:STRING> logged in from IP <:IP>"
    pub template: String,

    /// Human-readable description with simple wildcards
    /// e.g., "User * logged in from IP *"
    pub description: String,

    /// Number of logs matching this pattern
    pub frequency: usize,

    /// Percentage of total logs matching this pattern
    pub percentage: f64,

    /// Whether this pattern is considered an anomaly
    #[serde(default)]
    pub is_anomaly: bool,

    /// Variables extracted from the pattern
    pub variables: Vec<Variable>,

    /// Example log messages matching this pattern
    pub examples: Vec<Example>,
}

impl Pattern {
    /// Create a new pattern with the given template
    pub fn new(pattern_id: String, template: String, frequency: usize) -> Self {
        let description = template
            .replace("<:IP>", "*")
            .replace("<:NUM>", "*")
            .replace("<:UUID>", "*")
            .replace("<:TIMESTAMP>", "*")
            .replace("<:PATH>", "*")
            .replace("<:URL>", "*")
            .replace("<:EMAIL>", "*")
            .replace("<:HEX>", "*")
            .replace("<:STRING>", "*")
            .replace("<*>", "*");

        Self {
            pattern_id,
            template,
            description,
            frequency,
            percentage: 0.0,
            is_anomaly: false,
            variables: Vec::new(),
            examples: Vec::new(),
        }
    }

    /// Set the percentage
    pub fn with_percentage(mut self, percentage: f64) -> Self {
        self.percentage = percentage;
        self
    }

    /// Add variables
    pub fn with_variables(mut self, variables: Vec<Variable>) -> Self {
        self.variables = variables;
        self
    }

    /// Add examples
    pub fn with_examples(mut self, examples: Vec<Example>) -> Self {
        self.examples = examples;
        self
    }

    /// Mark as anomaly
    pub fn mark_anomaly(mut self) -> Self {
        self.is_anomaly = true;
        self
    }
}

/// A variable in a pattern
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Variable {
    /// Index of the variable in the template
    pub index: usize,

    /// Name/identifier of the variable
    pub name: String,

    /// Type of the variable (e.g., "ip", "num", "string")
    pub var_type: String,
}

impl Variable {
    /// Create a new variable
    pub fn new(index: usize, name: String, var_type: String) -> Self {
        Self {
            index,
            name,
            var_type,
        }
    }
}

/// An example log message matching a pattern
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Example {
    /// The original log message
    pub log_message: String,

    /// Extracted variable values
    #[serde(default)]
    pub variables: HashMap<String, String>,
}

impl Example {
    /// Create a new example
    pub fn new(log_message: String) -> Self {
        Self {
            log_message,
            variables: HashMap::new(),
        }
    }

    /// Add extracted variables
    pub fn with_variables(mut self, variables: HashMap<String, String>) -> Self {
        self.variables = variables;
        self
    }
}

/// Statistics from pattern extraction
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Statistics {
    /// Total number of logs analyzed
    pub total_logs_analyzed: usize,

    /// Number of patterns found
    pub total_patterns_found: usize,

    /// Percentage of logs covered by patterns
    pub coverage_percentage: f64,

    /// Time taken for extraction in milliseconds
    pub extraction_time_ms: u64,
}

impl Default for Statistics {
    fn default() -> Self {
        Self {
            total_logs_analyzed: 0,
            total_patterns_found: 0,
            coverage_percentage: 0.0,
            extraction_time_ms: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_description_generation() {
        let pattern = Pattern::new(
            "p1".to_string(),
            "User <:STRING> logged in from <:IP>".to_string(),
            100,
        );

        assert_eq!(pattern.description, "User * logged in from *");
    }

    #[test]
    fn test_pattern_builder() {
        let pattern = Pattern::new("p1".to_string(), "Error <:NUM>".to_string(), 50)
            .with_percentage(25.0)
            .with_variables(vec![Variable::new(0, "error_code".to_string(), "num".to_string())])
            .with_examples(vec![Example::new("Error 500".to_string())]);

        assert_eq!(pattern.percentage, 25.0);
        assert_eq!(pattern.variables.len(), 1);
        assert_eq!(pattern.examples.len(), 1);
    }
}
