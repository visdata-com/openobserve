// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Connector (IdP) management HTTP handlers

use actix_web::{delete, get, post, put, web, HttpResponse};

use super::super::error::Result;
use super::super::service::connector;
use super::super::types::{
    CreateOidcConnectorRequest, CreateLdapConnectorRequest, CreateSamlConnectorRequest,
    UpdateConnectorRequest,
};

/// GET /{org_id}/sso/providers - List all SSO providers
#[get("/{org_id}/sso/providers")]
pub async fn list_providers(_path: web::Path<String>) -> Result<HttpResponse> {
    let providers = connector::list_connectors().await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "code": 200,
        "data": providers
    })))
}

/// POST /{org_id}/sso/providers/oidc - Create OIDC provider
#[post("/{org_id}/sso/providers/oidc")]
pub async fn create_oidc_provider(
    _path: web::Path<String>,
    body: web::Json<CreateOidcConnectorRequest>,
) -> Result<HttpResponse> {
    let req = body.into_inner();

    connector::create_oidc_connector(req).await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "code": 200,
        "message": "OIDC provider created successfully"
    })))
}

/// POST /{org_id}/sso/providers/ldap - Create LDAP provider
#[post("/{org_id}/sso/providers/ldap")]
pub async fn create_ldap_provider(
    _path: web::Path<String>,
    body: web::Json<CreateLdapConnectorRequest>,
) -> Result<HttpResponse> {
    let req = body.into_inner();

    connector::create_ldap_connector(req).await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "code": 200,
        "message": "LDAP provider created successfully"
    })))
}

/// POST /{org_id}/sso/providers/saml - Create SAML provider
#[post("/{org_id}/sso/providers/saml")]
pub async fn create_saml_provider(
    _path: web::Path<String>,
    body: web::Json<CreateSamlConnectorRequest>,
) -> Result<HttpResponse> {
    let req = body.into_inner();

    connector::create_saml_connector(req).await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "code": 200,
        "message": "SAML provider created successfully"
    })))
}

/// GET /{org_id}/sso/providers/{provider_id} - Get provider details
#[get("/{org_id}/sso/providers/{provider_id}")]
pub async fn get_provider(path: web::Path<(String, String)>) -> Result<HttpResponse> {
    let (_org_id, provider_id) = path.into_inner();

    let provider = connector::get_connector(&provider_id).await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "code": 200,
        "data": provider
    })))
}

/// PUT /{org_id}/sso/providers/{provider_id} - Update provider
#[put("/{org_id}/sso/providers/{provider_id}")]
pub async fn update_provider(
    path: web::Path<(String, String)>,
    body: web::Json<UpdateConnectorRequest>,
) -> Result<HttpResponse> {
    let (_org_id, provider_id) = path.into_inner();
    let req = body.into_inner();

    // Get existing provider first
    let existing = connector::get_connector(&provider_id).await?;

    // Build update config
    let config = req.config.map(|c| c.to_string()).unwrap_or_default();
    let name = req.name.unwrap_or(existing.name);

    connector::update_connector(&provider_id, &existing.provider_type, &name, &config).await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "code": 200,
        "message": "Provider updated successfully"
    })))
}

/// DELETE /{org_id}/sso/providers/{provider_id} - Delete provider
#[delete("/{org_id}/sso/providers/{provider_id}")]
pub async fn delete_provider(path: web::Path<(String, String)>) -> Result<HttpResponse> {
    let (_org_id, provider_id) = path.into_inner();

    connector::delete_connector(&provider_id).await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "code": 200,
        "message": "Provider deleted successfully"
    })))
}
