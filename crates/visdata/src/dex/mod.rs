// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Dex Authentication module (compatible with o2_dex)
//!
//! This module provides SSO and authentication through Dex integration.
//!
//! ## Module Structure
//!
//! - `meta` - Auth types (Permission, RoleRequest, O2EntityAuthorization)
//! - `handler` - HTTP handlers (login, SSO, connectors)
//! - `service` - Token validation, connector management
//! - `config` - Dex configuration
//! - `client` - Dex HTTP/gRPC client
//! - `types` - Request/Response types

pub mod client;
pub mod config;
pub mod error;
pub mod handler;
pub mod meta;
pub mod service;
pub mod types;

// ============================================================================
// Public API Exports
// ============================================================================

pub use client::DexClient;
pub use config::DexConfig;
pub use error::{Error as AuthError, Result as AuthResult};

// Re-export meta for compatibility with o2_dex::meta
pub use meta::auth;
pub use meta::auth::{GroupRequest, O2EntityAuthorization, Permission, RoleRequest};
