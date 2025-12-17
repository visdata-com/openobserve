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

/// Add user to organization with a role
///
/// Compatible with o2_openfga::authorizer::authz::get_add_user_to_org_tuples
pub fn get_add_user_to_org_tuples(
    org_id: &str,
    user_email: &str,
    role: &str,
    tuples: &mut Vec<TupleKey>,
) {
    let user = schema::user_type(user_email);
    let org = schema::org_type(org_id);

    // Map role to relation
    let relation = match role.to_lowercase().as_str() {
        "root" | "admin" => "admin",
        "member" | "user" | "viewer" => "member",
        _ => "member", // Default to member
    };

    tuples.push(TupleKey::new(&user, relation, &org));
}

/// Get tuple for assigning a custom role to a user
///
/// Compatible with o2_openfga::authorizer::authz::get_user_crole_tuple
pub fn get_user_crole_tuple(org_id: &str, role_name: &str, user_email: &str) -> TupleKey {
    let user = schema::user_type(user_email);
    let role = schema::role_type(org_id, role_name);

    TupleKey::new(&user, "assignee", &role)
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
pub fn get_org_resource_permission_tuple(
    org_id: &str,
    resource_type: &str,
    role_name: &str,
    permission: &str,
) -> TupleKey {
    let role = schema::role_type(org_id, role_name);
    let role_assignee = format!("{}#assignee", role);
    let resource = schema::resource_object_all(org_id, resource_type);

    // Map permission to relation
    let relation = match permission.to_lowercase().as_str() {
        "allowall" | "admin" => "admin",
        "allowget" | "can_read" => "can_read",
        "allowlist" | "can_list" => "can_list",
        "allowpost" | "can_create" => "can_create",
        "allowput" | "can_update" => "can_update",
        "allowdelete" | "can_delete" => "can_delete",
        _ => "can_read",
    };

    TupleKey::new(&role_assignee, relation, &resource)
}

/// Get tuple for adding user to a group
pub fn get_group_member_tuple(org_id: &str, group_name: &str, user_email: &str) -> TupleKey {
    let user = schema::user_type(user_email);
    let group = schema::group_type(org_id, group_name);

    TupleKey::new(&user, "member", &group)
}

/// Get tuple for assigning a role to a group
pub fn get_group_role_tuple(org_id: &str, group_name: &str, role_name: &str) -> TupleKey {
    let group = schema::group_type(org_id, group_name);
    let group_member = format!("{}#member", group);
    let role = schema::role_type(org_id, role_name);

    TupleKey::new(&group_member, "assignee", &role)
}

/// Get tuple for service account creation
pub fn get_service_account_creation_tuple(org_id: &str, email: &str, tuples: &mut Vec<TupleKey>) {
    let user = schema::user_type(email);
    let org = schema::org_type(org_id);

    // Service accounts are members of the organization
    tuples.push(TupleKey::new(&user, "member", &org));
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

        assert_eq!(tuples.len(), 1);
        assert_eq!(tuples[0].user, "user:alice@example.com");
        assert_eq!(tuples[0].relation, "admin");
        assert_eq!(tuples[0].object, "organization:default");
    }

    #[test]
    fn test_get_user_crole_tuple() {
        let tuple = get_user_crole_tuple("default", "developer", "bob@example.com");

        assert_eq!(tuple.user, "user:bob@example.com");
        assert_eq!(tuple.relation, "assignee");
        assert_eq!(tuple.object, "role:default_developer");
    }

    #[test]
    fn test_get_group_member_tuple() {
        let tuple = get_group_member_tuple("default", "developers", "alice@example.com");

        assert_eq!(tuple.user, "user:alice@example.com");
        assert_eq!(tuple.relation, "member");
        assert_eq!(tuple.object, "group:default_developers");
    }
}
