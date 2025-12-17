// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Role management service

use std::collections::HashSet;

use crate::Visdata;
use super::super::error::{Error, Result};
use super::super::model::schema;
use super::super::types::{TupleKey, TupleKeyFilter, PermissionEntry, UserRoleOption};
use super::tuples;

/// System roles that cannot be deleted
const SYSTEM_ROLES: &[&str] = &["admin", "editor", "viewer"];

/// Placeholder user for role creation marker
const ROLE_MARKER_USER: &str = "user:__role_marker__";

/// Create a new role
pub async fn create_role(org_id: &str, role_name: &str) -> Result<()> {
    // Check if role already exists
    let existing = list_roles(org_id).await?;
    if existing.iter().any(|r| r.eq_ignore_ascii_case(role_name)) {
        return Err(Error::DuplicateEntry(format!(
            "Role '{}' already exists",
            role_name
        )));
    }

    // Check if trying to create a system role name
    if SYSTEM_ROLES.iter().any(|r| r.eq_ignore_ascii_case(role_name)) {
        return Err(Error::Validation(format!(
            "Cannot create role with system name: {}",
            role_name
        )));
    }

    // Write a marker tuple to OpenFGA so the role appears in list_roles
    // We use a placeholder user as assignee to mark the role as existing
    let role_object = schema::role_type(org_id, role_name);
    let marker_tuple = TupleKey::new(ROLE_MARKER_USER, "assignee", &role_object);
    tuples::update_tuples(vec![marker_tuple], vec![]).await?;

    tracing::info!("[RBAC] Created role: {} in org {}", role_name, org_id);

    Ok(())
}

/// List all roles in an organization (excludes system roles - they are fixed)
pub async fn list_roles(org_id: &str) -> Result<Vec<String>> {
    let visdata = Visdata::global();

    // Read all tuples without filter (OpenFGA requires object type in filter)
    let all_tuples = visdata.openfga().read(None).await?;

    // Extract unique role names for this org (excluding system roles)
    let prefix = format!("role:{}_", org_id);
    let mut roles: HashSet<String> = HashSet::new();

    for tuple in all_tuples {
        if tuple.key.object.starts_with(&prefix) {
            if let Some(role_name) = tuple.key.object.strip_prefix(&prefix) {
                // Skip system roles - they have fixed permissions and shouldn't be edited
                if !SYSTEM_ROLES.iter().any(|r| r.eq_ignore_ascii_case(role_name)) {
                    roles.insert(role_name.to_string());
                }
            }
        }
    }

    let mut result: Vec<String> = roles.into_iter().collect();
    result.sort();

    Ok(result)
}

/// List system roles (for user assignment dropdown)
pub async fn list_system_roles(_org_id: &str) -> Vec<UserRoleOption> {
    SYSTEM_ROLES
        .iter()
        .map(|r| UserRoleOption {
            label: capitalize(r),
            value: r.to_string(),
        })
        .collect()
}

/// List custom (non-system) roles
pub async fn list_custom_roles(org_id: &str) -> Result<Vec<UserRoleOption>> {
    let all_roles = list_roles(org_id).await?;

    let custom: Vec<UserRoleOption> = all_roles
        .into_iter()
        .filter(|r| !SYSTEM_ROLES.iter().any(|s| s.eq_ignore_ascii_case(r)))
        .map(|r| UserRoleOption {
            label: capitalize(&r),
            value: r.to_lowercase(),
        })
        .collect();

    Ok(custom)
}

