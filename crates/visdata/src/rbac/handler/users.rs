// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! User role/group query HTTP handlers

use actix_web::{get, web, HttpResponse};

use super::super::error::Result;
use super::super::service::{groups, roles};

/// GET /{org_id}/users/{email}/roles - Get all roles for a user
#[get("/{org_id}/users/{email}/roles")]
pub async fn get_user_roles(path: web::Path<(String, String)>) -> Result<HttpResponse> {
    let (org_id, email) = path.into_inner();

    let user_roles = groups::get_user_roles(&org_id, &email).await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "code": 200,
        "data": user_roles
    })))
}

/// GET /{org_id}/users/{email}/groups - Get all groups for a user
#[get("/{org_id}/users/{email}/groups")]
pub async fn get_user_groups(path: web::Path<(String, String)>) -> Result<HttpResponse> {
    let (org_id, email) = path.into_inner();

    let user_groups = groups::get_user_groups(&org_id, &email).await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "code": 200,
        "data": user_groups
    })))
}

/// GET /{org_id}/users/roles - List system roles (for dropdown)
#[get("/{org_id}/users/roles")]
pub async fn list_system_roles(path: web::Path<String>) -> Result<HttpResponse> {
    let org_id = path.into_inner();

    let system_roles = roles::list_system_roles(&org_id).await;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "code": 200,
        "data": system_roles
    })))
}

/// GET /{org_id}/users/custom_roles - List custom roles (for dropdown)
#[get("/{org_id}/users/custom_roles")]
pub async fn list_custom_roles(path: web::Path<String>) -> Result<HttpResponse> {
    let org_id = path.into_inner();

    let custom_roles = roles::list_custom_roles(&org_id).await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "code": 200,
        "data": custom_roles
    })))
}
