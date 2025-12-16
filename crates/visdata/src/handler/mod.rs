// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! HTTP handlers for VisData APIs

pub mod groups;
pub mod resources;
pub mod roles;
pub mod sso;
pub mod users;

// Re-export handlers for easy access
pub use groups::{create_group, delete_group, get_group, list_groups, update_group};
pub use resources::get_resources;
pub use roles::{
    create_role, delete_role, get_role_permissions, get_role_users, list_roles, update_role,
};
pub use sso::{
    create_ldap_provider, create_oidc_provider, delete_provider, list_providers, sso_callback,
    sso_login, update_provider,
};
pub use users::{get_user_groups, get_user_roles, list_custom_roles, list_system_roles};
