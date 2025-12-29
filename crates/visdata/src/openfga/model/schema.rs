// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! OpenFGA authorization model schema
//!
//! This model is equivalent to the DSL in visdata_deploy/openfga/store.yaml
//! and defines the complete RBAC permission system.

use super::super::types::TupleKey;

/// Get the OpenFGA authorization model in JSON format
///
/// This model defines 30+ resource types with fine-grained permissions:
/// - Core types: user, group, role, org
/// - Stream hierarchy: stream, logs, metrics, traces, metadata, index
/// - Dashboard hierarchy: dfolder, dashboard, template, savedviews
/// - Alert hierarchy: afolder, alert, destination
/// - Report hierarchy: rfolder, report
/// - And many more...
///
/// Each resource type supports:
/// - ALLOW_ALL, ALLOW_GET, ALLOW_POST, ALLOW_PUT, ALLOW_DELETE, ALLOW_LIST
/// - GET, POST, PUT, DELETE, LIST (computed from ALLOW_* and inheritance)
/// - owningOrg: relationship to organization
/// - selfParent: hierarchical permission inheritance
pub fn get_authorization_model_json() -> &'static str {
    include_str!("authorization_model.json")
}

/// Get the initial tuples for bootstrapping the system
///
/// These tuples set up:
/// - Root user with admin access to default and _meta organizations
/// - Organization resource ownership relationships
/// - Folder hierarchies
/// - Stream parent relationships
pub fn get_initial_tuples() -> Vec<TupleKey> {
    vec![
        // ============================================
        // Root 用户配置
        // ============================================
        TupleKey::new("org:default", "owningOrg", "user:root@visdata.com"),
        TupleKey::new("user:root@visdata.com", "admin", "org:default"),
        TupleKey::new("user:root@visdata.com", "org_context", "org:default"),
        TupleKey::new("user:root@visdata.com", "admin", "org:_meta"),
        TupleKey::new("user:root@visdata.com", "org_context", "org:_meta"),

        // ============================================
        // Default 组织 - 资源所有权
        // ============================================
        TupleKey::new("org:default", "owningOrg", "stream:_all_default"),
        TupleKey::new("org:default", "owningOrg", "logs:_all_default"),
        TupleKey::new("org:default", "owningOrg", "metrics:_all_default"),
        TupleKey::new("org:default", "owningOrg", "traces:_all_default"),
        TupleKey::new("org:default", "owningOrg", "metadata:_all_default"),
        TupleKey::new("org:default", "owningOrg", "index:_all_default"),
        TupleKey::new("org:default", "owningOrg", "dashboard:_all_default"),
        TupleKey::new("org:default", "owningOrg", "dfolder:_all_default"),
        TupleKey::new("org:default", "owningOrg", "savedviews:_all_default"),
        TupleKey::new("org:default", "owningOrg", "report:_all_default"),
        TupleKey::new("org:default", "owningOrg", "rfolder:_all_default"),
        TupleKey::new("org:default", "owningOrg", "alert:_all_default"),
        TupleKey::new("org:default", "owningOrg", "afolder:_all_default"),
        TupleKey::new("org:default", "owningOrg", "template:_all_default"),
        TupleKey::new("org:default", "owningOrg", "destination:_all_default"),
        TupleKey::new("org:default", "owningOrg", "function:_all_default"),
        TupleKey::new("org:default", "owningOrg", "pipeline:_all_default"),
        TupleKey::new("org:default", "owningOrg", "enrichment_table:_all_default"),
        TupleKey::new("org:default", "owningOrg", "summary:_all_default"),
        TupleKey::new("org:default", "owningOrg", "settings:_all_default"),
        TupleKey::new("org:default", "owningOrg", "kv:_all_default"),
        TupleKey::new("org:default", "owningOrg", "syslog-route:_all_default"),
        TupleKey::new("org:default", "owningOrg", "ratelimit:_all_default"),
        TupleKey::new("org:default", "owningOrg", "cipher_keys:_all_default"),
        TupleKey::new("org:default", "owningOrg", "license:_all_default"),
        TupleKey::new("org:default", "owningOrg", "user:_all_default"),
        TupleKey::new("org:default", "owningOrg", "group:_all_default"),
        TupleKey::new("org:default", "owningOrg", "role:_all_default"),
        TupleKey::new("org:default", "owningOrg", "passcode:_all_default"),
        TupleKey::new("org:default", "owningOrg", "rumtoken:_all_default"),
        TupleKey::new("org:default", "owningOrg", "service_accounts:_all_default"),
        TupleKey::new("org:default", "owningOrg", "search_jobs:_all_default"),
        TupleKey::new("org:default", "owningOrg", "action_scripts:_all_default"),
        TupleKey::new("org:default", "owningOrg", "ai:_all_default"),
        TupleKey::new("org:default", "owningOrg", "re_patterns:_all_default"),

        // ============================================
        // Default 组织 - 文件夹层级
        // ============================================
        TupleKey::new("org:default", "owningOrg", "dfolder:default"),
        TupleKey::new("dfolder:_all_default", "selfParent", "dfolder:default"),
        TupleKey::new("org:default", "owningOrg", "afolder:default"),
        TupleKey::new("afolder:_all_default", "selfParent", "afolder:default"),

        // ============================================
        // Default 组织 - Stream 父子关系
        // ============================================
        TupleKey::new("stream:_all_default", "parent", "logs:_all_default"),
        TupleKey::new("stream:_all_default", "parent", "metrics:_all_default"),
        TupleKey::new("stream:_all_default", "parent", "traces:_all_default"),
        TupleKey::new("stream:_all_default", "parent", "index:_all_default"),
        TupleKey::new("stream:_all_default", "parent", "metadata:_all_default"),

        // ============================================
        // _meta 组织 - 资源所有权
        // ============================================
        TupleKey::new("org:_meta", "owningOrg", "logs:audit"),
        TupleKey::new("org:_meta", "owningOrg", "stream:_all__meta"),
        TupleKey::new("org:_meta", "owningOrg", "logs:_all__meta"),
        TupleKey::new("org:_meta", "owningOrg", "metrics:_all__meta"),
        TupleKey::new("org:_meta", "owningOrg", "traces:_all__meta"),
        TupleKey::new("org:_meta", "owningOrg", "metadata:_all__meta"),
        TupleKey::new("org:_meta", "owningOrg", "index:_all__meta"),
        TupleKey::new("org:_meta", "owningOrg", "dashboard:_all__meta"),
        TupleKey::new("org:_meta", "owningOrg", "dfolder:_all__meta"),
        TupleKey::new("org:_meta", "owningOrg", "savedviews:_all__meta"),
        TupleKey::new("org:_meta", "owningOrg", "report:_all__meta"),
        TupleKey::new("org:_meta", "owningOrg", "rfolder:_all__meta"),
        TupleKey::new("org:_meta", "owningOrg", "alert:_all__meta"),
        TupleKey::new("org:_meta", "owningOrg", "afolder:_all__meta"),
        TupleKey::new("org:_meta", "owningOrg", "template:_all__meta"),
        TupleKey::new("org:_meta", "owningOrg", "destination:_all__meta"),
        TupleKey::new("org:_meta", "owningOrg", "function:_all__meta"),
        TupleKey::new("org:_meta", "owningOrg", "pipeline:_all__meta"),
        TupleKey::new("org:_meta", "owningOrg", "enrichment_table:_all__meta"),
        TupleKey::new("org:_meta", "owningOrg", "summary:_all__meta"),
        TupleKey::new("org:_meta", "owningOrg", "settings:_all__meta"),
        TupleKey::new("org:_meta", "owningOrg", "kv:_all__meta"),
        TupleKey::new("org:_meta", "owningOrg", "syslog-route:_all__meta"),
        TupleKey::new("org:_meta", "owningOrg", "ratelimit:_all__meta"),
        TupleKey::new("org:_meta", "owningOrg", "cipher_keys:_all__meta"),
        TupleKey::new("org:_meta", "owningOrg", "license:_all__meta"),
        TupleKey::new("org:_meta", "owningOrg", "user:_all__meta"),
        TupleKey::new("org:_meta", "owningOrg", "group:_all__meta"),
        TupleKey::new("org:_meta", "owningOrg", "role:_all__meta"),
        TupleKey::new("org:_meta", "owningOrg", "passcode:_all__meta"),
        TupleKey::new("org:_meta", "owningOrg", "rumtoken:_all__meta"),
        TupleKey::new("org:_meta", "owningOrg", "service_accounts:_all__meta"),
        TupleKey::new("org:_meta", "owningOrg", "search_jobs:_all__meta"),
        TupleKey::new("org:_meta", "owningOrg", "action_scripts:_all__meta"),
        TupleKey::new("org:_meta", "owningOrg", "ai:_all__meta"),
        TupleKey::new("org:_meta", "owningOrg", "re_patterns:_all__meta"),

        // ============================================
        // _meta 组织 - 文件夹层级
        // ============================================
        TupleKey::new("org:_meta", "owningOrg", "dfolder:default"),
        TupleKey::new("dfolder:_all__meta", "selfParent", "dfolder:default"),
        TupleKey::new("org:_meta", "owningOrg", "afolder:default"),
        TupleKey::new("afolder:_all__meta", "selfParent", "afolder:default"),

        // ============================================
        // _meta 组织 - Stream 父子关系
        // ============================================
        TupleKey::new("stream:_all__meta", "parent", "logs:_all__meta"),
        TupleKey::new("stream:_all__meta", "parent", "metrics:_all__meta"),
        TupleKey::new("stream:_all__meta", "parent", "traces:_all__meta"),
        TupleKey::new("stream:_all__meta", "parent", "index:_all__meta"),
        TupleKey::new("stream:_all__meta", "parent", "metadata:_all__meta"),
    ]
}

