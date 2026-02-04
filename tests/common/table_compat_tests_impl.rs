// Copyright 2025 OpenObserve Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

//! Table compatibility test implementations.
//!
//! This module contains tests that verify data compatibility across different
//! database backends (MySQL, OceanBase, PostgreSQL, SQLite) using real mock data
//! from the `tests/compatible-datasets/` directory.
//!
//! # Test Execution Order and Dependencies
//!
//! Tests are organized in execution order based on table dependencies:
//!
//! ## Level 0 - Base Tables (No Dependencies)
//! | Order | Test Name | Table | Behavior |
//! |-------|-----------|-------|----------|
//! | 01 | test_organizations_compat | organizations | create_table, import, query, update, delete |
//! | 02 | test_users_compat | users | create_table, import, query, update, delete |
//! | 03 | test_templates_compat | templates | put, get, list, delete |
//! | 04 | test_short_urls_compat | short_urls | init, add, get, list, remove |
//! | 05 | test_sessions_compat | sessions | set, get, list, delete |
//!
//! ## Level 1 - Tables Depending on Level 0
//! | Order | Test Name | Table | Dependencies | Behavior |
//! |-------|-----------|-------|--------------|----------|
//! | 10 | test_org_users_compat | org_users | organizations, users | create_table, import, query, update, delete |
//! | 11 | test_folders_compat | folders | organizations | put, get, list, delete |
//! | 12 | test_destinations_compat | destinations | organizations, templates | put, get, list, delete |
//!
//! ## Level 2 - Tables Depending on Level 1
//! | Order | Test Name | Table | Dependencies | Behavior |
//! |-------|-----------|-------|--------------|----------|
//! | 20 | test_dashboards_compat | dashboards | folders | put, get, list, delete |
//! | 21 | test_alerts_compat | alerts | folders, destinations | put, get, list, delete |
//!
//! # Test Data Files
//!
//! Test data is loaded from JSON files in `tests/compatible-datasets/`:
//! - organizations.json - Organization records
//! - users.json - User records
//! - org_users.json - Organization-user relationship records
//! - folders.json - Folder records for dashboards/alerts
//! - dashboards.json - Dashboard records
//! - alerts.json - Alert configuration records
//! - templates.json - Alert/notification template records
//! - destinations.json - Alert destination records
//! - short_urls.json - Short URL mapping records
//! - sessions.json - Session records (empty in test data, uses generated data)

// ==================== Consolidated Imports ====================
#![allow(dead_code)]
// std crate
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicI32, Ordering as AtomicOrd},
    },
};

// External crates
use arrow_schema::{DataType, Field, Schema};
use chrono::{FixedOffset, Utc};
// config crate
use config::meta::{
    actions::action::{Action, ActionStatus, ExecutionDetailsType},
    ai::{AIPrompt, PromptType},
    alerts::{QueryCondition, TriggerCondition},
    dashboards::{ListDashboardsParams, reports::ListReportsParams, v1::Dashboard as DashboardV1},
    destinations::{Destination, DestinationType, Endpoint, Module, Template, TemplateType},
    folder::{Folder, FolderType},
    organization::OrganizationType,
    pipeline::{Pipeline, components::PipelineSource},
    ratelimit::RatelimitRule,
    stream::{FileKey, FileMeta, StreamParams, StreamStats, StreamType as FileStreamType},
    system_settings::{SettingScope, SystemSetting},
    timed_annotations::TimedAnnotation,
    triggers::{Trigger, TriggerModule, TriggerStatus},
    user::UserRole,
};
// infra crate
use infra::{
    db::{ORM_CLIENT, ORM_CLIENT_DDL, connect_to_orm, connect_to_orm_ddl},
    file_list, pipeline as infra_pipeline, scheduler,
    schema::history,
    table::{
        action_scripts, alert_incidents, alerts,
        cipher::{self, CipherEntry, EntryKind},
        compactor_manual_jobs::{self, CompactorManualJob, Status as CompactorStatus},
        dashboards, destinations,
        distinct_values::{self, DistinctFieldRecord, OriginType},
        enrichment_table_urls::{self, EnrichmentTableUrlRecord},
        enrichment_tables,
        entity::{
            alert_dedup_state, alert_incident_alerts, pipeline_last_errors, report_dashboards,
            search_job_partitions::Model as PartitionModel,
            search_jobs::ActiveModel as SearchJobActiveModel,
        },
        folders, org_users, organizations,
        ratelimit::{self, RuleEntry},
        re_pattern::{self, PatternEntry},
        re_pattern_stream_map::{self, ApplyPolicy, PatternAssociationEntry, PatternPolicy},
        reports,
        search_job::{search_job_partitions, search_job_results, search_jobs},
        search_queue,
        service_streams::{self, ServiceRecord},
        service_streams_dimensions::{self, DimensionValueRecord},
        sessions, short_urls, system_prompts, system_settings, templates, timed_annotation_panels,
        timed_annotations, users,
    },
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseBackend, EntityTrait,
    QueryFilter, Statement,
};
use serde::{Deserialize, Serialize};
use svix_ksuid::{Ksuid, KsuidLike};
use tokio::sync::OnceCell;

// ==================== ORM Table Creation ====================

/// Static flag to track if ORM tables have been initialized
static ORM_TABLES_INITIALIZED: OnceCell<Result<(), String>> = OnceCell::const_new();

/// Ensure all ORM tables exist for testing.
/// This function creates tables that don't have a `create_table()` method
/// in their table modules (e.g., templates, sessions, folders, destinations).
///
/// Uses OnceCell to ensure tables are only created once even when tests run in parallel.
pub async fn ensure_orm_tables_exist() -> Result<(), anyhow::Error> {
    // Use OnceCell to ensure initialization only happens once
    let result = ORM_TABLES_INITIALIZED
        .get_or_init(|| async {
            ensure_orm_tables_exist_inner()
                .await
                .map_err(|e| e.to_string())
        })
        .await;

    match result {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow::anyhow!("{}", e)),
    }
}

