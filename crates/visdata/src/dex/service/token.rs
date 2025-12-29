// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Token validation and exchange service

use jsonwebtoken::{decode, decode_header, DecodingKey, Validation, Algorithm};
use reqwest::Client;
use std::collections::HashMap;

use crate::Visdata;
use super::super::error::{Error, Result};
use super::super::types::{
    AuthTokens, JwtClaims, PreLoginData, TokenValidationResponse, PkceData,
};

/// JWKS cache key
static JWKS_CACHE: once_cell::sync::Lazy<dashmap::DashMap<String, JwksKeys>> =
    once_cell::sync::Lazy::new(dashmap::DashMap::new);

/// PKCE state cache
static PKCE_CACHE: once_cell::sync::Lazy<dashmap::DashMap<String, PkceData>> =
    once_cell::sync::Lazy::new(dashmap::DashMap::new);

/// JWKS keys structure
#[derive(Clone)]
struct JwksKeys {
    keys: HashMap<String, DecodingKey>,
    fetched_at: std::time::Instant,
}

/// Verify a JWT token (compatible with existing format)
pub async fn verify_token(token: &str) -> Result<TokenValidationResponse> {
    let visdata = Visdata::global();
    let config = visdata.dex_config();

    // Decode header to get key ID
    let header = decode_header(token)?;
    let kid = header.kid.ok_or_else(|| Error::InvalidToken("Missing key ID".to_string()))?;

    // Get JWKS keys
    let keys = get_jwks_keys(&config.issuer_url).await?;
    let decoding_key = keys.keys.get(&kid).ok_or_else(|| {
        Error::InvalidToken(format!("Unknown key ID: {}", kid))
    })?;

    // Set up validation
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[&config.client_id]);
    validation.set_issuer(&[&config.issuer_url]);

    // Decode and validate
    let token_data = decode::<JwtClaims>(token, decoding_key, &validation)?;
    let claims = token_data.claims;

    // Check expiration
    let now = chrono::Utc::now().timestamp();
    if claims.exp < now {
        return Err(Error::TokenExpired);
    }

    // Build response (compatible with existing format)
    Ok(TokenValidationResponse {
        is_valid: true,
        user_email: claims.email.clone().unwrap_or_default(),
        user_name: claims.name.clone().unwrap_or_default(),
        family_name: claims.family_name.clone().unwrap_or_default(),
        given_name: claims.given_name.clone().unwrap_or_default(),
        is_internal_user: false, // Dex users are external
        user_role: None, // Role is determined by RBAC, not token
    })
}

/// Exchange authorization code for tokens
pub async fn exchange_code(code: &str, state: &str) -> Result<AuthTokens> {
    let visdata = Visdata::global();
    let config = visdata.dex_config();

    // Get PKCE data from cache
    let pkce = PKCE_CACHE.remove(state).map(|(_, v)| v);

    let client = Client::new();
    let token_url = format!("{}/token", config.issuer_url);

    let mut params = vec![
        ("grant_type", "authorization_code"),
        ("code", code),
        ("client_id", &config.client_id),
        ("redirect_uri", &config.redirect_uri),
    ];

    // Add client secret if not public
    if !config.client_secret.is_empty() {
        params.push(("client_secret", &config.client_secret));
    }

    // Add PKCE verifier if present
    let verifier;
    if let Some(ref p) = pkce {
        verifier = p.code_verifier.clone();
        params.push(("code_verifier", &verifier));
    }

    let response = client
        .post(&token_url)
        .form(&params)
        .send()
        .await?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(Error::InvalidToken(format!("Token exchange failed: {}", error_text)));
    }

    let token_response: serde_json::Value = response.json().await?;

    Ok(AuthTokens {
        access_token: token_response["access_token"].as_str().unwrap_or_default().to_string(),
        refresh_token: token_response["refresh_token"].as_str().map(|s| s.to_string()),
        id_token: token_response["id_token"].as_str().map(|s| s.to_string()),
        token_type: token_response["token_type"].as_str().unwrap_or("Bearer").to_string(),
        expires_in: token_response["expires_in"].as_i64().unwrap_or(3600),
    })
}

