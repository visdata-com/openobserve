// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Meta types for API requests and responses
//! These structures match the existing frontend API expectations

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

// ==================== Role API ====================

/// Request to create a new role
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRoleRequest {
    pub role: String,
}

/// Request to update a role (permissions and users)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRoleRequest {
    /// Permissions to add
    #[serde(default)]
    pub add: Option<Vec<PermissionEntry>>,
    /// Permissions to remove
    #[serde(default)]
    pub remove: Option<Vec<PermissionEntry>>,
    /// Users to add to the role
    #[serde(default)]
    pub add_users: Option<HashSet<String>>,
    /// Users to remove from the role
    #[serde(default)]
    pub remove_users: Option<HashSet<String>>,
}

/// A permission entry (object + permission type)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionEntry {
    /// Object in format "resource:entity" (e.g., "logs:_all_org123")
    pub object: String,
    /// Permission type (e.g., "AllowGet", "AllowAll")
    pub permission: String,
}

/// Role response for list/get
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleResponse {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub is_system: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Role permissions response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RolePermissionsResponse {
    pub role_id: String,
    pub role_name: String,
    pub permissions: Vec<PermissionEntry>,
}

/// Role users response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleUsersResponse {
    pub role_id: String,
    pub role_name: String,
    pub users: Vec<String>,
}

// ==================== Group API ====================

/// Request to create a new group
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateGroupRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Request to update a group (roles and users)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateGroupRequest {
    /// Roles to add to the group
    #[serde(default)]
    pub add_roles: Option<HashSet<String>>,
    /// Roles to remove from the group
    #[serde(default)]
    pub remove_roles: Option<HashSet<String>>,
    /// Users to add to the group
    #[serde(default)]
    pub add_users: Option<HashSet<String>>,
    /// Users to remove from the group
    #[serde(default)]
    pub remove_users: Option<HashSet<String>>,
}

/// Group response for list/get
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupResponse {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub roles: Vec<String>,
    pub users: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

// ==================== User API ====================

/// User roles response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRolesResponse {
    pub user_email: String,
    /// Roles directly assigned to the user
    pub direct_roles: Vec<RoleResponse>,
    /// Roles inherited from groups
    pub group_roles: Vec<GroupRoleInfo>,
}

/// Group role info (role inherited from a group)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupRoleInfo {
    pub group_name: String,
    pub role: RoleResponse,
}

/// User groups response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserGroupsResponse {
    pub user_email: String,
    pub groups: Vec<GroupResponse>,
}

// ==================== SSO API ====================

/// SSO provider response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SSOProviderResponse {
    pub id: String,
    pub name: String,
    pub provider_type: String,
    pub is_enabled: bool,
    pub is_default: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Request to create/update OIDC provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OIDCProviderRequest {
    pub name: String,
    pub issuer_url: String,
    pub client_id: String,
    pub client_secret: String,
    #[serde(default)]
    pub scopes: Option<Vec<String>>,
    #[serde(default)]
    pub redirect_uri: Option<String>,
    #[serde(default)]
    pub email_claim: Option<String>,
    #[serde(default)]
    pub name_claim: Option<String>,
    #[serde(default)]
    pub groups_claim: Option<String>,
    #[serde(default)]
    pub group_role_mappings: Option<std::collections::HashMap<String, String>>,
    #[serde(default)]
    pub auto_create_users: Option<bool>,
    #[serde(default)]
    pub default_role: Option<String>,
    #[serde(default)]
    pub is_enabled: Option<bool>,
    #[serde(default)]
    pub is_default: Option<bool>,
}

/// Request to create/update LDAP provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LDAPProviderRequest {
    pub name: String,
    pub server_url: String,
    pub bind_dn: String,
    pub bind_password: String,
    pub user_base_dn: String,
    #[serde(default)]
    pub user_filter: Option<String>,
    #[serde(default)]
    pub user_attr_email: Option<String>,
    #[serde(default)]
    pub user_attr_name: Option<String>,
    #[serde(default)]
    pub group_base_dn: Option<String>,
    #[serde(default)]
    pub group_filter: Option<String>,
    #[serde(default)]
    pub group_attr_name: Option<String>,
    #[serde(default)]
    pub group_role_mappings: Option<std::collections::HashMap<String, String>>,
    #[serde(default)]
    pub auto_create_users: Option<bool>,
    #[serde(default)]
    pub default_role: Option<String>,
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
    #[serde(default)]
    pub use_ssl: Option<bool>,
    #[serde(default)]
    pub skip_ssl_verify: Option<bool>,
    #[serde(default)]
    pub is_enabled: Option<bool>,
    #[serde(default)]
    pub is_default: Option<bool>,
}

/// SSO login request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SSOLoginRequest {
    pub provider_id: Option<String>,
    pub redirect_uri: Option<String>,
}

/// SSO login response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SSOLoginResponse {
    pub auth_url: String,
    pub state: String,
}

/// SSO callback query parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SSOCallbackQuery {
    /// Authorization code from IdP
    pub code: Option<String>,
    /// State for CSRF protection
    pub state: Option<String>,
    /// Error code if authentication failed
    pub error: Option<String>,
    /// Error description
    pub error_description: Option<String>,
}

// ==================== Common Responses ====================

/// Standard success response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageResponse {
    pub message: String,
}

impl MessageResponse {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// Standard list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListResponse<T> {
    pub items: Vec<T>,
    pub total: usize,
}

impl<T> ListResponse<T> {
    pub fn new(items: Vec<T>) -> Self {
        let total = items.len();
        Self { items, total }
    }
}
