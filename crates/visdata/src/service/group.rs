// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Group service - business logic for group management

use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QuerySelect,
};
use svix_ksuid::KsuidLike;
use std::collections::HashSet;

use crate::entity::{vd_group_roles, vd_group_users, vd_groups, vd_roles};
use crate::error::{Error, Result};
use crate::meta::GroupResponse;
use crate::Visdata;

/// Generate a unique ID using KSUID
fn generate_id() -> String {
    svix_ksuid::Ksuid::new(None, None).to_string()
}

/// Get current timestamp in microseconds
fn now_micros() -> i64 {
    chrono::Utc::now().timestamp_micros()
}

/// Create a new group
pub async fn create_group(
    org_id: &str,
    name: &str,
    display_name: Option<String>,
    description: Option<String>,
) -> Result<vd_groups::Model> {
    let db = Visdata::global().db();

    // Check if group already exists
    let existing = vd_groups::Entity::find()
        .filter(vd_groups::Column::OrgId.eq(org_id))
        .filter(vd_groups::Column::Name.eq(name))
        .one(db)
        .await?;

    if existing.is_some() {
        return Err(Error::DuplicateEntry(format!(
            "Group '{}' already exists in organization",
            name
        )));
    }

    let now = now_micros();
    let group = vd_groups::ActiveModel {
        id: ActiveValue::Set(generate_id()),
        org_id: ActiveValue::Set(org_id.to_string()),
        name: ActiveValue::Set(name.to_string()),
        display_name: ActiveValue::Set(display_name),
        description: ActiveValue::Set(description),
        external_id: ActiveValue::Set(None),
        created_at: ActiveValue::Set(now),
        updated_at: ActiveValue::Set(now),
    };

    let group = group.insert(db).await?;
    Ok(group)
}

/// Delete a group
pub async fn delete_group(org_id: &str, group_name: &str) -> Result<()> {
    let db = Visdata::global().db();
    let rbac = Visdata::global().rbac();

    // Find the group
    let group = vd_groups::Entity::find()
        .filter(vd_groups::Column::OrgId.eq(org_id))
        .filter(vd_groups::Column::Name.eq(group_name))
        .one(db)
        .await?
        .ok_or_else(|| Error::GroupNotFound(group_name.to_string()))?;

    let group_id = group.id.clone();

    // Delete group (cascades to roles and user assignments)
    vd_groups::Entity::delete_by_id(&group_id).exec(db).await?;

    // Invalidate cache
    rbac.invalidate_group_cache(org_id, &group_id);

    Ok(())
}

/// Add a role to a group
pub async fn add_role(org_id: &str, group_id: &str, role_name: &str) -> Result<()> {
    let db = Visdata::global().db();
    let rbac = Visdata::global().rbac();

    // Find the role by name
    let role = vd_roles::Entity::find()
        .filter(vd_roles::Column::OrgId.eq(org_id))
        .filter(vd_roles::Column::Name.eq(role_name))
        .one(db)
        .await?
        .ok_or_else(|| Error::RoleNotFound(role_name.to_string()))?;

    // Check if group already has this role
    let existing = vd_group_roles::Entity::find()
        .filter(vd_group_roles::Column::GroupId.eq(group_id))
        .filter(vd_group_roles::Column::RoleId.eq(&role.id))
        .one(db)
        .await?;

    if existing.is_some() {
        // Already has role, no-op
        return Ok(());
    }

    let assignment = vd_group_roles::ActiveModel {
        id: ActiveValue::Set(generate_id()),
        group_id: ActiveValue::Set(group_id.to_string()),
        org_id: ActiveValue::Set(org_id.to_string()),
        role_id: ActiveValue::Set(role.id),
        created_at: ActiveValue::Set(now_micros()),
    };

    assignment.insert(db).await?;

    // Invalidate cache
    rbac.invalidate_group_cache(org_id, group_id);

    Ok(())
}

/// Remove a role from a group
pub async fn remove_role(org_id: &str, group_id: &str, role_name: &str) -> Result<()> {
    let db = Visdata::global().db();
    let rbac = Visdata::global().rbac();

    // Find the role by name
    let role = vd_roles::Entity::find()
        .filter(vd_roles::Column::OrgId.eq(org_id))
        .filter(vd_roles::Column::Name.eq(role_name))
        .one(db)
        .await?;

    if let Some(role) = role {
        vd_group_roles::Entity::delete_many()
            .filter(vd_group_roles::Column::GroupId.eq(group_id))
            .filter(vd_group_roles::Column::RoleId.eq(&role.id))
            .exec(db)
            .await?;

        // Invalidate cache
        rbac.invalidate_group_cache(org_id, group_id);
    }

    Ok(())
}

