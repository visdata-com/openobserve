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

//! Database test helpers for MySQL/OceanBase integration tests.
//!
//! Provides utilities for connecting to real database instances for testing.

use std::{
    str::FromStr,
    sync::{
        Once,
        atomic::{AtomicBool, Ordering},
    },
};

use sqlx::{
    MySql, Pool,
    mysql::{MySqlConnectOptions, MySqlPoolOptions},
};

/// Macro to define database test instance structures with common functionality.
/// This eliminates code duplication between MySQL and OceanBase test instances.
macro_rules! define_db_test_instance {
    (
        $struct_name:ident,
        $feature:literal,
        // $schema_flag:ident,
        $default_dsn:literal,
        $env_var:literal,
        $db_display_name:literal,
        $is_oceanbase:expr
    ) => {
        // #[cfg(feature = $feature)]
        // #[allow(dead_code)]
        // static $schema_flag: AtomicBool = AtomicBool::new(false);

        #[doc = concat!("Real ", $db_display_name, " instance connection for integration testing.")]
        #[doc = "Use environment variable or default to the test instance."]
        #[cfg(feature = $feature)]
        pub struct $struct_name {
            pub pool: Pool<MySql>,
            pub dsn: String,
        }

        #[cfg(feature = $feature)]
        #[allow(dead_code)]
        impl $struct_name {
            #[doc = concat!("Default ", $db_display_name, " test instance DSN")]
            const DEFAULT_DSN: &'static str = $default_dsn;

            #[doc = concat!("Creates a connection to a real ", $db_display_name, " instance.")]
            #[doc = concat!("Uses ", $env_var, " environment variable or defaults to test instance.")]
            pub async fn new(db: Option<&str>) -> Self {
                let dsn = std::env::var($env_var)
                    .unwrap_or_else(|_| Self::DEFAULT_DSN.to_string());

                // First connect without database to create it if needed
                let base_dsn = dsn.rsplit_once('/').map(|(base, _)| base).unwrap_or(&dsn);
                let db_name = db.unwrap_or(dsn.rsplit_once('/').map(|(_, db)| db).unwrap_or("openobserve_test"));

                // Connect to server (without specific database)

                // Use connect_lazy_with and disable initialization queries for OceanBase compatibility:
                // - set_names(false): Disable SET NAMES query
                // - pipes_as_concat(false): Disable PIPES_AS_CONCAT (avoids subquery in SET sql_mode)
                // - no_engine_substitution(false): Disable NO_ENGINE_SUBSTITUTION (avoids subquery in SET sql_mode)
                // - collation("utf8mb4_general_ci"): OceanBase only supports utf8mb4_general_ci, not utf8mb4_unicode_ci
                // OceanBase doesn't support: SET sql_mode=(SELECT CONCAT(@@sql_mode, '...'))
                let server_opts = MySqlConnectOptions::from_str(&format!("{}/", base_dsn))
                    .expect(concat!("Invalid DSN for ", $db_display_name))
                    .charset("utf8mb4")
                    .collation("utf8mb4_general_ci")
                    .set_names(false)
                    .pipes_as_concat(false)
                    .no_engine_substitution(false);
                let server_pool = MySqlPoolOptions::new()
                    .connect_lazy_with(server_opts);

                // Create database if not exists
                sqlx::query(&format!("CREATE DATABASE IF NOT EXISTS `{}`", db_name))
                    .execute(&server_pool)
                    .await
                    .expect("Failed to create test database");

                // Set OceanBase system parameter for testing (only for OceanBase)
                if $is_oceanbase {
                    sqlx::query("ALTER SYSTEM SET open_cursors = 10000")
                        .execute(&server_pool)
                        .await
                        .expect("Failed to set open_cursors parameter");
                }

                // Now connect to the specific database

                // Use same connection options for OceanBase compatibility
                let db_opts = MySqlConnectOptions::from_str(&dsn)
                    .expect(concat!("Invalid DSN for ", $db_display_name))
                    .charset("utf8mb4")
                    .collation("utf8mb4_general_ci")
                    .set_names(false)
                    .pipes_as_concat(false)
                    .no_engine_substitution(false);
                let pool = MySqlPoolOptions::new()
                    .connect_lazy_with(db_opts);

                // Create schema only once using atomic flag
                // if !$schema_flag.load(Ordering::SeqCst) {
                Self::create_schema(&pool).await;
                    // $schema_flag.store(true, Ordering::SeqCst);
                // }

                // Always truncate to ensure clean state for each test
                sqlx::query("TRUNCATE TABLE meta")
                    .execute(&pool)
                    .await
                    .expect("Failed to truncate meta table");

                Self { pool, dsn }
            }

            /// Creates the meta table schema for testing.
            async fn create_schema(pool: &Pool<MySql>) {
                // Drop and recreate for clean state
                sqlx::query("DROP TABLE IF EXISTS meta")
                    .execute(pool)
                    .await
                    .expect("Failed to drop existing meta table");

                // Create table
                sqlx::query(
                    r#"CREATE TABLE IF NOT EXISTS meta
                    (
                        id       BIGINT NOT NULL PRIMARY KEY AUTO_INCREMENT,
                        module   VARCHAR(100) NOT NULL,
                        key1     VARCHAR(256) NOT NULL,
                        key2     VARCHAR(256) NOT NULL,
                        start_dt BIGINT NOT NULL DEFAULT 0,
                        value    LONGTEXT NOT NULL
                    )"#,
                )
                .execute(pool)
                .await
                .expect("Failed to create meta table");

                // Create indexes
                sqlx::query("CREATE INDEX meta_module_idx ON meta (module)")
                    .execute(pool)
                    .await
                    .expect("Failed to create meta_module_idx");

                sqlx::query("CREATE INDEX meta_module_key1_idx ON meta (module, key1)")
                    .execute(pool)
                    .await
                    .expect("Failed to create meta_module_key1_idx");

                sqlx::query("CREATE UNIQUE INDEX meta_module_start_dt_idx ON meta (module, key1, key2, start_dt)")
                    .execute(pool)
                    .await
                    .expect("Failed to create meta_module_start_dt_idx");
            }

            /// Truncates all test tables for isolation between tests.
            #[allow(dead_code)]
            pub async fn truncate(&self) {
                sqlx::query("TRUNCATE TABLE meta")
                    .execute(&self.pool)
                    .await
                    .expect("Failed to truncate meta table");
            }

            /// Gets the count of records in the meta table.
            #[allow(dead_code)]
            pub async fn count_meta_records(&self) -> i64 {
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM meta")
                    .fetch_one(&self.pool)
                    .await
                    .unwrap_or(0)
            }

            /// Gets the DSN for this instance.
            #[allow(dead_code)]
            pub fn get_dsn(&self) -> &str {
                &self.dsn
            }
        }
    };
}

