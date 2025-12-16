// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Permission cache using DashMap

use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::config::CacheConfig;

/// Cache entry with expiration
#[derive(Debug, Clone)]
struct CacheEntry<T> {
    value: T,
    expires_at: Instant,
}

impl<T> CacheEntry<T> {
    fn new(value: T, ttl: Duration) -> Self {
        Self {
            value,
            expires_at: Instant::now() + ttl,
        }
    }

    fn is_expired(&self) -> bool {
        Instant::now() > self.expires_at
    }
}

/// Permission cache key
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PermissionCacheKey {
    pub org_id: String,
    pub user_email: String,
    pub object: String,
    pub permission: String,
}

impl PermissionCacheKey {
    pub fn new(org_id: &str, user_email: &str, object: &str, permission: &str) -> Self {
        Self {
            org_id: org_id.to_string(),
            user_email: user_email.to_string(),
            object: object.to_string(),
            permission: permission.to_string(),
        }
    }
}

/// User roles cache key
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UserRolesCacheKey {
    pub org_id: String,
    pub user_email: String,
}

/// Cached user roles
#[derive(Debug, Clone)]
pub struct CachedUserRoles {
    pub direct_role_ids: Vec<String>,
    pub group_role_ids: Vec<String>,
}

/// Permission cache for fast lookups
pub struct PermissionCache {
    /// Permission check results cache: key -> allowed (true/false)
    permission_cache: Arc<DashMap<PermissionCacheKey, CacheEntry<bool>>>,
    /// User roles cache: (org_id, user_email) -> role IDs
    user_roles_cache: Arc<DashMap<UserRolesCacheKey, CacheEntry<CachedUserRoles>>>,
    /// Role permissions cache: role_id -> list of (object, permission)
    role_permissions_cache: Arc<DashMap<String, CacheEntry<Vec<(String, String)>>>>,
    /// Cache configuration
    config: CacheConfig,
    /// TTL duration
    ttl: Duration,
}

impl PermissionCache {
    /// Create a new permission cache
    pub fn new(config: CacheConfig) -> Self {
        let ttl = Duration::from_secs(config.ttl_seconds);
        Self {
            permission_cache: Arc::new(DashMap::new()),
            user_roles_cache: Arc::new(DashMap::new()),
            role_permissions_cache: Arc::new(DashMap::new()),
            config,
            ttl,
        }
    }

    /// Check if caching is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    // ==================== Permission Cache ====================

    /// Get cached permission result
    pub fn get_permission(&self, key: &PermissionCacheKey) -> Option<bool> {
        if !self.config.enabled {
            return None;
        }

        if let Some(entry) = self.permission_cache.get(key) {
            if !entry.is_expired() {
                return Some(entry.value);
            }
            // Remove expired entry
            drop(entry);
            self.permission_cache.remove(key);
        }
        None
    }

    /// Cache permission result
    pub fn set_permission(&self, key: PermissionCacheKey, allowed: bool) {
        if !self.config.enabled {
            return;
        }

        // Check max entries limit
        if self.permission_cache.len() >= self.config.max_entries {
            self.evict_expired_permissions();
        }

        self.permission_cache
            .insert(key, CacheEntry::new(allowed, self.ttl));
    }

    /// Evict expired permission entries
    fn evict_expired_permissions(&self) {
        self.permission_cache.retain(|_, v| !v.is_expired());
    }

    /// Evict all expired entries from all caches
    pub fn evict_all_expired(&self) {
        self.permission_cache.retain(|_, v| !v.is_expired());
        self.user_roles_cache.retain(|_, v| !v.is_expired());
        self.role_permissions_cache.retain(|_, v| !v.is_expired());
    }

    // ==================== User Roles Cache ====================

    /// Get cached user roles
    pub fn get_user_roles(&self, org_id: &str, user_email: &str) -> Option<CachedUserRoles> {
        if !self.config.enabled {
            return None;
        }

        let key = UserRolesCacheKey {
            org_id: org_id.to_string(),
            user_email: user_email.to_string(),
        };

        if let Some(entry) = self.user_roles_cache.get(&key) {
            if !entry.is_expired() {
                return Some(entry.value.clone());
            }
            drop(entry);
            self.user_roles_cache.remove(&key);
        }
        None
    }

