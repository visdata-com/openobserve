// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Resource type definitions (compatible with OFGA_MODELS)

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
pub static RESOURCE_TYPES: Lazy<HashMap<&'static str, Resource>> = Lazy::new(|| {
    let mut m = HashMap::new();

    // Stream types (parent: stream) - have entities (actual log streams, metric streams, etc.)
    m.insert("logs", resource("logs", "Logs", Some("stream"), 1, true, true));
    m.insert("metrics", resource("metrics", "Metrics", Some("stream"), 2, true, true));
    m.insert("traces", resource("traces", "Traces", Some("stream"), 3, true, true));
    m.insert("index", resource("index", "Index", Some("stream"), 4, true, true));

    // Stream (abstract parent - not visible, no entities itself)
    m.insert("stream", resource("stream", "Streams", None, 5, false, false));

    // Dashboard and folders - dashboards have entities
    m.insert("dashboard", resource("dashboard", "Dashboard", Some("dfolder"), 10, true, true));
    m.insert("dfolder", resource("dfolder", "Dashboard Folders", None, 11, true, true));
    m.insert("templates", resource("templates", "Templates", None, 12, true, true));

    // Alerts - alerts and folders have entities
    m.insert("alert", resource("alert", "Alert", Some("alert_folders"), 20, true, true));
    m.insert("alert_folders", resource("alert_folders", "Alert Folders", None, 21, true, true));
    m.insert("destinations", resource("destinations", "Destinations", None, 22, true, true));

    // Functions and pipelines - have entities
    m.insert("functions", resource("functions", "Functions", None, 30, true, true));
    m.insert("pipeline", resource("pipeline", "Pipeline", None, 31, true, true));

    // Reports and saved views - have entities
    m.insert("reports", resource("reports", "Reports", None, 40, true, true));
    m.insert("savedviews", resource("savedviews", "Saved Views", None, 41, true, true));

    // RBAC - have entities (specific roles and groups)
    m.insert("role", resource("role", "Role", None, 50, true, true));
    m.insert("group", resource("group", "Group", None, 51, true, true));

    // Organization management - have entities
    m.insert("org", resource("org", "Organization", None, 60, true, false));
    m.insert("serviceaccounts", resource("serviceaccounts", "Service Accounts", None, 61, true, true));

    // Other features - have entities
    m.insert("actionscripts", resource("actionscripts", "Action Scripts", None, 70, true, true));
    m.insert("cipherkeys", resource("cipherkeys", "Cipher Keys", None, 71, true, true));

    // Settings (usually admin only) - no specific entities
    m.insert("settings", resource("settings", "Settings", None, 80, true, false));
    m.insert("metadata", resource("metadata", "Metadata", None, 81, false, false));

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
    entity == format!("_all_{}", org_id) || entity == "_all"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_types() {
        assert!(RESOURCE_TYPES.contains_key("logs"));
        assert!(RESOURCE_TYPES.contains_key("dashboard"));
        assert!(RESOURCE_TYPES.contains_key("role"));
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
    fn test_top_level() {
        // Resources without parent should be top_level
        let stream = RESOURCE_TYPES.get("stream").unwrap();
        assert!(stream.top_level);

        // Resources with parent should not be top_level
        let logs = RESOURCE_TYPES.get("logs").unwrap();
        assert!(!logs.top_level);
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
        assert!(!is_all_org_entity("my_stream", "org123"));
    }
}
