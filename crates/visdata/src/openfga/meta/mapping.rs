// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Resource type mappings (compatible with o2_openfga::meta::mapping)
//!
//! This module provides the OFGA_MODELS mapping and related types
//! for OpenFGA resource management.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

/// Resource definition for OpenFGA model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    /// Resource key (e.g., "logs", "dashboard")
    pub key: String,
    /// Display name (e.g., "Logs", "Dashboards") - matches frontend expectation
    pub display_name: String,
    /// Parent resource type (e.g., "stream" for logs)
    pub parent: Option<String>,
    /// Display order
    pub order: i32,
    /// Whether this resource is visible in UI
    pub visible: bool,
    /// Whether this is a top-level resource
    pub top_level: bool,
    /// Whether this resource can have individual entities
    pub has_entities: bool,
}

impl Resource {
    fn new(
        key: &str,
        display_name: &str,
        parent: Option<&str>,
        order: i32,
        visible: bool,
        has_entities: bool,
    ) -> Self {
        Self {
            key: key.to_string(),
            display_name: display_name.to_string(),
            parent: parent.map(|s| s.to_string()),
            order,
            visible,
            top_level: parent.is_none(),
            has_entities,
        }
    }
}

/// OpenFGA resource type models (compatible with o2_openfga::meta::mapping::OFGA_MODELS)
///
/// This maps resource type keys to their metadata, based on store.yaml definitions.
pub static OFGA_MODELS: LazyLock<HashMap<&'static str, Resource>> = LazyLock::new(|| {
    let mut m = HashMap::new();

    // Core types
    m.insert("user", Resource::new("user", "Users", None, 1, true, true));
    m.insert("group", Resource::new("group", "Groups", None, 2, true, true));
    m.insert("role", Resource::new("role", "Roles", None, 3, true, true));
    m.insert("org", Resource::new("org", "Organizations", None, 4, true, false));

    // Stream hierarchy (data streams)
    m.insert("stream", Resource::new("stream", "Streams", None, 10, false, true));
    m.insert("logs", Resource::new("logs", "Logs", Some("stream"), 11, true, true));
    m.insert("metrics", Resource::new("metrics", "Metrics", Some("stream"), 12, true, true));
    m.insert("traces", Resource::new("traces", "Traces", Some("stream"), 13, true, true));
    m.insert("metadata", Resource::new("metadata", "Metadata", Some("stream"), 14, false, true));
    m.insert("index", Resource::new("index", "Index", Some("stream"), 15, true, true));

    // Dashboard hierarchy
    m.insert("dfolder", Resource::new("dfolder", "Dashboard Folders", None, 20, true, true));
    m.insert("dashboard", Resource::new("dashboard", "Dashboards", Some("dfolder"), 21, true, true));
    m.insert("template", Resource::new("template", "Templates", None, 22, true, true));
    m.insert("savedviews", Resource::new("savedviews", "Saved Views", None, 23, true, true));

    // Alert hierarchy
    m.insert("afolder", Resource::new("afolder", "Alert Folders", None, 30, true, true));
    m.insert("alert", Resource::new("alert", "Alerts", Some("afolder"), 31, true, true));
    m.insert("destination", Resource::new("destination", "Destinations", None, 32, true, true));

    // Report hierarchy
    m.insert("rfolder", Resource::new("rfolder", "Report Folders", None, 40, true, true));
    m.insert("report", Resource::new("report", "Reports", Some("rfolder"), 41, true, true));

    // Functions and pipelines
    m.insert("function", Resource::new("function", "Functions", None, 50, true, true));
    m.insert("pipeline", Resource::new("pipeline", "Pipelines", None, 51, true, true));

    // System resources
    m.insert("settings", Resource::new("settings", "Settings", None, 60, true, false));
    m.insert("kv", Resource::new("kv", "KV Store", None, 61, true, true));
    m.insert("enrichment_table", Resource::new("enrichment_table", "Enrichment Tables", None, 62, true, true));
    m.insert("summary", Resource::new("summary", "Summary", None, 63, true, true));
    m.insert("syslog-route", Resource::new("syslog-route", "Syslog Routes", None, 64, true, true));

    // Security and authentication
    m.insert("passcode", Resource::new("passcode", "Passcodes", None, 70, true, true));
    m.insert("rumtoken", Resource::new("rumtoken", "RUM Tokens", None, 71, true, true));
    m.insert("service_accounts", Resource::new("service_accounts", "Service Accounts", None, 72, true, true));
    m.insert("cipher_keys", Resource::new("cipher_keys", "Cipher Keys", None, 73, true, true));

    // Other resources
    m.insert("search_jobs", Resource::new("search_jobs", "Search Jobs", None, 80, true, true));
    m.insert("action_scripts", Resource::new("action_scripts", "Action Scripts", None, 81, true, true));
    m.insert("ratelimit", Resource::new("ratelimit", "Rate Limits", None, 82, true, true));
    m.insert("ai", Resource::new("ai", "AI", None, 83, true, false));
    m.insert("re_patterns", Resource::new("re_patterns", "Regex Patterns", None, 84, true, true));
    m.insert("license", Resource::new("license", "License", None, 90, true, false));

    m
});