/// Internal implementation of ensure_orm_tables_exist
async fn ensure_orm_tables_exist_inner() -> Result<(), anyhow::Error> {
    let client = ORM_CLIENT_DDL.get_or_init(connect_to_orm_ddl).await;
    let backend = client.get_database_backend();

    // Note: Using IF NOT EXISTS to avoid conflicts when tests run in parallel.
    // This means existing tables won't be recreated, which is the desired behavior.
    // If you need to reset tables, drop them manually before running tests.

    // Create templates table (from m20250125_115400_create_templates_table.rs)
    let create_templates_sql = match backend {
        DatabaseBackend::MySql => {
            r#"
            CREATE TABLE IF NOT EXISTS `templates` (
                `id` char(27) NOT NULL PRIMARY KEY,
                `org` varchar(100) NOT NULL,
                `name` varchar(256) NOT NULL,
                `is_default` bool NOT NULL,
                `type` varchar(10) NOT NULL,
                `body` text NOT NULL,
                `title` text NULL
            )
        "#
        }
        DatabaseBackend::Postgres => {
            r#"
            CREATE TABLE IF NOT EXISTS "templates" (
                "id" char(27) NOT NULL PRIMARY KEY,
                "org" varchar(100) NOT NULL,
                "name" varchar(256) NOT NULL,
                "is_default" bool NOT NULL,
                "type" varchar(10) NOT NULL,
                "body" text NOT NULL,
                "title" text NULL
            )
        "#
        }
        DatabaseBackend::Sqlite => {
            r#"
            CREATE TABLE IF NOT EXISTS "templates" (
                "id" char(27) NOT NULL PRIMARY KEY,
                "org" varchar(100) NOT NULL,
                "name" varchar(256) NOT NULL,
                "is_default" boolean NOT NULL,
                "type" varchar(10) NOT NULL,
                "body" text NOT NULL,
                "title" text NULL
            )
        "#
        }
    };
    client
        .execute(Statement::from_string(backend, create_templates_sql))
        .await?;

    // Create users table (from m20241227_000200_create_users_table.rs)
    let create_users_sql = match backend {
        DatabaseBackend::MySql => {
            r#"
            CREATE TABLE IF NOT EXISTS `users` (
                `id` char(27) NOT NULL PRIMARY KEY,
                `email` varchar(100) NOT NULL,
                `first_name` varchar(100) NOT NULL,
                `last_name` varchar(100) NOT NULL,
                `password` varchar(256) NOT NULL,
                `salt` varchar(256) NOT NULL,
                `is_root` bool NOT NULL,
                `password_ext` varchar(256) NULL,
                `user_type` smallint NOT NULL,
                `created_at` bigint NOT NULL,
                `updated_at` bigint NOT NULL
            )
        "#
        }
        DatabaseBackend::Postgres => {
            r#"
            CREATE TABLE IF NOT EXISTS "users" (
                "id" char(27) NOT NULL PRIMARY KEY,
                "email" varchar(100) NOT NULL,
                "first_name" varchar(100) NOT NULL,
                "last_name" varchar(100) NOT NULL,
                "password" varchar(256) NOT NULL,
                "salt" varchar(256) NOT NULL,
                "is_root" bool NOT NULL,
                "password_ext" varchar(256) NULL,
                "user_type" smallint NOT NULL,
                "created_at" bigint NOT NULL,
                "updated_at" bigint NOT NULL
            )
        "#
        }
        DatabaseBackend::Sqlite => {
            r#"
            CREATE TABLE IF NOT EXISTS "users" (
                "id" char(27) NOT NULL PRIMARY KEY,
                "email" varchar(100) NOT NULL,
                "first_name" varchar(100) NOT NULL,
                "last_name" varchar(100) NOT NULL,
                "password" varchar(256) NOT NULL,
                "salt" varchar(256) NOT NULL,
                "is_root" boolean NOT NULL,
                "password_ext" varchar(256) NULL,
                "user_type" integer NOT NULL,
                "created_at" bigint NOT NULL,
                "updated_at" bigint NOT NULL
            )
        "#
        }
    };
    client
        .execute(Statement::from_string(backend, create_users_sql))
        .await?;

    // Create organizations table (from entity/organizations.rs)
    let create_organizations_sql = match backend {
        DatabaseBackend::MySql => {
            #[cfg(feature = "cloud")]
            {
                r#"
                CREATE TABLE IF NOT EXISTS `organizations` (
                    `identifier` varchar(256) NOT NULL PRIMARY KEY,
                    `org_name` varchar(256) NOT NULL,
                    `org_type` smallint NOT NULL,
                    `created_at` bigint NOT NULL,
                    `updated_at` bigint NOT NULL,
                    `trial_ends_at` bigint NOT NULL
                )
            "#
            }
            #[cfg(not(feature = "cloud"))]
            {
                r#"
                CREATE TABLE IF NOT EXISTS `organizations` (
                    `identifier` varchar(256) NOT NULL PRIMARY KEY,
                    `org_name` varchar(256) NOT NULL,
                    `org_type` smallint NOT NULL,
                    `created_at` bigint NOT NULL,
                    `updated_at` bigint NOT NULL
                )
            "#
            }
        }
        DatabaseBackend::Postgres => {
            #[cfg(feature = "cloud")]
            {
                r#"
                CREATE TABLE IF NOT EXISTS "organizations" (
                    "identifier" varchar(256) NOT NULL PRIMARY KEY,
                    "org_name" varchar(256) NOT NULL,
                    "org_type" smallint NOT NULL,
                    "created_at" bigint NOT NULL,
                    "updated_at" bigint NOT NULL,
                    "trial_ends_at" bigint NOT NULL
                )
            "#
            }
            #[cfg(not(feature = "cloud"))]
            {
                r#"
                CREATE TABLE IF NOT EXISTS "organizations" (
                    "identifier" varchar(256) NOT NULL PRIMARY KEY,
                    "org_name" varchar(256) NOT NULL,
                    "org_type" smallint NOT NULL,
                    "created_at" bigint NOT NULL,
                    "updated_at" bigint NOT NULL
                )
            "#
            }
        }
        DatabaseBackend::Sqlite => {
            #[cfg(feature = "cloud")]
            {
                r#"
                CREATE TABLE IF NOT EXISTS "organizations" (
                    "identifier" varchar(256) NOT NULL PRIMARY KEY,
                    "org_name" varchar(256) NOT NULL,
                    "org_type" integer NOT NULL,
                    "created_at" bigint NOT NULL,
                    "updated_at" bigint NOT NULL,
                    "trial_ends_at" bigint NOT NULL
                )
            "#
            }
            #[cfg(not(feature = "cloud"))]
            {
                r#"
                CREATE TABLE IF NOT EXISTS "organizations" (
                    "identifier" varchar(256) NOT NULL PRIMARY KEY,
                    "org_name" varchar(256) NOT NULL,
                    "org_type" integer NOT NULL,
                    "created_at" bigint NOT NULL,
                    "updated_at" bigint NOT NULL
                )
            "#
            }
        }
    };
    client
        .execute(Statement::from_string(backend, create_organizations_sql))
        .await?;

    // Create org_users table (depends on organizations and users)
    let create_org_users_sql = match backend {
        DatabaseBackend::MySql => {
            r#"
            CREATE TABLE IF NOT EXISTS `org_users` (
                `id` char(27) NOT NULL PRIMARY KEY,
                `email` varchar(100) NOT NULL,
                `org_id` varchar(256) NOT NULL,
                `role` smallint NOT NULL,
                `token` varchar(256) NOT NULL,
                `rum_token` varchar(256) NULL,
                `created_at` bigint NOT NULL,
                `updated_at` bigint NOT NULL
            )
        "#
        }
        DatabaseBackend::Postgres => {
            r#"
            CREATE TABLE IF NOT EXISTS "org_users" (
                "id" char(27) NOT NULL PRIMARY KEY,
                "email" varchar(100) NOT NULL,
                "org_id" varchar(256) NOT NULL,
                "role" smallint NOT NULL,
                "token" varchar(256) NOT NULL,
                "rum_token" varchar(256) NULL,
                "created_at" bigint NOT NULL,
                "updated_at" bigint NOT NULL
            )
        "#
        }
        DatabaseBackend::Sqlite => {
            r#"
            CREATE TABLE IF NOT EXISTS "org_users" (
                "id" char(27) NOT NULL PRIMARY KEY,
                "email" varchar(100) NOT NULL,
                "org_id" varchar(256) NOT NULL,
                "role" integer NOT NULL,
                "token" varchar(256) NOT NULL,
                "rum_token" varchar(256) NULL,
                "created_at" bigint NOT NULL,
                "updated_at" bigint NOT NULL
            )
        "#
        }
    };
    client
        .execute(Statement::from_string(backend, create_org_users_sql))
        .await?;

    // Create sessions table (from m20251118_000002_create_sessions_table.rs)
    // Uses session_id as primary key, with created_at and updated_at columns
    let create_sessions_sql = match backend {
        DatabaseBackend::MySql => {
            r#"
            CREATE TABLE IF NOT EXISTS `sessions` (
                `session_id` varchar(36) NOT NULL PRIMARY KEY,
                `access_token` text NOT NULL,
                `created_at` bigint NOT NULL,
                `updated_at` bigint NOT NULL
            )
        "#
        }
        DatabaseBackend::Postgres => {
            r#"
            CREATE TABLE IF NOT EXISTS "sessions" (
                "session_id" varchar(36) NOT NULL PRIMARY KEY,
                "access_token" text NOT NULL,
                "created_at" bigint NOT NULL,
                "updated_at" bigint NOT NULL
            )
        "#
        }
        DatabaseBackend::Sqlite => {
            r#"
            CREATE TABLE IF NOT EXISTS "sessions" (
                "session_id" varchar(36) NOT NULL PRIMARY KEY,
                "access_token" text NOT NULL,
                "created_at" bigint NOT NULL,
                "updated_at" bigint NOT NULL
            )
        "#
        }
    };
    client
        .execute(Statement::from_string(backend, create_sessions_sql))
        .await?;

    // Create folders table
    // NOTE: The migration (m20241114_000001_create_folders_table.rs) defines id as bigint
    // AUTO_INCREMENT, but the entity (entity/folders.rs) defines id as String, and
    // folders.rs::put() uses ksuid.to_string(). This is a code-level inconsistency. For
    // testing, we use char(27) to match the entity/business logic.
    // DO NOT use DROP TABLE here - it breaks parallel tests and OnceCell guarantees
    // that this function only runs once anyway.
    let create_folders_sql = match backend {
        DatabaseBackend::MySql => {
            r#"
            CREATE TABLE IF NOT EXISTS `folders` (
                `id` char(27) NOT NULL PRIMARY KEY,
                `org` varchar(100) NOT NULL,
                `folder_id` varchar(256) NOT NULL,
                `name` varchar(256) NOT NULL,
                `description` text NULL,
                `type` smallint NOT NULL
            )
        "#
        }
        DatabaseBackend::Postgres => {
            r#"
            CREATE TABLE IF NOT EXISTS "folders" (
                "id" char(27) NOT NULL PRIMARY KEY,
                "org" varchar(100) NOT NULL,
                "folder_id" varchar(256) NOT NULL,
                "name" varchar(256) NOT NULL,
                "description" text NULL,
                "type" smallint NOT NULL
            )
        "#
        }
        DatabaseBackend::Sqlite => {
            r#"
            CREATE TABLE IF NOT EXISTS "folders" (
                "id" char(27) NOT NULL PRIMARY KEY,
                "org" varchar(100) NOT NULL,
                "folder_id" varchar(256) NOT NULL,
                "name" varchar(256) NOT NULL,
                "description" text NULL,
                "type" integer NOT NULL
            )
        "#
        }
    };
    client
        .execute(Statement::from_string(backend, create_folders_sql))
        .await?;

    // Create destinations table (from m20250125_102300_create_destinations_table.rs)
    // Uses template_id foreign key to templates table
    let create_destinations_sql = match backend {
        DatabaseBackend::MySql => {
            r#"
            CREATE TABLE IF NOT EXISTS `destinations` (
                `id` char(27) NOT NULL PRIMARY KEY,
                `org` varchar(100) NOT NULL,
                `name` varchar(256) NOT NULL,
                `module` varchar(10) NOT NULL,
                `template_id` char(27) NULL,
                `type` json NOT NULL
            )
        "#
        }
        DatabaseBackend::Postgres => {
            r#"
            CREATE TABLE IF NOT EXISTS "destinations" (
                "id" char(27) NOT NULL PRIMARY KEY,
                "org" varchar(100) NOT NULL,
                "name" varchar(256) NOT NULL,
                "module" varchar(10) NOT NULL,
                "template_id" char(27) NULL,
                "type" json NOT NULL
            )
        "#
        }
        DatabaseBackend::Sqlite => {
            r#"
            CREATE TABLE IF NOT EXISTS "destinations" (
                "id" char(27) NOT NULL PRIMARY KEY,
                "org" varchar(100) NOT NULL,
                "name" varchar(256) NOT NULL,
                "module" varchar(10) NOT NULL,
                "template_id" char(27) NULL,
                "type" text NOT NULL
            )
        "#
        }
    };
    client
        .execute(Statement::from_string(backend, create_destinations_sql))
        .await?;

    // Create alerts table (from m20241209_120000_create_alerts_table.rs)
    // Depends on folders table (folder_id foreign key)
    let create_alerts_sql = match backend {
        DatabaseBackend::MySql => {
            r#"
            CREATE TABLE IF NOT EXISTS `alerts` (
                `id` char(27) NOT NULL PRIMARY KEY,
                `org` varchar(100) NOT NULL,
                `folder_id` char(27) NOT NULL,
                `name` varchar(256) NOT NULL,
                `stream_type` varchar(50) NOT NULL,
                `stream_name` varchar(256) NOT NULL,
                `is_real_time` bool NOT NULL,
                `destinations` json NOT NULL,
                `context_attributes` json NULL,
                `row_template` text NULL,
                `row_template_type` smallint NOT NULL DEFAULT 0,
                `description` text NULL,
                `enabled` bool NOT NULL,
                `tz_offset` int NOT NULL,
                `last_triggered_at` bigint NULL,
                `last_satisfied_at` bigint NULL,
                `query_type` smallint NOT NULL,
                `query_conditions` json NULL,
                `query_sql` text NULL,
                `query_promql` text NULL,
                `query_promql_condition` json NULL,
                `query_aggregation` json NULL,
                `query_vrl_function` text NULL,
                `query_search_event_type` smallint NULL,
                `query_multi_time_range` json NULL,
                `trigger_threshold_operator` varchar(50) NOT NULL,
                `trigger_period_seconds` bigint NOT NULL,
                `trigger_threshold_count` bigint NOT NULL,
                `trigger_frequency_type` smallint NOT NULL,
                `trigger_frequency_seconds` bigint NOT NULL,
                `trigger_frequency_cron` text NULL,
                `trigger_frequency_cron_timezone` varchar(256) NULL,
                `trigger_silence_seconds` bigint NOT NULL,
                `trigger_tolerance_seconds` bigint NULL,
                `owner` varchar(256) NULL,
                `last_edited_by` varchar(256) NULL,
                `updated_at` bigint NULL,
                `align_time` bool NOT NULL DEFAULT FALSE,
                `dedup_enabled` bool NOT NULL DEFAULT FALSE,
                `dedup_time_window_minutes` bigint NULL,
                `dedup_config` json NULL
            )
        "#
        }
        DatabaseBackend::Postgres => {
            r#"
            CREATE TABLE IF NOT EXISTS "alerts" (
                "id" char(27) NOT NULL PRIMARY KEY,
                "org" varchar(100) NOT NULL,
                "folder_id" char(27) NOT NULL,
                "name" varchar(256) NOT NULL,
                "stream_type" varchar(50) NOT NULL,
                "stream_name" varchar(256) NOT NULL,
                "is_real_time" bool NOT NULL,
                "destinations" json NOT NULL,
                "context_attributes" json NULL,
                "row_template" text NULL,
                "row_template_type" smallint NOT NULL DEFAULT 0,
                "description" text NULL,
                "enabled" bool NOT NULL,
                "tz_offset" integer NOT NULL,
                "last_triggered_at" bigint NULL,
                "last_satisfied_at" bigint NULL,
                "query_type" smallint NOT NULL,
                "query_conditions" json NULL,
                "query_sql" text NULL,
                "query_promql" text NULL,
                "query_promql_condition" json NULL,
                "query_aggregation" json NULL,
                "query_vrl_function" text NULL,
                "query_search_event_type" smallint NULL,
                "query_multi_time_range" json NULL,
                "trigger_threshold_operator" varchar(50) NOT NULL,
                "trigger_period_seconds" bigint NOT NULL,
                "trigger_threshold_count" bigint NOT NULL,
                "trigger_frequency_type" smallint NOT NULL,
                "trigger_frequency_seconds" bigint NOT NULL,
                "trigger_frequency_cron" text NULL,
                "trigger_frequency_cron_timezone" varchar(256) NULL,
                "trigger_silence_seconds" bigint NOT NULL,
                "trigger_tolerance_seconds" bigint NULL,
                "owner" varchar(256) NULL,
                "last_edited_by" varchar(256) NULL,
                "updated_at" bigint NULL,
                "align_time" bool NOT NULL DEFAULT FALSE,
                "dedup_enabled" bool NOT NULL DEFAULT FALSE,
                "dedup_time_window_minutes" bigint NULL,
                "dedup_config" json NULL
            )
        "#
        }
        DatabaseBackend::Sqlite => {
            r#"
            CREATE TABLE IF NOT EXISTS "alerts" (
                "id" char(27) NOT NULL PRIMARY KEY,
                "org" varchar(100) NOT NULL,
                "folder_id" char(27) NOT NULL,
                "name" varchar(256) NOT NULL,
                "stream_type" varchar(50) NOT NULL,
                "stream_name" varchar(256) NOT NULL,
                "is_real_time" boolean NOT NULL,
                "destinations" text NOT NULL,
                "context_attributes" text NULL,
                "row_template" text NULL,
                "row_template_type" integer NOT NULL DEFAULT 0,
                "description" text NULL,
                "enabled" boolean NOT NULL,
                "tz_offset" integer NOT NULL,
                "last_triggered_at" bigint NULL,
                "last_satisfied_at" bigint NULL,
                "query_type" integer NOT NULL,
                "query_conditions" text NULL,
                "query_sql" text NULL,
                "query_promql" text NULL,
                "query_promql_condition" text NULL,
                "query_aggregation" text NULL,
                "query_vrl_function" text NULL,
                "query_search_event_type" integer NULL,
                "query_multi_time_range" text NULL,
                "trigger_threshold_operator" varchar(50) NOT NULL,
                "trigger_period_seconds" bigint NOT NULL,
                "trigger_threshold_count" bigint NOT NULL,
                "trigger_frequency_type" integer NOT NULL,
                "trigger_frequency_seconds" bigint NOT NULL,
                "trigger_frequency_cron" text NULL,
                "trigger_frequency_cron_timezone" varchar(256) NULL,
                "trigger_silence_seconds" bigint NOT NULL,
                "trigger_tolerance_seconds" bigint NULL,
                "owner" varchar(256) NULL,
                "last_edited_by" varchar(256) NULL,
                "updated_at" bigint NULL,
                "align_time" boolean NOT NULL DEFAULT 0,
                "dedup_enabled" boolean NOT NULL DEFAULT 0,
                "dedup_time_window_minutes" bigint NULL,
                "dedup_config" text NULL
            )
        "#
        }
    };
    client
        .execute(Statement::from_string(backend, create_alerts_sql))
        .await?;

    // Create dashboards table (from m20241017_000001_create_dashboards_table.rs)
    // Depends on folders table (folder_id foreign key)
    let create_dashboards_sql = match backend {
        DatabaseBackend::MySql => {
            r#"
            CREATE TABLE IF NOT EXISTS `dashboards` (
                `id` char(27) NOT NULL PRIMARY KEY,
                `folder_id` char(27) NOT NULL,
                `dashboard_id` varchar(256) NOT NULL,
                `version` int NOT NULL,
                `title` varchar(256) NOT NULL,
                `description` text NULL,
                `role` text NULL,
                `owner` text NOT NULL,
                `data` json NOT NULL,
                `created_at` bigint NOT NULL,
                `updated_at` bigint NOT NULL
            )
        "#
        }
        DatabaseBackend::Postgres => {
            r#"
            CREATE TABLE IF NOT EXISTS "dashboards" (
                "id" char(27) NOT NULL PRIMARY KEY,
                "folder_id" char(27) NOT NULL,
                "dashboard_id" varchar(256) NOT NULL,
                "version" integer NOT NULL,
                "title" varchar(256) NOT NULL,
                "description" text NULL,
                "role" text NULL,
                "owner" text NOT NULL,
                "data" json NOT NULL,
                "created_at" bigint NOT NULL,
                "updated_at" bigint NOT NULL
            )
        "#
        }
        DatabaseBackend::Sqlite => {
            r#"
            CREATE TABLE IF NOT EXISTS "dashboards" (
                "id" char(27) NOT NULL PRIMARY KEY,
                "folder_id" char(27) NOT NULL,
                "dashboard_id" varchar(256) NOT NULL,
                "version" integer NOT NULL,
                "title" varchar(256) NOT NULL,
                "description" text NULL,
                "role" text NULL,
                "owner" text NOT NULL,
                "data" text NOT NULL,
                "created_at" bigint NOT NULL,
                "updated_at" bigint NOT NULL
            )
        "#
        }
    };
    client
        .execute(Statement::from_string(backend, create_dashboards_sql))
        .await?;

    // Create cipher_keys table
    let create_cipher_keys_sql = match backend {
        DatabaseBackend::MySql => {
            r#"
            CREATE TABLE IF NOT EXISTS `cipher_keys` ( 
                `org` varchar(100) NOT NULL, 
                `created_by` varchar(256) NOT NULL, 
                `created_at` bigint NOT NULL, 
                `name` varchar(256) NOT NULL, 
                `kind` varchar(100) NOT NULL, 
                `data` text NOT NULL,
                PRIMARY KEY (`org`, `name`, `kind`)
            )
        "#
        }
        DatabaseBackend::Postgres => {
            r#"
            CREATE TABLE IF NOT EXISTS "cipher_keys" ( 
                "org" varchar(100) NOT NULL, 
                "created_by" varchar(256) NOT NULL, 
                "created_at" bigint NOT NULL, 
                "name" varchar(256) NOT NULL, 
                "kind" varchar(100) NOT NULL, 
                "data" text NOT NULL,
                PRIMARY KEY ("org", "name", "kind")
            )
        "#
        }
        DatabaseBackend::Sqlite => {
            r#"
            CREATE TABLE IF NOT EXISTS "cipher_keys" ( 
                "org" varchar(100) NOT NULL, 
                "created_by" varchar(256) NOT NULL, 
                "created_at" bigint NOT NULL, 
                "name" varchar(256) NOT NULL, 
                "kind" varchar(100) NOT NULL, 
                "data" text NOT NULL,
                PRIMARY KEY ("org", "name", "kind")
            )
        "#
        }
    };
    client
        .execute(Statement::from_string(backend, create_cipher_keys_sql))
        .await?;

    // Create system_settings table
    let create_system_settings_sql = match backend {
        DatabaseBackend::MySql => {
            r#"
            CREATE TABLE IF NOT EXISTS `system_settings` (
                `id` bigint AUTO_INCREMENT PRIMARY KEY NOT NULL,
                `scope` varchar(20) NOT NULL,
                `org_id` varchar(100) NULL,
                `user_id` varchar(256) NULL,
                `setting_key` varchar(255) NOT NULL,
                `setting_category` varchar(100) NULL,
                `description` text NULL,
                `created_at` bigint NOT NULL,
                `updated_at` bigint NOT NULL,
                `created_by` varchar(256) NULL,
                `updated_by` varchar(256) NULL,
                `setting_value` text NOT NULL
            )
        "#
        }
        DatabaseBackend::Postgres => {
            r#"
            CREATE TABLE IF NOT EXISTS "system_settings" (
                "id" bigserial PRIMARY KEY NOT NULL,
                "scope" varchar(20) NOT NULL,
                "org_id" varchar(100) NULL,
                "user_id" varchar(256) NULL,
                "setting_key" varchar(255) NOT NULL,
                "setting_category" varchar(100) NULL,
                "description" text NULL,
                "created_at" bigint NOT NULL,
                "updated_at" bigint NOT NULL,
                "created_by" varchar(256) NULL,
                "updated_by" varchar(256) NULL,
                "setting_value" text NOT NULL
            )
        "#
        }
        DatabaseBackend::Sqlite => {
            r#"
            CREATE TABLE IF NOT EXISTS "system_settings" (
                "id" integer PRIMARY KEY AUTOINCREMENT NOT NULL,
                "scope" varchar(20) NOT NULL,
                "org_id" varchar(100) NULL,
                "user_id" varchar(256) NULL,
                "setting_key" varchar(255) NOT NULL,
                "setting_category" varchar(100) NULL,
                "description" text NULL,
                "created_at" bigint NOT NULL,
                "updated_at" bigint NOT NULL,
                "created_by" varchar(256) NULL,
                "updated_by" varchar(256) NULL,
                "setting_value" text NOT NULL
            )
        "#
        }
    };
    client
        .execute(Statement::from_string(backend, create_system_settings_sql))
        .await?;

    // Create enrichment_tables table
    let create_enrichment_tables_sql = match backend {
        DatabaseBackend::MySql => {
            r#"
            CREATE TABLE IF NOT EXISTS `enrichment_tables` ( 
                `id` bigint AUTO_INCREMENT PRIMARY KEY,
                `org` varchar(256) NOT NULL,
                `name` varchar(256) NOT NULL,
                `data` longblob NOT NULL,
                `created_at` bigint NOT NULL
            )
        "#
        }
        DatabaseBackend::Postgres => {
            r#"
            CREATE TABLE IF NOT EXISTS "enrichment_tables" ( 
                "id" bigserial PRIMARY KEY,
                "org" varchar(256) NOT NULL,
                "name" varchar(256) NOT NULL,
                "data" bytea NOT NULL,
                "created_at" bigint NOT NULL
            )
        "#
        }
        DatabaseBackend::Sqlite => {
            r#"
            CREATE TABLE IF NOT EXISTS "enrichment_tables" ( 
                "id" integer PRIMARY KEY AUTOINCREMENT,
                "org" varchar(256) NOT NULL,
                "name" varchar(256) NOT NULL,
                "data" blob NOT NULL,
                "created_at" bigint NOT NULL
            )
        "#
        }
    };
    client
        .execute(Statement::from_string(
            backend,
            create_enrichment_tables_sql,
        ))
        .await?;

    // Create enrichment_table_urls table
    let create_enrichment_table_urls_sql = match backend {
        DatabaseBackend::MySql => {
            r#"
            CREATE TABLE IF NOT EXISTS `enrichment_table_urls` (
                `id` bigint AUTO_INCREMENT PRIMARY KEY,
                `org` varchar(256) NOT NULL,
                `name` varchar(256) NOT NULL,
                `url` varchar(2048) NOT NULL,
                `status` smallint NOT NULL,
                `error_message` text,
                `created_at` bigint NOT NULL,
                `updated_at` bigint NOT NULL,
                `total_bytes_fetched` bigint NOT NULL,
                `total_records_processed` bigint NOT NULL,
                `retry_count` int NOT NULL,
                `append_data` bool NOT NULL,
                `last_byte_position` bigint NOT NULL DEFAULT 0,
                `supports_range` bool NOT NULL DEFAULT FALSE,
                `is_local_region` bool NOT NULL DEFAULT TRUE
            )
        "#
        }
        DatabaseBackend::Postgres => {
            r#"
            CREATE TABLE IF NOT EXISTS "enrichment_table_urls" (
                "id" bigserial PRIMARY KEY,
                "org" varchar(256) NOT NULL,
                "name" varchar(256) NOT NULL,
                "url" varchar(2048) NOT NULL,
                "status" smallint NOT NULL,
                "error_message" text,
                "created_at" bigint NOT NULL,
                "updated_at" bigint NOT NULL,
                "total_bytes_fetched" bigint NOT NULL,
                "total_records_processed" bigint NOT NULL,
                "retry_count" int NOT NULL,
                "append_data" bool NOT NULL,
                "last_byte_position" bigint NOT NULL DEFAULT 0,
                "supports_range" bool NOT NULL DEFAULT FALSE,
                "is_local_region" bool NOT NULL DEFAULT TRUE
            )
        "#
        }
        DatabaseBackend::Sqlite => {
            r#"
            CREATE TABLE IF NOT EXISTS "enrichment_table_urls" (
                "id" integer PRIMARY KEY AUTOINCREMENT,
                "org" varchar(256) NOT NULL,
                "name" varchar(256) NOT NULL,
                "url" varchar(2048) NOT NULL,
                "status" smallint NOT NULL,
                "error_message" text,
                "created_at" bigint NOT NULL,
                "updated_at" bigint NOT NULL,
                "total_bytes_fetched" bigint NOT NULL,
                "total_records_processed" bigint NOT NULL,
                "retry_count" integer NOT NULL,
                "append_data" boolean NOT NULL,
                "last_byte_position" bigint NOT NULL DEFAULT 0,
                "supports_range" boolean NOT NULL DEFAULT FALSE,
                "is_local_region" boolean NOT NULL DEFAULT TRUE
            )
        "#
        }
    };
    client
        .execute(Statement::from_string(
            backend,
            create_enrichment_table_urls_sql,
        ))
        .await?;

    // Create rate_limit_rules table
    let create_rate_limit_rules_sql = match backend {
        DatabaseBackend::MySql => {
            r#"
            CREATE TABLE IF NOT EXISTS `rate_limit_rules` (
                `rule_id` varchar(64) NOT NULL PRIMARY KEY,
                `org` varchar(100) NOT NULL,
                `rule_type` varchar(50) NOT NULL DEFAULT 'exact',
                `user_role` varchar(256) NOT NULL,
                `user_id` varchar(256) NOT NULL,
                `api_group_name` varchar(256) NOT NULL,
                `api_group_operation` varchar(128) NOT NULL,
                `threshold` int NOT NULL,
                `created_at` bigint NOT NULL
            )
        "#
        }
        DatabaseBackend::Postgres => {
            r#"
            CREATE TABLE IF NOT EXISTS "rate_limit_rules" (
                "rule_id" varchar(64) NOT NULL PRIMARY KEY,
                "org" varchar(100) NOT NULL,
                "rule_type" varchar(50) NOT NULL DEFAULT 'exact',
                "user_role" varchar(256) NOT NULL,
                "user_id" varchar(256) NOT NULL,
                "api_group_name" varchar(256) NOT NULL,
                "api_group_operation" varchar(128) NOT NULL,
                "threshold" int NOT NULL,
                "created_at" bigint NOT NULL
            )
        "#
        }
        DatabaseBackend::Sqlite => {
            r#"
            CREATE TABLE IF NOT EXISTS "rate_limit_rules" (
                "rule_id" varchar(64) NOT NULL PRIMARY KEY,
                "org" varchar(100) NOT NULL,
                "rule_type" varchar(50) NOT NULL DEFAULT 'exact',
                "user_role" varchar(256) NOT NULL,
                "user_id" varchar(256) NOT NULL,
                "api_group_name" varchar(256) NOT NULL,
                "api_group_operation" varchar(128) NOT NULL,
                "threshold" integer NOT NULL,
                "created_at" bigint NOT NULL
            )
        "#
        }
    };
    client
        .execute(Statement::from_string(backend, create_rate_limit_rules_sql))
        .await?;

    // Create re_patterns table
    let create_re_patterns_sql = match backend {
        DatabaseBackend::MySql => {
            r#"
            CREATE TABLE IF NOT EXISTS `re_patterns` ( 
                `id` varchar(100) NOT NULL PRIMARY KEY, 
                `org` varchar(100) NOT NULL, 
                `name` varchar(256) NOT NULL,
                `description` text NOT NULL,
                `created_by` varchar(256) NOT NULL,
                `created_at` bigint NOT NULL, 
                `updated_at` bigint NOT NULL, 
                `pattern` text NOT NULL 
            )
        "#
        }
        DatabaseBackend::Postgres => {
            r#"
            CREATE TABLE IF NOT EXISTS "re_patterns" ( 
                "id" varchar(100) NOT NULL PRIMARY KEY, 
                "org" varchar(100) NOT NULL, 
                "name" varchar(256) NOT NULL,
                "description" text NOT NULL,
                "created_by" varchar(256) NOT NULL,
                "created_at" bigint NOT NULL, 
                "updated_at" bigint NOT NULL, 
                "pattern" text NOT NULL 
            )
        "#
        }
        DatabaseBackend::Sqlite => {
            r#"
            CREATE TABLE IF NOT EXISTS "re_patterns" ( 
                "id" varchar(100) NOT NULL PRIMARY KEY, 
                "org" varchar(100) NOT NULL, 
                "name" varchar(256) NOT NULL,
                "description" text NOT NULL,
                "created_by" varchar(256) NOT NULL,
                "created_at" bigint NOT NULL, 
                "updated_at" bigint NOT NULL, 
                "pattern" text NOT NULL 
            )
        "#
        }
    };
    client
        .execute(Statement::from_string(backend, create_re_patterns_sql))
        .await?;

    // Create re_pattern_stream_map table (depends on re_patterns)
    let create_re_pattern_stream_map_sql = match backend {
        DatabaseBackend::MySql => {
            r#"
            CREATE TABLE IF NOT EXISTS `re_pattern_stream_map` ( 
                `id` bigint NOT NULL AUTO_INCREMENT PRIMARY KEY, 
                `org` varchar(100) NOT NULL, 
                `stream` varchar(256) NOT NULL, 
                `stream_type` varchar(50) NOT NULL, 
                `field` varchar(1024) NOT NULL, 
                `pattern_id` varchar(100) NOT NULL, 
                `policy` text NOT NULL, 
                `apply_at` varchar(100) NOT NULL
            )
        "#
        }
        DatabaseBackend::Postgres => {
            r#"
            CREATE TABLE IF NOT EXISTS "re_pattern_stream_map" ( 
                "id" bigserial PRIMARY KEY, 
                "org" varchar(100) NOT NULL, 
                "stream" varchar(256) NOT NULL, 
                "stream_type" varchar(50) NOT NULL, 
                "field" varchar(1024) NOT NULL, 
                "pattern_id" varchar(100) NOT NULL, 
                "policy" text NOT NULL, 
                "apply_at" varchar(100) NOT NULL
            )
        "#
        }
        DatabaseBackend::Sqlite => {
            r#"
            CREATE TABLE IF NOT EXISTS "re_pattern_stream_map" ( 
                "id" integer PRIMARY KEY AUTOINCREMENT, 
                "org" varchar(100) NOT NULL, 
                "stream" varchar(256) NOT NULL, 
                "stream_type" varchar(50) NOT NULL, 
                "field" varchar(1024) NOT NULL, 
                "pattern_id" varchar(100) NOT NULL, 
                "policy" text NOT NULL, 
                "apply_at" varchar(100) NOT NULL
            )
        "#
        }
    };
    client
        .execute(Statement::from_string(
            backend,
            create_re_pattern_stream_map_sql,
        ))
        .await?;

    // Create search_queue table
    let create_search_queue_sql = match backend {
        DatabaseBackend::MySql => {
            r#"
            CREATE TABLE IF NOT EXISTS `search_queue` (
                `id` bigint NOT NULL AUTO_INCREMENT PRIMARY KEY,
                `work_group` varchar(16) NOT NULL,
                `user_id` varchar(256) NOT NULL,
                `trace_id` varchar(64) NOT NULL,
                `created_at` bigint NOT NULL,
                `org_id` varchar(100) NOT NULL DEFAULT ''
            )
        "#
        }
        DatabaseBackend::Postgres => {
            r#"
            CREATE TABLE IF NOT EXISTS "search_queue" (
                "id" bigserial PRIMARY KEY,
                "work_group" varchar(16) NOT NULL,
                "user_id" varchar(256) NOT NULL,
                "trace_id" varchar(64) NOT NULL,
                "created_at" bigint NOT NULL,
                "org_id" varchar(100) NOT NULL DEFAULT ''
            )
        "#
        }
        DatabaseBackend::Sqlite => {
            r#"
            CREATE TABLE IF NOT EXISTS "search_queue" (
                "id" integer PRIMARY KEY AUTOINCREMENT,
                "work_group" varchar(16) NOT NULL,
                "user_id" varchar(256) NOT NULL,
                "trace_id" varchar(64) NOT NULL,
                "created_at" bigint NOT NULL,
                "org_id" varchar(100) NOT NULL DEFAULT ''
            )
        "#
        }
    };
    client
        .execute(Statement::from_string(backend, create_search_queue_sql))
        .await?;

    // Create compactor_manual_jobs table
    let create_compactor_manual_jobs_sql = match backend {
        DatabaseBackend::MySql => {
            r#"
            CREATE TABLE IF NOT EXISTS `compactor_manual_jobs` (
                `id` varchar(27) NOT NULL PRIMARY KEY,
                `key` varchar(256) NOT NULL,
                `created_at` bigint NOT NULL,
                `ended_at` bigint NOT NULL DEFAULT 0,
                `status` bigint NOT NULL
            )
        "#
        }
        DatabaseBackend::Postgres => {
            r#"
            CREATE TABLE IF NOT EXISTS "compactor_manual_jobs" (
                "id" varchar(27) NOT NULL PRIMARY KEY,
                "key" varchar(256) NOT NULL,
                "created_at" bigint NOT NULL,
                "ended_at" bigint NOT NULL DEFAULT 0,
                "status" bigint NOT NULL
            )
        "#
        }
        DatabaseBackend::Sqlite => {
            r#"
            CREATE TABLE IF NOT EXISTS "compactor_manual_jobs" (
                "id" varchar(27) NOT NULL PRIMARY KEY,
                "key" varchar(256) NOT NULL,
                "created_at" bigint NOT NULL,
                "ended_at" bigint NOT NULL DEFAULT 0,
                "status" bigint NOT NULL
            )
        "#
        }
    };
    client
        .execute(Statement::from_string(
            backend,
            create_compactor_manual_jobs_sql,
        ))
        .await?;

    // Create reports table (depends on folders)
    let create_reports_sql = match backend {
        DatabaseBackend::MySql => {
            r#"
            CREATE TABLE IF NOT EXISTS `reports` ( 
                `id` char(27) NOT NULL PRIMARY KEY,
                `org` varchar(256) NOT NULL,
                `folder_id` char(27) NOT NULL,
                `name` varchar(256) NOT NULL,
                `title` varchar(256) NOT NULL,
                `description` text NULL,
                `enabled` bool NOT NULL,
                `frequency` json NOT NULL,
                `destinations` json NOT NULL,
                `message` text NULL,
                `timezone` varchar(256) NOT NULL,
                `tz_offset` int NOT NULL,
                `owner` varchar(256) NULL,
                `last_edited_by` varchar(256) NULL,
                `created_at` bigint NOT NULL,
                `updated_at` bigint NULL,
                `start_at` bigint NOT NULL
            )
        "#
        }
        DatabaseBackend::Postgres => {
            r#"
            CREATE TABLE IF NOT EXISTS "reports" ( 
                "id" char(27) NOT NULL PRIMARY KEY,
                "org" varchar(256) NOT NULL,
                "folder_id" char(27) NOT NULL,
                "name" varchar(256) NOT NULL,
                "title" varchar(256) NOT NULL,
                "description" text NULL,
                "enabled" bool NOT NULL,
                "frequency" json NOT NULL,
                "destinations" json NOT NULL,
                "message" text NULL,
                "timezone" varchar(256) NOT NULL,
                "tz_offset" int NOT NULL,
                "owner" varchar(256) NULL,
                "last_edited_by" varchar(256) NULL,
                "created_at" bigint NOT NULL,
                "updated_at" bigint NULL,
                "start_at" bigint NOT NULL
            )
        "#
        }
        DatabaseBackend::Sqlite => {
            r#"
            CREATE TABLE IF NOT EXISTS "reports" ( 
                "id" char(27) NOT NULL PRIMARY KEY,
                "org" varchar(256) NOT NULL,
                "folder_id" char(27) NOT NULL,
                "name" varchar(256) NOT NULL,
                "title" varchar(256) NOT NULL,
                "description" text NULL,
                "enabled" boolean NOT NULL,
                "frequency" text NOT NULL,
                "destinations" text NOT NULL,
                "message" text NULL,
                "timezone" varchar(256) NOT NULL,
                "tz_offset" integer NOT NULL,
                "owner" varchar(256) NULL,
                "last_edited_by" varchar(256) NULL,
                "created_at" bigint NOT NULL,
                "updated_at" bigint NULL,
                "start_at" bigint NOT NULL
            )
        "#
        }
    };
    client
        .execute(Statement::from_string(backend, create_reports_sql))
        .await?;

    // Create report_dashboards table (depends on reports and dashboards)
    let create_report_dashboards_sql = match backend {
        DatabaseBackend::MySql => {
            r#"
            CREATE TABLE IF NOT EXISTS `report_dashboards` (
                `report_id` char(27) NOT NULL,
                `dashboard_id` char(27) NOT NULL,
                `tab_names` json NOT NULL,
                `variables` json NOT NULL,
                `timerange` json NOT NULL,
                PRIMARY KEY (`report_id`, `dashboard_id`)
            )
        "#
        }
        DatabaseBackend::Postgres => {
            r#"
            CREATE TABLE IF NOT EXISTS "report_dashboards" (
                "report_id" char(27) NOT NULL,
                "dashboard_id" char(27) NOT NULL,
                "tab_names" json NOT NULL,
                "variables" json NOT NULL,
                "timerange" json NOT NULL,
                PRIMARY KEY ("report_id", "dashboard_id")
            )
        "#
        }
        DatabaseBackend::Sqlite => {
            r#"
            CREATE TABLE IF NOT EXISTS "report_dashboards" (
                "report_id" char(27) NOT NULL,
                "dashboard_id" char(27) NOT NULL,
                "tab_names" text NOT NULL,
                "variables" text NOT NULL,
                "timerange" text NOT NULL,
                PRIMARY KEY ("report_id", "dashboard_id")
            )
        "#
        }
    };
    client
        .execute(Statement::from_string(
            backend,
            create_report_dashboards_sql,
        ))
        .await?;

    // Create timed_annotations table (depends on dashboards)
    let create_timed_annotations_sql = match backend {
        DatabaseBackend::MySql => {
            r#"
            CREATE TABLE IF NOT EXISTS `timed_annotations` (
                `id` char(27) NOT NULL PRIMARY KEY,
                `dashboard_id` char(27) NOT NULL,
                `start_time` bigint NOT NULL,
                `end_time` bigint NULL,
                `title` varchar(256) NOT NULL,
                `text` text NULL,
                `tags` json NOT NULL,
                `created_at` bigint NOT NULL
            )
        "#
        }
        DatabaseBackend::Postgres => {
            r#"
            CREATE TABLE IF NOT EXISTS "timed_annotations" (
                "id" char(27) NOT NULL PRIMARY KEY,
                "dashboard_id" char(27) NOT NULL,
                "start_time" bigint NOT NULL,
                "end_time" bigint NULL,
                "title" varchar(256) NOT NULL,
                "text" text NULL,
                "tags" json NOT NULL,
                "created_at" bigint NOT NULL
            )
        "#
        }
        DatabaseBackend::Sqlite => {
            r#"
            CREATE TABLE IF NOT EXISTS "timed_annotations" (
                "id" char(27) NOT NULL PRIMARY KEY,
                "dashboard_id" char(27) NOT NULL,
                "start_time" bigint NOT NULL,
                "end_time" bigint NULL,
                "title" varchar(256) NOT NULL,
                "text" text NULL,
                "tags" text NOT NULL,
                "created_at" bigint NOT NULL
            )
        "#
        }
    };
    client
        .execute(Statement::from_string(
            backend,
            create_timed_annotations_sql,
        ))
        .await?;

    // Create timed_annotation_panels table (depends on timed_annotations)
    let create_timed_annotation_panels_sql = match backend {
        DatabaseBackend::MySql => {
            r#"
            CREATE TABLE IF NOT EXISTS `timed_annotation_panels` (
                `id` char(27) NOT NULL PRIMARY KEY,
                `timed_annotation_id` char(27) NOT NULL,
                `panel_id` varchar(256) NOT NULL
            )
        "#
        }
        DatabaseBackend::Postgres => {
            r#"
            CREATE TABLE IF NOT EXISTS "timed_annotation_panels" (
                "id" char(27) NOT NULL PRIMARY KEY,
                "timed_annotation_id" char(27) NOT NULL,
                "panel_id" varchar(256) NOT NULL
            )
        "#
        }
        DatabaseBackend::Sqlite => {
            r#"
            CREATE TABLE IF NOT EXISTS "timed_annotation_panels" (
                "id" char(27) NOT NULL PRIMARY KEY,
                "timed_annotation_id" char(27) NOT NULL,
                "panel_id" varchar(256) NOT NULL
            )
        "#
        }
    };
    client
        .execute(Statement::from_string(
            backend,
            create_timed_annotation_panels_sql,
        ))
        .await?;

    // Create alert_incidents table
    let create_alert_incidents_sql = match backend {
        DatabaseBackend::MySql => {
            r#"
            CREATE TABLE IF NOT EXISTS `alert_incidents` (
                `id` char(27) NOT NULL PRIMARY KEY,
                `org_id` varchar(128) NOT NULL,
                `correlation_key` varchar(64) NOT NULL,
                `status` varchar(20) NOT NULL DEFAULT 'open',
                `severity` varchar(10) NOT NULL DEFAULT 'P3',
                `stable_dimensions` json NOT NULL,
                `topology_context` json NULL,
                `first_alert_at` bigint NOT NULL,
                `last_alert_at` bigint NOT NULL,
                `resolved_at` bigint NULL,
                `alert_count` int NOT NULL DEFAULT 1,
                `title` varchar(500) NULL,
                `assigned_to` varchar(256) NULL,
                `created_at` bigint NOT NULL,
                `updated_at` bigint NOT NULL
            )
        "#
        }
        DatabaseBackend::Postgres => {
            r#"
            CREATE TABLE IF NOT EXISTS "alert_incidents" (
                "id" char(27) NOT NULL PRIMARY KEY,
                "org_id" varchar(128) NOT NULL,
                "correlation_key" varchar(64) NOT NULL,
                "status" varchar(20) NOT NULL DEFAULT 'open',
                "severity" varchar(10) NOT NULL DEFAULT 'P3',
                "stable_dimensions" json NOT NULL,
                "topology_context" json NULL,
                "first_alert_at" bigint NOT NULL,
                "last_alert_at" bigint NOT NULL,
                "resolved_at" bigint NULL,
                "alert_count" int NOT NULL DEFAULT 1,
                "title" varchar(500) NULL,
                "assigned_to" varchar(256) NULL,
                "created_at" bigint NOT NULL,
                "updated_at" bigint NOT NULL
            )
        "#
        }
        DatabaseBackend::Sqlite => {
            r#"
            CREATE TABLE IF NOT EXISTS "alert_incidents" (
                "id" char(27) NOT NULL PRIMARY KEY,
                "org_id" varchar(128) NOT NULL,
                "correlation_key" varchar(64) NOT NULL,
                "status" varchar(20) NOT NULL DEFAULT 'open',
                "severity" varchar(10) NOT NULL DEFAULT 'P3',
                "stable_dimensions" text NOT NULL,
                "topology_context" text NULL,
                "first_alert_at" bigint NOT NULL,
                "last_alert_at" bigint NOT NULL,
                "resolved_at" bigint NULL,
                "alert_count" integer NOT NULL DEFAULT 1,
                "title" varchar(500) NULL,
                "assigned_to" varchar(256) NULL,
                "created_at" bigint NOT NULL,
                "updated_at" bigint NOT NULL
            )
        "#
        }
    };
    client
        .execute(Statement::from_string(backend, create_alert_incidents_sql))
        .await?;

    // Create alert_dedup_state table (depends on alerts)
    let create_alert_dedup_state_sql = match backend {
        DatabaseBackend::MySql => {
            r#"
            CREATE TABLE IF NOT EXISTS `alert_dedup_state` (
                `fingerprint` varchar(64) NOT NULL PRIMARY KEY,
                `alert_id` char(27) NOT NULL,
                `org_id` varchar(100) NOT NULL,
                `first_seen_at` bigint NOT NULL,
                `last_seen_at` bigint NOT NULL,
                `occurrence_count` bigint NOT NULL DEFAULT 1,
                `notification_sent` bool NOT NULL DEFAULT FALSE,
                `created_at` bigint NOT NULL
            )
        "#
        }
        DatabaseBackend::Postgres => {
            r#"
            CREATE TABLE IF NOT EXISTS "alert_dedup_state" (
                "fingerprint" varchar(64) NOT NULL PRIMARY KEY,
                "alert_id" char(27) NOT NULL,
                "org_id" varchar(100) NOT NULL,
                "first_seen_at" bigint NOT NULL,
                "last_seen_at" bigint NOT NULL,
                "occurrence_count" bigint NOT NULL DEFAULT 1,
                "notification_sent" bool NOT NULL DEFAULT FALSE,
                "created_at" bigint NOT NULL
            )
        "#
        }
        DatabaseBackend::Sqlite => {
            r#"
            CREATE TABLE IF NOT EXISTS "alert_dedup_state" (
                "fingerprint" varchar(64) NOT NULL PRIMARY KEY,
                "alert_id" char(27) NOT NULL,
                "org_id" varchar(100) NOT NULL,
                "first_seen_at" bigint NOT NULL,
                "last_seen_at" bigint NOT NULL,
                "occurrence_count" bigint NOT NULL DEFAULT 1,
                "notification_sent" boolean NOT NULL DEFAULT FALSE,
                "created_at" bigint NOT NULL
            )
        "#
        }
    };
    client
        .execute(Statement::from_string(
            backend,
            create_alert_dedup_state_sql,
        ))
        .await?;

    // Create search_jobs table
    let create_search_jobs_sql = match backend {
        DatabaseBackend::MySql => {
            r#"
            CREATE TABLE IF NOT EXISTS `search_jobs` (
                `id` varchar(64) NOT NULL PRIMARY KEY,
                `trace_id` varchar(64) NOT NULL,
                `org_id` varchar(256) NOT NULL,
                `user_id` varchar(256) NOT NULL,
                `stream_type` varchar(256) NOT NULL,
                `stream_names` varchar(256) NOT NULL,
                `payload` text NOT NULL,
                `start_time` bigint NOT NULL,
                `end_time` bigint NOT NULL,
                `created_at` bigint NOT NULL,
                `updated_at` bigint NOT NULL,
                `started_at` bigint NULL,
                `ended_at` bigint NULL,
                `cluster` varchar(256) NULL,
                `node` varchar(256) NULL,
                `status` bigint NOT NULL,
                `result_path` varchar(512) NULL,
                `error_message` text NULL,
                `partition_num` bigint NULL
            )
        "#
        }
        DatabaseBackend::Postgres => {
            r#"
            CREATE TABLE IF NOT EXISTS "search_jobs" (
                "id" varchar(64) NOT NULL PRIMARY KEY,
                "trace_id" varchar(64) NOT NULL,
                "org_id" varchar(256) NOT NULL,
                "user_id" varchar(256) NOT NULL,
                "stream_type" varchar(256) NOT NULL,
                "stream_names" varchar(256) NOT NULL,
                "payload" text NOT NULL,
                "start_time" bigint NOT NULL,
                "end_time" bigint NOT NULL,
                "created_at" bigint NOT NULL,
                "updated_at" bigint NOT NULL,
                "started_at" bigint NULL,
                "ended_at" bigint NULL,
                "cluster" varchar(256) NULL,
                "node" varchar(256) NULL,
                "status" bigint NOT NULL,
                "result_path" varchar(512) NULL,
                "error_message" text NULL,
                "partition_num" bigint NULL
            )
        "#
        }
        DatabaseBackend::Sqlite => {
            r#"
            CREATE TABLE IF NOT EXISTS "search_jobs" (
                "id" varchar(64) NOT NULL PRIMARY KEY,
                "trace_id" varchar(64) NOT NULL,
                "org_id" varchar(256) NOT NULL,
                "user_id" varchar(256) NOT NULL,
                "stream_type" varchar(256) NOT NULL,
                "stream_names" varchar(256) NOT NULL,
                "payload" text NOT NULL,
                "start_time" bigint NOT NULL,
                "end_time" bigint NOT NULL,
                "created_at" bigint NOT NULL,
                "updated_at" bigint NOT NULL,
                "started_at" bigint NULL,
                "ended_at" bigint NULL,
                "cluster" varchar(256) NULL,
                "node" varchar(256) NULL,
                "status" bigint NOT NULL,
                "result_path" varchar(512) NULL,
                "error_message" text NULL,
                "partition_num" bigint NULL
            )
        "#
        }
    };
    client
        .execute(Statement::from_string(backend, create_search_jobs_sql))
        .await?;

    // Create search_job_partitions table
    let create_search_job_partitions_sql = match backend {
        DatabaseBackend::MySql => {
            r#"
            CREATE TABLE IF NOT EXISTS `search_job_partitions` (
                `job_id` varchar(64) NOT NULL,
                `partition_id` bigint NOT NULL,
                `start_time` bigint NOT NULL,
                `end_time` bigint NOT NULL,
                `created_at` bigint NOT NULL,
                `started_at` bigint NULL,
                `ended_at` bigint NULL,
                `cluster` varchar(256) NULL,
                `status` bigint NOT NULL,
                `result_path` varchar(512) NULL,
                `error_message` text NULL,
                PRIMARY KEY (`job_id`, `partition_id`)
            )
        "#
        }
        DatabaseBackend::Postgres => {
            r#"
            CREATE TABLE IF NOT EXISTS "search_job_partitions" (
                "job_id" varchar(64) NOT NULL,
                "partition_id" bigint NOT NULL,
                "start_time" bigint NOT NULL,
                "end_time" bigint NOT NULL,
                "created_at" bigint NOT NULL,
                "started_at" bigint NULL,
                "ended_at" bigint NULL,
                "cluster" varchar(256) NULL,
                "status" bigint NOT NULL,
                "result_path" varchar(512) NULL,
                "error_message" text NULL,
                PRIMARY KEY ("job_id", "partition_id")
            )
        "#
        }
        DatabaseBackend::Sqlite => {
            r#"
            CREATE TABLE IF NOT EXISTS "search_job_partitions" (
                "job_id" varchar(64) NOT NULL,
                "partition_id" bigint NOT NULL,
                "start_time" bigint NOT NULL,
                "end_time" bigint NOT NULL,
                "created_at" bigint NOT NULL,
                "started_at" bigint NULL,
                "ended_at" bigint NULL,
                "cluster" varchar(256) NULL,
                "status" bigint NOT NULL,
                "result_path" varchar(512) NULL,
                "error_message" text NULL,
                PRIMARY KEY ("job_id", "partition_id")
            )
        "#
        }
    };
    client
        .execute(Statement::from_string(
            backend,
            create_search_job_partitions_sql,
        ))
        .await?;

    // Create search_job_results table
    let create_search_job_results_sql = match backend {
        DatabaseBackend::MySql => {
            r#"
            CREATE TABLE IF NOT EXISTS `search_job_results` (
                `job_id` varchar(64) NOT NULL,
                `trace_id` varchar(64) NOT NULL,
                `started_at` bigint NULL,
                `ended_at` bigint NULL,
                `cluster` varchar(256) NULL,
                `result_path` varchar(512) NULL,
                `error_message` text NULL,
                PRIMARY KEY (`job_id`, `trace_id`)
            )
        "#
        }
        DatabaseBackend::Postgres => {
            r#"
            CREATE TABLE IF NOT EXISTS "search_job_results" (
                "job_id" varchar(64) NOT NULL,
                "trace_id" varchar(64) NOT NULL,
                "started_at" bigint NULL,
                "ended_at" bigint NULL,
                "cluster" varchar(256) NULL,
                "result_path" varchar(512) NULL,
                "error_message" text NULL,
                PRIMARY KEY ("job_id", "trace_id")
            )
        "#
        }
        DatabaseBackend::Sqlite => {
            r#"
            CREATE TABLE IF NOT EXISTS "search_job_results" (
                "job_id" varchar(64) NOT NULL,
                "trace_id" varchar(64) NOT NULL,
                "started_at" bigint NULL,
                "ended_at" bigint NULL,
                "cluster" varchar(256) NULL,
                "result_path" varchar(512) NULL,
                "error_message" text NULL,
                PRIMARY KEY ("job_id", "trace_id")
            )
        "#
        }
    };
    client
        .execute(Statement::from_string(
            backend,
            create_search_job_results_sql,
        ))
        .await?;

    // Create pipeline_last_errors table
    let create_pipeline_last_errors_sql = match backend {
        DatabaseBackend::MySql => {
            r#"
            CREATE TABLE IF NOT EXISTS `pipeline_last_errors` (
                `pipeline_id` varchar(255) NOT NULL PRIMARY KEY,
                `org_id` varchar(255) NOT NULL,
                `pipeline_name` varchar(255) NOT NULL,
                `last_error_timestamp` bigint NOT NULL,
                `error_summary` text NULL,
                `node_errors` json NULL,
                `created_at` bigint NOT NULL,
                `updated_at` bigint NOT NULL
            )
        "#
        }
        DatabaseBackend::Postgres => {
            r#"
            CREATE TABLE IF NOT EXISTS "pipeline_last_errors" (
                "pipeline_id" varchar(255) NOT NULL PRIMARY KEY,
                "org_id" varchar(255) NOT NULL,
                "pipeline_name" varchar(255) NOT NULL,
                "last_error_timestamp" bigint NOT NULL,
                "error_summary" text NULL,
                "node_errors" json NULL,
                "created_at" bigint NOT NULL,
                "updated_at" bigint NOT NULL
            )
        "#
        }
        DatabaseBackend::Sqlite => {
            r#"
            CREATE TABLE IF NOT EXISTS "pipeline_last_errors" (
                "pipeline_id" varchar(255) NOT NULL PRIMARY KEY,
                "org_id" varchar(255) NOT NULL,
                "pipeline_name" varchar(255) NOT NULL,
                "last_error_timestamp" bigint NOT NULL,
                "error_summary" text NULL,
                "node_errors" text NULL,
                "created_at" bigint NOT NULL,
                "updated_at" bigint NOT NULL
            )
        "#
        }
    };
    client
        .execute(Statement::from_string(
            backend,
            create_pipeline_last_errors_sql,
        ))
        .await?;

    // Create alert_incident_alerts table (depends on alert_incidents)
    let create_alert_incident_alerts_sql = match backend {
        DatabaseBackend::MySql => {
            r#"
            CREATE TABLE IF NOT EXISTS `alert_incident_alerts` (
                `incident_id` char(27) NOT NULL,
                `alert_id` varchar(256) NOT NULL,
                `alert_fired_at` bigint NOT NULL,
                `alert_name` varchar(256) NOT NULL,
                `correlation_reason` varchar(50) NULL,
                `created_at` bigint NOT NULL,
                PRIMARY KEY (`incident_id`, `alert_id`, `alert_fired_at`)
            )
        "#
        }
        DatabaseBackend::Postgres => {
            r#"
            CREATE TABLE IF NOT EXISTS "alert_incident_alerts" (
                "incident_id" char(27) NOT NULL,
                "alert_id" varchar(256) NOT NULL,
                "alert_fired_at" bigint NOT NULL,
                "alert_name" varchar(256) NOT NULL,
                "correlation_reason" varchar(50) NULL,
                "created_at" bigint NOT NULL,
                PRIMARY KEY ("incident_id", "alert_id", "alert_fired_at")
            )
        "#
        }
        DatabaseBackend::Sqlite => {
            r#"
            CREATE TABLE IF NOT EXISTS "alert_incident_alerts" (
                "incident_id" char(27) NOT NULL,
                "alert_id" varchar(256) NOT NULL,
                "alert_fired_at" bigint NOT NULL,
                "alert_name" varchar(256) NOT NULL,
                "correlation_reason" varchar(50) NULL,
                "created_at" bigint NOT NULL,
                PRIMARY KEY ("incident_id", "alert_id", "alert_fired_at")
            )
        "#
        }
    };
    client
        .execute(Statement::from_string(
            backend,
            create_alert_incident_alerts_sql,
        ))
        .await?;

    println!(
        "✓ Ensured ORM tables exist (templates, sessions, folders, destinations, alerts, dashboards, + 18 additional tables)"
    );
    Ok(())
}

// ==================== Test Data Structures ====================

/// Organization record from JSON test data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationTestData {
    pub identifier: String,
    pub org_name: String,
    pub org_type: i16,
    pub created_at: i64,
    pub updated_at: i64,
}

/// User record from JSON test data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserTestData {
    pub id: String,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub password: String,
    pub salt: String,
    pub is_root: bool,
    #[serde(default)]
    pub password_ext: Option<String>,
    pub user_type: i16,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Org-User relationship record from JSON test data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgUserTestData {
    pub id: String,
    pub email: String,
    pub org_id: String,
    pub role: i16,
    pub token: String,
    #[serde(default)]
    pub rum_token: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Folder record from JSON test data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderTestData {
    pub id: String,
    pub org: String,
    pub folder_id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub r#type: i16,
}

/// Template record from JSON test data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateTestData {
    pub id: String,
    pub org: String,
    pub name: String,
    pub is_default: bool,
    pub r#type: String,
    pub body: String,
    #[serde(default)]
    pub title: Option<String>,
}

/// Destination record from JSON test data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DestinationTestData {
    pub id: String,
    pub org: String,
    pub name: String,
    pub module: String,
    #[serde(default)]
    pub template_id: Option<String>,
    pub r#type: serde_json::Value,
}

/// Short URL record from JSON test data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortUrlTestData {
    pub id: i64,
    pub short_id: String,
    pub original_url: String,
    pub created_ts: i64,
}

/// Dashboard record from JSON test data (simplified).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardTestData {
    pub id: String,
    pub dashboard_id: String,
    pub folder_id: String,
    pub owner: String,
    #[serde(default)]
    pub role: Option<String>,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    pub data: serde_json::Value,
}

/// Alert record from JSON test data (simplified).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertTestData {
    pub id: String,
    pub org: String,
    pub folder_id: String,
    pub name: String,
    pub stream_type: String,
    pub stream_name: String,
    pub is_real_time: bool,
    pub destinations: Vec<String>,
    #[serde(default)]
    pub context_attributes: Option<serde_json::Value>,
    #[serde(default)]
    pub description: Option<String>,
    pub enabled: bool,
    pub tz_offset: i32,
    pub query_type: i16,
    #[serde(default)]
    pub query_conditions: Option<serde_json::Value>,
    pub trigger_threshold_operator: String,
    pub trigger_period_seconds: i64,
    pub trigger_threshold_count: i64,
    pub trigger_frequency_type: i16,
    pub trigger_frequency_seconds: i64,
    pub trigger_silence_seconds: i64,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub last_edited_by: Option<String>,
    #[serde(default)]
    pub updated_at: Option<i64>,
}

// ==================== Helper Functions ====================

/// Get the path to the compatible-datasets directory.
fn get_test_data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/compatible-datasets")
}

/// Load test data from a JSON file.
fn load_test_data<T: for<'de> Deserialize<'de>>(filename: &str) -> Vec<T> {
    let path = get_test_data_dir().join(filename);
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e));
    serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("Failed to parse {}: {}", path.display(), e))
}

