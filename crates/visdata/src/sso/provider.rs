// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! SSO Provider management

use async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
};
use std::sync::Arc;
use svix_ksuid::KsuidLike;

use crate::config::{LDAPConfig, OIDCConfig, SSOProviderType};
use crate::entity::vd_sso_providers;
use crate::error::{Error, Result};

/// User information from SSO authentication
#[derive(Debug, Clone)]
pub struct SSOUserInfo {
    /// External user ID from the provider
    pub external_id: String,
    /// User's email address
    pub email: String,
    /// User's display name
    pub name: Option<String>,
    /// Groups the user belongs to (if available)
    pub groups: Vec<String>,
}

/// SSO authentication result
#[derive(Debug, Clone)]
pub struct SSOAuthResult {
    /// User information
    pub user: SSOUserInfo,
    /// Access token (for OIDC)
    pub access_token: Option<String>,
    /// Refresh token (for OIDC)
    pub refresh_token: Option<String>,
}

/// SSO Provider trait
#[async_trait]
pub trait SSOProvider: Send + Sync {
    /// Get the provider type
    fn provider_type(&self) -> SSOProviderType;

    /// Get the authorization URL for initiating login
    async fn get_auth_url(&self, state: &str, redirect_uri: &str) -> Result<String>;

    /// Exchange authorization code for user info (OIDC callback)
    async fn exchange_code(&self, code: &str, redirect_uri: &str) -> Result<SSOAuthResult>;

    /// Authenticate user with credentials (LDAP)
    async fn authenticate(&self, username: &str, password: &str) -> Result<SSOAuthResult>;

    /// Test the connection to the provider
    async fn test_connection(&self) -> Result<()>;
}

/// SSO Provider Manager
pub struct SSOProviderManager {
    db: Arc<DatabaseConnection>,
    encryption_key: Option<Vec<u8>>,
}

impl SSOProviderManager {
    /// Create a new SSO provider manager
    pub fn new(db: Arc<DatabaseConnection>, encryption_key: Option<Vec<u8>>) -> Self {
        Self { db, encryption_key }
    }

    /// List all providers for an organization
    pub async fn list_providers(&self, org_id: &str) -> Result<Vec<vd_sso_providers::Model>> {
        let providers = vd_sso_providers::Entity::find()
            .filter(vd_sso_providers::Column::OrgId.eq(org_id))
            .all(self.db.as_ref())
            .await?;
        Ok(providers)
    }

    /// Get a provider by ID
    pub async fn get_provider(
        &self,
        org_id: &str,
        provider_id: &str,
    ) -> Result<Option<vd_sso_providers::Model>> {
        let provider = vd_sso_providers::Entity::find_by_id(provider_id)
            .filter(vd_sso_providers::Column::OrgId.eq(org_id))
            .one(self.db.as_ref())
            .await?;
        Ok(provider)
    }

    /// Get the default provider for an organization
    pub async fn get_default_provider(
        &self,
        org_id: &str,
    ) -> Result<Option<vd_sso_providers::Model>> {
        let provider = vd_sso_providers::Entity::find()
            .filter(vd_sso_providers::Column::OrgId.eq(org_id))
            .filter(vd_sso_providers::Column::IsDefault.eq(true))
            .filter(vd_sso_providers::Column::IsEnabled.eq(true))
            .one(self.db.as_ref())
            .await?;
        Ok(provider)
    }

    /// Create an OIDC provider
    pub async fn create_oidc_provider(
        &self,
        org_id: &str,
        name: &str,
        config: OIDCConfig,
        is_default: bool,
    ) -> Result<vd_sso_providers::Model> {
        // Encrypt sensitive data
        let config_json = self.encrypt_config(&serde_json::to_string(&config)?)?;

        let now = chrono::Utc::now().timestamp_micros();
        let provider = vd_sso_providers::ActiveModel {
            id: ActiveValue::Set(svix_ksuid::Ksuid::new(None, None).to_string()),
            org_id: ActiveValue::Set(org_id.to_string()),
            provider_type: ActiveValue::Set(SSOProviderType::OIDC.to_string()),
            name: ActiveValue::Set(name.to_string()),
            is_enabled: ActiveValue::Set(true),
            is_default: ActiveValue::Set(is_default),
            config_json: ActiveValue::Set(config_json),
            created_at: ActiveValue::Set(now),
            updated_at: ActiveValue::Set(now),
        };

        let provider = provider.insert(self.db.as_ref()).await?;
        Ok(provider)
    }

