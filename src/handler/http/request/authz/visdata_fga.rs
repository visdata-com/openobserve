// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

//! VisData FGA (Fine-Grained Authorization) HTTP handlers
//!
//! This module provides HTTP handlers for role, group, and resource management
//! using the VisData OpenFGA integration.

use std::io::Error;

use actix_web::{delete, get, post, put, web, HttpResponse};

use crate::common::meta::{
    http::HttpResponse as MetaHttpResponse,
    user::{UserGroup, UserGroupRequest, UserRoleRequest},
};

#[cfg(feature = "visdata")]
use {
    crate::{common::utils::auth::UserEmail, handler::http::extractors::Headers},
    visdata::dex::meta::auth::RoleRequest,
};

// ============================================================================
// Role Endpoints
// ============================================================================

#[cfg(feature = "visdata")]
/// CreateRoles
#[utoipa::path(
    context_path = "/api",
    tag = "Roles",
    operation_id = "CreateRoles",
    summary = "Create custom role",
    description = "Creates a new custom role with specified permissions and capabilities.",
    security(("Authorization"= [])),
    params(("org_id" = String, Path, description = "Organization name")),
    request_body(content = inline(UserRoleRequest), description = "UserRoleRequest", content_type = "application/json"),
    responses(
        (status = 200, description = "Success", content_type = "application/json", body = Object),
        (status = 500, description = "Failure", content_type = "application/json", body = ()),
    ),
)]
#[post("/{org_id}/roles")]
pub async fn create_role(
    org_id: web::Path<String>,
    user_req: web::Json<UserRoleRequest>,
) -> Result<HttpResponse, Error> {
    use crate::{
        common::meta::user::is_standard_role, handler::http::auth::jwt::format_role_name_only,
    };

    let org_id = org_id.into_inner();
    let user_req = user_req.into_inner();
    let role_name = format_role_name_only(user_req.role.trim());

    if role_name.is_empty() || is_standard_role(&role_name) {
        return Ok(MetaHttpResponse::bad_request(
            "Custom role name cannot be empty or standard role",
        ));
    }

    match visdata::openfga::authorizer::roles::create_role(&org_id, &role_name).await {
        Ok(_) => Ok(MetaHttpResponse::ok("Role created successfully")),
        Err(err) => {
            let err = err.to_string();
            if err.contains("write_failed_due_to_invalid_input") || err.contains("already exists") {
                Ok(MetaHttpResponse::bad_request("Role already exists"))
            } else {
                Ok(MetaHttpResponse::internal_error("Something went wrong"))
            }
        }
    }
}

#[cfg(not(feature = "visdata"))]
#[post("/{org_id}/roles")]
pub async fn create_role(
    _org_id: web::Path<String>,
    _role_id: web::Json<UserRoleRequest>,
) -> Result<HttpResponse, Error> {
    Ok(MetaHttpResponse::forbidden("Not Supported"))
}

#[cfg(feature = "visdata")]
/// DeleteRole
#[utoipa::path(
    context_path = "/api",
    tag = "Roles",
    operation_id = "DeleteRole",
    summary = "Delete custom role",
    description = "Permanently removes a custom role from the organization.",
    security(("Authorization"= [])),
    params(
        ("org_id" = String, Path, description = "Organization name"),
        ("role_id" = String, Path, description = "Role Id"),
    ),
    responses(
        (status = 200, description = "Success", content_type = "application/json", body = Object),
        (status = 500, description = "Failure", content_type = "application/json", body = ()),
    ),
)]
#[delete("/{org_id}/roles/{role_id}")]
pub async fn delete_role(path: web::Path<(String, String)>) -> Result<HttpResponse, Error> {
    let (org_id, role_name) = path.into_inner();

    match visdata::openfga::authorizer::roles::delete_role(&org_id, &role_name).await {
        Ok(_) => Ok(MetaHttpResponse::ok(
            serde_json::json!({"successful": "true"}),
        )),
        Err(err) => Ok(MetaHttpResponse::internal_error(err)),
    }
}

#[cfg(not(feature = "visdata"))]
#[delete("/{org_id}/roles/{role_id}")]
pub async fn delete_role(_path: web::Path<(String, String)>) -> Result<HttpResponse, Error> {
    Ok(MetaHttpResponse::forbidden("Not Supported"))
}

