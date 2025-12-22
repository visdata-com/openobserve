// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Authorization module (compatible with o2_openfga::authorizer)
//!
//! This module provides the authorization API for checking permissions,
//! managing roles, and managing groups.

pub mod authz;
pub mod groups;
pub mod roles;

pub use authz::*;
pub use groups::*;
pub use roles::*;
