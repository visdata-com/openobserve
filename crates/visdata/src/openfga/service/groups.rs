// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Group management service

use std::collections::HashSet;

use crate::Visdata;
use crate::common::generate_id;
use super::super::error::{Error, Result};
use super::super::model::schema;
use super::super::types::{TupleKey, TupleKeyFilter, GroupResponse};
use super::tuples;

/// Create a new group
pub async fn create_group(
    org_id: &str,
    name: &str,
    _display_name: Option<&str>,
    _description: Option<&str>,
) -> Result<String> {
    // Check if group already exists
    let existing = list_groups(org_id).await?;
    if existing.iter().any(|g| g.eq_ignore_ascii_case(name)) {
        return Err(Error::DuplicateEntry(format!(
            "Group '{}' already exists",
            name
        )));
    }

    // Generate ID for the group
    let id = generate_id();

    // Create the group by writing an owningOrg tuple
    // This marks the group as existing in the organization
    let group_object = schema::group_type(org_id, name);
    let org_object = format!("org:{}", org_id);

    let tuple = TupleKey {
        user: org_object,
        relation: "owningOrg".to_string(),
        object: group_object,
    };

    tuples::update_tuples(vec![tuple], vec![]).await?;

    tracing::info!("[RBAC] Created group: {} (id: {}) in org {}", name, id, org_id);

    Ok(id)
}

/// List all groups in an organization
pub async fn list_groups(org_id: &str) -> Result<Vec<String>> {
    let visdata = Visdata::global();

    // Read all tuples without filter (OpenFGA requires object type in filter)
    let all_tuples = visdata.openfga().read(None).await?;

    // Extract unique group names for this org
    let prefix = format!("group:{}_", org_id);
    let org_user = format!("org:{}", org_id);
    let mut groups: HashSet<String> = HashSet::new();

    for tuple in all_tuples {
        // Find groups by owningOrg relation (created groups)
        if tuple.key.relation == "owningOrg"
            && tuple.key.user == org_user
            && tuple.key.object.starts_with(&prefix)
        {
            if let Some(group_name) = tuple.key.object.strip_prefix(&prefix) {
                groups.insert(group_name.to_string());
            }
        }
        // Also include groups that have members (for backward compatibility)
        if tuple.key.relation == "member" && tuple.key.object.starts_with(&prefix) {
            if let Some(group_name) = tuple.key.object.strip_prefix(&prefix) {
                groups.insert(group_name.to_string());
            }
        }
    }

    let mut result: Vec<String> = groups.into_iter().collect();
    result.sort();

    Ok(result)
}

/// Get group details
pub async fn get_group(org_id: &str, group_name: &str) -> Result<GroupResponse> {
    let visdata = Visdata::global();
    let group_object = schema::group_type(org_id, group_name);

    // Get all members of the group
    let member_filter = TupleKeyFilter {
        user: None,
        relation: Some("member".to_string()),
        object: Some(group_object.clone()),
    };

    let member_tuples = visdata.openfga().read(Some(member_filter)).await?;

    let users: Vec<String> = member_tuples
        .into_iter()
        .filter_map(|t| t.key.user.strip_prefix("user:").map(|s| s.to_string()))
        .collect();

    // Get all roles assigned to the group
    // Note: Uses "grp_assigned" relation to match the OpenFGA model in store.yaml
    let group_object_ref = group_object.clone();
    let role_filter = TupleKeyFilter {
        user: Some(group_object_ref),
        relation: Some("grp_assigned".to_string()),
        object: None,
    };

    let role_tuples = visdata.openfga().read(Some(role_filter)).await?;

    let role_prefix = format!("role:{}_", org_id);
    let roles: Vec<String> = role_tuples
        .into_iter()
        .filter_map(|t| {
            t.key.object.strip_prefix(&role_prefix).map(|s| s.to_string())
        })
        .collect();

    // Check if group exists (has any members or roles)
    if users.is_empty() && roles.is_empty() {
        // Group might not exist - check by looking for any tuple mentioning it
        let any_filter = TupleKeyFilter {
            user: None,
            relation: None,
            object: Some(group_object),
        };

        let any_tuples = visdata.openfga().read(Some(any_filter)).await?;

        if any_tuples.is_empty() {
            return Err(Error::GroupNotFound(group_name.to_string()));
        }
    }

    let now = chrono::Utc::now().timestamp_micros();

    Ok(GroupResponse {
        id: generate_id(), // Generate consistent ID
        name: group_name.to_string(),
        display_name: Some(capitalize(group_name)),
        description: None,
        roles,
        users,
        created_at: now,
        updated_at: now,
    })
}