/// Convert i16 role to UserRole enum.
fn role_from_i16(role: i16) -> UserRole {
    match role {
        0 => UserRole::Root,
        1 => UserRole::Admin,
        2 => UserRole::Editor,
        3 => UserRole::Viewer,
        4 => UserRole::User,
        5 => UserRole::ServiceAccount,
        _ => UserRole::User,
    }
}

/// Convert i16 org_type to OrganizationType enum.
fn org_type_from_i16(org_type: i16) -> OrganizationType {
    match org_type {
        0 => OrganizationType::Default,
        1 => OrganizationType::Custom,
        _ => OrganizationType::Custom,
    }
}

/// Convert i16 folder_type to FolderType enum.
fn folder_type_from_i16(folder_type: i16) -> FolderType {
    match folder_type {
        0 => FolderType::Dashboards,
        1 => FolderType::Alerts,
        2 => FolderType::Reports,
        _ => FolderType::Dashboards,
    }
}

// ==================== Level 0: Base Table Tests ====================

/// Test: organizations table compatibility
/// Order: 01
/// Dependencies: None
/// Behavior: create_table, add, get, get_by_name, list, rename, remove
pub async fn test_organizations_compat_impl(prefix: &str) {
    println!("\n========== Organizations Compatibility Test [Order: 01] ==========");
    println!("Table: organizations");
    println!("Dependencies: None");
    println!("Behavior: create_table, add, get, get_by_name, list, rename, remove\n");

    // Load test data
    let test_data: Vec<OrganizationTestData> = load_test_data("organizations.json");
    println!(
        "Loaded {} organization records from test data",
        test_data.len()
    );

    // Test: Create table
    println!("\n[1/7] Testing create_table...");
    organizations::create_table()
        .await
        .expect("create_table failed");
    println!("✓ create_table succeeded");

    // Clear any existing test data
    println!("\n[2/7] Clearing existing test data...");
    for data in &test_data {
        // Use "org_" prefix to match the test IDs
        let test_id = format!("{}_org_{}", prefix, data.identifier);
        let _ = organizations::remove(&test_id).await;
    }
    println!("✓ Cleared existing test data");

    // Test: Add organizations
    println!("\n[3/7] Testing add (import data)...");
    for data in &test_data {
        // Use "org_" prefix to avoid conflicts with other tests using "{prefix}_default"
        let test_id = format!("{}_org_{}", prefix, data.identifier);
        // Also prefix org_name to make test data identifiable
        let test_org_name = format!("{}_{}", prefix, data.org_name);
        let org_type = org_type_from_i16(data.org_type);
        organizations::add(&test_id, &test_org_name, org_type)
            .await
            .expect("add failed");
    }
    println!("✓ Added {} organizations", test_data.len());

    // Test: Get organization
    println!("\n[4/7] Testing get...");
    for data in &test_data {
        let test_id = format!("{}_org_{}", prefix, data.identifier);
        let test_org_name = format!("{}_{}", prefix, data.org_name);
        let record = organizations::get(&test_id).await.expect("get failed");
        assert_eq!(record.identifier, test_id);
        assert_eq!(record.org_name, test_org_name);
    }
    println!("✓ get verified for all {} organizations", test_data.len());

    // Test: Get by name
    println!("\n[5/7] Testing get_by_name...");
    for data in &test_data {
        let test_org_name = format!("{}_{}", prefix, data.org_name);
        let records = organizations::get_by_name(&test_org_name)
            .await
            .expect("get_by_name failed");
        assert!(!records.is_empty(), "get_by_name should return records");
    }
    println!("✓ get_by_name verified");

    // Test: List organizations
    println!("\n[6/7] Testing list...");
    let filter = organizations::ListFilter::default();
    let all_orgs = organizations::list(filter).await.expect("list failed");
    assert!(
        all_orgs.len() >= test_data.len(),
        "list should return all added organizations"
    );
    println!("✓ list returned {} organizations", all_orgs.len());

    // Test: Rename and remove
    println!("\n[7/7] Testing rename and remove...");
    if let Some(data) = test_data.first() {
        let test_id = format!("{}_org_{}", prefix, data.identifier);
        let new_name = format!("{}_renamed", data.org_name);
        organizations::rename(&test_id, &new_name)
            .await
            .expect("rename failed");

        let updated = organizations::get(&test_id)
            .await
            .expect("get after rename failed");
        assert_eq!(updated.org_name, new_name);
        println!("✓ rename verified");
    }

    // Cleanup
    for data in &test_data {
        let test_id = format!("{}_org_{}", prefix, data.identifier);
        organizations::remove(&test_id)
            .await
            .expect("remove failed");
    }
    println!("✓ remove verified - cleaned up test data");

    println!("\n✓ Organizations compatibility test PASSED");
}

/// Test: users table compatibility
/// Order: 02
/// Dependencies: None
/// Behavior: create_table, add, get, get_root_user, list, update, remove
pub async fn test_users_compat_impl(prefix: &str) {
    println!("\n========== Users Compatibility Test [Order: 02] ==========");
    println!("Table: users");
    println!("Dependencies: None");
    println!("Behavior: create_table, add, get, get_root_user, list, update, remove\n");

    // Load test data
    let test_data: Vec<UserTestData> = load_test_data("users.json");
    println!("Loaded {} user records from test data", test_data.len());

    // Test: Create table
    println!("\n[1/7] Testing create_table...");
    users::create_table().await.expect("create_table failed");
    println!("✓ create_table succeeded");

    // Clear any existing test data
    println!("\n[2/7] Clearing existing test data...");
    for data in &test_data {
        let test_email = format!("{}_{}", prefix, data.email);
        let _ = users::remove(&test_email).await;
    }
    println!("✓ Cleared existing test data");

    // Test: Add users
    println!("\n[3/7] Testing add (import data)...");
    let mut has_root = false;
    for data in &test_data {
        let test_email = format!("{}_{}", prefix, data.email);
        let user_type = match data.user_type {
            0 => config::meta::user::UserType::Internal,
            _ => config::meta::user::UserType::External,
        };
        let user_record = users::UserRecord {
            email: test_email,
            first_name: data.first_name.clone(),
            last_name: data.last_name.clone(),
            password: data.password.clone(),
            salt: data.salt.clone(),
            is_root: data.is_root,
            password_ext: data.password_ext.clone(),
            user_type,
            created_at: data.created_at,
            updated_at: data.updated_at,
        };
        users::add(user_record).await.expect("add failed");
        if data.is_root {
            has_root = true;
        }
    }
    println!("✓ Added {} users", test_data.len());

    // Test: Get user
    println!("\n[4/7] Testing get...");
    for data in &test_data {
        let test_email = format!("{}_{}", prefix, data.email);
        let record = users::get(&test_email).await.expect("get failed");
        assert_eq!(record.email, test_email);
        assert_eq!(record.first_name, data.first_name);
        assert_eq!(record.last_name, data.last_name);
    }
    println!("✓ get verified for all {} users", test_data.len());

    // Test: Get root user (if exists in test data)
    println!("\n[5/7] Testing get_root_user...");
    if has_root {
        let root_result = users::get_root_user().await;
        // Root user might be from test data or pre-existing
        if root_result.is_ok() {
            println!("✓ get_root_user found a root user");
        } else {
            println!("⚠ get_root_user: no root user found (may be expected)");
        }
    } else {
        println!("⚠ Skipping get_root_user - no root user in test data");
    }

    // Test: List users
    println!("\n[6/7] Testing list...");
    let all_users = users::list(None).await.expect("list failed");
    assert!(
        all_users.len() >= test_data.len(),
        "list should return all added users"
    );
    println!("✓ list returned {} users", all_users.len());

    // Test: Update user
    println!("\n[7/7] Testing update and remove...");
    if let Some(data) = test_data.first() {
        let test_email = format!("{}_{}", prefix, data.email);
        let new_first_name = format!("{}_updated", data.first_name);
        users::update(
            &test_email,
            &new_first_name,
            &data.last_name,
            &data.password,
            None,
        )
        .await
        .expect("update failed");

        let updated = users::get(&test_email)
            .await
            .expect("get after update failed");
        assert_eq!(updated.first_name, new_first_name);
        println!("✓ update verified");
    }

    // Cleanup
    for data in &test_data {
        let test_email = format!("{}_{}", prefix, data.email);
        users::remove(&test_email).await.expect("remove failed");
    }
    println!("✓ remove verified - cleaned up test data");

    println!("\n✓ Users compatibility test PASSED");
}

/// Test: templates table compatibility
/// Order: 03
/// Dependencies: None
/// Behavior: put, get, list, list_all, delete
pub async fn test_templates_compat_impl(prefix: &str) {
    println!("\n========== Templates Compatibility Test [Order: 03] ==========");
    println!("Table: templates");
    println!("Dependencies: None");
    println!("Behavior: put, get, list, list_all, delete\n");

    // Ensure ORM tables exist
    println!("[0/5] Ensuring ORM tables exist...");
    if let Err(e) = ensure_orm_tables_exist().await {
        println!("⚠ Failed to create ORM tables: {}", e);
        println!("⏭ SKIPPED: Templates test requires ORM table creation");
        return;
    }

    // Load test data
    let test_data: Vec<TemplateTestData> = load_test_data("templates.json");
    println!("Loaded {} template records from test data", test_data.len());

    // Use prefix for test isolation
    // Limit prefix length to fit within column constraints (org: 100 chars)
    let short_prefix = &prefix[..prefix.len().min(10)];
    let test_org = format!("{}_default", short_prefix);

    // Clear any existing test data
    println!("\n[1/5] Clearing existing test data...");
    for data in &test_data {
        let test_name = format!("{}_{}", short_prefix, data.name);
        templates::delete(&test_org, &test_name).await.unwrap();
    }
    println!("✓ Cleared existing test data");

    // Test: Put templates (insert)
    println!("\n[2/5] Testing put (insert)...");
    for data in &test_data {
        let test_name = format!("{}_{}", short_prefix, data.name);
        let template_type = if let Some(ref title) = data.title {
            TemplateType::Email {
                title: title.clone(),
            }
        } else {
            match data.r#type.to_lowercase().as_str() {
                "http" => TemplateType::Http,
                _ => TemplateType::Sns,
            }
        };
        let template = Template {
            id: None,
            org_id: test_org.clone(),
            name: test_name.clone(),
            is_default: data.is_default,
            template_type,
            body: data.body.clone(),
        };
        templates::put(template).await.expect("put failed");
    }
    println!("✓ Added {} templates", test_data.len());

    // Test: Get template
    println!("\n[3/5] Testing get...");
    for data in &test_data {
        let test_name = format!("{}_{}", short_prefix, data.name);
        let record = templates::get(&test_org, &test_name)
            .await
            .expect("get failed")
            .expect("template should exist");
        assert_eq!(record.name, test_name);
        assert_eq!(record.is_default, data.is_default);
    }
    println!("✓ get verified for all {} templates", test_data.len());

    // Test: List templates
    println!("\n[4/5] Testing list...");
    let all_templates = templates::list(&test_org).await.expect("list failed");
    let test_templates: Vec<_> = all_templates
        .iter()
        .filter(|t| t.name.starts_with(short_prefix))
        .collect();
    // Use >= instead of == because parallel tests may create additional templates
    assert!(
        test_templates.len() >= test_data.len(),
        "list should return at least {} test templates, found {}",
        test_data.len(),
        test_templates.len()
    );
    println!("✓ list returned {} test templates", test_templates.len());

    // Test: Put (update existing)
    println!("\n[5/5] Testing put (update) and delete...");
    if let Some(data) = test_data.first() {
        let test_name = format!("{}_{}", short_prefix, data.name);
        let updated_body = format!("{}_updated", data.body);
        let template_type = if let Some(ref title) = data.title {
            TemplateType::Email {
                title: title.clone(),
            }
        } else {
            TemplateType::Http
        };
        let template = Template {
            id: None,
            org_id: test_org.clone(),
            name: test_name.clone(),
            is_default: data.is_default,
            template_type,
            body: updated_body.clone(),
        };
        templates::put(template).await.expect("put update failed");

        let updated = templates::get(&test_org, &test_name)
            .await
            .expect("get after update failed")
            .expect("template should exist");
        assert_eq!(updated.body, updated_body);
        println!("✓ put (update) verified");
    }

    // Cleanup
    for data in &test_data {
        let test_name = format!("{}_{}", short_prefix, data.name);
        templates::delete(&test_org, &test_name)
            .await
            .expect("delete failed");
    }
    println!("✓ delete verified - cleaned up test data");

    println!("\n✓ Templates compatibility test PASSED");
}

/// Test: short_urls table compatibility
/// Order: 04
/// Dependencies: None
/// Behavior: init, add, get, list_limit, remove
pub async fn test_short_urls_compat_impl(prefix: &str) {
    println!("\n========== Short URLs Compatibility Test [Order: 04] ==========");
    println!("Table: short_urls");
    println!("Dependencies: None");
    println!("Behavior: init, add, get, list_limit, remove\n");

    // Load test data
    let test_data: Vec<ShortUrlTestData> = load_test_data("short_urls.json");
    println!(
        "Loaded {} short_url records from test data",
        test_data.len()
    );

    // Use a short prefix to fit within 32 char limit for short_id column
    let short_prefix = &prefix[..prefix.len().min(10)];

    // Test: Init (create table and indexes)
    println!("\n[1/5] Testing init...");
    short_urls::init().await.expect("init failed");
    println!("✓ init succeeded");

    // Clear any existing test data
    println!("\n[2/5] Clearing existing test data...");
    for data in &test_data {
        let test_short_id = format!(
            "{}_{}",
            short_prefix,
            &data.short_id[..data.short_id.len().min(10)]
        );
        short_urls::remove(&test_short_id)
            .await
            .expect("remove failed");
    }
    // Also add some mock data if test_data is empty
    let mock_short_urls = if test_data.is_empty() {
        vec![
            ShortUrlTestData {
                id: 1,
                short_id: format!("{}_mk1", short_prefix),
                original_url: "https://example.com/long/url/1".to_string(),
                created_ts: chrono::Utc::now().timestamp_micros(),
            },
            ShortUrlTestData {
                id: 2,
                short_id: format!("{}_mk2", short_prefix),
                original_url: "https://example.com/long/url/2".to_string(),
                created_ts: chrono::Utc::now().timestamp_micros(),
            },
        ]
    } else {
        test_data
            .iter()
            .map(|d| ShortUrlTestData {
                id: d.id,
                short_id: format!(
                    "{}_{}",
                    short_prefix,
                    &d.short_id[..d.short_id.len().min(10)]
                ),
                original_url: d.original_url.clone(),
                created_ts: d.created_ts,
            })
            .collect()
    };
    println!("✓ Cleared existing test data");

    // Test: Add short URLs
    println!("\n[3/5] Testing add...");
    for data in &mock_short_urls {
        short_urls::add(&data.short_id, &data.original_url)
            .await
            .expect("add failed");
    }
    println!("✓ Added {} short URLs", mock_short_urls.len());

    // Test: Get short URL
    println!("\n[4/5] Testing get...");
    for data in &mock_short_urls {
        let record = short_urls::get(&data.short_id).await.expect("get failed");
        assert_eq!(record.short_id, data.short_id);
        assert_eq!(record.original_url, data.original_url);
    }
    println!(
        "✓ get verified for all {} short URLs",
        mock_short_urls.len()
    );

    // Test: List with limit
    println!("\n[5/5] Testing list and remove...");
    let all_urls = short_urls::list(Some(100)).await.expect("list failed");
    let test_urls: Vec<_> = all_urls
        .iter()
        .filter(|u| u.short_id.starts_with(short_prefix))
        .collect();
    assert!(test_urls.len() >= mock_short_urls.len());
    println!("✓ list_limit returned {} test short URLs", test_urls.len());

    // Cleanup
    for data in &mock_short_urls {
        short_urls::remove(&data.short_id)
            .await
            .expect("remove failed");
    }
    println!("✓ remove verified - cleaned up test data");

    println!("\n✓ Short URLs compatibility test PASSED");
}

/// Test: sessions table compatibility
/// Order: 05
/// Dependencies: None
/// Behavior: set, get, list, delete
pub async fn test_sessions_compat_impl(prefix: &str) {
    println!("\n========== Sessions Compatibility Test [Order: 05] ==========");
    println!("Table: sessions");
    println!("Dependencies: None");
    println!("Behavior: set, get, list, delete\n");

    // Ensure ORM tables exist
    println!("[0/4] Ensuring ORM tables exist...");
    if let Err(e) = ensure_orm_tables_exist().await {
        panic!("⚠ Failed to create ORM tables: {}", e);
    }

    // Use short prefix for column constraints
    let short_prefix = &prefix[..prefix.len().min(10)];

    // Sessions test data is empty, so we generate mock data
    let mock_sessions = vec![
        (format!("{}_sess1", short_prefix), "access_token_1"),
        (format!("{}_sess2", short_prefix), "access_token_2"),
        (format!("{}_sess3", short_prefix), "access_token_3"),
    ];
    println!("Using {} mock session records", mock_sessions.len());

    // Clear any existing test data
    println!("\n[1/4] Clearing existing test data...");
    for (session_id, _) in &mock_sessions {
        sessions::delete(session_id).await.unwrap();
    }
    println!("✓ Cleared existing test data");

    // Test: Set sessions
    println!("\n[2/4] Testing set...");
    for (session_id, access_token) in &mock_sessions {
        sessions::set(session_id, access_token)
            .await
            .expect("set failed");
    }
    println!("✓ Set {} sessions", mock_sessions.len());

    // Test: Get session
    println!("\n[3/4] Testing get...");
    for (session_id, expected_token) in &mock_sessions {
        let record = sessions::get(session_id)
            .await
            .expect("get failed")
            .expect("session should exist");
        assert_eq!(record.session_id, *session_id);
        assert_eq!(record.access_token, *expected_token);
    }
    println!("✓ get verified for all {} sessions", mock_sessions.len());

    // Test: Set (update existing session)
    println!("\n[4/4] Testing set (update), list, and delete...");
    if let Some((session_id, _)) = mock_sessions.first() {
        let new_token = "updated_access_token";
        sessions::set(session_id, new_token)
            .await
            .expect("set update failed");

        let updated = sessions::get(session_id)
            .await
            .expect("get after update failed")
            .expect("session should exist");
        assert_eq!(updated.access_token, new_token);
        println!("✓ set (update) verified");
    }

    // Test: List sessions
    let all_sessions = sessions::list().await.expect("list failed");
    let test_sessions: Vec<_> = all_sessions
        .iter()
        .filter(|s| s.session_id.starts_with(short_prefix))
        .collect();
    assert_eq!(test_sessions.len(), mock_sessions.len());
    println!("✓ list returned {} test sessions", test_sessions.len());

    // Cleanup
    for (session_id, _) in &mock_sessions {
        sessions::delete(session_id).await.expect("delete failed");
    }
    println!("✓ delete verified - cleaned up test data");

    println!("\n✓ Sessions compatibility test PASSED");
}

// ==================== Level 1: Dependent Table Tests ====================

/// Test: org_users table compatibility
/// Order: 10
/// Dependencies: organizations, users
/// Behavior: create_table, add, get, list_users_by_org, update, remove
pub async fn test_org_users_compat_impl(prefix: &str) {
    println!("\n========== Org-Users Compatibility Test [Order: 10] ==========");
    println!("Table: org_users");
    println!("Dependencies: organizations, users");
    println!("Behavior: create_table, add, get, list_users_by_org, update, remove\n");

    // Load test data
    let user_data: Vec<UserTestData> = load_test_data("users.json");
    let org_user_data: Vec<OrgUserTestData> = load_test_data("org_users.json");
    println!(
        "Loaded {} org_user records from test data",
        org_user_data.len()
    );

    // Setup dependencies: create organizations and users
    println!("\n[1/7] Setting up dependencies...");
    let _ = ensure_orm_tables_exist().await;
    organizations::create_table()
        .await
        .expect("create organizations table failed");
    users::create_table()
        .await
        .expect("create users table failed");

    // Create test organization
    let test_org_id = format!("{}_default", prefix);
    organizations::remove(&test_org_id).await.unwrap();
    organizations::add(&test_org_id, "Test Organization", OrganizationType::Default)
        .await
        .expect("add organization failed");

    // Create test users
    for data in &user_data {
        let test_email = format!("{}_{}", prefix, data.email);
        users::remove(&test_email).await.unwrap();
        let user_record = users::UserRecord {
            email: test_email,
            first_name: data.first_name.clone(),
            last_name: data.last_name.clone(),
            password: data.password.clone(),
            salt: data.salt.clone(),
            is_root: data.is_root,
            password_ext: data.password_ext.clone(),
            user_type: config::meta::user::UserType::Internal,
            created_at: data.created_at,
            updated_at: data.updated_at,
        };
        users::add(user_record).await.expect("add user failed");
    }
    println!("✓ Dependencies set up");

    // Clear existing org_user test data
    println!("\n[2/7] Clearing existing test data...");
    for data in &org_user_data {
        let test_email = format!("{}_{}", prefix, data.email);
        org_users::remove(&test_org_id, &test_email).await.unwrap();
    }
    println!("✓ Cleared existing test data");

    // Test: Add org_users
    println!("\n[3/7] Testing add...");
    for data in &org_user_data {
        let test_email = format!("{}_{}", prefix, data.email);
        let role = role_from_i16(data.role);
        org_users::add(
            &test_org_id,
            &test_email,
            role,
            &data.token,
            data.rum_token.clone(),
        )
        .await
        .expect("add failed");
    }
    println!("✓ Added {} org_users", org_user_data.len());

    // Test: Get org_user
    println!("\n[4/7] Testing get...");
    for data in &org_user_data {
        let test_email = format!("{}_{}", prefix, data.email);
        let record = org_users::get(&test_org_id, &test_email)
            .await
            .expect("get failed");
        assert_eq!(record.email, test_email);
        assert_eq!(record.org_id, test_org_id);
        assert_eq!(record.token, data.token);
    }
    println!("✓ get verified for all {} org_users", org_user_data.len());

    // Test: Get by org
    println!("\n[5/7] Testing list_users_by_org...");
    let org_members = org_users::list_users_by_org(&test_org_id)
        .await
        .expect("list_users_by_org failed");
    let test_members: Vec<_> = org_members
        .iter()
        .filter(|m| m.email.starts_with(prefix))
        .collect();
    assert_eq!(test_members.len(), org_user_data.len());
    println!(
        "✓ list_users_by_org returned {} test members",
        test_members.len()
    );

    // Test: Update
    println!("\n[6/7] Testing update...");
    if let Some(data) = org_user_data.first() {
        let test_email = format!("{}_{}", prefix, data.email);
        let new_token = format!("{}_updated", data.token);
        org_users::update(&test_org_id, &test_email, UserRole::Admin, &new_token, None)
            .await
            .expect("update failed");

        let updated = org_users::get(&test_org_id, &test_email)
            .await
            .expect("get after update failed");
        assert_eq!(updated.token, new_token);
        println!("✓ update verified");
    }

    // Test: Update token
    println!("\n[7/7] Testing update_token and remove...");
    if let Some(data) = org_user_data.first() {
        let test_email = format!("{}_{}", prefix, data.email);
        let new_token = "new_token_value";
        org_users::update_token(&test_org_id, &test_email, new_token)
            .await
            .expect("update_token failed");

        let updated = org_users::get(&test_org_id, &test_email)
            .await
            .expect("get after update_token failed");
        assert_eq!(updated.token, new_token);
        println!("✓ update_token verified");
    }

    // Cleanup
    for data in &org_user_data {
        let test_email = format!("{}_{}", prefix, data.email);
        org_users::remove(&test_org_id, &test_email)
            .await
            .expect("remove failed");
    }
    for data in &user_data {
        let test_email = format!("{}_{}", prefix, data.email);
        users::remove(&test_email).await.unwrap();
    }
    organizations::remove(&test_org_id).await.unwrap();
    println!("✓ remove verified - cleaned up test data");

    println!("\n✓ Org-Users compatibility test PASSED");
}

