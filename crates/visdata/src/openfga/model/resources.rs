// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Resource type definitions (compatible with OFGA_MODELS)
//!
//! This module defines all 30+ resource types from store.yaml.

use once_cell::sync::Lazy;
use std::collections::HashMap;

use super::super::types::Resource;

/// Helper to create a resource with default values
fn resource(
    key: &str,
    label: &str,
    parent: Option<&str>,
    order: i32,
    visible: bool,
    has_entities: bool,
) -> Resource {
    let top_level = parent.is_none();
    Resource {
        key: key.to_string(),
        label: label.to_string(),
        parent: parent.map(|s| s.to_string()),
        order,
        visible,
        top_level,
        has_entities,
    }
}

/// Resource types mapping (compatible with o2_openfga::meta::mapping::OFGA_MODELS)
///
/// This includes all 30+ resource types defined in store.yaml:
/// - Core types: user, group, role, org
/// - Stream hierarchy: stream, logs, metrics, traces, metadata, index
/// - Dashboard hierarchy: dfolder, dashboard, template, savedviews
/// - Alert hierarchy: afolder, alert, destination
/// - Report hierarchy: rfolder, report
/// - Functions: function, pipeline
/// - System: settings, kv, enrichment_table, summary, syslog-route
/// - Security: passcode, rumtoken, service_accounts, cipher_keys
/// - Other: search_jobs, action_scripts, ratelimit, ai, re_patterns, license
pub static RESOURCE_TYPES: Lazy<HashMap<&'static str, Resource>> = Lazy::new(|| {
    let mut m = HashMap::new();

    // ========================================================================
    // Core types
    // ========================================================================
    m.insert("user", resource("user", "Users", None, 1, true, true));
    m.insert("group", resource("group", "Groups", None, 2, true, true));
    m.insert("role", resource("role", "Roles", None, 3, true, true));
    m.insert("org", resource("org", "Organizations", None, 4, true, false));

    // ========================================================================
    // Stream hierarchy (data streams)
    // ========================================================================
    m.insert("stream", resource("stream", "Streams", None, 10, false, true));
    m.insert("logs", resource("logs", "Logs", Some("stream"), 11, true, true));
    m.insert("metrics", resource("metrics", "Metrics", Some("stream"), 12, true, true));
    m.insert("traces", resource("traces", "Traces", Some("stream"), 13, true, true));
    m.insert("metadata", resource("metadata", "Metadata", Some("stream"), 14, false, true));
    m.insert("index", resource("index", "Index", Some("stream"), 15, true, true));

    // ========================================================================
    // Dashboard hierarchy
    // ========================================================================
    m.insert("dfolder", resource("dfolder", "Dashboard Folders", None, 20, true, true));
    m.insert("dashboard", resource("dashboard", "Dashboards", Some("dfolder"), 21, true, true));
    m.insert("template", resource("template", "Templates", None, 22, true, true));
    m.insert("savedviews", resource("savedviews", "Saved Views", None, 23, true, true));

    // ========================================================================
    // Alert hierarchy
    // ========================================================================
    m.insert("afolder", resource("afolder", "Alert Folders", None, 30, true, true));
    m.insert("alert", resource("alert", "Alerts", Some("afolder"), 31, true, true));
    m.insert("destination", resource("destination", "Destinations", None, 32, true, true));

    // ========================================================================
    // Report hierarchy
    // ========================================================================
    m.insert("rfolder", resource("rfolder", "Report Folders", None, 40, true, true));
    m.insert("report", resource("report", "Reports", Some("rfolder"), 41, true, true));

    // ========================================================================
    // Functions and pipelines
    // ========================================================================
    m.insert("function", resource("function", "Functions", None, 50, true, true));
    m.insert("pipeline", resource("pipeline", "Pipelines", None, 51, true, true));

    // ========================================================================
    // System resources
    // ========================================================================
    m.insert("settings", resource("settings", "Settings", None, 60, true, false));
    m.insert("kv", resource("kv", "KV Store", None, 61, true, true));
    m.insert("enrichment_table", resource("enrichment_table", "Enrichment Tables", None, 62, true, true));
    m.insert("summary", resource("summary", "Summary", None, 63, true, true));
    m.insert("syslog-route", resource("syslog-route", "Syslog Routes", None, 64, true, true));

    // ========================================================================
    // Security and authentication
    // ========================================================================
    m.insert("passcode", resource("passcode", "Passcodes", None, 70, true, true));
    m.insert("rumtoken", resource("rumtoken", "RUM Tokens", None, 71, true, true));
    m.insert("service_accounts", resource("service_accounts", "Service Accounts", None, 72, true, true));
    m.insert("cipher_keys", resource("cipher_keys", "Cipher Keys", None, 73, true, true));

    // ========================================================================
    // Other resources
    // ========================================================================
    m.insert("search_jobs", resource("search_jobs", "Search Jobs", None, 80, true, true));
    m.insert("action_scripts", resource("action_scripts", "Action Scripts", None, 81, true, true));
    m.insert("ratelimit", resource("ratelimit", "Rate Limits", None, 82, true, true));
    m.insert("ai", resource("ai", "AI", None, 83, true, false));
    m.insert("re_patterns", resource("re_patterns", "Regex Patterns", None, 84, true, true));
    m.insert("license", resource("license", "License", None, 90, true, false));

    // ========================================================================
    // Legacy/backward compatibility aliases
    // ========================================================================
    // These provide compatibility with older code that uses different names
    m.insert("templates", resource("templates", "Templates", None, 22, true, true));
    m.insert("functions", resource("functions", "Functions", None, 50, true, true));
    m.insert("reports", resource("reports", "Reports", None, 41, true, true));
    m.insert("destinations", resource("destinations", "Destinations", None, 32, true, true));
    m.insert("alert_folders", resource("alert_folders", "Alert Folders", None, 30, true, true));
    m.insert("serviceaccounts", resource("serviceaccounts", "Service Accounts", None, 72, true, true));
    m.insert("actionscripts", resource("actionscripts", "Action Scripts", None, 81, true, true));
    m.insert("cipherkeys", resource("cipherkeys", "Cipher Keys", None, 73, true, true));

    m
});