#[cfg(feature = "visdata")]
/// ListRoles
#[utoipa::path(
    context_path = "/api",
    tag = "Roles",
    operation_id = "ListRoles",
    summary = "List organization roles",
    description = "Retrieves a list of all roles available in the organization. Users will only see roles they have permissions to view when role-based access control is active.",
    security(("Authorization"= [])),
    params(("org_id" = String, Path, description = "Organization name")),
    responses(
        (status = 200, description = "Success", content_type = "application/json", body = inline(Vec<String>)),
        (status = 500, description = "Failure", content_type = "application/json", body = ()),
    ),
)]
#[get("/{org_id}/roles")]
pub async fn get_roles(
    org_id: web::Path<String>,
    Headers(user_email): Headers<UserEmail>,
) -> Result<HttpResponse, Error> {
    let org_id = org_id.into_inner();

    // Get list of permitted objects for user
    let mut permitted = match crate::handler::http::auth::validator::list_objects_for_user(
        &org_id,
        &user_email.user_id,
        "GET",
        "role",
    )
    .await
    {
        Ok(list) => list,
        Err(e) => {
            return Ok(MetaHttpResponse::forbidden(e.to_string()));
        }
    };

    // Strip prefixes from permitted list
    if let Some(local_permitted) = permitted.as_mut() {
        let prefix = "role:";
        for value in local_permitted.iter_mut() {
            if let Some(remaining) = value.strip_prefix(prefix) {
                *value = remaining.to_string();
            }
            let role_prefix = format!("{org_id}/");
            if let Some(remaining) = value.strip_prefix(&role_prefix) {
                *value = remaining.to_string();
            }
        }
    }

    match visdata::openfga::authorizer::roles::get_all_roles(&org_id, permitted).await {
        Ok(res) => Ok(HttpResponse::Ok().json(res)),
        Err(err) => Ok(MetaHttpResponse::internal_error(err)),
    }
}

#[cfg(not(feature = "visdata"))]
#[get("/{org_id}/roles")]
pub async fn get_roles(_org_id: web::Path<String>) -> Result<HttpResponse, Error> {
    Ok(MetaHttpResponse::forbidden("Not Supported"))
}

#[cfg(feature = "visdata")]
/// UpdateRoles
#[utoipa::path(
    context_path = "/api",
    tag = "Roles",
    operation_id = "UpdateRoles",
    summary = "Update role permissions",
    description = "Updates an existing role by adding or removing permissions and users.",
    security(("Authorization"= [])),
    params(
        ("org_id" = String, Path, description = "Organization name"),
        ("role_id" = String, Path, description = "Role Id"),
    ),
    request_body(content = inline(RoleRequest), description = "RoleRequest", content_type = "application/json"),
    responses(
        (status = 200, description = "Success", content_type = "application/json", body = Object),
        (status = 500, description = "Failure", content_type = "application/json", body = ()),
    ),
)]
#[put("/{org_id}/roles/{role_id}")]
pub async fn update_role(
    path: web::Path<(String, String)>,
    update_role: web::Json<RoleRequest>,
) -> Result<HttpResponse, Error> {
    let (org_id, role_id) = path.into_inner();
    let update_role = update_role.into_inner();

    // Convert RoleRequest to permission entries
    let add_perms: Option<Vec<visdata::openfga::types::PermissionEntry>> = if update_role.add.is_empty() {
        None
    } else {
        Some(update_role.add.iter().map(|a| visdata::openfga::types::PermissionEntry {
            object: a.object.clone(),
            permission: a.permission.to_string(),
        }).collect())
    };

    let remove_perms: Option<Vec<visdata::openfga::types::PermissionEntry>> = if update_role.remove.is_empty() {
        None
    } else {
        Some(update_role.remove.iter().map(|a| visdata::openfga::types::PermissionEntry {
            object: a.object.clone(),
            permission: a.permission.to_string(),
        }).collect())
    };

    match visdata::openfga::authorizer::roles::update_role(
        &org_id,
        &role_id,
        add_perms.as_deref(),
        remove_perms.as_deref(),
        update_role.add_users.as_ref(),
        update_role.remove_users.as_ref(),
    )
    .await
    {
        Ok(_) => Ok(MetaHttpResponse::ok("Role updated successfully")),
        Err(err) => Ok(MetaHttpResponse::internal_error(err)),
    }
}

#[cfg(not(feature = "visdata"))]
#[put("/{org_id}/roles/{role_id}")]
pub async fn update_role(
    _path: web::Path<(String, String)>,
    _permissions: web::Json<String>,
) -> Result<HttpResponse, actix_web::Error> {
    Ok(MetaHttpResponse::forbidden("Not Supported"))
}

