// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Role management HTTP handlers

use actix_web::{delete, get, post, put, web, HttpResponse};

use super::super::error::Result;
use super::super::service::roles;
use super::super::types::{CreateRoleRequest, UpdateRoleRequest};

/// POST /{org_id}/roles - Create a new role
#[post("/{org_id}/roles")]
pub async fn create_role(
    path: web::Path<String>,
    body: web::Json<CreateRoleRequest>,
) -> Result<HttpResponse> {
    let org_id = path.into_inner();
    let req = body.into_inner();

    roles::create_role(&org_id, &req.role).await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "code": 200,
        "message": "Role created successfully"
    })))
}

/// GET /{org_id}/roles - List all roles
#[get("/{org_id}/roles")]
pub async fn list_roles(path: web::Path<String>) -> Result<HttpResponse> {
    let org_id = path.into_inner();

    let role_list = roles::list_roles(&org_id).await?;

    // Return array directly (matching enterprise format)
    Ok(HttpResponse::Ok().json(role_list))
}

/// PUT /{org_id}/roles/{role_name} - Update a role (add/remove permissions and users)
#[put("/{org_id}/roles/{role_name}")]
pub async fn update_role(
    path: web::Path<(String, String)>,
    body: web::Json<UpdateRoleRequest>,
) -> Result<HttpResponse> {
    let (org_id, role_name) = path.into_inner();
    let req = body.into_inner();

    // Add permissions
    if let Some(ref add_perms) = req.add {
        if !add_perms.is_empty() {
            roles::add_role_permissions(&org_id, &role_name, add_perms).await?;
        }
    }

    // Remove permissions
    if let Some(ref remove_perms) = req.remove {
        if !remove_perms.is_empty() {
            roles::remove_role_permissions(&org_id, &role_name, remove_perms).await?;
        }
    }

    // Add users
    if let Some(ref add_users) = req.add_users {
        if !add_users.is_empty() {
            roles::add_role_users(&org_id, &role_name, add_users).await?;
        }
    }

    // Remove users
    if let Some(ref remove_users) = req.remove_users {
        if !remove_users.is_empty() {
            roles::remove_role_users(&org_id, &role_name, remove_users).await?;
        }
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "code": 200,
        "message": "Role updated successfully"
    })))
}

/// DELETE /{org_id}/roles/{role_name} - Delete a role
#[delete("/{org_id}/roles/{role_name}")]
pub async fn delete_role(path: web::Path<(String, String)>) -> Result<HttpResponse> {
    let (org_id, role_name) = path.into_inner();

    roles::delete_role(&org_id, &role_name).await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "code": 200,
        "message": "Role deleted successfully"
    })))
}

/// GET /{org_id}/roles/{role_name}/permissions/{resource_type} - Get role permissions for a resource type
#[get("/{org_id}/roles/{role_name}/permissions/{resource_type}")]
pub async fn get_role_permissions(
    path: web::Path<(String, String, String)>,
) -> Result<HttpResponse> {
    let (org_id, role_name, resource_type) = path.into_inner();

    let permissions = roles::get_role_permissions(&org_id, &role_name, &resource_type).await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "code": 200,
        "data": permissions
    })))
}

/// GET /{org_id}/roles/{role_name}/users - Get users assigned to a role
#[get("/{org_id}/roles/{role_name}/users")]
pub async fn get_role_users(path: web::Path<(String, String)>) -> Result<HttpResponse> {
    let (org_id, role_name) = path.into_inner();

    let users = roles::get_role_users(&org_id, &role_name).await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "code": 200,
        "data": users
    })))
}
