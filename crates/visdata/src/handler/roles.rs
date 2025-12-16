// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Role management HTTP handlers

use actix_web::{delete, get, post, put, web, HttpResponse};

use crate::error::Error;
use crate::meta::{CreateRoleRequest, MessageResponse, UpdateRoleRequest};
use crate::service::role;

/// POST /{org_id}/roles - Create a new role
#[post("/{org_id}/roles")]
pub async fn create_role(
    path: web::Path<String>,
    body: web::Json<CreateRoleRequest>,
) -> Result<HttpResponse, Error> {
    let org_id = path.into_inner();
    let req = body.into_inner();

    role::create_role(&org_id, &req.role).await?;

    Ok(HttpResponse::Ok().json(MessageResponse::new("Role created successfully")))
}

/// GET /{org_id}/roles - List all roles for role management page
/// Returns a list of all role names (strings)
#[get("/{org_id}/roles")]
pub async fn list_roles(path: web::Path<String>) -> Result<HttpResponse, Error> {
    let org_id = path.into_inner();
    let roles = role::list_roles(&org_id).await?;
    // Return all role names for the role management page
    let role_names: Vec<String> = roles.into_iter().map(|r| r.name).collect();
    Ok(HttpResponse::Ok().json(role_names))
}

/// PUT /{org_id}/roles/{role_id} - Update a role (permissions and users)
#[put("/{org_id}/roles/{role_id}")]
pub async fn update_role(
    path: web::Path<(String, String)>,
    body: web::Json<UpdateRoleRequest>,
) -> Result<HttpResponse, Error> {
    let (org_id, role_id) = path.into_inner();
    let req = body.into_inner();

    role::update_role(
        &org_id,
        &role_id,
        req.add,
        req.remove,
        req.add_users,
        req.remove_users,
    )
    .await?;

    Ok(HttpResponse::Ok().json(MessageResponse::new("Role updated successfully")))
}

/// DELETE /{org_id}/roles/{role_id} - Delete a role
#[delete("/{org_id}/roles/{role_id}")]
pub async fn delete_role(path: web::Path<(String, String)>) -> Result<HttpResponse, Error> {
    let (org_id, role_id) = path.into_inner();

    role::delete_role(&org_id, &role_id).await?;

    Ok(HttpResponse::Ok().json(MessageResponse::new("Role deleted successfully")))
}

/// GET /{org_id}/roles/{role_id}/permissions/{resource} - Get role permissions for a resource
#[get("/{org_id}/roles/{role_id}/permissions/{resource}")]
pub async fn get_role_permissions(
    path: web::Path<(String, String, String)>,
) -> Result<HttpResponse, Error> {
    let (org_id, role_id, resource) = path.into_inner();

    let permissions = role::get_role_permissions(&org_id, &role_id, Some(&resource)).await?;

    Ok(HttpResponse::Ok().json(permissions))
}

/// GET /{org_id}/roles/{role_id}/users - Get users assigned to a role
#[get("/{org_id}/roles/{role_id}/users")]
pub async fn get_role_users(path: web::Path<(String, String)>) -> Result<HttpResponse, Error> {
    let (org_id, role_id) = path.into_inner();

    let users = role::get_role_users(&org_id, &role_id).await?;

    Ok(HttpResponse::Ok().json(users))
}