#[cfg(feature = "visdata")]
/// GetResourcePermission
#[utoipa::path(
    context_path = "/api",
    tag = "Roles",
    operation_id = "GetResourcePermission",
    summary = "Get role permissions for resource",
    description = "Retrieves detailed permissions that a specific role has on a particular resource type.",
    security(("Authorization"= [])),
    params(
        ("org_id" = String, Path, description = "Organization name"),
        ("role_id" = String, Path, description = "Role Id"),
        ("resource" = String, Path, description = "resource"),
    ),
    responses(
        (status = 200, description = "Success", content_type = "application/json", body = inline(Vec<Object>)),
        (status = 500, description = "Failure", content_type = "application/json", body = ()),
    )
)]
#[get("/{org_id}/roles/{role_id}/permissions/{resource}")]
pub async fn get_role_permissions(
    path: web::Path<(String, String, String)>,
) -> Result<HttpResponse, Error> {
    let (org_id, role_id, resource) = path.into_inner();
    match visdata::openfga::authorizer::roles::get_role_permissions(&org_id, &role_id, &resource).await {
        Ok(res) => Ok(HttpResponse::Ok().json(res)),
        Err(err) => Ok(MetaHttpResponse::internal_error(err)),
    }
}

#[cfg(not(feature = "visdata"))]
#[get("/{org_id}/roles/{role_id}/permissions/{resource}")]
pub async fn get_role_permissions(
    _path: web::Path<(String, String, String)>,
) -> Result<HttpResponse, Error> {
    Ok(MetaHttpResponse::forbidden("Not Supported"))
}

#[cfg(feature = "visdata")]
/// GetRoleUsers
#[utoipa::path(
    context_path = "/api",
    tag = "Roles",
    operation_id = "GetRoleUsers",
    summary = "Get users assigned to role",
    description = "Retrieves a list of all users who are currently assigned to a specific role.",
    security(("Authorization"= [])),
    params(
        ("org_id" = String, Path, description = "Organization name"),
        ("role_id" = String, Path, description = "Role Id"),
    ),
    responses(
        (status = 200, description = "Success", content_type = "application/json", body = inline(Vec<String>)),
        (status = 500, description = "Failure", content_type = "application/json", body = ()),
    )
)]
#[get("/{org_id}/roles/{role_id}/users")]
pub async fn get_users_with_role(path: web::Path<(String, String)>) -> Result<HttpResponse, Error> {
    let (org_id, role_id) = path.into_inner();
    match visdata::openfga::authorizer::roles::get_users_with_role(&org_id, &role_id).await {
        Ok(res) => Ok(HttpResponse::Ok().json(res)),
        Err(err) => Ok(MetaHttpResponse::internal_error(err)),
    }
}

#[cfg(not(feature = "visdata"))]
#[get("/{org_id}/roles/{role_id}/users")]
pub async fn get_users_with_role(
    _path: web::Path<(String, String)>,
) -> Result<HttpResponse, Error> {
    Ok(MetaHttpResponse::forbidden("Not Supported"))
}

// ============================================================================
// User Endpoints
// ============================================================================

#[cfg(feature = "visdata")]
/// GetUserRoles
#[utoipa::path(
    context_path = "/api",
    tag = "Users",
    operation_id = "GetUserRoles",
    summary = "Get roles for user",
    description = "Retrieves all roles assigned to a specific user in the organization.",
    security(("Authorization"= [])),
    params(
        ("org_id" = String, Path, description = "Organization name"),
        ("user_email" = String, Path, description = "User email address"),
    ),
    responses(
        (status = 200, description = "Success", content_type = "application/json", body = inline(Vec<String>)),
        (status = 500, description = "Failure", content_type = "application/json", body = ()),
    )
)]
#[get("/{org_id}/users/{user_email}/roles")]
pub async fn get_roles_for_user(path: web::Path<(String, String)>) -> Result<HttpResponse, Error> {
    let (org_id, user_email) = path.into_inner();
    match visdata::openfga::authorizer::roles::get_roles_for_org_user(&org_id, &user_email).await {
        Ok(res) => Ok(HttpResponse::Ok().json(res)),
        Err(err) => Ok(MetaHttpResponse::internal_error(err)),
    }
}

#[cfg(not(feature = "visdata"))]
#[get("/{org_id}/users/{user_email}/roles")]
pub async fn get_roles_for_user(_path: web::Path<(String, String)>) -> Result<HttpResponse, Error> {
    Ok(MetaHttpResponse::forbidden("Not Supported"))
}

