// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! VisData Enterprise Module for OpenObserve
//!
//! This module provides:
//! - Dex (Authentication) with Dex integration for SSO
//! - OpenFGA (Authorization) with OpenFGA integration for fine-grained access control
//! - Log Patterns (Analysis) with Drain algorithm for log pattern extraction
//!
//! ## Architecture (compatible with o2_enterprise)
//!
//! ```text
//! visdata/
//! ├── dex/              # Authentication module (compatible with o2_dex)
//! │   ├── config        # DexConfig
//! │   ├── client        # DexClient (HTTP-based)
//! │   ├── handler       # HTTP handlers (login, SSO, connectors)
//! │   ├── service       # Token validation, connector management
//! │   ├── meta          # Auth types (Permission, RoleRequest, etc.)
//! │   └── types         # Request/Response types
//! │
//! ├── openfga/          # Authorization module (compatible with o2_openfga)
//! │   ├── config        # OpenFGAConfig
//! │   ├── client        # OpenFGAClient (HTTP-based)
//! │   ├── authorizer    # Permission checking API (is_allowed, roles, groups)
//! │   ├── meta          # Resource mappings (OFGA_MODELS)
//! │   ├── model         # FGA schema, resource definitions
//! │   ├── service       # Internal service layer
//! │   └── types         # Request/Response types
//! │
//! ├── log_patterns/     # Log pattern extraction (compatible with o2_enterprise::log_patterns)
//! │   ├── config        # PatternExtractionConfig
//! │   ├── accumulator   # PatternAccumulator
//! │   ├── extractor     # Drain algorithm implementation
//! │   ├── sdr           # SDR type recognition (IP, NUM, UUID, etc.)
//! │   └── types         # Pattern, Statistics types
//! │
//! ├── enterprise/       # Enterprise compatibility layer
//! │   └── common        # Enterprise config utilities
//! │
//! ├── common/           # Shared utilities (KSUID generation)
//! └── config/           # VisdataConfig and get_config
//! ```

// ============================================================================
// Enterprise Modules (OpenFGA + Dex + Log Patterns)
// ============================================================================

pub mod common;
pub mod config;
pub mod dex;
pub mod enterprise;
pub mod log_patterns;
pub mod meta;
pub mod openfga;

// ============================================================================
// Backward Compatibility Aliases
// ============================================================================

/// Alias for `dex` module (compatible with o2_dex)
pub use dex as auth;

// ============================================================================
// Re-exports
// ============================================================================

pub use config::VisdataConfig;

use std::sync::{Arc, OnceLock};
use tokio::sync::RwLock;

/// Error types for visdata
pub mod error {
    use thiserror::Error;

    #[derive(Debug, Error)]
    pub enum Error {
        #[error("Already initialized")]
        AlreadyInitialized,

        #[error("Not initialized")]
        NotInitialized,

        #[error("Internal error: {0}")]
        Internal(String),

        #[error("Configuration error: {0}")]
        Config(String),

        #[error("OpenFGA error: {0}")]
        OpenFGA(String),

        #[error("Dex error: {0}")]
        Dex(String),
    }

    pub type Result<T> = std::result::Result<T, Error>;
}

pub use error::{Error, Result};

/// Global VisData instance
static VISDATA: OnceLock<Visdata> = OnceLock::new();

/// Shutdown signal sender for background tasks
static SHUTDOWN_TX: OnceLock<tokio::sync::watch::Sender<bool>> = OnceLock::new();

/// Main VisData module instance
pub struct Visdata {
    /// OpenFGA client for authorization
    openfga_client: Arc<openfga::OpenFGAClient>,
    /// Dex client for authentication
    dex_client: Arc<RwLock<dex::DexClient>>,
    /// Dex configuration
    dex_cfg: dex::DexConfig,
    /// OpenFGA configuration
    openfga_cfg: openfga::OpenFGAConfig,
    /// Main configuration
    config: Arc<RwLock<VisdataConfig>>,
}

impl Visdata {
    /// Initialize the VisData module with enterprise config (OpenFGA + Dex)
    ///
    /// # Arguments
    /// * `config` - Configuration containing OpenFGA and Dex settings
    ///
    /// # Environment Variables
    /// - `VISDATA_OPENFGA_URL` - OpenFGA HTTP API URL (default: http://localhost:8080)
    /// - `VISDATA_OPENFGA_STORE` - OpenFGA store name (default: openobserve)
    /// - `VISDATA_DEX_GRPC_URL` - Dex gRPC URL (default: http://localhost:5557)
    /// - `VISDATA_DEX_ISSUER_URL` - Dex OIDC issuer URL (default: http://localhost:5556)
    /// - `VISDATA_DEX_CLIENT_ID` - OAuth2 client ID (default: openobserve)
    /// - `VISDATA_DEX_CLIENT_SECRET` - OAuth2 client secret
    /// - `VISDATA_DEX_REDIRECT_URI` - OAuth2 redirect URI
    pub async fn init_enterprise(cfg: VisdataConfig) -> Result<()> {
        // Initialize OpenFGA client
        let openfga_cfg = openfga::OpenFGAConfig::default()
            .with_api_url(&cfg.openfga_url)
            .with_store_name(&cfg.openfga_store_name);

        let openfga_client = openfga::OpenFGAClient::new(&openfga_cfg)
            .await
            .map_err(|e| Error::OpenFGA(format!("OpenFGA init failed: {}", e)))?;

        // Initialize Dex client
        let dex_cfg = dex::DexConfig::new(&cfg.dex_grpc_url)
            .with_issuer(&cfg.dex_issuer_url)
            .with_client(&cfg.dex_client_id, &cfg.dex_client_secret)
            .with_redirect_uri(&cfg.dex_redirect_uri);

        let dex_client = dex::DexClient::new(&dex_cfg)
            .await
            .map_err(|e| Error::Dex(format!("Dex init failed: {}", e)))?;

        // Set up shutdown channel
        let (shutdown_tx, _shutdown_rx) = tokio::sync::watch::channel(false);
        SHUTDOWN_TX
            .set(shutdown_tx)
            .map_err(|_| Error::AlreadyInitialized)?;

        let instance = Visdata {
            openfga_client: Arc::new(openfga_client),
            dex_client: Arc::new(RwLock::new(dex_client)),
            dex_cfg,
            openfga_cfg,
            config: Arc::new(RwLock::new(cfg)),
        };

        VISDATA
            .set(instance)
            .map_err(|_| Error::AlreadyInitialized)?;

        tracing::info!("[VISDATA] Enterprise module initialized (OpenFGA + Dex)");
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
    ///
    /// # Panics
    /// Panics if VisData has not been initialized via `init_enterprise()`
    pub fn global() -> &'static Visdata {
        VISDATA.get().expect("VisData not initialized. Call Visdata::init_enterprise() first")
    }

    /// Try to get the global VisData instance
    ///
    /// Returns `None` if VisData has not been initialized
    pub fn try_global() -> Option<&'static Visdata> {
        VISDATA.get()
    }

    /// Get the OpenFGA client
    pub fn openfga(&self) -> &openfga::OpenFGAClient {
        &self.openfga_client
    }

    /// Get the Dex client
    pub fn dex(&self) -> &RwLock<dex::DexClient> {
        &self.dex_client
    }

    /// Get the Dex configuration
    pub fn dex_config(&self) -> &dex::DexConfig {
        &self.dex_cfg
    }

    /// Get the OpenFGA configuration
    pub fn openfga_config(&self) -> &openfga::OpenFGAConfig {
        &self.openfga_cfg
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
