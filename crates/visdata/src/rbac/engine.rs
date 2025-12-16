// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! RBAC Engine - Core permission checking logic

use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect,
};
use std::sync::Arc;

use super::cache::{CachedUserRoles, PermissionCache, PermissionCacheKey};
use super::resources::Permission;
use crate::config::CacheConfig;
use crate::entity::{
    vd_group_roles, vd_group_users, vd_role_permissions, vd_role_users, vd_roles,
};
use crate::error::{Error, Result};

/// RBAC Engine for permission checking
pub struct RBACEngine {
    db: Arc<DatabaseConnection>,
    cache: Arc<PermissionCache>,
}

impl RBACEngine {
    /// Create a new RBAC engine
    pub async fn new(db: Arc<DatabaseConnection>) -> Result<Self> {
        let cache = Arc::new(PermissionCache::new(CacheConfig::default()));
        Ok(Self { db, cache })
    }

    /// Create a new RBAC engine with custom cache config
    pub async fn with_cache_config(
        db: Arc<DatabaseConnection>,
        cache_config: CacheConfig,
    ) -> Result<Self> {
        let cache = Arc::new(PermissionCache::new(cache_config));
        Ok(Self { db, cache })
    }

    /// Get the permission cache
    pub fn cache(&self) -> &Arc<PermissionCache> {
        &self.cache
    }

    /// Check if a user has permission for an object
    ///
    /// # Arguments
    /// * `org_id` - Organization ID
    /// * `user_email` - User email
    /// * `object` - Object in format "resource:entity" (e.g., "logs:my_stream")
    /// * `permission` - Permission to check (e.g., "AllowGet")
    ///
    /// # Returns
    /// * `true` if user has permission, `false` otherwise
    pub async fn check_permission(
        &self,
        org_id: &str,
        user_email: &str,
        object: &str,
        permission: &str,
    ) -> Result<bool> {
        // Parse permission
        let required_permission: Permission = permission.parse()?;

        // Check cache first
        let cache_key = PermissionCacheKey::new(org_id, user_email, object, permission);
        if let Some(cached) = self.cache.get_permission(&cache_key) {
            return Ok(cached);
        }

        // Get user's effective roles
        let user_roles = self.get_user_roles(org_id, user_email).await?;
        let all_role_ids: Vec<&str> = user_roles
            .direct_role_ids
            .iter()
            .chain(user_roles.group_role_ids.iter())
            .map(|s| s.as_str())
            .collect();

        if all_role_ids.is_empty() {
            self.cache.set_permission(cache_key, false);
            return Ok(false);
        }

        // Check permissions for each role
        let allowed = self
            .check_roles_permission(&all_role_ids, org_id, object, &required_permission)
            .await?;

        // Cache result
        self.cache.set_permission(cache_key, allowed);

        Ok(allowed)
    }

    /// Get all role IDs for a user (direct + from groups)
    async fn get_user_roles(&self, org_id: &str, user_email: &str) -> Result<CachedUserRoles> {
        // Check cache
        if let Some(cached) = self.cache.get_user_roles(org_id, user_email) {
            return Ok(cached);
        }

        // Get direct role assignments
        let direct_roles: Vec<String> = vd_role_users::Entity::find()
            .filter(vd_role_users::Column::OrgId.eq(org_id))
            .filter(vd_role_users::Column::UserEmail.eq(user_email))
            .select_only()
            .column(vd_role_users::Column::RoleId)
            .into_tuple()
            .all(self.db.as_ref())
            .await?;

        // Get group IDs the user belongs to
        let group_ids: Vec<String> = vd_group_users::Entity::find()
            .filter(vd_group_users::Column::OrgId.eq(org_id))
            .filter(vd_group_users::Column::UserEmail.eq(user_email))
            .select_only()
            .column(vd_group_users::Column::GroupId)
            .into_tuple()
            .all(self.db.as_ref())
            .await?;

        // Get roles from groups
        let group_roles: Vec<String> = if group_ids.is_empty() {
            vec![]
        } else {
            vd_group_roles::Entity::find()
                .filter(vd_group_roles::Column::GroupId.is_in(group_ids))
                .select_only()
                .column(vd_group_roles::Column::RoleId)
                .into_tuple()
                .all(self.db.as_ref())
                .await?
        };

        let cached = CachedUserRoles {
            direct_role_ids: direct_roles,
            group_role_ids: group_roles,
        };

        // Cache result
        self.cache.set_user_roles(org_id, user_email, cached.clone());

        Ok(cached)
    }