// Define MySQL test instance
define_db_test_instance!(
    RealMySqlInstance,
    "db-mysql-tests",
    // MYSQL_SCHEMA_INITIALIZED,
    "mysql://root:oceanbase123@10.10.14.64:2881/openobserve_test",
    "ZO_TEST_MYSQL_DSN",
    "MySQL",
    false
);

// Define OceanBase test instance
define_db_test_instance!(
    RealOceanBaseInstance,
    "db-oceanbase-tests",
    // OCEANBASE_SCHEMA_INITIALIZED,
    "mysql://root:oceanbase123@10.10.14.66:2881/openobserve_test",
    "ZO_TEST_OCEANBASE_DSN",
    "OceanBase",
    true
);

/// Initialize OpenObserve config for MysqlDb trait testing.
/// This sets up the environment variables needed for MysqlDb to connect to the test database.
/// Must be called before any MysqlDb operations.
#[cfg(feature = "db-mysql-tests")]
#[allow(dead_code)]
static CONFIG_INIT: Once = Once::new();

#[cfg(feature = "db-mysql-tests")]
static CONFIG_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Initialize config for MysqlDb trait tests.
/// Sets up environment variables so that MysqlDb uses the test MySQL instance.
///
/// # Safety
/// This function uses `std::env::set_var` which is unsafe because modifying environment
/// variables is not thread-safe. This is acceptable in test context where tests run
/// serially with `#[serial]` attribute.
#[cfg(feature = "db-mysql-tests")]
#[allow(dead_code)]
pub fn init_config_for_mysql_tests() {
    CONFIG_INIT.call_once(|| {
        let dsn = std::env::var("ZO_TEST_MYSQL_DSN")
            .unwrap_or_else(|_| RealMySqlInstance::DEFAULT_DSN.to_string());

        // SAFETY: Tests run serially with #[serial] attribute, so no concurrent access
        // to environment variables. This is the standard pattern for test setup.
        unsafe {
            // Set required environment variables for MysqlDb
            std::env::set_var("ZO_META_STORE", "mysql");
            std::env::set_var("ZO_META_MYSQL_DSN", &dsn);
            std::env::set_var("ZO_META_MYSQL_RO_DSN", &dsn);
            std::env::set_var("ZO_META_DDL_DSN", &dsn);
            std::env::set_var("ZO_LOCAL_MODE", "true");
            std::env::set_var("ZO_LOCAL_MODE_STORAGE", "disk");

            // Set lock timeout for testing (30 seconds to handle slow remote DB)
            std::env::set_var("ZO_META_TRANSACTION_LOCK_TIMEOUT", "30");

            // Set data directory to temp
            let tmp_dir = std::env::temp_dir().join("openobserve_test");
            std::fs::create_dir_all(&tmp_dir).ok();
            std::env::set_var("ZO_DATA_DIR", tmp_dir.to_string_lossy().as_ref());

            // Connection pool settings for tests
            // Note: get_for_update uses 2 connections simultaneously (lock_tx + data tx)
            // Increased pool size to handle concurrent tests
            std::env::set_var("ZO_META_CONNECTION_POOL_ACQUIRE_TIMEOUT", "60"); // 60 seconds
            std::env::set_var("ZO_META_CONNECTION_POOL_MIN_SIZE", "2"); // Keep some connections ready
            std::env::set_var("ZO_META_CONNECTION_POOL_MAX_SIZE", "20"); // Larger pool for concurrency
            // Short idle and max lifetime to release connections faster between tests
            std::env::set_var("ZO_META_CONNECTION_POOL_IDLE_TIMEOUT", "5");
            std::env::set_var("ZO_META_CONNECTION_POOL_MAX_LIFETIME", "30");
        }

        // Refresh config to pick up new environment variables
        config::refresh_config().expect("Failed to refresh config");

        CONFIG_INITIALIZED.store(true, Ordering::SeqCst);
        println!(
            "✓ OpenObserve config initialized for MySQL tests (DSN: {})",
            dsn
        );
    });
}

