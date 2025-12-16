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

//! Migration to create VisData SSO and RBAC tables.
//! Only runs when the 'visdata' feature is enabled.

use sea_orm_migration::prelude::*;

use super::get_text_type;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create vd_roles table
        manager
            .create_table(create_vd_roles_table())
            .await?;

        // Create vd_role_permissions table
        manager
            .create_table(create_vd_role_permissions_table())
            .await?;

        // Create vd_role_users table
        manager
            .create_table(create_vd_role_users_table())
            .await?;

        // Create vd_groups table
        manager
            .create_table(create_vd_groups_table())
            .await?;

        // Create vd_group_roles table
        manager
            .create_table(create_vd_group_roles_table())
            .await?;

        // Create vd_group_users table
        manager
            .create_table(create_vd_group_users_table())
            .await?;

        // Create vd_sso_providers table
        manager
            .create_table(create_vd_sso_providers_table())
            .await?;

        // Create vd_sso_user_mappings table
        manager
            .create_table(create_vd_sso_user_mappings_table())
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop tables in reverse order to handle foreign key constraints
        manager
            .drop_table(Table::drop().table(VdSsoUserMappings::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(VdSsoProviders::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(VdGroupUsers::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(VdGroupRoles::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(VdGroups::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(VdRoleUsers::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(VdRolePermissions::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(VdRoles::Table).to_owned())
            .await?;
        Ok(())
    }
}

// ============================================================================
// vd_roles - Role definitions
// ============================================================================

fn create_vd_roles_table() -> TableCreateStatement {
    Table::create()
        .table(VdRoles::Table)
        .if_not_exists()
        .col(
            ColumnDef::new(VdRoles::Id)
                .string_len(27)
                .not_null()
                .primary_key(),
        )
        .col(
            ColumnDef::new(VdRoles::OrgId)
                .string_len(100)
                .not_null(),
        )
        .col(
            ColumnDef::new(VdRoles::Name)
                .string_len(100)
                .not_null(),
        )
        .col(
            ColumnDef::new(VdRoles::DisplayName)
                .string_len(200),
        )
        .col(
            ColumnDef::new(VdRoles::Description)
                .custom(Alias::new(&get_text_type())),
        )
        .col(
            ColumnDef::new(VdRoles::IsSystem)
                .boolean()
                .not_null()
                .default(false),
        )
        .col(
            ColumnDef::new(VdRoles::CreatedAt)
                .big_integer()
                .not_null(),
        )
        .col(
            ColumnDef::new(VdRoles::UpdatedAt)
                .big_integer()
                .not_null(),
        )
        .index(
            Index::create()
                .unique()
                .name("idx_vd_roles_org_name")
                .col(VdRoles::OrgId)
                .col(VdRoles::Name),
        )
        .to_owned()
}

#[derive(DeriveIden)]
enum VdRoles {
    Table,
    Id,
    OrgId,
    Name,
    DisplayName,
    Description,
    IsSystem,
    CreatedAt,
    UpdatedAt,
}

// ============================================================================
// vd_role_permissions - Role permission assignments
// ============================================================================

fn create_vd_role_permissions_table() -> TableCreateStatement {
    Table::create()
        .table(VdRolePermissions::Table)
        .if_not_exists()
        .col(
            ColumnDef::new(VdRolePermissions::Id)
                .string_len(27)
                .not_null()
                .primary_key(),
        )
        .col(
            ColumnDef::new(VdRolePermissions::RoleId)
                .string_len(27)
                .not_null(),
        )
        .col(
            ColumnDef::new(VdRolePermissions::OrgId)
                .string_len(100)
                .not_null(),
        )
        .col(
            ColumnDef::new(VdRolePermissions::Object)
                .string_len(500)
                .not_null(),
        )
        .col(
            ColumnDef::new(VdRolePermissions::Permission)
                .string_len(50)
                .not_null(),
        )
        .col(
            ColumnDef::new(VdRolePermissions::CreatedAt)
                .big_integer()
                .not_null(),
        )
        .index(
            Index::create()
                .unique()
                .name("idx_vd_role_perms_unique")
                .col(VdRolePermissions::RoleId)
                .col(VdRolePermissions::Object)
                .col(VdRolePermissions::Permission),
        )
        .index(
            Index::create()
                .name("idx_vd_role_perms_role_org")
                .col(VdRolePermissions::RoleId)
                .col(VdRolePermissions::OrgId),
        )
        .index(
            Index::create()
                .name("idx_vd_role_perms_object")
                .col(VdRolePermissions::Object),
        )
        .to_owned()
}

#[derive(DeriveIden)]
enum VdRolePermissions {
    Table,
    Id,
    RoleId,
    OrgId,
    Object,
    Permission,
    CreatedAt,
}

// ============================================================================
// vd_role_users - Role to user assignments
// ============================================================================

fn create_vd_role_users_table() -> TableCreateStatement {
    Table::create()
        .table(VdRoleUsers::Table)
        .if_not_exists()
        .col(
            ColumnDef::new(VdRoleUsers::Id)
                .string_len(27)
                .not_null()
                .primary_key(),
        )
        .col(
            ColumnDef::new(VdRoleUsers::RoleId)
                .string_len(27)
                .not_null(),
        )
        .col(
            ColumnDef::new(VdRoleUsers::OrgId)
                .string_len(100)
                .not_null(),
        )
        .col(
            ColumnDef::new(VdRoleUsers::UserEmail)
                .string_len(100)
                .not_null(),
        )
        .col(
            ColumnDef::new(VdRoleUsers::CreatedAt)
                .big_integer()
                .not_null(),
        )
        .index(
            Index::create()
                .unique()
                .name("idx_vd_role_users_unique")
                .col(VdRoleUsers::RoleId)
                .col(VdRoleUsers::OrgId)
                .col(VdRoleUsers::UserEmail),
        )
        .index(
            Index::create()
                .name("idx_vd_role_users_user_org")
                .col(VdRoleUsers::UserEmail)
                .col(VdRoleUsers::OrgId),
        )
        .to_owned()
}

#[derive(DeriveIden)]
enum VdRoleUsers {
    Table,
    Id,
    RoleId,
    OrgId,
    UserEmail,
    CreatedAt,
}

// ============================================================================
// vd_groups - Group definitions
// ============================================================================

fn create_vd_groups_table() -> TableCreateStatement {
    Table::create()
        .table(VdGroups::Table)
        .if_not_exists()
        .col(
            ColumnDef::new(VdGroups::Id)
                .string_len(27)
                .not_null()
                .primary_key(),
        )
        .col(
            ColumnDef::new(VdGroups::OrgId)
                .string_len(100)
                .not_null(),
        )
        .col(
            ColumnDef::new(VdGroups::Name)
                .string_len(100)
                .not_null(),
        )
        .col(
            ColumnDef::new(VdGroups::DisplayName)
                .string_len(200),
        )
        .col(
            ColumnDef::new(VdGroups::Description)
                .custom(Alias::new(&get_text_type())),
        )
        .col(
            ColumnDef::new(VdGroups::ExternalId)
                .string_len(256),
        )
        .col(
            ColumnDef::new(VdGroups::CreatedAt)
                .big_integer()
                .not_null(),
        )
        .col(
            ColumnDef::new(VdGroups::UpdatedAt)
                .big_integer()
                .not_null(),
        )
        .index(
            Index::create()
                .unique()
                .name("idx_vd_groups_org_name")
                .col(VdGroups::OrgId)
                .col(VdGroups::Name),
        )
        .to_owned()
}

#[derive(DeriveIden)]
enum VdGroups {
    Table,
    Id,
    OrgId,
    Name,
    DisplayName,
    Description,
    ExternalId,
    CreatedAt,
    UpdatedAt,
}

// ============================================================================
// vd_group_roles - Group to role assignments
// ============================================================================

fn create_vd_group_roles_table() -> TableCreateStatement {
    Table::create()
        .table(VdGroupRoles::Table)
        .if_not_exists()
        .col(
            ColumnDef::new(VdGroupRoles::Id)
                .string_len(27)
                .not_null()
                .primary_key(),
        )
        .col(
            ColumnDef::new(VdGroupRoles::GroupId)
                .string_len(27)
                .not_null(),
        )
        .col(
            ColumnDef::new(VdGroupRoles::OrgId)
                .string_len(100)
                .not_null(),
        )
        .col(
            ColumnDef::new(VdGroupRoles::RoleId)
                .string_len(27)
                .not_null(),
        )
        .col(
            ColumnDef::new(VdGroupRoles::CreatedAt)
                .big_integer()
                .not_null(),
        )
        .index(
            Index::create()
                .unique()
                .name("idx_vd_group_roles_unique")
                .col(VdGroupRoles::GroupId)
                .col(VdGroupRoles::RoleId),
        )
        .index(
            Index::create()
                .name("idx_vd_group_roles_group")
                .col(VdGroupRoles::GroupId),
        )
        .index(
            Index::create()
                .name("idx_vd_group_roles_role")
                .col(VdGroupRoles::RoleId),
        )
        .to_owned()
}

#[derive(DeriveIden)]
enum VdGroupRoles {
    Table,
    Id,
    GroupId,
    OrgId,
    RoleId,
    CreatedAt,
}

// ============================================================================
// vd_group_users - Group to user assignments
// ============================================================================

fn create_vd_group_users_table() -> TableCreateStatement {
    Table::create()
        .table(VdGroupUsers::Table)
        .if_not_exists()
        .col(
            ColumnDef::new(VdGroupUsers::Id)
                .string_len(27)
                .not_null()
                .primary_key(),
        )
        .col(
            ColumnDef::new(VdGroupUsers::GroupId)
                .string_len(27)
                .not_null(),
        )
        .col(
            ColumnDef::new(VdGroupUsers::OrgId)
                .string_len(100)
                .not_null(),
        )
        .col(
            ColumnDef::new(VdGroupUsers::UserEmail)
                .string_len(100)
                .not_null(),
        )
        .col(
            ColumnDef::new(VdGroupUsers::CreatedAt)
                .big_integer()
                .not_null(),
        )
        .index(
            Index::create()
                .unique()
                .name("idx_vd_group_users_unique")
                .col(VdGroupUsers::GroupId)
                .col(VdGroupUsers::UserEmail),
        )
        .index(
            Index::create()
                .name("idx_vd_group_users_user_org")
                .col(VdGroupUsers::UserEmail)
                .col(VdGroupUsers::OrgId),
        )
        .to_owned()
}

#[derive(DeriveIden)]
enum VdGroupUsers {
    Table,
    Id,
    GroupId,
    OrgId,
    UserEmail,
    CreatedAt,
}

// ============================================================================
// vd_sso_providers - SSO provider configurations
// ============================================================================

fn create_vd_sso_providers_table() -> TableCreateStatement {
    Table::create()
        .table(VdSsoProviders::Table)
        .if_not_exists()
        .col(
            ColumnDef::new(VdSsoProviders::Id)
                .string_len(27)
                .not_null()
                .primary_key(),
        )
        .col(
            ColumnDef::new(VdSsoProviders::OrgId)
                .string_len(100)
                .not_null(),
        )
        .col(
            ColumnDef::new(VdSsoProviders::ProviderType)
                .string_len(50)
                .not_null(),
        )
        .col(
            ColumnDef::new(VdSsoProviders::Name)
                .string_len(100)
                .not_null(),
        )
        .col(
            ColumnDef::new(VdSsoProviders::IsEnabled)
                .boolean()
                .not_null()
                .default(true),
        )
        .col(
            ColumnDef::new(VdSsoProviders::IsDefault)
                .boolean()
                .not_null()
                .default(false),
        )
        .col(
            ColumnDef::new(VdSsoProviders::ConfigJson)
                .custom(Alias::new(&get_text_type()))
                .not_null(),
        )
        .col(
            ColumnDef::new(VdSsoProviders::CreatedAt)
                .big_integer()
                .not_null(),
        )
        .col(
            ColumnDef::new(VdSsoProviders::UpdatedAt)
                .big_integer()
                .not_null(),
        )
        .index(
            Index::create()
                .unique()
                .name("idx_vd_sso_providers_org_name")
                .col(VdSsoProviders::OrgId)
                .col(VdSsoProviders::Name),
        )
        .to_owned()
}

#[derive(DeriveIden)]
enum VdSsoProviders {
    Table,
    Id,
    OrgId,
    ProviderType,
    Name,
    IsEnabled,
    IsDefault,
    ConfigJson,
    CreatedAt,
    UpdatedAt,
}

// ============================================================================
// vd_sso_user_mappings - SSO user mappings
// ============================================================================

fn create_vd_sso_user_mappings_table() -> TableCreateStatement {
    Table::create()
        .table(VdSsoUserMappings::Table)
        .if_not_exists()
        .col(
            ColumnDef::new(VdSsoUserMappings::Id)
                .string_len(27)
                .not_null()
                .primary_key(),
        )
        .col(
            ColumnDef::new(VdSsoUserMappings::ProviderId)
                .string_len(27)
                .not_null(),
        )
        .col(
            ColumnDef::new(VdSsoUserMappings::ExternalId)
                .string_len(256)
                .not_null(),
        )
        .col(
            ColumnDef::new(VdSsoUserMappings::UserEmail)
                .string_len(100)
                .not_null(),
        )
        .col(
            ColumnDef::new(VdSsoUserMappings::ExternalGroups)
                .custom(Alias::new(&get_text_type())),
        )
        .col(
            ColumnDef::new(VdSsoUserMappings::LastSyncAt)
                .big_integer(),
        )
        .col(
            ColumnDef::new(VdSsoUserMappings::CreatedAt)
                .big_integer()
                .not_null(),
        )
        .index(
            Index::create()
                .unique()
                .name("idx_vd_sso_mappings_provider_ext")
                .col(VdSsoUserMappings::ProviderId)
                .col(VdSsoUserMappings::ExternalId),
        )
        .to_owned()
}

#[derive(DeriveIden)]
enum VdSsoUserMappings {
    Table,
    Id,
    ProviderId,
    ExternalId,
    UserEmail,
    ExternalGroups,
    LastSyncAt,
    CreatedAt,
}

#[cfg(test)]
mod tests {
    use collapse::*;

    use super::*;

    #[test]
    fn postgres_vd_roles() {
        collapsed_eq!(
            &create_vd_roles_table().to_string(PostgresQueryBuilder),
            r#"
                CREATE TABLE IF NOT EXISTS "vd_roles" (
                    "id" varchar(27) NOT NULL PRIMARY KEY,
                    "org_id" varchar(100) NOT NULL,
                    "name" varchar(100) NOT NULL,
                    "display_name" varchar(200),
                    "description" text,
                    "is_system" bool NOT NULL DEFAULT FALSE,
                    "created_at" bigint NOT NULL,
                    "updated_at" bigint NOT NULL
                )
            "#
        );
    }

    #[test]
    fn postgres_vd_role_permissions() {
        collapsed_eq!(
            &create_vd_role_permissions_table().to_string(PostgresQueryBuilder),
            r#"
                CREATE TABLE IF NOT EXISTS "vd_role_permissions" (
                    "id" varchar(27) NOT NULL PRIMARY KEY,
                    "role_id" varchar(27) NOT NULL,
                    "org_id" varchar(100) NOT NULL,
                    "object" varchar(500) NOT NULL,
                    "permission" varchar(50) NOT NULL,
                    "created_at" bigint NOT NULL
                )
            "#
        );
    }

    #[test]
    fn postgres_vd_groups() {
        collapsed_eq!(
            &create_vd_groups_table().to_string(PostgresQueryBuilder),
            r#"
                CREATE TABLE IF NOT EXISTS "vd_groups" (
                    "id" varchar(27) NOT NULL PRIMARY KEY,
                    "org_id" varchar(100) NOT NULL,
                    "name" varchar(100) NOT NULL,
                    "display_name" varchar(200),
                    "description" text,
                    "external_id" varchar(256),
                    "created_at" bigint NOT NULL,
                    "updated_at" bigint NOT NULL
                )
            "#
        );
    }
}