/// Generate organization-scoped type name
pub fn org_type(org_id: &str) -> String {
    format!("org:{}", org_id)
}

/// Generate user type name
pub fn user_type(user_email: &str) -> String {
    format!("user:{}", user_email)
}

/// Generate role type name
pub fn role_type(org_id: &str, role_name: &str) -> String {
    format!("role:{}_{}", org_id, role_name)
}

/// Generate group type name
pub fn group_type(org_id: &str, group_name: &str) -> String {
    format!("group:{}_{}", org_id, group_name)
}

/// Generate resource object name
/// Format: "{resource_type}:{entity_id}"
/// e.g., "logs:my_stream" or "dashboard:my_dashboard"
///
/// Note: org_id is kept for API compatibility but the actual object format
/// in store.yaml uses "{type}:{entity_id}" without org prefix in entity_id.
/// The org relationship is established via owningOrg tuples.
pub fn resource_object(_org_id: &str, resource_type: &str, entity_id: &str) -> String {
    format!("{}:{}", resource_type, entity_id)
}

/// Generate "all org" resource object name (for type-level permissions)
/// Format: "{resource_type}:_all_{org_id}"
/// e.g., "logs:_all_default" means all logs resources in org "default"
pub fn resource_object_all(org_id: &str, resource_type: &str) -> String {
    format!("{}:_all_{}", resource_type, org_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_authorization_model_is_valid_json() {
        let model = get_authorization_model_json();
        let parsed: serde_json::Value = serde_json::from_str(model).unwrap();
        assert!(parsed.get("schema_version").is_some());
        assert!(parsed.get("type_definitions").is_some());
    }

    #[test]
    fn test_type_generation() {
        assert_eq!(org_type("default"), "org:default");
        assert_eq!(user_type("alice@example.com"), "user:alice@example.com");
        assert_eq!(role_type("default", "admin"), "role:default_admin");
        assert_eq!(group_type("default", "developers"), "group:default_developers");
        assert_eq!(
            resource_object("logs", "my_stream"),
            "logs:my_stream"
        );
        assert_eq!(
            resource_object_all("default", "dfolder"),
            "dfolder:_all_default"
        );
    }

    #[test]
    fn test_initial_tuples_not_empty() {
        let tuples = get_initial_tuples();
        assert!(!tuples.is_empty());
        // Should have tuples for root user
        assert!(tuples.iter().any(|t| t.user.contains("root@visdata.com")));
        // Should have tuples for default org
        assert!(tuples.iter().any(|t| t.user == "org:default" || t.object.contains("default")));
        // Should have tuples for _meta org
        assert!(tuples.iter().any(|t| t.user == "org:_meta" || t.object.contains("_meta")));
    }
}
