// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! SSO provider entity

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "vd_sso_providers")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub org_id: String,
    /// Provider type: oidc or ldap
    pub provider_type: String,
    pub name: String,
    #[sea_orm(default_value = true)]
    pub is_enabled: bool,
    #[sea_orm(default_value = false)]
    pub is_default: bool,
    /// Encrypted configuration JSON
    pub config_json: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::vd_sso_user_mappings::Entity")]
    UserMappings,
}

impl Related<super::vd_sso_user_mappings::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserMappings.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
