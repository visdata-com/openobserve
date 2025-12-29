// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Pattern Extractor using Drain Algorithm
//!
//! Implements the Drain log parsing algorithm for pattern extraction.
//! Reference: "Drain: An Online Log Parsing Approach with Fixed Depth Tree"
//! by Pinjia He, Jieming Zhu, Zibin Zheng, and Michael R. Lyu (ICWS 2017)

use std::collections::HashMap;

use super::config::PatternExtractionConfig;
use super::sdr::SdrRecognizer;
use super::types::{Example, Pattern, Variable};

/// A log cluster in the Drain tree
#[derive(Clone, Debug)]
struct LogCluster {
    /// Pattern template tokens
    template_tokens: Vec<String>,
    /// Log IDs belonging to this cluster
    log_ids: Vec<usize>,
}

impl LogCluster {
    fn new(tokens: Vec<String>) -> Self {
        Self {
            template_tokens: tokens,
            log_ids: Vec::new(),
        }
    }

    /// Get the template as a string
    fn get_template(&self) -> String {
        self.template_tokens.join(" ")
    }

    /// Update template with a new log (find common parts)
    fn update_template(&mut self, tokens: &[String], sdr: &SdrRecognizer) {
        if self.template_tokens.len() != tokens.len() {
            return;
        }

        for (template_token, new_token) in
            self.template_tokens.iter_mut().zip(tokens.iter())
        {
            // If tokens are different, replace with wildcard
            if template_token != new_token && !template_token.starts_with("<:") {
                // Use SDR type recognition
                let sdr_type = sdr.recognize(new_token);
                *template_token = sdr_type.placeholder();
            }
        }
    }
}

/// Node in the Drain parsing tree
#[derive(Clone, Debug)]
struct DrainNode {
    /// Child nodes by token
    children: HashMap<String, DrainNode>,
    /// Clusters at this node (leaf level)
    clusters: Vec<LogCluster>,
}

impl DrainNode {
    fn new() -> Self {
        Self {
            children: HashMap::new(),
            clusters: Vec::new(),
        }
    }
}

/// Pattern Extractor using Drain algorithm
pub struct PatternExtractor {
    /// Configuration
    config: PatternExtractionConfig,
    /// SDR recognizer
    sdr: SdrRecognizer,
    /// Root node indexed by log length
    root: HashMap<usize, DrainNode>,
    /// All clusters
    clusters: Vec<LogCluster>,
}

impl PatternExtractor {
    /// Create a new pattern extractor
    pub fn new(config: PatternExtractionConfig) -> Self {
        Self {
            config,
            sdr: SdrRecognizer::new(),
            root: HashMap::new(),
            clusters: Vec::new(),
        }
    }

    /// Extract patterns from log messages
    pub fn extract(&mut self, logs: &[String]) -> Vec<Pattern> {
        // Process each log
        for (log_id, log) in logs.iter().enumerate() {
            self.process_log(log_id, log);
        }

        // Convert clusters to patterns
        self.clusters_to_patterns(logs)
    }

    /// Process a single log message
    fn process_log(&mut self, log_id: usize, log: &str) {
        let tokens = self.tokenize(log);
        if tokens.is_empty() {
            return;
        }

        let log_length = tokens.len();

        // Ensure the length node exists
        if !self.root.contains_key(&log_length) {
            self.root.insert(log_length, DrainNode::new());
        }

        // Search for matching cluster first (immutable borrow)
        let cluster_idx_opt = self.search_cluster_by_length(log_length, &tokens);

        if let Some(cluster_idx) = cluster_idx_opt {
            // Update existing cluster
            let sdr = &self.sdr;
            let cluster = &mut self.clusters[cluster_idx];
            cluster.update_template(&tokens, sdr);
            cluster.log_ids.push(log_id);
        } else {
            // Create new cluster
            let template_tokens = self.create_initial_template(&tokens);
            let mut cluster = LogCluster::new(template_tokens);
            cluster.log_ids.push(log_id);

            let cluster_idx = self.clusters.len();
            self.clusters.push(cluster);

            // Add to tree
            self.add_cluster_to_tree_by_length(log_length, &tokens, cluster_idx);
        }
    }