/// Delete a role
pub async fn delete_role(org_id: &str, role_name: &str) -> Result<()> {
    // Cannot delete system roles
    if SYSTEM_ROLES.iter().any(|r| r.eq_ignore_ascii_case(role_name)) {
        return Err(Error::Validation(format!(
            "Cannot delete system role: {}",
            role_name
        )));
    }

    let visdata = Visdata::global();
    let role_object = schema::role_type(org_id, role_name);

    // Find all tuples related to this role
    let filter = TupleKeyFilter {
        user: None,
        relation: None,
        object: Some(role_object.clone()),
    };

    let role_tuples = visdata.openfga().read(Some(filter)).await?;

    // Also find tuples where role is the user (for permission grants)
    let role_assignee = format!("{}#assignee", role_object);
    let filter2 = TupleKeyFilter {
        user: Some(role_assignee),
        relation: None,
        object: None,
    };

    let permission_tuples = visdata.openfga().read(Some(filter2)).await?;

    // Delete all related tuples
    let mut deletes: Vec<TupleKey> = Vec::new();
    deletes.extend(role_tuples.into_iter().map(|t| t.key));
    deletes.extend(permission_tuples.into_iter().map(|t| t.key));

    if !deletes.is_empty() {
        tuples::update_tuples(vec![], deletes).await?;
    }

    tracing::info!("[RBAC] Deleted role: {} in org {}", role_name, org_id);

    Ok(())
}

/// Get users assigned to a role
pub async fn get_role_users(org_id: &str, role_name: &str) -> Result<Vec<String>> {
    let visdata = Visdata::global();
    let role_object = schema::role_type(org_id, role_name);

    // Find all users with assignee relation to this role
    let filter = TupleKeyFilter {
        user: None,
        relation: Some("assignee".to_string()),
        object: Some(role_object),
    };

    let tuples = visdata.openfga().read(Some(filter)).await?;

    // Extract user emails
    let users: Vec<String> = tuples
        .into_iter()
        .filter_map(|t| {
            t.key.user.strip_prefix("user:").map(|s| s.to_string())
        })
        .collect();

    Ok(users)
}

/// Get permissions assigned to a role for a specific resource type
pub async fn get_role_permissions(
    org_id: &str,
    role_name: &str,
    resource_type: &str,
) -> Result<Vec<PermissionEntry>> {
    let visdata = Visdata::global();
    let role_object = schema::role_type(org_id, role_name);
    let role_assignee = format!("{}#assignee", role_object);

    tracing::debug!(
        "[RBAC] get_role_permissions: org={}, role={}, resource_type={}, role_assignee={}",
        org_id, role_name, resource_type, role_assignee
    );

    // Find all permission tuples for this role
    let filter = TupleKeyFilter {
        user: Some(role_assignee.clone()),
        relation: None,
        object: None,
    };

    let tuples = visdata.openfga().read(Some(filter)).await?;

    tracing::debug!(
        "[RBAC] Found {} tuples for role_assignee={}",
        tuples.len(), role_assignee
    );

    // Filter to matching resource type and convert to PermissionEntry
    let resource_prefix = format!("resource:{}_{}_", org_id, resource_type);

    let permissions: Vec<PermissionEntry> = tuples
        .into_iter()
        .filter(|t| {
            let matches = t.key.object.starts_with(&resource_prefix);
            tracing::debug!(
                "[RBAC] Checking tuple object={}, prefix={}, matches={}",
                t.key.object, resource_prefix, matches
            );
            matches
        })
        .map(|t| {
            let entity = t.key.object
                .strip_prefix(&resource_prefix)
                .unwrap_or(&t.key.object)
                .to_string();

            let permission = relation_to_permission(&t.key.relation);

            PermissionEntry {
                object: format!("{}:{}", resource_type, entity),
                permission,
            }
        })
        .collect();

    tracing::debug!(
        "[RBAC] Returning {} permissions for resource_type={}",
        permissions.len(), resource_type
    );

    Ok(permissions)
}

