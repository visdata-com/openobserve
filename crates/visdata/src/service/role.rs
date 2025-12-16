// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Role service - business logic for role management

use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, QueryFilter,
};
use svix_ksuid::KsuidLike;
use std::collections::HashSet;

use crate::entity::{vd_role_permissions, vd_role_users, vd_roles};
use crate::error::{Error, Result};
use crate::meta::{PermissionEntry, RoleResponse};
use crate::Visdata;

/// Generate a unique ID using KSUID
fn generate_id() -> String {
    svix_ksuid::Ksuid::new(None, None).to_string()
}

/// Get current timestamp in microseconds
fn now_micros() -> i64 {
    chrono::Utc::now().timestamp_micros()
}

/// Create a new role
pub async fn create_role(org_id: &str, name: &str) -> Result<vd_roles::Model> {
    let db = Visdata::global().db();

    // Check if role already exists
    let existing = vd_roles::Entity::find()
        .filter(vd_roles::Column::OrgId.eq(org_id))
        .filter(vd_roles::Column::Name.eq(name))
        .one(db)
        .await?;

    if existing.is_some() {
        return Err(Error::DuplicateEntry(format!(
            "Role '{}' already exists in organization",
            name
        )));
    }

    let now = now_micros();
    let role = vd_roles::ActiveModel {
        id: ActiveValue::Set(generate_id()),
        org_id: ActiveValue::Set(org_id.to_string()),
        name: ActiveValue::Set(name.to_string()),
        display_name: ActiveValue::Set(None),
        description: ActiveValue::Set(None),
        is_system: ActiveValue::Set(false),
        created_at: ActiveValue::Set(now),
        updated_at: ActiveValue::Set(now),
    };

    let role = role.insert(db).await?;
    Ok(role)
}

/// Delete a role (accepts role name or role ID)
pub async fn delete_role(org_id: &str, role_name_or_id: &str) -> Result<()> {
    let db = Visdata::global().db();
    let rbac = Visdata::global().rbac();

    // First try to find role by name (most common case from frontend)
    let role = rbac.get_role_by_name(org_id, role_name_or_id).await?;

    let role = match role {
        Some(r) => r,
        None => {
            // Try to find by ID as fallback
            rbac.get_role(org_id, role_name_or_id)
                .await?
                .ok_or_else(|| Error::RoleNotFound(role_name_or_id.to_string()))?
        }
    };

    // Don't allow deleting system roles
    if role.is_system {
        return Err(Error::PermissionDenied(
            "Cannot delete system role".to_string(),
        ));
    }

    let role_id = role.id.clone();

    // Delete role (cascades to permissions and user assignments)
    vd_roles::Entity::delete_by_id(&role_id).exec(db).await?;

    // Invalidate cache
    rbac.invalidate_role_cache(org_id, &role_id);

    Ok(())
}

/// Add a permission to a role
pub async fn add_permission(
    org_id: &str,
    role_id: &str,
    object: &str,
    permission: &str,
) -> Result<()> {
    let db = Visdata::global().db();
    let rbac = Visdata::global().rbac();

    // Validate permission
    let _: crate::rbac::Permission = permission.parse()?;

    // Check if role exists
    let _ = vd_roles::Entity::find_by_id(role_id)
        .filter(vd_roles::Column::OrgId.eq(org_id))
        .one(db)
        .await?
        .ok_or_else(|| Error::RoleNotFound(role_id.to_string()))?;

    // Check if permission already exists
    let existing = vd_role_permissions::Entity::find()
        .filter(vd_role_permissions::Column::RoleId.eq(role_id))
        .filter(vd_role_permissions::Column::Object.eq(object))
        .filter(vd_role_permissions::Column::Permission.eq(permission))
        .one(db)
        .await?;

    if existing.is_some() {
        // Permission already exists, no-op
        return Ok(());
    }

    let perm = vd_role_permissions::ActiveModel {
        id: ActiveValue::Set(generate_id()),
        role_id: ActiveValue::Set(role_id.to_string()),
        org_id: ActiveValue::Set(org_id.to_string()),
        object: ActiveValue::Set(object.to_string()),
        permission: ActiveValue::Set(permission.to_string()),
        created_at: ActiveValue::Set(now_micros()),
    };

    perm.insert(db).await?;

    // Invalidate cache
    rbac.invalidate_role_cache(org_id, role_id);

    Ok(())
}