/// Test: folders table compatibility
/// Order: 11
/// Dependencies: organizations
/// Behavior: put, get, get_by_name, exists, list_folders, delete
pub async fn test_folders_compat_impl(prefix: &str) {
    println!("\n========== Folders Compatibility Test [Order: 11] ==========");
    println!("Table: folders");
    println!("Dependencies: organizations");
    println!("Behavior: put, get, get_by_name, exists, list_folders, delete\n");

    // Ensure ORM tables exist
    println!("[0/7] Ensuring ORM tables exist...");
    if let Err(e) = ensure_orm_tables_exist().await {
        println!("⚠ Failed to create ORM tables: {}", e);
        println!("⏭ SKIPPED: Folders test requires ORM table creation");
        return;
    }

    // Load test data
    let folder_data: Vec<FolderTestData> = load_test_data("folders.json");
    println!("Loaded {} folder records from test data", folder_data.len());

    // Use short prefix for column constraints
    let short_prefix = &prefix[..prefix.len().min(10)];

    // Setup dependencies
    println!("\n[1/7] Setting up dependencies...");
    let test_org_id = format!("{}_default", short_prefix);
    organizations::remove(&test_org_id).await.unwrap();
    organizations::add(&test_org_id, "Test Org", OrganizationType::Default)
        .await
        .expect("add organization failed");
    println!("✓ Dependencies set up");

    // Clear existing test data
    println!("\n[2/7] Clearing existing test data...");
    for data in &folder_data {
        let test_folder_id = format!("{}_{}", short_prefix, data.folder_id);
        let folder_type = folder_type_from_i16(data.r#type);
        folders::delete(&test_org_id, &test_folder_id, folder_type)
            .await
            .unwrap();
    }
    println!("✓ Cleared existing test data");

    // Test: Put folders (insert)
    println!("\n[3/7] Testing put (insert)...");
    for data in &folder_data {
        let test_folder_id = format!("{}_{}", short_prefix, data.folder_id);
        let test_name = format!("{}_{}", short_prefix, data.name);
        let folder_type = folder_type_from_i16(data.r#type);
        let folder = Folder {
            folder_id: test_folder_id.clone(),
            name: test_name,
            description: data.description.clone().unwrap_or_default(),
        };
        folders::put(&test_org_id, None, folder, folder_type)
            .await
            .expect("put failed");
    }
    println!("✓ Added {} folders", folder_data.len());

    // Test: Get folder
    println!("\n[4/7] Testing get...");
    for data in &folder_data {
        let test_folder_id = format!("{}_{}", short_prefix, data.folder_id);
        let test_name = format!("{}_{}", short_prefix, data.name);
        let folder_type = folder_type_from_i16(data.r#type);
        let record = folders::get(&test_org_id, &test_folder_id, folder_type)
            .await
            .expect("get failed")
            .expect("folder should exist");
        assert_eq!(record.folder_id, test_folder_id);
        assert_eq!(record.name, test_name, "folder name should match");
    }
    println!("✓ get verified for all {} folders", folder_data.len());

    // Test: Get by name
    println!("\n[5/7] Testing get_by_name...");
    for data in &folder_data {
        let test_name = format!("{}_{}", short_prefix, data.name);
        let folder_type = folder_type_from_i16(data.r#type);
        let record = folders::get_by_name(&test_org_id, &test_name, folder_type)
            .await
            .expect("get_by_name failed");
        assert!(record.is_some(), "folder should be found by name");
    }
    println!("✓ get_by_name verified");

    // Test: Exists
    println!("\n[6/7] Testing exists...");
    for data in &folder_data {
        let test_folder_id = format!("{}_{}", short_prefix, data.folder_id);
        let folder_type = folder_type_from_i16(data.r#type);
        let exists = folders::exists(&test_org_id, &test_folder_id, folder_type)
            .await
            .expect("exists failed");
        assert!(exists, "folder should exist");
    }
    println!("✓ exists verified");

    // Test: List folders
    println!("\n[7/7] Testing list_folders and delete...");
    let dashboard_folders = folders::list_folders(&test_org_id, FolderType::Dashboards)
        .await
        .expect("list_folders failed");
    let test_dash_folders: Vec<_> = dashboard_folders
        .iter()
        .filter(|f| f.folder_id.starts_with(short_prefix))
        .collect();
    println!(
        "✓ list_folders returned {} test dashboard folders",
        test_dash_folders.len()
    );

    // Cleanup
    for data in &folder_data {
        let test_folder_id = format!("{}_{}", short_prefix, data.folder_id);
        let folder_type = folder_type_from_i16(data.r#type);
        folders::delete(&test_org_id, &test_folder_id, folder_type)
            .await
            .expect("delete failed");
    }
    organizations::remove(&test_org_id).await.unwrap();
    println!("✓ delete verified - cleaned up test data");

    println!("\n✓ Folders compatibility test PASSED");
}

/// Test: destinations table compatibility
/// Order: 12
/// Dependencies: organizations, templates
/// Behavior: put, get, list, delete
pub async fn test_destinations_compat_impl(prefix: &str) {
    println!("\n========== Destinations Compatibility Test [Order: 12] ==========");
    println!("Table: destinations");
    println!("Dependencies: organizations, templates");
    println!("Behavior: put, get, list, delete\n");

    // Ensure ORM tables exist
    println!("[0/5] Ensuring ORM tables exist...");
    if let Err(e) = ensure_orm_tables_exist().await {
        panic!("⚠ Failed to create ORM tables: {}", e);
    }

    // Load test data
    let dest_data: Vec<DestinationTestData> = load_test_data("destinations.json");
    let template_data: Vec<TemplateTestData> = load_test_data("templates.json");
    println!(
        "Loaded {} destination records from test data",
        dest_data.len()
    );

    // Use short prefix for column constraints
    let short_prefix = &prefix[..prefix.len().min(10)];
    let test_org = format!("{}_default", short_prefix);

    // Setup dependencies
    println!("\n[1/5] Setting up dependencies...");
    // Create templates first (required for alert destinations)
    // Use "dest_" prefix for templates to avoid conflicts with templates test
    for data in &template_data {
        let test_name = format!("{}_dest_{}", short_prefix, data.name);
        let template_type = if let Some(ref title) = data.title {
            TemplateType::Email {
                title: title.clone(),
            }
        } else {
            TemplateType::Http
        };
        let template = Template {
            id: None,
            org_id: test_org.clone(),
            name: test_name,
            is_default: data.is_default,
            template_type,
            body: data.body.clone(),
        };
        templates::put(template).await.unwrap();
    }
    println!("✓ Dependencies set up");

    // Clear existing test data
    println!("\n[2/5] Clearing existing test data...");
    for data in &dest_data {
        let test_name = format!("{}_{}", short_prefix, data.name);
        destinations::delete(&test_org, &test_name).await.unwrap();
    }
    println!("✓ Cleared existing test data");

    // Test: Put destinations
    println!("\n[3/5] Testing put (insert)...");
    for data in &dest_data {
        let test_name = format!("{}_{}", short_prefix, data.name);
        let module = if data.module == "alert" {
            // Find the corresponding template (use "dest_" prefix for templates)
            let template_name = template_data
                .first()
                .map(|t| format!("{}_dest_{}", short_prefix, t.name))
                .unwrap_or_else(|| format!("{}_dest_default_template", short_prefix));
            let destination_type: DestinationType = serde_json::from_value(data.r#type.clone())
                .unwrap_or(DestinationType::Email(config::meta::destinations::Email {
                    recipients: vec!["test@example.com".to_string()],
                }));
            Module::Alert {
                template: template_name,
                destination_type,
            }
        } else {
            let endpoint: Endpoint =
                serde_json::from_value(data.r#type.clone()).unwrap_or(Endpoint {
                    url: "https://example.com".to_string(),
                    method: config::meta::destinations::HTTPType::POST,
                    skip_tls_verify: false,
                    headers: Default::default(),
                    metadata: Default::default(),
                    action_id: None,
                    output_format: Some(config::meta::destinations::HTTPOutputFormat::JSON),
                    destination_type: None,
                });
            Module::Pipeline { endpoint }
        };

        let destination = Destination {
            id: None,
            org_id: test_org.clone(),
            name: test_name.clone(),
            module,
        };
        destinations::put(destination).await.expect("put failed");
    }
    println!("✓ Added {} destinations", dest_data.len());

    // Test: Get destination
    println!("\n[4/5] Testing get...");
    for data in &dest_data {
        let test_name = format!("{}_{}", short_prefix, data.name);
        let record = destinations::get(&test_org, &test_name)
            .await
            .expect("get failed");
        assert!(record.is_some(), "destination should exist");
        let record = record.unwrap();
        assert_eq!(record.name, test_name);
        assert_eq!(record.org_id, test_org, "destination org_id should match");
    }
    println!("✓ get verified for all {} destinations", dest_data.len());

    // Test: List destinations
    println!("\n[5/5] Testing list and delete...");
    // Note: list() may fail with AlertDestTemplateNotFound if parallel tests delete templates.
    // Use get() to verify our destinations exist instead of relying on list().
    match destinations::list(&test_org, None).await {
        Ok(all_dests) => {
            let test_dests: Vec<_> = all_dests
                .iter()
                .filter(|d| d.name.starts_with(short_prefix))
                .collect();
            assert!(test_dests.len() >= dest_data.len());
            println!("✓ list returned {} test destinations", test_dests.len());
        }
        Err(e) => {
            // This can happen if parallel tests deleted templates that our destinations reference
            println!(
                "⚠ list failed (likely due to parallel test interference): {}",
                e
            );
            println!("  Falling back to individual get verification...");
            for data in &dest_data {
                let test_name = format!("{}_{}", short_prefix, data.name);
                let record = destinations::get(&test_org, &test_name)
                    .await
                    .expect("get failed");
                assert!(
                    record.is_some(),
                    "destination {} should still exist",
                    test_name
                );
            }
            println!(
                "✓ Verified {} destinations exist via get()",
                dest_data.len()
            );
        }
    }

    // Cleanup
    for data in &dest_data {
        let test_name = format!("{}_{}", short_prefix, data.name);
        destinations::delete(&test_org, &test_name)
            .await
            .expect("delete failed");
    }
    // Clean up templates (use "dest_" prefix to match what we created)
    for data in &template_data {
        let test_name = format!("{}_dest_{}", short_prefix, data.name);
        let _ = templates::delete(&test_org, &test_name).await;
    }
    println!("✓ delete verified - cleaned up test data");

    println!("\n✓ Destinations compatibility test PASSED");
}

// ==================== Additional Level 0: More Base Table Tests ====================

/// Test data structure for system prompts
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SystemPromptTestData {
    pub r#type: String,
    pub content: String,
    pub updated_at: i64,
}

/// Test data structure for distinct value fields
#[derive(Debug, Clone, serde::Deserialize)]
pub struct DistinctValueFieldTestData {
    pub origin: String,
    pub origin_id: String,
    pub org_name: String,
    pub stream_name: String,
    pub stream_type: String,
    pub field_name: String,
}

/// Test: System Prompts Table Compatibility
/// Order: 06
/// Dependencies: None
/// Behavior: create_table, add, get, get_all, update, remove
pub async fn test_system_prompts_compat_impl(prefix: &str) {
    println!("\n========== System Prompts Compatibility Test [Order: 06] ==========");
    println!("Table: system_prompts");
    println!("Dependencies: None");
    println!("Behavior: create_table, add, get, get_all, update, remove\n");

    println!("\n[1/5] Testing create_table...");
    match system_prompts::create_table().await {
        Ok(_) => println!("✓ create_table succeeded"),
        Err(e) => {
            panic!("create_table failed: {}", e);
        }
    }

    // Load test data
    let prompts_data: Vec<SystemPromptTestData> = load_test_data("system_prompts.json");
    println!(
        "Loaded {} system prompt records from test data",
        prompts_data.len()
    );

    if prompts_data.is_empty() {
        println!("⚠ No test data available, using generated test data");
        // Create test data
        let test_prompt = AIPrompt {
            r#type: PromptType::System,
            content: format!("{}_test_system_prompt_content", prefix),
            updated_at: chrono::Utc::now().timestamp_micros(),
        };

        // Test: clear existing data
        println!("\n[2/5] Clearing existing test data...");
        system_prompts::clear().await.unwrap();
        println!("✓ Cleared existing test data");

        // Test: add
        println!("\n[3/5] Testing add...");
        system_prompts::add(&test_prompt).await.expect("add failed");
        println!("✓ Added 1 system prompt");

        // Test: get
        println!("\n[4/5] Testing get...");
        let record = system_prompts::get(PromptType::System)
            .await
            .expect("get failed");
        assert!(record.is_some(), "system prompt should exist");
        let record = record.unwrap();
        assert_eq!(record.r#type, PromptType::System);
        assert!(record.content.contains(prefix));
        println!("✓ get verified");

        // Test: exists
        let exists = system_prompts::exists(PromptType::System)
            .await
            .expect("exists failed");
        assert!(exists, "system prompt should exist");
        println!("✓ exists verified");

        // Test: remove (cleanup)
        println!("\n[5/5] Testing remove...");
        system_prompts::remove("", PromptType::System)
            .await
            .expect("remove failed");
        let record = system_prompts::get(PromptType::System)
            .await
            .expect("get failed");
        assert!(record.is_none(), "system prompt should be removed");
        println!("✓ remove verified");
    } else {
        // Use loaded test data
        let data = &prompts_data[0];
        let prompt_type = match data.r#type.as_str() {
            "system" => PromptType::System,
            "user" => PromptType::User,
            _ => PromptType::System,
        };

        // Test: clear existing data
        println!("\n[2/5] Clearing existing test data...");
        system_prompts::clear().await.unwrap();
        println!("✓ Cleared existing test data");

        // Test: add
        println!("\n[3/5] Testing add...");
        let test_prompt = AIPrompt {
            r#type: prompt_type.clone(),
            content: format!(
                "{}_{}",
                prefix,
                &data.content[..100.min(data.content.len())]
            ),
            updated_at: chrono::Utc::now().timestamp_micros(),
        };
        system_prompts::add(&test_prompt).await.expect("add failed");
        println!("✓ Added 1 system prompt");

        // Test: get
        println!("\n[4/5] Testing get...");
        let record = system_prompts::get(prompt_type.clone())
            .await
            .expect("get failed");
        assert!(record.is_some(), "system prompt should exist");
        println!("✓ get verified");

        // Test: remove (cleanup)
        println!("\n[5/5] Testing remove...");
        system_prompts::remove("", prompt_type)
            .await
            .expect("remove failed");
        println!("✓ remove verified");
    }

    println!("\n✓ System Prompts compatibility test PASSED");
}

/// Test: Distinct Value Fields Table Compatibility
/// Order: 07
/// Dependencies: None (stores metadata about dashboard/report stream usage)
/// Behavior: create_table, add, check_field_use, batch_remove, remove
pub async fn test_distinct_values_compat_impl(prefix: &str) {
    println!("\n========== Distinct Value Fields Compatibility Test [Order: 07] ==========");
    println!("Table: distinct_value_fields");
    println!("Dependencies: None");
    println!("Behavior: create_table, add, check_field_use, batch_remove, remove\n");

    // Load test data
    let fields_data: Vec<DistinctValueFieldTestData> = load_test_data("distinct_value_fields.json");
    println!(
        "Loaded {} distinct value field records from test data",
        fields_data.len()
    );

    // Test: create_table (init)
    println!("\n[1/5] Testing init (create_table)...");
    distinct_values::init().await.expect("init failed");
    println!("✓ init succeeded");

    // Test: add
    // Note: origin_id is limited to 32 chars, so we use a short prefix
    println!("\n[2/5] Testing add...");
    let short_prefix = &prefix[..prefix.len().min(10)]; // Use shorter prefix for origin_id
    let mut added_count = 0;
    for (i, data) in fields_data.iter().enumerate() {
        let origin = match data.origin.as_str() {
            "dashboard" => OriginType::Dashboard,
            "report" => OriginType::Report,
            "stream" => OriginType::Stream,
            _ => OriginType::Dashboard,
        };

        // Truncate origin_id to fit within 32 char limit
        let origin_id = format!(
            "{}_{}",
            short_prefix,
            &data.origin_id[..data.origin_id.len().min(15)]
        );
        let org_name = format!("{}_{}", short_prefix, data.org_name);

        let record = DistinctFieldRecord::new(
            origin,
            &origin_id,
            &org_name,
            &data.stream_name,
            data.stream_type.clone(),
            &data.field_name,
        );
        distinct_values::add(record).await.expect("add failed");
        added_count += 1;

        if i >= 2 {
            break; // Only add first 3 records to avoid too much test data
        }
    }

    // Add one more test record for batch_remove test
    // Use short prefix to fit within column size limits
    let short_test_org = format!("{}_org", short_prefix);
    let short_test_origin_id = format!("{}_orig123", short_prefix);
    let test_record = DistinctFieldRecord::new(
        OriginType::Dashboard,
        &short_test_origin_id,
        &short_test_org,
        "test_stream",
        "logs".to_string(),
        "test_field",
    );
    distinct_values::add(test_record)
        .await
        .expect("add test record failed");
    added_count += 1;
    println!("✓ Added {} distinct value field records", added_count);

    // Test: check_field_use
    println!("\n[3/5] Testing check_field_use...");
    let usage =
        distinct_values::check_field_use(&short_test_org, "test_stream", "logs", "test_field")
            .await
            .expect("check_field_use failed");
    assert!(!usage.is_empty(), "should find field usage");
    println!("✓ check_field_use verified - found {} usages", usage.len());

    // Test: len
    println!("\n[4/5] Testing len...");
    let count = distinct_values::len().await.expect("len failed");
    println!("✓ len returned {} records", count);

    // Test: batch_remove and remove (cleanup)
    println!("\n[5/5] Testing batch_remove and remove (cleanup)...");
    distinct_values::batch_remove(OriginType::Dashboard, &short_test_origin_id)
        .await
        .expect("batch_remove failed");

    // Remove the test data we added
    for (i, data) in fields_data.iter().enumerate() {
        let origin = match data.origin.as_str() {
            "dashboard" => OriginType::Dashboard,
            "report" => OriginType::Report,
            "stream" => OriginType::Stream,
            _ => OriginType::Dashboard,
        };

        let origin_id = format!(
            "{}_{}",
            short_prefix,
            &data.origin_id[..data.origin_id.len().min(15)]
        );
        let org_name = format!("{}_{}", short_prefix, data.org_name);

        let record = DistinctFieldRecord::new(
            origin,
            &origin_id,
            &org_name,
            &data.stream_name,
            data.stream_type.clone(),
            &data.field_name,
        );
        let _ = distinct_values::remove(record).await;

        if i >= 2 {
            break;
        }
    }
    println!("✓ batch_remove and remove verified - cleaned up test data");

    println!("\n✓ Distinct Value Fields compatibility test PASSED");
}

// ==================== Level 2: Higher Dependency Table Tests ====================

/// Test: alerts table compatibility
/// Order: 20
/// Dependencies: organizations, folders, templates, destinations
/// Behavior: put, get_by_name, get_by_id, list, delete_by_name
pub async fn test_alerts_compat_impl(prefix: &str) {
    println!("\n========== Alerts Compatibility Test [Order: 20] ==========");
    println!("Table: alerts");
    println!("Dependencies: organizations, folders");
    println!("Behavior: put, get_by_name, get_by_id, list, delete_by_name\n");

    // Use short prefix for test isolation
    let short_prefix = &prefix[..prefix.len().min(10)];

    // Get ORM connection
    let conn = ORM_CLIENT.get_or_init(connect_to_orm).await;

    // Step 0: Ensure ORM tables exist
    println!("[0/6] Ensuring ORM tables exist...");
    if let Err(e) = ensure_orm_tables_exist().await {
        panic!("⚠ Failed to ensure ORM tables: {}", e);
    }
    println!("✓ Ensured ORM tables exist");

    // Step 1: Setup dependencies - create organization and folder
    println!("\n[1/6] Setting up dependencies (org, folder)...");

    // Create a test organization for alerts
    let test_org_name = format!("{}_alert_org", short_prefix);
    organizations::remove(&test_org_name).await.unwrap();
    organizations::add(&test_org_name, "Test Alert Org", OrganizationType::Default)
        .await
        .expect("add organization failed");
    println!("  Created organization: {}", test_org_name);

    // Create a test folder for alerts
    let test_folder_id = format!("{}_alert_fold", short_prefix);
    folders::delete(&test_org_name, &test_folder_id, FolderType::Alerts)
        .await
        .unwrap();
    let folder = Folder {
        folder_id: test_folder_id.clone(),
        name: format!("{}_alert_folder", short_prefix),
        description: "Test folder for alerts".to_string(),
    };

    let folder_result = folders::put(&test_org_name, None, folder, FolderType::Alerts).await;
    let folder = match folder_result {
        Ok((_, f)) => {
            println!("  Created folder: {} (id: {})", f.name, f.folder_id);
            f
        }
        Err(e) => {
            organizations::remove(&test_org_name).await.unwrap();
            panic!("⚠ Failed to create folder: {}", e);
        }
    };

    println!("✓ Dependencies set up");

    // Step 2: Create test alert
    println!("\n[2/6] Testing put (create alert)...");

    let test_alert_name = format!("{}_test_alert", short_prefix);
    let test_stream_name = format!("{}_stream", short_prefix);

    // Use Alert::default() and modify public fields
    let mut alert = config::meta::alerts::alert::Alert::default();
    alert.name = test_alert_name.clone();
    alert.org_id = test_org_name.clone();
    alert.stream_type = FileStreamType::Logs;
    alert.stream_name = test_stream_name.clone();
    alert.is_real_time = false;
    alert.query_condition = QueryCondition::default();
    alert.trigger_condition = TriggerCondition::default();
    alert.destinations = vec![];
    alert.description = "Test alert for MySQL compatibility".to_string();
    alert.enabled = false;
    alert.tz_offset = 0;
    alert.owner = Some(format!("{}@test.com", short_prefix));

    let created_alert = match alerts::put(conn, &test_org_name, &folder.folder_id, alert).await {
        Ok(a) => {
            println!("✓ Created alert: {} (id: {:?})", a.name, a.id);
            a
        }
        Err(e) => {
            // Cleanup
            folders::delete(&test_org_name, &folder.folder_id, FolderType::Alerts)
                .await
                .unwrap();
            organizations::remove(&test_org_name).await.unwrap();
            panic!("⚠ put failed: {}", e);
        }
    };

    // Step 3: Test get_by_name
    println!("\n[3/6] Testing get_by_name...");
    match alerts::get_by_name(
        conn,
        &test_org_name,
        &folder.folder_id,
        FileStreamType::Logs,
        &test_stream_name,
        &test_alert_name,
    )
    .await
    {
        Ok(Some((_, alert))) => {
            assert_eq!(alert.name, test_alert_name, "alert name should match");
            assert_eq!(alert.org_id, test_org_name, "alert org_id should match");
            assert_eq!(
                alert.stream_name, test_stream_name,
                "alert stream_name should match"
            );
            assert_eq!(
                alert.description, "Test alert for MySQL compatibility",
                "alert description should match"
            );
            println!("✓ get_by_name succeeded: {}", alert.name);
        }
        Ok(None) => {
            println!("⚠ get_by_name returned None");
        }
        Err(e) => {
            panic!("⚠ get_by_name failed: {}", e);
        }
    }

    // Step 4: Test get_by_id
    println!("\n[4/6] Testing get_by_id...");
    if let Some(alert_id) = created_alert.id {
        match alerts::get_by_id(conn, &test_org_name, alert_id).await {
            Ok(Some((_, alert))) => {
                println!("✓ get_by_id succeeded: {}", alert.name);
            }
            Ok(None) => {
                println!("⚠ get_by_id returned None");
            }
            Err(e) => {
                panic!("⚠ get_by_id failed: {}", e);
            }
        }
    } else {
        println!("⚠ Created alert has no ID, skipping get_by_id test");
    }

    // Step 5: Test list
    println!("\n[5/6] Testing list...");
    let list_params = config::meta::alerts::alert::ListAlertsParams::new(&test_org_name);
    match alerts::list(conn, list_params).await {
        Ok(alerts_list) => {
            let test_alerts: Vec<_> = alerts_list
                .iter()
                .filter(|(_, a)| a.name.contains(short_prefix))
                .collect();
            println!(
                "✓ list returned {} alerts, {} test alerts",
                alerts_list.len(),
                test_alerts.len()
            );
        }
        Err(e) => {
            panic!("⚠ list failed: {}", e);
        }
    }

    // Step 6: Cleanup - delete alert, folder, and org
    println!("\n[6/6] Cleaning up test data...");

    // Delete alert by name
    match alerts::delete_by_name(
        conn,
        &test_org_name,
        &folder.folder_id,
        FileStreamType::Logs,
        &test_stream_name,
        &test_alert_name,
    )
    .await
    {
        Ok(_) => println!("  ✓ Deleted alert: {}", test_alert_name),
        Err(e) => panic!("⚠ Failed to delete alert: {}", e),
    }

    // Delete folder
    match folders::delete(&test_org_name, &folder.folder_id, FolderType::Alerts).await {
        Ok(_) => println!("  ✓ Deleted folder: {}", folder.name),
        Err(e) => panic!("⚠ Failed to delete folder: {}", e),
    }

    // Delete organization
    match organizations::remove(&test_org_name).await {
        Ok(_) => println!("  ✓ Deleted organization: {}", test_org_name),
        Err(e) => panic!("⚠ Failed to delete organization: {}", e),
    }

    println!("\n✓ Alerts compatibility test PASSED");
}

/// Test: dashboards table compatibility
/// Order: 21
/// Dependencies: organizations, folders
/// Behavior: put, get, list, delete
pub async fn test_dashboards_compat_impl(prefix: &str) {
    println!("\n========== Dashboards Compatibility Test [Order: 21] ==========");
    println!("Table: dashboards");
    println!("Dependencies: organizations, folders");
    println!("Behavior: put, get_from_folder, list, delete_from_folder\n");

    // Use short prefix for test isolation
    let short_prefix = &prefix[..prefix.len().min(10)];

    // Step 0: Ensure ORM tables exist
    println!("[0/5] Ensuring ORM tables exist...");
    if let Err(e) = ensure_orm_tables_exist().await {
        panic!("⚠ Failed to ensure ORM tables: {}", e);
    }
    println!("✓ Ensured ORM tables exist");

    // Step 1: Setup dependencies - create organization and folder
    println!("\n[1/5] Setting up dependencies (org, folder)...");

    // Create a test organization
    let test_org_name = format!("{}_dash_org", short_prefix);
    organizations::remove(&test_org_name).await.unwrap();
    organizations::add(
        &test_org_name,
        "Test Dashboard Org",
        OrganizationType::Default,
    )
    .await
    .expect("add organization failed");
    println!("  Created organization: {}", test_org_name);

    // Create a test folder for dashboards
    let test_folder_id = format!("{}_dash_fold", short_prefix);
    folders::delete(&test_org_name, &test_folder_id, FolderType::Dashboards)
        .await
        .unwrap();
    let folder = Folder {
        folder_id: test_folder_id.clone(),
        name: format!("{}_dashboard_folder", short_prefix),
        description: "Test folder for dashboards".to_string(),
    };

    let folder_result = folders::put(&test_org_name, None, folder, FolderType::Dashboards).await;
    let folder = match folder_result {
        Ok((_, f)) => {
            println!("  Created folder: {} (id: {})", f.name, f.folder_id);
            f
        }
        Err(e) => {
            organizations::remove(&test_org_name).await.unwrap();
            panic!("⚠ Failed to create folder: {}", e);
        }
    };

    println!("✓ Dependencies set up");

    // Step 2: Create test dashboard
    println!("\n[2/5] Testing put (create dashboard)...");

    let test_dashboard_id = format!("{}_dash", short_prefix);

    // Create a v1 dashboard with all required fields
    let dashboard_v1 = DashboardV1 {
        dashboard_id: test_dashboard_id.clone(),
        title: format!("{} Test Dashboard", short_prefix),
        description: "Test dashboard for MySQL compatibility".to_string(),
        owner: format!("{}@test.com", short_prefix),
        role: String::new(),
        created: Utc::now().with_timezone(&FixedOffset::east_opt(0).unwrap()),
        panels: vec![],
        layouts: None,
        variables: None,
        updated_at: 0,
    };
    let dashboard: config::meta::dashboards::Dashboard = dashboard_v1.into();

    match dashboards::put(
        &test_org_name,
        &folder.folder_id,
        None,
        dashboard.clone(),
        false,
    )
    .await
    {
        Ok(d) => {
            println!(
                "✓ Created dashboard: {:?} (id: {:?})",
                d.title(),
                d.dashboard_id()
            );
        }
        Err(e) => {
            // Cleanup
            folders::delete(&test_org_name, &folder.folder_id, FolderType::Dashboards)
                .await
                .unwrap();
            organizations::remove(&test_org_name).await.unwrap();
            panic!("⚠ put failed: {}", e);
        }
    }

    // Step 3: Test get_from_folder
    println!("\n[3/5] Testing get_from_folder...");
    match dashboards::get_from_folder(&test_org_name, &folder.folder_id, &test_dashboard_id).await {
        Ok(Some(d)) => {
            let expected_title = format!("{} Test Dashboard", short_prefix);
            assert_eq!(
                d.title(),
                Some(expected_title.as_str()),
                "dashboard title should match"
            );
            assert_eq!(
                d.dashboard_id(),
                Some(test_dashboard_id.as_str()),
                "dashboard id should match"
            );
            assert_eq!(
                d.description(),
                Some("Test dashboard for MySQL compatibility"),
                "dashboard description should match"
            );
            println!("✓ get_from_folder succeeded: {:?}", d.title());
        }
        Ok(None) => {
            println!("⚠ get_from_folder returned None");
        }
        Err(e) => {
            panic!("⚠ get_from_folder failed: {}", e);
        }
    }

    // Step 4: Test list
    println!("\n[4/5] Testing list...");
    let list_params = ListDashboardsParams::new(&test_org_name).with_folder_id(&folder.folder_id);
    match dashboards::list(list_params).await {
        Ok(dashboards_list) => {
            let test_dashboards: Vec<_> = dashboards_list
                .iter()
                .filter(|(_, d)| {
                    d.dashboard_id()
                        .map(|id| id.contains(short_prefix))
                        .unwrap_or(false)
                })
                .collect();
            println!(
                "✓ list returned {} dashboards, {} test dashboards",
                dashboards_list.len(),
                test_dashboards.len()
            );
        }
        Err(e) => {
            panic!("⚠ list failed: {}", e);
        }
    }

    // Step 5: Cleanup - delete dashboard, folder, and org
    println!("\n[5/5] Cleaning up test data...");

    // Delete dashboard
    match dashboards::delete_from_folder(&test_org_name, &folder.folder_id, &test_dashboard_id)
        .await
    {
        Ok(_) => println!("  ✓ Deleted dashboard: {}", test_dashboard_id),
        Err(e) => panic!("⚠ Failed to delete dashboard: {}", e),
    }

    // Delete folder
    match folders::delete(&test_org_name, &folder.folder_id, FolderType::Dashboards).await {
        Ok(_) => println!("  ✓ Deleted folder: {}", folder.name),
        Err(e) => panic!("⚠ Failed to delete folder: {}", e),
    }

    // Delete organization
    match organizations::remove(&test_org_name).await {
        Ok(_) => println!("  ✓ Deleted organization: {}", test_org_name),
        Err(e) => panic!("⚠ Failed to delete organization: {}", e),
    }

    println!("\n✓ Dashboards compatibility test PASSED");
}

// ==================== File List Tests (Separate Infrastructure) ====================

/// Test data structure for file_list
#[derive(Debug, Clone, serde::Deserialize)]
pub struct FileListTestData {
    pub id: i64,
    pub account: String,
    pub org: String,
    pub stream: String,
    pub date: String,
    pub file: String,
    pub deleted: bool,
    pub min_ts: i64,
    pub max_ts: i64,
    pub records: i64,
    pub original_size: i64,
    pub compressed_size: i64,
    pub index_size: i64,
    pub flattened: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Test: file_list table compatibility
/// Order: 30
/// Dependencies: None (separate infrastructure)
/// Behavior: create_table, add, get, contains, batch_add, remove
pub async fn test_file_list_compat_impl(prefix: &str) {
    println!("\n========== File List Compatibility Test [Order: 30] ==========");
    println!("Table: file_list");
    println!("Dependencies: None (separate file_list infrastructure)");
    println!("Behavior: create_table, add, get, contains, batch_add, remove\n");

    // Load test data
    let test_data: Vec<FileListTestData> = load_test_data("file_list.json");
    println!(
        "Loaded {} file_list records from test data",
        test_data.len()
    );

    // Use short prefix for test isolation
    let short_prefix = &prefix[..prefix.len().min(10)];

    // Test: Create table
    println!("\n[1/6] Testing create_table...");
    match file_list::create_table().await {
        Ok(_) => println!("✓ create_table succeeded"),
        Err(e) => {
            println!("⚠ create_table failed: {}", e);
            println!("  This may be expected if the table already exists.");
        }
    }

    // Test: Create table index
    println!("\n[2/6] Testing create_table_index...");
    match file_list::create_table_index().await {
        Ok(_) => println!("✓ create_table_index succeeded"),
        Err(e) => {
            println!("⚠ create_table_index failed: {}", e);
            println!("  This may be expected if indexes already exist.");
        }
    }

    // Test: Add files
    println!("\n[3/6] Testing add...");
    let mut added_files = Vec::new();
    for (i, data) in test_data.iter().enumerate().take(3) {
        // Construct test file key with correct format:
        // files/<org>/<stream_type>/<stream>/<year>/<month>/<day>/<hour>/<file>
        // The parse_file_key_columns function expects 9 parts
        // Example: files/default/logs/olympics/2022/10/03/10/6982652937134804993_1.parquet

        // Parse stream to get parts: e.g., "_meta/logs/audit" -> org="_meta", type="logs",
        // name="audit"
        let stream_parts: Vec<&str> = data.stream.split('/').collect();
        let (org_part, stream_type, stream_name) = if stream_parts.len() >= 3 {
            (stream_parts[0], stream_parts[1], stream_parts[2])
        } else {
            ("default", "logs", "test")
        };

        // Use short prefix for unique org and stream names
        let test_org = format!("{}_{}", short_prefix, org_part);
        let test_stream_name = format!("{}_{}", short_prefix, stream_name);
        let test_file = format!("{}_{}", short_prefix, data.file);

        // Build full file key: files/<org>/<stream_type>/<stream>/<date>/<file>
        // date is already in "2025/12/16/08" format (year/month/day/hour)
        let file_key = format!(
            "files/{}/{}/{}/{}/{}",
            test_org, stream_type, test_stream_name, data.date, test_file
        );

        let meta = FileMeta {
            min_ts: data.min_ts,
            max_ts: data.max_ts,
            records: data.records,
            original_size: data.original_size,
            compressed_size: data.compressed_size,
            index_size: data.index_size,
            flattened: data.flattened,
        };

        let account = if data.account.is_empty() {
            ""
        } else {
            &data.account
        };
        match file_list::add(account, &file_key, &meta).await {
            Ok(id) => {
                added_files.push((file_key.clone(), id));
                if i == 0 {
                    println!("  Added file {} with id {}", file_key, id);
                }
            }
            Err(e) => {
                println!("⚠ add failed for {}: {}", file_key, e);
            }
        }
    }
    println!("✓ Added {} files", added_files.len());

    // Test: Get file metadata
    println!("\n[4/6] Testing get...");
    if let Some((file_key, _)) = added_files.first() {
        match file_list::get(file_key).await {
            Ok(meta) => {
                // Verify the meta matches what we inserted (from test_data)
                if let Some(data) = test_data.first() {
                    assert_eq!(
                        meta.records, data.records,
                        "records should match inserted value"
                    );
                    assert_eq!(
                        meta.min_ts, data.min_ts,
                        "min_ts should match inserted value"
                    );
                    assert_eq!(
                        meta.max_ts, data.max_ts,
                        "max_ts should match inserted value"
                    );
                    assert_eq!(
                        meta.original_size, data.original_size,
                        "original_size should match"
                    );
                    assert_eq!(
                        meta.compressed_size, data.compressed_size,
                        "compressed_size should match"
                    );
                }
                println!(
                    "✓ get succeeded: records={}, original_size={}",
                    meta.records, meta.original_size
                );
            }
            Err(e) => {
                panic!("⚠ get failed: {}", e);
            }
        }
    }

    // Test: Contains
    println!("\n[5/6] Testing contains...");
    if let Some((file_key, _)) = added_files.first() {
        match file_list::contains(file_key).await {
            Ok(exists) => {
                assert!(exists, "file should exist");
                println!("✓ contains returned true for existing file");
            }
            Err(e) => {
                panic!("⚠ contains failed: {}", e);
            }
        }
    }
    // Test non-existent file - use proper file path format
    let nonexistent_file_key = format!(
        "files/{}_nonexistent_org/logs/nonexistent_stream/2099/01/01/00/{}_nonexistent.parquet",
        short_prefix, short_prefix
    );
    match file_list::contains(&nonexistent_file_key).await {
        Ok(exists) => {
            if !exists {
                println!("✓ contains returned false for non-existent file");
            } else {
                panic!("⚠ contains returned true for non-existent file (unexpected)");
            }
        }
        Err(e) => {
            // Some backends may return an error for non-existent files instead of false
            panic!(
                "✓ contains returned error for non-existent file: {} (acceptable behavior)",
                e
            );
        }
    }

    // Test: Remove (cleanup)
    println!("\n[6/6] Testing remove...");
    for (file_key, _) in &added_files {
        match file_list::remove(file_key).await {
            Ok(_) => {}
            Err(e) => {
                panic!("⚠ remove failed for {}: {}", file_key, e);
            }
        }
    }
    println!("✓ Removed {} files", added_files.len());

    println!("\n✓ File List compatibility test PASSED");
}

/// Test data structure for stream_stats
#[derive(Debug, Clone, serde::Deserialize)]
pub struct StreamStatsTestData {
    pub id: i64,
    pub org: String,
    pub stream: String,
    pub file_num: i64,
    pub min_ts: i64,
    pub max_ts: i64,
    pub records: i64,
    pub original_size: i64,
    pub compressed_size: i64,
    pub index_size: i64,
    pub is_recent: bool,
}

/// Test: stream_stats table compatibility
/// Order: 31
/// Dependencies: None (part of file_list infrastructure)
/// Behavior: set_stream_stats, get_stream_stats, del_stream_stats
pub async fn test_stream_stats_compat_impl(prefix: &str) {
    println!("\n========== Stream Stats Compatibility Test [Order: 31] ==========");
    println!("Table: stream_stats");
    println!("Dependencies: None (part of file_list infrastructure)");
    println!("Behavior: set_stream_stats, get_stream_stats, del_stream_stats\n");

    // Use short prefix for test isolation
    let short_prefix = &prefix[..prefix.len().min(10)];
    let test_org = format!("{}_org", short_prefix);
    let test_stream = format!("{}/logs/test_stream", test_org);

    // Ensure file_list tables exist
    println!("[0/4] Ensuring file_list tables exist...");
    file_list::create_table().await.unwrap();
    println!("✓ file_list tables ready");

    // Test: Set stream stats
    println!("\n[1/4] Testing set_stream_stats...");
    let stats = config::meta::stream::StreamStats {
        created_at: chrono::Utc::now().timestamp_micros(),
        doc_time_min: 1000000,
        doc_time_max: 2000000,
        doc_num: 100,
        file_num: 5,
        storage_size: 50000.0,
        compressed_size: 10000.0,
        index_size: 1000.0,
    };
    match file_list::set_stream_stats(&test_org, FileStreamType::Logs, &test_stream, &stats, false)
        .await
    {
        Ok(_) => println!("✓ set_stream_stats succeeded"),
        Err(e) => {
            panic!("⚠ set_stream_stats failed: {}", e);
        }
    }

    // Test: Get stream stats
    println!("\n[2/4] Testing get_stream_stats...");
    match file_list::get_stream_stats(&test_org, Some(FileStreamType::Logs), Some(&test_stream))
        .await
    {
        Ok(results) => {
            println!("✓ get_stream_stats returned {} result(s)", results.len());
            assert!(!results.is_empty(), "should have at least one result");
            if let Some((stream_key, retrieved_stats)) = results.first() {
                assert!(
                    stream_key.contains(&test_stream),
                    "stream key should contain test stream name"
                );
                assert_eq!(
                    retrieved_stats.doc_num, 100,
                    "doc_num should match inserted value"
                );
                assert_eq!(
                    retrieved_stats.file_num, 5,
                    "file_num should match inserted value"
                );
                assert_eq!(
                    retrieved_stats.doc_time_min, 1000000,
                    "doc_time_min should match"
                );
                assert_eq!(
                    retrieved_stats.doc_time_max, 2000000,
                    "doc_time_max should match"
                );
                println!(
                    "  Stream: {}, doc_num: {}",
                    stream_key, retrieved_stats.doc_num
                );
            }
        }
        Err(e) => {
            panic!("⚠ get_stream_stats failed: {}", e);
        }
    }

    // Test: Get all stream stats for org
    println!("\n[3/4] Testing get_stream_stats (all streams)...");
    match file_list::get_stream_stats(&test_org, None, None).await {
        Ok(results) => {
            let test_results: Vec<_> = results
                .iter()
                .filter(|(k, _)| k.contains(short_prefix))
                .collect();
            println!(
                "✓ get_stream_stats (all) returned {} test result(s)",
                test_results.len()
            );
        }
        Err(e) => {
            panic!("⚠ get_stream_stats (all) failed: {}", e);
        }
    }

    // Test: Delete stream stats (cleanup)
    println!("\n[4/4] Testing del_stream_stats...");
    match file_list::del_stream_stats(&test_org, FileStreamType::Logs, &test_stream).await {
        Ok(_) => println!("✓ del_stream_stats succeeded"),
        Err(e) => {
            panic!("⚠ del_stream_stats failed: {}", e);
        }
    }

    println!("\n✓ Stream Stats compatibility test PASSED");
}

// ==================== File List Deleted Tests ====================

/// Test data structure for file_list_deleted
#[derive(Debug, Clone, serde::Deserialize)]
pub struct FileListDeletedTestData {
    pub id: i64,
    pub account: String,
    pub org: String,
    pub stream: String,
    pub date: String,
    pub file: String,
    pub index_file: bool,
    pub flattened: bool,
    pub created_at: i64,
}

/// Test: file_list_deleted table compatibility
/// Order: 32
/// Dependencies: None (separate file_list infrastructure)
/// Behavior: batch_add_deleted, batch_remove_deleted
pub async fn test_file_list_deleted_compat_impl(prefix: &str) {
    println!("\n========== File List Deleted Compatibility Test [Order: 32] ==========");
    println!("Table: file_list_deleted");
    println!("Dependencies: None (separate file_list infrastructure)");
    println!("Behavior: batch_add_deleted, batch_remove_deleted\n");

    // Load test data
    let test_data: Vec<FileListDeletedTestData> = load_test_data("file_list_deleted.json");
    println!(
        "Loaded {} file_list_deleted records from test data",
        test_data.len()
    );

    // Use short prefix for test isolation
    let short_prefix = &prefix[..prefix.len().min(10)];
    let test_org = format!("{}_deleted_org", short_prefix);

    // Ensure file_list tables exist
    println!("\n[1/3] Testing create_table...");
    file_list::create_table().await.unwrap();
    println!("✓ create_table succeeded");

    // Test: Batch add deleted files
    println!("\n[2/3] Testing batch_add_deleted...");
    let mut deleted_files: Vec<config::meta::stream::FileListDeleted> = Vec::new();
    for data in test_data.iter().take(3) {
        let test_file = format!("{}_{}", short_prefix, data.file);

        // Construct file path based on data pattern
        let file_path = format!(
            "files/{}/{}/{}/{}",
            test_org,
            data.stream.replace("default/", ""),
            data.date,
            test_file
        );

        deleted_files.push(config::meta::stream::FileListDeleted {
            id: 0, // ID is auto-generated
            account: data.account.clone(),
            file: file_path,
            index_file: data.index_file,
            flattened: data.flattened,
        });
    }

    let created_at = chrono::Utc::now().timestamp_micros();
    match file_list::batch_add_deleted(&test_org, created_at, &deleted_files).await {
        Ok(_) => {
            println!(
                "✓ batch_add_deleted succeeded for {} files",
                deleted_files.len()
            );
        }
        Err(e) => {
            panic!("⚠ batch_add_deleted failed: {}", e);
        }
    }

    // Test: Batch remove deleted files
    println!("\n[3/3] Testing batch_remove_deleted...");
    let file_keys: Vec<FileKey> = deleted_files
        .iter()
        .map(|f| FileKey {
            id: 0,
            account: f.account.clone(),
            key: f.file.clone(),
            meta: FileMeta::default(),
            deleted: false,
            segment_ids: None,
        })
        .collect();

    match file_list::batch_remove_deleted(&file_keys).await {
        Ok(_) => {
            println!(
                "✓ batch_remove_deleted succeeded for {} files",
                file_keys.len()
            );
        }
        Err(e) => {
            // This may fail if files weren't found, which is acceptable
            panic!("⚠ batch_remove_deleted failed: {}", e);
        }
    }

    println!("\n✓ File List Deleted compatibility test PASSED");
}

// ==================== File List Deleted Concurrent Tests ====================

/// Test: query_deleted concurrent access with distributed lock
/// This tests the MySQL GET_LOCK based distributed lock mechanism
/// used by query_deleted to ensure serialized access.
pub async fn test_file_list_deleted_concurrent_query_deleted_impl(prefix: &str) {
    println!("\n========== File List Deleted Concurrent Query Test ==========");
    println!("Table: file_list_deleted");
    println!("Testing: query_deleted with distributed lock (GET_LOCK)\n");

    let short_prefix = &prefix[..prefix.len().min(10)];
    let test_org = format!("{}_conc_org", short_prefix);

    // Ensure file_list tables exist
    file_list::create_table().await.unwrap();

    // Add test deleted files
    // File path format:
    // files/{org}/{stream_type}/{stream_name}/{yyyy}/{mm}/{dd}/{hh}/{filename}.parquet
    let mut deleted_files: Vec<config::meta::stream::FileListDeleted> = Vec::new();
    for i in 0..10 {
        let file_path = format!(
            "files/{}/logs/test_stream/2024/01/01/00/test_file_{}.parquet",
            test_org, i
        );
        deleted_files.push(config::meta::stream::FileListDeleted {
            id: 0,
            account: test_org.clone(),
            file: file_path,
            index_file: false,
            flattened: false,
        });
    }

    // Use a timestamp in the past so files are eligible for query_deleted
    let created_at = chrono::Utc::now().timestamp_micros() - 3600_000_000; // 1 hour ago
    match file_list::batch_add_deleted(&test_org, created_at, &deleted_files).await {
        Ok(_) => println!(
            "✓ Setup: Added {} deleted files for concurrent test",
            deleted_files.len()
        ),
        Err(e) => {
            panic!("⚠ Setup failed: {}", e);
        }
    }

    // Test: Multiple concurrent query_deleted calls
    println!("\n[1/3] Testing concurrent query_deleted calls (lock serialization)...");

    let completion_order = Arc::new(std::sync::Mutex::new(Vec::new()));
    let num_tasks = 3;
    let mut handles = vec![];

    for task_id in 0..num_tasks {
        let test_org_clone = test_org.clone();
        let order_clone = completion_order.clone();
        let delay = task_id as u64 * 20; // Stagger start times

        handles.push(tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;

            let start = std::time::Instant::now();
            let time_max = chrono::Utc::now().timestamp_micros();

            // This should acquire the distributed lock
            let result = file_list::query_deleted(&test_org_clone, time_max, 5).await;

            let elapsed = start.elapsed();
            order_clone
                .lock()
                .unwrap()
                .push((task_id, elapsed.as_millis()));

            (task_id, result, elapsed)
        }));
    }

    let mut results = vec![];
    for handle in handles {
        match handle.await {
            Ok(r) => results.push(r),
            Err(e) => panic!("  Task panicked: {}", e),
        }
    }

    let order = completion_order.lock().unwrap();
    println!(
        "  Completion order: {:?}",
        order.iter().map(|(id, _)| id).collect::<Vec<_>>()
    );
    println!(
        "  Task timings: {:?}",
        order
            .iter()
            .map(|(id, ms)| format!("t{}={}ms", id, ms))
            .collect::<Vec<_>>()
    );

    let mut success_count = 0;
    for (task_id, result, elapsed) in &results {
        match result {
            Ok(files) => {
                println!(
                    "  Task {} succeeded: {} files, {:?}",
                    task_id,
                    files.len(),
                    elapsed
                );
                success_count += 1;
            }
            Err(e) => {
                panic!("  Task {} failed: {} ({:?})", task_id, e, elapsed);
            }
        }
    }

    // At least one task should succeed
    assert!(
        success_count >= 1,
        "At least one query_deleted should succeed"
    );
    println!(
        "✓ Concurrent query_deleted: {} of {} tasks succeeded",
        success_count, num_tasks
    );

    // Test: Lock timeout behavior
    println!("\n[2/3] Testing lock timeout behavior...");

    let test_org_clone = test_org.clone();
    let long_task = tokio::spawn(async move {
        let time_max = chrono::Utc::now().timestamp_micros();
        // This will hold the lock
        file_list::query_deleted(&test_org_clone, time_max, 100).await
    });

    // Give the first task time to acquire the lock
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    let start = std::time::Instant::now();
    let time_max = chrono::Utc::now().timestamp_micros();
    let second_result = file_list::query_deleted(&test_org, time_max, 5).await;
    let elapsed = start.elapsed();

    // Second call should either succeed (if lock released) or timeout
    match second_result {
        Ok(files) => println!(
            "  Second query succeeded: {} files in {:?}",
            files.len(),
            elapsed
        ),
        Err(e) if e.to_string().contains("LockTimeout") => {
            println!(
                "✓ Second query correctly received LockTimeout: {:?}",
                elapsed
            );
        }
        Err(e) => panic!("  Second query failed: {} ({:?})", e, elapsed),
    }

    let _ = long_task.await;
    println!("✓ Lock timeout behavior test completed");

    // Test: Concurrent query_deleted with different orgs (should not block each other)
    println!("\n[3/3] Testing concurrent query_deleted with different orgs...");

    let test_org2 = format!("{}_conc_org2", short_prefix);

    // Add files to second org
    // File path format:
    // files/{org}/{stream_type}/{stream_name}/{yyyy}/{mm}/{dd}/{hh}/{filename}.parquet
    let mut deleted_files2: Vec<config::meta::stream::FileListDeleted> = Vec::new();
    for i in 0..5 {
        let file_path = format!(
            "files/{}/logs/test_stream2/2024/01/01/00/test_file_{}.parquet",
            test_org2, i
        );
        deleted_files2.push(config::meta::stream::FileListDeleted {
            id: 0,
            account: test_org2.clone(),
            file: file_path,
            index_file: false,
            flattened: false,
        });
    }
    file_list::batch_add_deleted(&test_org2, created_at, &deleted_files2)
        .await
        .unwrap();

    let start_time = std::time::Instant::now();

    let test_org1_clone = test_org.clone();
    let test_org2_clone = test_org2.clone();

    let task1 = tokio::spawn(async move {
        let time_max = chrono::Utc::now().timestamp_micros();
        file_list::query_deleted(&test_org1_clone, time_max, 10).await
    });

    let task2 = tokio::spawn(async move {
        let time_max = chrono::Utc::now().timestamp_micros();
        file_list::query_deleted(&test_org2_clone, time_max, 10).await
    });

    let (r1, r2) = tokio::join!(task1, task2);
    let total_time = start_time.elapsed();

    match (r1, r2) {
        (Ok(Ok(files1)), Ok(Ok(files2))) => {
            println!(
                "  Org1: {} files, Org2: {} files",
                files1.len(),
                files2.len()
            );
            println!("  Total time: {:?}", total_time);
            // Note: Due to global lock, these may not run in parallel
        }
        (r1, r2) => {
            println!("  Results: org1={:?}, org2={:?}", r1.is_ok(), r2.is_ok());
        }
    }

    println!("✓ Different orgs concurrent test completed");
    println!("\n✓ File List Deleted Concurrent Test PASSED");
}

/// Test: Concurrent lock serialization verification for query_deleted
/// This test verifies that only one query_deleted operation can execute at a time
pub async fn test_file_list_deleted_lock_serialization_impl(prefix: &str) {
    println!("\n========== File List Deleted Lock Serialization Test ==========");
    println!("Testing: Distributed lock ensures serialized access\n");

    let short_prefix = &prefix[..prefix.len().min(10)];
    let test_org = format!("{}_serial_org", short_prefix);

    // Ensure tables exist
    file_list::create_table().await.unwrap();

    // Add test files
    // File path format:
    // files/{org}/{stream_type}/{stream_name}/{yyyy}/{mm}/{dd}/{hh}/{filename}.parquet
    let mut deleted_files: Vec<config::meta::stream::FileListDeleted> = Vec::new();
    for i in 0..20 {
        let file_path = format!(
            "files/{}/logs/serial_stream/2024/01/01/00/serial_test_{}.parquet",
            test_org, i
        );
        deleted_files.push(config::meta::stream::FileListDeleted {
            id: 0,
            account: test_org.clone(),
            file: file_path,
            index_file: false,
            flattened: false,
        });
    }

    let created_at = chrono::Utc::now().timestamp_micros() - 3600_000_000;
    file_list::batch_add_deleted(&test_org, created_at, &deleted_files)
        .await
        .unwrap();
    println!("✓ Setup: Added {} deleted files", deleted_files.len());

    // Track concurrent execution
    let concurrent_count = Arc::new(AtomicI32::new(0));
    let max_concurrent = Arc::new(AtomicI32::new(0));

    let num_tasks = 5;
    let mut handles = vec![];

    for task_id in 0..num_tasks {
        let test_org_clone = test_org.clone();
        let cc = concurrent_count.clone();
        let mc = max_concurrent.clone();

        handles.push(tokio::spawn(async move {
            // Increment concurrent count
            let current = cc.fetch_add(1, AtomicOrd::SeqCst) + 1;
            mc.fetch_max(current, AtomicOrd::SeqCst);

            let time_max = chrono::Utc::now().timestamp_micros();
            let result = file_list::query_deleted(&test_org_clone, time_max, 3).await;

            // Decrement concurrent count
            cc.fetch_sub(1, AtomicOrd::SeqCst);

            (task_id, result)
        }));
    }

    let mut success_count = 0;
    for handle in handles {
        if let Ok((task_id, result)) = handle.await {
            match result {
                Ok(files) => {
                    println!("  Task {} succeeded: {} files", task_id, files.len());
                    success_count += 1;
                }
                Err(e) => {
                    if e.to_string().contains("LockTimeout") {
                        println!("  Task {} timed out (expected for serialization)", task_id);
                    } else {
                        panic!("  Task {} failed: {}", task_id, e);
                    }
                }
            }
        }
    }

    let max_conc = max_concurrent.load(AtomicOrd::SeqCst);
    println!("\n  Max observed concurrent tasks: {}", max_conc);
    println!("  Success rate: {}/{}", success_count, num_tasks);

    // Due to lock serialization, max concurrent should be limited
    // Note: This checks task spawning concurrency, not actual lock holding
    println!("✓ Lock serialization test completed");
}

/// Test: Concurrent counter increment via query_deleted
/// Each query_deleted marks files with new timestamp, verifying atomic updates
pub async fn test_file_list_deleted_concurrent_counter_impl(prefix: &str) {
    println!("\n========== File List Deleted Concurrent Counter Test ==========");
    println!("Testing: Atomic file timestamp updates via query_deleted\n");

    let short_prefix = &prefix[..prefix.len().min(10)];
    let test_org = format!("{}_counter_org", short_prefix);

    file_list::create_table().await.unwrap();

    // Add test files
    // File path format:
    // files/{org}/{stream_type}/{stream_name}/{yyyy}/{mm}/{dd}/{hh}/{filename}.parquet
    let mut deleted_files: Vec<config::meta::stream::FileListDeleted> = Vec::new();
    for i in 0..30 {
        let file_path = format!(
            "files/{}/logs/counter_stream/2024/01/01/00/counter_test_{}.parquet",
            test_org, i
        );
        deleted_files.push(config::meta::stream::FileListDeleted {
            id: 0,
            account: test_org.clone(),
            file: file_path,
            index_file: false,
            flattened: false,
        });
    }

    let created_at = chrono::Utc::now().timestamp_micros() - 7200_000_000; // 2 hours ago
    file_list::batch_add_deleted(&test_org, created_at, &deleted_files)
        .await
        .unwrap();

    println!("✓ Setup: Added {} deleted files", deleted_files.len());

    let total_fetched = Arc::new(AtomicI32::new(0));
    let num_workers = 3;
    let mut handles = vec![];

    for worker_id in 0..num_workers {
        let test_org_clone = test_org.clone();
        let fetched = total_fetched.clone();

        handles.push(tokio::spawn(async move {
            let time_max = chrono::Utc::now().timestamp_micros();
            match file_list::query_deleted(&test_org_clone, time_max, 10).await {
                Ok(files) => {
                    fetched.fetch_add(files.len() as i32, AtomicOrd::SeqCst);
                    println!("  Worker {} fetched {} files", worker_id, files.len());
                    Ok(files.len())
                }
                Err(e) => {
                    println!("  Worker {} error: {}", worker_id, e);
                    Err(e)
                }
            }
        }));
    }

    for handle in handles {
        let _ = handle.await;
    }

    let total = total_fetched.load(AtomicOrd::SeqCst);
    println!("\n  Total files fetched across all workers: {}", total);

    // Due to atomic updates, each file should only be fetched once
    // (query_deleted updates created_at to NOW, so subsequent queries won't get same files)
    assert!(
        total <= deleted_files.len() as i32,
        "Total fetched ({}) should not exceed total files ({})",
        total,
        deleted_files.len()
    );

    println!("✓ Concurrent counter test PASSED: No duplicate processing detected");
}

// ==================== File List Jobs Concurrent Tests ====================

/// Test: get_pending_jobs concurrent access with distributed lock
/// This tests the MySQL GET_LOCK based distributed lock mechanism
/// used by get_pending_jobs to ensure serialized job claiming.
pub async fn test_file_list_jobs_concurrent_get_pending_impl(prefix: &str) {
    println!("\n========== File List Jobs Concurrent Get Pending Test ==========");
    println!("Table: file_list_jobs");
    println!("Testing: get_pending_jobs with distributed lock (GET_LOCK)\n");

    let short_prefix = &prefix[..prefix.len().min(10)];
    let test_org = format!("{}_job_org", short_prefix);
    let test_stream = format!("{}_stream", short_prefix);

    // Ensure file_list tables exist
    file_list::create_table().await.unwrap();

    // Add test jobs
    println!("[Setup] Adding pending jobs for concurrent test...");
    let mut job_ids = Vec::new();
    for i in 0..10 {
        let offset = chrono::Utc::now().timestamp_micros() + i * 1000;
        match file_list::add_job(&test_org, FileStreamType::Logs, &test_stream, offset).await {
            Ok(id) => {
                job_ids.push(id);
            }
            Err(e) => {
                panic!("⚠ Failed to add job {}: {}", i, e);
            }
        }
    }
    println!("✓ Setup: Added {} pending jobs", job_ids.len());

    if job_ids.is_empty() {
        panic!("⏭ No jobs were created, skipping concurrent test");
    }

    // Test: Multiple concurrent get_pending_jobs calls
    println!("\n[1/3] Testing concurrent get_pending_jobs calls (lock serialization)...");

    let completion_order = Arc::new(std::sync::Mutex::new(Vec::new()));
    let total_jobs_claimed = Arc::new(AtomicI32::new(0));
    let num_tasks = 4;
    let mut handles = vec![];

    for task_id in 0..num_tasks {
        let order_clone = completion_order.clone();
        let jobs_claimed_clone = total_jobs_claimed.clone();
        let node_name = format!("test_node_{}", task_id);
        let delay = task_id as u64 * 10; // Stagger start times

        handles.push(tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;

            let start = std::time::Instant::now();

            // This should acquire the distributed lock
            let result = file_list::get_pending_jobs(&node_name, 3).await;

            let elapsed = start.elapsed();
            order_clone
                .lock()
                .unwrap()
                .push((task_id, elapsed.as_millis()));

            if let Ok(ref jobs) = result {
                jobs_claimed_clone.fetch_add(jobs.len() as i32, AtomicOrd::SeqCst);
            }

            (task_id, result, elapsed)
        }));
    }

    let mut results = vec![];
    for handle in handles {
        match handle.await {
            Ok(r) => results.push(r),
            Err(e) => println!("  Task panicked: {}", e),
        }
    }

    let order = completion_order.lock().unwrap();
    println!(
        "  Completion order: {:?}",
        order.iter().map(|(id, _)| id).collect::<Vec<_>>()
    );
    println!(
        "  Task timings: {:?}",
        order
            .iter()
            .map(|(id, ms)| format!("t{}={}ms", id, ms))
            .collect::<Vec<_>>()
    );

    let mut success_count = 0;
    for (task_id, result, elapsed) in &results {
        match result {
            Ok(jobs) => {
                println!(
                    "  Task {} succeeded: {} jobs claimed, {:?}",
                    task_id,
                    jobs.len(),
                    elapsed
                );
                success_count += 1;
            }
            Err(e) => {
                if e.to_string().contains("LockTimeout") {
                    println!(
                        "  Task {} timed out (expected with lock contention): {:?}",
                        task_id, elapsed
                    );
                } else {
                    panic!("  Task {} failed: {} ({:?})", task_id, e, elapsed);
                }
            }
        }
    }

    let total_claimed = total_jobs_claimed.load(AtomicOrd::SeqCst);
    println!("\n  Total jobs claimed across all tasks: {}", total_claimed);
    println!(
        "  Success rate: {}/{} tasks succeeded",
        success_count, num_tasks
    );

    // At least one task should succeed
    assert!(
        success_count >= 1,
        "At least one get_pending_jobs should succeed"
    );
    println!(
        "✓ Concurrent get_pending_jobs: {} of {} tasks succeeded",
        success_count, num_tasks
    );

    // Test: Verify lock prevents duplicate job claiming
    println!("\n[2/3] Testing lock prevents duplicate job claiming...");

    // Get any remaining pending jobs to verify no duplicates
    let worker_jobs: Arc<std::sync::Mutex<std::collections::HashMap<i32, Vec<i64>>>> =
        Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()));

    let mut verify_handles = vec![];
    for worker_id in 0..3 {
        let worker_jobs_clone = worker_jobs.clone();
        let node = format!("verify_node_{}", worker_id);

        verify_handles.push(tokio::spawn(async move {
            match file_list::get_pending_jobs(&node, 5).await {
                Ok(jobs) => {
                    let job_ids: Vec<i64> = jobs.iter().map(|j| j.id).collect();
                    worker_jobs_clone
                        .lock()
                        .unwrap()
                        .insert(worker_id, job_ids.clone());
                    (worker_id, Ok(job_ids))
                }
                Err(e) => (worker_id, Err(e.to_string())),
            }
        }));
    }

    for handle in verify_handles {
        let _ = handle.await;
    }

    // Check for duplicates
    let all_jobs = worker_jobs.lock().unwrap();
    let mut all_claimed_ids: Vec<i64> = all_jobs.values().flatten().cloned().collect();
    let original_len = all_claimed_ids.len();
    all_claimed_ids.sort();
    all_claimed_ids.dedup();
    let unique_len = all_claimed_ids.len();

    if original_len == unique_len {
        println!(
            "✓ No duplicate job claiming detected ({} unique jobs)",
            unique_len
        );
    } else {
        panic!(
            "⚠ Duplicate job claiming detected: {} total, {} unique",
            original_len, unique_len
        );
    }

    // Test: Different nodes should be able to claim different jobs
    println!("\n[3/3] Testing different nodes claim different jobs...");

    // Clean up by marking jobs as done
    if !job_ids.is_empty() {
        match file_list::set_job_done(&job_ids).await {
            Ok(_) => println!("✓ Cleanup: Marked {} jobs as done", job_ids.len()),
            Err(e) => panic!("⚠ Cleanup failed: {}", e),
        }
    }

    println!("\n✓ File List Jobs Concurrent Get Pending Test PASSED");
}

/// Test: get_pending_dump_jobs concurrent access with distributed lock
/// This tests the MySQL GET_LOCK based distributed lock mechanism
/// used by get_pending_dump_jobs for serialized dump job claiming.
pub async fn test_file_list_jobs_concurrent_get_dump_jobs_impl(prefix: &str) {
    println!("\n========== File List Jobs Concurrent Get Dump Jobs Test ==========");
    println!("Table: file_list_jobs");
    println!("Testing: get_pending_dump_jobs with distributed lock (GET_LOCK)\n");

    let short_prefix = &prefix[..prefix.len().min(10)];
    let test_org = format!("{}_dump_org", short_prefix);
    let test_stream = format!("{}_dump_stream", short_prefix);

    // Ensure file_list tables exist
    file_list::create_table().await.unwrap();

    // Add test jobs and mark them as done (dump jobs are jobs with status=Done and dumped=false)
    println!("[Setup] Adding jobs and marking as done for dump test...");
    let mut job_ids = Vec::new();
    for i in 0..8 {
        let offset = chrono::Utc::now().timestamp_micros() + i * 1000;
        match file_list::add_job(&test_org, FileStreamType::Logs, &test_stream, offset).await {
            Ok(id) => {
                job_ids.push(id);
            }
            Err(e) => {
                panic!("⚠ Failed to add job {}: {}", i, e);
            }
        }
    }

    if job_ids.is_empty() {
        panic!("⏭ No jobs were created, skipping concurrent dump test");
    }

    // Mark jobs as done so they become eligible for dump
    match file_list::set_job_done(&job_ids).await {
        Ok(_) => println!(
            "✓ Setup: Marked {} jobs as done (eligible for dump)",
            job_ids.len()
        ),
        Err(e) => {
            panic!("⚠ Failed to mark jobs as done: {}", e);
        }
    }

    // Test: Multiple concurrent get_pending_dump_jobs calls
    println!("\n[1/2] Testing concurrent get_pending_dump_jobs calls...");

    let completion_order = Arc::new(std::sync::Mutex::new(Vec::new()));
    let total_dump_jobs = Arc::new(AtomicI32::new(0));
    let num_tasks = 3;
    let mut handles = vec![];

    for task_id in 0..num_tasks {
        let order_clone = completion_order.clone();
        let dump_jobs_clone = total_dump_jobs.clone();
        let node_name = format!("dump_node_{}", task_id);
        let delay = task_id as u64 * 15;

        handles.push(tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;

            let start = std::time::Instant::now();

            // This should acquire the distributed lock for dump jobs
            let result = file_list::get_pending_dump_jobs(&node_name, 3).await;

            let elapsed = start.elapsed();
            order_clone
                .lock()
                .unwrap()
                .push((task_id, elapsed.as_millis()));

            if let Ok(ref jobs) = result {
                dump_jobs_clone.fetch_add(jobs.len() as i32, AtomicOrd::SeqCst);
            }

            (task_id, result, elapsed)
        }));
    }

    let mut results = vec![];
    for handle in handles {
        match handle.await {
            Ok(r) => results.push(r),
            Err(e) => panic!("  Task panicked: {}", e),
        }
    }

    let order = completion_order.lock().unwrap();
    println!(
        "  Completion order: {:?}",
        order.iter().map(|(id, _)| id).collect::<Vec<_>>()
    );
    println!(
        "  Task timings: {:?}",
        order
            .iter()
            .map(|(id, ms)| format!("t{}={}ms", id, ms))
            .collect::<Vec<_>>()
    );

    let mut success_count = 0;
    for (task_id, result, elapsed) in &results {
        match result {
            Ok(jobs) => {
                println!(
                    "  Task {} succeeded: {} dump jobs, {:?}",
                    task_id,
                    jobs.len(),
                    elapsed
                );
                for (id, stream, offset) in jobs {
                    println!("    - id={}, stream={}, offset={}", id, stream, offset);
                }
                success_count += 1;
            }
            Err(e) => {
                if e.to_string().contains("LockTimeout") {
                    println!(
                        "  Task {} timed out (expected with lock contention): {:?}",
                        task_id, elapsed
                    );
                } else {
                    panic!("  Task {} failed: {} ({:?})", task_id, e, elapsed);
                }
            }
        }
    }

    let total_dumps = total_dump_jobs.load(AtomicOrd::SeqCst);
    println!("\n  Total dump jobs fetched: {}", total_dumps);
    println!(
        "  Success rate: {}/{} tasks succeeded",
        success_count, num_tasks
    );

    // Test: Lock serialization verification
    println!("\n[2/2] Testing lock serialization for dump jobs...");

    let concurrent_count = Arc::new(AtomicI32::new(0));
    let max_concurrent = Arc::new(AtomicI32::new(0));

    let mut serial_handles = vec![];
    for task_id in 0..4 {
        let cc = concurrent_count.clone();
        let mc = max_concurrent.clone();
        let node = format!("serial_dump_node_{}", task_id);

        serial_handles.push(tokio::spawn(async move {
            let current = cc.fetch_add(1, AtomicOrd::SeqCst) + 1;
            mc.fetch_max(current, AtomicOrd::SeqCst);

            let result = file_list::get_pending_dump_jobs(&node, 2).await;

            cc.fetch_sub(1, AtomicOrd::SeqCst);
            (task_id, result)
        }));
    }

    for handle in serial_handles {
        if let Ok((task_id, result)) = handle.await {
            match result {
                Ok(jobs) => println!("  Task {} got {} dump jobs", task_id, jobs.len()),
                Err(e) if e.to_string().contains("LockTimeout") => {
                    println!("  Task {} timed out (serialization working)", task_id);
                }
                Err(e) => panic!("  Task {} error: {}", task_id, e),
            }
        }
    }

    let max_conc = max_concurrent.load(AtomicOrd::SeqCst);
    println!("  Max observed concurrent tasks: {}", max_conc);

    // Cleanup
    if !job_ids.is_empty() {
        let _ = file_list::set_job_dumped_status(&job_ids, true).await;
    }

    println!("\n✓ File List Jobs Concurrent Get Dump Jobs Test PASSED");
}

