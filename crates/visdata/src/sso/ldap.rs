// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! LDAP authentication provider

use async_trait::async_trait;
use ldap3::{LdapConnAsync, LdapConnSettings, Scope as LdapScope, SearchEntry};
use std::time::Duration;

use super::provider::{SSOAuthResult, SSOProvider, SSOUserInfo};
use crate::config::{LDAPConfig, SSOProviderType};
use crate::error::{Error, Result};

/// LDAP authentication provider
pub struct LDAPProvider {
    config: LDAPConfig,
}

impl LDAPProvider {
    /// Create a new LDAP provider
    pub fn new(config: LDAPConfig) -> Self {
        Self { config }
    }

    /// Create LDAP connection settings
    fn connection_settings(&self) -> LdapConnSettings {
        let mut settings = LdapConnSettings::new()
            .set_conn_timeout(Duration::from_secs(self.config.timeout_seconds));

        // use_ssl determines if we use ldaps:// or start with ldap:// and upgrade
        if !self.config.use_ssl && self.config.server_url.starts_with("ldap://") {
            // If use_ssl is false but server isn't ldaps://, try STARTTLS
            settings = settings.set_starttls(true);
        }

        if self.config.skip_ssl_verify {
            settings = settings.set_no_tls_verify(true);
        }

        settings
    }

    /// Connect to LDAP server
    async fn connect(&self) -> Result<ldap3::Ldap> {
        let settings = self.connection_settings();
        let (conn, mut ldap) = LdapConnAsync::with_settings(settings, &self.config.server_url)
            .await
            .map_err(|e| Error::LDAP(format!("Failed to connect to LDAP server: {}", e)))?;

        // Spawn the connection handler
        tokio::spawn(async move {
            if let Err(e) = conn.drive().await {
                tracing::error!("[VISDATA] LDAP connection error: {}", e);
            }
        });

        // Bind with service account
        ldap.simple_bind(&self.config.bind_dn, &self.config.bind_password)
            .await
            .map_err(|e| Error::LDAP(format!("Failed to bind to LDAP: {}", e)))?
            .success()
            .map_err(|e| Error::LDAP(format!("LDAP bind failed: {}", e)))?;

        Ok(ldap)
    }

    /// Search for a user in LDAP
    async fn search_user(&self, ldap: &mut ldap3::Ldap, username: &str) -> Result<SearchEntry> {
        // Replace {0} in filter with username
        let filter = self.config.user_filter.replace("{0}", username);

        let (rs, _res) = ldap
            .search(
                &self.config.user_base_dn,
                LdapScope::Subtree,
                &filter,
                vec![
                    &self.config.user_attr_email,
                    &self.config.user_attr_name,
                    "dn",
                ],
            )
            .await
            .map_err(|e| Error::LDAP(format!("LDAP search failed: {}", e)))?
            .success()
            .map_err(|e| Error::LDAP(format!("LDAP search error: {}", e)))?;

        let entries: Vec<SearchEntry> = rs.into_iter().map(SearchEntry::construct).collect();

        entries
            .into_iter()
            .next()
            .ok_or_else(|| Error::LDAP(format!("User '{}' not found in LDAP", username)))
    }

    /// Get user's groups from LDAP
    async fn get_user_groups(&self, ldap: &mut ldap3::Ldap, user_dn: &str) -> Result<Vec<String>> {
        // Check if group search is configured
        let group_base_dn = match &self.config.group_base_dn {
            Some(dn) => dn,
            None => return Ok(vec![]), // No group configuration, return empty
        };

        let group_filter = match &self.config.group_filter {
            Some(f) => f.replace("{0}", user_dn),
            None => format!("(member={})", user_dn), // Default filter
        };

        let (rs, _res) = ldap
            .search(
                group_base_dn,
                LdapScope::Subtree,
                &group_filter,
                vec![&self.config.group_attr_name],
            )
            .await
            .map_err(|e| Error::LDAP(format!("LDAP group search failed: {}", e)))?
            .success()
            .map_err(|e| Error::LDAP(format!("LDAP group search error: {}", e)))?;

        let groups: Vec<String> = rs
            .into_iter()
            .map(SearchEntry::construct)
            .filter_map(|entry| {
                entry
                    .attrs
                    .get(&self.config.group_attr_name)
                    .and_then(|v| v.first().cloned())
            })
            .collect();

        Ok(groups)
    }
}

#[async_trait]
impl SSOProvider for LDAPProvider {
    fn provider_type(&self) -> SSOProviderType {
        SSOProviderType::LDAP
    }

    async fn get_auth_url(&self, _state: &str, _redirect_uri: &str) -> Result<String> {
        // LDAP doesn't use authorization URL
        Err(Error::LDAP(
            "LDAP doesn't support authorization URL flow. Use direct authentication.".to_string(),
        ))
    }

    async fn exchange_code(&self, _code: &str, _redirect_uri: &str) -> Result<SSOAuthResult> {
        // LDAP doesn't use authorization code
        Err(Error::LDAP(
            "LDAP doesn't support authorization code flow. Use direct authentication.".to_string(),
        ))
    }

    async fn authenticate(&self, username: &str, password: &str) -> Result<SSOAuthResult> {
        let mut ldap = self.connect().await?;

        // Search for the user
        let user_entry = self.search_user(&mut ldap, username).await?;
        let user_dn = user_entry.dn.clone();

        // Attempt to bind with user credentials
        ldap.simple_bind(&user_dn, password)
            .await
            .map_err(|e| Error::LDAP(format!("Authentication failed: {}", e)))?
            .success()
            .map_err(|_| Error::LDAP("Invalid credentials".to_string()))?;

        // Re-bind as service account to get groups
        ldap.simple_bind(&self.config.bind_dn, &self.config.bind_password)
            .await
            .map_err(|e| Error::LDAP(format!("Failed to re-bind: {}", e)))?;

        // Get user's groups
        let groups = self.get_user_groups(&mut ldap, &user_dn).await?;

        // Extract user attributes
        let email = user_entry
            .attrs
            .get(&self.config.user_attr_email)
            .and_then(|v| v.first().cloned())
            .ok_or_else(|| {
                Error::LDAP(format!(
                    "User has no {} attribute",
                    self.config.user_attr_email
                ))
            })?;

        let name = user_entry
            .attrs
            .get(&self.config.user_attr_name)
            .and_then(|v| v.first().cloned());

        // Unbind
        let _ = ldap.unbind().await;

        Ok(SSOAuthResult {
            user: SSOUserInfo {
                external_id: user_dn,
                email,
                name,
                groups,
            },
            access_token: None,
            refresh_token: None,
        })
    }

    async fn test_connection(&self) -> Result<()> {
        let mut ldap = self.connect().await?;

        // Try a simple search to verify connectivity
        let _ = ldap
            .search(
                &self.config.user_base_dn,
                LdapScope::Base,
                "(objectClass=*)",
                vec!["dn"],
            )
            .await
            .map_err(|e| Error::LDAP(format!("LDAP test search failed: {}", e)))?
            .success()
            .map_err(|e| Error::LDAP(format!("LDAP test error: {}", e)))?;

        let _ = ldap.unbind().await;

        Ok(())
    }
}