    /// Cache user roles
    pub fn set_user_roles(&self, org_id: &str, user_email: &str, roles: CachedUserRoles) {
        if !self.config.enabled {
            return;
        }

        let key = UserRolesCacheKey {
            org_id: org_id.to_string(),
            user_email: user_email.to_string(),
        };

        self.user_roles_cache
            .insert(key, CacheEntry::new(roles, self.ttl));
    }

    // ==================== Role Permissions Cache ====================

    /// Get cached role permissions
    pub fn get_role_permissions(&self, role_id: &str) -> Option<Vec<(String, String)>> {
        if !self.config.enabled {
            return None;
        }

        if let Some(entry) = self.role_permissions_cache.get(role_id) {
            if !entry.is_expired() {
                return Some(entry.value.clone());
            }
            drop(entry);
            self.role_permissions_cache.remove(role_id);
        }
        None
    }

    /// Cache role permissions
    pub fn set_role_permissions(&self, role_id: &str, permissions: Vec<(String, String)>) {
        if !self.config.enabled {
            return;
        }

        self.role_permissions_cache.insert(
            role_id.to_string(),
            CacheEntry::new(permissions, self.ttl),
        );
    }

    // ==================== Invalidation ====================

    /// Invalidate all caches for a user
    pub fn invalidate_user(&self, org_id: &str, user_email: &str) {
        // Remove user roles cache
        let roles_key = UserRolesCacheKey {
            org_id: org_id.to_string(),
            user_email: user_email.to_string(),
        };
        self.user_roles_cache.remove(&roles_key);

        // Remove all permission cache entries for this user
        self.permission_cache.retain(|k, _| {
            !(k.org_id == org_id && k.user_email == user_email)
        });
    }

    /// Invalidate all caches for a role
    pub fn invalidate_role(&self, org_id: &str, role_id: &str) {
        // Remove role permissions cache
        self.role_permissions_cache.remove(role_id);

        // Remove all permission cache entries for this org
        // (we don't know which users have this role without querying DB)
        self.permission_cache.retain(|k, _| k.org_id != org_id);

        // Remove all user roles cache for this org
        self.user_roles_cache.retain(|k, _| k.org_id != org_id);
    }

    /// Invalidate all caches for a group
    pub fn invalidate_group(&self, org_id: &str, _group_id: &str) {
        // Remove all caches for this org since group membership affects permissions
        self.permission_cache.retain(|k, _| k.org_id != org_id);
        self.user_roles_cache.retain(|k, _| k.org_id != org_id);
    }

    /// Invalidate all caches for an organization
    pub fn invalidate_org(&self, org_id: &str) {
        self.permission_cache.retain(|k, _| k.org_id != org_id);
        self.user_roles_cache.retain(|k, _| k.org_id != org_id);
        // Note: role_permissions_cache entries don't have org_id
        // They will expire naturally
    }