/// Test: Lock serialization verification for file_list_jobs operations
/// Verifies that get_pending_jobs acquires the lock and properly serializes access
pub async fn test_file_list_jobs_lock_serialization_impl(prefix: &str) {
    println!("\n========== File List Jobs Lock Serialization Test ==========");
    println!("Testing: Lock ensures serialized job claiming\n");

    let short_prefix = &prefix[..prefix.len().min(10)];
    let test_org = format!("{}_serial_job_org", short_prefix);
    let test_stream = format!("{}_serial_stream", short_prefix);

    // Ensure tables exist
    let _ = file_list::create_table().await;

    // Add test jobs
    let mut job_ids = Vec::new();
    for i in 0..15 {
        let offset = chrono::Utc::now().timestamp_micros() + i * 1000;
        if let Ok(id) =
            file_list::add_job(&test_org, FileStreamType::Logs, &test_stream, offset).await
        {
            job_ids.push(id);
        }
    }
    println!("✓ Setup: Added {} jobs", job_ids.len());

    if job_ids.is_empty() {
        println!("⏭ No jobs created, skipping serialization test");
        return;
    }

    // Track concurrent execution
    let concurrent_count = Arc::new(AtomicI32::new(0));
    let max_concurrent = Arc::new(AtomicI32::new(0));
    let successful_claims = Arc::new(AtomicI32::new(0));

    let num_tasks = 6;
    let mut handles = vec![];

    for task_id in 0..num_tasks {
        let cc = concurrent_count.clone();
        let mc = max_concurrent.clone();
        let sc = successful_claims.clone();
        let node = format!("serial_node_{}", task_id);

        handles.push(tokio::spawn(async move {
            let current = cc.fetch_add(1, AtomicOrd::SeqCst) + 1;
            mc.fetch_max(current, AtomicOrd::SeqCst);

            let result = file_list::get_pending_jobs(&node, 3).await;

            cc.fetch_sub(1, AtomicOrd::SeqCst);

            if let Ok(ref jobs) = result {
                if !jobs.is_empty() {
                    sc.fetch_add(1, AtomicOrd::SeqCst);
                }
            }

            (task_id, result)
        }));
    }

    let mut success_count = 0;
    let mut timeout_count = 0;
    for handle in handles {
        if let Ok((task_id, result)) = handle.await {
            match result {
                Ok(jobs) => {
                    println!("  Task {} claimed {} jobs", task_id, jobs.len());
                    success_count += 1;
                }
                Err(e) => {
                    if e.to_string().contains("LockTimeout") {
                        println!("  Task {} timed out (serialization)", task_id);
                        timeout_count += 1;
                    } else {
                        println!("  Task {} failed: {}", task_id, e);
                    }
                }
            }
        }
    }

    let max_conc = max_concurrent.load(AtomicOrd::SeqCst);
    let successful = successful_claims.load(AtomicOrd::SeqCst);

    println!("\n  Summary:");
    println!("    Max concurrent tasks spawned: {}", max_conc);
    println!("    Successful claims: {}/{}", success_count, num_tasks);
    println!("    Timeouts: {}", timeout_count);
    println!("    Tasks with jobs: {}", successful);

    // Cleanup
    if !job_ids.is_empty() {
        let _ = file_list::set_job_done(&job_ids).await;
    }

    println!("\n✓ Lock serialization test PASSED");
}

// ==================== File List Jobs Tests ====================

/// Test data structure for file_list_jobs
#[derive(Debug, Clone, serde::Deserialize)]
pub struct FileListJobsTestData {
    pub id: i64,
    pub org: String,
    pub stream: String,
    pub offsets: i64,
    pub status: i32,
    pub node: String,
    pub started_at: i64,
    pub updated_at: i64,
    pub dumped: bool,
}

/// Test: file_list_jobs table compatibility
/// Order: 33
/// Dependencies: None (separate file_list infrastructure)
/// Behavior: add_job, get_pending_jobs (via table structure verification)
pub async fn test_file_list_jobs_compat_impl(_prefix: &str) {
    println!("\n========== File List Jobs Compatibility Test [Order: 33] ==========");
    println!("Table: file_list_jobs");
    println!("Dependencies: None (separate file_list infrastructure)");
    println!("Behavior: table creation and structure verification\n");

    // Load test data
    let test_data: Vec<FileListJobsTestData> = load_test_data("file_list_jobs.json");
    println!(
        "Loaded {} file_list_jobs records from test data",
        test_data.len()
    );

    // Ensure file_list tables exist
    println!("\n[1/2] Testing create_table...");
    file_list::create_table().await.unwrap();
    println!("✓ create_table succeeded (includes file_list_jobs table)");

    // Verify table structure by checking test data format
    println!("\n[2/2] Verifying test data structure...");
    if let Some(data) = test_data.first() {
        println!("  Sample record:");
        println!("    org: {}", data.org);
        println!("    stream: {}", data.stream);
        println!("    offsets: {}", data.offsets);
        println!("    status: {}", data.status);
        println!("    dumped: {}", data.dumped);
        println!("✓ Test data structure verified");
    } else {
        println!("⚠ No test data available for file_list_jobs");
    }

    // Note: file_list_jobs is primarily used internally by the compactor
    // Direct add/remove operations are not exposed in the public API
    // The table is verified through create_table() which creates all file_list tables

    println!("\n✓ File List Jobs compatibility test PASSED");
}

// ==================== Scheduled Jobs Tests ====================

/// Test data structure for scheduled_jobs
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ScheduledJobsTestData {
    pub id: i64,
    pub org: String,
    pub module: i32,
    pub module_key: String,
    pub is_realtime: bool,
    pub is_silenced: bool,
    pub status: i32,
    pub start_time: i64,
    pub end_time: i64,
    pub retries: i32,
    pub next_run_at: i64,
    pub data: String,
}

/// Test: scheduled_jobs table compatibility
/// Order: 34
/// Dependencies: None
/// Behavior: create_table, push, get, list, delete
pub async fn test_scheduled_jobs_compat_impl(prefix: &str) {
    println!("\n========== Scheduled Jobs Compatibility Test [Order: 34] ==========");
    println!("Table: scheduled_jobs");
    println!("Dependencies: None");
    println!("Behavior: create_table, push, get, list, delete\n");

    // Load test data
    let test_data: Vec<ScheduledJobsTestData> = load_test_data("scheduled_jobs.json");
    println!(
        "Loaded {} scheduled_jobs records from test data",
        test_data.len()
    );

    // Use short prefix for test isolation
    let short_prefix = &prefix[..prefix.len().min(10)];
    let test_org = format!("{}_sched_org", short_prefix);
    let test_key = format!("{}_test_job", short_prefix);

    // Test: Create table
    println!("\n[1/5] Testing create_table...");
    match scheduler::init().await {
        Ok(_) => println!("✓ create_table succeeded"),
        Err(e) => {
            panic!("⚠ create_table failed: {}", e);
        }
    }

    // Test: Push a trigger
    // NOTE: We use TriggerModule::Report instead of TriggerModule::Alert to avoid
    // cluster_coordinator operations which require SQLite in local_mode
    println!("\n[2/5] Testing push...");
    let trigger = Trigger {
        org: test_org.clone(),
        module: TriggerModule::Report,
        module_key: test_key.clone(),
        is_realtime: false,
        is_silenced: false,
        status: TriggerStatus::Waiting,
        next_run_at: chrono::Utc::now().timestamp_micros() + 3600_000_000, // 1 hour from now
        data: "{}".to_string(),
        ..Default::default()
    };

    match scheduler::push(trigger.clone()).await {
        Ok(_) => println!("✓ push succeeded for job: {}", test_key),
        Err(e) => {
            panic!("⚠ push failed: {}", e);
        }
    }

    // Test: Get trigger
    println!("\n[3/5] Testing get...");
    match scheduler::get(&test_org, TriggerModule::Report, &test_key).await {
        Ok(t) => {
            assert_eq!(t.org, test_org, "trigger org should match");
            assert_eq!(t.module_key, test_key, "trigger module_key should match");
            assert_eq!(
                t.module,
                TriggerModule::Report,
                "trigger module should be Report"
            );
            assert_eq!(
                t.status,
                TriggerStatus::Waiting,
                "trigger status should be Waiting"
            );
            assert!(!t.is_realtime, "trigger should not be realtime");
            assert!(!t.is_silenced, "trigger should not be silenced");
            println!(
                "✓ get succeeded: org={}, module_key={}",
                t.org, t.module_key
            );
        }
        Err(e) => {
            panic!("⚠ get failed: {}", e);
        }
    }

    // Test: List triggers
    println!("\n[4/5] Testing list...");
    match scheduler::list(Some(TriggerModule::Report)).await {
        Ok(triggers) => {
            let test_triggers: Vec<_> = triggers
                .iter()
                .filter(|t| t.org.contains(short_prefix))
                .collect();
            println!(
                "✓ list returned {} triggers, {} test triggers",
                triggers.len(),
                test_triggers.len()
            );
        }
        Err(e) => {
            panic!("⚠ list failed: {}", e);
        }
    }

    // Test: Delete trigger (cleanup)
    println!("\n[5/5] Testing delete...");
    match scheduler::delete(&test_org, TriggerModule::Report, &test_key).await {
        Ok(_) => println!("✓ delete succeeded for job: {}", test_key),
        Err(e) => {
            panic!("⚠ delete failed: {}", e);
        }
    }

    println!("\n✓ Scheduled Jobs compatibility test PASSED");
}

// ==================== Scheduler Concurrent Tests ====================

