// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Dex authentication configuration

use serde::{Deserialize, Serialize};

/// Dex authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DexConfig {
    /// Dex gRPC API URL (e.g., "http://dex:5557")
    pub grpc_url: String,

    /// OAuth2 Client ID
    pub client_id: String,

    /// OAuth2 Client Secret
    pub client_secret: String,

    /// OIDC Issuer URL (e.g., "https://dex.example.com")
    pub issuer_url: String,

    /// OAuth2 Redirect URI
    pub redirect_uri: String,

    /// Default organization for new users
    pub default_org: String,

    /// Default role for new users
    pub default_role: String,

    /// Enable native (username/password) login
    pub native_login_enabled: bool,

    /// OIDC group claim name
    pub group_claim: String,

    /// OIDC scopes to request
    pub scopes: Vec<String>,

    /// gRPC connection timeout in seconds
    pub timeout_seconds: u64,
}

impl Default for DexConfig {
    fn default() -> Self {
        Self {
            grpc_url: "http://localhost:5557".to_string(),
            client_id: "openobserve".to_string(),
            client_secret: String::new(),
            issuer_url: "http://localhost:5556".to_string(),
            redirect_uri: "http://localhost:5080/config/redirect".to_string(),
            default_org: "default".to_string(),
            default_role: "viewer".to_string(),
            native_login_enabled: true,
            group_claim: "groups".to_string(),
            scopes: vec![
                "openid".to_string(),
                "email".to_string(),
                "profile".to_string(),
                "groups".to_string(),
                "offline_access".to_string(),
            ],
            timeout_seconds: 30,
        }
    }
}

impl DexConfig {
    /// Create a new DexConfig with the given gRPC URL
    pub fn new(grpc_url: &str) -> Self {
        Self {
            grpc_url: grpc_url.to_string(),
            ..Default::default()
        }
    }

    /// Set the issuer URL
    pub fn with_issuer(mut self, issuer_url: &str) -> Self {
        self.issuer_url = issuer_url.to_string();
        self
    }

    /// Set OAuth2 client credentials
    pub fn with_client(mut self, client_id: &str, client_secret: &str) -> Self {
        self.client_id = client_id.to_string();
        self.client_secret = client_secret.to_string();
        self
    }

    /// Set redirect URI
    pub fn with_redirect_uri(mut self, uri: &str) -> Self {
        self.redirect_uri = uri.to_string();
        self
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.grpc_url.is_empty() {
            return Err("Dex gRPC URL is required".to_string());
        }
        if self.client_id.is_empty() {
            return Err("OAuth2 client ID is required".to_string());
        }
        if self.issuer_url.is_empty() {
            return Err("OIDC issuer URL is required".to_string());
        }
        Ok(())
    }
}
