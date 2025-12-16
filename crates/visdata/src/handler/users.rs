// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! User query HTTP handlers

use actix_web::{get, web, HttpResponse};
use serde::Serialize;

use crate::error::Error;
use crate::Visdata;

/// Response format for user roles list (matches frontend expectation)
#[derive(Debug, Serialize)]
pub struct UserRoleOption {
    pub label: String,
    pub value: String,
}

/// GET /{org_id}/users/roles - List system roles for user assignment (single select)
/// Returns system roles (is_system = true) in {label, value} format for dropdown
#[get("/{org_id}/users/roles")]
pub async fn list_system_roles(path: web::Path<String>) -> Result<HttpResponse, Error> {
    let org_id = path.into_inner();
    let rbac = Visdata::global().rbac();

    // Get all roles and filter to only system roles (is_system = true)
    let roles = rbac.list_roles(&org_id).await?;
    let system_roles: Vec<UserRoleOption> = roles
        .into_iter()
        .filter(|r| r.is_system)
        .map(|r| UserRoleOption {
            label: r.display_name.unwrap_or_else(|| r.name.clone()),
            value: r.name.to_lowercase(), // frontend expects lowercase value like "admin"
        })
        .collect();

    Ok(HttpResponse::Ok().json(system_roles))
}

/// GET /{org_id}/users/custom_roles - List custom roles for user assignment (multi select)
/// Returns custom roles (is_system = false) in {label, value} format for dropdown
#[get("/{org_id}/users/custom_roles")]
pub async fn list_custom_roles(path: web::Path<String>) -> Result<HttpResponse, Error> {
    let org_id = path.into_inner();
    let rbac = Visdata::global().rbac();

    // Get all roles and filter to only custom roles (is_system = false)
    let roles = rbac.list_roles(&org_id).await?;
    let custom_roles: Vec<UserRoleOption> = roles
        .into_iter()
        .filter(|r| !r.is_system)
        .map(|r| UserRoleOption {
            label: r.display_name.unwrap_or_else(|| r.name.clone()),
            value: r.name,
        })
        .collect();

    Ok(HttpResponse::Ok().json(custom_roles))
}

/// GET /{org_id}/users/{user_email}/roles - Get all custom role names for a user
/// Returns a simple array of role names (strings) for the user edit form
#[get("/{org_id}/users/{user_email}/roles")]
pub async fn get_user_roles(path: web::Path<(String, String)>) -> Result<HttpResponse, Error> {
    let (org_id, user_email) = path.into_inner();
    let rbac = Visdata::global().rbac();

    // Get direct roles and filter to only custom roles (is_system = false)
    let direct_roles = rbac.get_user_direct_roles(&org_id, &user_email).await?;
    let mut role_names: Vec<String> = direct_roles
        .into_iter()
        .filter(|r| !r.is_system) // Only custom roles, not system roles
        .map(|r| r.name)
        .collect();

    // Get roles from groups
    let groups = rbac.get_user_groups(&org_id, &user_email).await?;
    for group in groups {
        use crate::entity::{vd_group_roles, vd_roles};
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

        let db = Visdata::global().db();
        let role_ids: Vec<String> = vd_group_roles::Entity::find()
            .filter(vd_group_roles::Column::GroupId.eq(&group.id))
            .select_only()
            .column(vd_group_roles::Column::RoleId)
            .into_tuple()
            .all(db)
            .await?;

        if !role_ids.is_empty() {
            let roles = vd_roles::Entity::find()
                .filter(vd_roles::Column::Id.is_in(role_ids))
                .filter(vd_roles::Column::IsSystem.eq(false)) // Only custom roles
                .all(db)
                .await?;

            for role in roles {
                if !role_names.contains(&role.name) {
                    role_names.push(role.name);
                }
            }
        }
    }

    // Return simple array of role names
    Ok(HttpResponse::Ok().json(role_names))
}

/// GET /{org_id}/users/{user_email}/groups - Get all group names for a user
/// Returns a simple array of group names (strings)
#[get("/{org_id}/users/{user_email}/groups")]
pub async fn get_user_groups(path: web::Path<(String, String)>) -> Result<HttpResponse, Error> {
    let (org_id, user_email) = path.into_inner();
    let rbac = Visdata::global().rbac();

    let groups = rbac.get_user_groups(&org_id, &user_email).await?;

    // Return simple array of group names
    let group_names: Vec<String> = groups.into_iter().map(|g| g.name).collect();
    Ok(HttpResponse::Ok().json(group_names))
}
