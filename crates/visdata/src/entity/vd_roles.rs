// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Role entity

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "vd_roles")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub org_id: String,
    pub name: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    #[sea_orm(default_value = false)]
    pub is_system: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::vd_role_permissions::Entity")]
    Permissions,
    #[sea_orm(has_many = "super::vd_role_users::Entity")]
    Users,
    #[sea_orm(has_many = "super::vd_group_roles::Entity")]
    GroupRoles,
}

impl Related<super::vd_role_permissions::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Permissions.def()
    }
}

impl Related<super::vd_role_users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Users.def()
    }
}

impl Related<super::vd_group_roles::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::GroupRoles.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
