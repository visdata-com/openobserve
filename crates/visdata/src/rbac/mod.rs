// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! RBAC (Role-Based Access Control) module

pub mod cache;
pub mod engine;
pub mod resources;

pub use cache::{CacheCleaner, CacheStats, PermissionCache};
pub use engine::RBACEngine;
pub use resources::{Permission, ResourceType, RESOURCE_DEFINITIONS};