/// Refresh access token using refresh token
pub async fn refresh_token(refresh_token_str: &str) -> Result<AuthTokens> {
    let visdata = Visdata::global();
    let config = visdata.dex_config();

    let client = Client::new();
    let token_url = format!("{}/token", config.issuer_url);

    let mut params = vec![
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token_str),
        ("client_id", &config.client_id),
    ];

    if !config.client_secret.is_empty() {
        params.push(("client_secret", &config.client_secret));
    }

    let response = client
        .post(&token_url)
        .form(&params)
        .send()
        .await?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(Error::InvalidToken(format!("Token refresh failed: {}", error_text)));
    }

    let token_response: serde_json::Value = response.json().await?;

    Ok(AuthTokens {
        access_token: token_response["access_token"].as_str().unwrap_or_default().to_string(),
        refresh_token: token_response["refresh_token"].as_str().map(|s| s.to_string()),
        id_token: token_response["id_token"].as_str().map(|s| s.to_string()),
        token_type: token_response["token_type"].as_str().unwrap_or("Bearer").to_string(),
        expires_in: token_response["expires_in"].as_i64().unwrap_or(3600),
    })
}

/// Generate pre-login data (auth URL with PKCE)
pub async fn pre_login(connector_id: Option<&str>) -> Result<PreLoginData> {
    let visdata = Visdata::global();
    let config = visdata.dex_config();

    // Generate PKCE
    let pkce = generate_pkce();
    let state = pkce.state.clone();

    // Store PKCE in cache (expires in 10 minutes)
    PKCE_CACHE.insert(state.clone(), pkce.clone());

    // Build auth URL
    let mut auth_url = format!(
        "{}/auth?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}&code_challenge={}&code_challenge_method=S256",
        config.issuer_url,
        urlencoding::encode(&config.client_id),
        urlencoding::encode(&config.redirect_uri),
        urlencoding::encode(&config.scopes.join(" ")),
        urlencoding::encode(&state),
        urlencoding::encode(&pkce.code_challenge),
    );

    // Add connector hint if specified
    if let Some(connector) = connector_id {
        auth_url.push_str(&format!("&connector_id={}", urlencoding::encode(connector)));
    }

    Ok(PreLoginData {
        state,
        auth_url,
    })
}

/// Generate PKCE code verifier and challenge
fn generate_pkce() -> PkceData {
    use base64::Engine;
    use rand::Rng;
    use rand::distr::Alphanumeric;

    // Generate random state
    let state: String = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();

    // Generate code verifier (43-128 characters)
    let code_verifier: String = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(64)
        .map(char::from)
        .collect();

    // Generate code challenge (SHA256 hash of verifier, base64url encoded)
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(code_verifier.as_bytes());
    let hash = hasher.finalize();
    let code_challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash);

    PkceData {
        code_verifier,
        code_challenge,
        state,
    }
}

/// Fetch JWKS keys from issuer
async fn get_jwks_keys(issuer_url: &str) -> Result<JwksKeys> {
    // Check cache (refresh every 5 minutes)
    if let Some(cached) = JWKS_CACHE.get(issuer_url) {
        if cached.fetched_at.elapsed() < std::time::Duration::from_secs(300) {
            return Ok(cached.clone());
        }
    }

    // First, discover the JWKS URI from the OIDC discovery endpoint
    let client = Client::new();
    let discovery_url = format!("{}/.well-known/openid-configuration", issuer_url);

    let jwks_url = match client.get(&discovery_url).send().await {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(config) = resp.json::<serde_json::Value>().await {
                config["jwks_uri"]
                    .as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("{}/keys", issuer_url))
            } else {
                format!("{}/keys", issuer_url)
            }
        }
        _ => format!("{}/keys", issuer_url), // Fallback to Dex default
    };

    let response = client.get(&jwks_url).send().await?;
    if !response.status().is_success() {
        return Err(Error::HttpError(format!(
            "Failed to fetch JWKS: {}",
            response.status()
        )));
    }

    let jwks: serde_json::Value = response.json().await?;
    let keys_array = jwks["keys"].as_array().ok_or_else(|| {
        Error::ConfigError("Invalid JWKS response".to_string())
    })?;

    let mut keys = HashMap::new();
    for key in keys_array {
        if let (Some(kid), Some(n), Some(e)) = (
            key["kid"].as_str(),
            key["n"].as_str(),
            key["e"].as_str(),
        ) {
            if let Ok(decoding_key) = DecodingKey::from_rsa_components(n, e) {
                keys.insert(kid.to_string(), decoding_key);
            }
        }
    }

    let jwks_keys = JwksKeys {
        keys,
        fetched_at: std::time::Instant::now(),
    };

    JWKS_CACHE.insert(issuer_url.to_string(), jwks_keys.clone());

    Ok(jwks_keys)
}

/// Verify native login credentials
pub async fn verify_native_login(email: &str, password: &str) -> Result<bool> {
    let visdata = Visdata::global();
    let mut dex = visdata.dex().write().await;

    dex.verify_password(email, password).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_pkce() {
        let pkce = generate_pkce();
        assert_eq!(pkce.state.len(), 32);
        assert_eq!(pkce.code_verifier.len(), 64);
        assert!(!pkce.code_challenge.is_empty());
    }
}