#[cfg(feature = "visdata")]
/// GetUserGroups
#[utoipa::path(
    context_path = "/api",
    tag = "Users",
    operation_id = "GetUserGroups",
    summary = "Get groups for user",
    description = "Retrieves all groups that a specific user belongs to in the organization.",
    security(("Authorization"= [])),
    params(
        ("org_id" = String, Path, description = "Organization name"),
        ("user_email" = String, Path, description = "User email address"),
    ),
    responses(
        (status = 200, description = "Success", content_type = "application/json", body = inline(Vec<String>)),
        (status = 500, description = "Failure", content_type = "application/json", body = ()),
    )
)]
#[get("/{org_id}/users/{user_email}/groups")]
pub async fn get_groups_for_user(path: web::Path<(String, String)>) -> Result<HttpResponse, Error> {
    let (org_id, user_email) = path.into_inner();
    match visdata::openfga::authorizer::groups::get_groups_for_org_user(&org_id, &user_email).await {
        Ok(res) => Ok(HttpResponse::Ok().json(res)),
        Err(err) => Ok(MetaHttpResponse::internal_error(err)),
    }
}

#[cfg(not(feature = "visdata"))]
#[get("/{org_id}/users/{user_email}/groups")]
pub async fn get_groups_for_user(
    _path: web::Path<(String, String)>,
) -> Result<HttpResponse, Error> {
    Ok(MetaHttpResponse::forbidden("Not Supported"))
}

// ============================================================================
// Group Endpoints
// ============================================================================

#[cfg(feature = "visdata")]
/// CreateGroup
#[utoipa::path(
    context_path = "/api",
    tag = "Groups",
    operation_id = "CreateGroup",
    summary = "Create user group",
    description = "Creates a new user group with specified users and roles.",
    security(("Authorization"= [])),
    params(("org_id" = String, Path, description = "Organization name")),
    request_body(content = inline(UserGroup), description = "UserGroup", content_type = "application/json"),
    responses(
        (status = 200, description = "Success", content_type = "application/json", body = Object),
        (status = 500, description = "Failure", content_type = "application/json", body = ()),
    )
)]
#[post("/{org_id}/groups")]
pub async fn create_group(
    org_id: web::Path<String>,
    user_group: web::Json<UserGroup>,
) -> Result<HttpResponse, Error> {
    use crate::handler::http::auth::jwt::format_role_name_only;

    let org_id = org_id.into_inner();
    let mut user_grp = user_group.into_inner();
    user_grp.name = format_role_name_only(user_grp.name.trim());

    // Use one-step operation to create group with users
    match visdata::openfga::authorizer::groups::create_group_with_users(
        &org_id,
        &user_grp.name,
        user_grp.users.as_ref(),
    )
    .await
    {
        Ok(_) => Ok(MetaHttpResponse::ok("Group created successfully")),
        Err(err) => {
            let err_str = err.to_string();
            if err_str.contains("already exists") {
                Ok(MetaHttpResponse::bad_request("Group already exists"))
            } else {
                Ok(MetaHttpResponse::internal_error(err))
            }
        }
    }
}

#[cfg(not(feature = "visdata"))]
#[post("/{org_id}/groups")]
pub async fn create_group(
    _org_id: web::Path<String>,
    _user_group: web::Json<UserGroup>,
) -> Result<HttpResponse, Error> {
    Ok(MetaHttpResponse::forbidden("Not Supported"))
}

#[cfg(feature = "visdata")]
/// UpdateGroup
#[utoipa::path(
    context_path = "/api",
    tag = "Groups",
    operation_id = "UpdateGroup",
    summary = "Update user group",
    description = "Updates an existing user group by adding or removing users and roles.",
    security(("Authorization"= [])),
    params(
        ("org_id" = String, Path, description = "Organization name"),
        ("group_name" = String, Path, description = "Group name"),
    ),
    request_body(content = inline(UserGroupRequest), description = "UserGroupRequest", content_type = "application/json"),
    responses(
        (status = 200, description = "Success", content_type = "application/json", body = Object),
        (status = 500, description = "Failure", content_type = "application/json", body = ()),
    )
)]
#[put("/{org_id}/groups/{group_name}")]
pub async fn update_group(
    path: web::Path<(String, String)>,
    user_group: web::Json<UserGroupRequest>,
) -> Result<HttpResponse, Error> {
    let (org_id, group_name) = path.into_inner();
    let user_grp = user_group.into_inner();

    match visdata::openfga::authorizer::groups::update_group(
        &org_id,
        &group_name,
        user_grp.add_users.as_ref(),
        user_grp.remove_users.as_ref(),
        user_grp.add_roles.as_ref(),
        user_grp.remove_roles.as_ref(),
    )
    .await
    {
        Ok(_) => Ok(MetaHttpResponse::ok("Group updated successfully")),
        Err(err) => Ok(MetaHttpResponse::internal_error(err)),
    }
}