/// Resource keys that are not available in cloud deployments
/// (compatible with o2_openfga::meta::mapping::NON_CLOUD_RESOURCE_KEYS)
pub static NON_CLOUD_RESOURCE_KEYS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    let mut s = HashSet::new();
    s.insert("license");
    s.insert("cipher_keys");
    s
});

/// Get resource by key
pub fn get_resource(key: &str) -> Option<&'static Resource> {
    OFGA_MODELS.get(key)
}

/// Check if a resource type exists
pub fn is_valid_resource_type(key: &str) -> bool {
    OFGA_MODELS.contains_key(key)
}

/// Get all visible resource types
pub fn get_visible_resources() -> Vec<&'static Resource> {
    let mut resources: Vec<_> = OFGA_MODELS
        .values()
        .filter(|r| r.visible)
        .collect();
    resources.sort_by_key(|r| r.order);
    resources
}

/// Get all top-level resource types
pub fn get_top_level_resources() -> Vec<&'static Resource> {
    let mut resources: Vec<_> = OFGA_MODELS
        .values()
        .filter(|r| r.top_level && r.visible)
        .collect();
    resources.sort_by_key(|r| r.order);
    resources
}

/// Get child resources for a parent type
pub fn get_child_resources(parent_key: &str) -> Vec<&'static Resource> {
    let mut resources: Vec<_> = OFGA_MODELS
        .values()
        .filter(|r| r.parent.as_deref() == Some(parent_key))
        .collect();
    resources.sort_by_key(|r| r.order);
    resources
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ofga_models_contains_required_types() {
        assert!(OFGA_MODELS.contains_key("user"));
        assert!(OFGA_MODELS.contains_key("group"));
        assert!(OFGA_MODELS.contains_key("role"));
        assert!(OFGA_MODELS.contains_key("org"));
        assert!(OFGA_MODELS.contains_key("logs"));
        assert!(OFGA_MODELS.contains_key("dashboard"));
        assert!(OFGA_MODELS.contains_key("alert"));
    }

    #[test]
    fn test_stream_hierarchy() {
        let logs = get_resource("logs").unwrap();
        assert_eq!(logs.parent.as_deref(), Some("stream"));

        let metrics = get_resource("metrics").unwrap();
        assert_eq!(metrics.parent.as_deref(), Some("stream"));

        let traces = get_resource("traces").unwrap();
        assert_eq!(traces.parent.as_deref(), Some("stream"));
    }

    #[test]
    fn test_dashboard_hierarchy() {
        let dashboard = get_resource("dashboard").unwrap();
        assert_eq!(dashboard.parent.as_deref(), Some("dfolder"));
    }

    #[test]
    fn test_get_visible_resources() {
        let visible = get_visible_resources();
        assert!(!visible.is_empty());
        // Stream should not be visible (it's a parent type only)
        assert!(!visible.iter().any(|r| r.key == "stream"));
    }

    #[test]
    fn test_get_child_resources() {
        let stream_children = get_child_resources("stream");
        assert!(!stream_children.is_empty());
        assert!(stream_children.iter().any(|r| r.key == "logs"));
        assert!(stream_children.iter().any(|r| r.key == "metrics"));
        assert!(stream_children.iter().any(|r| r.key == "traces"));
    }

    #[test]
    fn test_non_cloud_resources() {
        assert!(NON_CLOUD_RESOURCE_KEYS.contains("license"));
        assert!(NON_CLOUD_RESOURCE_KEYS.contains("cipher_keys"));
        assert!(!NON_CLOUD_RESOURCE_KEYS.contains("logs"));
    }
}
