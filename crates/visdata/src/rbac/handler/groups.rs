// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Group management HTTP handlers

use actix_web::{delete, get, post, put, web, HttpResponse};

use super::super::error::Result;
use super::super::service::groups;
use super::super::types::{CreateGroupRequest, UpdateGroupRequest};

/// POST /{org_id}/groups - Create a new group
#[post("/{org_id}/groups")]
pub async fn create_group(
    path: web::Path<String>,
    body: web::Json<CreateGroupRequest>,
) -> Result<HttpResponse> {
    let org_id = path.into_inner();
    let req = body.into_inner();

    let group_id = groups::create_group(
        &org_id,
        &req.name,
        req.display_name.as_deref(),
        req.description.as_deref(),
    )
    .await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "code": 200,
        "message": "Group created successfully",
        "data": {
            "id": group_id
        }
    })))
}

/// GET /{org_id}/groups - List all groups
#[get("/{org_id}/groups")]
pub async fn list_groups(path: web::Path<String>) -> Result<HttpResponse> {
    let org_id = path.into_inner();

    let group_list = groups::list_groups(&org_id).await?;

    // Return array directly (matching enterprise format)
    Ok(HttpResponse::Ok().json(group_list))
}

/// GET /{org_id}/groups/{group_name} - Get group details
#[get("/{org_id}/groups/{group_name}")]
pub async fn get_group(path: web::Path<(String, String)>) -> Result<HttpResponse> {
    let (org_id, group_name) = path.into_inner();

    let group = groups::get_group(&org_id, &group_name).await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "code": 200,
        "data": group
    })))
}

/// PUT /{org_id}/groups/{group_name} - Update a group (add/remove roles and users)
#[put("/{org_id}/groups/{group_name}")]
pub async fn update_group(
    path: web::Path<(String, String)>,
    body: web::Json<UpdateGroupRequest>,
) -> Result<HttpResponse> {
    let (org_id, group_name) = path.into_inner();
    let req = body.into_inner();

    // Add roles
    if let Some(ref add_roles) = req.add_roles {
        if !add_roles.is_empty() {
            groups::add_group_roles(&org_id, &group_name, add_roles).await?;
        }
    }

    // Remove roles
    if let Some(ref remove_roles) = req.remove_roles {
        if !remove_roles.is_empty() {
            groups::remove_group_roles(&org_id, &group_name, remove_roles).await?;
        }
    }

    // Add users
    if let Some(ref add_users) = req.add_users {
        if !add_users.is_empty() {
            groups::add_group_users(&org_id, &group_name, add_users).await?;
        }
    }

    // Remove users
    if let Some(ref remove_users) = req.remove_users {
        if !remove_users.is_empty() {
            groups::remove_group_users(&org_id, &group_name, remove_users).await?;
        }
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "code": 200,
        "message": "Group updated successfully"
    })))
}

/// DELETE /{org_id}/groups/{group_name} - Delete a group
#[delete("/{org_id}/groups/{group_name}")]
pub async fn delete_group(path: web::Path<(String, String)>) -> Result<HttpResponse> {
    let (org_id, group_name) = path.into_inner();

    groups::delete_group(&org_id, &group_name).await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "code": 200,
        "message": "Group deleted successfully"
    })))
}
