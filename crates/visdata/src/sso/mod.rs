// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! SSO (Single Sign-On) module
//!
//! Supports OIDC and LDAP authentication providers

pub mod oidc;
pub mod ldap;
pub mod provider;

pub use provider::{SSOProvider, SSOProviderManager};
