// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! OIDC (OpenID Connect) authentication provider
//!
//! This module provides OIDC authentication using the openidconnect crate.

use async_trait::async_trait;
use openidconnect::{
    core::{CoreClient, CoreProviderMetadata, CoreResponseType},
    reqwest::async_http_client,
    AuthenticationFlow, AuthorizationCode, ClientId, ClientSecret, CsrfToken, IssuerUrl, Nonce,
    OAuth2TokenResponse, RedirectUrl, Scope, TokenResponse,
};

use super::provider::{SSOAuthResult, SSOProvider, SSOUserInfo};
use crate::config::{OIDCConfig, SSOProviderType};
use crate::error::{Error, Result};

/// OIDC authentication provider
pub struct OIDCProvider {
    config: OIDCConfig,
}

impl OIDCProvider {
    /// Create a new OIDC provider
    pub fn new(config: OIDCConfig) -> Self {
        Self { config }
    }

    /// Get the configuration
    pub fn config(&self) -> &OIDCConfig {
        &self.config
    }

    /// Create a new OIDC client
    async fn create_client(&self) -> Result<CoreClient> {
        let issuer_url = IssuerUrl::new(self.config.issuer_url.clone())
            .map_err(|e| Error::OIDC(format!("Invalid issuer URL: {}", e)))?;

        let provider_metadata =
            CoreProviderMetadata::discover_async(issuer_url, async_http_client)
                .await
                .map_err(|e| Error::OIDC(format!("Failed to discover OIDC provider: {}", e)))?;

        let client = CoreClient::from_provider_metadata(
            provider_metadata,
            ClientId::new(self.config.client_id.clone()),
            Some(ClientSecret::new(self.config.client_secret.clone())),
        );

        Ok(client)
    }
}

#[async_trait]
impl SSOProvider for OIDCProvider {
    fn provider_type(&self) -> SSOProviderType {
        SSOProviderType::OIDC
    }

    async fn get_auth_url(&self, state: &str, redirect_uri: &str) -> Result<String> {
        let client = self.create_client().await?;

        let redirect_url = RedirectUrl::new(redirect_uri.to_string())
            .map_err(|e| Error::OIDC(format!("Invalid redirect URI: {}", e)))?;

        let state_owned = state.to_string();

        let mut auth_request = client
            .authorize_url(
                AuthenticationFlow::<CoreResponseType>::AuthorizationCode,
                move || CsrfToken::new(state_owned.clone()),
                Nonce::new_random,
            )
            .set_redirect_uri(std::borrow::Cow::Owned(redirect_url));

        for scope in &self.config.scopes {
            auth_request = auth_request.add_scope(Scope::new(scope.clone()));
        }

        let (auth_url, _csrf_token, _nonce) = auth_request.url();

        Ok(auth_url.to_string())
    }

    async fn exchange_code(&self, code: &str, redirect_uri: &str) -> Result<SSOAuthResult> {
        let client = self.create_client().await?;

        let redirect_url = RedirectUrl::new(redirect_uri.to_string())
            .map_err(|e| Error::OIDC(format!("Invalid redirect URI: {}", e)))?;

        let token_response = client
            .exchange_code(AuthorizationCode::new(code.to_string()))
            .set_redirect_uri(std::borrow::Cow::Owned(redirect_url))
            .request_async(async_http_client)
            .await
            .map_err(|e| Error::OIDC(format!("Failed to exchange code: {}", e)))?;

        let id_token = token_response
            .id_token()
            .ok_or_else(|| Error::OIDC("No ID token in response".to_string()))?;

        let claims = id_token
            .claims(&client.id_token_verifier(), |_: Option<&Nonce>| Ok(()))
            .map_err(|e| Error::OIDC(format!("Failed to verify ID token: {}", e)))?;

        let subject = claims.subject().to_string();

        let email = claims
            .email()
            .map(|e| e.as_str().to_string())
            .ok_or_else(|| Error::OIDC("No email claim in ID token".to_string()))?;

        let name: Option<String> = claims
            .name()
            .and_then(|names| names.get(None).map(|n| n.as_str().to_string()));

        Ok(SSOAuthResult {
            user: SSOUserInfo {
                external_id: subject,
                email,
                name,
                groups: vec![],
            },
            access_token: Some(token_response.access_token().secret().clone()),
            refresh_token: token_response
                .refresh_token()
                .map(|t| t.secret().clone()),
        })
    }

    async fn authenticate(&self, _username: &str, _password: &str) -> Result<SSOAuthResult> {
        Err(Error::OIDC(
            "OIDC doesn't support direct credential authentication. Use authorization code flow."
                .to_string(),
        ))
    }

    async fn test_connection(&self) -> Result<()> {
        let issuer_url = IssuerUrl::new(self.config.issuer_url.clone())
            .map_err(|e| Error::OIDC(format!("Invalid issuer URL: {}", e)))?;

        CoreProviderMetadata::discover_async(issuer_url, async_http_client)
            .await
            .map_err(|e| Error::OIDC(format!("Failed to connect to OIDC provider: {}", e)))?;

        Ok(())
    }
}