/// Remove a permission from a role
pub async fn remove_permission(
    org_id: &str,
    role_id: &str,
    object: &str,
    permission: &str,
) -> Result<()> {
    let db = Visdata::global().db();
    let rbac = Visdata::global().rbac();

    // Delete the permission
    vd_role_permissions::Entity::delete_many()
        .filter(vd_role_permissions::Column::RoleId.eq(role_id))
        .filter(vd_role_permissions::Column::OrgId.eq(org_id))
        .filter(vd_role_permissions::Column::Object.eq(object))
        .filter(vd_role_permissions::Column::Permission.eq(permission))
        .exec(db)
        .await?;

    // Invalidate cache
    rbac.invalidate_role_cache(org_id, role_id);

    Ok(())
}

/// Add a user to a role (accepts role name or role ID)
pub async fn add_user(org_id: &str, role_name_or_id: &str, user_email: &str) -> Result<()> {
    let db = Visdata::global().db();
    let rbac = Visdata::global().rbac();

    // First try to find role by name (most common case)
    let role = rbac.get_role_by_name(org_id, role_name_or_id).await?;

    let role_id = match role {
        Some(r) => r.id,
        None => {
            // Try to find by ID as fallback
            rbac.get_role(org_id, role_name_or_id)
                .await?
                .ok_or_else(|| Error::RoleNotFound(role_name_or_id.to_string()))?
                .id
        }
    };

    // Check if user already has this role
    let existing = vd_role_users::Entity::find()
        .filter(vd_role_users::Column::RoleId.eq(&role_id))
        .filter(vd_role_users::Column::OrgId.eq(org_id))
        .filter(vd_role_users::Column::UserEmail.eq(user_email))
        .one(db)
        .await?;

    if existing.is_some() {
        // User already has role, no-op
        return Ok(());
    }

    let assignment = vd_role_users::ActiveModel {
        id: ActiveValue::Set(generate_id()),
        role_id: ActiveValue::Set(role_id.to_string()),
        org_id: ActiveValue::Set(org_id.to_string()),
        user_email: ActiveValue::Set(user_email.to_string()),
        created_at: ActiveValue::Set(now_micros()),
    };

    assignment.insert(db).await?;

    // Invalidate cache
    rbac.invalidate_user_cache(org_id, user_email);

    Ok(())
}

/// Remove a user from a role (accepts role name or role ID)
pub async fn remove_user(org_id: &str, role_name_or_id: &str, user_email: &str) -> Result<()> {
    let db = Visdata::global().db();
    let rbac = Visdata::global().rbac();

    // First try to find role by name (most common case)
    let role = rbac.get_role_by_name(org_id, role_name_or_id).await?;

    let role_id = match role {
        Some(r) => r.id,
        None => {
            // Try to find by ID as fallback
            match rbac.get_role(org_id, role_name_or_id).await? {
                Some(r) => r.id,
                None => {
                    // Role doesn't exist, nothing to remove
                    return Ok(());
                }
            }
        }
    };

    vd_role_users::Entity::delete_many()
        .filter(vd_role_users::Column::RoleId.eq(&role_id))
        .filter(vd_role_users::Column::OrgId.eq(org_id))
        .filter(vd_role_users::Column::UserEmail.eq(user_email))
        .exec(db)
        .await?;

    // Invalidate cache
    rbac.invalidate_user_cache(org_id, user_email);

    Ok(())
}

