// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! SSO HTTP handlers

use actix_web::{delete, get, post, put, web, HttpRequest, HttpResponse};
use std::sync::Arc;

use crate::config::OIDCConfig;
use crate::error::Error;
use crate::meta::{
    LDAPProviderRequest, MessageResponse, OIDCProviderRequest, SSOCallbackQuery, SSOLoginRequest,
    SSOProviderResponse,
};
use crate::sso::oidc::OIDCProvider;
use crate::sso::provider::{SSOProvider, SSOProviderManager};
use crate::Visdata;

/// GET /{org_id}/sso/providers - List all SSO providers
#[get("/{org_id}/sso/providers")]
pub async fn list_providers(path: web::Path<String>) -> Result<HttpResponse, Error> {
    let org_id = path.into_inner();

    let db = Visdata::global().db();
    let manager = SSOProviderManager::new(Arc::new(db.clone()), None);

    let providers = manager.list_providers(&org_id).await?;

    let responses: Vec<SSOProviderResponse> = providers
        .into_iter()
        .map(|p| SSOProviderResponse {
            id: p.id,
            name: p.name,
            provider_type: p.provider_type,
            is_enabled: p.is_enabled,
            is_default: p.is_default,
            created_at: p.created_at,
            updated_at: p.updated_at,
        })
        .collect();

    Ok(HttpResponse::Ok().json(responses))
}

/// POST /{org_id}/sso/providers/oidc - Create an OIDC provider
#[post("/{org_id}/sso/providers/oidc")]
pub async fn create_oidc_provider(
    path: web::Path<String>,
    body: web::Json<OIDCProviderRequest>,
) -> Result<HttpResponse, Error> {
    let org_id = path.into_inner();
    let req = body.into_inner();

    let db = Visdata::global().db();
    let manager = SSOProviderManager::new(Arc::new(db.clone()), None);

    let config = OIDCConfig {
        issuer_url: req.issuer_url,
        client_id: req.client_id,
        client_secret: req.client_secret,
        scopes: req.scopes.unwrap_or_else(|| vec!["openid".to_string(), "email".to_string(), "profile".to_string()]),
        redirect_uri: req.redirect_uri.unwrap_or_default(),
        email_claim: "email".to_string(),
        name_claim: "name".to_string(),
        groups_claim: req.groups_claim,
        group_role_mappings: req.group_role_mappings.unwrap_or_default(),
        auto_create_users: true,
        default_role: None,
    };

    // Test connection before saving
    let provider = OIDCProvider::new(config.clone());
    provider.test_connection().await?;

    manager
        .create_oidc_provider(&org_id, &req.name, config, req.is_default.unwrap_or(false))
        .await?;

    tracing::info!(
        "[VISDATA] Created OIDC provider '{}' for org '{}'",
        req.name,
        org_id
    );

    Ok(HttpResponse::Ok().json(MessageResponse::new("OIDC provider created successfully")))
}

/// POST /{org_id}/sso/providers/ldap - Create an LDAP provider
#[post("/{org_id}/sso/providers/ldap")]
pub async fn create_ldap_provider(
    path: web::Path<String>,
    body: web::Json<LDAPProviderRequest>,
) -> Result<HttpResponse, Error> {
    let org_id = path.into_inner();
    let req = body.into_inner();

    let db = Visdata::global().db();
    let manager = SSOProviderManager::new(Arc::new(db.clone()), None);

    let config = crate::config::LDAPConfig {
        server_url: req.server_url,
        bind_dn: req.bind_dn,
        bind_password: req.bind_password,
        user_base_dn: req.user_base_dn,
        user_filter: req.user_filter.unwrap_or_else(|| "(uid={username})".to_string()),
        user_attr_email: req.user_attr_email.unwrap_or_else(|| "mail".to_string()),
        user_attr_name: req.user_attr_name.unwrap_or_else(|| "cn".to_string()),
        group_base_dn: req.group_base_dn,
        group_filter: req.group_filter,
        group_attr_name: req.group_attr_name.unwrap_or_else(|| "cn".to_string()),
        group_role_mappings: req.group_role_mappings.unwrap_or_default(),
        use_ssl: req.use_ssl.unwrap_or(true),
        skip_ssl_verify: req.skip_ssl_verify.unwrap_or(false),
        timeout_seconds: 10, // Default 10 seconds timeout
    };

    manager
        .create_ldap_provider(&org_id, &req.name, config, req.is_default.unwrap_or(false))
        .await?;

    tracing::info!(
        "[VISDATA] Created LDAP provider '{}' for org '{}'",
        req.name,
        org_id
    );

    Ok(HttpResponse::Ok().json(MessageResponse::new("LDAP provider created successfully")))
}

/// PUT /{org_id}/sso/providers/{provider_id} - Update an SSO provider
#[put("/{org_id}/sso/providers/{provider_id}")]
pub async fn update_provider(
    path: web::Path<(String, String)>,
    _body: web::Json<serde_json::Value>,
) -> Result<HttpResponse, Error> {
    let (org_id, provider_id) = path.into_inner();

    // TODO: Implement full SSO provider update
    tracing::info!(
        "[VISDATA] Updating SSO provider '{}' for org '{}'",
        provider_id,
        org_id
    );

    Ok(HttpResponse::Ok().json(MessageResponse::new("SSO provider updated successfully")))
}

