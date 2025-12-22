// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Authentication types (compatible with existing API format)

use serde::{Deserialize, Serialize};

// ============================================================================
// Login Types
// ============================================================================

/// Sign-in request (compatible with existing format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignInUser {
    pub name: String,
    pub password: String,
}

/// Sign-in response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignInResponse {
    pub status: bool,
    pub message: String,
}

/// Pre-login response (for OIDC flow)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreLoginData {
    pub state: String,
    pub auth_url: String,
}

/// Auth tokens response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub id_token: Option<String>,
    pub token_type: String,
    pub expires_in: i64,
}

/// Token validation response (compatible with existing format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenValidationResponse {
    pub is_valid: bool,
    pub user_email: String,
    pub user_name: String,
    pub family_name: String,
    pub given_name: String,
    pub is_internal_user: bool,
    pub user_role: Option<String>,
}

/// Refresh token request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

// ============================================================================
// Connector Types
// ============================================================================

/// SSO Provider (compatible with existing format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SsoProvider {
    pub id: String,
    #[serde(rename = "type")]
    pub provider_type: String,
    pub name: String,
    pub enabled: bool,
}

/// Create OIDC connector request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOidcConnectorRequest {
    pub id: String,
    pub name: String,
    pub issuer: String,
    pub client_id: String,
    pub client_secret: String,
    #[serde(default)]
    pub scopes: Vec<String>,
    pub redirect_uri: Option<String>,
    #[serde(default)]
    pub insecure_skip_verify: bool,
    /// Claim to use for user groups
    pub groups_claim: Option<String>,
    /// Claim to use for user email
    pub email_claim: Option<String>,
}

/// Create LDAP connector request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateLdapConnectorRequest {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub use_ssl: bool,
    #[serde(default)]
    pub start_tls: bool,
    #[serde(default)]
    pub insecure_skip_verify: bool,
    /// Bind DN for LDAP queries
    pub bind_dn: String,
    /// Bind password
    pub bind_password: String,
    /// User search base DN
    pub user_search_base_dn: String,
    /// User search filter
    pub user_search_filter: Option<String>,
    /// User search username attribute
    pub user_search_username: Option<String>,
    /// User search ID attribute
    pub user_search_id_attr: Option<String>,
    /// User search email attribute
    pub user_search_email_attr: Option<String>,
    /// User search name attribute
    pub user_search_name_attr: Option<String>,
    /// Group search base DN
    pub group_search_base_dn: Option<String>,
    /// Group search filter
    pub group_search_filter: Option<String>,
}

/// Create SAML connector request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSamlConnectorRequest {
    pub id: String,
    pub name: String,
    /// SAML SSO URL
    pub sso_url: String,
    /// SAML Entity Issuer
    pub entity_issuer: Option<String>,
    /// SAML SSO Issuer
    pub sso_issuer: Option<String>,
    /// CA certificate (PEM format)
    pub ca: Option<String>,
    /// Redirect URI
    pub redirect_uri: Option<String>,
    /// Attribute mapping for name
    pub name_attr: Option<String>,
    /// Attribute mapping for email
    pub email_attr: Option<String>,
    /// Attribute mapping for groups
    pub groups_attr: Option<String>,
}

/// Update connector request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConnectorRequest {
    pub name: Option<String>,
    pub enabled: Option<bool>,
    /// Type-specific configuration as JSON
    pub config: Option<serde_json::Value>,
}

/// Connector details response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub connector_type: String,
    pub name: String,
    pub config: serde_json::Value,
}

// ============================================================================
// User Info Types
// ============================================================================

/// OIDC UserInfo
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub sub: String,
    pub email: Option<String>,
    pub email_verified: Option<bool>,
    pub name: Option<String>,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    pub preferred_username: Option<String>,
    pub picture: Option<String>,
    pub groups: Option<Vec<String>>,
}

/// JWT Claims
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    pub sub: String,
    pub iss: String,
    pub aud: StringOrVec,
    pub exp: i64,
    pub iat: i64,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub email_verified: Option<bool>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub given_name: Option<String>,
    #[serde(default)]
    pub family_name: Option<String>,
    #[serde(default)]
    pub groups: Option<Vec<String>>,
}

/// String or Vec<String> for audience claim
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StringOrVec {
    Single(String),
    Multiple(Vec<String>),
}

impl StringOrVec {
    pub fn contains(&self, value: &str) -> bool {
        match self {
            StringOrVec::Single(s) => s == value,
            StringOrVec::Multiple(v) => v.iter().any(|s| s == value),
        }
    }
}

// ============================================================================
// SSO Flow Types
// ============================================================================

/// SSO login initiation response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SsoLoginResponse {
    pub redirect_url: String,
    pub state: String,
}

/// SSO callback query parameters
#[derive(Debug, Clone, Deserialize)]
pub struct SsoCallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

/// PKCE data for OAuth2 flow
#[derive(Debug, Clone)]
pub struct PkceData {
    pub code_verifier: String,
    pub code_challenge: String,
    pub state: String,
}
