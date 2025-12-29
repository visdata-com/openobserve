// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Authorization checking API (compatible with o2_openfga::authorizer::authz)
//!
//! This module provides the core authorization checking functions that are
//! compatible with the o2_openfga::authorizer::authz API.

use crate::Visdata;
use crate::openfga::error::Result;
use crate::openfga::model::schema;
use crate::openfga::service::{checker, tuples};
use crate::openfga::types::TupleKey;

// Re-export core functions from service layer
pub use checker::{is_allowed, check_permissions, list_objects_for_user};
pub use tuples::{
    update_tuples,
    get_add_user_to_org_tuples,
    get_user_crole_tuple,
    get_org_creation_tuples,
    get_ownership_tuple,
    get_resource_parent_tuple,
    get_org_resource_permission_tuple,
    get_group_member_tuple,
    get_group_role_tuple,
    get_service_account_creation_tuple,
    get_new_user_creation_tuple,
    get_delete_user_from_org_tuples,
    role_to_fga_relation,
    get_delete_user_system_role_tuples,
    get_delete_all_user_from_org_tuples,
    update_user_role,
};

/// Initialize OpenFGA (compatible with o2_openfga::authorizer::authz::init_open_fga)
///
/// This function ensures the OpenFGA store and model are properly set up.
/// In the VisData implementation, this is handled during Visdata::init_enterprise().
pub async fn init_open_fga() -> Result<()> {
    // Verify that OpenFGA is initialized
    let visdata = Visdata::try_global()
        .ok_or_else(|| crate::openfga::error::Error::NotInitialized(
            "OpenFGA not initialized. Call Visdata::init_enterprise() first".to_string()
        ))?;

    // Verify connection is working
    let _config = visdata.openfga().config().await;

    tracing::info!("[RBAC] OpenFGA initialized successfully");
    Ok(())
}

/// Add a user to an organization (compatible with o2_openfga::authorizer::authz::add_user_to_org)
pub async fn add_user_to_org(
    org_id: &str,
    user_email: &str,
    role: &str,
) -> Result<()> {
    let mut writes = Vec::new();
    get_add_user_to_org_tuples(org_id, user_email, role, &mut writes);
    update_tuples(writes, vec![]).await
}

/// Delete a user from an organization (compatible with o2_openfga::authorizer::authz::delete_user_from_org)
///
/// This removes all possible role tuples (admin/editor/viewer/allowed_user) and org_context
pub async fn delete_user_from_org(
    org_id: &str,
    user_email: &str,
) -> Result<()> {
    let mut deletes = Vec::new();
    get_delete_all_user_from_org_tuples(org_id, user_email, &mut deletes);
    update_tuples(vec![], deletes).await
}

/// Delete a user from an organization with known role
/// (compatible with o2_openfga::authorizer::authz::delete_user_from_org with role)
pub async fn delete_user_from_org_with_role(
    org_id: &str,
    user_email: &str,
    role: &str,
) -> Result<()> {
    let mut deletes = Vec::new();
    get_delete_user_system_role_tuples(org_id, user_email, role, &mut deletes);
    update_tuples(vec![], deletes).await
}

/// Save organization tuples (compatible with o2_openfga::authorizer::authz::save_org_tuples)
pub async fn save_org_tuples(org_id: &str) -> Result<()> {
    let mut writes = Vec::new();
    get_org_creation_tuples(org_id, &mut writes);
    update_tuples(writes, vec![]).await
}

/// Delete organization tuples (compatible with o2_openfga::authorizer::authz::delete_org_tuples)
pub async fn delete_org_tuples(org_id: &str) -> Result<()> {
    let visdata = Visdata::global();
    let org = schema::org_type(org_id);

    // Read all tuples related to this organization
    let filter = crate::openfga::types::TupleKeyFilter {
        user: None,
        relation: None,
        object: Some(org),
    };

    let org_tuples = visdata.openfga().read(Some(filter)).await?;

    // Delete all related tuples
    let deletes: Vec<TupleKey> = org_tuples.into_iter().map(|t| t.key).collect();

    if !deletes.is_empty() {
        update_tuples(vec![], deletes).await?;
    }

    Ok(())
}

/// List objects that a user can access (compatible with o2_openfga::authorizer::authz::list_objects)
///
/// Returns a list of object IDs that the user has the specified permission on.
pub async fn list_objects(
    org_id: &str,
    user_id: &str,
    permission: &str,
    object_type: &str,
    role: &str,
) -> Result<Option<Vec<String>>> {
    list_objects_for_user(org_id, user_id, permission, object_type, role).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_signatures_exist() {
        // This test just verifies the function signatures are correct
        // Actual testing requires a running OpenFGA instance
        let _ = is_allowed;
        let _ = init_open_fga;
        let _ = add_user_to_org;
        let _ = delete_user_from_org;
        let _ = save_org_tuples;
        let _ = delete_org_tuples;
        let _ = update_tuples;
        let _ = list_objects;
    }
}

