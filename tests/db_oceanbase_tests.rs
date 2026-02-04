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

//! OceanBase integration tests for OceanBaseDb trait implementation.
//!
//! These tests verify the OceanBaseDb implementation of the Db trait
//! using real OceanBase database connections.
//!
//! OceanBaseDb uses NATS distributed locks instead of MySQL GET_LOCK
//! for compatibility with OceanBase versions prior to V4.2.0.
//!
//! Default connection: mysql://root:oceanbase123@10.10.14.64:2881/openobserve_test
//!
//! # Running Tests
//!
//! ```bash
//! cargo test --test db_oceanbase_tests --features db-oceanbase-tests -- --test-threads=1 --nocapture
//! ```
//!
//! Or with custom DSN:
//! ```bash
//! ZO_TEST_OCEANBASE_DSN="mysql://user:pass@host:port/db" \
//!   cargo test --test db_oceanbase_tests --features db-oceanbase-tests -- --test-threads=1
//! ```

#![cfg(feature = "db-oceanbase-tests")]

mod common;

use common::{
    db_helpers::{RealOceanBaseInstance, init_config_for_oceanbase_tests},
    db_tests_impl,
};
use infra::db::oceanbase::OceanBaseDb;
use once_cell::sync::Lazy;

// ==================== Global Runtime ====================

static TEST_RUNTIME: Lazy<tokio::runtime::Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .expect("Failed to create test runtime")
});

/// Setup test environment and return (db, prefix)
async fn setup_test(db: Option<&str>) -> (OceanBaseDb, String) {
    init_config_for_oceanbase_tests();
    let _ = RealOceanBaseInstance::new(db).await; // Ensures schema and truncation
    let db = OceanBaseDb::new();
    let prefix = format!(
        "oceanbase_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros()
    );
    (db, prefix)
}

// ==================== Schema Tests ====================

mod schema_tests {
    use super::*;

    #[tokio::test]
    async fn test_required_indexes_exist() {
        init_config_for_oceanbase_tests();
        let instance = RealOceanBaseInstance::new(None).await;
        db_tests_impl::test_required_indexes_exist_impl(&instance.pool).await;
    }

    #[tokio::test]
    async fn test_unique_constraint_on_composite_key() {
        init_config_for_oceanbase_tests();
        let instance = RealOceanBaseInstance::new(None).await;
        db_tests_impl::test_unique_constraint_impl(&instance.pool).await;
    }
}

// ==================== Get For Update Tests ====================

mod get_for_update_tests {
    use super::*;

    #[test]
    fn test_get_for_update_basic_update() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) = setup_test(Some("get_for_update_basic_update")).await;
            db_tests_impl::test_get_for_update_basic_update_impl(&db, &prefix).await;
        });
    }

    #[test]
    fn test_get_for_update_returns_none() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) = setup_test(Some("get_for_update_returns_none")).await;
            db_tests_impl::test_get_for_update_returns_none_impl(&db, &prefix).await;
        });
    }

    #[test]
    fn test_get_for_update_returns_error() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) = setup_test(Some("get_for_update_returns_error")).await;
            db_tests_impl::test_get_for_update_returns_error_impl(&db, &prefix).await;
        });
    }

    #[test]
    fn test_get_for_update_insert_when_not_exist() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) = setup_test(Some("get_for_update_insert_when_not_exist")).await;
            db_tests_impl::test_get_for_update_insert_when_not_exist_impl(&db, &prefix).await;
        });
    }

    #[test]
    fn test_get_for_update_with_new_key() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) = setup_test(Some("get_for_update_with_new_key")).await;
            db_tests_impl::test_get_for_update_with_new_key_impl(&db, &prefix).await;
        });
    }

    #[test]
    fn test_get_for_update_with_start_dt() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) = setup_test(Some("get_for_update_with_start_dt")).await;
            db_tests_impl::test_get_for_update_with_start_dt_impl(&db, &prefix).await;
        });
    }

    #[test]
    fn test_get_for_update_without_start_dt_gets_latest() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) =
                setup_test(Some("get_for_update_without_start_dt_gets_latest")).await;
            db_tests_impl::test_get_for_update_without_start_dt_gets_latest_impl(&db, &prefix)
                .await;
        });
    }

    #[test]
    fn test_get_for_update_update_and_new_key() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) = setup_test(Some("get_for_update_update_and_new_key")).await;
            db_tests_impl::test_get_for_update_update_and_new_key_impl(&db, &prefix).await;
        });
    }
}

// ==================== CRUD Tests ====================

mod crud_tests {
    use super::*;

    #[test]
    fn test_put_and_get() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) = setup_test(Some("put_and_get")).await;
            db_tests_impl::test_put_and_get_impl(&db, &prefix).await;
        });
    }

    #[test]
    fn test_delete() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) = setup_test(Some("delete")).await;
            db_tests_impl::test_delete_impl(&db, &prefix).await;
        });
    }

    #[test]
    fn test_count() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) = setup_test(Some("count")).await;
            db_tests_impl::test_count_impl(&db, &prefix).await;
        });
    }

    #[test]
    fn test_get_nonexistent_key() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) = setup_test(Some("get_nonexistent_key")).await;
            db_tests_impl::test_get_nonexistent_key_impl(&db, &prefix).await;
        });
    }

    #[test]
    fn test_put_overwrites() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) = setup_test(Some("put_overwrites")).await;
            db_tests_impl::test_put_overwrites_impl(&db, &prefix).await;
        });
    }

    #[test]
    fn test_delete_with_prefix() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) = setup_test(Some("delete_with_prefix")).await;
            db_tests_impl::test_delete_with_prefix_impl(&db, &prefix).await;
        });
    }

    #[test]
    fn test_list() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) = setup_test(Some("list")).await;
            db_tests_impl::test_list_impl(&db, &prefix).await;
        });
    }

    #[test]
    fn test_list_keys() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) = setup_test(Some("list_keys")).await;
            db_tests_impl::test_list_keys_impl(&db, &prefix).await;
        });
    }

    #[test]
    fn test_stats() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) = setup_test(Some("stats")).await;
            db_tests_impl::test_stats_impl(&db, &prefix).await;
        });
    }
}

