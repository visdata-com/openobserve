// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Role management API (compatible with o2_openfga::authorizer::roles)
//!
//! This module provides role management functions that are compatible
//! with the o2_openfga::authorizer::roles API.

use std::collections::HashSet;

use crate::openfga::error::Result;
use crate::openfga::service::roles as role_service;
use crate::openfga::service::tuples;
use crate::openfga::types::{PermissionEntry, RoleResponse, UserRoleOption};

// Re-export from tuples for compatibility with o2_openfga::authorizer::roles
pub use tuples::{get_role_key, get_user_crole_removal_tuples};

/// Create a new role (compatible with o2_openfga::authorizer::roles::create_role)
pub async fn create_role(org_id: &str, role_name: &str) -> Result<()> {
    role_service::create_role(org_id, role_name).await
}

/// Delete a role (compatible with o2_openfga::authorizer::roles::delete_role)
pub async fn delete_role(org_id: &str, role_name: &str) -> Result<()> {
    role_service::delete_role(org_id, role_name).await
}

/// Get all roles in an organization (compatible with o2_openfga::authorizer::roles::get_all_roles)
///
/// Returns a list of custom role names (excluding system roles).
/// If `permitted` is Some, only return roles that are in the permitted list.
pub async fn get_all_roles(org_id: &str, permitted: Option<Vec<String>>) -> Result<Vec<String>> {
    let all_roles = role_service::list_roles(org_id).await?;

    // Filter by permitted if specified
    match permitted {
        Some(allowed) => {
            let filtered: Vec<String> = all_roles
                .into_iter()
                .filter(|role| allowed.iter().any(|p| p.eq_ignore_ascii_case(role)))
                .collect();
            Ok(filtered)
        }
        None => Ok(all_roles),
    }
}

/// Get all roles including system roles for dropdown options
pub async fn get_all_role_options(org_id: &str) -> Result<Vec<UserRoleOption>> {
    let mut options = role_service::list_system_roles(org_id).await;
    let custom = role_service::list_custom_roles(org_id).await?;
    options.extend(custom);
    Ok(options)
}

/// Update a role (compatible with o2_openfga::authorizer::roles::update_role)
///
/// This function handles adding/removing permissions and users to/from a role.
pub async fn update_role(
    org_id: &str,
    role_name: &str,
    add_permissions: Option<&[PermissionEntry]>,
    remove_permissions: Option<&[PermissionEntry]>,
    add_users: Option<&HashSet<String>>,
    remove_users: Option<&HashSet<String>>,
) -> Result<()> {
    // Add permissions
    if let Some(perms) = add_permissions {
        if !perms.is_empty() {
            role_service::add_role_permissions(org_id, role_name, perms).await?;
        }
    }

    // Remove permissions
    if let Some(perms) = remove_permissions {
        if !perms.is_empty() {
            role_service::remove_role_permissions(org_id, role_name, perms).await?;
        }
    }

    // Add users
    if let Some(users) = add_users {
        if !users.is_empty() {
            role_service::add_role_users(org_id, role_name, users).await?;
        }
    }

    // Remove users
    if let Some(users) = remove_users {
        if !users.is_empty() {
            role_service::remove_role_users(org_id, role_name, users).await?;
        }
    }

    Ok(())
}

/// Get role permissions (compatible with o2_openfga::authorizer::roles::get_role_permissions)
pub async fn get_role_permissions(
    org_id: &str,
    role_name: &str,
    resource_type: &str,
) -> Result<Vec<PermissionEntry>> {
    role_service::get_role_permissions(org_id, role_name, resource_type).await
}

/// Get users with a specific role (compatible with o2_openfga::authorizer::roles::get_users_with_role)
pub async fn get_users_with_role(org_id: &str, role_name: &str) -> Result<Vec<String>> {
    role_service::get_role_users(org_id, role_name).await
}

/// Get roles for a user in an organization (compatible with o2_openfga::authorizer::roles::get_roles_for_org_user)
pub async fn get_roles_for_org_user(org_id: &str, user_email: &str) -> Result<Vec<String>> {
    crate::openfga::service::groups::get_user_roles(org_id, user_email).await
}

/// Get a role by name with full details
pub async fn get_role(org_id: &str, role_name: &str) -> Result<RoleResponse> {
    let users = get_users_with_role(org_id, role_name).await?;
    let now = chrono::Utc::now().timestamp_micros();

    Ok(RoleResponse {
        name: role_name.to_string(),
        label: capitalize(role_name),
        users,
        created_at: now,
        updated_at: now,
    })
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
    fn test_capitalize() {
        assert_eq!(capitalize("admin"), "Admin");
        assert_eq!(capitalize("developer"), "Developer");
        assert_eq!(capitalize(""), "");
    }
}