/// Delete a group
pub async fn delete_group(org_id: &str, group_name: &str) -> Result<()> {
    let visdata = Visdata::global();
    let group_object = schema::group_type(org_id, group_name);

    // Find all tuples related to this group
    let member_filter = TupleKeyFilter {
        user: None,
        relation: None,
        object: Some(group_object.clone()),
    };

    let member_tuples = visdata.openfga().read(Some(member_filter)).await?;

    // Also find role assignment tuples where group is the user
    let group_member = format!("{}#member", group_object);
    let role_filter = TupleKeyFilter {
        user: Some(group_member),
        relation: None,
        object: None,
    };

    let role_tuples = visdata.openfga().read(Some(role_filter)).await?;

    // Delete all related tuples
    let mut deletes: Vec<TupleKey> = Vec::new();
    deletes.extend(member_tuples.into_iter().map(|t| t.key));
    deletes.extend(role_tuples.into_iter().map(|t| t.key));

    if !deletes.is_empty() {
        tuples::update_tuples(vec![], deletes).await?;
    }

    tracing::info!("[RBAC] Deleted group: {} in org {}", group_name, org_id);

    Ok(())
}

/// Add users to a group
pub async fn add_group_users(
    org_id: &str,
    group_name: &str,
    users: &HashSet<String>,
) -> Result<()> {
    if users.is_empty() {
        return Ok(());
    }

    let writes: Vec<TupleKey> = users
        .iter()
        .map(|email| tuples::get_group_member_tuple(org_id, group_name, email))
        .collect();

    tuples::update_tuples(writes, vec![]).await
}

/// Remove users from a group
pub async fn remove_group_users(
    org_id: &str,
    group_name: &str,
    users: &HashSet<String>,
) -> Result<()> {
    if users.is_empty() {
        return Ok(());
    }

    let deletes: Vec<TupleKey> = users
        .iter()
        .map(|email| tuples::get_group_member_tuple(org_id, group_name, email))
        .collect();

    tuples::update_tuples(vec![], deletes).await
}

/// Add roles to a group
pub async fn add_group_roles(
    org_id: &str,
    group_name: &str,
    roles: &HashSet<String>,
) -> Result<()> {
    if roles.is_empty() {
        return Ok(());
    }

    let writes: Vec<TupleKey> = roles
        .iter()
        .map(|role_name| tuples::get_group_role_tuple(org_id, group_name, role_name))
        .collect();

    tuples::update_tuples(writes, vec![]).await
}

/// Remove roles from a group
pub async fn remove_group_roles(
    org_id: &str,
    group_name: &str,
    roles: &HashSet<String>,
) -> Result<()> {
    if roles.is_empty() {
        return Ok(());
    }

    let deletes: Vec<TupleKey> = roles
        .iter()
        .map(|role_name| tuples::get_group_role_tuple(org_id, group_name, role_name))
        .collect();

    tuples::update_tuples(vec![], deletes).await
}

/// Get all groups a user belongs to
pub async fn get_user_groups(org_id: &str, user_email: &str) -> Result<Vec<String>> {
    let visdata = Visdata::global();
    let user = schema::user_type(user_email);

    // Find all group memberships for this user
    let filter = TupleKeyFilter {
        user: Some(user),
        relation: Some("member".to_string()),
        object: None,
    };

    let tuples = visdata.openfga().read(Some(filter)).await?;

    // Extract group names for this org
    let prefix = format!("group:{}_", org_id);
    let groups: Vec<String> = tuples
        .into_iter()
        .filter_map(|t| {
            t.key.object.strip_prefix(&prefix).map(|s| s.to_string())
        })
        .collect();

    Ok(groups)
}

/// Get all roles assigned to a user (both direct and via groups)
pub async fn get_user_roles(org_id: &str, user_email: &str) -> Result<Vec<String>> {
    let visdata = Visdata::global();
    let user = schema::user_type(user_email);

    // Get directly assigned roles
    // Note: Uses "assigned" to match the OpenFGA model in store.yaml
    let direct_filter = TupleKeyFilter {
        user: Some(user.clone()),
        relation: Some("assigned".to_string()),
        object: None,
    };

    let direct_tuples = visdata.openfga().read(Some(direct_filter)).await?;

    let role_prefix = format!("role:{}_", org_id);
    let mut roles: HashSet<String> = direct_tuples
        .into_iter()
        .filter_map(|t| {
            t.key.object.strip_prefix(&role_prefix).map(|s| s.to_string())
        })
        .collect();

    // Get roles from group memberships
    let groups = get_user_groups(org_id, user_email).await?;

    for group_name in groups {
        let group_object = schema::group_type(org_id, &group_name);

        // Note: Uses "grp_assigned" to match the OpenFGA model in store.yaml
        let group_role_filter = TupleKeyFilter {
            user: Some(group_object),
            relation: Some("grp_assigned".to_string()),
            object: None,
        };

        let group_role_tuples = visdata.openfga().read(Some(group_role_filter)).await?;

        for tuple in group_role_tuples {
            if let Some(role_name) = tuple.key.object.strip_prefix(&role_prefix) {
                roles.insert(role_name.to_string());
            }
        }
    }

    let mut result: Vec<String> = roles.into_iter().collect();
    result.sort();

    Ok(result)
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
        assert_eq!(capitalize("developers"), "Developers");
        assert_eq!(capitalize(""), "");
    }
}
