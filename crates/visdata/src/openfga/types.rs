// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! RBAC types compatible with existing API formats

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

// ============================================================================
// OpenFGA Types
// ============================================================================

/// OpenFGA tuple key
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct TupleKey {
    pub user: String,
    pub relation: String,
    pub object: String,
}

impl TupleKey {
    pub fn new(user: impl Into<String>, relation: impl Into<String>, object: impl Into<String>) -> Self {
        Self {
            user: user.into(),
            relation: relation.into(),
            object: object.into(),
        }
    }
}

/// OpenFGA tuple with optional condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tuple {
    pub key: TupleKey,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
}

/// OpenFGA check request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckRequest {
    pub tuple_key: TupleKey,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_model_id: Option<String>,
}

/// OpenFGA check response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResponse {
    pub allowed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
}

/// OpenFGA write request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub writes: Option<TupleKeys>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deletes: Option<TupleKeys>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_model_id: Option<String>,
}

/// Wrapper for tuple keys array
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TupleKeys {
    pub tuple_keys: Vec<TupleKey>,
}

/// OpenFGA list objects request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListObjectsRequest {
    pub user: String,
    pub relation: String,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_model_id: Option<String>,
}

/// OpenFGA list objects response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListObjectsResponse {
    pub objects: Vec<String>,
}

/// OpenFGA read request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tuple_key: Option<TupleKeyFilter>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub continuation_token: Option<String>,
}

/// Tuple key filter for read operations
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TupleKeyFilter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
}

/// OpenFGA read response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadResponse {
    pub tuples: Vec<Tuple>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub continuation_token: Option<String>,
}

/// OpenFGA store
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Store {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// Create store request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateStoreRequest {
    pub name: String,
}

/// Create store response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateStoreResponse {
    pub id: String,
    pub name: String,
}

/// List stores response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListStoresResponse {
    pub stores: Vec<Store>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub continuation_token: Option<String>,
}

// ============================================================================
// API Request/Response Types (Compatible with existing API)
// ============================================================================

/// Create role request (compatible with existing API)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRoleRequest {
    pub role: String,
}

/// Update role request (compatible with existing API)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRoleRequest {
    #[serde(default)]
    pub add: Option<Vec<PermissionEntry>>,
    #[serde(default)]
    pub remove: Option<Vec<PermissionEntry>>,
    #[serde(default)]
    pub add_users: Option<HashSet<String>>,
    #[serde(default)]
    pub remove_users: Option<HashSet<String>>,
}

/// Permission entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct PermissionEntry {
    /// Resource object in format "resource:entity" (e.g., "logs:my_stream")
    pub object: String,
    /// Permission type: AllowAll, AllowList, AllowGet, AllowPost, AllowPut, AllowDelete
    pub permission: String,
}

/// Create group request (compatible with existing API)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateGroupRequest {
    pub name: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

/// Update group request (compatible with existing API)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateGroupRequest {
    #[serde(default)]
    pub add_roles: Option<HashSet<String>>,
    #[serde(default)]
    pub remove_roles: Option<HashSet<String>>,
    #[serde(default)]
    pub add_users: Option<HashSet<String>>,
    #[serde(default)]
    pub remove_users: Option<HashSet<String>>,
}

/// Group response (compatible with existing API)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupResponse {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub roles: Vec<String>,
    pub users: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// User role option for dropdown (compatible with existing API)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRoleOption {
    pub label: String,
    pub value: String,
}

/// Role response (compatible with existing API)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleResponse {
    pub name: String,
    pub label: String,
    pub users: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Resource definition (compatible with OFGA_MODELS)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub key: String,
    /// Display name (serialized as display_name for frontend compatibility)
    #[serde(rename = "display_name")]
    pub label: String,
    #[serde(default)]
    pub parent: Option<String>,
    pub order: i32,
    pub visible: bool,
    /// Whether this is a top-level resource (no parent)
    #[serde(default)]
    pub top_level: bool,
    /// Whether this resource can have entity instances
    #[serde(default)]
    pub has_entities: bool,
}

// ============================================================================
// Permission Types
// ============================================================================

/// Permission type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Permission {
    AllowAll,
    AllowList,
    AllowGet,
    AllowPost,
    AllowPut,
    AllowDelete,
}

impl Permission {
    /// Convert from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "allowall" | "allow_all" => Some(Permission::AllowAll),
            "allowlist" | "allow_list" => Some(Permission::AllowList),
            "allowget" | "allow_get" => Some(Permission::AllowGet),
            "allowpost" | "allow_post" => Some(Permission::AllowPost),
            "allowput" | "allow_put" => Some(Permission::AllowPut),
            "allowdelete" | "allow_delete" => Some(Permission::AllowDelete),
            _ => None,
        }
    }

    /// Convert to OpenFGA relation name
    pub fn to_relation(&self) -> &'static str {
        match self {
            Permission::AllowAll => "admin",
            Permission::AllowList => "can_list",
            Permission::AllowGet => "can_read",
            Permission::AllowPost => "can_create",
            Permission::AllowPut => "can_update",
            Permission::AllowDelete => "can_delete",
        }
    }

    /// Convert from HTTP method
    pub fn from_method(method: &str, is_list: bool) -> Self {
        match method.to_uppercase().as_str() {
            "GET" if is_list => Permission::AllowList,
            "GET" => Permission::AllowGet,
            "POST" => Permission::AllowPost,
            "PUT" | "PATCH" => Permission::AllowPut,
            "DELETE" => Permission::AllowDelete,
            _ => Permission::AllowGet,
        }
    }

    /// Check if this permission implies another permission
    pub fn implies(&self, other: &Permission) -> bool {
        match self {
            Permission::AllowAll => true,
            _ => self == other,
        }
    }
}

impl std::fmt::Display for Permission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Permission::AllowAll => write!(f, "AllowAll"),
            Permission::AllowList => write!(f, "AllowList"),
            Permission::AllowGet => write!(f, "AllowGet"),
            Permission::AllowPost => write!(f, "AllowPost"),
            Permission::AllowPut => write!(f, "AllowPut"),
            Permission::AllowDelete => write!(f, "AllowDelete"),
        }
    }
}
