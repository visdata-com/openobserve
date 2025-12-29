// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Tuple operations service (compatible with o2_openfga::authorizer::authz)

use crate::Visdata;
use super::super::error::Result;
use super::super::model::schema;
use super::super::types::TupleKey;

/// Batch update tuples (add and/or delete)
///
/// Compatible with o2_openfga::authorizer::authz::update_tuples
pub async fn update_tuples(
    writes: Vec<TupleKey>,
    deletes: Vec<TupleKey>,
) -> Result<()> {
    if writes.is_empty() && deletes.is_empty() {
        return Ok(());
    }

    let visdata = Visdata::global();
    visdata.openfga().write(writes, deletes).await
}

/// Map UserRole to OpenFGA relation on org type
///
/// Based on store.yaml org type definition:
/// - admin: [user] and org_context
/// - editor: [user] and org_context
/// - viewer: [user] and org_context
/// - allowed_user: [user] and org_context (for User/ServiceAccount roles)
pub fn role_to_fga_relation(role: &str) -> &'static str {
    match role.to_lowercase().as_str() {
        "root" | "admin" => "admin",
        "editor" => "editor",
        "viewer" => "viewer",
        "user" | "serviceaccount" | "service_account" => "allowed_user",
        _ => "allowed_user",
    }
}

/// Add user to organization with a system role
///
/// Compatible with o2_openfga::authorizer::authz::get_add_user_to_org_tuples
///
/// Based on store.yaml org type definition:
/// - admin: [user] and org_context
/// - editor: [user] and org_context
/// - viewer: [user] and org_context
/// - allowed_user: [user] and org_context (for User/ServiceAccount roles)
///
/// This function adds TWO tuples:
/// 1. Role tuple: user:email -> admin/editor/viewer/allowed_user -> org:org_id
/// 2. Context tuple: user:email -> org_context -> org:org_id
pub fn get_add_user_to_org_tuples(
    org_id: &str,
    user_email: &str,
    role: &str,
    tuples: &mut Vec<TupleKey>,
) {
    let user = schema::user_type(user_email);
    let org = schema::org_type(org_id);

    // Map role to relation
    let relation = role_to_fga_relation(role);

    // Add role tuple
    tuples.push(TupleKey::new(&user, relation, &org));
    // Add org_context tuple (required for the intersection)
    tuples.push(TupleKey::new(&user, "org_context", &org));
}

/// Get tuple for assigning a custom role to a user
///
/// Compatible with o2_openfga::authorizer::authz::get_user_crole_tuple
///
/// Note: Uses "assigned" relation to match the OpenFGA model definition in store.yaml
pub fn get_user_crole_tuple(org_id: &str, role_name: &str, user_email: &str) -> TupleKey {
    let user = schema::user_type(user_email);
    let role = schema::role_type(org_id, role_name);

    TupleKey::new(&user, "assigned", &role)
}

/// Get the full role key for an organization and role name
///
/// Compatible with o2_openfga::authorizer::roles::get_role_key
pub fn get_role_key(org_id: &str, role_name: &str) -> String {
    schema::role_type(org_id, role_name)
}

/// Get tuples for removing a user from a custom role
///
/// Compatible with o2_openfga::authorizer::roles::get_user_crole_removal_tuples
///
/// Note: Uses "assigned" relation to match the OpenFGA model definition in store.yaml
pub fn get_user_crole_removal_tuples(
    user_email: &str,
    role_key: &str,
    tuples: &mut Vec<TupleKey>,
) {
    let user = schema::user_type(user_email);
    // Remove user from role assigned relation
    tuples.push(TupleKey::new(&user, "assigned", role_key));
}

/// Get tuples for organization creation
pub fn get_org_creation_tuples(org_id: &str, tuples: &mut Vec<TupleKey>) {
    // Create organization object
    let org = schema::org_type(org_id);

    // Organization is self-referential for member relation
    // This allows inheriting permissions from organization to resources
    tuples.push(TupleKey::new(&org, "member", &org));
}

/// Get tuple for resource ownership
pub fn get_ownership_tuple(
    org_id: &str,
    resource_type: &str,
    entity_id: &str,
    owner_email: &str,
) -> TupleKey {
    let user = schema::user_type(owner_email);
    let resource = schema::resource_object(org_id, resource_type, entity_id);

    TupleKey::new(&user, "owner", &resource)
}

/// Get tuple for resource parent (organization)
pub fn get_resource_parent_tuple(
    org_id: &str,
    resource_type: &str,
    entity_id: &str,
) -> TupleKey {
    let org = schema::org_type(org_id);
    let resource = schema::resource_object(org_id, resource_type, entity_id);

    TupleKey::new(&org, "parent", &resource)
}