/// Check if config is initialized for MySQL tests.
#[cfg(feature = "db-mysql-tests")]
#[allow(dead_code)]
pub fn is_config_initialized() -> bool {
    CONFIG_INITIALIZED.load(Ordering::SeqCst)
}

// ============================================================================
// OceanBase config initialization
// ============================================================================

#[cfg(feature = "db-oceanbase-tests")]
#[allow(dead_code)]
static OB_CONFIG_INIT: Once = Once::new();

#[cfg(feature = "db-oceanbase-tests")]
static OB_CONFIG_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Initialize config for MysqlDb trait tests using OceanBase.
/// Sets up environment variables so that MysqlDb uses the test OceanBase instance.
///
/// # Safety
/// This function uses `std::env::set_var` which is unsafe because modifying environment
/// variables is not thread-safe. This is acceptable in test context where tests run
/// serially with `#[serial]` attribute.
#[cfg(feature = "db-oceanbase-tests")]
#[allow(dead_code)]
pub fn init_config_for_oceanbase_tests() {
    OB_CONFIG_INIT.call_once(|| {
        let dsn = std::env::var("ZO_TEST_OCEANBASE_DSN")
            .unwrap_or_else(|_| RealOceanBaseInstance::DEFAULT_DSN.to_string());

        // Respect command-line ZO_LOCAL_MODE if set, otherwise default to "true"
        let local_mode = std::env::var("ZO_LOCAL_MODE").unwrap_or_else(|_| "true".to_string());

        // SAFETY: Tests run serially with #[serial] attribute, so no concurrent access
        // to environment variables. This is the standard pattern for test setup.
        unsafe {
            // Set required environment variables for OceanBaseDb
            // Note: OceanBaseDb uses NATS distributed locks instead of MySQL GET_LOCK
            // for compatibility with OceanBase versions prior to V4.2.0
            std::env::set_var("ZO_META_STORE", "oceanbase");
            std::env::set_var("ZO_META_MYSQL_DSN", &dsn);
            std::env::set_var("ZO_META_MYSQL_RO_DSN", &dsn);
            std::env::set_var("ZO_META_DDL_DSN", &dsn);
            std::env::set_var("ZO_LOCAL_MODE", &local_mode);
            std::env::set_var("ZO_LOCAL_MODE_STORAGE", "disk");

            // Set lock timeout for testing (30 seconds to handle slow remote DB)
            std::env::set_var("ZO_META_TRANSACTION_LOCK_TIMEOUT", "30");

            // Set data directory to temp
            let tmp_dir = std::env::temp_dir().join("openobserve_oceanbase_test");
            std::fs::create_dir_all(&tmp_dir).ok();
            std::env::set_var("ZO_DATA_DIR", tmp_dir.to_string_lossy().as_ref());

            // Connection pool settings for tests
            // OceanBaseDb delegates to MysqlDb for database operations
            // Increased pool size to handle concurrent tests
            std::env::set_var("ZO_META_CONNECTION_POOL_ACQUIRE_TIMEOUT", "60"); // 60 seconds
            std::env::set_var("ZO_META_CONNECTION_POOL_MIN_SIZE", "2"); // Keep some connections ready
            std::env::set_var("ZO_META_CONNECTION_POOL_MAX_SIZE", "20"); // Larger pool for concurrency
            // Short idle and max lifetime to release connections faster between tests
            std::env::set_var("ZO_META_CONNECTION_POOL_IDLE_TIMEOUT", "5");
            std::env::set_var("ZO_META_CONNECTION_POOL_MAX_LIFETIME", "30");
        }

        // Refresh config to pick up new environment variables
        config::refresh_config().expect("Failed to refresh config");

        OB_CONFIG_INITIALIZED.store(true, Ordering::SeqCst);
        println!(
            "✓ OpenObserve config initialized for OceanBase tests (DSN: {}, LOCAL_MODE: {})",
            dsn, local_mode
        );
    });
}

