// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Database entities for VisData module

pub mod vd_roles;
pub mod vd_role_permissions;
pub mod vd_role_users;
pub mod vd_groups;
pub mod vd_group_roles;
pub mod vd_group_users;
pub mod vd_sso_providers;
pub mod vd_sso_user_mappings;

pub use vd_roles::Entity as VdRoles;
pub use vd_role_permissions::Entity as VdRolePermissions;
pub use vd_role_users::Entity as VdRoleUsers;
pub use vd_groups::Entity as VdGroups;
pub use vd_group_roles::Entity as VdGroupRoles;
pub use vd_group_users::Entity as VdGroupUsers;
pub use vd_sso_providers::Entity as VdSsoProviders;
pub use vd_sso_user_mappings::Entity as VdSsoUserMappings;