/// Get tuple for organization-wide resource permission
/// This grants permission to all resources of a type in an org
///
/// Note: Uses "has" relation to match the OpenFGA model in store.yaml
pub fn get_org_resource_permission_tuple(
    org_id: &str,
    resource_type: &str,
    role_name: &str,
    permission: &str,
) -> TupleKey {
    let role = schema::role_type(org_id, role_name);
    let role_has = format!("{}#has", role);
    let resource = schema::resource_object_all(org_id, resource_type);

    // Map permission to relation (use ALLOW_* format to match store.yaml)
    let relation = match permission.to_lowercase().as_str() {
        "allowall" | "admin" => "ALLOW_ALL",
        "allowget" | "can_read" => "ALLOW_GET",
        "allowlist" | "can_list" => "ALLOW_LIST",
        "allowpost" | "can_create" => "ALLOW_POST",
        "allowput" | "can_update" => "ALLOW_PUT",
        "allowdelete" | "can_delete" => "ALLOW_DELETE",
        _ => "ALLOW_GET",
    };

    TupleKey::new(&role_has, relation, &resource)
}

/// Get tuple for adding user to a group
pub fn get_group_member_tuple(org_id: &str, group_name: &str, user_email: &str) -> TupleKey {
    let user = schema::user_type(user_email);
    let group = schema::group_type(org_id, group_name);

    TupleKey::new(&user, "member", &group)
}

/// Get tuple for assigning a role to a group
///
/// Note: Uses "grp_assigned" relation to match the OpenFGA model in store.yaml
pub fn get_group_role_tuple(org_id: &str, group_name: &str, role_name: &str) -> TupleKey {
    let group = schema::group_type(org_id, group_name);
    let role = schema::role_type(org_id, role_name);

    // group -> grp_assigned -> role (not group#member -> assignee)
    TupleKey::new(&group, "grp_assigned", &role)
}

/// Get tuple for service account creation
pub fn get_service_account_creation_tuple(org_id: &str, email: &str, tuples: &mut Vec<TupleKey>) {
    let user = schema::user_type(email);
    let org = schema::org_type(org_id);

    // Service accounts are members of the organization
    tuples.push(TupleKey::new(&user, "allowed_user", &org));
    // Add org_context for the intersection to work
    tuples.push(TupleKey::new(&user, "org_context", &org));
}

/// Get tuples for new user creation
///
/// Compatible with o2_openfga::authorizer::authz::get_new_user_creation_tuple
///
/// This function creates basic tuples for a new user to access the system.
/// It adds the user to the default organization with user role and org_context,
/// allowing the user to call organization API endpoints.
///
/// Note: This uses the default org from DexConfig. The caller should have already
/// created the organization before calling this function.
pub fn get_new_user_creation_tuple(user_email: &str, tuples: &mut Vec<TupleKey>) {
    // Get default org from visdata config
    let default_org = crate::Visdata::try_global()
        .map(|v| v.dex_config().default_org.clone())
        .unwrap_or_else(|| "default".to_string());

    let user = schema::user_type(user_email);
    let org = schema::org_type(&default_org);

    // Add user as allowed_user of the default org (basic user access)
    tuples.push(TupleKey::new(&user, "allowed_user", &org));
    // Add org_context for the intersection to work
    tuples.push(TupleKey::new(&user, "org_context", &org));
}

/// Get tuples for removing user's system role from org
///
/// Returns tuples to delete when user's role is removed from an org
pub fn get_delete_user_system_role_tuples(
    org_id: &str,
    user_email: &str,
    role: &str,
    tuples: &mut Vec<TupleKey>,
) {
    let user = schema::user_type(user_email);
    let org = schema::org_type(org_id);
    let relation = role_to_fga_relation(role);

    // Delete role tuple
    tuples.push(TupleKey::new(&user, relation, &org));
    // Delete org_context tuple
    tuples.push(TupleKey::new(&user, "org_context", &org));
}

/// Update user's system role in OpenFGA
///
/// When user's role changes (e.g., Admin -> Editor), we need to:
/// 1. Delete old role tuple
/// 2. Add new role tuple
/// (org_context tuple stays the same)
pub async fn update_user_role(
    org_id: &str,
    user_email: &str,
    old_role: &str,
    new_role: &str,
) -> Result<()> {
    let user = schema::user_type(user_email);
    let org = schema::org_type(org_id);

    let old_relation = role_to_fga_relation(old_role);
    let new_relation = role_to_fga_relation(new_role);

    // If the relation is the same, no need to update
    if old_relation == new_relation {
        return Ok(());
    }

    let mut writes = vec![];
    let mut deletes = vec![];

    // Delete old role tuple
    deletes.push(TupleKey::new(&user, old_relation, &org));
    // Add new role tuple
    writes.push(TupleKey::new(&user, new_relation, &org));

    // org_context stays, no need to update

    update_tuples(writes, deletes).await
}

