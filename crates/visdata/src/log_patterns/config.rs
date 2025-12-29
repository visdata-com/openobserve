// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Pattern Extraction Configuration
//!
//! Configuration structure compatible with o2_enterprise::log_patterns::PatternExtractionConfig

use serde::{Deserialize, Serialize};

/// Configuration for pattern extraction
///
/// This structure mirrors o2_enterprise::log_patterns::PatternExtractionConfig
/// for API compatibility.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PatternExtractionConfig {
    /// Maximum number of logs to analyze for pattern extraction
    /// Default: 10000 (aligned with industry standards for pattern quality)
    pub max_logs_for_extraction: usize,

    /// Minimum cluster size for a pattern to be considered valid
    /// Default: 2
    pub min_cluster_size: usize,

    /// Similarity threshold for grouping logs (0.0-1.0)
    /// Default: 0.6
    pub similarity_threshold: f64,

    /// Drain algorithm tree depth
    /// Default: 4
    pub xdrain_depth: usize,

    /// Maximum child nodes per tree node
    /// Default: 100
    pub xdrain_max_child: usize,

    /// Maximum number of clusters/patterns to extract
    /// Default: 1000
    pub max_clusters: usize,

    /// Minimum field length for pattern extraction
    /// Default: 1
    pub min_field_length: usize,

    /// Full-text search fields to extract log messages from
    pub fts_fields: Vec<String>,
}

impl Default for PatternExtractionConfig {
    fn default() -> Self {
        Self {
            max_logs_for_extraction: 10000,
            min_cluster_size: 2,
            similarity_threshold: 0.6,
            xdrain_depth: 4,
            xdrain_max_child: 100,
            max_clusters: 1000,
            min_field_length: 1,
            fts_fields: Vec::new(),
        }
    }
}

impl PatternExtractionConfig {
    /// Create a new config with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum logs for extraction
    pub fn with_max_logs(mut self, max_logs: usize) -> Self {
        self.max_logs_for_extraction = max_logs;
        self
    }

    /// Set the minimum cluster size
    pub fn with_min_cluster_size(mut self, size: usize) -> Self {
        self.min_cluster_size = size;
        self
    }

    /// Set the similarity threshold
    pub fn with_similarity_threshold(mut self, threshold: f64) -> Self {
        self.similarity_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Set the drain depth
    pub fn with_drain_depth(mut self, depth: usize) -> Self {
        self.xdrain_depth = depth;
        self
    }

    /// Set the max child nodes
    pub fn with_max_child(mut self, max_child: usize) -> Self {
        self.xdrain_max_child = max_child;
        self
    }

    /// Set the FTS fields
    pub fn with_fts_fields(mut self, fields: Vec<String>) -> Self {
        self.fts_fields = fields;
        self
    }
}
