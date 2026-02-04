// Copyright 2025 OpenObserve Inc.

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

//! ```bash
//! # Run all tests with OceanBase+NATS backend (requires db-oceanbase-nats-tests feature)
//! ZO_TEST_OCEANBASE_DSN="mysql://root@127.0.0.1:2881/openobserve_test" ZO_NATS_ADDR="127.0.0.1:4222" \
//! cargo test --test db_oceanbase_nats_table_compat_tests --features db-oceanbase-nats-tests -- --test-threads=1 --nocapture
//!
//! # Run a single test (--test-threads=1 is optional for single tests)
//! ZO_TEST_OCEANBASE_DSN="mysql://root@127.0.0.1:2881/openobserve_test" ZO_NATS_ADDR="127.0.0.1:4222" \
//! cargo test --test db_oceanbase_nats_table_compat_tests --features db-oceanbase-nats-tests test_oceanbase_nats_organizations_compat -- --test-threads=1 --nocapture
//! ```

#![cfg(feature = "db-oceanbase-nats-tests")]

mod common;

use common::{
    db_helpers::{RealOceanBaseInstance, init_config_for_oceanbase_nats_tests},
    table_compat_tests_impl,
};
use once_cell::sync::Lazy;

// ==================== Global Runtime ====================

static TEST_RUNTIME: Lazy<tokio::runtime::Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .expect("Failed to create test runtime")
});

/// Generate a unique test prefix based on timestamp
async fn generate_test_prefix() -> String {
    let _ = RealOceanBaseInstance::new(None).await; // Ensures schema and truncation
    format!(
        "test_ob_nats_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros()
    )
}

// ==================== Level 0: Base Table Tests ====================

/// Test organizations table compatibility with OceanBase+NATS
/// Order: 01, Dependencies: None
#[test]
fn test_oceanbase_nats_organizations_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_organizations_compat_impl(&prefix).await;
    });
}

/// Test users table compatibility with OceanBase+NATS
/// Order: 02, Dependencies: None
#[test]
fn test_oceanbase_nats_users_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_users_compat_impl(&prefix).await;
    });
}

/// Test templates table compatibility with OceanBase+NATS
/// Order: 03, Dependencies: None
#[test]
fn test_oceanbase_nats_templates_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_templates_compat_impl(&prefix).await;
    });
}

/// Test short_urls table compatibility with OceanBase+NATS
/// Order: 04, Dependencies: None
#[test]
fn test_oceanbase_nats_short_urls_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_short_urls_compat_impl(&prefix).await;
    });
}

/// Test sessions table compatibility with OceanBase+NATS
/// Order: 05, Dependencies: None
#[test]
fn test_oceanbase_nats_sessions_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_sessions_compat_impl(&prefix).await;
    });
}

/// Test system_prompts table compatibility with OceanBase+NATS
/// Order: 06, Dependencies: None
#[test]
fn test_oceanbase_nats_system_prompts_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_system_prompts_compat_impl(&prefix).await;
    });
}

/// Test distinct_value_fields table compatibility with OceanBase+NATS
/// Order: 07, Dependencies: None
#[test]
fn test_oceanbase_nats_distinct_values_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_distinct_values_compat_impl(&prefix).await;
    });
}

// ==================== Level 1: Dependent Table Tests ====================

/// Test org_users table compatibility with OceanBase+NATS
/// Order: 10, Dependencies: organizations, users
#[test]
fn test_oceanbase_nats_org_users_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_org_users_compat_impl(&prefix).await;
    });
}

/// Test folders table compatibility with OceanBase+NATS
/// Order: 11, Dependencies: organizations
#[test]
fn test_oceanbase_nats_folders_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_folders_compat_impl(&prefix).await;
    });
}

/// Test destinations table compatibility with OceanBase+NATS
/// Order: 12, Dependencies: organizations, templates
#[test]
fn test_oceanbase_nats_destinations_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_destinations_compat_impl(&prefix).await;
    });
}

// ==================== Level 2: Complex Table Tests ====================

/// Test alerts table compatibility with OceanBase+NATS
/// Order: 20, Dependencies: organizations, folders, templates, destinations
#[test]
fn test_oceanbase_nats_alerts_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_alerts_compat_impl(&prefix).await;
    });
}

/// Test dashboards table compatibility with OceanBase+NATS
/// Order: 21, Dependencies: organizations, folders
#[test]
fn test_oceanbase_nats_dashboards_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_dashboards_compat_impl(&prefix).await;
    });
}

// ==================== Level 3: File List Infrastructure Tests ====================

/// Test file_list table compatibility with OceanBase+NATS
/// Order: 20, Dependencies: None (independent infrastructure)
#[test]
fn test_oceanbase_nats_file_list_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_file_list_compat_impl(&prefix).await;
    });
}

/// Test stream_stats table compatibility with OceanBase+NATS
/// Order: 21, Dependencies: None (independent infrastructure)
#[test]
fn test_oceanbase_nats_stream_stats_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_stream_stats_compat_impl(&prefix).await;
    });
}

