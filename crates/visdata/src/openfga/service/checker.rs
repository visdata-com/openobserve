// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Permission checking service (compatible with o2_openfga::authorizer::authz)

use crate::Visdata;
use super::super::error::Result;
use super::super::model::{resources, schema};
use super::super::types::{Permission, TupleKey};

/// Check if a user has permission on an object
///
/// Compatible with o2_openfga::authorizer::authz::is_allowed
pub async fn is_allowed(
    org_id: &str,
    user_id: &str,
    method: &str,
    object: &str,        // Format: "resource_type:entity_id"
    _parent_id: &str,    // Not used in current implementation
    role: &str,
) -> Result<bool> {
    let visdata = Visdata::global();
    let config = visdata.openfga().config().await;

    // Skip check if not enabled
    if !config.enabled {
        return Ok(true);
    }

    // Root users bypass all checks
    if role.eq_ignore_ascii_case("root") {
        return Ok(true);
    }

    // Parse object format: "resource_type:entity_id"
    let (resource_type, entity_id) = match resources::parse_object(object) {
        Some(parts) => parts,
        None => {
            tracing::warn!("[RBAC] Invalid object format: {}", object);
            return Ok(false);
        }
    };

    // Validate resource type
    if !resources::is_valid_resource_type(resource_type) {
        tracing::warn!("[RBAC] Unknown resource type: {}", resource_type);
        return Ok(false);
    }

    // Determine if this is a list operation
    let is_list = resources::is_all_org_entity(entity_id, org_id);

    // Convert HTTP method to permission
    let permission = Permission::from_method(method, is_list);
    let relation = permission.to_relation();

    // Build tuple key for check
    let user = schema::user_type(user_id);
    let fga_object = if is_list {
        schema::resource_object_all(org_id, resource_type)
    } else {
        schema::resource_object(org_id, resource_type, entity_id)
    };

    let tuple_key = TupleKey::new(&user, relation, &fga_object);

    // Perform check
    match visdata.openfga().check(&tuple_key).await {
        Ok(allowed) => {
            tracing::debug!(
                "[RBAC] Check: user={}, relation={}, object={} -> {}",
                user_id, relation, fga_object, allowed
            );
            Ok(allowed)
        }
        Err(e) => {
            tracing::error!("[RBAC] Check failed: {}", e);
            Ok(false)
        }
    }
}

/// Check user permissions (for use in HTTP validator)
///
/// This is a simplified wrapper around is_allowed for use in middleware
pub async fn check_permissions(
    user_id: &str,
    org_id: &str,
    method: &str,
    object: &str,
    role: &str,
) -> bool {
    match is_allowed(org_id, user_id, method, object, "", role).await {
        Ok(allowed) => allowed,
        Err(e) => {
            tracing::error!("[RBAC] Permission check error: {}", e);
            false
        }
    }
}

/// List objects that a user can access with a specific permission
///
/// Compatible with o2_openfga::authorizer::authz::list_objects
///
/// Returns:
/// - Some(Vec<String>) if permission filtering is enabled
/// - None if permission filtering is disabled (return all objects)
pub async fn list_objects_for_user(
    org_id: &str,
    user_id: &str,
    permission: &str,       // "AllowList" or "AllowGet"
    object_type: &str,      // "role", "group", "dashboard", etc.
    role: &str,
) -> Result<Option<Vec<String>>> {
    let visdata = Visdata::global();
    let config = visdata.openfga().config().await;

    // Skip if not enabled or user is root
    if !config.enabled || role.eq_ignore_ascii_case("root") {
        return Ok(None);
    }

    // Skip if list_only_permitted is disabled
    if !config.list_only_permitted {
        return Ok(None);
    }

    // Convert permission string to relation
    let relation = match Permission::from_str(permission) {
        Some(p) => p.to_relation(),
        None => "can_read", // Default to read permission
    };

    // Build user type
    let user = schema::user_type(user_id);

    // The object type in OpenFGA is "resource" with org-scoped naming
    let fga_type = "resource";

    // List objects
    match visdata.openfga().list_objects(&user, relation, fga_type).await {
        Ok(objects) => {
            // Filter to only include objects from this org and resource type
            let prefix = format!("resource:{}_{}_{}", org_id, object_type, "");
            let filtered: Vec<String> = objects
                .into_iter()
                .filter(|o| o.starts_with(&prefix))
                .map(|o| {
                    // Extract entity ID from "resource:{org}_{type}_{entity}"
                    o.strip_prefix(&prefix)
                        .unwrap_or(&o)
                        .to_string()
                })
                .collect();

            Ok(Some(filtered))
        }
        Err(e) => {
            tracing::error!("[RBAC] List objects failed: {}", e);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_to_relation() {
        assert_eq!(Permission::AllowAll.to_relation(), "admin");
        assert_eq!(Permission::AllowGet.to_relation(), "can_read");
        assert_eq!(Permission::AllowList.to_relation(), "can_list");
        assert_eq!(Permission::AllowPost.to_relation(), "can_create");
        assert_eq!(Permission::AllowPut.to_relation(), "can_update");
        assert_eq!(Permission::AllowDelete.to_relation(), "can_delete");
    }

    #[test]
    fn test_permission_from_method() {
        assert_eq!(Permission::from_method("GET", false), Permission::AllowGet);
        assert_eq!(Permission::from_method("GET", true), Permission::AllowList);
        assert_eq!(Permission::from_method("POST", false), Permission::AllowPost);
        assert_eq!(Permission::from_method("PUT", false), Permission::AllowPut);
        assert_eq!(Permission::from_method("DELETE", false), Permission::AllowDelete);
    }
}