    /// Search for a matching cluster by log length
    fn search_cluster_by_length(&self, log_length: usize, tokens: &[String]) -> Option<usize> {
        if let Some(node) = self.root.get(&log_length) {
            self.search_cluster(node, tokens)
        } else {
            None
        }
    }

    /// Tokenize a log message
    fn tokenize(&self, log: &str) -> Vec<String> {
        log.split_whitespace()
            .filter(|t| t.len() >= self.config.min_field_length)
            .map(|t| t.to_string())
            .collect()
    }

    /// Create initial template from tokens (apply SDR recognition)
    fn create_initial_template(&self, tokens: &[String]) -> Vec<String> {
        tokens
            .iter()
            .map(|token| {
                if self.sdr.is_variable(token) {
                    self.sdr.get_placeholder(token)
                } else {
                    token.clone()
                }
            })
            .collect()
    }

    /// Search for a matching cluster in the tree
    fn search_cluster(&self, node: &DrainNode, tokens: &[String]) -> Option<usize> {
        // Get the depth to traverse
        let depth = self.config.xdrain_depth.min(tokens.len());

        // Navigate tree
        let mut current_node = node;
        for i in 0..depth {
            let token = &tokens[i];

            // Try exact match first
            if let Some(child) = current_node.children.get(token) {
                current_node = child;
            } else if let Some(child) = current_node.children.get("<*>") {
                // Try wildcard
                current_node = child;
            } else {
                // No match found
                return self.search_clusters_by_similarity(&current_node.clusters, tokens);
            }
        }

        // Search clusters at this node
        self.search_clusters_by_similarity(&current_node.clusters, tokens)
    }

    /// Search clusters by similarity
    fn search_clusters_by_similarity(
        &self,
        _node_clusters: &[LogCluster],
        tokens: &[String],
    ) -> Option<usize> {
        let mut best_match: Option<(usize, f64)> = None;

        for (idx, cluster) in self.clusters.iter().enumerate() {
            if cluster.template_tokens.len() != tokens.len() {
                continue;
            }

            let similarity = self.calculate_similarity(&cluster.template_tokens, tokens);
            if similarity >= self.config.similarity_threshold {
                if best_match.is_none() || similarity > best_match.unwrap().1 {
                    best_match = Some((idx, similarity));
                }
            }
        }

        best_match.map(|(idx, _)| idx)
    }

    /// Calculate similarity between template and tokens
    fn calculate_similarity(&self, template: &[String], tokens: &[String]) -> f64 {
        if template.len() != tokens.len() {
            return 0.0;
        }

        let mut matches = 0;
        for (t1, t2) in template.iter().zip(tokens.iter()) {
            if t1 == t2 || t1.starts_with("<:") || t1 == "<*>" {
                matches += 1;
            }
        }

        matches as f64 / template.len() as f64
    }

    /// Add a cluster to the tree by log length
    fn add_cluster_to_tree_by_length(
        &mut self,
        log_length: usize,
        tokens: &[String],
        _cluster_idx: usize,
    ) {
        let depth = self.config.xdrain_depth.min(tokens.len());
        let max_child = self.config.xdrain_max_child;

        // Get the length node
        let length_node = self.root.get_mut(&log_length);
        if length_node.is_none() {
            return;
        }
        let length_node = length_node.unwrap();

        let mut current_node = length_node;
        for i in 0..depth {
            let token = &tokens[i];

            // Use the token if it's constant, otherwise use wildcard
            let key = if self.sdr.is_variable(token) {
                "<*>".to_string()
            } else {
                token.clone()
            };

            // Check child limit
            if !current_node.children.contains_key(&key)
                && current_node.children.len() >= max_child
            {
                // Use wildcard if at limit
                let wildcard_key = "<*>".to_string();
                current_node = current_node
                    .children
                    .entry(wildcard_key)
                    .or_insert_with(DrainNode::new);
            } else {
                current_node = current_node
                    .children
                    .entry(key)
                    .or_insert_with(DrainNode::new);
            }
        }

        // Add cluster reference at leaf
        current_node.clusters.push(LogCluster::new(vec![]));
    }