/// Test file_list_deleted table compatibility with OceanBase+NATS
/// Order: 32, Dependencies: None (independent infrastructure)
#[test]
fn test_oceanbase_nats_file_list_deleted_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_file_list_deleted_compat_impl(&prefix).await;
    });
}

/// Test file_list_deleted concurrent query_deleted with distributed lock (NATS)
/// Order: 32a, Dependencies: file_list_deleted
#[test]
fn test_oceanbase_nats_file_list_deleted_concurrent_query_deleted() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_file_list_deleted_concurrent_query_deleted_impl(&prefix)
            .await;
    });
}

/// Test file_list_deleted lock serialization verification
/// Order: 32b, Dependencies: file_list_deleted
#[test]
fn test_oceanbase_nats_file_list_deleted_lock_serialization() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_file_list_deleted_lock_serialization_impl(&prefix).await;
    });
}

/// Test file_list_deleted concurrent counter (atomic updates)
/// Order: 32c, Dependencies: file_list_deleted
#[test]
fn test_oceanbase_nats_file_list_deleted_concurrent_counter() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_file_list_deleted_concurrent_counter_impl(&prefix).await;
    });
}

/// Test file_list_jobs table compatibility with OceanBase+NATS
/// Order: 33, Dependencies: None (independent infrastructure)
#[test]
fn test_oceanbase_nats_file_list_jobs_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_file_list_jobs_compat_impl(&prefix).await;
    });
}

/// Test file_list_jobs concurrent get_pending_jobs with distributed lock (NATS)
/// Order: 33a, Dependencies: file_list_jobs
#[test]
fn test_oceanbase_nats_file_list_jobs_concurrent_get_pending() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_file_list_jobs_concurrent_get_pending_impl(&prefix).await;
    });
}

/// Test file_list_jobs concurrent get_pending_dump_jobs with distributed lock (NATS)
/// Order: 33b, Dependencies: file_list_jobs
#[test]
fn test_oceanbase_nats_file_list_jobs_concurrent_get_dump_jobs() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_file_list_jobs_concurrent_get_dump_jobs_impl(&prefix).await;
    });
}

/// Test file_list_jobs lock serialization verification
/// Order: 33c, Dependencies: file_list_jobs
#[test]
fn test_oceanbase_nats_file_list_jobs_lock_serialization() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_file_list_jobs_lock_serialization_impl(&prefix).await;
    });
}

/// Test scheduled_jobs table compatibility with OceanBase+NATS
/// Order: 34, Dependencies: None
#[test]
fn test_oceanbase_nats_scheduled_jobs_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_scheduled_jobs_compat_impl(&prefix).await;
    });
}

/// Test scheduled_jobs concurrent pull with distributed lock (NATS)
/// Order: 34a, Dependencies: scheduled_jobs
#[test]
fn test_oceanbase_nats_scheduled_jobs_concurrent_pull() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_scheduled_jobs_concurrent_pull_impl(&prefix).await;
    });
}

/// Test scheduled_jobs pull lock serialization verification
/// Order: 34b, Dependencies: scheduled_jobs
#[test]
fn test_oceanbase_nats_scheduled_jobs_pull_lock_serialization() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_scheduled_jobs_pull_lock_serialization_impl(&prefix).await;
    });
}

// ==================== Level 4: Additional Tables ====================

/// Test cipher_keys table compatibility with OceanBase+NATS
/// Order: 40, Dependencies: None
#[test]
fn test_oceanbase_nats_cipher_keys_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_cipher_keys_compat_impl(&prefix).await;
    });
}

/// Test system_settings table compatibility with OceanBase+NATS
/// Order: 41, Dependencies: None
#[test]
fn test_oceanbase_nats_system_settings_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_system_settings_compat_impl(&prefix).await;
    });
}

/// Test action_scripts table compatibility with OceanBase+NATS
/// Order: 42, Dependencies: None
#[test]
fn test_oceanbase_nats_action_scripts_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_action_scripts_compat_impl(&prefix).await;
    });
}

/// Test enrichment_tables table compatibility with OceanBase+NATS
/// Order: 43, Dependencies: None
#[test]
fn test_oceanbase_nats_enrichment_tables_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_enrichment_tables_compat_impl(&prefix).await;
    });
}

/// Test enrichment_table_urls table compatibility with OceanBase+NATS
/// Order: 44, Dependencies: None
#[test]
fn test_oceanbase_nats_enrichment_table_urls_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_enrichment_table_urls_compat_impl(&prefix).await;
    });
}

/// Test rate_limit_rules table compatibility with OceanBase+NATS
/// Order: 45, Dependencies: None
#[test]
fn test_oceanbase_nats_rate_limit_rules_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_rate_limit_rules_compat_impl(&prefix).await;
    });
}

/// Test re_patterns table compatibility with OceanBase+NATS
/// Order: 46, Dependencies: None
#[test]
fn test_oceanbase_nats_re_patterns_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_re_patterns_compat_impl(&prefix).await;
    });
}