/// Get resource by key
pub fn get_resource(key: &str) -> Option<&Resource> {
    RESOURCE_TYPES.get(key)
}

/// Get all visible resources sorted by order
pub fn get_all_resources() -> Vec<&'static Resource> {
    let mut resources: Vec<_> = RESOURCE_TYPES.values().filter(|r| r.visible).collect();
    resources.sort_by_key(|r| r.order);
    resources
}

/// Get top-level resources (no parent)
pub fn get_top_level_resources() -> Vec<&'static Resource> {
    let mut resources: Vec<_> = RESOURCE_TYPES
        .values()
        .filter(|r| r.top_level && r.visible)
        .collect();
    resources.sort_by_key(|r| r.order);
    resources
}

/// Get child resources for a parent type
pub fn get_child_resources(parent_key: &str) -> Vec<&'static Resource> {
    let mut resources: Vec<_> = RESOURCE_TYPES
        .values()
        .filter(|r| r.parent.as_deref() == Some(parent_key))
        .collect();
    resources.sort_by_key(|r| r.order);
    resources
}

/// Check if a resource type is valid
pub fn is_valid_resource_type(key: &str) -> bool {
    RESOURCE_TYPES.contains_key(key)
}

/// Get the OpenFGA type name for a resource
/// Format: "org_{org_id}_{resource_key}"
pub fn get_fga_type(org_id: &str, resource_key: &str) -> String {
    format!("org_{}_{}", org_id, resource_key)
}

