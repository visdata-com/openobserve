// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! RBAC service layer

pub mod checker;
pub mod tuples;
pub mod roles;
pub mod groups;

// Re-export checker functions
pub use checker::{is_allowed, check_permissions, list_objects_for_user};

// Re-export tuples functions
pub use tuples::{
    update_tuples, get_add_user_to_org_tuples, get_user_crole_tuple,
    get_org_creation_tuples, get_ownership_tuple, get_resource_parent_tuple,
    get_org_resource_permission_tuple, get_group_member_tuple, get_group_role_tuple,
    get_service_account_creation_tuple, get_delete_user_from_org_tuples,
    // System role sync functions
    role_to_fga_relation, get_delete_user_system_role_tuples,
    update_user_role, get_delete_all_user_from_org_tuples,
    // Custom role functions
    get_role_key, get_user_crole_removal_tuples,
};

// Re-export roles functions
pub use roles::{
    create_role, list_roles, list_system_roles, list_custom_roles,
    delete_role, get_role_users, get_role_permissions,
    add_role_permissions, remove_role_permissions,
    add_role_users, remove_role_users,
};

// Re-export groups functions
pub use groups::{
    create_group, list_groups, get_group, delete_group,
    add_group_users, remove_group_users, add_group_roles, remove_group_roles,
    get_user_groups, get_user_roles,
};