/// Add permissions to a role
pub async fn add_role_permissions(
    org_id: &str,
    role_name: &str,
    permissions: &[PermissionEntry],
) -> Result<()> {
    if permissions.is_empty() {
        return Ok(());
    }

    let role_object = schema::role_type(org_id, role_name);
    let role_assignee = format!("{}#assignee", role_object);

    let mut writes = Vec::new();

    for perm in permissions {
        // Parse object format: "resource_type:entity_id"
        let (resource_type, entity_id) = match perm.object.split_once(':') {
            Some(parts) => parts,
            None => continue,
        };

        let relation = permission_to_relation(&perm.permission);
        let resource = if entity_id.starts_with("_all") {
            schema::resource_object_all(org_id, resource_type)
        } else {
            schema::resource_object(org_id, resource_type, entity_id)
        };

        writes.push(TupleKey::new(&role_assignee, relation, &resource));
    }

    if !writes.is_empty() {
        tuples::update_tuples(writes, vec![]).await?;
    }

    Ok(())
}

/// Remove permissions from a role
pub async fn remove_role_permissions(
    org_id: &str,
    role_name: &str,
    permissions: &[PermissionEntry],
) -> Result<()> {
    if permissions.is_empty() {
        return Ok(());
    }

    let role_object = schema::role_type(org_id, role_name);
    let role_assignee = format!("{}#assignee", role_object);

    let mut deletes = Vec::new();

    for perm in permissions {
        let (resource_type, entity_id) = match perm.object.split_once(':') {
            Some(parts) => parts,
            None => continue,
        };

        let relation = permission_to_relation(&perm.permission);
        let resource = if entity_id.starts_with("_all") {
            schema::resource_object_all(org_id, resource_type)
        } else {
            schema::resource_object(org_id, resource_type, entity_id)
        };

        deletes.push(TupleKey::new(&role_assignee, relation, &resource));
    }

    if !deletes.is_empty() {
        tuples::update_tuples(vec![], deletes).await?;
    }

    Ok(())
}

/// Add users to a role
pub async fn add_role_users(
    org_id: &str,
    role_name: &str,
    users: &HashSet<String>,
) -> Result<()> {
    if users.is_empty() {
        return Ok(());
    }

    let writes: Vec<TupleKey> = users
        .iter()
        .map(|email| tuples::get_user_crole_tuple(org_id, role_name, email))
        .collect();

    tuples::update_tuples(writes, vec![]).await
}

/// Remove users from a role
pub async fn remove_role_users(
    org_id: &str,
    role_name: &str,
    users: &HashSet<String>,
) -> Result<()> {
    if users.is_empty() {
        return Ok(());
    }

    let deletes: Vec<TupleKey> = users
        .iter()
        .map(|email| tuples::get_user_crole_tuple(org_id, role_name, email))
        .collect();

    tuples::update_tuples(vec![], deletes).await
}

/// Convert permission string to OpenFGA relation
fn permission_to_relation(permission: &str) -> &'static str {
    match permission.to_lowercase().as_str() {
        "allowall" => "admin",
        "allowlist" => "can_list",
        "allowget" => "can_read",
        "allowpost" => "can_create",
        "allowput" => "can_update",
        "allowdelete" => "can_delete",
        _ => "can_read",
    }
}

/// Convert OpenFGA relation to permission string
fn relation_to_permission(relation: &str) -> String {
    match relation {
        "admin" => "AllowAll",
        "can_list" => "AllowList",
        "can_read" => "AllowGet",
        "can_create" => "AllowPost",
        "can_update" => "AllowPut",
        "can_delete" => "AllowDelete",
        _ => "AllowGet",
    }
    .to_string()
}

/// Capitalize first letter
fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_conversion() {
        assert_eq!(permission_to_relation("AllowAll"), "admin");
        assert_eq!(permission_to_relation("AllowGet"), "can_read");
        assert_eq!(relation_to_permission("admin"), "AllowAll");
        assert_eq!(relation_to_permission("can_read"), "AllowGet");
    }

    #[test]
    fn test_capitalize() {
        assert_eq!(capitalize("admin"), "Admin");
        assert_eq!(capitalize("viewer"), "Viewer");
        assert_eq!(capitalize(""), "");
    }
}