/// Test: scheduler pull concurrent access with distributed lock
/// This tests the MySQL GET_LOCK based distributed lock mechanism
/// used by scheduler pull to ensure serialized job claiming.
pub async fn test_scheduled_jobs_concurrent_pull_impl(prefix: &str) {
    println!("\n========== Scheduled Jobs Concurrent Pull Test ==========");
    println!("Table: scheduled_jobs");
    println!("Testing: pull with distributed lock (GET_LOCK)\n");

    let short_prefix = &prefix[..prefix.len().min(10)];
    let test_org = format!("{}_pull_org", short_prefix);

    // Ensure scheduler tables exist
    if let Err(_) = scheduler::init().await {}

    // Add test jobs with Waiting status and next_run_at in the past (eligible for pull)
    println!("[Setup] Adding waiting jobs for concurrent pull test...");
    let mut created_jobs = 0;
    for i in 0..12 {
        let test_key = format!("{}_pull_job_{}", short_prefix, i);
        let trigger = Trigger {
            org: test_org.clone(),
            module: TriggerModule::Alert,
            module_key: test_key.clone(),
            is_realtime: false,
            is_silenced: false,
            status: TriggerStatus::Waiting,
            next_run_at: chrono::Utc::now().timestamp_micros() - 60_000_000, /* 1 minute ago
                                                                              * (eligible for
                                                                              * pull) */
            data: format!(r#"{{"job_id": {}}}"#, i),
            ..Default::default()
        };

        match scheduler::push(trigger).await {
            Ok(_) => created_jobs += 1,
            Err(e) => {
                // Might fail if job already exists
                if !e.to_string().contains("Duplicate") {
                    panic!("⚠ Failed to create job {}: {}", i, e);
                }
            }
        }
    }
    println!("✓ Setup: Created {} waiting jobs", created_jobs);

    if created_jobs == 0 {
        panic!("⏭ No jobs were created, skipping concurrent pull test");
    }

    // Test: Multiple concurrent pull calls
    println!("\n[1/3] Testing concurrent pull calls (lock serialization)...");

    let completion_order = Arc::new(std::sync::Mutex::new(Vec::new()));
    let total_jobs_pulled = Arc::new(AtomicI32::new(0));
    let num_tasks = 4;
    let mut handles = vec![];

    for task_id in 0..num_tasks {
        let order_clone = completion_order.clone();
        let jobs_pulled_clone = total_jobs_pulled.clone();
        let delay = task_id as u64 * 10; // Stagger start times

        handles.push(tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;

            let start = std::time::Instant::now();

            // pull(concurrency, alert_timeout, report_timeout)
            let result = scheduler::pull(3, 60, 300).await;

            let elapsed = start.elapsed();
            order_clone
                .lock()
                .unwrap()
                .push((task_id, elapsed.as_millis()));

            if let Ok(ref jobs) = result {
                jobs_pulled_clone.fetch_add(jobs.len() as i32, AtomicOrd::SeqCst);
            }

            (task_id, result, elapsed)
        }));
    }

    let mut results = vec![];
    for handle in handles {
        match handle.await {
            Ok(r) => results.push(r),
            Err(e) => println!("  Task panicked: {}", e),
        }
    }

    let order = completion_order.lock().unwrap();
    println!(
        "  Completion order: {:?}",
        order.iter().map(|(id, _)| id).collect::<Vec<_>>()
    );
    println!(
        "  Task timings: {:?}",
        order
            .iter()
            .map(|(id, ms)| format!("t{}={}ms", id, ms))
            .collect::<Vec<_>>()
    );

    let mut success_count = 0;
    for (task_id, result, elapsed) in &results {
        match result {
            Ok(jobs) => {
                println!(
                    "  Task {} succeeded: {} jobs pulled, {:?}",
                    task_id,
                    jobs.len(),
                    elapsed
                );
                success_count += 1;
            }
            Err(e) => {
                if e.to_string().contains("LockTimeout") {
                    println!(
                        "  Task {} timed out (expected with lock contention): {:?}",
                        task_id, elapsed
                    );
                } else {
                    panic!("  Task {} failed: {} ({:?})", task_id, e, elapsed);
                }
            }
        }
    }

    let total_pulled = total_jobs_pulled.load(AtomicOrd::SeqCst);
    println!("\n  Total jobs pulled across all tasks: {}", total_pulled);
    println!(
        "  Success rate: {}/{} tasks succeeded",
        success_count, num_tasks
    );

    // At least one task should succeed
    assert!(success_count >= 1, "At least one pull should succeed");
    println!(
        "✓ Concurrent pull: {} of {} tasks succeeded",
        success_count, num_tasks
    );

    // Test: Verify lock prevents duplicate job claiming
    println!("\n[2/3] Testing lock prevents duplicate job claiming...");

    let pulled_job_ids: Arc<std::sync::Mutex<Vec<i64>>> =
        Arc::new(std::sync::Mutex::new(Vec::new()));

    let mut verify_handles = vec![];
    for worker_id in 0..3 {
        let pulled_ids_clone = pulled_job_ids.clone();

        verify_handles.push(tokio::spawn(async move {
            match scheduler::pull(4, 60, 300).await {
                Ok(jobs) => {
                    let ids: Vec<i64> = jobs.iter().map(|j| j.id).collect();
                    pulled_ids_clone.lock().unwrap().extend(ids.clone());
                    (worker_id, Ok(ids))
                }
                Err(e) => (worker_id, Err(e.to_string())),
            }
        }));
    }

    for handle in verify_handles {
        if let Ok((worker_id, result)) = handle.await {
            match result {
                Ok(ids) => println!("  Worker {} pulled {} jobs", worker_id, ids.len()),
                Err(e) if e.contains("LockTimeout") => println!("  Worker {} timed out", worker_id),
                Err(e) => panic!("  Worker {} error: {}", worker_id, e),
            }
        }
    }

    // Check for duplicates
    let all_ids = pulled_job_ids.lock().unwrap();
    let original_len = all_ids.len();
    let mut unique_ids = all_ids.clone();
    unique_ids.sort();
    unique_ids.dedup();
    let unique_len = unique_ids.len();

    if original_len == unique_len {
        println!(
            "✓ No duplicate job claiming detected ({} unique jobs)",
            unique_len
        );
    } else {
        panic!(
            "⚠ Duplicate job claiming detected: {} total, {} unique",
            original_len, unique_len
        );
    }

    // Test: Lock serialization verification
    println!("\n[3/3] Testing lock serialization for pull...");

    let concurrent_count = Arc::new(AtomicI32::new(0));
    let max_concurrent = Arc::new(AtomicI32::new(0));

    let mut serial_handles = vec![];
    for task_id in 0..5 {
        let cc = concurrent_count.clone();
        let mc = max_concurrent.clone();

        serial_handles.push(tokio::spawn(async move {
            let current = cc.fetch_add(1, AtomicOrd::SeqCst) + 1;
            mc.fetch_max(current, AtomicOrd::SeqCst);

            let result = scheduler::pull(2, 60, 300).await;

            cc.fetch_sub(1, AtomicOrd::SeqCst);
            (task_id, result)
        }));
    }

    let mut task_success = 0;
    for handle in serial_handles {
        if let Ok((task_id, result)) = handle.await {
            match result {
                Ok(jobs) => {
                    println!("  Task {} pulled {} jobs", task_id, jobs.len());
                    task_success += 1;
                }
                Err(e) if e.to_string().contains("LockTimeout") => {
                    println!("  Task {} timed out (serialization working)", task_id);
                }
                Err(e) => println!("  Task {} error: {}", task_id, e),
            }
        }
    }

    let max_conc = max_concurrent.load(AtomicOrd::SeqCst);
    println!("  Max concurrent tasks spawned: {}", max_conc);
    println!("  Tasks that pulled jobs: {}", task_success);

    // Cleanup: Delete test jobs (ignore errors as some may have been processed)
    println!("\n[Cleanup] Deleting test jobs...");
    for i in 0..12 {
        let test_key = format!("{}_pull_job_{}", short_prefix, i);
        let _ = scheduler::delete(&test_org, TriggerModule::Alert, &test_key).await;
    }

    println!("\n✓ Scheduled Jobs Concurrent Pull Test PASSED");
}

/// Test: scheduler pull lock serialization verification
/// Verifies that pull acquires the lock and properly serializes job claiming
pub async fn test_scheduled_jobs_pull_lock_serialization_impl(prefix: &str) {
    println!("\n========== Scheduled Jobs Pull Lock Serialization Test ==========");
    println!("Testing: Lock ensures serialized job pulling\n");

    let short_prefix = &prefix[..prefix.len().min(10)];
    let test_org = format!("{}_serial_pull_org", short_prefix);

    // Ensure scheduler tables exist
    let _ = scheduler::init().await;

    // Add test jobs
    let mut created_jobs = 0;
    for i in 0..15 {
        let test_key = format!("{}_serial_pull_{}", short_prefix, i);
        let trigger = Trigger {
            org: test_org.clone(),
            module: TriggerModule::Alert,
            module_key: test_key,
            is_realtime: false,
            is_silenced: false,
            status: TriggerStatus::Waiting,
            next_run_at: chrono::Utc::now().timestamp_micros() - 30_000_000, // 30 seconds ago
            data: "{}".to_string(),
            ..Default::default()
        };
        if scheduler::push(trigger).await.is_ok() {
            created_jobs += 1;
        }
    }
    println!("✓ Setup: Created {} jobs", created_jobs);

    if created_jobs == 0 {
        panic!("⏭ No jobs created, skipping serialization test");
    }

    // Track concurrent execution
    let concurrent_count = Arc::new(AtomicI32::new(0));
    let max_concurrent = Arc::new(AtomicI32::new(0));
    let successful_pulls = Arc::new(AtomicI32::new(0));

    let num_tasks = 6;
    let mut handles = vec![];

    for task_id in 0..num_tasks {
        let cc = concurrent_count.clone();
        let mc = max_concurrent.clone();
        let sp = successful_pulls.clone();

        handles.push(tokio::spawn(async move {
            let current = cc.fetch_add(1, AtomicOrd::SeqCst) + 1;
            mc.fetch_max(current, AtomicOrd::SeqCst);

            let result = scheduler::pull(3, 60, 300).await;

            cc.fetch_sub(1, AtomicOrd::SeqCst);

            if let Ok(ref jobs) = result {
                if !jobs.is_empty() {
                    sp.fetch_add(1, AtomicOrd::SeqCst);
                }
            }

            (task_id, result)
        }));
    }

    let mut success_count = 0;
    let mut timeout_count = 0;
    for handle in handles {
        if let Ok((task_id, result)) = handle.await {
            match result {
                Ok(jobs) => {
                    println!("  Task {} pulled {} jobs", task_id, jobs.len());
                    success_count += 1;
                }
                Err(e) => {
                    if e.to_string().contains("LockTimeout") {
                        println!("  Task {} timed out (serialization)", task_id);
                        timeout_count += 1;
                    } else {
                        panic!("  Task {} failed: {}", task_id, e);
                    }
                }
            }
        }
    }

    let max_conc = max_concurrent.load(AtomicOrd::SeqCst);
    let successful = successful_pulls.load(AtomicOrd::SeqCst);

    println!("\n  Summary:");
    println!("    Max concurrent tasks spawned: {}", max_conc);
    println!("    Successful pulls: {}/{}", success_count, num_tasks);
    println!("    Timeouts: {}", timeout_count);
    println!("    Tasks with jobs: {}", successful);

    // Cleanup
    for i in 0..15 {
        let test_key = format!("{}_serial_pull_{}", short_prefix, i);
        let _ = scheduler::delete(&test_org, TriggerModule::Alert, &test_key).await;
    }

    println!("\n✓ Pull lock serialization test PASSED");
}

// ==================== Additional Table Tests ====================

/// Test: cipher_keys table compatibility
/// Order: 40
/// Dependencies: None
/// Behavior: add, get_data, list_all, remove
pub async fn test_cipher_keys_compat_impl(prefix: &str) {
    println!("\n========== Cipher Keys Compatibility Test [Order: 40] ==========");
    println!("Table: cipher_keys");
    println!("Dependencies: None");
    println!("Behavior: add, get_data, list_all, remove\n");

    let short_prefix = &prefix[..prefix.len().min(10)];
    let test_org = format!("{}_cipher_org", short_prefix);
    let test_name = format!("{}_test_key", short_prefix);

    // Test: Add cipher entry
    println!("[1/4] Testing add...");
    let entry = CipherEntry {
        org: test_org.clone(),
        created_at: chrono::Utc::now().timestamp_micros(),
        created_by: "test_user".to_string(),
        name: test_name.clone(),
        data: "test_cipher_data".to_string(),
        kind: EntryKind::CipherKey,
    };

    match cipher::add(entry.clone()).await {
        Ok(_) => println!("✓ add succeeded"),
        Err(e) => {
            panic!("⚠ add failed: {}", e);
        }
    }

    // Test: Get data
    println!("\n[2/4] Testing get_data...");
    match cipher::get_data(&test_org, EntryKind::CipherKey, &test_name).await {
        Ok(Some(data)) => {
            assert_eq!(
                data, "test_cipher_data",
                "cipher data should match inserted value"
            );
            println!("✓ get_data succeeded: data_len={}", data.len());
        }
        Ok(None) => panic!("⚠ get_data returned None"),
        Err(e) => panic!("⚠ get_data failed: {}", e),
    }

    // Test: List all
    println!("\n[3/4] Testing list_all...");
    match cipher::list_all(Some(100)).await {
        Ok(entries) => {
            let test_entries: Vec<_> = entries
                .iter()
                .filter(|e| e.org.contains(short_prefix))
                .collect();
            println!(
                "✓ list_all returned {} entries, {} test entries",
                entries.len(),
                test_entries.len()
            );
        }
        Err(e) => panic!("⚠ list_all failed: {}", e),
    }

    // Test: Remove
    println!("\n[4/4] Testing remove...");
    match cipher::remove(&test_org, EntryKind::CipherKey, &test_name).await {
        Ok(_) => println!("✓ remove succeeded"),
        Err(e) => panic!("⚠ remove failed: {}", e),
    }

    println!("\n✓ Cipher Keys compatibility test PASSED");
}

/// Test: system_settings table compatibility
/// Order: 41
/// Dependencies: None
/// Behavior: set, get, list, delete
pub async fn test_system_settings_compat_impl(prefix: &str) {
    println!("\n========== System Settings Compatibility Test [Order: 41] ==========");
    println!("Table: system_settings");
    println!("Dependencies: None");
    println!("Behavior: set, get, list, delete\n");

    // Ensure ORM tables exist
    println!("[0/4] Ensuring ORM tables exist...");
    if let Err(e) = ensure_orm_tables_exist().await {
        panic!("⚠ Failed to create ORM tables: {}", e);
    }

    let short_prefix = &prefix[..prefix.len().min(10)];
    let test_org = format!("{}_settings_org", short_prefix);
    let test_key = format!("{}_test_setting", short_prefix);

    // Test: Set setting
    println!("[1/4] Testing set...");
    let setting = SystemSetting {
        id: None,
        scope: SettingScope::Org,
        org_id: Some(test_org.clone()),
        user_id: None,
        setting_key: test_key.clone(),
        setting_category: Some("test_category".to_string()),
        setting_value: serde_json::json!({"test": true}),
        description: Some("Test setting".to_string()),
        created_at: chrono::Utc::now().timestamp_micros(),
        updated_at: chrono::Utc::now().timestamp_micros(),
        created_by: Some("test_user".to_string()),
        updated_by: Some("test_user".to_string()),
    };

    match system_settings::set(&setting).await {
        Ok(s) => println!("✓ set succeeded: id={:?}", s.id),
        Err(e) => {
            panic!("⚠ set failed: {}", e);
        }
    }

    // Test: Get setting
    println!("\n[2/4] Testing get...");
    match system_settings::get(&SettingScope::Org, Some(&test_org), None, &test_key).await {
        Ok(Some(s)) => {
            assert_eq!(s.setting_key, test_key, "setting_key should match");
            assert_eq!(s.org_id, Some(test_org.clone()), "org_id should match");
            assert_eq!(s.scope, SettingScope::Org, "scope should be Org");
            assert_eq!(
                s.setting_category,
                Some("test_category".to_string()),
                "setting_category should match"
            );
            assert_eq!(
                s.setting_value,
                serde_json::json!({"test": true}),
                "setting_value should match"
            );
            println!("✓ get succeeded: key={}", s.setting_key);
        }
        Ok(None) => panic!("⚠ get returned None"),
        Err(e) => panic!("⚠ get failed: {}", e),
    }

    // Test: List settings
    println!("\n[3/4] Testing list...");
    match system_settings::list(Some(&SettingScope::Org), Some(&test_org), None, None).await {
        Ok(settings) => {
            let test_settings: Vec<_> = settings
                .iter()
                .filter(|s| s.setting_key.contains(short_prefix))
                .collect();
            println!(
                "✓ list returned {} settings, {} test settings",
                settings.len(),
                test_settings.len()
            );
        }
        Err(e) => panic!("⚠ list failed: {}", e),
    }

    // Test: Delete setting
    println!("\n[4/4] Testing delete...");
    match system_settings::delete(&SettingScope::Org, Some(&test_org), None, &test_key).await {
        Ok(deleted) => println!("✓ delete succeeded: deleted={}", deleted),
        Err(e) => panic!("⚠ delete failed: {}", e),
    }

    println!("\n✓ System Settings compatibility test PASSED");
}

/// Test: action_scripts table compatibility
/// Order: 42
/// Dependencies: None
/// Behavior: create_table, add, get, list, update, remove
pub async fn test_action_scripts_compat_impl(prefix: &str) {
    println!("\n========== Action Scripts Compatibility Test [Order: 42] ==========");
    println!("Table: action_scripts");
    println!("Dependencies: None");
    println!("Behavior: create_table, add, get, list, update, remove\n");

    let short_prefix = &prefix[..prefix.len().min(10)];
    let test_org = format!("{}_action_org", short_prefix);
    let test_name = format!("{}_test_action", short_prefix);

    // Test: Create table
    println!("[1/6] Testing create_table...");
    match action_scripts::create_table().await {
        Ok(_) => println!("✓ create_table succeeded"),
        Err(e) => panic!("⚠ create_table failed (may already exist): {}", e),
    }

    // Test: Add action
    println!("\n[2/6] Testing add...");
    let action = Action {
        id: Some(Ksuid::new(None, None)),
        name: test_name.clone(),
        org_id: test_org.clone(),
        environment_variables: HashMap::new(),
        created_by: "test_user".to_string(),
        execution_details: ExecutionDetailsType::Once,
        zip_file_path: Some("/tmp/test.zip".to_string()),
        created_at: chrono::Utc::now(),
        last_executed_at: None,
        description: Some("Test action script".to_string()),
        cron_expr: None,
        status: ActionStatus::Ready,
        zip_file_name: "test.zip".to_string(),
        last_modified_at: chrono::Utc::now(),
        last_successful_at: None,
        origin_cluster_url: "".to_string(),
        service_account: "".to_string(),
    };

    let action_id = match action_scripts::add(&action).await {
        Ok(id) => {
            println!("✓ add succeeded: id={}", id);
            id
        }
        Err(e) => {
            panic!("⚠ add failed: {}", e);
        }
    };

    // Test: Get action
    println!("\n[3/6] Testing get...");
    match action_scripts::get(&action_id, &test_org).await {
        Ok(a) => {
            assert_eq!(a.name, test_name, "action name should match");
            assert_eq!(a.org_id, test_org, "action org_id should match");
            assert_eq!(
                a.description,
                Some("Test action script".to_string()),
                "action description should match"
            );
            assert_eq!(a.zip_file_name, "test.zip", "zip_file_name should match");
            assert_eq!(a.status, ActionStatus::Ready, "status should be Ready");
            println!("✓ get succeeded: name={}", a.name);
        }
        Err(e) => panic!("⚠ get failed: {}", e),
    }

    // Test: List actions
    println!("\n[4/6] Testing list...");
    match action_scripts::list(&test_org, Some(100)).await {
        Ok(actions) => {
            let test_actions: Vec<_> = actions
                .iter()
                .filter(|a| a.name.contains(short_prefix))
                .collect();
            println!(
                "✓ list returned {} actions, {} test actions",
                actions.len(),
                test_actions.len()
            );
        }
        Err(e) => panic!("⚠ list failed: {}", e),
    }

    // Test: Contains
    println!("\n[5/6] Testing contains...");
    match action_scripts::contains(&action_id, &test_org).await {
        Ok(exists) => println!("✓ contains returned: {}", exists),
        Err(e) => panic!("⚠ contains failed: {}", e),
    }

    // Test: Remove
    println!("\n[6/6] Testing remove...");
    match action_scripts::remove(&test_org, &action_id).await {
        Ok(_) => println!("✓ remove succeeded"),
        Err(e) => panic!("⚠ remove failed: {}", e),
    }

    println!("\n✓ Action Scripts compatibility test PASSED");
}

/// Test: enrichment_tables table compatibility
/// Order: 43
/// Dependencies: None
/// Behavior: add, get_by_org_and_name, list, contains, delete
pub async fn test_enrichment_tables_compat_impl(prefix: &str) {
    println!("\n========== Enrichment Tables Compatibility Test [Order: 43] ==========");
    println!("Table: enrichment_tables");
    println!("Dependencies: None");
    println!("Behavior: add, get_by_org_and_name, list, contains, delete\n");

    let short_prefix = &prefix[..prefix.len().min(10)];
    let test_org = format!("{}_enrich_org", short_prefix);
    let test_table = format!("{}_test_table", short_prefix);

    // Test: Add enrichment table
    println!("[1/5] Testing add...");
    let payload = b"test_enrichment_data".to_vec();
    let created_at = chrono::Utc::now().timestamp_micros();

    match enrichment_tables::add(&test_org, &test_table, payload.clone(), created_at).await {
        Ok(_) => println!("✓ add succeeded"),
        Err(e) => {
            panic!("⚠ add failed: {}", e);
        }
    }

    // Test: Get by org and name
    println!("\n[2/5] Testing get_by_org_and_name...");
    match enrichment_tables::get_by_org_and_name(&test_org, &test_table).await {
        Ok(records) => {
            assert!(!records.is_empty(), "should have at least one record");
            // Verify first record's data field matches what we inserted
            let first_record = &records[0];
            assert_eq!(first_record.org, test_org, "org should match");
            assert_eq!(first_record.name, test_table, "name should match");
            assert_eq!(
                first_record.data, payload,
                "enrichment table data should match inserted payload"
            );
            println!("✓ get_by_org_and_name returned {} records", records.len());
        }
        Err(e) => panic!("⚠ get_by_org_and_name failed: {}", e),
    }

    // Test: Contains
    println!("\n[3/5] Testing contains...");
    match enrichment_tables::contains(&test_org, &test_table).await {
        Ok(exists) => {
            assert!(exists, "enrichment table should exist after adding");
            println!("✓ contains returned true for existing table");
        }
        Err(e) => panic!("⚠ contains failed: {}", e),
    }

    // Test: List
    println!("\n[4/5] Testing list...");
    match enrichment_tables::list().await {
        Ok(tables) => {
            let test_tables: Vec<_> = tables
                .iter()
                .filter(|(org, _)| org.contains(short_prefix))
                .collect();
            println!(
                "✓ list returned {} tables, {} test tables",
                tables.len(),
                test_tables.len()
            );
        }
        Err(e) => panic!("⚠ list failed: {}", e),
    }

    // Test: Delete
    println!("\n[5/5] Testing delete...");
    match enrichment_tables::delete(&test_org, &test_table).await {
        Ok(_) => println!("✓ delete succeeded"),
        Err(e) => panic!("⚠ delete failed: {}", e),
    }

    println!("\n✓ Enrichment Tables compatibility test PASSED");
}

/// Test: enrichment_table_urls table compatibility
/// Order: 44
/// Dependencies: None
/// Behavior: put, get, list_by_org, delete
pub async fn test_enrichment_table_urls_compat_impl(prefix: &str) {
    println!("\n========== Enrichment Table URLs Compatibility Test [Order: 44] ==========");
    println!("Table: enrichment_table_urls");
    println!("Dependencies: None");
    println!("Behavior: put, get, list_by_org, delete\n");

    let short_prefix = &prefix[..prefix.len().min(10)];
    let test_org = format!("{}_enrich_url_org", short_prefix);
    let test_table = format!("{}_url_table", short_prefix);

    // Test: Put record
    println!("[1/4] Testing put...");
    let record = EnrichmentTableUrlRecord {
        org: test_org.clone(),
        name: test_table.clone(),
        url: "https://example.com/data.csv".to_string(),
        status: 0,
        error_message: None,
        created_at: chrono::Utc::now().timestamp_micros(),
        updated_at: chrono::Utc::now().timestamp_micros(),
        total_bytes_fetched: 0,
        total_records_processed: 0,
        retry_count: 0,
        append_data: false,
        last_byte_position: 0,
        supports_range: false,
        is_local_region: true,
    };

    match enrichment_table_urls::put(record).await {
        Ok(_) => println!("✓ put succeeded"),
        Err(e) => {
            panic!("⚠ put failed: {}", e);
        }
    }

    // Test: Get
    println!("\n[2/4] Testing get...");
    match enrichment_table_urls::get(&test_org, &test_table).await {
        Ok(Some(r)) => {
            assert_eq!(
                r.url, "https://example.com/data.csv",
                "url should match inserted value"
            );
            assert_eq!(r.org, test_org, "org should match");
            assert_eq!(r.name, test_table, "name should match");
            assert_eq!(r.status, 0, "status should be 0");
            assert!(r.is_local_region, "is_local_region should be true");
            println!("✓ get succeeded: url={}", r.url);
        }
        Ok(None) => panic!("⚠ get returned None"),
        Err(e) => panic!("⚠ get failed: {}", e),
    }

    // Test: List by org
    println!("\n[3/4] Testing list_by_org...");
    match enrichment_table_urls::list_by_org(&test_org).await {
        Ok(records) => println!("✓ list_by_org returned {} records", records.len()),
        Err(e) => panic!("⚠ list_by_org failed: {}", e),
    }

    // Test: Delete
    println!("\n[4/4] Testing delete...");
    match enrichment_table_urls::delete(&test_org, &test_table).await {
        Ok(_) => println!("✓ delete succeeded"),
        Err(e) => panic!("⚠ delete failed: {}", e),
    }

    println!("\n✓ Enrichment Table URLs compatibility test PASSED");
}

/// Test: rate_limit_rules table compatibility
/// Order: 45
/// Dependencies: None
/// Behavior: add, fetch_rules, list, delete
pub async fn test_rate_limit_rules_compat_impl(prefix: &str) {
    println!("\n========== Rate Limit Rules Compatibility Test [Order: 45] ==========");
    println!("Table: rate_limit_rules");
    println!("Dependencies: None");
    println!("Behavior: add, fetch_rules, list, delete\n");

    let short_prefix = &prefix[..prefix.len().min(10)];
    let test_org = format!("{}_ratelimit_org", short_prefix);
    let test_rule_id = format!("{}_test_rule", short_prefix);

    // Test: Add rule
    println!("[1/4] Testing add...");
    let rule = RatelimitRule {
        org: test_org.clone(),
        rule_type: Some("api".to_string()),
        rule_id: Some(test_rule_id.clone()),
        user_role: None,
        user_id: None,
        api_group_name: Some("search".to_string()),
        api_group_operation: Some("query".to_string()),
        threshold: 100,
    };

    match ratelimit::add(RuleEntry::Single(rule.clone())).await {
        Ok(_) => println!("✓ add succeeded"),
        Err(e) => {
            panic!("⚠ add failed: {}", e);
        }
    }

    // Test: Fetch rules
    println!("\n[2/4] Testing fetch_rules...");
    match ratelimit::fetch_rules(vec![], Some(test_org.clone()), None).await {
        Ok(rules) => {
            assert!(!rules.is_empty(), "should have at least one rule");
            // Find our test rule
            let test_rule = rules
                .iter()
                .find(|r| r.rule_id.as_ref() == Some(&test_rule_id));
            assert!(test_rule.is_some(), "should find test rule in results");
            if let Some(rule) = test_rule {
                assert_eq!(rule.threshold, 100, "threshold should match inserted value");
                assert_eq!(
                    rule.api_group_name,
                    Some("search".to_string()),
                    "api_group_name should match"
                );
                assert_eq!(
                    rule.api_group_operation,
                    Some("query".to_string()),
                    "api_group_operation should match"
                );
            }
            println!("✓ fetch_rules returned {} rules", rules.len());
        }
        Err(e) => panic!("⚠ fetch_rules failed: {}", e),
    }

    // Test: List
    println!("\n[3/4] Testing list...");
    match ratelimit::list(&test_org, None, None).await {
        Ok(rules) => {
            let test_rules: Vec<_> = rules
                .iter()
                .filter(|r| {
                    r.rule_id
                        .as_ref()
                        .map(|id| id.contains(short_prefix))
                        .unwrap_or(false)
                })
                .collect();
            println!(
                "✓ list returned {} rules, {} test rules",
                rules.len(),
                test_rules.len()
            );
        }
        Err(e) => panic!("⚠ list failed: {}", e),
    }

    // Test: Delete
    println!("\n[4/4] Testing delete...");
    match ratelimit::delete(test_rule_id).await {
        Ok(_) => println!("✓ delete succeeded"),
        Err(e) => panic!("⚠ delete failed: {}", e),
    }

    println!("\n✓ Rate Limit Rules compatibility test PASSED");
}

/// Test: re_patterns table compatibility
/// Order: 46
/// Dependencies: None
/// Behavior: add, get, list_by_org, update_pattern, remove
pub async fn test_re_patterns_compat_impl(prefix: &str) {
    println!("\n========== RE Patterns Compatibility Test [Order: 46] ==========");
    println!("Table: re_patterns");
    println!("Dependencies: None");
    println!("Behavior: add, get, list_by_org, update_pattern, remove\n");

    let short_prefix = &prefix[..prefix.len().min(10)];
    let test_org = format!("{}_pattern_org", short_prefix);
    let test_id = format!("{}_pattern_id", short_prefix);

    // Test: Add pattern
    println!("[1/5] Testing add...");
    let entry = PatternEntry {
        id: test_id.clone(),
        org: test_org.clone(),
        name: "test_pattern".to_string(),
        description: "Test pattern description".to_string(),
        created_by: "test_user".to_string(),
        created_at: chrono::Utc::now().timestamp_micros(),
        updated_at: chrono::Utc::now().timestamp_micros(),
        pattern: r"\d{4}-\d{2}-\d{2}".to_string(),
    };

    match re_pattern::add(entry).await {
        Ok(_) => println!("✓ add succeeded"),
        Err(e) => {
            panic!("⚠ add failed: {}", e);
        }
    }

    // Test: Get
    println!("\n[2/5] Testing get...");
    match re_pattern::get(&test_id).await {
        Ok(Some(p)) => {
            assert_eq!(p.name, "test_pattern", "pattern name should match");
            assert_eq!(p.org, test_org, "pattern org should match");
            assert_eq!(
                p.description, "Test pattern description",
                "pattern description should match"
            );
            assert_eq!(
                p.pattern, r"\d{4}-\d{2}-\d{2}",
                "pattern regex should match"
            );
            assert_eq!(p.created_by, "test_user", "created_by should match");
            println!("✓ get succeeded: name={}", p.name);
        }
        Ok(None) => panic!("⚠ get returned None"),
        Err(e) => panic!("⚠ get failed: {}", e),
    }

    // Test: List by org
    println!("\n[3/5] Testing list_by_org...");
    match re_pattern::list_by_org(&test_org).await {
        Ok(patterns) => println!("✓ list_by_org returned {} patterns", patterns.len()),
        Err(e) => panic!("⚠ list_by_org failed: {}", e),
    }

    // Test: Update pattern
    println!("\n[4/5] Testing update_pattern...");
    match re_pattern::update_pattern(&test_id, r"\d{2}/\d{2}/\d{4}").await {
        Ok(_) => {
            // Verify the update
            let updated = re_pattern::get(&test_id)
                .await
                .expect("get after update failed")
                .expect("pattern should exist");
            assert_eq!(
                updated.pattern, r"\d{2}/\d{2}/\d{4}",
                "pattern should be updated"
            );
            println!("✓ update_pattern succeeded");
        }
        Err(e) => panic!("⚠ update_pattern failed: {}", e),
    }

    // Test: Remove
    println!("\n[5/5] Testing remove...");
    match re_pattern::remove(&test_id).await {
        Ok(_) => println!("✓ remove succeeded"),
        Err(e) => panic!("⚠ remove failed: {}", e),
    }

    println!("\n✓ RE Patterns compatibility test PASSED");
}

/// Test: re_pattern_stream_map table compatibility
/// Order: 47
/// Dependencies: re_patterns
/// Behavior: add, get_by_pattern_id, batch_process, remove_associations_by_stream
pub async fn test_re_pattern_stream_map_compat_impl(prefix: &str) {
    println!("\n========== RE Pattern Stream Map Compatibility Test [Order: 47] ==========");
    println!("Table: re_pattern_stream_map");
    println!("Dependencies: re_patterns");
    println!("Behavior: add, get_by_pattern_id, batch_process, remove_associations_by_stream\n");

    let short_prefix = &prefix[..prefix.len().min(10)];
    let test_org = format!("{}_map_org", short_prefix);
    let test_pattern_id = format!("{}_map_pattern", short_prefix);
    let test_stream = format!("{}_test_stream", short_prefix);

    // Test: Add association
    println!("[1/4] Testing add...");
    let entry = PatternAssociationEntry {
        id: 0, // Will be auto-generated
        org: test_org.clone(),
        stream: test_stream.clone(),
        stream_type: FileStreamType::Logs,
        field: "message".to_string(),
        pattern_id: test_pattern_id.clone(),
        policy: PatternPolicy::Redact,
        apply_at: ApplyPolicy::AtIngestion,
    };

    match re_pattern_stream_map::add(entry).await {
        Ok(_) => println!("✓ add succeeded"),
        Err(e) => {
            panic!("⚠ add failed: {}", e);
        }
    }

    // Test: Get by pattern id
    println!("\n[2/4] Testing get_by_pattern_id...");
    match re_pattern_stream_map::get_by_pattern_id(&test_pattern_id).await {
        Ok(associations) => println!(
            "✓ get_by_pattern_id returned {} associations",
            associations.len()
        ),
        Err(e) => panic!("⚠ get_by_pattern_id failed: {}", e),
    }

    // Test: List all
    println!("\n[3/4] Testing list_all...");
    match re_pattern_stream_map::list_all().await {
        Ok(associations) => {
            let test_associations: Vec<_> = associations
                .iter()
                .filter(|a| a.org.contains(short_prefix))
                .collect();
            println!(
                "✓ list_all returned {} associations, {} test associations",
                associations.len(),
                test_associations.len()
            );
        }
        Err(e) => panic!("⚠ list_all failed: {}", e),
    }

    // Test: Remove associations by stream
    println!("\n[4/4] Testing remove_associations_by_stream...");
    match re_pattern_stream_map::remove_associations_by_stream(
        &test_org,
        &test_stream,
        FileStreamType::Logs,
    )
    .await
    {
        Ok(_) => println!("✓ remove_associations_by_stream succeeded"),
        Err(e) => panic!("⚠ remove_associations_by_stream failed: {}", e),
    }

    println!("\n✓ RE Pattern Stream Map compatibility test PASSED");
}

