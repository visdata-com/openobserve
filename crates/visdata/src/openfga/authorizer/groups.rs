// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Group management API (compatible with o2_openfga::authorizer::groups)
//!
//! This module provides group management functions that are compatible
//! with the o2_openfga::authorizer::groups API.

use std::collections::HashSet;

use crate::openfga::error::Result;
use crate::openfga::service::groups as group_service;
use crate::openfga::types::GroupResponse;

/// Create a new group (compatible with o2_openfga::authorizer::groups::create_group)
///
/// Optionally accepts initial users to add to the group in one operation.
pub async fn create_group(
    org_id: &str,
    name: &str,
    display_name: Option<&str>,
    description: Option<&str>,
) -> Result<String> {
    group_service::create_group(org_id, name, display_name, description).await
}

/// Create a new group with initial users (one-step operation)
///
/// This is the preferred method when creating a group with users.
pub async fn create_group_with_users(
    org_id: &str,
    name: &str,
    users: Option<&HashSet<String>>,
) -> Result<String> {
    // First create the group
    let id = group_service::create_group(org_id, name, None, None).await?;

    // Then add users if provided
    if let Some(users) = users {
        if !users.is_empty() {
            group_service::add_group_users(org_id, name, users).await?;
        }
    }

    Ok(id)
}

/// Delete a group
pub async fn delete_group(org_id: &str, group_name: &str) -> Result<()> {
    group_service::delete_group(org_id, group_name).await
}

/// Get all groups in an organization (compatible with o2_openfga::authorizer::groups::get_all_groups)
///
/// If `permitted` is Some, only return groups that are in the permitted list.
pub async fn get_all_groups(org_id: &str, permitted: Option<Vec<String>>) -> Result<Vec<String>> {
    let all_groups = group_service::list_groups(org_id).await?;

    // Filter by permitted if specified
    match permitted {
        Some(allowed) => {
            let filtered: Vec<String> = all_groups
                .into_iter()
                .filter(|group| allowed.iter().any(|p| p.eq_ignore_ascii_case(group)))
                .collect();
            Ok(filtered)
        }
        None => Ok(all_groups),
    }
}

/// Get group details (compatible with o2_openfga::authorizer::groups::get_group_details)
pub async fn get_group_details(org_id: &str, group_name: &str) -> Result<GroupResponse> {
    group_service::get_group(org_id, group_name).await
}

/// Get groups for a user in an organization (compatible with o2_openfga::authorizer::groups::get_groups_for_org_user)
pub async fn get_groups_for_org_user(org_id: &str, user_email: &str) -> Result<Vec<String>> {
    group_service::get_user_groups(org_id, user_email).await
}

/// Update a group (compatible with o2_openfga::authorizer::groups::update_group)
///
/// This function handles adding/removing users and roles to/from a group.
pub async fn update_group(
    org_id: &str,
    group_name: &str,
    add_users: Option<&HashSet<String>>,
    remove_users: Option<&HashSet<String>>,
    add_roles: Option<&HashSet<String>>,
    remove_roles: Option<&HashSet<String>>,
) -> Result<()> {
    // Add users
    if let Some(users) = add_users {
        if !users.is_empty() {
            group_service::add_group_users(org_id, group_name, users).await?;
        }
    }

    // Remove users
    if let Some(users) = remove_users {
        if !users.is_empty() {
            group_service::remove_group_users(org_id, group_name, users).await?;
        }
    }

    // Add roles
    if let Some(roles) = add_roles {
        if !roles.is_empty() {
            group_service::add_group_roles(org_id, group_name, roles).await?;
        }
    }

    // Remove roles
    if let Some(roles) = remove_roles {
        if !roles.is_empty() {
            group_service::remove_group_roles(org_id, group_name, roles).await?;
        }
    }

    Ok(())
}

/// Get users in a group
pub async fn get_group_users(org_id: &str, group_name: &str) -> Result<Vec<String>> {
    let details = get_group_details(org_id, group_name).await?;
    Ok(details.users)
}

/// Get roles assigned to a group
pub async fn get_group_roles(org_id: &str, group_name: &str) -> Result<Vec<String>> {
    let details = get_group_details(org_id, group_name).await?;
    Ok(details.roles)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_signatures_exist() {
        // This test just verifies the function signatures are correct
        let _ = create_group;
        let _ = delete_group;
        let _ = get_all_groups;
        let _ = get_group_details;
        let _ = get_groups_for_org_user;
        let _ = update_group;
    }
}