    /// Create an LDAP provider
    pub async fn create_ldap_provider(
        &self,
        org_id: &str,
        name: &str,
        config: LDAPConfig,
        is_default: bool,
    ) -> Result<vd_sso_providers::Model> {
        // Encrypt sensitive data
        let config_json = self.encrypt_config(&serde_json::to_string(&config)?)?;

        let now = chrono::Utc::now().timestamp_micros();
        let provider = vd_sso_providers::ActiveModel {
            id: ActiveValue::Set(svix_ksuid::Ksuid::new(None, None).to_string()),
            org_id: ActiveValue::Set(org_id.to_string()),
            provider_type: ActiveValue::Set(SSOProviderType::LDAP.to_string()),
            name: ActiveValue::Set(name.to_string()),
            is_enabled: ActiveValue::Set(true),
            is_default: ActiveValue::Set(is_default),
            config_json: ActiveValue::Set(config_json),
            created_at: ActiveValue::Set(now),
            updated_at: ActiveValue::Set(now),
        };

        let provider = provider.insert(self.db.as_ref()).await?;
        Ok(provider)
    }

    /// Delete a provider
    pub async fn delete_provider(&self, org_id: &str, provider_id: &str) -> Result<()> {
        vd_sso_providers::Entity::delete_by_id(provider_id)
            .filter(vd_sso_providers::Column::OrgId.eq(org_id))
            .exec(self.db.as_ref())
            .await?;
        Ok(())
    }

    /// Get OIDC config from provider
    pub fn get_oidc_config(&self, provider: &vd_sso_providers::Model) -> Result<OIDCConfig> {
        if provider.provider_type != SSOProviderType::OIDC.to_string() {
            return Err(Error::Config(format!(
                "Provider '{}' is not an OIDC provider",
                provider.name
            )));
        }

        let decrypted = self.decrypt_config(&provider.config_json)?;
        let config: OIDCConfig = serde_json::from_str(&decrypted)?;
        Ok(config)
    }

    /// Get LDAP config from provider
    pub fn get_ldap_config(&self, provider: &vd_sso_providers::Model) -> Result<LDAPConfig> {
        if provider.provider_type != SSOProviderType::LDAP.to_string() {
            return Err(Error::Config(format!(
                "Provider '{}' is not an LDAP provider",
                provider.name
            )));
        }

        let decrypted = self.decrypt_config(&provider.config_json)?;
        let config: LDAPConfig = serde_json::from_str(&decrypted)?;
        Ok(config)
    }

    /// Encrypt configuration JSON
    fn encrypt_config(&self, config: &str) -> Result<String> {
        match &self.encryption_key {
            Some(key) => {
                use aes_gcm::{
                    aead::{Aead, KeyInit},
                    Aes256Gcm, Nonce,
                };
                use base64::{engine::general_purpose::STANDARD, Engine};
                use rand::RngCore;

                let cipher = Aes256Gcm::new_from_slice(key)
                    .map_err(|e| Error::Encryption(e.to_string()))?;

                let mut nonce_bytes = [0u8; 12];
                rand::rng().fill_bytes(&mut nonce_bytes);
                let nonce = Nonce::from_slice(&nonce_bytes);

                let ciphertext = cipher
                    .encrypt(nonce, config.as_bytes())
                    .map_err(|e| Error::Encryption(e.to_string()))?;

                // Prepend nonce to ciphertext
                let mut result = nonce_bytes.to_vec();
                result.extend(ciphertext);

                Ok(STANDARD.encode(result))
            }
            None => {
                // No encryption key, store as plain text (not recommended for production)
                Ok(config.to_string())
            }
        }
    }

    /// Decrypt configuration JSON
    fn decrypt_config(&self, encrypted: &str) -> Result<String> {
        match &self.encryption_key {
            Some(key) => {
                use aes_gcm::{
                    aead::{Aead, KeyInit},
                    Aes256Gcm, Nonce,
                };
                use base64::{engine::general_purpose::STANDARD, Engine};

                let data = STANDARD
                    .decode(encrypted)
                    .map_err(|e| Error::Encryption(e.to_string()))?;

                if data.len() < 12 {
                    return Err(Error::Encryption("Invalid encrypted data".to_string()));
                }

                let (nonce_bytes, ciphertext) = data.split_at(12);
                let nonce = Nonce::from_slice(nonce_bytes);

                let cipher = Aes256Gcm::new_from_slice(key)
                    .map_err(|e| Error::Encryption(e.to_string()))?;

                let plaintext = cipher
                    .decrypt(nonce, ciphertext)
                    .map_err(|e| Error::Encryption(e.to_string()))?;

                String::from_utf8(plaintext).map_err(|e| Error::Encryption(e.to_string()))
            }
            None => {
                // No encryption key, assume plain text
                Ok(encrypted.to_string())
            }
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Config(err.to_string())
    }
}
