// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Resource definitions HTTP handlers

use actix_web::{get, web, HttpResponse};

use crate::error::Error;
use crate::rbac::resources::get_resource_definitions;

/// GET /{org_id}/resources - Get all resource type definitions
#[get("/{org_id}/resources")]
pub async fn get_resources(path: web::Path<String>) -> Result<HttpResponse, Error> {
    let _org_id = path.into_inner();
    let resources = get_resource_definitions();
    Ok(HttpResponse::Ok().json(resources))
}
