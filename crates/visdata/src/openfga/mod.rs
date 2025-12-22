// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! OpenFGA Authorization module (compatible with o2_openfga)
//!
//! This module provides fine-grained authorization through OpenFGA integration.
//!
//! ## Module Structure
//!
//! - `authorizer` - Permission checking API (is_allowed, roles, groups)
//! - `meta` - Resource mappings (OFGA_MODELS)
//! - `model` - FGA schema and resource definitions
//! - `service` - Internal service layer
//! - `config` - OpenFGA configuration
//! - `client` - OpenFGA HTTP client
//! - `types` - Request/Response types

pub mod authorizer;
pub mod client;
pub mod config;
pub mod error;
pub mod meta;
pub mod model;
pub mod service;
pub mod types;

// ============================================================================
// Public API Exports
// ============================================================================

pub use client::OpenFGAClient;
pub use config::OpenFGAConfig;
pub use error::{Error as RbacError, Result as RbacResult};

// Re-export authorizer for compatibility with o2_openfga::authorizer
pub use authorizer::authz;
pub use authorizer::groups;
pub use authorizer::roles;

// Re-export meta for compatibility with o2_openfga::meta
pub use meta::mapping;
pub use meta::mapping::{OFGA_MODELS, NON_CLOUD_RESOURCE_KEYS, Resource};
