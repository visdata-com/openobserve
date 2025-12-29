// Copyright 2025 OpenObserve Inc.
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

#[cfg(feature = "enterprise")]
use {
    crate::{
        common::{
            meta::http::HttpResponse as MetaHttpResponse,
            utils::auth::{AuthExtractor, is_root_user},
        },
        service::users::get_user,
    },
    actix_web::HttpResponse,
    config::meta::{stream::StreamType, user::User},
    o2_openfga::meta::mapping::OFGA_MODELS,
};

#[cfg(all(feature = "visdata", not(feature = "enterprise")))]
use {
    crate::{
        common::{
            meta::http::HttpResponse as MetaHttpResponse,
            utils::auth::is_root_user,
        },
        service::users::get_user,
    },
    actix_web::HttpResponse,
    config::meta::{stream::StreamType, user::User},
    visdata::openfga::meta::mapping::OFGA_MODELS,
};

// Check permissions on stream
#[cfg(feature = "enterprise")]
pub async fn check_stream_permissions(
    stream_name: &str,
    org_id: &str,
    user_id: &str,
    stream_type: &StreamType,
) -> Option<HttpResponse> {
    if !is_root_user(user_id) {
        let user: User = get_user(Some(org_id), user_id).await.unwrap();
        let stream_type_str = stream_type.as_str();

        if !crate::handler::http::auth::validator::check_permissions(
            user_id,
            AuthExtractor {
                auth: "".to_string(),
                method: "GET".to_string(),
                o2_type: format!(
                    "{}:{}",
                    OFGA_MODELS
                        .get(stream_type_str)
                        .map_or(stream_type_str, |model| model.key),
                    stream_name
                ),
                org_id: org_id.to_string(),
                bypass_check: false,
                parent_id: "".to_string(),
            },
            user.role,
            user.is_external,
        )
        .await
        {
            return Some(MetaHttpResponse::forbidden("Unauthorized Access"));
        }
    }
    None
}

// Check permissions on stream (visdata version)
#[allow(dead_code)] // Used by patterns handler under visdata feature
#[cfg(all(feature = "visdata", not(feature = "enterprise")))]
pub async fn check_stream_permissions(
    stream_name: &str,
    org_id: &str,
    user_id: &str,
    stream_type: &StreamType,
) -> Option<HttpResponse> {
    if !is_root_user(user_id) {
        let user: User = get_user(Some(org_id), user_id).await.unwrap();
        let stream_type_str = stream_type.as_str();
        let role_str = user.role.to_string();

        // Use visdata's OpenFGA authorizer for permission check
        let resource_type = OFGA_MODELS
            .get(stream_type_str)
            .map_or(stream_type_str.to_string(), |model| model.key.to_string());
        let object = format!("{}:{}", resource_type, stream_name);

        match visdata::openfga::authorizer::authz::is_allowed(
            org_id,
            user_id,
            "GET",     // HTTP method for read access
            &object,   // Format: "resource_type:entity_id"
            "",        // parent_id (not used)
            &role_str,
        )
        .await
        {
            Ok(allowed) => {
                if !allowed {
                    return Some(MetaHttpResponse::forbidden("Unauthorized Access"));
                }
            }
            Err(e) => {
                log::error!("Failed to check permissions for stream {}: {}", stream_name, e);
                // Allow access if OpenFGA is not available (fallback to role-based)
                let role = role_str.as_str();
                if role != "Admin" && role != "Root" && role != "Member" {
                    return Some(MetaHttpResponse::forbidden("Unauthorized Access"));
                }
            }
        }
    }
    None
}

#[cfg(feature = "enterprise")]
pub async fn check_resource_permissions(
    org_id: &str,
    user_id: &str,
    resource_type: &str,
    resource_id: &str,
    method: &str,
) -> Option<HttpResponse> {
    if !is_root_user(user_id) {
        let user: User = get_user(Some(org_id), user_id).await.unwrap();

        if !crate::handler::http::auth::validator::check_permissions(
            user_id,
            AuthExtractor {
                auth: "".to_string(),
                method: method.to_string(),
                o2_type: format!(
                    "{}:{}",
                    OFGA_MODELS
                        .get(resource_type)
                        .map_or(resource_type, |model| model.key),
                    resource_id
                ),
                org_id: org_id.to_string(),
                bypass_check: false,
                parent_id: "".to_string(),
            },
            user.role,
            user.is_external,
        )
        .await
        {
            return Some(MetaHttpResponse::forbidden("Unauthorized Access"));
        }
    }
    None
}
