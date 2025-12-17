// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Resource definition HTTP handlers

use actix_web::{get, web, HttpResponse};

use super::super::error::Result;
use super::super::model::resources;

/// GET /{org_id}/resources - Get all resource definitions
#[get("/{org_id}/resources")]
pub async fn get_resources(_path: web::Path<String>) -> Result<HttpResponse> {
    let all_resources = resources::get_all_resources();

    // Return array directly (matching enterprise format)
    // Resource struct already has correct field names via serde rename
    Ok(HttpResponse::Ok().json(all_resources))
}

/// GET /{org_id}/resources/{resource_type} - Get a specific resource definition
#[get("/{org_id}/resources/{resource_type}")]
pub async fn get_resource(path: web::Path<(String, String)>) -> Result<HttpResponse> {
    let (_org_id, resource_type) = path.into_inner();

    match resources::get_resource(&resource_type) {
        Some(resource) => Ok(HttpResponse::Ok().json(serde_json::json!({
            "code": 200,
            "data": {
                "key": resource.key,
                "label": resource.label,
                "parent": resource.parent,
                "order": resource.order,
                "visible": resource.visible,
            }
        }))),
        None => Ok(HttpResponse::NotFound().json(serde_json::json!({
            "code": 404,
            "message": format!("Resource type '{}' not found", resource_type)
        }))),
    }
}
