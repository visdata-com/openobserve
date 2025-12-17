// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Login/logout HTTP handlers

use actix_web::{get, post, web, HttpRequest, HttpResponse, cookie::Cookie};

use super::super::error::{Error, Result};
use super::super::service::token;
use super::super::types::{
    SignInUser, SignInResponse, SsoCallbackQuery, RefreshTokenRequest,
};

/// POST /auth/login - Native login with username/password
#[post("/auth/login")]
pub async fn post_login(body: web::Json<SignInUser>) -> Result<HttpResponse> {
    let req = body.into_inner();

    // Verify credentials via Dex
    let verified = token::verify_native_login(&req.name, &req.password).await?;

    if !verified {
        return Ok(HttpResponse::Unauthorized().json(SignInResponse {
            status: false,
            message: "Invalid credentials".to_string(),
        }));
    }

    // Generate pre-login (this will redirect to Dex for actual token)
    let pre_login = token::pre_login(Some("local")).await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "status": true,
        "message": "Login successful",
        "redirect_url": pre_login.auth_url
    })))
}

/// GET /auth/login - Get auth cookie/status
#[get("/auth/login")]
pub async fn get_login(req: HttpRequest) -> Result<HttpResponse> {
    // Check for existing auth cookie
    if let Some(cookie) = req.cookie("auth_token") {
        let token = cookie.value();

        // Verify the token
        match token::verify_token(token).await {
            Ok(validation) => {
                return Ok(HttpResponse::Ok().json(serde_json::json!({
                    "status": true,
                    "user": {
                        "email": validation.user_email,
                        "name": validation.user_name,
                    }
                })));
            }
            Err(_) => {
                // Token invalid, clear cookie
                let mut response = HttpResponse::Unauthorized().json(SignInResponse {
                    status: false,
                    message: "Token expired or invalid".to_string(),
                });

                // Clear the cookie
                let cookie = Cookie::build("auth_token", "")
                    .path("/")
                    .max_age(actix_web::cookie::time::Duration::ZERO)
                    .finish();
                response.add_cookie(&cookie).ok();

                return Ok(response);
            }
        }
    }

    Ok(HttpResponse::Unauthorized().json(SignInResponse {
        status: false,
        message: "Not authenticated".to_string(),
    }))
}

/// POST /auth/refresh - Refresh access token
#[post("/auth/refresh")]
pub async fn refresh_token_handler(
    body: web::Json<RefreshTokenRequest>,
) -> Result<HttpResponse> {
    let req = body.into_inner();

    let tokens = token::refresh_token(&req.refresh_token).await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "code": 200,
        "data": tokens
    })))
}

/// GET /{org_id}/sso/login - Initiate SSO login
#[get("/{org_id}/sso/login")]
pub async fn sso_login(
    path: web::Path<String>,
    query: web::Query<SsoLoginQuery>,
) -> Result<HttpResponse> {
    let _org_id = path.into_inner();
    let connector_id = query.connector_id.as_deref();

    let pre_login = token::pre_login(connector_id).await?;

    Ok(HttpResponse::Found()
        .insert_header(("Location", pre_login.auth_url))
        .finish())
}

/// GET /{org_id}/sso/callback - SSO callback handler
#[get("/{org_id}/sso/callback")]
pub async fn sso_callback(
    path: web::Path<String>,
    query: web::Query<SsoCallbackQuery>,
) -> Result<HttpResponse> {
    let _org_id = path.into_inner();
    let params = query.into_inner();

    // Check for error
    if let Some(error) = params.error {
        let description = params.error_description.unwrap_or_default();
        return Err(Error::InvalidToken(format!("{}: {}", error, description)));
    }

    // Get code and state
    let code = params.code.ok_or_else(|| {
        Error::InvalidToken("Missing authorization code".to_string())
    })?;
    let state = params.state.ok_or_else(|| {
        Error::InvalidToken("Missing state parameter".to_string())
    })?;

    // Exchange code for tokens
    let tokens = token::exchange_code(&code, &state).await?;

    // Build response with cookie
    let mut response = HttpResponse::Found()
        .insert_header(("Location", "/web/"))
        .finish();

    // Set auth cookie
    let cookie = Cookie::build("auth_token", &tokens.access_token)
        .path("/")
        .http_only(true)
        .secure(true)
        .same_site(actix_web::cookie::SameSite::Lax)
        .max_age(actix_web::cookie::time::Duration::seconds(tokens.expires_in))
        .finish();
    response.add_cookie(&cookie).ok();

    // Set refresh token cookie if present
    if let Some(ref refresh) = tokens.refresh_token {
        let refresh_cookie = Cookie::build("refresh_token", refresh)
            .path("/auth")
            .http_only(true)
            .secure(true)
            .same_site(actix_web::cookie::SameSite::Strict)
            .max_age(actix_web::cookie::time::Duration::days(30))
            .finish();
        response.add_cookie(&refresh_cookie).ok();
    }

    Ok(response)
}

/// POST /auth/logout - Logout and clear cookies
#[post("/auth/logout")]
pub async fn logout() -> Result<HttpResponse> {
    let mut response = HttpResponse::Ok().json(SignInResponse {
        status: true,
        message: "Logged out successfully".to_string(),
    });

    // Clear auth cookie
    let auth_cookie = Cookie::build("auth_token", "")
        .path("/")
        .max_age(actix_web::cookie::time::Duration::ZERO)
        .finish();
    response.add_cookie(&auth_cookie).ok();

    // Clear refresh cookie
    let refresh_cookie = Cookie::build("refresh_token", "")
        .path("/auth")
        .max_age(actix_web::cookie::time::Duration::ZERO)
        .finish();
    response.add_cookie(&refresh_cookie).ok();

    Ok(response)
}

/// Query parameters for SSO login
#[derive(Debug, serde::Deserialize)]
pub struct SsoLoginQuery {
    pub connector_id: Option<String>,
}