// ==================== Edge Case Tests ====================

mod edge_case_tests {
    use super::*;

    #[test]
    fn test_unicode_values() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) = setup_test(Some("unicode_values")).await;
            db_tests_impl::test_unicode_values_impl(&db, &prefix).await;
        });
    }

    #[test]
    fn test_special_characters() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) = setup_test(Some("special_characters")).await;
            db_tests_impl::test_special_characters_impl(&db, &prefix).await;
        });
    }

    #[test]
    fn test_empty_value() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) = setup_test(Some("empty_value")).await;
            db_tests_impl::test_empty_value_impl(&db, &prefix).await;
        });
    }

    #[test]
    fn test_large_value() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) = setup_test(Some("large_value")).await;
            db_tests_impl::test_large_value_impl(&db, &prefix).await;
        });
    }

    #[test]
    fn test_start_dt_variations() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) = setup_test(Some("start_dt_variations")).await;
            db_tests_impl::test_start_dt_variations_impl(&db, &prefix).await;
        });
    }
}

// ==================== Concurrent Tests ====================

mod concurrent_tests {
    use super::*;

    #[test]
    fn test_concurrent_two_clients_same_key() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) = setup_test(Some("concurrent_two_clients_same_key")).await;
            db_tests_impl::test_concurrent_two_clients_same_key_impl(&db, &prefix).await;
        });
    }

    #[test]
    fn test_concurrent_lock_serialization() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) = setup_test(Some("concurrent_lock_serialization")).await;
            db_tests_impl::test_concurrent_lock_serialization_impl(&db, &prefix).await;
        });
    }

    #[test]
    fn test_concurrent_counter_increment() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) = setup_test(Some("concurrent_counter_increment")).await;
            db_tests_impl::test_concurrent_counter_increment_impl(&db, &prefix).await;
        });
    }

    #[test]
    fn test_concurrent_different_keys() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) = setup_test(Some("concurrent_different_keys")).await;
            db_tests_impl::test_concurrent_different_keys_impl(&db, &prefix).await;
        });
    }

    #[test]
    fn test_lock_timeout_returns_error() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) = setup_test(Some("lock_timeout_returns_error")).await;
            db_tests_impl::test_lock_timeout_returns_error_impl(&db, &prefix).await;
        });
    }

    #[test]
    fn test_lock_released_on_success() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) = setup_test(Some("lock_released_on_success")).await;
            db_tests_impl::test_lock_released_on_success_impl(&db, &prefix).await;
        });
    }

    #[test]
    fn test_lock_released_on_update_fn_error() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) = setup_test(Some("lock_released_on_update_fn_error")).await;
            db_tests_impl::test_lock_released_on_update_fn_error_impl(&db, &prefix).await;
        });
    }

    #[test]
    fn test_lock_released_on_transaction_error() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) = setup_test(Some("lock_released_on_transaction_error")).await;
            db_tests_impl::test_lock_released_on_transaction_error_impl(&db, &prefix).await;
        });
    }

    #[test]
    fn test_lock_contention_fairness() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) = setup_test(Some("lock_contention_fairness")).await;
            db_tests_impl::test_lock_contention_fairness_impl(&db, &prefix, "OceanBase").await;
        });
    }

    #[test]
    fn test_concurrent_insert_same_new_key() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) = setup_test(Some("concurrent_insert_same_new_key")).await;
            db_tests_impl::test_concurrent_insert_same_new_key_impl(&db, &prefix).await;
        });
    }

    #[test]
    fn test_concurrent_update_with_new_key() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) = setup_test(Some("concurrent_update_with_new_key")).await;
            db_tests_impl::test_concurrent_update_with_new_key_impl(&db, &prefix).await;
        });
    }

    #[test]
    fn test_long_running_update_fn() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) = setup_test(Some("long_running_update_fn")).await;
            db_tests_impl::test_long_running_update_fn_impl(&db, &prefix).await;
        });
    }

    #[test]
    fn test_concurrent_with_different_start_dt() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) = setup_test(Some("concurrent_with_different_start_dt")).await;
            db_tests_impl::test_concurrent_with_different_start_dt_impl(&db, &prefix).await;
        });
    }

    #[test]
    fn test_high_concurrency_20_clients() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) = setup_test(Some("high_concurrency_20_clients")).await;
            db_tests_impl::test_high_concurrency_20_clients_impl(&db, &prefix).await;
        });
    }

    #[test]
    fn test_rapid_sequential_updates() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) = setup_test(Some("rapid_sequential_updates")).await;
            db_tests_impl::test_rapid_sequential_updates_impl(&db, &prefix).await;
        });
    }

    #[test]
    fn test_mixed_read_write_concurrency() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) = setup_test(Some("mixed_read_write_concurrency")).await;
            db_tests_impl::test_mixed_read_write_concurrency_impl(&db, &prefix).await;
        });
    }

    #[test]
    fn test_concurrent_across_connection_pools() {
        TEST_RUNTIME.block_on(async {
            let (db, prefix) = setup_test(Some("concurrent_across_connection_pools")).await;
            db_tests_impl::test_concurrent_across_connection_pools_impl(&db, &prefix).await;
        });
    }
}