    /// Check if any of the roles has the required permission
    async fn check_roles_permission(
        &self,
        role_ids: &[&str],
        org_id: &str,
        object: &str,
        required_permission: &Permission,
    ) -> Result<bool> {
        // Parse object to get resource and entity
        let (resource, _entity) = parse_object(object)?;
        let all_object = format!("{}:_all_{}", resource, org_id);

        for role_id in role_ids {
            let permissions = self.get_role_permissions(role_id).await?;

            for (perm_object, perm_str) in &permissions {
                // Check if object matches (exact or wildcard)
                let object_matches = perm_object == object || perm_object == &all_object;

                if object_matches {
                    // Parse and check permission
                    if let Ok(perm) = perm_str.parse::<Permission>() {
                        if perm.grants(required_permission) {
                            return Ok(true);
                        }
                    }
                }
            }
        }

        Ok(false)
    }

    /// Get all permissions for a role
    async fn get_role_permissions(&self, role_id: &str) -> Result<Vec<(String, String)>> {
        // Check cache
        if let Some(cached) = self.cache.get_role_permissions(role_id) {
            return Ok(cached);
        }

        // Query database
        let permissions: Vec<(String, String)> = vd_role_permissions::Entity::find()
            .filter(vd_role_permissions::Column::RoleId.eq(role_id))
            .select_only()
            .column(vd_role_permissions::Column::Object)
            .column(vd_role_permissions::Column::Permission)
            .into_tuple()
            .all(self.db.as_ref())
            .await?;

        // Cache result
        self.cache.set_role_permissions(role_id, permissions.clone());

        Ok(permissions)
    }

    // ==================== Role Management ====================

    /// Get a role by ID
    pub async fn get_role(&self, org_id: &str, role_id: &str) -> Result<Option<vd_roles::Model>> {
        let role = vd_roles::Entity::find_by_id(role_id)
            .filter(vd_roles::Column::OrgId.eq(org_id))
            .one(self.db.as_ref())
            .await?;
        Ok(role)
    }

    /// Get a role by name
    pub async fn get_role_by_name(
        &self,
        org_id: &str,
        name: &str,
    ) -> Result<Option<vd_roles::Model>> {
        let role = vd_roles::Entity::find()
            .filter(vd_roles::Column::OrgId.eq(org_id))
            .filter(vd_roles::Column::Name.eq(name))
            .one(self.db.as_ref())
            .await?;
        Ok(role)
    }

    /// List all roles in an organization
    pub async fn list_roles(&self, org_id: &str) -> Result<Vec<vd_roles::Model>> {
        let roles = vd_roles::Entity::find()
            .filter(vd_roles::Column::OrgId.eq(org_id))
            .all(self.db.as_ref())
            .await?;
        Ok(roles)
    }

    /// Get users assigned to a role
    pub async fn get_role_users(&self, org_id: &str, role_id: &str) -> Result<Vec<String>> {
        let users: Vec<String> = vd_role_users::Entity::find()
            .filter(vd_role_users::Column::OrgId.eq(org_id))
            .filter(vd_role_users::Column::RoleId.eq(role_id))
            .select_only()
            .column(vd_role_users::Column::UserEmail)
            .into_tuple()
            .all(self.db.as_ref())
            .await?;
        Ok(users)
    }

    /// Get permissions for a role (for API response)
    pub async fn get_role_permissions_list(
        &self,
        org_id: &str,
        role_id: &str,
    ) -> Result<Vec<vd_role_permissions::Model>> {
        let permissions = vd_role_permissions::Entity::find()
            .filter(vd_role_permissions::Column::OrgId.eq(org_id))
            .filter(vd_role_permissions::Column::RoleId.eq(role_id))
            .all(self.db.as_ref())
            .await?;
        Ok(permissions)
    }

    // ==================== User Queries ====================

    /// Get all roles for a user (direct assignments)
    pub async fn get_user_direct_roles(
        &self,
        org_id: &str,
        user_email: &str,
    ) -> Result<Vec<vd_roles::Model>> {
        let role_ids: Vec<String> = vd_role_users::Entity::find()
            .filter(vd_role_users::Column::OrgId.eq(org_id))
            .filter(vd_role_users::Column::UserEmail.eq(user_email))
            .select_only()
            .column(vd_role_users::Column::RoleId)
            .into_tuple()
            .all(self.db.as_ref())
            .await?;

        if role_ids.is_empty() {
            return Ok(vec![]);
        }

        let roles = vd_roles::Entity::find()
            .filter(vd_roles::Column::Id.is_in(role_ids))
            .all(self.db.as_ref())
            .await?;

        Ok(roles)
    }