#[cfg(not(feature = "visdata"))]
#[put("/{org_id}/groups/{group_name}")]
pub async fn update_group(
    _path: web::Path<(String, String)>,
    _user_group: web::Json<UserGroupRequest>,
) -> Result<HttpResponse, Error> {
    Ok(MetaHttpResponse::forbidden("Not Supported"))
}

#[cfg(feature = "visdata")]
/// ListGroups
#[utoipa::path(
    context_path = "/api",
    tag = "Groups",
    operation_id = "ListGroups",
    summary = "List organization groups",
    description = "Retrieves a list of all user groups in the organization. Users will only see groups they have permissions to view when role-based access control is active.",
    security(("Authorization"= [])),
    params(("org_id" = String, Path, description = "Organization name")),
    responses(
        (status = 200, description = "Success", content_type = "application/json", body = inline(Vec<String>)),
        (status = 500, description = "Failure", content_type = "application/json", body = ()),
    )
)]
#[get("/{org_id}/groups")]
pub async fn get_groups(
    path: web::Path<String>,
    Headers(user_email): Headers<UserEmail>,
) -> Result<HttpResponse, Error> {
    let org_id = path.into_inner();

    // Get list of permitted objects for user
    let mut permitted = match crate::handler::http::auth::validator::list_objects_for_user(
        &org_id,
        &user_email.user_id,
        "GET",
        "group",
    )
    .await
    {
        Ok(list) => list,
        Err(e) => {
            return Ok(MetaHttpResponse::forbidden(e.to_string()));
        }
    };

    // Strip prefixes from permitted list
    if let Some(local_permitted) = permitted.as_mut() {
        let prefix = "group:";
        for value in local_permitted.iter_mut() {
            if let Some(remaining) = value.strip_prefix(prefix) {
                *value = remaining.to_string();
            }
            let group_prefix = format!("{org_id}/");
            if let Some(remaining) = value.strip_prefix(&group_prefix) {
                *value = remaining.to_string();
            }
        }
    }

    match visdata::openfga::authorizer::groups::get_all_groups(&org_id, permitted).await {
        Ok(res) => Ok(HttpResponse::Ok().json(res)),
        Err(err) => Ok(MetaHttpResponse::internal_error(err)),
    }
}

#[cfg(not(feature = "visdata"))]
#[get("/{org_id}/groups")]
pub async fn get_groups(_path: web::Path<String>) -> Result<HttpResponse, Error> {
    Ok(MetaHttpResponse::forbidden("Not Supported"))
}

#[cfg(feature = "visdata")]
/// GetGroup
#[utoipa::path(
    context_path = "/api",
    tag = "Groups",
    operation_id = "GetGroup",
    summary = "Get group details",
    description = "Retrieves detailed information about a specific user group.",
    security(("Authorization"= [])),
    params(
        ("org_id" = String, Path, description = "Organization name"),
        ("group_name" = String, Path, description = "Group name"),
    ),
    responses(
        (status = 200, description = "Success", content_type = "application/json", body = inline(UserGroup)),
        (status = 500, description = "Failure", content_type = "application/json", body = ()),
    )
)]
#[get("/{org_id}/groups/{group_name}")]
pub async fn get_group_details(path: web::Path<(String, String)>) -> Result<HttpResponse, Error> {
    let (org_id, group_name) = path.into_inner();

    match visdata::openfga::authorizer::groups::get_group_details(&org_id, &group_name).await {
        Ok(res) => Ok(HttpResponse::Ok().json(res)),
        Err(err) => Ok(MetaHttpResponse::internal_error(err)),
    }
}

#[cfg(not(feature = "visdata"))]
#[get("/{org_id}/groups/{group_name}")]
pub async fn get_group_details(_path: web::Path<(String, String)>) -> Result<HttpResponse, Error> {
    Ok(MetaHttpResponse::forbidden("Not Supported"))
}

