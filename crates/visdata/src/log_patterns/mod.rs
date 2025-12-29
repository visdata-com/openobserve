// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Log Patterns Module for VisData
//!
//! This module provides log pattern extraction functionality compatible with
//! o2_enterprise::log_patterns API. It uses the Drain algorithm for log clustering
//! and adds SDR (Structured Data Recognition) type identification.
//!
//! ## Features
//! - Pattern extraction from log streams using Drain algorithm
//! - SDR type recognition (IP, NUM, UUID, TIMESTAMP, etc.)
//! - Compatible API with o2_enterprise::log_patterns
//!
//! ## Usage
//! ```rust,ignore
//! use visdata::log_patterns::{PatternAccumulator, PatternExtractionConfig, extract_patterns_from_stream};
//!
//! let config = PatternExtractionConfig::default();
//! let mut accumulator = PatternAccumulator::new(config.clone());
//! accumulator.add_hits(&search_hits);
//!
//! let result = extract_patterns_from_stream(accumulator, "stream_name".to_string(), config, 1000).await?;
//! ```

mod accumulator;
mod config;
mod extractor;
mod sdr;
mod types;

// Re-export public API (compatible with o2_enterprise::log_patterns)
pub use accumulator::{AccumulatorStats, PatternAccumulator};
pub use config::PatternExtractionConfig;
pub use extractor::PatternExtractor;
pub use sdr::{SdrRecognizer, SdrType};
pub use types::{Example, Pattern, PatternExtractionResponse, Statistics, Variable};

use std::time::Instant;

/// Extract patterns from accumulated logs (query-time extraction)
///
/// This is the main entry point for pattern extraction during search queries.
/// It processes the accumulated logs using the Drain algorithm and SDR recognition.
///
/// # Arguments
/// * `accumulator` - PatternAccumulator containing collected log messages
/// * `_stream_name` - Name of the stream (for metadata)
/// * `config` - Pattern extraction configuration
/// * `total_logs_seen` - Total number of logs seen (for coverage calculation)
///
/// # Returns
/// `PatternExtractionResponse` containing extracted patterns and statistics
pub async fn extract_patterns_from_stream(
    accumulator: PatternAccumulator,
    _stream_name: String,
    config: PatternExtractionConfig,
    total_logs_seen: usize,
) -> Result<PatternExtractionResponse, anyhow::Error> {
    let start = Instant::now();

    // Create extractor and extract patterns
    let mut extractor = PatternExtractor::new(config);
    let logs = accumulator.get_logs();
    let patterns = extractor.extract(&logs);

    let extraction_time = start.elapsed();

    // Calculate coverage
    let total_matched: usize = patterns.iter().map(|p| p.frequency).sum();
    let coverage_percentage = if total_logs_seen > 0 {
        (total_matched as f64 / total_logs_seen as f64) * 100.0
    } else {
        0.0
    };

    let patterns_count = patterns.len();
    let logs_count = logs.len();

    Ok(PatternExtractionResponse {
        patterns,
        statistics: Statistics {
            total_logs_analyzed: logs_count,
            total_patterns_found: patterns_count,
            coverage_percentage,
            extraction_time_ms: extraction_time.as_millis() as u64,
        },
    })
}

/// Extract patterns from raw logs (compaction-time extraction)
///
/// This function is reserved for Compaction-time pattern persistence scenarios.
/// Currently returns empty results; can be implemented later as needed.
///
/// # Arguments
/// * `_logs` - Vector of (timestamp, log_data) tuples
/// * `_fts_fields` - Full-text search fields to extract from
///
/// # Returns
/// Empty vector (placeholder for future implementation)
pub fn extract_patterns_from_logs(
    _logs: &[(i64, serde_json::Map<String, serde_json::Value>)],
    _fts_fields: &[String],
) -> Result<Vec<serde_json::Map<String, serde_json::Value>>, anyhow::Error> {
    // TODO: Implement compaction-time pattern extraction
    // Currently returns empty results without affecting system operation
    Ok(Vec::new())
}
