// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Pattern Accumulator
//!
//! Collects log messages from search results for pattern extraction.
//! Compatible with o2_enterprise::log_patterns::PatternAccumulator API.

use super::config::PatternExtractionConfig;
use serde::{Deserialize, Serialize};

/// Statistics from the accumulator
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccumulatorStats {
    /// Number of logs actually accumulated
    pub accumulated_logs: usize,
    /// Total logs seen by the accumulator
    pub total_logs_seen: usize,
    /// Whether sampling was applied
    pub was_sampled: bool,
}

/// Accumulator for collecting log messages for pattern extraction
///
/// This collects log messages from search hits and prepares them for
/// pattern extraction using the Drain algorithm.
pub struct PatternAccumulator {
    /// Configuration
    config: PatternExtractionConfig,
    /// Accumulated log messages
    logs: Vec<String>,
    /// Original log objects (for example extraction)
    log_objects: Vec<serde_json::Value>,
    /// Total logs seen
    total_seen: usize,
    /// Whether we've hit the max limit
    at_capacity: bool,
}

impl PatternAccumulator {
    /// Create a new accumulator with the given configuration
    pub fn new(config: PatternExtractionConfig) -> Self {
        Self {
            config,
            logs: Vec::new(),
            log_objects: Vec::new(),
            total_seen: 0,
            at_capacity: false,
        }
    }

    /// Add search hits to the accumulator
    ///
    /// Extracts log messages from the hits using configured FTS fields.
    /// Stops accumulating when max_logs_for_extraction is reached.
    pub fn add_hits(&mut self, hits: &[serde_json::Value]) {
        for hit in hits {
            self.total_seen += 1;

            // Skip if already at capacity
            if self.at_capacity {
                continue;
            }

            // Try to extract log message from the hit
            if let Some(log_msg) = self.extract_log_message(hit) {
                if !log_msg.is_empty() {
                    self.logs.push(log_msg);
                    self.log_objects.push(hit.clone());

                    // Check capacity
                    if self.logs.len() >= self.config.max_logs_for_extraction {
                        self.at_capacity = true;
                    }
                }
            }
        }
    }

    /// Extract log message from a hit object
    ///
    /// Tries FTS fields first, then falls back to common field names.
    fn extract_log_message(&self, hit: &serde_json::Value) -> Option<String> {
        let obj = hit.as_object()?;

        // Try configured FTS fields first
        for field in &self.config.fts_fields {
            if let Some(value) = obj.get(field) {
                if let Some(s) = value.as_str() {
                    return Some(s.to_string());
                }
            }
        }

        // Try common log message field names
        let common_fields = [
            "message",
            "msg",
            "log",
            "body",
            "_source",
            "log_message",
            "content",
            "text",
            "_raw",
        ];

        for field in common_fields {
            if let Some(value) = obj.get(field) {
                if let Some(s) = value.as_str() {
                    return Some(s.to_string());
                }
            }
        }

        // Try to get any string field that looks like a log message
        for (key, value) in obj {
            // Skip metadata fields
            if key.starts_with('_') && key != "_source" && key != "_raw" {
                continue;
            }
            if let Some(s) = value.as_str() {
                // Skip short strings that are unlikely to be log messages
                if s.len() > 20 {
                    return Some(s.to_string());
                }
            }
        }

        // Last resort: stringify the entire object
        Some(hit.to_string())
    }

    /// Get accumulator statistics
    pub fn stats(&self) -> AccumulatorStats {
        AccumulatorStats {
            accumulated_logs: self.logs.len(),
            total_logs_seen: self.total_seen,
            was_sampled: self.at_capacity,
        }
    }

    /// Get the accumulated log messages
    pub fn get_logs(&self) -> &[String] {
        &self.logs
    }

    /// Get the accumulated log objects (for example extraction)
    pub fn get_log_objects(&self) -> &[serde_json::Value] {
        &self.log_objects
    }

    /// Get the number of accumulated logs
    pub fn len(&self) -> usize {
        self.logs.len()
    }

    /// Check if the accumulator is empty
    pub fn is_empty(&self) -> bool {
        self.logs.is_empty()
    }

    /// Check if the accumulator is at capacity
    pub fn is_at_capacity(&self) -> bool {
        self.at_capacity
    }

    /// Consume the accumulator and return the logs
    pub fn into_logs(self) -> Vec<String> {
        self.logs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_accumulator_basic() {
        let config = PatternExtractionConfig::default().with_max_logs(100);
        let mut acc = PatternAccumulator::new(config);

        let hits = vec![
            json!({"message": "User john logged in"}),
            json!({"message": "User jane logged out"}),
        ];

        acc.add_hits(&hits);

        assert_eq!(acc.len(), 2);
        assert!(!acc.is_at_capacity());

        let stats = acc.stats();
        assert_eq!(stats.accumulated_logs, 2);
        assert_eq!(stats.total_logs_seen, 2);
    }

    #[test]
    fn test_accumulator_capacity() {
        let config = PatternExtractionConfig::default().with_max_logs(2);
        let mut acc = PatternAccumulator::new(config);

        let hits = vec![
            json!({"message": "Log 1"}),
            json!({"message": "Log 2"}),
            json!({"message": "Log 3"}),
            json!({"message": "Log 4"}),
        ];

        acc.add_hits(&hits);

        assert_eq!(acc.len(), 2);
        assert!(acc.is_at_capacity());

        let stats = acc.stats();
        assert_eq!(stats.accumulated_logs, 2);
        assert_eq!(stats.total_logs_seen, 4);
        assert!(stats.was_sampled);
    }

    #[test]
    fn test_fts_field_extraction() {
        let config = PatternExtractionConfig::default()
            .with_fts_fields(vec!["body".to_string()]);
        let mut acc = PatternAccumulator::new(config);

        let hits = vec![json!({
            "message": "Should be ignored",
            "body": "This is the log body"
        })];

        acc.add_hits(&hits);

        let logs = acc.get_logs();
        assert_eq!(logs[0], "This is the log body");
    }

    #[test]
    fn test_common_field_fallback() {
        let config = PatternExtractionConfig::default();
        let mut acc = PatternAccumulator::new(config);

        let hits = vec![
            json!({"msg": "Using msg field"}),
            json!({"log": "Using log field"}),
        ];

        acc.add_hits(&hits);

        assert_eq!(acc.len(), 2);
        assert_eq!(acc.get_logs()[0], "Using msg field");
        assert_eq!(acc.get_logs()[1], "Using log field");
    }
}
