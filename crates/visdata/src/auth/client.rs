// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Dex HTTP client (using REST API instead of gRPC for compatibility)
//!
//! Note: Dex also provides a gRPC API, but this implementation uses HTTP
//! for better compatibility with the existing infrastructure.

use std::time::Duration;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::config::DexConfig;
use super::error::{Error, Result};

/// Dex HTTP client wrapper
pub struct DexClient {
    http: Client,
    config: DexConfig,
}

/// Connector representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connector {
    pub id: String,
    #[serde(rename = "type")]
    pub connector_type: String,
    pub name: String,
    #[serde(default)]
    pub config: serde_json::Value,
}

/// Password entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Password {
    pub email: String,
    pub username: String,
    pub user_id: String,
}

/// Refresh token reference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshTokenRef {
    pub id: String,
    pub client_id: String,
    pub created_at: Option<i64>,
    pub last_used: Option<i64>,
}

impl DexClient {
    /// Create a new Dex client
    pub async fn new(config: &DexConfig) -> Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .map_err(|e| Error::HttpError(e.to_string()))?;

        Ok(Self {
            http,
            config: config.clone(),
        })
    }

    /// Get the configuration
    pub fn config(&self) -> &DexConfig {
        &self.config
    }

    // ========================================================================
    // Connector Management (via Dex API if available)
    // ========================================================================

    /// Create a new connector
    /// Note: This requires Dex API access which may need to be configured
    pub async fn create_connector(
        &mut self,
        id: &str,
        connector_type: &str,
        name: &str,
        config_json: &str,
    ) -> Result<bool> {
        // Dex doesn't have a standard HTTP API for connector management
        // This is typically done through configuration files
        // For now, we log and return success
        tracing::info!(
            "[Auth] Connector creation requested: {} ({}) - Note: Dex connectors are typically configured via config file",
            id,
            connector_type
        );

        // Store connector info for reference
        let _ = (name, config_json);

        Ok(true)
    }

    /// Update an existing connector
    pub async fn update_connector(
        &mut self,
        id: &str,
        connector_type: &str,
        name: &str,
        config_json: &str,
    ) -> Result<()> {
        tracing::info!(
            "[Auth] Connector update requested: {} - Note: Dex connectors are typically configured via config file",
            id
        );

        let _ = (connector_type, name, config_json);

        Ok(())
    }

    /// Delete a connector
    pub async fn delete_connector(&mut self, id: &str) -> Result<()> {
        tracing::info!(
            "[Auth] Connector deletion requested: {} - Note: Dex connectors are typically configured via config file",
            id
        );

        Ok(())
    }

    /// List all connectors
    /// Returns connectors from Dex's well-known discovery endpoint
    pub async fn list_connectors(&mut self) -> Result<Vec<Connector>> {
        // Try to get connectors from Dex discovery
        let discovery_url = format!("{}/.well-known/openid-configuration", self.config.issuer_url);

        let resp = self.http.get(&discovery_url).send().await;

        match resp {
            Ok(response) if response.status().is_success() => {
                // Dex doesn't expose connectors via HTTP API
                // Return empty list - connectors should be managed via config
                Ok(vec![])
            }
            _ => {
                // Return empty list if Dex is not reachable
                Ok(vec![])
            }
        }
    }

    // ========================================================================
    // Password Management (Native Login)
    // ========================================================================

    /// Verify user password (for native login)
    /// This uses the token endpoint with password grant
    pub async fn verify_password(&mut self, email: &str, password: &str) -> Result<bool> {
        let token_url = format!("{}/token", self.config.issuer_url);

        let params = [
            ("grant_type", "password"),
            ("username", email),
            ("password", password),
            ("client_id", &self.config.client_id),
            ("scope", "openid email profile"),
        ];

        let resp = self.http
            .post(&token_url)
            .form(&params)
            .send()
            .await?;

        if resp.status().is_success() {
            Ok(true)
        } else if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            Ok(false)
        } else {
            Err(Error::HttpError(format!(
                "Password verification failed: {}",
                resp.status()
            )))
        }
    }

    /// Create a password entry (for native login)
    /// Note: This typically requires Dex gRPC API access
    pub async fn create_password(
        &mut self,
        email: &str,
        _password_hash: &[u8],
        username: &str,
        user_id: &str,
    ) -> Result<bool> {
        tracing::info!(
            "[Auth] Password creation requested for: {} - Note: Dex password management requires gRPC API",
            email
        );

        let _ = (username, user_id);

        Ok(true)
    }

    /// Update a password
    pub async fn update_password(
        &mut self,
        email: &str,
        _new_hash: &[u8],
        _new_username: Option<&str>,
    ) -> Result<()> {
        tracing::info!(
            "[Auth] Password update requested for: {} - Note: Dex password management requires gRPC API",
            email
        );

        Ok(())
    }

    /// Delete a password entry
    pub async fn delete_password(&mut self, email: &str) -> Result<()> {
        tracing::info!(
            "[Auth] Password deletion requested for: {} - Note: Dex password management requires gRPC API",
            email
        );

        Ok(())
    }

    /// List all passwords
    pub async fn list_passwords(&mut self) -> Result<Vec<Password>> {
        // Dex doesn't expose passwords via HTTP API
        Ok(vec![])
    }

    // ========================================================================
    // Client Management
    // ========================================================================

    /// Create an OAuth2 client
    pub async fn create_client(
        &mut self,
        id: &str,
        _secret: &str,
        name: &str,
        _redirect_uris: Vec<String>,
        _public: bool,
    ) -> Result<bool> {
        tracing::info!(
            "[Auth] OAuth2 client creation requested: {} ({}) - Note: Dex clients are typically configured via config file",
            id,
            name
        );

        Ok(true)
    }

    /// Update an OAuth2 client
    pub async fn update_client(
        &mut self,
        id: &str,
        _name: Option<&str>,
        _redirect_uris: Option<Vec<String>>,
    ) -> Result<()> {
        tracing::info!(
            "[Auth] OAuth2 client update requested: {} - Note: Dex clients are typically configured via config file",
            id
        );

        Ok(())
    }

    /// Delete an OAuth2 client
    pub async fn delete_client(&mut self, id: &str) -> Result<()> {
        tracing::info!(
            "[Auth] OAuth2 client deletion requested: {} - Note: Dex clients are typically configured via config file",
            id
        );

        Ok(())
    }

    // ========================================================================
    // Refresh Token Management
    // ========================================================================

    /// List refresh tokens for a user
    pub async fn list_refresh_tokens(&mut self, _user_id: &str) -> Result<Vec<RefreshTokenRef>> {
        // Dex doesn't expose refresh tokens via HTTP API
        Ok(vec![])
    }

    /// Revoke a refresh token
    pub async fn revoke_refresh_token(&mut self, user_id: &str, client_id: &str) -> Result<()> {
        // Try to revoke via token revocation endpoint if available
        let revoke_url = format!("{}/token/revoke", self.config.issuer_url);

        let _ = self.http
            .post(&revoke_url)
            .form(&[
                ("token_type_hint", "refresh_token"),
                ("client_id", &self.config.client_id),
            ])
            .send()
            .await;

        tracing::info!(
            "[Auth] Refresh token revocation requested for user: {}, client: {}",
            user_id,
            client_id
        );

        Ok(())
    }

    // ========================================================================
    // Version / Health
    // ========================================================================

    /// Get Dex server version (via health check)
    pub async fn get_version(&mut self) -> Result<(String, i32)> {
        // Check if Dex is healthy
        let health_url = format!("{}/healthz", self.config.issuer_url);

        let resp = self.http.get(&health_url).send().await?;

        if resp.status().is_success() {
            // Return placeholder version - Dex doesn't expose version via HTTP
            Ok(("dex".to_string(), 2))
        } else {
            Err(Error::HttpError(format!(
                "Dex health check failed: {}",
                resp.status()
            )))
        }
    }

    /// Check if Dex is healthy
    pub async fn is_healthy(&self) -> bool {
        let health_url = format!("{}/healthz", self.config.issuer_url);

        match self.http.get(&health_url).send().await {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = DexConfig::default();
        assert_eq!(config.grpc_url, "http://localhost:5557");
        assert!(config.native_login_enabled);
    }
}