    /// Get all groups for a user
    pub async fn get_user_groups(
        &self,
        org_id: &str,
        user_email: &str,
    ) -> Result<Vec<crate::entity::vd_groups::Model>> {
        use crate::entity::vd_groups;

        let group_ids: Vec<String> = vd_group_users::Entity::find()
            .filter(vd_group_users::Column::OrgId.eq(org_id))
            .filter(vd_group_users::Column::UserEmail.eq(user_email))
            .select_only()
            .column(vd_group_users::Column::GroupId)
            .into_tuple()
            .all(self.db.as_ref())
            .await?;

        if group_ids.is_empty() {
            return Ok(vec![]);
        }

        let groups = vd_groups::Entity::find()
            .filter(vd_groups::Column::Id.is_in(group_ids))
            .all(self.db.as_ref())
            .await?;

        Ok(groups)
    }

    // ==================== Cache Management ====================

    /// Invalidate cache for a user
    pub fn invalidate_user_cache(&self, org_id: &str, user_email: &str) {
        self.cache.invalidate_user(org_id, user_email);
    }

    /// Invalidate cache for a role
    pub fn invalidate_role_cache(&self, org_id: &str, role_id: &str) {
        self.cache.invalidate_role(org_id, role_id);
    }

    /// Invalidate cache for a group
    pub fn invalidate_group_cache(&self, org_id: &str, group_id: &str) {
        self.cache.invalidate_group(org_id, group_id);
    }

    /// Clear all caches
    pub fn clear_cache(&self) {
        self.cache.clear_all();
    }
}

/// Parse object string into resource and entity parts
fn parse_object(object: &str) -> Result<(&str, &str)> {
    let parts: Vec<&str> = object.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(Error::Validation(format!(
            "Invalid object format: {}. Expected 'resource:entity'",
            object
        )));
    }
    Ok((parts[0], parts[1]))
}

/// Check permission with simplified API (uses global instance)
pub async fn check_permission(
    org_id: &str,
    user_email: &str,
    object: &str,
    http_method: &str,
) -> Result<bool> {
    let visdata = crate::Visdata::global();

    // Convert HTTP method to permission
    let is_list = object.contains("_all_");
    let permission = Permission::from_http_method(http_method, is_list)
        .ok_or_else(|| Error::InvalidPermission(format!("Unknown HTTP method: {}", http_method)))?;

    visdata
        .rbac()
        .check_permission(org_id, user_email, object, &permission.to_string())
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_object() {
        let (resource, entity) = parse_object("logs:my_stream").unwrap();
        assert_eq!(resource, "logs");
        assert_eq!(entity, "my_stream");

        let (resource, entity) = parse_object("dashboard:folder1/dash1").unwrap();
        assert_eq!(resource, "dashboard");
        assert_eq!(entity, "folder1/dash1");

        let (resource, entity) = parse_object("logs:_all_org123").unwrap();
        assert_eq!(resource, "logs");
        assert_eq!(entity, "_all_org123");

        assert!(parse_object("invalid").is_err());
    }

    #[test]
    fn test_permission_grants() {
        assert!(Permission::AllowAll.grants(&Permission::AllowGet));
        assert!(Permission::AllowAll.grants(&Permission::AllowPost));
        assert!(Permission::AllowAll.grants(&Permission::AllowAll));

        assert!(Permission::AllowGet.grants(&Permission::AllowGet));
        assert!(!Permission::AllowGet.grants(&Permission::AllowPost));
        assert!(!Permission::AllowGet.grants(&Permission::AllowAll));
    }

    #[test]
    fn test_permission_from_http_method() {
        assert_eq!(
            Permission::from_http_method("GET", false),
            Some(Permission::AllowGet)
        );
        assert_eq!(
            Permission::from_http_method("GET", true),
            Some(Permission::AllowList)
        );
        assert_eq!(
            Permission::from_http_method("POST", false),
            Some(Permission::AllowPost)
        );
        assert_eq!(
            Permission::from_http_method("PUT", false),
            Some(Permission::AllowPut)
        );
        assert_eq!(
            Permission::from_http_method("DELETE", false),
            Some(Permission::AllowDelete)
        );
        assert_eq!(Permission::from_http_method("OPTIONS", false), None);
    }
}