/// Update role permissions and users (accepts role name or role ID)
pub async fn update_role(
    org_id: &str,
    role_name_or_id: &str,
    add_perms: Option<Vec<PermissionEntry>>,
    remove_perms: Option<Vec<PermissionEntry>>,
    add_users: Option<HashSet<String>>,
    remove_users: Option<HashSet<String>>,
) -> Result<()> {
    // First try to find role by name (most common case from frontend)
    let role = Visdata::global()
        .rbac()
        .get_role_by_name(org_id, role_name_or_id)
        .await?;

    let role_id = match role {
        Some(r) => r.id,
        None => {
            // Try to find by ID as fallback
            Visdata::global()
                .rbac()
                .get_role(org_id, role_name_or_id)
                .await?
                .ok_or_else(|| Error::RoleNotFound(role_name_or_id.to_string()))?
                .id
        }
    };

    // Handle permission additions
    if let Some(perms) = add_perms {
        for perm in perms {
            add_permission(org_id, &role_id, &perm.object, &perm.permission).await?;
        }
    }

    // Handle permission removals
    if let Some(perms) = remove_perms {
        for perm in perms {
            remove_permission(org_id, &role_id, &perm.object, &perm.permission).await?;
        }
    }

    // Handle user additions
    if let Some(users) = add_users {
        for user in users {
            add_user(org_id, &role_id, &user).await?;
        }
    }

    // Handle user removals
    if let Some(users) = remove_users {
        for user in users {
            remove_user(org_id, &role_id, &user).await?;
        }
    }

    // Update the role's updated_at timestamp
    let db = Visdata::global().db();
    let role_model = vd_roles::Entity::find_by_id(&role_id)
        .one(db)
        .await?
        .ok_or_else(|| Error::RoleNotFound(role_id.to_string()))?;

    let mut role_active: vd_roles::ActiveModel = role_model.into();
    role_active.updated_at = ActiveValue::Set(now_micros());
    role_active.update(db).await?;

    Ok(())
}

/// Get role by ID with response format
pub async fn get_role(org_id: &str, role_id: &str) -> Result<RoleResponse> {
    let role = Visdata::global()
        .rbac()
        .get_role(org_id, role_id)
        .await?
        .ok_or_else(|| Error::RoleNotFound(role_id.to_string()))?;

    Ok(role_to_response(role))
}

/// Get role by name with response format
pub async fn get_role_by_name(org_id: &str, name: &str) -> Result<RoleResponse> {
    let role = Visdata::global()
        .rbac()
        .get_role_by_name(org_id, name)
        .await?
        .ok_or_else(|| Error::RoleNotFound(name.to_string()))?;

    Ok(role_to_response(role))
}

/// List all roles in an organization
pub async fn list_roles(org_id: &str) -> Result<Vec<RoleResponse>> {
    let roles = Visdata::global().rbac().list_roles(org_id).await?;
    Ok(roles.into_iter().map(role_to_response).collect())
}

/// Get permissions for a role (accepts role name or role ID)
pub async fn get_role_permissions(
    org_id: &str,
    role_name_or_id: &str,
    resource_filter: Option<&str>,
) -> Result<Vec<PermissionEntry>> {
    // First try to find role by name (most common case from frontend)
    let role = Visdata::global()
        .rbac()
        .get_role_by_name(org_id, role_name_or_id)
        .await?;

    let role_id = match role {
        Some(r) => r.id,
        None => {
            // Try to find by ID as fallback
            Visdata::global()
                .rbac()
                .get_role(org_id, role_name_or_id)
                .await?
                .ok_or_else(|| Error::RoleNotFound(role_name_or_id.to_string()))?
                .id
        }
    };

    let permissions = Visdata::global()
        .rbac()
        .get_role_permissions_list(org_id, &role_id)
        .await?;

    let mut result: Vec<PermissionEntry> = permissions
        .into_iter()
        .filter(|p| {
            if let Some(resource) = resource_filter {
                p.object.starts_with(&format!("{}:", resource))
            } else {
                true
            }
        })
        .map(|p| PermissionEntry {
            object: p.object,
            permission: p.permission,
        })
        .collect();

    // Sort for consistent output
    result.sort_by(|a, b| a.object.cmp(&b.object));

    Ok(result)
}

/// Get users assigned to a role (accepts role name or role ID)
pub async fn get_role_users(org_id: &str, role_name_or_id: &str) -> Result<Vec<String>> {
    // First try to find role by name (most common case from frontend)
    let role = Visdata::global()
        .rbac()
        .get_role_by_name(org_id, role_name_or_id)
        .await?;

    let role_id = match role {
        Some(r) => r.id,
        None => {
            // Try to find by ID as fallback
            Visdata::global()
                .rbac()
                .get_role(org_id, role_name_or_id)
                .await?
                .ok_or_else(|| Error::RoleNotFound(role_name_or_id.to_string()))?
                .id
        }
    };

    Visdata::global().rbac().get_role_users(org_id, &role_id).await
}

/// Convert role model to response
fn role_to_response(role: vd_roles::Model) -> RoleResponse {
    RoleResponse {
        id: role.id,
        name: role.name,
        display_name: role.display_name,
        description: role.description,
        is_system: role.is_system,
        created_at: role.created_at,
        updated_at: role.updated_at,
    }
}