    /// Clear all caches
    pub fn clear_all(&self) {
        self.permission_cache.clear();
        self.user_roles_cache.clear();
        self.role_permissions_cache.clear();
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            permission_entries: self.permission_cache.len(),
            user_roles_entries: self.user_roles_cache.len(),
            role_permissions_entries: self.role_permissions_cache.len(),
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub permission_entries: usize,
    pub user_roles_entries: usize,
    pub role_permissions_entries: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_cache() -> PermissionCache {
        PermissionCache::new(CacheConfig {
            enabled: true,
            ttl_seconds: 1, // 1 second TTL for tests
            max_entries: 100,
        })
    }

    fn create_disabled_cache() -> PermissionCache {
        PermissionCache::new(CacheConfig {
            enabled: false,
            ttl_seconds: 300,
            max_entries: 100,
        })
    }

    #[test]
    fn test_permission_cache_set_get() {
        let cache = create_test_cache();

        let key = PermissionCacheKey::new("org1", "user@test.com", "logs:stream1", "AllowGet");

        // Initially empty
        assert!(cache.get_permission(&key).is_none());

        // Set value
        cache.set_permission(key.clone(), true);

        // Get value
        assert_eq!(cache.get_permission(&key), Some(true));
    }

    #[test]
    fn test_permission_cache_disabled() {
        let cache = create_disabled_cache();

        let key = PermissionCacheKey::new("org1", "user@test.com", "logs:stream1", "AllowGet");

        // Set should be no-op
        cache.set_permission(key.clone(), true);

        // Get should return None
        assert!(cache.get_permission(&key).is_none());
    }

    #[test]
    fn test_permission_cache_expiration() {
        let cache = create_test_cache();

        let key = PermissionCacheKey::new("org1", "user@test.com", "logs:stream1", "AllowGet");

        cache.set_permission(key.clone(), true);
        assert_eq!(cache.get_permission(&key), Some(true));

        // Wait for expiration (TTL is 1 second)
        std::thread::sleep(std::time::Duration::from_millis(1100));

        // Should be expired
        assert!(cache.get_permission(&key).is_none());
    }

    #[test]
    fn test_user_roles_cache() {
        let cache = create_test_cache();

        let roles = CachedUserRoles {
            direct_role_ids: vec!["role1".to_string(), "role2".to_string()],
            group_role_ids: vec!["role3".to_string()],
        };

        // Initially empty
        assert!(cache.get_user_roles("org1", "user@test.com").is_none());

        // Set value
        cache.set_user_roles("org1", "user@test.com", roles.clone());

        // Get value
        let cached = cache.get_user_roles("org1", "user@test.com").unwrap();
        assert_eq!(cached.direct_role_ids, vec!["role1", "role2"]);
        assert_eq!(cached.group_role_ids, vec!["role3"]);
    }

    #[test]
    fn test_role_permissions_cache() {
        let cache = create_test_cache();

        let permissions = vec![
            ("logs:stream1".to_string(), "AllowGet".to_string()),
            ("logs:_all_org1".to_string(), "AllowList".to_string()),
        ];

        // Initially empty
        assert!(cache.get_role_permissions("role1").is_none());

        // Set value
        cache.set_role_permissions("role1", permissions.clone());

        // Get value
        let cached = cache.get_role_permissions("role1").unwrap();
        assert_eq!(cached.len(), 2);
    }

    #[test]
    fn test_invalidate_user() {
        let cache = create_test_cache();

        let key = PermissionCacheKey::new("org1", "user@test.com", "logs:stream1", "AllowGet");
        cache.set_permission(key.clone(), true);

        let roles = CachedUserRoles {
            direct_role_ids: vec!["role1".to_string()],
            group_role_ids: vec![],
        };
        cache.set_user_roles("org1", "user@test.com", roles);

        // Both should be cached
        assert!(cache.get_permission(&key).is_some());
        assert!(cache.get_user_roles("org1", "user@test.com").is_some());

        // Invalidate user
        cache.invalidate_user("org1", "user@test.com");

        // Both should be gone
        assert!(cache.get_permission(&key).is_none());
        assert!(cache.get_user_roles("org1", "user@test.com").is_none());
    }

    #[test]
    fn test_invalidate_role() {
        let cache = create_test_cache();

        cache.set_role_permissions("role1", vec![("logs:stream1".to_string(), "AllowGet".to_string())]);

        // Should be cached
        assert!(cache.get_role_permissions("role1").is_some());

        // Invalidate role
        cache.invalidate_role("org1", "role1");

        // Should be gone
        assert!(cache.get_role_permissions("role1").is_none());
    }

    #[test]
    fn test_clear_all() {
        let cache = create_test_cache();

        // Add some data
        let key = PermissionCacheKey::new("org1", "user@test.com", "logs:stream1", "AllowGet");
        cache.set_permission(key.clone(), true);
        cache.set_user_roles("org1", "user@test.com", CachedUserRoles {
            direct_role_ids: vec!["role1".to_string()],
            group_role_ids: vec![],
        });
        cache.set_role_permissions("role1", vec![]);

        // Verify data exists
        let stats = cache.stats();
        assert!(stats.permission_entries > 0);
        assert!(stats.user_roles_entries > 0);
        assert!(stats.role_permissions_entries > 0);

        // Clear all
        cache.clear_all();

        // Verify empty
        let stats = cache.stats();
        assert_eq!(stats.permission_entries, 0);
        assert_eq!(stats.user_roles_entries, 0);
        assert_eq!(stats.role_permissions_entries, 0);
    }

    #[test]
    fn test_evict_all_expired() {
        let cache = create_test_cache();

        // Add some data
        let key = PermissionCacheKey::new("org1", "user@test.com", "logs:stream1", "AllowGet");
        cache.set_permission(key.clone(), true);
        cache.set_role_permissions("role1", vec![]);

        // Verify data exists
        let stats = cache.stats();
        assert!(stats.permission_entries > 0);
        assert!(stats.role_permissions_entries > 0);

        // Wait for expiration
        std::thread::sleep(std::time::Duration::from_millis(1100));

        // Evict expired
        cache.evict_all_expired();

        // Verify empty
        let stats = cache.stats();
        assert_eq!(stats.permission_entries, 0);
        assert_eq!(stats.role_permissions_entries, 0);
    }

    #[test]
    fn test_cache_stats() {
        let cache = create_test_cache();

        let stats = cache.stats();
        assert_eq!(stats.permission_entries, 0);
        assert_eq!(stats.user_roles_entries, 0);
        assert_eq!(stats.role_permissions_entries, 0);

        // Add data
        cache.set_permission(
            PermissionCacheKey::new("org1", "user1@test.com", "logs:s1", "AllowGet"),
            true,
        );
        cache.set_permission(
            PermissionCacheKey::new("org1", "user2@test.com", "logs:s1", "AllowGet"),
            false,
        );
        cache.set_user_roles("org1", "user1@test.com", CachedUserRoles {
            direct_role_ids: vec![],
            group_role_ids: vec![],
        });
        cache.set_role_permissions("role1", vec![]);

        let stats = cache.stats();
        assert_eq!(stats.permission_entries, 2);
        assert_eq!(stats.user_roles_entries, 1);
        assert_eq!(stats.role_permissions_entries, 1);
    }
}

/// Background cache cleaner that periodically removes expired entries
pub struct CacheCleaner {
    cache: Arc<PermissionCache>,
    cleanup_interval: Duration,
    shutdown: tokio::sync::watch::Receiver<bool>,
}

impl CacheCleaner {
    /// Create a new cache cleaner
    pub fn new(
        cache: Arc<PermissionCache>,
        cleanup_interval_secs: u64,
        shutdown: tokio::sync::watch::Receiver<bool>,
    ) -> Self {
        Self {
            cache,
            cleanup_interval: Duration::from_secs(cleanup_interval_secs),
            shutdown,
        }
    }

