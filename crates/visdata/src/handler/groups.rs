// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Group management HTTP handlers

use actix_web::{delete, get, post, put, web, HttpResponse};

use crate::error::Error;
use crate::meta::{CreateGroupRequest, MessageResponse, UpdateGroupRequest};
use crate::service::group;

/// POST /{org_id}/groups - Create a new group
#[post("/{org_id}/groups")]
pub async fn create_group(
    path: web::Path<String>,
    body: web::Json<CreateGroupRequest>,
) -> Result<HttpResponse, Error> {
    let org_id = path.into_inner();
    let req = body.into_inner();

    group::create_group(&org_id, &req.name, req.display_name, req.description).await?;

    Ok(HttpResponse::Ok().json(MessageResponse::new("Group created successfully")))
}

/// GET /{org_id}/groups - List all groups
/// Returns a list of group names (strings) to match enterprise API format
#[get("/{org_id}/groups")]
pub async fn list_groups(path: web::Path<String>) -> Result<HttpResponse, Error> {
    let org_id = path.into_inner();
    let groups = group::list_groups(&org_id).await?;
    // Frontend expects a list of group names, not group objects
    let group_names: Vec<String> = groups.into_iter().map(|g| g.name).collect();
    Ok(HttpResponse::Ok().json(group_names))
}

/// GET /{org_id}/groups/{group_name} - Get a specific group
#[get("/{org_id}/groups/{group_name}")]
pub async fn get_group(path: web::Path<(String, String)>) -> Result<HttpResponse, Error> {
    let (org_id, group_name) = path.into_inner();
    let group = group::get_group(&org_id, &group_name).await?;
    Ok(HttpResponse::Ok().json(group))
}

/// PUT /{org_id}/groups/{group_name} - Update a group (roles and users)
#[put("/{org_id}/groups/{group_name}")]
pub async fn update_group(
    path: web::Path<(String, String)>,
    body: web::Json<UpdateGroupRequest>,
) -> Result<HttpResponse, Error> {
    let (org_id, group_name) = path.into_inner();
    let req = body.into_inner();

    group::update_group(
        &org_id,
        &group_name,
        req.add_roles,
        req.remove_roles,
        req.add_users,
        req.remove_users,
    )
    .await?;

    Ok(HttpResponse::Ok().json(MessageResponse::new("Group updated successfully")))
}

/// DELETE /{org_id}/groups/{group_name} - Delete a group
#[delete("/{org_id}/groups/{group_name}")]
pub async fn delete_group(path: web::Path<(String, String)>) -> Result<HttpResponse, Error> {
    let (org_id, group_name) = path.into_inner();

    group::delete_group(&org_id, &group_name).await?;

    Ok(HttpResponse::Ok().json(MessageResponse::new("Group deleted successfully")))
}
