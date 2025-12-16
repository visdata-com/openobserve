// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! VisData Enterprise Module for OpenObserve
//!
//! This module provides:
//! - SSO (Single Sign-On) with OIDC and LDAP support
//! - RBAC (Role-Based Access Control) with fine-grained permissions

pub mod config;
pub mod error;
pub mod meta;
pub mod rbac;
pub mod service;
pub mod sso;
pub mod handler;
pub mod entity;

pub use config::VisdataConfig;
pub use error::{Error, Result};

use sea_orm::DatabaseConnection;
use std::sync::{Arc, OnceLock};
use tokio::sync::RwLock;

/// Global VisData instance
static VISDATA: OnceLock<Visdata> = OnceLock::new();

/// Shutdown signal sender for background tasks
static SHUTDOWN_TX: OnceLock<tokio::sync::watch::Sender<bool>> = OnceLock::new();

/// Main VisData module instance
pub struct Visdata {
    db: Arc<DatabaseConnection>,
    rbac_engine: Arc<rbac::RBACEngine>,
    config: Arc<RwLock<VisdataConfig>>,
    /// Handle to the cache cleaner task
    #[allow(dead_code)]
    cache_cleaner_handle: Option<tokio::task::JoinHandle<()>>,
}

impl Visdata {
    /// Initialize the VisData module
    pub async fn init(db: Arc<DatabaseConnection>, config: VisdataConfig) -> Result<()> {
        let rbac_engine = rbac::RBACEngine::new(db.clone()).await?;
        let rbac_engine = Arc::new(rbac_engine);

        // Set up shutdown channel for background tasks
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        SHUTDOWN_TX
            .set(shutdown_tx)
            .map_err(|_| Error::AlreadyInitialized)?;

        // Start cache cleaner if caching is enabled
        let cache_cleaner_handle = if config.cache.enabled {
            let cleaner = rbac::CacheCleaner::new(
                rbac_engine.cache().clone(),
                config.cache.ttl_seconds / 2, // Clean at half the TTL interval
                shutdown_rx,
            );
            Some(cleaner.start())
        } else {
            None
        };

        let instance = Visdata {
            db,
            rbac_engine,
            config: Arc::new(RwLock::new(config)),
            cache_cleaner_handle,
        };

        VISDATA
            .set(instance)
            .map_err(|_| Error::AlreadyInitialized)?;

        // Initialize default roles for all existing organizations
        if let Err(e) = service::init::init_all_orgs().await {
            tracing::warn!("[VISDATA] Failed to initialize default roles: {}", e);
        }

        tracing::info!("[VISDATA] Module initialized successfully");
        Ok(())
    }

    /// Shutdown the VisData module and stop background tasks
    pub fn shutdown() {
        if let Some(tx) = SHUTDOWN_TX.get() {
            let _ = tx.send(true);
            tracing::info!("[VISDATA] Shutdown signal sent");
        }
    }

    /// Get the global VisData instance
    pub fn global() -> &'static Visdata {
        VISDATA.get().expect("VisData not initialized. Call Visdata::init() first")
    }

    /// Get the RBAC engine
    pub fn rbac(&self) -> &rbac::RBACEngine {
        &self.rbac_engine
    }

    /// Get the database connection
    pub fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    /// Get the configuration
    pub async fn config(&self) -> VisdataConfig {
        self.config.read().await.clone()
    }
}

/// Check if VisData module is initialized
pub fn is_initialized() -> bool {
    VISDATA.get().is_some()
}