/// Delete all user tuples from org (for user removal)
///
/// Removes all possible role relations for a user from an org
pub fn get_delete_all_user_from_org_tuples(
    org_id: &str,
    user_email: &str,
    tuples: &mut Vec<TupleKey>,
) {
    let user = schema::user_type(user_email);
    let org = schema::org_type(org_id);

    // Remove all possible role relations
    tuples.push(TupleKey::new(&user, "admin", &org));
    tuples.push(TupleKey::new(&user, "editor", &org));
    tuples.push(TupleKey::new(&user, "viewer", &org));
    tuples.push(TupleKey::new(&user, "allowed_user", &org));
    tuples.push(TupleKey::new(&user, "org_context", &org));
}

/// Delete user from organization tuples
pub fn get_delete_user_from_org_tuples(
    org_id: &str,
    user_email: &str,
    tuples: &mut Vec<TupleKey>,
) {
    let user = schema::user_type(user_email);
    let org = schema::org_type(org_id);

    // Remove all possible relations
    tuples.push(TupleKey::new(&user, "owner", &org));
    tuples.push(TupleKey::new(&user, "admin", &org));
    tuples.push(TupleKey::new(&user, "member", &org));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_add_user_to_org_tuples() {
        let mut tuples = Vec::new();
        get_add_user_to_org_tuples("default", "alice@example.com", "admin", &mut tuples);

        // Should add 2 tuples: role + org_context
        assert_eq!(tuples.len(), 2);
        // Role tuple
        assert_eq!(tuples[0].user, "user:alice@example.com");
        assert_eq!(tuples[0].relation, "admin");
        assert_eq!(tuples[0].object, "org:default");
        // Context tuple
        assert_eq!(tuples[1].user, "user:alice@example.com");
        assert_eq!(tuples[1].relation, "org_context");
        assert_eq!(tuples[1].object, "org:default");
    }

    #[test]
    fn test_get_user_crole_tuple() {
        let tuple = get_user_crole_tuple("default", "developer", "bob@example.com");

        assert_eq!(tuple.user, "user:bob@example.com");
        assert_eq!(tuple.relation, "assigned");
        assert_eq!(tuple.object, "role:default_developer");
    }

    #[test]
    fn test_get_group_member_tuple() {
        let tuple = get_group_member_tuple("default", "developers", "alice@example.com");

        assert_eq!(tuple.user, "user:alice@example.com");
        assert_eq!(tuple.relation, "member");
        assert_eq!(tuple.object, "group:default_developers");
    }

    #[test]
    fn test_role_to_fga_relation() {
        assert_eq!(role_to_fga_relation("Admin"), "admin");
        assert_eq!(role_to_fga_relation("admin"), "admin");
        assert_eq!(role_to_fga_relation("Root"), "admin");
        assert_eq!(role_to_fga_relation("Editor"), "editor");
        assert_eq!(role_to_fga_relation("Viewer"), "viewer");
        assert_eq!(role_to_fga_relation("User"), "allowed_user");
        assert_eq!(role_to_fga_relation("ServiceAccount"), "allowed_user");
    }

    #[test]
    fn test_get_add_user_to_org_tuples_with_editor() {
        let mut tuples = Vec::new();
        get_add_user_to_org_tuples("myorg", "bob@example.com", "Editor", &mut tuples);

        assert_eq!(tuples.len(), 2);
        assert_eq!(tuples[0].relation, "editor");
        assert_eq!(tuples[1].relation, "org_context");
    }

    #[test]
    fn test_get_add_user_to_org_tuples_with_viewer() {
        let mut tuples = Vec::new();
        get_add_user_to_org_tuples("myorg", "carol@example.com", "Viewer", &mut tuples);

        assert_eq!(tuples.len(), 2);
        assert_eq!(tuples[0].relation, "viewer");
        assert_eq!(tuples[1].relation, "org_context");
    }

    #[test]
    fn test_get_delete_all_user_from_org_tuples() {
        let mut tuples = Vec::new();
        get_delete_all_user_from_org_tuples("default", "alice@example.com", &mut tuples);

        assert_eq!(tuples.len(), 5);
        // Should contain all possible role relations
        let relations: Vec<&str> = tuples.iter().map(|t| t.relation.as_str()).collect();
        assert!(relations.contains(&"admin"));
        assert!(relations.contains(&"editor"));
        assert!(relations.contains(&"viewer"));
        assert!(relations.contains(&"allowed_user"));
        assert!(relations.contains(&"org_context"));
    }
}
