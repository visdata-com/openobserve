// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Configuration for VisData module

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Main configuration for VisData module
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisdataConfig {
    /// Whether RBAC is enabled
    pub rbac_enabled: bool,
    /// Whether SSO is enabled
    pub sso_enabled: bool,
    /// Cache configuration
    pub cache: CacheConfig,
    /// Encryption key for sensitive data (base64 encoded, 32 bytes for AES-256)
    pub encryption_key: Option<String>,

    // ========================================================================
    // Enterprise Configuration (OpenFGA + Dex)
    // ========================================================================

    /// OpenFGA HTTP API URL
    #[serde(default = "default_openfga_url")]
    pub openfga_url: String,

    /// OpenFGA store name
    #[serde(default = "default_openfga_store_name")]
    pub openfga_store_name: String,

    /// Dex gRPC URL
    #[serde(default = "default_dex_grpc_url")]
    pub dex_grpc_url: String,

    /// Dex OIDC issuer URL
    #[serde(default = "default_dex_issuer_url")]
    pub dex_issuer_url: String,

    /// Dex OAuth2 client ID
    #[serde(default = "default_dex_client_id")]
    pub dex_client_id: String,

    /// Dex OAuth2 client secret
    #[serde(default)]
    pub dex_client_secret: String,

    /// Dex OAuth2 redirect URI
    #[serde(default = "default_dex_redirect_uri")]
    pub dex_redirect_uri: String,

    // ========================================================================
    // Log Patterns Configuration
    // ========================================================================

    /// Maximum number of logs to analyze for pattern extraction
    #[serde(default = "default_log_patterns_max_logs")]
    pub log_patterns_max_logs: usize,

    /// Minimum cluster size for a pattern to be considered valid
    #[serde(default = "default_log_patterns_min_cluster_size")]
    pub log_patterns_min_cluster_size: usize,

    /// Similarity threshold for grouping logs (0.0-1.0)
    #[serde(default = "default_log_patterns_similarity_threshold")]
    pub log_patterns_similarity_threshold: f64,

    /// Drain algorithm tree depth
    #[serde(default = "default_log_patterns_drain_depth")]
    pub log_patterns_drain_depth: usize,

    /// Maximum child nodes per tree node
    #[serde(default = "default_log_patterns_drain_max_child")]
    pub log_patterns_drain_max_child: usize,

    /// Maximum number of clusters/patterns to extract
    #[serde(default = "default_log_patterns_max_clusters")]
    pub log_patterns_max_clusters: usize,
}

fn default_openfga_url() -> String {
    "http://localhost:8080".to_string()
}

fn default_openfga_store_name() -> String {
    "openobserve".to_string()
}

fn default_dex_grpc_url() -> String {
    "http://localhost:5557".to_string()
}

fn default_dex_issuer_url() -> String {
    "http://localhost:5556".to_string()
}

fn default_dex_client_id() -> String {
    "openobserve".to_string()
}

fn default_dex_redirect_uri() -> String {
    "http://localhost:5080/config/redirect".to_string()
}

// Log Patterns defaults
fn default_log_patterns_max_logs() -> usize {
    10000
}

fn default_log_patterns_min_cluster_size() -> usize {
    2
}

fn default_log_patterns_similarity_threshold() -> f64 {
    0.6
}

fn default_log_patterns_drain_depth() -> usize {
    4
}

fn default_log_patterns_drain_max_child() -> usize {
    100
}

fn default_log_patterns_max_clusters() -> usize {
    1000
}

impl Default for VisdataConfig {
    fn default() -> Self {
        Self {
            rbac_enabled: true,
            sso_enabled: true,
            cache: CacheConfig::default(),
            encryption_key: None,
            // Enterprise defaults
            openfga_url: default_openfga_url(),
            openfga_store_name: default_openfga_store_name(),
            dex_grpc_url: default_dex_grpc_url(),
            dex_issuer_url: default_dex_issuer_url(),
            dex_client_id: default_dex_client_id(),
            dex_client_secret: String::new(),
            dex_redirect_uri: default_dex_redirect_uri(),
            // Log Patterns defaults
            log_patterns_max_logs: default_log_patterns_max_logs(),
            log_patterns_min_cluster_size: default_log_patterns_min_cluster_size(),
            log_patterns_similarity_threshold: default_log_patterns_similarity_threshold(),
            log_patterns_drain_depth: default_log_patterns_drain_depth(),
            log_patterns_drain_max_child: default_log_patterns_drain_max_child(),
            log_patterns_max_clusters: default_log_patterns_max_clusters(),
        }
    }
}

/// Cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Whether permission cache is enabled
    pub enabled: bool,
    /// Cache TTL in seconds
    pub ttl_seconds: u64,
    /// Maximum number of entries in cache
    pub max_entries: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            ttl_seconds: 300, // 5 minutes
            max_entries: 10000,
        }
    }
}

/// OIDC provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OIDCConfig {
    /// OIDC issuer URL
    pub issuer_url: String,
    /// Client ID
    pub client_id: String,
    /// Client secret (encrypted in database)
    pub client_secret: String,
    /// Scopes to request
    #[serde(default = "default_oidc_scopes")]
    pub scopes: Vec<String>,
    /// Redirect URI after authentication
    pub redirect_uri: String,
    /// Claim name for email
    #[serde(default = "default_email_claim")]
    pub email_claim: String,
    /// Claim name for display name
    #[serde(default = "default_name_claim")]
    pub name_claim: String,
    /// Claim name for groups (optional)
    pub groups_claim: Option<String>,
    /// Mapping from external groups to internal roles
    #[serde(default)]
    pub group_role_mappings: HashMap<String, String>,
    /// Whether to auto-create users on first login
    #[serde(default = "default_true")]
    pub auto_create_users: bool,
    /// Default role for auto-created users
    pub default_role: Option<String>,
}

fn default_oidc_scopes() -> Vec<String> {
    vec![
        "openid".to_string(),
        "profile".to_string(),
        "email".to_string(),
    ]
}

fn default_email_claim() -> String {
    "email".to_string()
}

fn default_name_claim() -> String {
    "name".to_string()
}

fn default_true() -> bool {
    true
}

/// LDAP provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LDAPConfig {
    /// LDAP server URL (e.g., ldap://ldap.example.com:389 or ldaps://...)
    pub server_url: String,
    /// Bind DN for connecting to LDAP
    pub bind_dn: String,
    /// Bind password (encrypted in database)
    pub bind_password: String,
    /// Base DN for user searches
    pub user_base_dn: String,
    /// LDAP filter for user searches (use {username} for username placeholder)
    #[serde(default = "default_user_filter")]
    pub user_filter: String,
    /// Attribute name for user email
    #[serde(default = "default_ldap_email_attr")]
    pub user_attr_email: String,
    /// Attribute name for user display name
    #[serde(default = "default_ldap_name_attr")]
    pub user_attr_name: String,
    /// Base DN for group searches (optional)
    #[serde(default)]
    pub group_base_dn: Option<String>,
    /// LDAP filter for group searches
    #[serde(default)]
    pub group_filter: Option<String>,
    /// Attribute name for group name
    #[serde(default = "default_ldap_group_attr")]
    pub group_attr_name: String,
    /// Mapping from LDAP groups to internal roles
    #[serde(default)]
    pub group_role_mappings: HashMap<String, String>,
    /// Whether to use SSL/TLS
    #[serde(default = "default_true")]
    pub use_ssl: bool,
    /// Skip TLS certificate verification (not recommended for production)
    #[serde(default)]
    pub skip_ssl_verify: bool,
    /// Connection timeout in seconds
    #[serde(default = "default_ldap_timeout")]
    pub timeout_seconds: u64,
}

fn default_user_filter() -> String {
    "(&(objectClass=person)(uid={0}))".to_string()
}

fn default_ldap_email_attr() -> String {
    "mail".to_string()
}

fn default_ldap_name_attr() -> String {
    "cn".to_string()
}

// Default group filter - kept for potential future use with serde
#[allow(dead_code)]
fn default_group_filter() -> String {
    "(&(objectClass=groupOfNames)(member={0}))".to_string()
}

fn default_ldap_group_attr() -> String {
    "cn".to_string()
}

fn default_ldap_timeout() -> u64 {
    10
}

/// SSO provider type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SSOProviderType {
    OIDC,
    LDAP,
}

impl std::fmt::Display for SSOProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SSOProviderType::OIDC => write!(f, "oidc"),
            SSOProviderType::LDAP => write!(f, "ldap"),
        }
    }
}

impl std::str::FromStr for SSOProviderType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "oidc" => Ok(SSOProviderType::OIDC),
            "ldap" => Ok(SSOProviderType::LDAP),
            _ => Err(format!("Invalid SSO provider type: {}", s)),
        }
    }
}