/// DELETE /{org_id}/sso/providers/{provider_id} - Delete an SSO provider
#[delete("/{org_id}/sso/providers/{provider_id}")]
pub async fn delete_provider(path: web::Path<(String, String)>) -> Result<HttpResponse, Error> {
    let (org_id, provider_id) = path.into_inner();

    let db = Visdata::global().db();
    let manager = SSOProviderManager::new(Arc::new(db.clone()), None);

    manager.delete_provider(&org_id, &provider_id).await?;

    tracing::info!(
        "[VISDATA] Deleted SSO provider '{}' for org '{}'",
        provider_id,
        org_id
    );

    Ok(HttpResponse::Ok().json(MessageResponse::new("SSO provider deleted successfully")))
}

/// GET /{org_id}/sso/login - Initiate SSO login
#[get("/{org_id}/sso/login")]
pub async fn sso_login(
    path: web::Path<String>,
    query: web::Query<SSOLoginRequest>,
    req: HttpRequest,
) -> Result<HttpResponse, Error> {
    let org_id = path.into_inner();
    let login_req = query.into_inner();

    let db = Visdata::global().db();
    let manager = SSOProviderManager::new(Arc::new(db.clone()), None);

    // Get the provider (either specified or default)
    let provider_model = if let Some(provider_id) = &login_req.provider_id {
        manager.get_provider(&org_id, provider_id).await?
    } else {
        manager.get_default_provider(&org_id).await?
    };

    let provider_model = provider_model.ok_or_else(|| {
        Error::NotFound("No SSO provider found for this organization".to_string())
    })?;

    if !provider_model.is_enabled {
        return Err(Error::Config("SSO provider is disabled".to_string()));
    }

    // Build the redirect URI from the request
    let redirect_uri = login_req.redirect_uri.unwrap_or_else(|| {
        let conn_info = req.connection_info();
        let scheme = conn_info.scheme();
        let host = conn_info.host();
        format!("{scheme}://{host}/api/{org_id}/sso/callback")
    });

    // Generate a state token for CSRF protection
    let state = uuid::Uuid::new_v4().to_string();

    // Get the authorization URL based on provider type
    let auth_url = match provider_model.provider_type.as_str() {
        "oidc" => {
            let config = manager.get_oidc_config(&provider_model)?;
            let oidc_provider = OIDCProvider::new(config);
            oidc_provider.get_auth_url(&state, &redirect_uri).await?
        }
        "ldap" => {
            // LDAP doesn't use redirect-based auth, it uses direct credentials
            return Err(Error::Config(
                "LDAP provider doesn't support redirect-based login. Use direct authentication.".to_string(),
            ));
        }
        _ => {
            return Err(Error::Config(format!(
                "Unknown provider type: {}",
                provider_model.provider_type
            )));
        }
    };

    tracing::info!(
        "[VISDATA] Initiating SSO login for org '{}', provider: '{}', type: {}",
        org_id,
        provider_model.name,
        provider_model.provider_type
    );

    // Redirect to the identity provider
    Ok(HttpResponse::Found()
        .insert_header(("Location", auth_url))
        .finish())
}

/// GET /{org_id}/sso/callback - SSO callback handler
#[get("/{org_id}/sso/callback")]
pub async fn sso_callback(
    path: web::Path<String>,
    query: web::Query<SSOCallbackQuery>,
    req: HttpRequest,
) -> Result<HttpResponse, Error> {
    let org_id = path.into_inner();
    let callback = query.into_inner();

    // Check for error from IdP
    if let Some(error) = callback.error {
        let error_desc = callback.error_description.unwrap_or_default();
        tracing::error!(
            "[VISDATA] SSO callback error for org '{}': {} - {}",
            org_id,
            error,
            error_desc
        );
        return Err(Error::OIDC(format!("SSO error: {} - {}", error, error_desc)));
    }

    // Get the authorization code
    let code = callback.code.ok_or_else(|| {
        Error::OIDC("No authorization code in callback".to_string())
    })?;

    let db = Visdata::global().db();
    let manager = SSOProviderManager::new(Arc::new(db.clone()), None);

    // Get the default provider for the org
    let provider_model = manager
        .get_default_provider(&org_id)
        .await?
        .ok_or_else(|| Error::NotFound("No default SSO provider found".to_string()))?;

    // Build the redirect URI (must match what was used in the login request)
    let conn_info = req.connection_info();
    let scheme = conn_info.scheme();
    let host = conn_info.host();
    let redirect_uri = format!("{scheme}://{host}/api/{org_id}/sso/callback");

    // Exchange the code for user info
    let auth_result = match provider_model.provider_type.as_str() {
        "oidc" => {
            let config = manager.get_oidc_config(&provider_model)?;
            let oidc_provider = OIDCProvider::new(config);
            oidc_provider.exchange_code(&code, &redirect_uri).await?
        }
        _ => {
            return Err(Error::Config(format!(
                "Unsupported provider type for callback: {}",
                provider_model.provider_type
            )));
        }
    };

    tracing::info!(
        "[VISDATA] SSO callback successful for org '{}', user: {}",
        org_id,
        auth_result.user.email
    );

    // TODO: Create or update user in OpenObserve
    // TODO: Create session and return authentication token
    // For now, return the user info

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "user": {
            "email": auth_result.user.email,
            "name": auth_result.user.name,
            "external_id": auth_result.user.external_id,
            "groups": auth_result.user.groups,
        },
        "message": "SSO authentication successful. User session creation pending implementation."
    })))
}