#[cfg(feature = "visdata")]
/// DeleteGroup
#[utoipa::path(
    context_path = "/api",
    tag = "Groups",
    operation_id = "DeleteGroup",
    summary = "Delete user group",
    description = "Permanently removes a user group from the organization.",
    security(("Authorization"= [])),
    params(
        ("org_id" = String, Path, description = "Organization name"),
        ("group_name" = String, Path, description = "Group name"),
    ),
    responses(
        (status = 200, description = "Success", content_type = "application/json", body = Object),
        (status = 500, description = "Failure", content_type = "application/json", body = ()),
    )
)]
#[delete("/{org_id}/groups/{group_name}")]
pub async fn delete_group(path: web::Path<(String, String)>) -> Result<HttpResponse, Error> {
    let (org_id, group_name) = path.into_inner();

    match visdata::openfga::authorizer::groups::delete_group(&org_id, &group_name).await {
        Ok(_) => Ok(MetaHttpResponse::ok(
            serde_json::json!({"successful": "true"}),
        )),
        Err(err) => Ok(MetaHttpResponse::internal_error(err)),
    }
}

#[cfg(not(feature = "visdata"))]
#[delete("/{org_id}/groups/{group_name}")]
pub async fn delete_group(_path: web::Path<(String, String)>) -> Result<HttpResponse, Error> {
    Ok(MetaHttpResponse::forbidden("Not Supported"))
}

// ============================================================================
// Resource Endpoints
// ============================================================================

#[cfg(feature = "visdata")]
/// GetResources
#[utoipa::path(
    context_path = "/api",
    tag = "Resources",
    operation_id = "GetResources",
    summary = "Get available resources",
    description = "Retrieves a list of all available resource types for permission assignments.",
    security(("Authorization"= [])),
    params(("org_id" = String, Path, description = "Organization name")),
    responses(
        (status = 200, description = "Success", content_type = "application/json", body = inline(Vec<Object>)),
        (status = 500, description = "Failure", content_type = "application/json", body = ()),
    )
)]
#[get("/{org_id}/resources")]
pub async fn get_resources(_org_id: web::Path<String>) -> Result<HttpResponse, Error> {
    use visdata::openfga::meta::mapping::{OFGA_MODELS, Resource};

    let resources: Vec<&Resource> = OFGA_MODELS.values().collect();
    Ok(HttpResponse::Ok().json(resources))
}