    /// Start the background cleanup task
    pub fn start(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            self.run().await;
        })
    }

    /// Run the cleanup loop
    async fn run(mut self) {
        tracing::info!(
            "[VISDATA] Cache cleaner started, interval: {:?}",
            self.cleanup_interval
        );

        loop {
            tokio::select! {
                _ = tokio::time::sleep(self.cleanup_interval) => {
                    self.cleanup();
                }
                _ = self.shutdown.changed() => {
                    if *self.shutdown.borrow() {
                        tracing::info!("[VISDATA] Cache cleaner shutting down");
                        break;
                    }
                }
            }
        }
    }

    /// Perform cleanup of expired entries
    fn cleanup(&self) {
        let before_stats = self.cache.stats();

        self.cache.evict_all_expired();

        let after_stats = self.cache.stats();

        let permissions_removed = before_stats.permission_entries.saturating_sub(after_stats.permission_entries);
        let user_roles_removed = before_stats.user_roles_entries.saturating_sub(after_stats.user_roles_entries);
        let role_perms_removed = before_stats.role_permissions_entries.saturating_sub(after_stats.role_permissions_entries);

        if permissions_removed > 0 || user_roles_removed > 0 || role_perms_removed > 0 {
            tracing::debug!(
                "[VISDATA] Cache cleanup: removed {} permissions, {} user_roles, {} role_permissions",
                permissions_removed,
                user_roles_removed,
                role_perms_removed
            );
        }
    }
}