/// Check if config is initialized for OceanBase tests.
#[cfg(feature = "db-oceanbase-tests")]
#[allow(dead_code)]
pub fn is_oceanbase_config_initialized() -> bool {
    OB_CONFIG_INITIALIZED.load(Ordering::SeqCst)
}

// ============================================================================
// OceanBase + NATS distributed lock config initialization
// ============================================================================

#[cfg(feature = "db-oceanbase-nats-tests")]
#[allow(dead_code)]
static OB_NATS_CONFIG_INIT: Once = Once::new();

#[cfg(feature = "db-oceanbase-nats-tests")]
static OB_NATS_CONFIG_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Initialize config for OceanBase tests with NATS distributed lock.
/// This sets up the environment variables for cluster mode (non-local mode)
/// where OceanBaseDb uses NATS distributed locks.
///
/// Requires NATS server running at ZO_NATS_ADDR (default: localhost:4222).
///
/// # Safety
/// This function uses `std::env::set_var` which is unsafe because modifying environment
/// variables is not thread-safe. This is acceptable in test context where tests run
/// serially with `#[serial]` attribute.
#[cfg(feature = "db-oceanbase-nats-tests")]
#[allow(dead_code)]
pub fn init_config_for_oceanbase_nats_tests() {
    OB_NATS_CONFIG_INIT.call_once(|| {
        let dsn = std::env::var("ZO_TEST_OCEANBASE_DSN").unwrap_or_else(|_| {
            "mysql://root:oceanbase123@10.10.14.66:2881/openobserve_test".to_string()
        });

        let nats_addr =
            std::env::var("ZO_NATS_ADDR").unwrap_or_else(|_| "10.10.14.63:4222".to_string());

        // SAFETY: Tests run serially with #[serial] attribute, so no concurrent access
        // to environment variables. This is the standard pattern for test setup.
        unsafe {
            // Set required environment variables for OceanBaseDb with NATS
            std::env::set_var("ZO_META_STORE", "oceanbaselegacy");
            std::env::set_var("ZO_META_MYSQL_DSN", &dsn);
            std::env::set_var("ZO_META_MYSQL_RO_DSN", &dsn);
            std::env::set_var("ZO_META_DDL_DSN", &dsn);

            // CRITICAL: Set local_mode to false to enable NATS distributed locks
            std::env::set_var("ZO_LOCAL_MODE", "false");
            std::env::set_var("ZO_LOCAL_MODE_STORAGE", "disk");

            // NATS configuration
            std::env::set_var("ZO_CLUSTER_COORDINATOR", "nats");
            std::env::set_var("ZO_NATS_ADDR", &nats_addr);
            std::env::set_var("ZO_NATS_PREFIX", "o2_test_");
            std::env::set_var("ZO_NATS_LOCK_WAIT_TIMEOUT", "30");
            // Use 1 replica for single-node NATS (non-clustered mode)
            std::env::set_var("ZO_NATS_REPLICAS", "1");

            // Set lock timeout for testing (30 seconds)
            std::env::set_var("ZO_META_TRANSACTION_LOCK_TIMEOUT", "30");

            // Set data directory to temp
            let tmp_dir = std::env::temp_dir().join("openobserve_oceanbase_nats_test");
            std::fs::create_dir_all(&tmp_dir).ok();
            std::env::set_var("ZO_DATA_DIR", tmp_dir.to_string_lossy().as_ref());

            // Connection pool settings for tests
            std::env::set_var("ZO_META_CONNECTION_POOL_ACQUIRE_TIMEOUT", "60");
            std::env::set_var("ZO_META_CONNECTION_POOL_MIN_SIZE", "2");
            std::env::set_var("ZO_META_CONNECTION_POOL_MAX_SIZE", "20");
            std::env::set_var("ZO_META_CONNECTION_POOL_IDLE_TIMEOUT", "5");
            std::env::set_var("ZO_META_CONNECTION_POOL_MAX_LIFETIME", "30");
        }

        // Refresh config to pick up new environment variables
        config::refresh_config().expect("Failed to refresh config");

        OB_NATS_CONFIG_INITIALIZED.store(true, Ordering::SeqCst);
        println!(
            "✓ OpenObserve config initialized for OceanBase + NATS tests (DSN: {}, NATS: {})",
            dsn, nats_addr
        );
    });
}

/// Check if config is initialized for OceanBase + NATS tests.
#[cfg(feature = "db-oceanbase-nats-tests")]
#[allow(dead_code)]
pub fn is_oceanbase_nats_config_initialized() -> bool {
    OB_NATS_CONFIG_INITIALIZED.load(Ordering::SeqCst)
}