/// Add a user to a group
pub async fn add_user(org_id: &str, group_id: &str, user_email: &str) -> Result<()> {
    let db = Visdata::global().db();
    let rbac = Visdata::global().rbac();

    // Check if user already in group
    let existing = vd_group_users::Entity::find()
        .filter(vd_group_users::Column::GroupId.eq(group_id))
        .filter(vd_group_users::Column::UserEmail.eq(user_email))
        .one(db)
        .await?;

    if existing.is_some() {
        // Already in group, no-op
        return Ok(());
    }

    let assignment = vd_group_users::ActiveModel {
        id: ActiveValue::Set(generate_id()),
        group_id: ActiveValue::Set(group_id.to_string()),
        org_id: ActiveValue::Set(org_id.to_string()),
        user_email: ActiveValue::Set(user_email.to_string()),
        created_at: ActiveValue::Set(now_micros()),
    };

    assignment.insert(db).await?;

    // Invalidate cache
    rbac.invalidate_user_cache(org_id, user_email);

    Ok(())
}

/// Remove a user from a group
pub async fn remove_user(org_id: &str, group_id: &str, user_email: &str) -> Result<()> {
    let db = Visdata::global().db();
    let rbac = Visdata::global().rbac();

    vd_group_users::Entity::delete_many()
        .filter(vd_group_users::Column::GroupId.eq(group_id))
        .filter(vd_group_users::Column::UserEmail.eq(user_email))
        .exec(db)
        .await?;

    // Invalidate cache
    rbac.invalidate_user_cache(org_id, user_email);

    Ok(())
}

/// Update group roles and users
pub async fn update_group(
    org_id: &str,
    group_name: &str,
    add_roles: Option<HashSet<String>>,
    remove_roles: Option<HashSet<String>>,
    add_users: Option<HashSet<String>>,
    remove_users: Option<HashSet<String>>,
) -> Result<()> {
    let db = Visdata::global().db();

    // Find the group
    let group = vd_groups::Entity::find()
        .filter(vd_groups::Column::OrgId.eq(org_id))
        .filter(vd_groups::Column::Name.eq(group_name))
        .one(db)
        .await?
        .ok_or_else(|| Error::GroupNotFound(group_name.to_string()))?;

    let group_id = &group.id;

    // Handle role additions
    if let Some(roles) = add_roles {
        for role_name in roles {
            add_role(org_id, group_id, &role_name).await?;
        }
    }

    // Handle role removals
    if let Some(roles) = remove_roles {
        for role_name in roles {
            remove_role(org_id, group_id, &role_name).await?;
        }
    }

    // Handle user additions
    if let Some(users) = add_users {
        for user in users {
            add_user(org_id, group_id, &user).await?;
        }
    }

    // Handle user removals
    if let Some(users) = remove_users {
        for user in users {
            remove_user(org_id, group_id, &user).await?;
        }
    }

    // Update the group's updated_at timestamp
    let mut group: vd_groups::ActiveModel = group.into();
    group.updated_at = ActiveValue::Set(now_micros());
    group.update(db).await?;

    Ok(())
}

/// Get group by name with full details
pub async fn get_group(org_id: &str, group_name: &str) -> Result<GroupResponse> {
    let db = Visdata::global().db();

    let group = vd_groups::Entity::find()
        .filter(vd_groups::Column::OrgId.eq(org_id))
        .filter(vd_groups::Column::Name.eq(group_name))
        .one(db)
        .await?
        .ok_or_else(|| Error::GroupNotFound(group_name.to_string()))?;

    group_to_response(db, group).await
}

/// List all groups in an organization
pub async fn list_groups(org_id: &str) -> Result<Vec<GroupResponse>> {
    let db = Visdata::global().db();

    let groups = vd_groups::Entity::find()
        .filter(vd_groups::Column::OrgId.eq(org_id))
        .all(db)
        .await?;

    let mut responses = Vec::with_capacity(groups.len());
    for group in groups {
        responses.push(group_to_response(db, group).await?);
    }

    Ok(responses)
}

/// Convert group model to response with roles and users
async fn group_to_response(db: &DatabaseConnection, group: vd_groups::Model) -> Result<GroupResponse> {
    // Get role names for this group
    let role_ids: Vec<String> = vd_group_roles::Entity::find()
        .filter(vd_group_roles::Column::GroupId.eq(&group.id))
        .select_only()
        .column(vd_group_roles::Column::RoleId)
        .into_tuple()
        .all(db)
        .await?;

    let roles: Vec<String> = if role_ids.is_empty() {
        vec![]
    } else {
        vd_roles::Entity::find()
            .filter(vd_roles::Column::Id.is_in(role_ids))
            .select_only()
            .column(vd_roles::Column::Name)
            .into_tuple()
            .all(db)
            .await?
    };

    // Get users in this group
    let users: Vec<String> = vd_group_users::Entity::find()
        .filter(vd_group_users::Column::GroupId.eq(&group.id))
        .select_only()
        .column(vd_group_users::Column::UserEmail)
        .into_tuple()
        .all(db)
        .await?;

    Ok(GroupResponse {
        id: group.id,
        name: group.name,
        display_name: group.display_name,
        description: group.description,
        roles,
        users,
        created_at: group.created_at,
        updated_at: group.updated_at,
    })
}