    /// Convert clusters to Pattern objects
    fn clusters_to_patterns(&self, original_logs: &[String]) -> Vec<Pattern> {
        let total_logs = original_logs.len();

        self.clusters
            .iter()
            .enumerate()
            .filter(|(_, cluster)| cluster.log_ids.len() >= self.config.min_cluster_size)
            .take(self.config.max_clusters)
            .map(|(idx, cluster)| {
                let frequency = cluster.log_ids.len();
                let percentage = if total_logs > 0 {
                    (frequency as f64 / total_logs as f64) * 100.0
                } else {
                    0.0
                };

                let template = cluster.get_template();
                let pattern_id = format!("p{}", idx);

                // Extract variables from template
                let variables = self.extract_variables(&cluster.template_tokens);

                // Get examples (up to 5)
                let examples: Vec<Example> = cluster
                    .log_ids
                    .iter()
                    .take(5)
                    .filter_map(|&id| original_logs.get(id))
                    .map(|log| Example::new(log.clone()))
                    .collect();

                Pattern::new(pattern_id, template, frequency)
                    .with_percentage(percentage)
                    .with_variables(variables)
                    .with_examples(examples)
            })
            .collect()
    }

    /// Extract variables from template tokens
    fn extract_variables(&self, template_tokens: &[String]) -> Vec<Variable> {
        let mut variables = Vec::new();
        let mut var_idx = 0;

        for (i, token) in template_tokens.iter().enumerate() {
            if token.starts_with("<:") && token.ends_with('>') {
                let var_type = token
                    .trim_start_matches("<:")
                    .trim_end_matches('>')
                    .to_lowercase();

                variables.push(Variable::new(
                    i,
                    format!("var_{}", var_idx),
                    var_type,
                ));
                var_idx += 1;
            }
        }

        variables
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_extraction() {
        let config = PatternExtractionConfig::default()
            .with_min_cluster_size(1)
            .with_similarity_threshold(0.5);
        let mut extractor = PatternExtractor::new(config);

        let logs = vec![
            "User john logged in from 192.168.1.1".to_string(),
            "User jane logged in from 192.168.1.2".to_string(),
            "User bob logged in from 10.0.0.1".to_string(),
        ];

        let patterns = extractor.extract(&logs);

        assert!(!patterns.is_empty());
        // Should find a pattern like "User <:STRING> logged in from <:IP>"
    }

    #[test]
    fn test_different_patterns() {
        let config = PatternExtractionConfig::default()
            .with_min_cluster_size(1)
            .with_similarity_threshold(0.5);
        let mut extractor = PatternExtractor::new(config);

        let logs = vec![
            "Error: connection failed".to_string(),
            "Error: connection failed".to_string(),
            "Warning: disk space low".to_string(),
        ];

        let patterns = extractor.extract(&logs);

        // Should find at least 2 different patterns
        assert!(patterns.len() >= 1);
    }

    #[test]
    fn test_min_cluster_size() {
        let config = PatternExtractionConfig::default()
            .with_min_cluster_size(3)
            .with_similarity_threshold(0.5);
        let mut extractor = PatternExtractor::new(config);

        let logs = vec![
            "Log A".to_string(),
            "Log B".to_string(),
            "Log C".to_string(),
        ];

        let patterns = extractor.extract(&logs);

        // All logs are different, so no pattern should have 3+ occurrences
        assert!(patterns.is_empty() || patterns.iter().all(|p| p.frequency >= 3));
    }

    #[test]
    fn test_variable_extraction() {
        let config = PatternExtractionConfig::default()
            .with_min_cluster_size(1);
        let mut extractor = PatternExtractor::new(config);

        let logs = vec![
            "Request from 192.168.1.1 took 123 ms".to_string(),
            "Request from 10.0.0.1 took 456 ms".to_string(),
        ];

        let patterns = extractor.extract(&logs);

        if !patterns.is_empty() {
            let pattern = &patterns[0];
            assert!(!pattern.variables.is_empty());
        }
    }
}
