// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Authentication metadata types (compatible with o2_dex::meta::auth)
//!
//! This module provides types for role requests and entity authorization
//! that are compatible with the o2_dex::meta::auth API.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Permission enum (compatible with o2_dex::meta::auth::Permission)
///
/// Represents the different permission levels that can be granted
/// on resources. These correspond to the ALLOW_* relations in OpenFGA.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "PascalCase")]
pub enum Permission {
    /// Full access to the resource
    AllowAll,
    /// Permission to list resources
    AllowList,
    /// Permission to read/get resources
    AllowGet,
    /// Permission to create resources
    AllowPost,
    /// Permission to update resources
    AllowPut,
    /// Permission to delete resources
    AllowDelete,
}

impl Permission {
    /// Convert to OpenFGA relation name
    pub fn to_relation(&self) -> &'static str {
        match self {
            Permission::AllowAll => "ALLOW_ALL",
            Permission::AllowList => "ALLOW_LIST",
            Permission::AllowGet => "ALLOW_GET",
            Permission::AllowPost => "ALLOW_POST",
            Permission::AllowPut => "ALLOW_PUT",
            Permission::AllowDelete => "ALLOW_DELETE",
        }
    }

    /// Convert to internal relation name used in OpenFGA tuples
    pub fn to_internal_relation(&self) -> &'static str {
        match self {
            Permission::AllowAll => "admin",
            Permission::AllowList => "can_list",
            Permission::AllowGet => "can_read",
            Permission::AllowPost => "can_create",
            Permission::AllowPut => "can_update",
            Permission::AllowDelete => "can_delete",
        }
    }

    /// Create from HTTP method
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

    /// Parse from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "allowall" | "allow_all" | "admin" => Some(Permission::AllowAll),
            "allowlist" | "allow_list" | "can_list" => Some(Permission::AllowList),
            "allowget" | "allow_get" | "can_read" => Some(Permission::AllowGet),
            "allowpost" | "allow_post" | "can_create" => Some(Permission::AllowPost),
            "allowput" | "allow_put" | "can_update" => Some(Permission::AllowPut),
            "allowdelete" | "allow_delete" | "can_delete" => Some(Permission::AllowDelete),
            _ => None,
        }
    }

    /// Get all permissions
    pub fn all() -> Vec<Permission> {
        vec![
            Permission::AllowAll,
            Permission::AllowList,
            Permission::AllowGet,
            Permission::AllowPost,
            Permission::AllowPut,
            Permission::AllowDelete,
        ]
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

/// Entity authorization (compatible with o2_dex::meta::auth::O2EntityAuthorization)
///
/// Represents a permission grant on a specific object.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct O2EntityAuthorization {
    /// Object identifier in format "resource_type:entity_id"
    /// e.g., "logs:my_stream" or "dashboard:_all_default"
    pub object: String,
    /// Permission level
    pub permission: Permission,
}

impl O2EntityAuthorization {
    /// Create a new entity authorization
    pub fn new(object: impl Into<String>, permission: Permission) -> Self {
        Self {
            object: object.into(),
            permission,
        }
    }

    /// Parse the resource type from the object
    pub fn resource_type(&self) -> Option<&str> {
        self.object.split(':').next()
    }

    /// Parse the entity ID from the object
    pub fn entity_id(&self) -> Option<&str> {
        self.object.split(':').nth(1)
    }
}

/// Role request (compatible with o2_dex::meta::auth::RoleRequest)
///
/// Used for updating role permissions and user assignments.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct RoleRequest {
    /// Permissions to add to the role
    #[serde(default)]
    pub add: Vec<O2EntityAuthorization>,
    /// Permissions to remove from the role
    #[serde(default)]
    pub remove: Vec<O2EntityAuthorization>,
    /// Users to add to the role
    #[serde(default)]
    pub add_users: Option<HashSet<String>>,
    /// Users to remove from the role
    #[serde(default)]
    pub remove_users: Option<HashSet<String>>,
}

