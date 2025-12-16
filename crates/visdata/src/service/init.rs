// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Initialization service - creates default roles and permissions

use sea_orm::{ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, QueryFilter};
use svix_ksuid::KsuidLike;

use crate::entity::{vd_role_permissions, vd_roles};
use crate::error::Result;
use crate::rbac::Permission;
use crate::Visdata;

/// Generate a unique ID using KSUID
fn generate_id() -> String {
    svix_ksuid::Ksuid::new(None, None).to_string()
}

/// Get current timestamp in microseconds
fn now_micros() -> i64 {
    chrono::Utc::now().timestamp_micros()
}

/// Default system roles
#[derive(Debug, Clone)]
struct DefaultRole {
    name: &'static str,
    display_name: &'static str,
    description: &'static str,
    /// Permission templates - use {org} placeholder for org_id
    permissions: Vec<(&'static str, Permission)>,
}

/// Format permission object with org_id
/// Replaces {org} placeholder with actual org_id, using "_all_{org_id}" format
fn format_permission_object(template: &str, org_id: &str) -> String {
    template.replace("*", &format!("_all_{}", org_id))
}

/// Get the list of default roles to create
fn get_default_roles() -> Vec<DefaultRole> {
    vec![
        DefaultRole {
            name: "Admin",
            display_name: "Administrator",
            description: "Full access to all resources in the organization",
            permissions: vec![
                // Organization-wide admin permission
                ("org:*", Permission::AllowAll),
            ],
        },
        DefaultRole {
            name: "Editor",
            display_name: "Editor",
            description: "Can view and edit most resources, but cannot manage users or settings",
            permissions: vec![
                // Streams
                ("stream:*", Permission::AllowList),
                ("stream:*", Permission::AllowGet),
                ("stream:*", Permission::AllowPost),
                ("stream:*", Permission::AllowPut),
                ("stream:*", Permission::AllowDelete),
                // Dashboards
                ("dashboard:*", Permission::AllowList),
                ("dashboard:*", Permission::AllowGet),
                ("dashboard:*", Permission::AllowPost),
                ("dashboard:*", Permission::AllowPut),
                ("dashboard:*", Permission::AllowDelete),
                // Alerts
                ("alert:*", Permission::AllowList),
                ("alert:*", Permission::AllowGet),
                ("alert:*", Permission::AllowPost),
                ("alert:*", Permission::AllowPut),
                ("alert:*", Permission::AllowDelete),
                // Functions
                ("function:*", Permission::AllowList),
                ("function:*", Permission::AllowGet),
                ("function:*", Permission::AllowPost),
                ("function:*", Permission::AllowPut),
                ("function:*", Permission::AllowDelete),
                // Pipelines
                ("pipeline:*", Permission::AllowList),
                ("pipeline:*", Permission::AllowGet),
                ("pipeline:*", Permission::AllowPost),
                ("pipeline:*", Permission::AllowPut),
                ("pipeline:*", Permission::AllowDelete),
                // Reports
                ("report:*", Permission::AllowList),
                ("report:*", Permission::AllowGet),
                ("report:*", Permission::AllowPost),
                ("report:*", Permission::AllowPut),
                ("report:*", Permission::AllowDelete),
                // Folders
                ("folder:*", Permission::AllowList),
                ("folder:*", Permission::AllowGet),
                ("folder:*", Permission::AllowPost),
                ("folder:*", Permission::AllowPut),
                ("folder:*", Permission::AllowDelete),
            ],
        },
        DefaultRole {
            name: "Viewer",
            display_name: "Viewer",
            description: "Read-only access to most resources",
            permissions: vec![
                // Streams - read only
                ("stream:*", Permission::AllowList),
                ("stream:*", Permission::AllowGet),
                // Dashboards - read only
                ("dashboard:*", Permission::AllowList),
                ("dashboard:*", Permission::AllowGet),
                // Alerts - read only
                ("alert:*", Permission::AllowList),
                ("alert:*", Permission::AllowGet),
                // Functions - read only
                ("function:*", Permission::AllowList),
                ("function:*", Permission::AllowGet),
                // Pipelines - read only
                ("pipeline:*", Permission::AllowList),
                ("pipeline:*", Permission::AllowGet),
                // Reports - read only
                ("report:*", Permission::AllowList),
                ("report:*", Permission::AllowGet),
                // Folders - read only
                ("folder:*", Permission::AllowList),
                ("folder:*", Permission::AllowGet),
            ],
        },
        DefaultRole {
            name: "Ingester",
            display_name: "Ingester",
            description: "Can only ingest data into streams",
            permissions: vec![
                // Only write permission for streams (ingestion)
                ("stream:*", Permission::AllowPost),
            ],
        },
    ]
}

/// Initialize default roles for an organization
/// This should be called when a new organization is created
pub async fn init_default_roles(org_id: &str) -> Result<()> {
    let db = Visdata::global().db();
    let now = now_micros();

    for default_role in get_default_roles() {
        // Check if role already exists
        let existing = vd_roles::Entity::find()
            .filter(vd_roles::Column::OrgId.eq(org_id))
            .filter(vd_roles::Column::Name.eq(default_role.name))
            .one(db)
            .await?;

        if existing.is_some() {
            tracing::debug!(
                "[VISDATA] Role '{}' already exists in org '{}', skipping",
                default_role.name,
                org_id
            );
            continue;
        }

        // Create the role
        let role_id = generate_id();
        let role = vd_roles::ActiveModel {
            id: ActiveValue::Set(role_id.clone()),
            org_id: ActiveValue::Set(org_id.to_string()),
            name: ActiveValue::Set(default_role.name.to_string()),
            display_name: ActiveValue::Set(Some(default_role.display_name.to_string())),
            description: ActiveValue::Set(Some(default_role.description.to_string())),
            is_system: ActiveValue::Set(true), // Mark as system role
            created_at: ActiveValue::Set(now),
            updated_at: ActiveValue::Set(now),
        };

        role.insert(db).await?;
        tracing::info!(
            "[VISDATA] Created default role '{}' in org '{}'",
            default_role.name,
            org_id
        );

        // Create permissions for this role
        for (object_template, permission) in default_role.permissions {
            // Convert "resource:*" to "resource:_all_{org_id}" format
            let object = format_permission_object(object_template, org_id);
            let perm = vd_role_permissions::ActiveModel {
                id: ActiveValue::Set(generate_id()),
                role_id: ActiveValue::Set(role_id.clone()),
                org_id: ActiveValue::Set(org_id.to_string()),
                object: ActiveValue::Set(object),
                permission: ActiveValue::Set(permission.to_string()),
                created_at: ActiveValue::Set(now),
            };
            perm.insert(db).await?;
        }
    }

    Ok(())
}

/// Initialize default roles for all existing organizations
/// This should be called once during system initialization
pub async fn init_all_orgs() -> Result<()> {
    // Get all organizations from the main system
    let orgs = get_all_org_ids().await?;

    for org_id in orgs {
        if let Err(e) = init_default_roles(&org_id).await {
            tracing::warn!(
                "[VISDATA] Failed to initialize default roles for org '{}': {}",
                org_id,
                e
            );
        }
    }

    tracing::info!("[VISDATA] Default roles initialization completed");
    Ok(())
}

/// Get all organization IDs from the system
async fn get_all_org_ids() -> Result<Vec<String>> {
    // Query the organizations table from infra
    use infra::table::entity::organizations;
    use sea_orm::EntityTrait;

    let db = Visdata::global().db();
    let orgs = organizations::Entity::find()
        .all(db)
        .await?;

    Ok(orgs.into_iter().map(|o| o.identifier).collect())
}