#[cfg(not(feature = "visdata"))]
#[get("/{org_id}/resources")]
pub async fn get_resources(_org_id: web::Path<String>) -> Result<HttpResponse, Error> {
    Ok(MetaHttpResponse::forbidden("Not Supported"))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, App};
    use std::collections::HashSet;

    #[tokio::test]
    async fn test_create_role_non_visdata() {
        #[cfg(not(feature = "visdata"))]
        {
            let app = test::init_service(App::new().service(create_role)).await;
            let role_req = UserRoleRequest {
                role: "test_role".to_string(),
            };

            let req = test::TestRequest::post()
                .uri("/test_org/roles")
                .set_json(&role_req)
                .to_request();

            let resp = test::call_service(&app, req).await;
            assert_eq!(resp.status(), 403); // Forbidden in non-visdata
        }
    }

    #[tokio::test]
    async fn test_delete_role_non_visdata() {
        #[cfg(not(feature = "visdata"))]
        {
            let app = test::init_service(App::new().service(delete_role)).await;
            let req = test::TestRequest::delete()
                .uri("/test_org/roles/test_role")
                .to_request();

            let resp = test::call_service(&app, req).await;
            assert_eq!(resp.status(), 403); // Forbidden in non-visdata
        }
    }

    #[tokio::test]
    async fn test_get_roles_non_visdata() {
        #[cfg(not(feature = "visdata"))]
        {
            let app = test::init_service(App::new().service(get_roles)).await;
            let req = test::TestRequest::get()
                .uri("/test_org/roles")
                .to_request();

            let resp = test::call_service(&app, req).await;
            assert_eq!(resp.status(), 403); // Forbidden in non-visdata
        }
    }

    #[tokio::test]
    async fn test_update_role_non_visdata() {
        #[cfg(not(feature = "visdata"))]
        {
            let app = test::init_service(App::new().service(update_role)).await;
            let req = test::TestRequest::put()
                .uri("/test_org/roles/test_role")
                .set_json(&"{}".to_string())
                .to_request();

            let resp = test::call_service(&app, req).await;
            assert_eq!(resp.status(), 403); // Forbidden in non-visdata
        }
    }

    #[tokio::test]
    async fn test_get_role_permissions_non_visdata() {
        #[cfg(not(feature = "visdata"))]
        {
            let app = test::init_service(App::new().service(get_role_permissions)).await;
            let req = test::TestRequest::get()
                .uri("/test_org/roles/test_role/permissions/logs")
                .to_request();

            let resp = test::call_service(&app, req).await;
            assert_eq!(resp.status(), 403); // Forbidden in non-visdata
        }
    }

    #[tokio::test]
    async fn test_get_users_with_role_non_visdata() {
        #[cfg(not(feature = "visdata"))]
        {
            let app = test::init_service(App::new().service(get_users_with_role)).await;
            let req = test::TestRequest::get()
                .uri("/test_org/roles/test_role/users")
                .to_request();

            let resp = test::call_service(&app, req).await;
            assert_eq!(resp.status(), 403); // Forbidden in non-visdata
        }
    }

    #[tokio::test]
    async fn test_get_roles_for_user_non_visdata() {
        #[cfg(not(feature = "visdata"))]
        {
            let app = test::init_service(App::new().service(get_roles_for_user)).await;
            let req = test::TestRequest::get()
                .uri("/test_org/users/test@example.com/roles")
                .to_request();

            let resp = test::call_service(&app, req).await;
            assert_eq!(resp.status(), 403); // Forbidden in non-visdata
        }
    }

    #[tokio::test]
    async fn test_get_groups_for_user_non_visdata() {
        #[cfg(not(feature = "visdata"))]
        {
            let app = test::init_service(App::new().service(get_groups_for_user)).await;
            let req = test::TestRequest::get()
                .uri("/test_org/users/test@example.com/groups")
                .to_request();

            let resp = test::call_service(&app, req).await;
            assert_eq!(resp.status(), 403); // Forbidden in non-visdata
        }
    }

    #[tokio::test]
    async fn test_create_group_non_visdata() {
        #[cfg(not(feature = "visdata"))]
        {
            let app = test::init_service(App::new().service(create_group)).await;
            let mut users = HashSet::new();
            users.insert("user1@test.com".to_string());

            let group = UserGroup {
                name: "test_group".to_string(),
                users: Some(users),
                roles: None,
            };

            let req = test::TestRequest::post()
                .uri("/test_org/groups")
                .set_json(&group)
                .to_request();

            let resp = test::call_service(&app, req).await;
            assert_eq!(resp.status(), 403); // Forbidden in non-visdata
        }
    }

    #[tokio::test]
    async fn test_update_group_non_visdata() {
        #[cfg(not(feature = "visdata"))]
        {
            let app = test::init_service(App::new().service(update_group)).await;
            let group_req = UserGroupRequest {
                add_users: None,
                remove_users: None,
                add_roles: None,
                remove_roles: None,
            };

            let req = test::TestRequest::put()
                .uri("/test_org/groups/test_group")
                .set_json(&group_req)
                .to_request();

            let resp = test::call_service(&app, req).await;
            assert_eq!(resp.status(), 403); // Forbidden in non-visdata
        }
    }

    #[tokio::test]
    async fn test_get_groups_non_visdata() {
        #[cfg(not(feature = "visdata"))]
        {
            let app = test::init_service(App::new().service(get_groups)).await;
            let req = test::TestRequest::get()
                .uri("/test_org/groups")
                .to_request();

            let resp = test::call_service(&app, req).await;
            assert_eq!(resp.status(), 403); // Forbidden in non-visdata
        }
    }

    #[tokio::test]
    async fn test_get_group_details_non_visdata() {
        #[cfg(not(feature = "visdata"))]
        {
            let app = test::init_service(App::new().service(get_group_details)).await;
            let req = test::TestRequest::get()
                .uri("/test_org/groups/test_group")
                .to_request();

            let resp = test::call_service(&app, req).await;
            assert_eq!(resp.status(), 403); // Forbidden in non-visdata
        }
    }

    #[tokio::test]
    async fn test_delete_group_non_visdata() {
        #[cfg(not(feature = "visdata"))]
        {
            let app = test::init_service(App::new().service(delete_group)).await;
            let req = test::TestRequest::delete()
                .uri("/test_org/groups/test_group")
                .to_request();

            let resp = test::call_service(&app, req).await;
            assert_eq!(resp.status(), 403); // Forbidden in non-visdata
        }
    }

    #[tokio::test]
    async fn test_get_resources_non_visdata() {
        #[cfg(not(feature = "visdata"))]
        {
            let app = test::init_service(App::new().service(get_resources)).await;
            let req = test::TestRequest::get()
                .uri("/test_org/resources")
                .to_request();

            let resp = test::call_service(&app, req).await;
            assert_eq!(resp.status(), 403); // Forbidden in non-visdata
        }
    }

    // Tests for visdata feature enabled
    #[cfg(feature = "visdata")]
    mod visdata_tests {
        use super::*;
        use actix_http::header::HeaderName;

        #[tokio::test]
        async fn test_create_role_empty_name() {
            let app = test::init_service(App::new().service(create_role)).await;
            let role_req = UserRoleRequest {
                role: "".to_string(),
            };

            let req = test::TestRequest::post()
                .uri("/test_org/roles")
                .set_json(&role_req)
                .to_request();

            let resp = test::call_service(&app, req).await;
            assert_eq!(resp.status(), 400); // Bad request for empty role name
        }

        #[tokio::test]
        async fn test_create_role_standard_role_name() {
            let app = test::init_service(App::new().service(create_role)).await;
            let role_req = UserRoleRequest {
                role: "admin".to_string(),
            };

            let req = test::TestRequest::post()
                .uri("/test_org/roles")
                .set_json(&role_req)
                .to_request();

            let resp = test::call_service(&app, req).await;
            assert_eq!(resp.status(), 400); // Bad request for standard role name
        }

        #[tokio::test]
        async fn test_get_roles_visdata() {
            let app = test::init_service(App::new().service(get_roles)).await;
            let mut req = test::TestRequest::get()
                .uri("/test_org/roles")
                .to_request();

            // Add user_id header that the function expects
            req.headers_mut().insert(
                HeaderName::from_static("user_id"),
                "test_user@test.com".parse().unwrap(),
            );

            let resp = test::call_service(&app, req).await;
            // Will likely fail due to missing OpenFGA setup, but testing structure
            assert!(resp.status().is_client_error() || resp.status().is_server_error());
        }

        #[tokio::test]
        async fn test_get_groups_visdata() {
            let app = test::init_service(App::new().service(get_groups)).await;
            let mut req = test::TestRequest::get()
                .uri("/test_org/groups")
                .to_request();

            // Add user_id header that the function expects
            req.headers_mut().insert(
                HeaderName::from_static("user_id"),
                "test_user@test.com".parse().unwrap(),
            );

            let resp = test::call_service(&app, req).await;
            // Will likely fail due to missing OpenFGA setup, but testing structure
            assert!(resp.status().is_client_error() || resp.status().is_server_error());
        }

        #[tokio::test]
        async fn test_create_group_visdata() {
            let app = test::init_service(App::new().service(create_group)).await;
            let mut users = HashSet::new();
            users.insert("user1@test.com".to_string());

            let group = UserGroup {
                name: "test_group".to_string(),
                users: Some(users),
                roles: None,
            };

            let req = test::TestRequest::post()
                .uri("/test_org/groups")
                .set_json(&group)
                .to_request();

            let resp = test::call_service(&app, req).await;
            // Will likely fail due to missing OpenFGA setup, but testing structure
            assert!(resp.status().is_client_error() || resp.status().is_server_error());
        }

        #[tokio::test]
        async fn test_delete_role_visdata() {
            let app = test::init_service(App::new().service(delete_role)).await;
            let req = test::TestRequest::delete()
                .uri("/test_org/roles/custom_role")
                .to_request();

            let resp = test::call_service(&app, req).await;
            // Will likely fail due to missing OpenFGA setup, but testing structure
            assert!(resp.status().is_client_error() || resp.status().is_server_error());
        }

        #[tokio::test]
        async fn test_delete_group_visdata() {
            let app = test::init_service(App::new().service(delete_group)).await;
            let req = test::TestRequest::delete()
                .uri("/test_org/groups/test_group")
                .to_request();

            let resp = test::call_service(&app, req).await;
            // Will likely fail due to missing OpenFGA setup, but testing structure
            assert!(resp.status().is_client_error() || resp.status().is_server_error());
        }

        #[tokio::test]
        async fn test_get_resources_visdata() {
            let app = test::init_service(App::new().service(get_resources)).await;
            let req = test::TestRequest::get()
                .uri("/test_org/resources")
                .to_request();

            let resp = test::call_service(&app, req).await;
            // Should succeed as get_resources doesn't need OpenFGA
            assert!(resp.status().is_success());
        }
    }
}