impl RoleRequest {
    /// Create an empty role request
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a permission
    pub fn with_add(mut self, authorization: O2EntityAuthorization) -> Self {
        self.add.push(authorization);
        self
    }

    /// Remove a permission
    pub fn with_remove(mut self, authorization: O2EntityAuthorization) -> Self {
        self.remove.push(authorization);
        self
    }

    /// Add users
    pub fn with_add_users(mut self, users: HashSet<String>) -> Self {
        self.add_users = Some(users);
        self
    }

    /// Remove users
    pub fn with_remove_users(mut self, users: HashSet<String>) -> Self {
        self.remove_users = Some(users);
        self
    }

    /// Check if the request is empty
    pub fn is_empty(&self) -> bool {
        self.add.is_empty()
            && self.remove.is_empty()
            && self.add_users.as_ref().map_or(true, |u| u.is_empty())
            && self.remove_users.as_ref().map_or(true, |u| u.is_empty())
    }
}

/// Group request for updating group memberships
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct GroupRequest {
    /// Users to add to the group
    #[serde(default)]
    pub add_users: Option<HashSet<String>>,
    /// Users to remove from the group
    #[serde(default)]
    pub remove_users: Option<HashSet<String>>,
    /// Roles to add to the group
    #[serde(default)]
    pub add_roles: Option<HashSet<String>>,
    /// Roles to remove from the group
    #[serde(default)]
    pub remove_roles: Option<HashSet<String>>,
}

impl GroupRequest {
    /// Create an empty group request
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if the request is empty
    pub fn is_empty(&self) -> bool {
        self.add_users.as_ref().map_or(true, |u| u.is_empty())
            && self.remove_users.as_ref().map_or(true, |u| u.is_empty())
            && self.add_roles.as_ref().map_or(true, |r| r.is_empty())
            && self.remove_roles.as_ref().map_or(true, |r| r.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_to_relation() {
        assert_eq!(Permission::AllowAll.to_relation(), "ALLOW_ALL");
        assert_eq!(Permission::AllowGet.to_relation(), "ALLOW_GET");
        assert_eq!(Permission::AllowList.to_relation(), "ALLOW_LIST");
        assert_eq!(Permission::AllowPost.to_relation(), "ALLOW_POST");
        assert_eq!(Permission::AllowPut.to_relation(), "ALLOW_PUT");
        assert_eq!(Permission::AllowDelete.to_relation(), "ALLOW_DELETE");
    }

    #[test]
    fn test_permission_from_method() {
        assert_eq!(Permission::from_method("GET", false), Permission::AllowGet);
        assert_eq!(Permission::from_method("GET", true), Permission::AllowList);
        assert_eq!(Permission::from_method("POST", false), Permission::AllowPost);
        assert_eq!(Permission::from_method("PUT", false), Permission::AllowPut);
        assert_eq!(Permission::from_method("DELETE", false), Permission::AllowDelete);
    }

    #[test]
    fn test_permission_from_str() {
        assert_eq!(Permission::from_str("AllowAll"), Some(Permission::AllowAll));
        assert_eq!(Permission::from_str("allow_get"), Some(Permission::AllowGet));
        assert_eq!(Permission::from_str("can_read"), Some(Permission::AllowGet));
        assert_eq!(Permission::from_str("invalid"), None);
    }

    #[test]
    fn test_entity_authorization() {
        let auth = O2EntityAuthorization::new("logs:my_stream", Permission::AllowGet);
        assert_eq!(auth.resource_type(), Some("logs"));
        assert_eq!(auth.entity_id(), Some("my_stream"));
    }

    #[test]
    fn test_role_request() {
        let request = RoleRequest::new()
            .with_add(O2EntityAuthorization::new("logs:test", Permission::AllowGet));

        assert!(!request.is_empty());
        assert_eq!(request.add.len(), 1);
    }
}