/// Parse resource:entity format
/// Returns (resource_type, entity_id)
pub fn parse_object(object: &str) -> Option<(&str, &str)> {
    object.split_once(':')
}

/// Check if entity is an "all org" wildcard
pub fn is_all_org_entity(entity: &str, org_id: &str) -> bool {
    entity == format!("_all_{}", org_id) || entity == "_all" || entity.starts_with("_all")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_types_count() {
        // Should have 30+ unique resource types
        assert!(RESOURCE_TYPES.len() >= 30);
    }

    #[test]
    fn test_core_types_exist() {
        assert!(RESOURCE_TYPES.contains_key("user"));
        assert!(RESOURCE_TYPES.contains_key("group"));
        assert!(RESOURCE_TYPES.contains_key("role"));
        assert!(RESOURCE_TYPES.contains_key("org"));
    }

    #[test]
    fn test_stream_hierarchy() {
        assert!(RESOURCE_TYPES.contains_key("stream"));
        assert!(RESOURCE_TYPES.contains_key("logs"));
        assert!(RESOURCE_TYPES.contains_key("metrics"));
        assert!(RESOURCE_TYPES.contains_key("traces"));
        assert!(RESOURCE_TYPES.contains_key("index"));

        // Check parent relationships
        let logs = RESOURCE_TYPES.get("logs").unwrap();
        assert_eq!(logs.parent.as_deref(), Some("stream"));
    }

    #[test]
    fn test_dashboard_hierarchy() {
        assert!(RESOURCE_TYPES.contains_key("dfolder"));
        assert!(RESOURCE_TYPES.contains_key("dashboard"));

        let dashboard = RESOURCE_TYPES.get("dashboard").unwrap();
        assert_eq!(dashboard.parent.as_deref(), Some("dfolder"));
    }

    #[test]
    fn test_alert_hierarchy() {
        assert!(RESOURCE_TYPES.contains_key("afolder"));
        assert!(RESOURCE_TYPES.contains_key("alert"));
        assert!(RESOURCE_TYPES.contains_key("destination"));

        let alert = RESOURCE_TYPES.get("alert").unwrap();
        assert_eq!(alert.parent.as_deref(), Some("afolder"));
    }

    #[test]
    fn test_report_hierarchy() {
        assert!(RESOURCE_TYPES.contains_key("rfolder"));
        assert!(RESOURCE_TYPES.contains_key("report"));

        let report = RESOURCE_TYPES.get("report").unwrap();
        assert_eq!(report.parent.as_deref(), Some("rfolder"));
    }

    #[test]
    fn test_get_all_resources() {
        let resources = get_all_resources();
        assert!(!resources.is_empty());
        // Should be sorted by order
        for i in 1..resources.len() {
            assert!(resources[i - 1].order <= resources[i].order);
        }
    }

    #[test]
    fn test_get_top_level_resources() {
        let resources = get_top_level_resources();
        assert!(!resources.is_empty());
        // All should have no parent
        for r in resources {
            assert!(r.parent.is_none());
        }
    }

    #[test]
    fn test_get_child_resources() {
        let children = get_child_resources("stream");
        assert!(!children.is_empty());
        assert!(children.iter().any(|r| r.key == "logs"));
        assert!(children.iter().any(|r| r.key == "metrics"));
        assert!(children.iter().any(|r| r.key == "traces"));
    }

    #[test]
    fn test_parse_object() {
        let result = parse_object("logs:my_stream");
        assert_eq!(result, Some(("logs", "my_stream")));

        let result = parse_object("dashboard:folder/dash1");
        assert_eq!(result, Some(("dashboard", "folder/dash1")));
    }

    #[test]
    fn test_is_all_org_entity() {
        assert!(is_all_org_entity("_all_org123", "org123"));
        assert!(is_all_org_entity("_all", "org123"));
        assert!(is_all_org_entity("_all_default", "default"));
        assert!(!is_all_org_entity("my_stream", "org123"));
    }
}