/// Test re_pattern_stream_map table compatibility with OceanBase+NATS
/// Order: 47, Dependencies: re_patterns
#[test]
fn test_oceanbase_nats_re_pattern_stream_map_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_re_pattern_stream_map_compat_impl(&prefix).await;
    });
}

/// Test service_streams table compatibility with OceanBase+NATS
/// Order: 48, Dependencies: None
#[test]
fn test_oceanbase_nats_service_streams_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_service_streams_compat_impl(&prefix).await;
    });
}

/// Test service_streams_dimensions table compatibility with OceanBase+NATS
/// Order: 49, Dependencies: None
#[test]
fn test_oceanbase_nats_service_streams_dimensions_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_service_streams_dimensions_compat_impl(&prefix).await;
    });
}

/// Test search_queue table compatibility with OceanBase+NATS
/// Order: 50, Dependencies: None
#[test]
fn test_oceanbase_nats_search_queue_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_search_queue_compat_impl(&prefix).await;
    });
}

/// Test compactor_manual_jobs table compatibility with OceanBase+NATS
/// Order: 51, Dependencies: None
#[test]
fn test_oceanbase_nats_compactor_manual_jobs_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_compactor_manual_jobs_compat_impl(&prefix).await;
    });
}

/// Test alert_incidents table compatibility with OceanBase+NATS
/// Order: 52, Dependencies: organizations
#[test]
fn test_oceanbase_nats_alert_incidents_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_alert_incidents_compat_impl(&prefix).await;
    });
}

/// Test file_list_dump_stats table compatibility with OceanBase+NATS
/// Order: 53, Dependencies: None (file_list infrastructure)
#[test]
fn test_oceanbase_nats_file_list_dump_stats_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_file_list_dump_stats_compat_impl(&prefix).await;
    });
}

/// Test schema_history table compatibility with OceanBase+NATS
/// Order: 54, Dependencies: None
#[test]
fn test_oceanbase_nats_schema_history_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_schema_history_compat_impl(&prefix).await;
    });
}

/// Test reports table compatibility with OceanBase+NATS
/// Order: 55, Dependencies: organizations
#[test]
fn test_oceanbase_nats_reports_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_reports_compat_impl(&prefix).await;
    });
}

/// Test timed_annotations table compatibility with OceanBase+NATS
/// Order: 56, Dependencies: organizations
#[test]
fn test_oceanbase_nats_timed_annotations_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_timed_annotations_compat_impl(&prefix).await;
    });
}

/// Test timed_annotation_panels table compatibility with OceanBase+NATS
/// Order: 57, Dependencies: timed_annotations
#[test]
fn test_oceanbase_nats_timed_annotation_panels_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_timed_annotation_panels_compat_impl(&prefix).await;
    });
}

/// Test search_jobs table compatibility with OceanBase+NATS
/// Order: 58, Dependencies: organizations
#[test]
fn test_oceanbase_nats_search_jobs_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_search_jobs_compat_impl(&prefix).await;
    });
}

/// Test search_job_partitions table compatibility with OceanBase+NATS
/// Order: 59, Dependencies: search_jobs
#[test]
fn test_oceanbase_nats_search_job_partitions_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_search_job_partitions_compat_impl(&prefix).await;
    });
}

/// Test search_job_results table compatibility with OceanBase+NATS
/// Order: 60, Dependencies: search_jobs
#[test]
fn test_oceanbase_nats_search_job_results_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_search_job_results_compat_impl(&prefix).await;
    });
}

/// Test pipeline table compatibility with OceanBase+NATS
/// Order: 61, Dependencies: None
#[test]
fn test_oceanbase_nats_pipeline_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_pipeline_compat_impl(&prefix).await;
    });
}

/// Test alert_dedup_state table compatibility with OceanBase+NATS
/// Order: 62, Dependencies: alerts
#[test]
fn test_oceanbase_nats_alert_dedup_state_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_alert_dedup_state_compat_impl(&prefix).await;
    });
}

/// Test file_list_history table compatibility with OceanBase+NATS
/// Order: 63, Dependencies: None (file_list infrastructure)
#[test]
fn test_oceanbase_nats_file_list_history_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_file_list_history_compat_impl(&prefix).await;
    });
}

/// Test pipeline_last_errors table compatibility with OceanBase+NATS
/// Order: 64, Dependencies: pipelines (soft dependency)
#[test]
fn test_oceanbase_nats_pipeline_last_errors_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_pipeline_last_errors_compat_impl(&prefix).await;
    });
}

/// Test alert_incident_alerts table compatibility with OceanBase+NATS
/// Order: 65, Dependencies: alert_incidents
#[test]
fn test_oceanbase_nats_alert_incident_alerts_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_alert_incident_alerts_compat_impl(&prefix).await;
    });
}

/// Test report_dashboards table compatibility with OceanBase+NATS
/// Order: 66, Dependencies: reports, dashboards
#[test]
fn test_oceanbase_nats_report_dashboards_compat() {
    TEST_RUNTIME.block_on(async {
        init_config_for_oceanbase_nats_tests();
        let prefix = generate_test_prefix().await;
        table_compat_tests_impl::test_report_dashboards_compat_impl(&prefix).await;
    });
}
