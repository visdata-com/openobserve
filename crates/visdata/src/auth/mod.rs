// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Authentication module using Dex
//!
//! This module provides SSO and authentication through Dex integration.

pub mod config;
pub mod error;
pub mod types;
pub mod client;
pub mod service;
pub mod handler;

pub use config::DexConfig;
pub use client::DexClient;
pub use error::{Error as AuthError, Result as AuthResult};