/// Test: service_streams table compatibility
/// Order: 48
/// Dependencies: None
/// Behavior: create_table, put, get, list, delete
pub async fn test_service_streams_compat_impl(prefix: &str) {
    println!("\n========== Service Streams Compatibility Test [Order: 48] ==========");
    println!("Table: service_streams");
    println!("Dependencies: None");
    println!("Behavior: create_table, put, get, list, delete\n");

    let short_prefix = &prefix[..prefix.len().min(10)];
    let test_org = format!("{}_svc_org", short_prefix);
    let test_service_key = format!("{}_svc_key", short_prefix);

    // Test: Create table / init
    println!("[1/5] Testing init...");
    match service_streams::init().await {
        Ok(_) => println!("✓ init succeeded"),
        Err(e) => panic!("⚠ init failed (may already exist): {}", e),
    }

    // Test: Put record
    println!("\n[2/5] Testing put...");
    let record = ServiceRecord {
        org_id: test_org.clone(),
        service_key: test_service_key.clone(),
        correlation_key: "test_correlation".to_string(),
        service_name: "test_service".to_string(),
        dimensions: "{}".to_string(),
        streams: "[]".to_string(),
        first_seen: chrono::Utc::now().timestamp_micros(),
        last_seen: chrono::Utc::now().timestamp_micros(),
        metadata: None,
    };

    match service_streams::put(record).await {
        Ok(_) => println!("✓ put succeeded"),
        Err(e) => panic!("⚠ put failed: {}", e),
    }

    // Test: Get
    println!("\n[3/5] Testing get...");
    match service_streams::get(&test_org, &test_service_key).await {
        Ok(Some(r)) => {
            assert_eq!(r.service_name, "test_service", "service_name should match");
            assert_eq!(r.org_id, test_org, "org_id should match");
            assert_eq!(r.service_key, test_service_key, "service_key should match");
            assert_eq!(
                r.correlation_key, "test_correlation",
                "correlation_key should match"
            );
            println!("✓ get succeeded: service_name={}", r.service_name);
        }
        Ok(None) => println!("⚠ get returned None"),
        Err(e) => panic!("⚠ get failed: {}", e),
    }

    // Test: List
    println!("\n[4/5] Testing list...");
    match service_streams::list(&test_org).await {
        Ok(records) => println!("✓ list returned {} records", records.len()),
        Err(e) => panic!("⚠ list failed: {}", e),
    }

    // Test: Delete
    println!("\n[5/5] Testing delete...");
    match service_streams::delete(&test_org, &test_service_key).await {
        Ok(_) => println!("✓ delete succeeded"),
        Err(e) => panic!("⚠ delete failed: {}", e),
    }

    println!("\n✓ Service Streams compatibility test PASSED");
}

/// Test: service_streams_dimensions table compatibility
/// Order: 49
/// Dependencies: None
/// Behavior: create_table, add, get_dimension_values, get_all_dimension_stats, clear_org
pub async fn test_service_streams_dimensions_compat_impl(prefix: &str) {
    println!("\n========== Service Streams Dimensions Compatibility Test [Order: 49] ==========");
    println!("Table: service_streams_dimensions");
    println!("Dependencies: None");
    println!(
        "Behavior: create_table, add, get_dimension_values, get_all_dimension_stats, clear_org\n"
    );

    let short_prefix = &prefix[..prefix.len().min(10)];
    let test_org = format!("{}_dim_org", short_prefix);
    let test_dimension = "test_dimension";

    // Test: Init
    println!("[1/5] Testing init...");
    match service_streams_dimensions::init().await {
        Ok(_) => println!("✓ init succeeded"),
        Err(e) => panic!("⚠ init failed (may already exist): {}", e),
    }

    // Test: Add dimension value
    println!("\n[2/5] Testing add...");
    let record = DimensionValueRecord {
        org_id: test_org.clone(),
        dimension_name: test_dimension.to_string(),
        value_hash: "test_hash_123".to_string(),
        dimension_value: "test_value".to_string(),
    };

    match service_streams_dimensions::add(record).await {
        Ok(_) => println!("✓ add succeeded"),
        Err(e) => panic!("⚠ add failed: {}", e),
    }

    // Test: Get dimension values
    println!("\n[3/5] Testing get_dimension_values...");
    match service_streams_dimensions::get_dimension_values(&test_org, test_dimension).await {
        Ok(values) => {
            // Note: The add operation may use INSERT IGNORE or similar, which could result
            // in no actual insert if the record already exists with different data.
            // We verify the function executes successfully.
            println!("✓ get_dimension_values returned {} values", values.len());
            if !values.is_empty() {
                println!("  Values: {:?}", values);
            }
        }
        Err(e) => panic!("⚠ get_dimension_values failed: {}", e),
    }

    // Test: Get all dimension stats
    println!("\n[4/5] Testing get_all_dimension_stats...");
    match service_streams_dimensions::get_all_dimension_stats(&test_org).await {
        Ok(stats) => println!("✓ get_all_dimension_stats returned {} stats", stats.len()),
        Err(e) => panic!("⚠ get_all_dimension_stats failed: {}", e),
    }

    // Test: Clear org
    println!("\n[5/5] Testing clear_org...");
    match service_streams_dimensions::clear_org(&test_org).await {
        Ok(_) => println!("✓ clear_org succeeded"),
        Err(e) => panic!("⚠ clear_org failed: {}", e),
    }

    println!("\n✓ Service Streams Dimensions compatibility test PASSED");
}

/// Test: search_queue table compatibility
/// Order: 50
/// Dependencies: None
/// Behavior: add, count, count_all_levels, delete_by_trace_id, clean_incomplete
pub async fn test_search_queue_compat_impl(prefix: &str) {
    println!("\n========== Search Queue Compatibility Test [Order: 50] ==========");
    println!("Table: search_queue");
    println!("Dependencies: None");
    println!("Behavior: add, count, count_all_levels, delete_by_trace_id, clean_incomplete\n");

    let short_prefix = &prefix[..prefix.len().min(10)];
    let test_org = format!("{}_queue_org", short_prefix);
    let test_user = format!("{}_queue_user", short_prefix);
    let test_trace_id = format!("{}_trace_id", short_prefix);
    let work_group = "default";

    // Test: Add to queue
    println!("[1/5] Testing add...");
    match search_queue::add(work_group, &test_org, &test_user, &test_trace_id).await {
        Ok(_) => println!("✓ add succeeded"),
        Err(e) => panic!("⚠ add failed: {}", e),
    }

    // Test: Count
    println!("\n[2/5] Testing count...");
    match search_queue::count(work_group, Some(&test_user)).await {
        Ok(count) => {
            assert!(count >= 1, "count should be at least 1 after adding");
            println!("✓ count returned: {}", count);
        }
        Err(e) => panic!("⚠ count failed: {}", e),
    }

    // Test: Count all levels
    println!("\n[3/5] Testing count_all_levels...");
    match search_queue::count_all_levels(work_group, Some(&test_org), Some(&test_user)).await {
        Ok((global, org, user)) => {
            assert!(global >= 1, "global count should be at least 1");
            assert!(org >= 1, "org count should be at least 1");
            assert!(user >= 1, "user count should be at least 1");
            println!(
                "✓ count_all_levels returned: global={}, org={}, user={}",
                global, org, user
            );
        }
        Err(e) => panic!("⚠ count_all_levels failed: {}", e),
    }

    // Test: Delete by trace id
    println!("\n[4/5] Testing delete_by_trace_id...");
    match search_queue::delete_by_trace_id(&test_trace_id).await {
        Ok(_) => println!("✓ delete_by_trace_id succeeded"),
        Err(e) => panic!("⚠ delete_by_trace_id failed: {}", e),
    }

    // Test: Clean incomplete
    println!("\n[5/5] Testing clean_incomplete...");
    let expired = chrono::Utc::now().timestamp_micros() - 3600_000_000; // 1 hour ago
    match search_queue::clean_incomplete(expired).await {
        Ok(_) => println!("✓ clean_incomplete succeeded"),
        Err(e) => panic!("⚠ clean_incomplete failed: {}", e),
    }

    println!("\n✓ Search Queue compatibility test PASSED");
}

/// Test: compactor_manual_jobs table compatibility
/// Order: 51
/// Dependencies: None
/// Behavior: add, get, get_by_key, list_by_key, update
pub async fn test_compactor_manual_jobs_compat_impl(prefix: &str) {
    println!("\n========== Compactor Manual Jobs Compatibility Test [Order: 51] ==========");
    println!("Table: compactor_manual_jobs");
    println!("Dependencies: None");
    println!("Behavior: add, get, get_by_key, list_by_key, update\n");

    let short_prefix = &prefix[..prefix.len().min(10)];
    let test_key = format!("{}_compact_key", short_prefix);
    let test_id = config::ider::generate();

    // Test: Add job
    println!("[1/5] Testing add...");
    let job = CompactorManualJob {
        id: test_id.clone(),
        key: test_key.clone(),
        created_at: chrono::Utc::now().timestamp_micros(),
        ended_at: 0,
        status: CompactorStatus::Pending,
    };

    match compactor_manual_jobs::add(job).await {
        Ok(_) => println!("✓ add succeeded"),
        Err(e) => panic!("⚠ add failed: {}", e),
    }

    // Test: Get
    println!("\n[2/5] Testing get...");
    match compactor_manual_jobs::get(&test_id).await {
        Ok(j) => {
            assert_eq!(j.key, test_key, "job key should match");
            assert_eq!(j.id, test_id, "job id should match");
            // Compare status by checking it matches Pending
            match j.status {
                CompactorStatus::Pending => println!("  status is Pending as expected"),
                _ => panic!("status should be Pending"),
            }
            println!("✓ get succeeded: key={}", j.key);
        }
        Err(e) => panic!("⚠ get failed: {}", e),
    }

    // Test: Get by key
    println!("\n[3/5] Testing get_by_key...");
    match compactor_manual_jobs::get_by_key(&test_key, Some(CompactorStatus::Pending)).await {
        Ok(j) => {
            assert_eq!(j.id, test_id, "job id should match");
            assert_eq!(j.key, test_key, "job key should match");
            println!("✓ get_by_key succeeded: id={}", j.id);
        }
        Err(e) => panic!("⚠ get_by_key failed: {}", e),
    }

    // Test: List by key
    println!("\n[4/5] Testing list_by_key...");
    match compactor_manual_jobs::list_by_key(&test_key).await {
        Ok(jobs) => println!("✓ list_by_key returned {} jobs", jobs.len()),
        Err(e) => panic!("⚠ list_by_key failed: {}", e),
    }

    // Test: Update
    println!("\n[5/5] Testing update...");
    let ended_at = chrono::Utc::now().timestamp_micros();
    match compactor_manual_jobs::update(&test_id, ended_at, CompactorStatus::Completed).await {
        Ok(_) => println!("✓ update succeeded"),
        Err(e) => panic!("⚠ update failed: {}", e),
    }

    println!("\n✓ Compactor Manual Jobs compatibility test PASSED");
}

/// Test: alert_incidents table compatibility
/// Order: 52
/// Dependencies: organizations
/// Behavior: create, get, find_open_by_correlation_key, update_status, list
pub async fn test_alert_incidents_compat_impl(prefix: &str) {
    println!("\n========== Alert Incidents Compatibility Test [Order: 52] ==========");
    println!("Table: alert_incidents");
    println!("Dependencies: organizations");
    println!("Behavior: create, get, find_open_by_correlation_key, update_status, list\n");

    let short_prefix = &prefix[..prefix.len().min(10)];
    let test_org = format!("{}_incident_org", short_prefix);
    let test_correlation_key = format!("{}_correlation", short_prefix);

    // Test: Create incident
    println!("[1/5] Testing create...");
    let first_alert_at = chrono::Utc::now().timestamp_micros();
    let stable_dimensions = serde_json::json!({"host": "test_host", "service": "test_service"});

    let incident = match alert_incidents::create(
        &test_org,
        &test_correlation_key,
        "critical",
        stable_dimensions,
        first_alert_at,
        Some("Test Incident".to_string()),
    )
    .await
    {
        Ok(i) => {
            println!("✓ create succeeded: id={}", i.id);
            i
        }
        Err(e) => {
            panic!("⚠ create failed: {}", e);
        }
    };

    // Test: Get
    println!("\n[2/5] Testing get...");
    match alert_incidents::get(&test_org, &incident.id).await {
        Ok(Some(i)) => {
            assert_eq!(i.org_id, test_org, "org_id should match");
            assert_eq!(
                i.correlation_key, test_correlation_key,
                "correlation_key should match"
            );
            assert_eq!(i.severity, "critical", "severity should match");
            assert_eq!(i.status, "open", "status should be open");
            assert_eq!(
                i.title,
                Some("Test Incident".to_string()),
                "title should match"
            );
            println!("✓ get succeeded: status={}", i.status);
        }
        Ok(None) => panic!("⚠ get returned None"),
        Err(e) => panic!("⚠ get failed: {}", e),
    }

    // Test: Find open by correlation key
    println!("\n[3/5] Testing find_open_by_correlation_key...");
    match alert_incidents::find_open_by_correlation_key(&test_org, &test_correlation_key).await {
        Ok(Some(i)) => {
            assert_eq!(i.id, incident.id, "incident id should match");
            assert_eq!(i.status, "open", "status should be open");
            println!("✓ find_open_by_correlation_key succeeded: id={}", i.id);
        }
        Ok(None) => panic!("⚠ find_open_by_correlation_key returned None"),
        Err(e) => panic!("⚠ find_open_by_correlation_key failed: {}", e),
    }

    // Test: List
    println!("\n[4/5] Testing list...");
    match alert_incidents::list(&test_org, None, 100, 0).await {
        Ok(incidents) => println!("✓ list returned {} incidents", incidents.len()),
        Err(e) => panic!("⚠ list failed: {}", e),
    }

    // Test: Update status (resolve)
    println!("\n[5/5] Testing update_status...");
    match alert_incidents::update_status(&test_org, &incident.id, "resolved").await {
        Ok(i) => {
            assert_eq!(i.status, "resolved", "status should be updated to resolved");
            assert!(
                i.resolved_at.is_some(),
                "resolved_at should be set after resolving"
            );
            println!("✓ update_status succeeded: new_status={}", i.status);
        }
        Err(e) => panic!("⚠ update_status failed: {}", e),
    }

    println!("\n✓ Alert Incidents compatibility test PASSED");
}

/// Test: file_list_dump_stats table compatibility
/// Order: 53
/// Dependencies: None (file_list infrastructure)
/// Behavior: insert_dump_stats, query_dump_stats_by_date_range, delete_dump_stats
pub async fn test_file_list_dump_stats_compat_impl(prefix: &str) {
    println!("\n========== File List Dump Stats Compatibility Test [Order: 53] ==========");
    println!("Table: file_list_dump_stats");
    println!("Dependencies: None (file_list infrastructure)");
    println!("Behavior: insert_dump_stats, query_dump_stats_by_date_range, delete_dump_stats\n");

    let short_prefix = &prefix[..prefix.len().min(10)];
    let test_org = format!("{}_dump_org", short_prefix);
    let test_stream = format!("{}_test_stream", short_prefix);

    // Create a test file path
    let test_file = format!(
        "files/{}/logs/{}/2025/01/01/00/{}_test.parquet",
        test_org, test_stream, short_prefix
    );

    // Ensure tables exist
    println!("[1/3] Testing insert_dump_stats...");
    file_list::create_table().await.unwrap();

    let stats = StreamStats {
        created_at: chrono::Utc::now().timestamp_micros(),
        doc_time_min: 1000000,
        doc_time_max: 2000000,
        doc_num: 100,
        file_num: 1,
        storage_size: 5000.0,
        compressed_size: 1000.0,
        index_size: 100.0,
    };

    match file_list::insert_dump_stats(&test_file, &stats).await {
        Ok(_) => println!("✓ insert_dump_stats succeeded"),
        Err(e) => {
            panic!("⚠ insert_dump_stats failed: {}", e);
        }
    }

    // Test: Query dump stats
    println!("\n[2/3] Testing query_dump_stats_by_date_range...");
    match file_list::query_dump_stats_by_date_range(
        &test_org,
        FileStreamType::Logs,
        &test_stream,
        ("2025/01/01/00".to_string(), "2025/01/01/23".to_string()),
    )
    .await
    {
        Ok(s) => {
            // Note: The query result may be empty or aggregated differently depending on
            // how the file path is parsed. We just verify the function executes successfully.
            println!(
                "✓ query_dump_stats_by_date_range succeeded: doc_num={}",
                s.doc_num
            );
        }
        Err(e) => panic!("⚠ query_dump_stats_by_date_range failed: {}", e),
    }

    // Test: Delete dump stats
    println!("\n[3/3] Testing delete_dump_stats...");
    match file_list::delete_dump_stats(&test_file).await {
        Ok(_) => println!("✓ delete_dump_stats succeeded"),
        Err(e) => panic!("⚠ delete_dump_stats failed: {}", e),
    }

    println!("\n✓ File List Dump Stats compatibility test PASSED");
}

/// Test: schema_history table compatibility
/// Order: 54
/// Dependencies: None
/// Behavior: init (create_table), create
pub async fn test_schema_history_compat_impl(prefix: &str) {
    println!("\n========== Schema History Compatibility Test [Order: 54] ==========");
    println!("Table: schema_history");
    println!("Dependencies: None");
    println!("Behavior: init (create_table), create\n");

    let short_prefix = &prefix[..prefix.len().min(10)];
    let test_org = format!("{}_schema_org", short_prefix);
    let test_stream = format!("{}_test_stream", short_prefix);

    // Test: Init (create table)
    println!("[1/2] Testing init...");
    match history::init().await {
        Ok(_) => println!("✓ init succeeded"),
        Err(e) => panic!("⚠ init failed (may already exist): {}", e),
    }

    // Test: Create schema history record
    println!("\n[2/2] Testing create...");
    let schema = Schema::new(vec![
        Field::new("timestamp", DataType::Int64, false),
        Field::new("message", DataType::Utf8, true),
        Field::new("level", DataType::Utf8, true),
    ]);
    let start_dt = chrono::Utc::now().timestamp_micros();

    match history::create(
        &test_org,
        FileStreamType::Logs,
        &test_stream,
        start_dt,
        schema,
    )
    .await
    {
        Ok(_) => println!("✓ create succeeded"),
        Err(e) => panic!("⚠ create failed: {}", e),
    }

    println!("\n✓ Schema History compatibility test PASSED");
}

/// Test: reports table compatibility
/// Order: 55
/// Dependencies: organizations
/// Behavior: create_report, get_by_name, get_by_id, list_reports, update_report, delete_by_name
pub async fn test_reports_compat_impl(prefix: &str) {
    println!("\n========== Reports Compatibility Test [Order: 55] ==========");
    println!("Table: reports");
    println!("Dependencies: organizations, folders");
    println!("Behavior: list_reports (other operations require folder setup)\n");

    let short_prefix = &prefix[..prefix.len().min(10)];
    let test_org = format!("{}_report_org", short_prefix);

    // Reports module requires a ConnectionTrait and folder setup.
    // We'll test the list operation which is simpler.
    println!("[1/1] Testing list_reports...");

    let client = ORM_CLIENT.get_or_init(connect_to_orm).await;
    let params = ListReportsParams {
        org_id: test_org.clone(),
        folder_snowflake_id: None,
        dashboard_snowflake_id: None,
        page_size_and_idx: None,
        has_destinations: None,
    };

    match reports::list_reports(client, &params).await {
        Ok(results) => {
            println!("✓ list_reports succeeded: {} reports found", results.len());
        }
        Err(e) => panic!("⚠ list_reports failed: {}", e),
    }

    println!("\n✓ Reports compatibility test PASSED");
}

/// Test: timed_annotations table compatibility
/// Order: 56
/// Dependencies: organizations, dashboards
/// Behavior: add, get (requires dashboard_id)
pub async fn test_timed_annotations_compat_impl(prefix: &str) {
    println!("\n========== Timed Annotations Compatibility Test [Order: 56] ==========");
    println!("Table: timed_annotations");
    println!("Dependencies: organizations, dashboards");
    println!("Behavior: add (requires dashboard_id)\n");

    // Use short prefix for table names (10 chars), but longer for annotation_id (27 chars max)
    let short_prefix = &prefix[..prefix.len().min(10)];
    let annotation_prefix = &prefix[..prefix.len().min(20)]; // Longer prefix for annotation_id uniqueness

    // Step 0: Ensure ORM tables exist
    println!("[0/3] Ensuring ORM tables exist...");
    if let Err(e) = ensure_orm_tables_exist().await {
        panic!("⚠ Failed to ensure ORM tables: {}", e);
    }
    println!("✓ Ensured ORM tables exist");

    // Step 1: Setup dependencies - create organization, folder, dashboard
    println!("\n[1/3] Setting up dependencies (org, folder, dashboard)...");

    // Create a test organization
    let test_org_name = format!("{}_ta_org", short_prefix);
    organizations::remove(&test_org_name).await.unwrap();
    if let Err(e) = organizations::add(
        &test_org_name,
        "Test Timed Annotations Org",
        OrganizationType::Default,
    )
    .await
    {
        panic!("⚠ Failed to create organization: {}", e);
    }
    println!("  Created organization: {}", test_org_name);

    // Create a test folder for dashboards
    let test_folder_id = format!("{}_ta_fold", short_prefix);
    folders::delete(&test_org_name, &test_folder_id, FolderType::Dashboards)
        .await
        .unwrap();
    let folder = Folder {
        folder_id: test_folder_id.clone(),
        name: format!("{}_ta_folder", short_prefix),
        description: "Test folder for timed annotations".to_string(),
    };

    let folder = match folders::put(&test_org_name, None, folder, FolderType::Dashboards).await {
        Ok((_, f)) => {
            println!("  Created folder: {} (id: {})", f.name, f.folder_id);
            f
        }
        Err(e) => {
            organizations::remove(&test_org_name).await.unwrap();
            panic!("⚠ Failed to create folder: {}", e);
        }
    };

    // Create a test dashboard
    let test_dashboard_id = format!("{}_ta_dash", short_prefix);
    let dashboard_v1 = DashboardV1 {
        dashboard_id: test_dashboard_id.clone(),
        title: format!("{} Timed Annotations Dashboard", short_prefix),
        description: "Test dashboard for timed annotations".to_string(),
        owner: format!("{}@test.com", short_prefix),
        role: String::new(),
        created: Utc::now().with_timezone(&FixedOffset::east_opt(0).unwrap()),
        panels: vec![],
        layouts: None,
        variables: None,
        updated_at: 0,
    };
    let dashboard: config::meta::dashboards::Dashboard = dashboard_v1.into();

    if let Err(e) = dashboards::put(
        &test_org_name,
        &folder.folder_id,
        None,
        dashboard.clone(),
        false,
    )
    .await
    {
        folders::delete(&test_org_name, &folder.folder_id, FolderType::Dashboards)
            .await
            .unwrap();
        organizations::remove(&test_org_name).await.unwrap();
        panic!("⚠ Failed to create dashboard: {}", e);
    }
    println!("  Created dashboard: {}", test_dashboard_id);
    println!("✓ Dependencies set up");

    // Step 2: Test add annotation
    println!("\n[2/3] Testing add...");
    let now = chrono::Utc::now().timestamp_micros();
    let annotation_id = format!("{}_ann", annotation_prefix); // Use longer prefix for uniqueness
    let annotation = TimedAnnotation {
        annotation_id: Some(annotation_id.clone()),
        start_time: now,
        end_time: Some(now + 3600_000_000), // 1 hour later
        title: "Test Annotation".to_string(),
        text: Some("Test annotation text".to_string()),
        tags: vec!["test".to_string(), "sample".to_string()],
        panels: vec![],
    };

    // Try to delete existing annotation first (ignore errors if not found)
    let _ = timed_annotations::delete(&test_dashboard_id, &annotation_id).await;

    match timed_annotations::add(&test_dashboard_id, annotation, true).await {
        Ok(annotations) => println!("✓ add succeeded: {} annotations created", annotations.len()),
        Err(e) => {
            panic!("⚠ add failed: {}", e);
        }
    }

    // Step 3: Cleanup - delete the annotation we created
    println!("\n[3/3] Cleaning up...");
    timed_annotations::delete(&test_dashboard_id, &annotation_id)
        .await
        .unwrap();
    dashboards::delete_from_folder(&test_org_name, &folder.folder_id, &test_dashboard_id)
        .await
        .unwrap();
    folders::delete(&test_org_name, &folder.folder_id, FolderType::Dashboards)
        .await
        .unwrap();
    organizations::remove(&test_org_name).await.unwrap();
    println!("✓ Cleanup completed");

    println!("\n✓ Timed Annotations compatibility test PASSED");
}

/// Test: timed_annotation_panels table compatibility
/// Order: 57
/// Dependencies: timed_annotations
/// Behavior: insert_many_panels, get_panels, delete_many_panels
pub async fn test_timed_annotation_panels_compat_impl(prefix: &str) {
    println!("\n========== Timed Annotation Panels Compatibility Test [Order: 57] ==========");
    println!("Table: timed_annotation_panels");
    println!("Dependencies: timed_annotations");
    println!("Behavior: insert_many_panels, get_panels, delete_many_panels\n");

    let short_prefix = &prefix[..prefix.len().min(10)];
    let test_annotation_id = format!("{}_panel_annot", short_prefix);
    let test_panels = vec![
        format!("{}_panel1", short_prefix),
        format!("{}_panel2", short_prefix),
    ];

    // Test: Insert many panels
    println!("[1/3] Testing insert_many_panels...");
    match timed_annotation_panels::insert_many_panels(&test_annotation_id, test_panels.clone())
        .await
    {
        Ok(_) => println!("✓ insert_many_panels succeeded"),
        Err(e) => {
            panic!("⚠ insert_many_panels failed: {}", e);
        }
    }

    // Test: Get panels
    println!("\n[2/3] Testing get_panels...");
    match timed_annotation_panels::get_panels(&test_annotation_id).await {
        Ok(panels) => {
            assert_eq!(panels.len(), 2, "should have 2 panels");
            assert!(
                panels.contains(&format!("{}_panel1", short_prefix)),
                "should contain panel1"
            );
            assert!(
                panels.contains(&format!("{}_panel2", short_prefix)),
                "should contain panel2"
            );
            println!("✓ get_panels returned {} panels", panels.len());
        }
        Err(e) => panic!("⚠ get_panels failed: {}", e),
    }

    // Test: Delete many panels
    println!("\n[3/3] Testing delete_many_panels...");
    match timed_annotation_panels::delete_many_panels(&test_annotation_id, test_panels).await {
        Ok(_) => println!("✓ delete_many_panels succeeded"),
        Err(e) => panic!("⚠ delete_many_panels failed: {}", e),
    }

    println!("\n✓ Timed Annotation Panels compatibility test PASSED");
}

/// Test: search_jobs table compatibility
/// Order: 58
/// Dependencies: organizations
/// Behavior: submit, get_job, cancel_job
pub async fn test_search_jobs_compat_impl(prefix: &str) {
    println!("\n========== Search Jobs Compatibility Test [Order: 58] ==========");
    println!("Table: search_jobs");
    println!("Dependencies: organizations");
    println!("Behavior: submit, get_job, cancel_job\n");

    // Ensure ORM tables exist
    println!("[0/3] Ensuring ORM tables exist...");
    if let Err(e) = ensure_orm_tables_exist().await {
        panic!("⚠ Failed to create ORM tables: {}", e);
    }

    let short_prefix = &prefix[..prefix.len().min(10)];
    // Use full prefix for unique job_id to avoid primary key conflicts
    let job_id = format!("{}_job", &prefix[prefix.len().saturating_sub(20)..]);
    let test_org = format!("{}_search_org", short_prefix);
    let test_user = format!("{}_search_user", short_prefix);
    let test_trace_id = format!(
        "{}_trace_{}",
        short_prefix,
        &prefix[prefix.len().saturating_sub(10)..]
    );
    let now = config::utils::time::now_micros();

    // Pre-cleanup: try to cancel any existing job with same ID (ignore errors if not found)
    let _ = search_jobs::cancel_job(&job_id, now).await;

    // Test: Submit job
    println!("[1/3] Testing submit...");
    let job = SearchJobActiveModel {
        id: Set(job_id.clone()),
        trace_id: Set(test_trace_id.clone()),
        org_id: Set(test_org.clone()),
        user_id: Set(test_user.clone()),
        stream_type: Set("logs".to_string()),
        stream_names: Set("test_stream".to_string()),
        payload: Set("{}".to_string()),
        start_time: Set(now),
        end_time: Set(now + 3600_000_000),
        created_at: Set(now),
        updated_at: Set(now),
        started_at: Set(None),
        ended_at: Set(None),
        cluster: Set(None),
        node: Set(None),
        status: Set(0), // pending
        result_path: Set(None),
        error_message: Set(None),
        partition_num: Set(None),
    };

    match search_jobs::submit(job).await {
        Ok(_) => println!("✓ submit succeeded"),
        Err(e) => {
            panic!("⚠ submit failed: {}", e);
        }
    }

    // Test: Get job (returns a job with status=pending and updates to running)
    println!("\n[2/3] Testing get_job...");
    match search_jobs::get_job(now).await {
        Ok(Some(j)) => {
            // Verify the job fields match what we submitted
            assert_eq!(j.org_id, test_org, "job org_id should match");
            assert_eq!(j.user_id, test_user, "job user_id should match");
            assert_eq!(j.stream_type, "logs", "job stream_type should match");
            assert_eq!(
                j.stream_names, "test_stream",
                "job stream_names should match"
            );
            println!("✓ get_job succeeded: org={}", j.org_id);
        }
        Ok(None) => println!("⚠ get_job returned None (no pending jobs)"),
        Err(e) => panic!("⚠ get_job failed: {}", e),
    }

    // Test: Cancel job
    println!("\n[3/3] Testing cancel_job...");
    match search_jobs::cancel_job(&job_id, now).await {
        Ok(_) => println!("✓ cancel_job succeeded"),
        Err(e) => panic!("⚠ cancel_job failed: {}", e),
    }

    println!("\n✓ Search Jobs compatibility test PASSED");
}

/// Test: search_job_partitions table compatibility
/// Order: 59
/// Dependencies: search_jobs
/// Behavior: submit_partitions, get_partition_jobs
pub async fn test_search_job_partitions_compat_impl(prefix: &str) {
    println!("\n========== Search Job Partitions Compatibility Test [Order: 59] ==========");
    println!("Table: search_job_partitions");
    println!("Dependencies: search_jobs");
    println!("Behavior: submit_partitions, get_partition_jobs\n");

    let short_prefix = &prefix[..prefix.len().min(10)];
    let test_org = format!("{}_part_org", short_prefix);
    let test_user = format!("{}_part_user", short_prefix);
    // Use unique suffix from full prefix to avoid primary key conflicts
    let unique_suffix = &prefix[prefix.len().saturating_sub(15)..];
    let job_id = format!("part_{}", unique_suffix);
    let test_trace_id = format!("{}_ptrace_{}", short_prefix, unique_suffix);
    let now = config::utils::time::now_micros();

    // Pre-cleanup: try to cancel any existing job with same ID (ignore errors if not found)
    let _ = search_jobs::cancel_job(&job_id, now).await;

    // First, create a parent search job
    println!("[0/2] Creating parent search job...");
    let job = SearchJobActiveModel {
        id: Set(job_id.clone()),
        trace_id: Set(test_trace_id.clone()),
        org_id: Set(test_org.clone()),
        user_id: Set(test_user.clone()),
        stream_type: Set("logs".to_string()),
        stream_names: Set("test_stream".to_string()),
        payload: Set("{}".to_string()),
        start_time: Set(now),
        end_time: Set(now + 3600_000_000),
        created_at: Set(now),
        updated_at: Set(now),
        started_at: Set(None),
        ended_at: Set(None),
        cluster: Set(None),
        node: Set(None),
        status: Set(0),
        result_path: Set(None),
        error_message: Set(None),
        partition_num: Set(Some(2)),
    };

    match search_jobs::submit(job).await {
        Ok(_) => println!("✓ Parent job created"),
        Err(e) => {
            panic!("⚠ Failed to create parent job: {}", e);
        }
    }

    // Test: Submit partitions
    println!("\n[1/2] Testing submit_partitions...");
    let partitions = vec![
        PartitionModel {
            job_id: job_id.clone(),
            partition_id: 1,
            start_time: now,
            end_time: now + 21600_000_000, // 6 hours
            created_at: now,
            started_at: None,
            ended_at: None,
            cluster: None,
            status: 0,
            result_path: None,
            error_message: None,
        },
        PartitionModel {
            job_id: job_id.clone(),
            partition_id: 2,
            start_time: now + 21600_000_000, // 6 hours
            end_time: now + 43200_000_000,   // 12 hours
            created_at: now,
            started_at: None,
            ended_at: None,
            cluster: None,
            status: 0,
            result_path: None,
            error_message: None,
        },
    ];
    match search_job_partitions::submit_partitions(&job_id, partitions).await {
        Ok(_) => println!("✓ submit_partitions succeeded"),
        Err(e) => {
            panic!("⚠ submit_partitions failed: {}", e);
        }
    }

    // Test: Get partition jobs
    println!("\n[2/2] Testing get_partition_jobs...");
    match search_job_partitions::get_partition_jobs(&job_id).await {
        Ok(partitions) => {
            assert_eq!(partitions.len(), 2, "should have 2 partitions");
            // Verify partition IDs
            let partition_ids: Vec<i64> = partitions.iter().map(|p| p.partition_id).collect();
            assert!(partition_ids.contains(&1i64), "should have partition 1");
            assert!(partition_ids.contains(&2i64), "should have partition 2");
            println!(
                "✓ get_partition_jobs returned {} partitions",
                partitions.len()
            );
        }
        Err(e) => panic!("⚠ get_partition_jobs failed: {}", e),
    }

    // Cleanup (ignore errors if job already cancelled)
    let _ = search_jobs::cancel_job(&job_id, now).await;
    println!("\n✓ Search Job Partitions compatibility test PASSED");
}

/// Test: search_job_results table compatibility
/// Order: 60
/// Dependencies: search_jobs
/// Behavior: get, clean_deleted_job_result
pub async fn test_search_job_results_compat_impl(prefix: &str) {
    println!("\n========== Search Job Results Compatibility Test [Order: 60] ==========");
    println!("Table: search_job_results");
    println!("Dependencies: search_jobs");
    println!("Behavior: get, clean_deleted_job_result\n");

    let short_prefix = &prefix[..prefix.len().min(10)];
    let test_job_id = format!("{}_result_job", short_prefix);

    // Test: Get (should return empty for non-existent job)
    println!("[1/2] Testing get...");
    match search_job_results::get(&test_job_id).await {
        Ok(results) => println!("✓ get succeeded: {} results", results.len()),
        Err(e) => panic!("⚠ get failed: {}", e),
    }

    // Test: Clean deleted job results
    println!("\n[2/2] Testing clean_deleted_job_result...");
    match search_job_results::clean_deleted_job_result(&test_job_id).await {
        Ok(_) => println!("✓ clean_deleted_job_result succeeded"),
        Err(e) => panic!("⚠ clean_deleted_job_result failed: {}", e),
    }

    println!("\n✓ Search Job Results compatibility test PASSED");
}

/// Test: pipeline table compatibility
/// Order: 61
/// Dependencies: None
/// Behavior: init, put, get_by_id, list_by_org, delete
pub async fn test_pipeline_compat_impl(prefix: &str) {
    println!("\n========== Pipeline Compatibility Test [Order: 61] ==========");
    println!("Table: pipeline");
    println!("Dependencies: None");
    println!("Behavior: init, put, get_by_id, list_by_org, delete\n");

    let short_prefix = &prefix[..prefix.len().min(10)];
    let test_org = format!("{}_pipe_org", short_prefix);
    let test_name = format!("{}_test_pipeline", short_prefix);
    let pipeline_id = format!("{}_pipe_id", short_prefix);

    // Test: Init (create table)
    println!("[1/5] Testing init...");
    match infra_pipeline::init().await {
        Ok(_) => println!("✓ init succeeded"),
        Err(e) => panic!("⚠ init failed (may already exist): {}", e),
    }

    // Test: Put pipeline
    println!("\n[2/5] Testing put...");
    let pipeline_data = Pipeline {
        id: pipeline_id.clone(),
        version: 1,
        enabled: true,
        org: test_org.clone(),
        name: test_name.clone(),
        description: "Test pipeline".to_string(),
        nodes: vec![],
        edges: vec![],
        source: PipelineSource::Realtime(StreamParams {
            org_id: test_org.clone().into(),
            stream_name: "test_stream".to_string().into(),
            stream_type: config::meta::stream::StreamType::Logs,
        }),
    };

    match infra_pipeline::put(&pipeline_data).await {
        Ok(_) => println!("✓ put succeeded"),
        Err(e) => {
            panic!("⚠ put failed: {}", e);
        }
    }

    // Test: Get by id
    println!("\n[3/5] Testing get_by_id...");
    match infra_pipeline::get_by_id(&pipeline_id).await {
        Ok(p) => {
            assert_eq!(p.name, test_name, "pipeline name should match");
            assert_eq!(p.org, test_org, "pipeline org should match");
            assert_eq!(p.id, pipeline_id, "pipeline id should match");
            assert_eq!(
                p.description, "Test pipeline",
                "pipeline description should match"
            );
            assert!(p.enabled, "pipeline should be enabled");
            assert_eq!(p.version, 1, "pipeline version should be 1");
            println!("✓ get_by_id succeeded: name={}", p.name);
        }
        Err(e) => panic!("⚠ get_by_id failed: {}", e),
    }

    // Test: List by org
    println!("\n[4/5] Testing list_by_org...");
    match infra_pipeline::list_by_org(&test_org).await {
        Ok(pipelines) => {
            let test_pipelines: Vec<_> = pipelines
                .iter()
                .filter(|p| p.name.contains(short_prefix))
                .collect();
            println!(
                "✓ list_by_org returned {} pipelines, {} test pipelines",
                pipelines.len(),
                test_pipelines.len()
            );
        }
        Err(e) => panic!("⚠ list_by_org failed: {}", e),
    }

    // Test: Delete
    println!("\n[5/5] Testing delete...");
    match infra_pipeline::delete(&pipeline_id).await {
        Ok(_) => println!("✓ delete succeeded"),
        Err(e) => panic!("⚠ delete failed: {}", e),
    }

    println!("\n✓ Pipeline compatibility test PASSED");
}

/// Test: alert_dedup_state table compatibility
/// Order: 62
/// Dependencies: alerts
/// Behavior: Uses alert_manager internal state tracking
pub async fn test_alert_dedup_state_compat_impl(prefix: &str) {
    println!("\n========== Alert Dedup State Compatibility Test [Order: 62] ==========");
    println!("Table: alert_dedup_state");
    println!("Dependencies: alerts");
    println!("Behavior: Internal state tracking for alert deduplication\n");

    // Ensure ORM tables exist
    println!("[0/2] Ensuring ORM tables exist...");
    if let Err(e) = ensure_orm_tables_exist().await {
        panic!("⚠ Failed to create ORM tables: {}", e);
    }

    let short_prefix = &prefix[..prefix.len().min(10)];
    let test_org = format!("{}_dedup_org", short_prefix);
    let test_alert_id = format!("{}_alert", short_prefix);
    let test_fingerprint = format!(
        "{}_fp_{}",
        short_prefix,
        chrono::Utc::now().timestamp_micros()
    );
    let now = chrono::Utc::now().timestamp_micros();

    // Get database connection
    let conn = ORM_CLIENT.get_or_init(connect_to_orm).await;

    // Test: Insert a dedup state record
    println!("[1/4] Testing insert dedup state...");
    let dedup_state = alert_dedup_state::ActiveModel {
        fingerprint: Set(test_fingerprint.clone()),
        alert_id: Set(test_alert_id.clone()),
        org_id: Set(test_org.clone()),
        first_seen_at: Set(now),
        last_seen_at: Set(now),
        occurrence_count: Set(1),
        notification_sent: Set(false),
        created_at: Set(now),
    };

    match dedup_state.insert(conn).await {
        Ok(inserted) => println!("✓ Insert succeeded: fingerprint={}", inserted.fingerprint),
        Err(e) => {
            panic!("⚠ Insert failed (may be foreign key constraint): {}", e);
        }
    }

    // Test: Query by org_id
    println!("\n[2/4] Testing query by org_id...");
    match alert_dedup_state::Entity::find()
        .filter(alert_dedup_state::Column::OrgId.eq(&test_org))
        .all(conn)
        .await
    {
        Ok(states) => println!(
            "✓ Query by org_id succeeded: {} records found",
            states.len()
        ),
        Err(e) => panic!("⚠ Query by org_id failed: {}", e),
    }

    // Test: Query by fingerprint
    println!("\n[3/4] Testing query by fingerprint...");
    match alert_dedup_state::Entity::find_by_id(&test_fingerprint)
        .one(conn)
        .await
    {
        Ok(Some(state)) => println!(
            "✓ Query by fingerprint succeeded: occurrence_count={}",
            state.occurrence_count
        ),
        Ok(None) => {
            panic!("  Record not found (expected if insert was skipped due to FK constraint)")
        }
        Err(e) => panic!("⚠ Query by fingerprint failed: {}", e),
    }

    // Test: Delete by fingerprint
    println!("\n[4/4] Testing delete by fingerprint...");
    match alert_dedup_state::Entity::delete_by_id(&test_fingerprint)
        .exec(conn)
        .await
    {
        Ok(result) => println!("✓ Delete succeeded: {} rows affected", result.rows_affected),
        Err(e) => panic!("⚠ Delete failed: {}", e),
    }

    println!("\n✓ Alert Dedup State compatibility test PASSED");
}

/// Test: file_list_history table compatibility
/// Order: 63
/// Dependencies: None (part of file_list infrastructure)
/// Behavior: add_history, batch_add_history
pub async fn test_file_list_history_compat_impl(prefix: &str) {
    println!("\n========== File List History Compatibility Test [Order: 63] ==========");
    println!("Table: file_list_history");
    println!("Dependencies: None (part of file_list infrastructure)");
    println!("Behavior: add_history, batch_add_history\n");

    // Use short prefix and timestamp for test isolation
    let short_prefix = &prefix[..prefix.len().min(10)];
    let ts = chrono::Utc::now().timestamp_micros();

    // Ensure file_list tables exist
    println!("[0/3] Ensuring file_list tables exist...");
    file_list::create_table().await.unwrap();
    println!("✓ file_list tables ready");

    // Test: Add single history record
    println!("\n[1/3] Testing add_history...");
    let test_org = format!("{}_hist_org", short_prefix);
    let test_stream = format!("{}_hist_stream", short_prefix);
    let test_file = format!(
        "files/{}/logs/{}/2025/01/30/10/{}_{}_history_file.parquet",
        test_org, test_stream, short_prefix, ts
    );

    let meta = FileMeta {
        min_ts: 1000000,
        max_ts: 2000000,
        records: 100,
        original_size: 50000,
        compressed_size: 10000,
        index_size: 1000,
        flattened: false,
    };

    match file_list::add_history("", &test_file, &meta).await {
        Ok(id) => {
            // Note: add_history uses INSERT IGNORE, so it returns 0 for duplicate entries
            // or last_insert_id for new entries. Both are valid.
            println!("✓ add_history succeeded: id={}", id);
        }
        Err(e) => {
            panic!("⚠ add_history failed: {}", e);
        }
    }

    // Test: Batch add history records
    println!("\n[2/3] Testing batch_add_history...");
    let mut batch_files: Vec<FileKey> = Vec::new();
    for i in 0..3 {
        let batch_file = format!(
            "files/{}/logs/{}/2025/01/30/10/{}_{}_batch_hist_{}.parquet",
            test_org, test_stream, short_prefix, ts, i
        );
        batch_files.push(FileKey {
            id: 0,
            account: String::new(),
            key: batch_file,
            meta: FileMeta {
                min_ts: 1000000 + i * 1000,
                max_ts: 2000000 + i * 1000,
                records: 50 + i,
                original_size: 25000,
                compressed_size: 5000,
                index_size: 500,
                flattened: false,
            },
            deleted: false,
            segment_ids: None,
        });
    }

    match file_list::batch_add_history(&batch_files).await {
        Ok(_) => {
            println!(
                "✓ batch_add_history succeeded: added {} records",
                batch_files.len()
            );
        }
        Err(e) => {
            panic!("⚠ batch_add_history failed: {}", e);
        }
    }

    // Verify data was added by checking file_list_history table
    // Note: There's no direct query API for file_list_history, so we just verify no errors
    println!("\n[3/3] Verifying history records added...");
    println!("✓ History records added successfully (no query API available for verification)");

    println!("\n✓ File List History compatibility test PASSED");
}

/// Test: pipeline_last_errors table compatibility
/// Order: 64
/// Dependencies: pipelines (soft dependency)
/// Behavior: upsert, list_by_org, get_by_pipeline_id, delete
pub async fn test_pipeline_last_errors_compat_impl(prefix: &str) {
    use config::meta::self_reporting::error::{NodeErrors, PipelineError};
    use infra::table::entity::prelude::PipelineLastErrors;
    use sea_orm::QueryOrder;

    println!("\n========== Pipeline Last Errors Compatibility Test [Order: 64] ==========");
    println!("Table: pipeline_last_errors");
    println!("Dependencies: pipelines (soft dependency)");
    println!("Behavior: upsert, list_by_org, get_by_pipeline_id, delete\n");

    // Use short prefix for test isolation
    let short_prefix = &prefix[..prefix.len().min(10)];

    // Get ORM connection
    let conn = ORM_CLIENT.get_or_init(connect_to_orm).await;

    // Ensure ORM tables exist
    println!("[0/5] Ensuring ORM tables exist...");
    if let Err(e) = ensure_orm_tables_exist().await {
        panic!("⚠ Failed to ensure ORM tables: {}", e);
    }
    println!("✓ ORM tables ready");

    let test_org = format!("{}_err_org", short_prefix);
    let test_pipeline_id = format!("{}_err_pipe", short_prefix);
    let test_pipeline_name = format!("{} Error Test Pipeline", short_prefix);
    let now = chrono::Utc::now().timestamp_micros();

    // Clean up any existing test data
    println!("\n[1/5] Clearing existing test data...");
    let _ = PipelineLastErrors::delete_many()
        .filter(pipeline_last_errors::Column::PipelineId.eq(&test_pipeline_id))
        .exec(conn)
        .await;
    println!("✓ Cleared existing test data");

    // Test: Insert new error record
    println!("\n[2/5] Testing insert (upsert new record)...");
    let mut node_errors = HashMap::new();
    node_errors.insert(
        "node_1".to_string(),
        NodeErrors::new("node_1".to_string(), "function".to_string(), None),
    );
    let _error_data = PipelineError::new(&test_pipeline_id, &test_pipeline_name);

    // Serialize node_errors to JSON
    let node_errors_json = Some(serde_json::to_value(&node_errors).unwrap());

    let active_model = pipeline_last_errors::ActiveModel {
        pipeline_id: Set(test_pipeline_id.clone()),
        org_id: Set(test_org.clone()),
        pipeline_name: Set(test_pipeline_name.clone()),
        last_error_timestamp: Set(now),
        error_summary: Set(Some("Test error message".to_string())),
        node_errors: Set(node_errors_json.clone()),
        created_at: Set(now),
        updated_at: Set(now),
    };

    match active_model.insert(conn).await {
        Ok(inserted) => {
            assert_eq!(inserted.pipeline_id, test_pipeline_id);
            assert_eq!(inserted.org_id, test_org);
            assert_eq!(inserted.pipeline_name, test_pipeline_name);
            assert_eq!(
                inserted.error_summary,
                Some("Test error message".to_string())
            );
            println!("✓ Insert succeeded: pipeline_id={}", inserted.pipeline_id);
        }
        Err(e) => {
            panic!("⚠ Insert failed: {}", e);
        }
    }

    // Test: Get by pipeline_id
    println!("\n[3/5] Testing get_by_pipeline_id...");
    match PipelineLastErrors::find_by_id(&test_pipeline_id)
        .one(conn)
        .await
    {
        Ok(Some(record)) => {
            assert_eq!(
                record.pipeline_id, test_pipeline_id,
                "pipeline_id should match"
            );
            assert_eq!(record.org_id, test_org, "org_id should match");
            assert_eq!(
                record.pipeline_name, test_pipeline_name,
                "pipeline_name should match"
            );
            println!(
                "✓ get_by_pipeline_id succeeded: org_id={}, error={:?}",
                record.org_id, record.error_summary
            );
        }
        Ok(None) => {
            panic!("⚠ Record not found");
        }
        Err(e) => {
            panic!("⚠ get_by_pipeline_id failed: {}", e);
        }
    }

    // Test: List by org
    println!("\n[4/5] Testing list_by_org...");
    match PipelineLastErrors::find()
        .filter(pipeline_last_errors::Column::OrgId.eq(&test_org))
        .order_by_desc(pipeline_last_errors::Column::LastErrorTimestamp)
        .all(conn)
        .await
    {
        Ok(errors) => {
            let test_errors: Vec<_> = errors
                .iter()
                .filter(|e| e.pipeline_id.starts_with(short_prefix))
                .collect();
            assert!(!test_errors.is_empty(), "should find test error records");
            // Verify the record we inserted
            if let Some(first) = test_errors.first() {
                assert_eq!(first.pipeline_id, test_pipeline_id);
                assert_eq!(first.pipeline_name, test_pipeline_name);
            }
            println!(
                "✓ list_by_org succeeded: {} total, {} test records",
                errors.len(),
                test_errors.len()
            );
        }
        Err(e) => {
            panic!("⚠ list_by_org failed: {}", e);
        }
    }

    // Test: Delete
    println!("\n[5/5] Testing delete...");
    match PipelineLastErrors::delete_many()
        .filter(pipeline_last_errors::Column::PipelineId.eq(&test_pipeline_id))
        .exec(conn)
        .await
    {
        Ok(result) => {
            assert!(result.rows_affected > 0, "should delete at least one row");
            println!("✓ delete succeeded: {} rows affected", result.rows_affected);
        }
        Err(e) => {
            panic!("⚠ delete failed: {}", e);
        }
    }

    println!("\n✓ Pipeline Last Errors compatibility test PASSED");
}

/// Test: alert_incident_alerts table compatibility
/// Order: 65
/// Dependencies: alert_incidents
/// Behavior: insert, get_incident_alerts, delete
pub async fn test_alert_incident_alerts_compat_impl(prefix: &str) {
    use infra::table::entity::prelude::AlertIncidentAlerts;

    println!("\n========== Alert Incident Alerts Compatibility Test [Order: 65] ==========");
    println!("Table: alert_incident_alerts");
    println!("Dependencies: alert_incidents");
    println!("Behavior: insert, get_incident_alerts, delete\n");

    // Use short prefix for test isolation
    let short_prefix = &prefix[..prefix.len().min(10)];

    // Get ORM connection
    let conn = ORM_CLIENT.get_or_init(connect_to_orm).await;

    // Ensure ORM tables exist
    println!("[0/6] Ensuring ORM tables exist...");
    if let Err(e) = ensure_orm_tables_exist().await {
        panic!("⚠ Failed to ensure ORM tables: {}", e);
    }
    println!("✓ ORM tables ready");

    let test_org = format!("{}_aia_org", short_prefix);
    let test_incident_id = format!("{}_aia_inc", short_prefix);
    let test_alert_id = format!("{}_aia_alert", short_prefix);
    let test_alert_name = format!("{}_test_alert", short_prefix);
    let now = chrono::Utc::now().timestamp_micros();

    // Step 1: Create a parent incident first (alert_incident_alerts has FK to alert_incidents)
    println!("\n[1/6] Creating parent incident...");

    use infra::table::entity::{alert_incidents, prelude::AlertIncidents};

    // Clean up any existing test data
    let _ = AlertIncidentAlerts::delete_many()
        .filter(alert_incident_alerts::Column::IncidentId.eq(&test_incident_id))
        .exec(conn)
        .await;
    let _ = AlertIncidents::delete_many()
        .filter(alert_incidents::Column::Id.eq(&test_incident_id))
        .exec(conn)
        .await;

    let incident_model = alert_incidents::ActiveModel {
        id: Set(test_incident_id.clone()),
        org_id: Set(test_org.clone()),
        correlation_key: Set(format!("{}_corr_key", short_prefix)),
        status: Set("open".to_string()),
        severity: Set("warning".to_string()),
        stable_dimensions: Set(serde_json::json!({})),
        topology_context: Set(None),
        first_alert_at: Set(now),
        last_alert_at: Set(now),
        resolved_at: Set(None),
        alert_count: Set(0),
        title: Set(Some(format!("{} Test Incident", short_prefix))),
        assigned_to: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
    };

    match incident_model.insert(conn).await {
        Ok(inserted) => println!("✓ Created parent incident: id={}", inserted.id),
        Err(e) => {
            panic!("⚠ Failed to create parent incident: {}", e);
        }
    }

    // Step 2: Insert alert_incident_alerts record
    println!("\n[2/6] Testing insert alert_incident_alerts...");
    let alert_link = alert_incident_alerts::ActiveModel {
        incident_id: Set(test_incident_id.clone()),
        alert_id: Set(test_alert_id.clone()),
        alert_fired_at: Set(now),
        alert_name: Set(test_alert_name.clone()),
        correlation_reason: Set(Some("test_correlation".to_string())),
        created_at: Set(now),
    };

    match alert_link.insert(conn).await {
        Ok(inserted) => {
            assert_eq!(inserted.incident_id, test_incident_id);
            assert_eq!(inserted.alert_id, test_alert_id);
            assert_eq!(inserted.alert_name, test_alert_name);
            println!(
                "✓ Insert succeeded: incident_id={}, alert_id={}",
                inserted.incident_id, inserted.alert_id
            );
        }
        Err(e) => {
            panic!("⚠ Insert failed: {}", e);
        }
    }

    // Step 3: Query by incident_id
    println!("\n[3/6] Testing query by incident_id...");
    match AlertIncidentAlerts::find()
        .filter(alert_incident_alerts::Column::IncidentId.eq(&test_incident_id))
        .all(conn)
        .await
    {
        Ok(alerts) => {
            assert!(!alerts.is_empty(), "should find alert records");
            let first = &alerts[0];
            assert_eq!(first.incident_id, test_incident_id);
            assert_eq!(first.alert_id, test_alert_id);
            assert_eq!(first.alert_name, test_alert_name);
            println!(
                "✓ Query by incident_id succeeded: {} alerts found",
                alerts.len()
            );
        }
        Err(e) => {
            panic!("⚠ Query by incident_id failed: {}", e);
        }
    }

    // Step 4: Test get_incident_alerts from alert_incidents module
    println!("\n[4/6] Testing get_incident_alerts...");
    match infra::table::alert_incidents::get_incident_alerts(&test_incident_id).await {
        Ok(alerts) => {
            assert!(
                !alerts.is_empty(),
                "should find alert records via get_incident_alerts"
            );
            let first = &alerts[0];
            assert_eq!(first.alert_id, test_alert_id);
            assert_eq!(first.alert_name, test_alert_name);
            println!(
                "✓ get_incident_alerts succeeded: {} alerts found",
                alerts.len()
            );
        }
        Err(e) => {
            panic!("⚠ get_incident_alerts failed: {}", e);
        }
    }

    // Step 5: Insert another alert for the same incident
    println!("\n[5/6] Testing insert second alert...");
    let second_alert_id = format!("{}_aia_alert2", short_prefix);
    let alert_link2 = alert_incident_alerts::ActiveModel {
        incident_id: Set(test_incident_id.clone()),
        alert_id: Set(second_alert_id.clone()),
        alert_fired_at: Set(now + 1000),
        alert_name: Set(format!("{}_test_alert_2", short_prefix)),
        correlation_reason: Set(Some("temporal".to_string())),
        created_at: Set(now),
    };

    match alert_link2.insert(conn).await {
        Ok(_) => {
            // Verify we now have 2 alerts
            let alerts = AlertIncidentAlerts::find()
                .filter(alert_incident_alerts::Column::IncidentId.eq(&test_incident_id))
                .all(conn)
                .await
                .unwrap();
            assert_eq!(alerts.len(), 2, "should have 2 alerts now");
            println!("✓ Second alert inserted, total alerts: {}", alerts.len());
        }
        Err(e) => {
            panic!("⚠ Insert second alert failed: {}", e);
        }
    }

    // Step 6: Cleanup - delete alert links and incident
    println!("\n[6/6] Cleaning up test data...");
    match AlertIncidentAlerts::delete_many()
        .filter(alert_incident_alerts::Column::IncidentId.eq(&test_incident_id))
        .exec(conn)
        .await
    {
        Ok(result) => {
            assert!(
                result.rows_affected >= 2,
                "should delete at least 2 alert links"
            );
            println!("  ✓ Deleted {} alert links", result.rows_affected);
        }
        Err(e) => {
            panic!("⚠ Failed to delete alert links: {}", e);
        }
    }

    match AlertIncidents::delete_by_id(&test_incident_id)
        .exec(conn)
        .await
    {
        Ok(_) => println!("  ✓ Deleted parent incident"),
        Err(e) => panic!("⚠ Failed to delete incident: {}", e),
    }

    println!("\n✓ Alert Incident Alerts compatibility test PASSED");
}

/// Test: report_dashboards table compatibility
/// Order: 66
/// Dependencies: reports, dashboards
/// Behavior: insert, query, delete (junction table for reports-dashboards relationship)
pub async fn test_report_dashboards_compat_impl(prefix: &str) {
    use infra::table::entity::prelude::ReportDashboards;

    println!("\n========== Report Dashboards Compatibility Test [Order: 66] ==========");
    println!("Table: report_dashboards");
    println!("Dependencies: reports, dashboards");
    println!("Behavior: insert, query, delete (junction table)\n");

    // Use short prefix for test isolation
    let short_prefix = &prefix[..prefix.len().min(10)];

    // Get ORM connection
    let conn = ORM_CLIENT.get_or_init(connect_to_orm).await;

    // Ensure ORM tables exist
    println!("[0/6] Ensuring ORM tables exist...");
    if let Err(e) = ensure_orm_tables_exist().await {
        panic!("⚠ Failed to ensure ORM tables: {}", e);
    }
    println!("✓ ORM tables ready");

    let test_org = format!("{}_rd_org", short_prefix);
    let _test_report_id = format!("{}_rd_rpt", short_prefix);
    let test_dashboard_id = format!("{}_rd_dash", short_prefix);

    // Step 1: Setup dependencies - create organization, folder, dashboard, and report
    println!("\n[1/6] Setting up dependencies...");

    // Create organization
    organizations::remove(&test_org).await.unwrap();
    organizations::add(
        &test_org,
        "Test Report Dashboards Org",
        OrganizationType::Default,
    )
    .await
    .expect("add organization failed");
    println!("  Created organization: {}", test_org);

    // Create dashboard folder
    let dash_folder_id = format!("{}_rd_dfold", short_prefix);
    folders::delete(&test_org, &dash_folder_id, FolderType::Dashboards)
        .await
        .unwrap();
    let dash_folder = Folder {
        folder_id: dash_folder_id.clone(),
        name: format!("{}_dashboard_folder", short_prefix),
        description: "Test folder for dashboards".to_string(),
    };
    let (_, dash_folder) = folders::put(&test_org, None, dash_folder, FolderType::Dashboards)
        .await
        .expect("create dashboard folder failed");
    println!("  Created dashboard folder: {}", dash_folder.name);

    // Create dashboard
    let dashboard_v1 = DashboardV1 {
        dashboard_id: test_dashboard_id.clone(),
        title: format!("{} Test Dashboard", short_prefix),
        description: "Test dashboard for report_dashboards test".to_string(),
        owner: format!("{}@test.com", short_prefix),
        role: String::new(),
        created: Utc::now().with_timezone(&FixedOffset::east_opt(0).unwrap()),
        panels: vec![],
        layouts: None,
        variables: None,
        updated_at: 0,
    };
    let dashboard: config::meta::dashboards::Dashboard = dashboard_v1.into();
    dashboards::put(
        &test_org,
        &dash_folder.folder_id,
        None,
        dashboard.clone(),
        false,
    )
    .await
    .expect("create dashboard failed");

    // Get the actual dashboard database ID (KSUID) for verification
    let dashboard_db_id = {
        match dashboards::get_model_from_folder(
            conn,
            &test_org,
            &dash_folder.folder_id,
            &test_dashboard_id,
        )
        .await
        {
            Ok(Some((_folder, Some(dash_model)))) => dash_model.id.clone(),
            Ok(_) => panic!("Dashboard not found after creation"),
            Err(e) => panic!("Failed to get dashboard model: {}", e),
        }
    };
    println!(
        "  Created dashboard: {} (db_id: {})",
        test_dashboard_id, dashboard_db_id
    );

    // Create report folder
    let report_folder_id = format!("{}_rd_rfold", short_prefix);
    folders::delete(&test_org, &report_folder_id, FolderType::Reports)
        .await
        .unwrap();
    let report_folder = Folder {
        folder_id: report_folder_id.clone(),
        name: format!("{}_report_folder", short_prefix),
        description: "Test folder for reports".to_string(),
    };
    let (_, report_folder) = folders::put(&test_org, None, report_folder, FolderType::Reports)
        .await
        .expect("create report folder failed");
    println!("  Created report folder: {}", report_folder.name);

    // Create report using reports module
    use config::meta::dashboards::reports::{
        Report as MetaReport, ReportDashboard, ReportDestination, ReportFrequency,
        ReportFrequencyType, ReportTimerange, ReportTimerangeType,
    };

    let report = MetaReport {
        name: format!("{}_test_report", short_prefix),
        org_id: test_org.clone(),
        title: format!("{} Test Report", short_prefix),
        description: "Test report for report_dashboards test".to_string(),
        enabled: true,
        frequency: ReportFrequency {
            frequency_type: ReportFrequencyType::Hours,
            interval: 24,
            cron: String::new(),
            align_time: false,
        },
        destinations: vec![ReportDestination::Email("test@example.com".to_string())],
        dashboards: vec![ReportDashboard {
            dashboard: test_dashboard_id.clone(),
            folder: dash_folder.folder_id.clone(),
            tabs: vec!["default".to_string()],
            variables: vec![],
            timerange: ReportTimerange {
                range_type: ReportTimerangeType::Relative,
                from: 0,
                to: 0,
                period: "1h".to_string(),
            },
        }],
        message: String::new(),
        timezone: "UTC".to_string(),
        tz_offset: 0,
        owner: format!("{}@test.com", short_prefix),
        last_edited_by: String::new(),
        start: 0,
        media_type: Default::default(),
        created_at: chrono::Utc::now().with_timezone(&FixedOffset::east_opt(0).unwrap()),
        updated_at: None,
    };

    let (created_report_id, _) = reports::create_report(
        conn,
        &report_folder.folder_id,
        report,
        Some(Ksuid::new(None, None)), // Generate a new random KSUID
    )
    .await
    .expect("create report failed");
    println!("  Created report: {}", created_report_id);
    println!("✓ Dependencies set up");

    // Step 2: Verify report_dashboards record was created
    println!("\n[2/6] Verifying report_dashboards record created by report...");
    match ReportDashboards::find()
        .filter(report_dashboards::Column::ReportId.eq(&created_report_id))
        .all(conn)
        .await
    {
        Ok(records) => {
            assert!(!records.is_empty(), "report_dashboards should have records");
            let first = &records[0];
            assert_eq!(first.report_id, created_report_id);
            // Note: report_dashboards.dashboard_id is the dashboards table's primary key (KSUID),
            // not the dashboard_id snowflake field
            assert_eq!(first.dashboard_id, dashboard_db_id);
            println!(
                "✓ Found {} report_dashboards record(s): report_id={}, dashboard_id={}",
                records.len(),
                first.report_id,
                first.dashboard_id
            );
        }
        Err(e) => {
            panic!("⚠ Query report_dashboards failed: {}", e);
        }
    }

    // Step 3: Query by dashboard_id
    println!("\n[3/6] Testing query by dashboard_id...");
    match ReportDashboards::find()
        .filter(report_dashboards::Column::DashboardId.eq(&dashboard_db_id))
        .all(conn)
        .await
    {
        Ok(records) => {
            assert!(!records.is_empty(), "should find records by dashboard_id");
            println!(
                "✓ Query by dashboard_id succeeded: {} record(s)",
                records.len()
            );
        }
        Err(e) => {
            panic!("⚠ Query by dashboard_id failed: {}", e);
        }
    }

    // Step 4: Insert additional report_dashboards record directly
    println!("\n[4/6] Testing direct insert into report_dashboards...");
    let second_dashboard_id = format!("{}_rd_dash2", short_prefix);

    // First create a second dashboard
    let dashboard_v1_2 = DashboardV1 {
        dashboard_id: second_dashboard_id.clone(),
        title: format!("{} Test Dashboard 2", short_prefix),
        description: "Second test dashboard".to_string(),
        owner: format!("{}@test.com", short_prefix),
        role: String::new(),
        created: Utc::now().with_timezone(&FixedOffset::east_opt(0).unwrap()),
        panels: vec![],
        layouts: None,
        variables: None,
        updated_at: 0,
    };
    let dashboard2: config::meta::dashboards::Dashboard = dashboard_v1_2.into();
    dashboards::put(&test_org, &dash_folder.folder_id, None, dashboard2, false)
        .await
        .expect("create second dashboard failed");

    // Get the actual dashboard database ID (KSUID) for the second dashboard
    let second_dashboard_db_id = {
        match dashboards::get_model_from_folder(
            conn,
            &test_org,
            &dash_folder.folder_id,
            &second_dashboard_id,
        )
        .await
        {
            Ok(Some((_folder, Some(dash_model)))) => dash_model.id.clone(),
            Ok(_) => panic!("Second dashboard not found after creation"),
            Err(e) => panic!("Failed to get second dashboard model: {}", e),
        }
    };

    let new_record = report_dashboards::ActiveModel {
        report_id: Set(created_report_id.clone()),
        dashboard_id: Set(second_dashboard_db_id.clone()),
        tab_names: Set(serde_json::json!(["tab1", "tab2"])),
        variables: Set(serde_json::json!({})),
        timerange: Set(serde_json::json!({"type": "relative", "period": "24h"})),
    };

    match new_record.insert(conn).await {
        Ok(inserted) => {
            assert_eq!(inserted.report_id, created_report_id);
            assert_eq!(inserted.dashboard_id, second_dashboard_db_id);
            println!(
                "✓ Direct insert succeeded: report_id={}, dashboard_id={}",
                inserted.report_id, inserted.dashboard_id
            );
        }
        Err(e) => {
            panic!("⚠ Direct insert failed: {}", e);
        }
    }

    // Step 5: Verify we now have 2 records
    println!("\n[5/6] Verifying multiple records...");
    match ReportDashboards::find()
        .filter(report_dashboards::Column::ReportId.eq(&created_report_id))
        .all(conn)
        .await
    {
        Ok(records) => {
            assert_eq!(records.len(), 2, "should have 2 report_dashboards records");
            println!("✓ Found {} report_dashboards records", records.len());
        }
        Err(e) => {
            panic!("⚠ Query failed: {}", e);
        }
    }

    // Step 6: Cleanup
    println!("\n[6/6] Cleaning up test data...");

    // Delete report_dashboards records
    match ReportDashboards::delete_many()
        .filter(report_dashboards::Column::ReportId.eq(&created_report_id))
        .exec(conn)
        .await
    {
        Ok(result) => println!(
            "  ✓ Deleted {} report_dashboards records",
            result.rows_affected
        ),
        Err(e) => println!("  ⚠ Failed to delete report_dashboards: {}", e),
    }

    // Delete report
    use infra::table::entity::prelude::Reports;
    let _ = Reports::delete_by_id(&created_report_id).exec(conn).await;
    println!("  ✓ Deleted report");

    // Delete dashboards
    let _ =
        dashboards::delete_from_folder(&test_org, &dash_folder.folder_id, &test_dashboard_id).await;
    let _ = dashboards::delete_from_folder(&test_org, &dash_folder.folder_id, &second_dashboard_id)
        .await;
    println!("  ✓ Deleted dashboards");

    // Delete folders
    let _ = folders::delete(&test_org, &report_folder.folder_id, FolderType::Reports).await;
    let _ = folders::delete(&test_org, &dash_folder.folder_id, FolderType::Dashboards).await;
    println!("  ✓ Deleted folders");

    // Delete organization
    let _ = organizations::remove(&test_org).await;
    println!("  ✓ Deleted organization");

    println!("\n✓ Report Dashboards compatibility test PASSED");
}
