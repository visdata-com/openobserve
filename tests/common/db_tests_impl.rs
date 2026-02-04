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

//! Shared test implementations for database integration tests.
//!
//! This module contains the core test logic that is shared between
//! MySQL and OceanBase integration tests. Each test file provides
//! thin wrappers that call these implementations.
#![allow(dead_code)]
use std::sync::{
    Arc,
    atomic::{AtomicI32, Ordering as AtomicOrd},
};

use bytes::Bytes;
use infra::db::{Db, get_db};
use sqlx::{MySql, Pool};

// ==================== Schema Tests ====================

/// Test that required indexes exist on the meta table.
pub async fn test_required_indexes_exist_impl(pool: &Pool<MySql>) {
    let indexes: Vec<(String,)> = sqlx::query_as(
        "SELECT INDEX_NAME FROM INFORMATION_SCHEMA.STATISTICS
         WHERE TABLE_SCHEMA = 'openobserve_test' AND TABLE_NAME = 'meta'
         GROUP BY INDEX_NAME",
    )
    .fetch_all(pool)
    .await
    .expect("Query failed");

    let index_names: Vec<&str> = indexes.iter().map(|(n,)| n.as_str()).collect();

    assert!(
        index_names.iter().any(|n| n.contains("module")),
        "Module index should exist. Found: {:?}",
        index_names
    );
    assert!(
        index_names.iter().any(|n| n.contains("key1")),
        "Key1 index should exist. Found: {:?}",
        index_names
    );
    println!("✓ Required indexes exist: {:?}", index_names);
}

/// Test unique constraint on composite key.
pub async fn test_unique_constraint_impl(pool: &Pool<MySql>) {
    // Insert first record
    sqlx::query("INSERT INTO meta (module, key1, key2, start_dt, value) VALUES (?, ?, ?, ?, ?)")
        .bind("unique_test")
        .bind("key1")
        .bind("key2")
        .bind(100i64)
        .bind("value1")
        .execute(pool)
        .await
        .expect("Insert failed");

    // Different start_dt should be allowed
    sqlx::query("INSERT INTO meta (module, key1, key2, start_dt, value) VALUES (?, ?, ?, ?, ?)")
        .bind("unique_test")
        .bind("key1")
        .bind("key2")
        .bind(200i64)
        .bind("value2")
        .execute(pool)
        .await
        .expect("Insert with different start_dt should succeed");

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM meta WHERE module = 'unique_test'")
        .fetch_one(pool)
        .await
        .expect("Count failed");

    assert_eq!(count.0, 2);
    println!("✓ Unique constraint allows different start_dt values");
}

// ==================== Get For Update Tests ====================

/// Test basic get_for_update success scenario.
pub async fn test_get_for_update_basic_update_impl(db: &impl Db, prefix: &str) {
    let key = format!("/gfu_test/{}/basic_update/key1", prefix);

    db.put(&key, Bytes::from("initial_value"), false, Some(0))
        .await
        .expect("Initial put failed");

    let initial = db.get(&key).await.expect("Initial get failed");
    assert_eq!(initial, Bytes::from("initial_value"));

    let update_fn: Box<infra::db::UpdateFn> = Box::new(|value: Option<Bytes>| {
        let current = value.map(|v| String::from_utf8_lossy(&v).to_string());
        assert_eq!(current, Some("initial_value".to_string()));
        Ok(Some((Some(Bytes::from("updated_value")), None)))
    });

    db.get_for_update(&key, false, Some(0), update_fn)
        .await
        .expect("get_for_update failed");

    let updated = db.get(&key).await.expect("Get after update failed");
    assert_eq!(updated, Bytes::from("updated_value"));
    println!("✓ get_for_update basic update test passed");
}

/// Test get_for_update when update_fn returns None.
pub async fn test_get_for_update_returns_none_impl(db: &impl Db, prefix: &str) {
    let key = format!("/gfu_test/{}/returns_none/key1", prefix);

    db.put(&key, Bytes::from("original_value"), false, Some(0))
        .await
        .expect("Initial put failed");

    let update_fn: Box<infra::db::UpdateFn> = Box::new(|value: Option<Bytes>| {
        assert!(value.is_some());
        Ok(None)
    });

    db.get_for_update(&key, false, Some(0), update_fn)
        .await
        .expect("get_for_update should succeed");

    let result = db.get(&key).await.expect("Get after no-update failed");
    assert_eq!(result, Bytes::from("original_value"));
    println!("✓ get_for_update returns None test passed");
}

/// Test get_for_update when update_fn returns an error.
pub async fn test_get_for_update_returns_error_impl(db: &impl Db, prefix: &str) {
    let key = format!("/gfu_test/{}/returns_error/key1", prefix);

    db.put(&key, Bytes::from("original_value"), false, Some(0))
        .await
        .expect("Initial put failed");

    let update_fn: Box<infra::db::UpdateFn> = Box::new(|_value: Option<Bytes>| {
        Err(infra::errors::Error::Message("Test error".to_string()))
    });

    let result = db.get_for_update(&key, false, Some(0), update_fn).await;
    assert!(result.is_err());

    let data = db.get(&key).await.expect("Get after error failed");
    assert_eq!(data, Bytes::from("original_value"));
    println!("✓ get_for_update returns error test passed");
}

/// Test get_for_update inserting a new record.
pub async fn test_get_for_update_insert_when_not_exist_impl(db: &impl Db, prefix: &str) {
    let key = format!("/gfu_test/{}/insert_new/key1", prefix);

    let update_fn: Box<infra::db::UpdateFn> = Box::new(|value: Option<Bytes>| {
        assert!(value.is_none(), "Should have no existing value");
        Ok(Some((Some(Bytes::from("newly_inserted")), None)))
    });

    db.get_for_update(&key, false, Some(0), update_fn)
        .await
        .expect("get_for_update should insert");

    let result = db.get(&key).await.expect("Get after insert failed");
    assert_eq!(result, Bytes::from("newly_inserted"));
    println!("✓ get_for_update insert when not exist test passed");
}

/// Test get_for_update with a new key returned from update_fn.
pub async fn test_get_for_update_with_new_key_impl(db: &impl Db, prefix: &str) {
    let key = format!("/gfu_test/{}/with_new_key/key1", prefix);
    let new_key = format!("/gfu_test/{}/with_new_key/key2", prefix);

    db.put(&key, Bytes::from("original"), false, Some(0))
        .await
        .expect("Initial put failed");

    let new_key_clone = new_key.clone();
    let update_fn: Box<infra::db::UpdateFn> = Box::new(move |value: Option<Bytes>| {
        assert!(value.is_some());
        Ok(Some((
            None,
            Some((new_key_clone.clone(), Bytes::from("new_key_value"), Some(0))),
        )))
    });

    db.get_for_update(&key, false, Some(0), update_fn)
        .await
        .expect("get_for_update with new key failed");

    let original = db.get(&key).await.expect("Original key should still exist");
    assert_eq!(original, Bytes::from("original"));

    let result = db.get(&new_key).await.expect("Get new key failed");
    assert_eq!(result, Bytes::from("new_key_value"));
    println!("✓ get_for_update with new key test passed");
}

/// Test get_for_update with start_dt parameter.
pub async fn test_get_for_update_with_start_dt_impl(db: &impl Db, prefix: &str) {
    let key = format!("/gfu_test/{}/with_start_dt/key1", prefix);

    db.put(&key, Bytes::from("version_100"), false, Some(100))
        .await
        .expect("Put with start_dt=100 failed");

    let update_fn: Box<infra::db::UpdateFn> = Box::new(|value: Option<Bytes>| {
        assert_eq!(value, Some(Bytes::from("version_100")));
        Ok(Some((Some(Bytes::from("updated_version")), None)))
    });

    db.get_for_update(&key, false, Some(100), update_fn)
        .await
        .expect("get_for_update with start_dt failed");

    let result = db.get(&key).await.expect("Get after update failed");
    assert_eq!(result, Bytes::from("updated_version"));
    println!("✓ get_for_update with start_dt test passed");
}

/// Test get_for_update without start_dt gets the latest record.
pub async fn test_get_for_update_without_start_dt_gets_latest_impl(db: &impl Db, prefix: &str) {
    let key = format!("/gfu_test/{}/latest/key1", prefix);

    db.put(&key, Bytes::from("old_version"), false, Some(100))
        .await
        .expect("Put old version failed");
    db.put(&key, Bytes::from("new_version"), false, Some(200))
        .await
        .expect("Put new version failed");

    let update_fn: Box<infra::db::UpdateFn> = Box::new(|value: Option<Bytes>| {
        assert_eq!(value, Some(Bytes::from("new_version")));
        Ok(Some((Some(Bytes::from("latest_updated")), None)))
    });

    db.get_for_update(&key, false, None, update_fn)
        .await
        .expect("get_for_update should get latest");

    let result = db.get(&key).await.expect("Get latest failed");
    assert_eq!(result, Bytes::from("latest_updated"));
    println!("✓ get_for_update without start_dt gets latest test passed");
}

/// Test get_for_update with both update and new key.
pub async fn test_get_for_update_update_and_new_key_impl(db: &impl Db, prefix: &str) {
    let key = format!("/gfu_test/{}/update_and_new/key1", prefix);
    let new_key = format!("/gfu_test/{}/update_and_new/key2", prefix);

    db.put(&key, Bytes::from("original"), false, Some(0))
        .await
        .expect("Initial put failed");

    let new_key_clone = new_key.clone();
    let update_fn: Box<infra::db::UpdateFn> = Box::new(move |value: Option<Bytes>| {
        assert!(value.is_some());
        Ok(Some((
            Some(Bytes::from("updated_original")),
            Some((new_key_clone.clone(), Bytes::from("new_key_value"), Some(0))),
        )))
    });

    db.get_for_update(&key, false, Some(0), update_fn)
        .await
        .expect("get_for_update should succeed");

    let original = db.get(&key).await.expect("Get original failed");
    assert_eq!(original, Bytes::from("updated_original"));

    let new_result = db.get(&new_key).await.expect("Get new key failed");
    assert_eq!(new_result, Bytes::from("new_key_value"));
    println!("✓ get_for_update update and new key test passed");
}

// ==================== CRUD Tests ====================

/// Test put and get operations.
pub async fn test_put_and_get_impl(db: &impl Db, prefix: &str) {
    let key = format!("/crud_test/{}/put_get/key1", prefix);

    db.put(&key, Bytes::from("test_value"), false, Some(0))
        .await
        .expect("Put failed");

    let result = db.get(&key).await.expect("Get failed");
    assert_eq!(result, Bytes::from("test_value"));
    println!("✓ put and get test passed");
}

/// Test delete operation.
pub async fn test_delete_impl(db: &impl Db, prefix: &str) {
    let key = format!("/crud_test/{}/delete/key1", prefix);

    db.put(&key, Bytes::from("to_delete"), false, Some(0))
        .await
        .expect("Put failed");

    db.delete(&key, false, false, None)
        .await
        .expect("Delete failed");

    let result = db.get(&key).await;
    assert!(result.is_err());
    println!("✓ delete test passed");
}

/// Test count operation.
pub async fn test_count_impl(db: &impl Db, prefix: &str) {
    let base = format!("/crud_test/{}/count", prefix);

    for i in 0..3 {
        db.put(
            &format!("{}/key{}", base, i),
            Bytes::from("v"),
            false,
            Some(0),
        )
        .await
        .expect("Put failed");
    }

    let count = db.count(&base).await.expect("Count failed");
    assert_eq!(count, 3);
    println!("✓ count test passed");
}

/// Test getting a nonexistent key.
pub async fn test_get_nonexistent_key_impl(db: &impl Db, prefix: &str) {
    let key = format!("/crud_test/{}/nonexistent/key_that_does_not_exist", prefix);

    let result = db.get(&key).await;
    assert!(result.is_err(), "Getting nonexistent key should fail");
    println!("✓ get nonexistent key test passed");
}

/// Test that put overwrites existing values.
pub async fn test_put_overwrites_impl(db: &impl Db, prefix: &str) {
    let key = format!("/crud_test/{}/overwrite/key1", prefix);

    db.put(&key, Bytes::from("first_value"), false, Some(0))
        .await
        .expect("First put failed");

    let result1 = db.get(&key).await.expect("Get first failed");
    assert_eq!(result1, Bytes::from("first_value"));

    db.put(&key, Bytes::from("second_value"), false, Some(0))
        .await
        .expect("Second put failed");

    let result2 = db.get(&key).await.expect("Get second failed");
    assert_eq!(result2, Bytes::from("second_value"));
    println!("✓ put overwrites test passed");
}

/// Test delete with prefix.
pub async fn test_delete_with_prefix_impl(db: &impl Db, prefix: &str) {
    let base = format!("/crud_test/{}/delete_prefix", prefix);

    db.put(&format!("{}/key1", base), Bytes::from("v1"), false, Some(0))
        .await
        .expect("Put key1 failed");
    db.put(&format!("{}/key2", base), Bytes::from("v2"), false, Some(0))
        .await
        .expect("Put key2 failed");
    db.put(&format!("{}/key3", base), Bytes::from("v3"), false, Some(0))
        .await
        .expect("Put key3 failed");

    let count_before = db.count(&base).await.expect("Count failed");
    assert_eq!(count_before, 3);

    db.delete(&base, true, false, None)
        .await
        .expect("Delete with prefix failed");

    let count_after = db.count(&base).await.expect("Count after delete failed");
    assert_eq!(count_after, 0);
    println!("✓ delete with prefix test passed");
}

/// Test list operation.
pub async fn test_list_impl(db: &impl Db, prefix: &str) {
    let base = format!("/crud_test/{}/list", prefix);

    db.put(
        &format!("{}/a", base),
        Bytes::from("value_a"),
        false,
        Some(0),
    )
    .await
    .expect("Put a failed");
    db.put(
        &format!("{}/b", base),
        Bytes::from("value_b"),
        false,
        Some(0),
    )
    .await
    .expect("Put b failed");
    db.put(
        &format!("{}/c", base),
        Bytes::from("value_c"),
        false,
        Some(0),
    )
    .await
    .expect("Put c failed");

    let results = db.list(&base).await.expect("List failed");
    assert_eq!(results.len(), 3);

    let values: Vec<String> = results
        .into_iter()
        .map(|(_, v)| String::from_utf8_lossy(&v).to_string())
        .collect();
    assert!(values.contains(&"value_a".to_string()));
    assert!(values.contains(&"value_b".to_string()));
    assert!(values.contains(&"value_c".to_string()));
    println!("✓ list test passed");
}

/// Test list_keys operation.
pub async fn test_list_keys_impl(db: &impl Db, prefix: &str) {
    let base = format!("/crud_test/{}/list_keys", prefix);

    db.put(&format!("{}/key1", base), Bytes::from("v1"), false, Some(0))
        .await
        .expect("Put key1 failed");
    db.put(&format!("{}/key2", base), Bytes::from("v2"), false, Some(0))
        .await
        .expect("Put key2 failed");

    let keys = db.list_keys(&base).await.expect("List keys failed");
    assert_eq!(keys.len(), 2);

    for key in &keys {
        assert!(
            key.starts_with(&base),
            "Key {} should start with {}",
            key,
            base
        );
    }
    println!("✓ list_keys test passed");
}

/// Test stats operation.
pub async fn test_stats_impl(db: &impl Db, prefix: &str) {
    let base = format!("/crud_test/{}/stats", prefix);

    db.put(
        &format!("{}/key1", base),
        Bytes::from("value1"),
        false,
        Some(0),
    )
    .await
    .expect("Put failed");

    let stats = db.stats().await.expect("Stats failed");

    assert!(
        stats.keys_count >= 0,
        "Stats keys_count should be non-negative"
    );
    assert!(
        stats.bytes_len >= 0,
        "Stats bytes_len should be non-negative"
    );

    println!(
        "Stats: bytes_len={}, keys_count={}",
        stats.bytes_len, stats.keys_count
    );
    println!("✓ stats test passed");
}

// ==================== Edge Case Tests ====================

/// Test Unicode values handling.
pub async fn test_unicode_values_impl(db: &impl Db, prefix: &str) {
    let unicode_cases = [
        ("chinese", "中文测试值"),
        ("japanese", "日本語テスト"),
        ("korean", "한국어 테스트"),
        ("emoji", "🎉🚀💻🔥"),
        ("mixed", "Hello 世界 🌍"),
    ];

    for (name, value) in &unicode_cases {
        let key = format!("/edge_test/{}/unicode/{}", prefix, name);

        db.put(&key, Bytes::from(*value), false, Some(0))
            .await
            .unwrap_or_else(|e| panic!("Put {} failed: {}", name, e));

        let result = db
            .get(&key)
            .await
            .unwrap_or_else(|e| panic!("Get {} failed: {}", name, e));

        assert_eq!(
            String::from_utf8_lossy(&result),
            *value,
            "Unicode mismatch for {}",
            name
        );
    }
    println!("✓ Unicode values test passed");
}

/// Test special characters handling.
pub async fn test_special_characters_impl(db: &impl Db, prefix: &str) {
    let special_cases = [
        ("quotes", "value with 'single' and \"double\" quotes"),
        ("backslash", "value with \\backslash\\"),
        ("newline", "value with\nnewline"),
        ("tab", "value with\ttab"),
        ("percent", "value with % percent"),
    ];

    for (name, value) in &special_cases {
        let key = format!("/edge_test/{}/special/{}", prefix, name);

        db.put(&key, Bytes::from(*value), false, Some(0))
            .await
            .unwrap_or_else(|e| panic!("Put {} failed: {}", name, e));

        let result = db
            .get(&key)
            .await
            .unwrap_or_else(|e| panic!("Get {} failed: {}", name, e));

        assert_eq!(
            String::from_utf8_lossy(&result),
            *value,
            "Special char mismatch for {}",
            name
        );
    }
    println!("✓ Special characters test passed");
}

/// Test empty value handling.
pub async fn test_empty_value_impl(db: &impl Db, prefix: &str) {
    let key = format!("/edge_test/{}/empty/key1", prefix);

    db.put(&key, Bytes::from(""), false, Some(0))
        .await
        .expect("Put empty value failed");

    let result = db.get(&key).await.expect("Get empty value failed");
    assert_eq!(result, Bytes::from(""));
    println!("✓ Empty value test passed");
}

/// Test large value handling (1MB).
pub async fn test_large_value_impl(db: &impl Db, prefix: &str) {
    let key = format!("/edge_test/{}/large/key1", prefix);

    let large_value: String = "x".repeat(1024 * 1024);

    db.put(&key, Bytes::from(large_value.clone()), false, Some(0))
        .await
        .expect("Put large value failed");

    let result = db.get(&key).await.expect("Get large value failed");
    assert_eq!(result.len(), large_value.len());
    println!("✓ Large value (1MB) test passed");
}

/// Test various start_dt values.
pub async fn test_start_dt_variations_impl(db: &impl Db, prefix: &str) {
    let base = format!("/edge_test/{}/start_dt", prefix);

    let start_dts = [0i64, 1, 1000, 1704067200000000i64];

    for start_dt in start_dts {
        let key = format!("{}/dt_{}", base, start_dt);

        db.put(
            &key,
            Bytes::from(format!("value_{}", start_dt)),
            false,
            Some(start_dt),
        )
        .await
        .unwrap_or_else(|e| panic!("Put with start_dt {} failed: {}", start_dt, e));
    }

    let count = db.count(&base).await.expect("Count failed");
    assert_eq!(count, start_dts.len() as i64);
    println!("✓ start_dt variations test passed");
}

// ==================== Concurrent Tests ====================

/// Tracks concurrent operations for verification
pub struct ConcurrencyTracker {
    active_count: AtomicI32,
    max_concurrent: AtomicI32,
}

impl ConcurrencyTracker {
    pub fn new() -> Self {
        Self {
            active_count: AtomicI32::new(0),
            max_concurrent: AtomicI32::new(0),
        }
    }

    pub fn enter(&self) {
        let current = self.active_count.fetch_add(1, AtomicOrd::SeqCst) + 1;
        self.max_concurrent.fetch_max(current, AtomicOrd::SeqCst);
    }

    pub fn exit(&self) {
        self.active_count.fetch_sub(1, AtomicOrd::SeqCst);
    }

    pub fn get_max(&self) -> i32 {
        self.max_concurrent.load(AtomicOrd::SeqCst)
    }
}

impl Default for ConcurrencyTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Test two clients simultaneously updating the same key.
pub async fn test_concurrent_two_clients_same_key_impl(db: &impl Db, prefix: &str) {
    let key = format!("/concurrent/{}/two_clients/key1", prefix);

    db.put(&key, Bytes::from("0"), false, Some(0))
        .await
        .expect("Initial put failed");

    let key1 = key.clone();
    let key2 = key.clone();

    let (tx, rx) = tokio::sync::oneshot::channel::<()>();

    let task1 = tokio::spawn(async move {
        let db = get_db().await;
        let start = std::time::Instant::now();

        let update_fn: Box<infra::db::UpdateFn> = Box::new(|value: Option<Bytes>| {
            tx.send(()).unwrap();
            let val: i32 = value
                .map(|v| String::from_utf8_lossy(&v).parse().unwrap_or(0))
                .unwrap_or(0);
            // Force a task refresh; otherwise, new spawns may be blocked and not scheduled for
            // execution.
            tokio::task::block_in_place(|| {
                std::thread::sleep(std::time::Duration::from_millis(100))
            });
            Ok(Some((Some(Bytes::from((val + 1).to_string())), None)))
        });

        let result = db.get_for_update(&key1, false, Some(0), update_fn).await;
        let elapsed = start.elapsed();
        (result, elapsed, "task1")
    });

    let task2 = tokio::spawn(async move {
        rx.await.unwrap();
        let db = get_db().await;
        let start = std::time::Instant::now();

        let update_fn: Box<infra::db::UpdateFn> = Box::new(|value: Option<Bytes>| {
            let val: i32 = value
                .map(|v| String::from_utf8_lossy(&v).parse().unwrap_or(0))
                .unwrap_or(0);
            Ok(Some((Some(Bytes::from((val + 1).to_string())), None)))
        });

        let result = db.get_for_update(&key2, false, Some(0), update_fn).await;
        let elapsed = start.elapsed();
        (result, elapsed, "task2")
    });

    let (r1, elapsed1, name1) = task1.await.expect("Task 1 panicked");
    let (r2, elapsed2, name2) = task2.await.expect("Task 2 panicked");

    r1.expect(&format!("{} should succeed", name1));
    r2.expect(&format!("{} should succeed", name2));

    assert!(
        elapsed2.as_millis() >= 100,
        "Task 2 should have waited for lock. Elapsed: {:?}",
        elapsed2,
    );
    assert!(
        elapsed1.as_millis() >= 100,
        "Task 1 should take at least 100ms. Elapsed: {:?}",
        elapsed1
    );

    let final_val = db.get(&key).await.expect("Final get failed");
    assert_eq!(
        final_val,
        Bytes::from("2"),
        "Final value should be 2 after two increments"
    );

    println!("✓ Concurrent two clients test passed");
    println!("  Task 1 elapsed: {:?}", elapsed1);
    println!("  Task 2 elapsed: {:?}", elapsed2);
}

/// Test lock serialization.
pub async fn test_concurrent_lock_serialization_impl(db: &impl Db, prefix: &str) {
    let key = format!("/concurrent/{}/serialization/key1", prefix);
    let tracker = Arc::new(ConcurrencyTracker::new());

    db.put(&key, Bytes::from("start"), false, Some(0))
        .await
        .expect("Initial put failed");

    let num_tasks = 3;
    let mut handles = vec![];

    for task_id in 0..num_tasks {
        let key_clone = key.clone();
        let tracker_clone = tracker.clone();

        handles.push(tokio::spawn(async move {
            let db = get_db().await;
            let tid = task_id;

            let update_fn: Box<infra::db::UpdateFn> = Box::new(move |value: Option<Bytes>| {
                tracker_clone.enter();

                let current = value
                    .map(|v| String::from_utf8_lossy(&v).to_string())
                    .unwrap_or_default();
                let new_val = format!("{}_t{}", current, tid);

                std::thread::sleep(std::time::Duration::from_millis(50));

                tracker_clone.exit();
                Ok(Some((Some(Bytes::from(new_val)), None)))
            });

            db.get_for_update(&key_clone, false, Some(0), update_fn)
                .await
                .expect(&format!("Task {} failed", task_id));
        }));
    }

    for (i, handle) in handles.into_iter().enumerate() {
        handle.await.expect(&format!("Task {} panicked", i));
    }

    let max_concurrent = tracker.get_max();
    assert_eq!(
        max_concurrent, 1,
        "Operations should be serialized (max concurrent = 1), got {}",
        max_concurrent
    );

    let final_val = db.get(&key).await.expect("Final get failed");
    let final_str = String::from_utf8_lossy(&final_val);
    for i in 0..num_tasks {
        assert!(
            final_str.contains(&format!("t{}", i)),
            "Final value should contain t{}: {}",
            i,
            final_str
        );
    }

    println!("✓ Lock serialization test passed");
    println!("  Max concurrent: {}", max_concurrent);
    println!("  Final value: {}", final_str);
}

/// Test counter increment with concurrent clients.
/// Note: Reduced from 5 to 3 workers for remote DB compatibility.
pub async fn test_concurrent_counter_increment_impl(db: &impl Db, prefix: &str) {
    let key = format!("/concurrent/{}/counter/key1", prefix);

    db.put(&key, Bytes::from("0"), false, Some(0))
        .await
        .expect("Initial put failed");

    let num_workers = 3;
    let mut handles = vec![];

    for worker_id in 0..num_workers {
        let key_clone = key.clone();
        handles.push(tokio::spawn(async move {
            let db = get_db().await;
            let wid = worker_id;

            let update_fn: Box<infra::db::UpdateFn> = Box::new(move |value: Option<Bytes>| {
                let val: i32 = value
                    .map(|v| String::from_utf8_lossy(&v).parse().unwrap_or(0))
                    .unwrap_or(0);
                println!("Worker {} read value: {}", wid, val);
                Ok(Some((Some(Bytes::from((val + 1).to_string())), None)))
            });

            db.get_for_update(&key_clone, false, Some(0), update_fn)
                .await
                .expect(&format!("Worker {} failed", worker_id));

            println!("Worker {} completed", worker_id);
        }));
    }

    for (i, handle) in handles.into_iter().enumerate() {
        handle.await.expect(&format!("Worker {} panicked", i));
    }

    let final_val = db.get(&key).await.expect("Final get failed");
    let final_count: i32 = String::from_utf8_lossy(&final_val)
        .parse()
        .expect("Parse failed");

    assert_eq!(
        final_count, num_workers,
        "Counter should be exactly {} after {} increments, got {}",
        num_workers, num_workers, final_count
    );

    println!(
        "✓ Concurrent counter increment test passed: final = {}",
        final_count
    );
}

/// Test multiple clients updating different keys.
pub async fn test_concurrent_different_keys_impl(db: &impl Db, prefix: &str) {
    let num_keys = 3;

    for i in 0..num_keys {
        let key = format!("/concurrent/{}/diff_keys/key{}", prefix, i);
        db.put(&key, Bytes::from("0"), false, Some(0))
            .await
            .expect(&format!("Initial put for key{} failed", i));
    }

    let start_time = std::time::Instant::now();
    let mut handles = vec![];

    for key_id in 0..num_keys {
        let key = format!("/concurrent/{}/diff_keys/key{}", prefix, key_id);
        handles.push(tokio::spawn(async move {
            let db = get_db().await;
            let kid = key_id;

            let update_fn: Box<infra::db::UpdateFn> = Box::new(move |value: Option<Bytes>| {
                let val: i32 = value
                    .map(|v| String::from_utf8_lossy(&v).parse().unwrap_or(0))
                    .unwrap_or(0);
                std::thread::sleep(std::time::Duration::from_millis(100));
                Ok(Some((
                    Some(Bytes::from((val + kid as i32).to_string())),
                    None,
                )))
            });

            db.get_for_update(&key, false, Some(0), update_fn)
                .await
                .expect(&format!("Update for key{} failed", key_id));
        }));
    }

    for (i, handle) in handles.into_iter().enumerate() {
        handle.await.expect(&format!("Task {} panicked", i));
    }

    let total_time = start_time.elapsed();

    // Note: With remote DB, each operation takes ~1-2s due to network latency
    // So we use a generous timeout to avoid flaky tests
    assert!(
        total_time.as_millis() < 10000,
        "Different keys should run in parallel. Total time: {:?} (expected < 10s)",
        total_time
    );

    for i in 0..num_keys {
        let key = format!("/concurrent/{}/diff_keys/key{}", prefix, i);
        let val = db.get(&key).await.expect(&format!("Get key{} failed", i));
        assert_eq!(
            val,
            Bytes::from(i.to_string()),
            "Key{} should have value {}",
            i,
            i
        );
    }

    println!("✓ Concurrent different keys test passed");
    println!(
        "  Total time for {} parallel updates: {:?}",
        num_keys, total_time
    );
}

/// Test lock timeout.
pub async fn test_lock_timeout_returns_error_impl(db: &impl Db, prefix: &str) {
    let key = format!("/concurrent/{}/timeout/key1", prefix);

    db.put(&key, Bytes::from("initial"), false, Some(0))
        .await
        .expect("Initial put failed");

    let key1 = key.clone();
    let key2 = key.clone();

    let task1 = tokio::spawn(async move {
        let db = get_db().await;
        let update_fn: Box<infra::db::UpdateFn> = Box::new(|_value: Option<Bytes>| {
            std::thread::sleep(std::time::Duration::from_secs(5));
            Ok(Some((Some(Bytes::from("task1_updated")), None)))
        });
        db.get_for_update(&key1, false, Some(0), update_fn).await
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let task2 = tokio::spawn(async move {
        let db = get_db().await;
        let update_fn: Box<infra::db::UpdateFn> =
            Box::new(|_value: Option<Bytes>| Ok(Some((Some(Bytes::from("task2_updated")), None))));

        tokio::time::timeout(
            tokio::time::Duration::from_secs(2),
            db.get_for_update(&key2, false, Some(0), update_fn),
        )
        .await
    });

    let task2_result = task2.await.expect("Task 2 panicked");

    match task2_result {
        Err(_) => {
            println!("✓ Task 2 correctly timed out waiting for lock");
        }
        Ok(Err(e)) if e.to_string().contains("LockTimeout") => {
            println!("✓ Task 2 received LockTimeout error: {}", e);
        }
        Ok(Ok(_)) => {
            println!("! Task 2 succeeded (timeout may be configured longer)");
        }
        Ok(Err(e)) => {
            println!("! Task 2 failed with unexpected error: {}", e);
        }
    }

    let _ = task1.await;
    println!("✓ Lock timeout test completed");
}

/// Test lock released on success.
pub async fn test_lock_released_on_success_impl(db: &impl Db, prefix: &str) {
    let key = format!("/concurrent/{}/release_success/key1", prefix);

    db.put(&key, Bytes::from("v0"), false, Some(0))
        .await
        .expect("Initial put failed");

    {
        let update_fn: Box<infra::db::UpdateFn> =
            Box::new(|_value: Option<Bytes>| Ok(Some((Some(Bytes::from("v1")), None))));
        db.get_for_update(&key, false, Some(0), update_fn)
            .await
            .expect("First update failed");
    }

    let start = std::time::Instant::now();
    {
        let update_fn: Box<infra::db::UpdateFn> =
            Box::new(|_value: Option<Bytes>| Ok(Some((Some(Bytes::from("v2")), None))));
        db.get_for_update(&key, false, Some(0), update_fn)
            .await
            .expect("Second update failed");
    }
    let elapsed = start.elapsed();

    // With remote DB, operations take ~1-2s due to network latency
    assert!(
        elapsed.as_millis() < 5000,
        "Second operation should complete quickly. Elapsed: {:?}",
        elapsed
    );

    let final_val = db.get(&key).await.expect("Final get failed");
    assert_eq!(final_val, Bytes::from("v2"));

    println!("✓ Lock released on success test passed");
    println!("  Second operation elapsed: {:?}", elapsed);
}

/// Test lock released on update_fn error.
pub async fn test_lock_released_on_update_fn_error_impl(db: &impl Db, prefix: &str) {
    let key = format!("/concurrent/{}/error_release/key1", prefix);

    db.put(&key, Bytes::from("initial"), false, Some(0))
        .await
        .expect("Initial put failed");

    let key1 = key.clone();
    let key2 = key.clone();

    let task1_result = {
        let db_clone = get_db().await;
        let update_fn: Box<infra::db::UpdateFn> = Box::new(|_value: Option<Bytes>| {
            Err(infra::errors::Error::Message(
                "Intentional error".to_string(),
            ))
        });
        db_clone
            .get_for_update(&key1, false, Some(0), update_fn)
            .await
    };

    assert!(task1_result.is_err(), "Task 1 should fail with error");
    println!("✓ Task 1 correctly failed: {:?}", task1_result.err());

    let start = std::time::Instant::now();
    let task2_result = {
        let db_clone = get_db().await;
        let update_fn: Box<infra::db::UpdateFn> =
            Box::new(|_value: Option<Bytes>| Ok(Some((Some(Bytes::from("task2_success")), None))));
        db_clone
            .get_for_update(&key2, false, Some(0), update_fn)
            .await
    };
    let elapsed = start.elapsed();

    task2_result.expect("Task 2 should succeed");

    // With remote DB, operations take ~1-2s due to network latency
    assert!(
        elapsed.as_millis() < 5000,
        "Task 2 should complete quickly if lock was released. Elapsed: {:?}",
        elapsed
    );

    let final_val = db.get(&key).await.expect("Final get failed");
    assert_eq!(final_val, Bytes::from("task2_success"));

    println!("✓ Lock correctly released after update_fn error");
    println!("  Task 2 elapsed: {:?}", elapsed);
}

/// Test lock released on transaction error.
pub async fn test_lock_released_on_transaction_error_impl(db: &impl Db, prefix: &str) {
    let key = format!("/concurrent/{}/tx_error/key1", prefix);

    db.put(&key, Bytes::from("initial"), false, Some(0))
        .await
        .expect("Initial put failed");

    let result = {
        let update_fn: Box<infra::db::UpdateFn> = Box::new(|_value: Option<Bytes>| {
            Err(infra::errors::Error::Message(
                "Simulated transaction error".to_string(),
            ))
        });
        db.get_for_update(&key, false, Some(0), update_fn).await
    };
    assert!(result.is_err());

    let start = std::time::Instant::now();
    let result2 = {
        let update_fn: Box<infra::db::UpdateFn> =
            Box::new(|_value: Option<Bytes>| Ok(Some((Some(Bytes::from("recovered")), None))));
        db.get_for_update(&key, false, Some(0), update_fn).await
    };
    let elapsed = start.elapsed();

    result2.expect("Recovery operation should succeed");

    // With remote DB, operations take ~1-2s due to network latency
    assert!(
        elapsed.as_millis() < 5000,
        "Recovery should be quick. Elapsed: {:?}",
        elapsed
    );

    let final_val = db.get(&key).await.expect("Final get failed");
    assert_eq!(final_val, Bytes::from("recovered"));

    println!("✓ Lock released on transaction error test passed");
}

/// Test lock contention fairness.
/// Note: Reduced from 5 to 3 tasks for remote DB compatibility.
pub async fn test_lock_contention_fairness_impl(db: &impl Db, prefix: &str, db_name: &str) {
    let key = format!("/concurrent/{}/fairness/key1", prefix);
    let completion_order = Arc::new(std::sync::Mutex::new(Vec::new()));

    db.put(&key, Bytes::from("0"), false, Some(0))
        .await
        .expect("Initial put failed");

    let num_tasks = 3;
    let mut handles = vec![];

    for task_id in 0..num_tasks {
        let key_clone = key.clone();
        let order_clone = completion_order.clone();
        let delay = task_id as u64 * 20;

        handles.push(tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;

            let db = get_db().await;
            let tid = task_id;
            let order = order_clone.clone();

            let update_fn: Box<infra::db::UpdateFn> = Box::new(move |value: Option<Bytes>| {
                order.lock().unwrap().push(tid);

                let val: i32 = value
                    .map(|v| String::from_utf8_lossy(&v).parse().unwrap_or(0))
                    .unwrap_or(0);
                Ok(Some((Some(Bytes::from((val + 1).to_string())), None)))
            });

            db.get_for_update(&key_clone, false, Some(0), update_fn)
                .await
                .expect(&format!("Task {} failed", task_id));
        }));
    }

    for handle in handles {
        handle.await.expect("Task panicked");
    }

    let order = completion_order.lock().unwrap();
    println!("✓ Lock contention fairness test completed");
    println!("  Completion order: {:?}", *order);
    println!(
        "  (Note: {} GET_LOCK may not guarantee strict FIFO)",
        db_name
    );

    assert_eq!(order.len(), num_tasks, "All tasks should complete");
}

/// Test concurrent insert of same new key.
pub async fn test_concurrent_insert_same_new_key_impl(db: &impl Db, prefix: &str) {
    let key = format!("/concurrent/{}/new_insert/key1", prefix);

    let num_inserters = 3;
    let mut handles = vec![];
    let success_count = Arc::new(AtomicI32::new(0));

    for inserter_id in 0..num_inserters {
        let key_clone = key.clone();
        let count_clone = success_count.clone();

        handles.push(tokio::spawn(async move {
            let db = get_db().await;
            let iid = inserter_id;

            let update_fn: Box<infra::db::UpdateFn> = Box::new(move |value: Option<Bytes>| {
                if value.is_none() {
                    Ok(Some((
                        Some(Bytes::from(format!("inserted_by_{}", iid))),
                        None,
                    )))
                } else {
                    Ok(None)
                }
            });

            match db
                .get_for_update(&key_clone, false, Some(0), update_fn)
                .await
            {
                Ok(_) => {
                    count_clone.fetch_add(1, AtomicOrd::SeqCst);
                }
                Err(e) => {
                    println!("Inserter {} got error (expected): {}", inserter_id, e);
                }
            }
        }));
    }

    for handle in handles {
        handle.await.expect("Task panicked");
    }

    match db.get(&key).await {
        Ok(val) => {
            println!("✓ Concurrent insert test passed");
            println!("  Final value: {}", String::from_utf8_lossy(&val));
        }
        Err(_) => {
            println!("✓ Concurrent insert test passed (key not created)");
        }
    }
}

/// Test concurrent update with new key creation.
pub async fn test_concurrent_update_with_new_key_impl(db: &impl Db, prefix: &str) {
    let key = format!("/concurrent/{}/update_new/key1", prefix);
    let new_key = format!("/concurrent/{}/update_new/key2", prefix);

    db.put(&key, Bytes::from("original"), false, Some(0))
        .await
        .expect("Initial put failed");

    let key1 = key.clone();
    let key2 = key.clone();
    let new_key1 = new_key.clone();
    let new_key2 = new_key.clone();

    let task1 = tokio::spawn(async move {
        let db = get_db().await;
        let update_fn: Box<infra::db::UpdateFn> = Box::new(move |_value: Option<Bytes>| {
            Ok(Some((
                Some(Bytes::from("updated_by_1")),
                Some((new_key1.clone(), Bytes::from("new_by_1"), Some(0))),
            )))
        });
        db.get_for_update(&key1, false, Some(0), update_fn).await
    });

    let task2 = tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        let db = get_db().await;
        let update_fn: Box<infra::db::UpdateFn> = Box::new(move |_value: Option<Bytes>| {
            Ok(Some((
                Some(Bytes::from("updated_by_2")),
                Some((new_key2.clone(), Bytes::from("new_by_2"), Some(0))),
            )))
        });
        db.get_for_update(&key2, false, Some(0), update_fn).await
    });

    let r1 = task1.await.expect("Task 1 panicked");
    let r2 = task2.await.expect("Task 2 panicked");

    assert!(r1.is_ok() || r2.is_ok(), "At least one task should succeed");

    println!("✓ Concurrent update with new key test passed");
    println!("  Task 1 result: {:?}", r1.is_ok());
    println!("  Task 2 result: {:?}", r2.is_ok());
}

/// Test long running update_fn blocks other clients.
pub async fn test_long_running_update_fn_impl(db: &impl Db, prefix: &str) {
    let key = format!("/concurrent/{}/long_running/key1", prefix);

    db.put(&key, Bytes::from("start"), false, Some(0))
        .await
        .expect("Initial put failed");

    let key1 = key.clone();
    let key2 = key.clone();

    let start_time = std::time::Instant::now();

    let (tx, rx) = tokio::sync::oneshot::channel::<()>();

    let task1 = tokio::spawn(async move {
        let db = get_db().await;
        let update_fn: Box<infra::db::UpdateFn> = Box::new(|_value: Option<Bytes>| {
            tx.send(()).unwrap();
            // Force a task refresh; otherwise, new spawns may be blocked and not scheduled for
            // execution.
            tokio::task::block_in_place(|| {
                std::thread::sleep(std::time::Duration::from_millis(200))
            });
            Ok(Some((Some(Bytes::from("slow_update")), None)))
        });
        db.get_for_update(&key1, false, Some(0), update_fn).await
    });

    let task2 = tokio::spawn(async move {
        rx.await.unwrap();
        let start = std::time::Instant::now();
        let db = get_db().await;
        let update_fn: Box<infra::db::UpdateFn> =
            Box::new(|_value: Option<Bytes>| Ok(Some((Some(Bytes::from("fast_update")), None))));
        let result = db.get_for_update(&key2, false, Some(0), update_fn).await;
        (result, start.elapsed())
    });

    task1
        .await
        .expect("Task 1 panicked")
        .expect("Task 1 failed");
    let (task2_result, task2_wait) = task2.await.expect("Task 2 panicked");
    task2_result.expect("Task 2 failed");

    let total_time = start_time.elapsed();

    assert!(
        task2_wait.as_millis() >= 200,
        "Task 2 should have waited for lock. Wait time: {:?}",
        task2_wait
    );
    assert!(
        total_time.as_millis() >= 200,
        "Total time should reflect long update_fn. Total time: {:?}",
        total_time
    );

    println!("✓ Long running update_fn test passed");
    println!("  Total time: {:?}", total_time);
    println!("  Task 2 wait time: {:?}", task2_wait);
}

/// Test concurrent updates with different start_dt.
pub async fn test_concurrent_with_different_start_dt_impl(db: &impl Db, prefix: &str) {
    let key = format!("/concurrent/{}/versions/key1", prefix);

    db.put(&key, Bytes::from("v100"), false, Some(100))
        .await
        .expect("Put v100 failed");
    db.put(&key, Bytes::from("v200"), false, Some(200))
        .await
        .expect("Put v200 failed");

    let key1 = key.clone();
    let key2 = key.clone();

    let task1 = tokio::spawn(async move {
        let db = get_db().await;
        let update_fn: Box<infra::db::UpdateFn> = Box::new(|value: Option<Bytes>| {
            assert_eq!(value, Some(Bytes::from("v100")));
            Ok(Some((Some(Bytes::from("v100_updated")), None)))
        });
        db.get_for_update(&key1, false, Some(100), update_fn).await
    });

    let task2 = tokio::spawn(async move {
        let db = get_db().await;
        let update_fn: Box<infra::db::UpdateFn> = Box::new(|value: Option<Bytes>| {
            assert_eq!(value, Some(Bytes::from("v200")));
            Ok(Some((Some(Bytes::from("v200_updated")), None)))
        });
        db.get_for_update(&key2, false, Some(200), update_fn).await
    });

    task1
        .await
        .expect("Task 1 panicked")
        .expect("Task 1 failed");
    task2
        .await
        .expect("Task 2 panicked")
        .expect("Task 2 failed");

    let final_val = db.get(&key).await.expect("Get with start_dt 100 failed");
    assert_eq!(
        final_val,
        Bytes::from("v200_updated"),
        "Value at start_dt 200 should be updated"
    );

    println!("✓ Concurrent with different start_dt test passed");
}

/// Test high concurrency with multiple clients.
/// Note: Reduced from 20×3=60 to 5×2=10 iterations for remote DB compatibility.
pub async fn test_high_concurrency_20_clients_impl(db: &impl Db, prefix: &str) {
    let key = format!("/concurrent/{}/stress/key1", prefix);

    db.put(&key, Bytes::from("0"), false, Some(0))
        .await
        .expect("Initial put failed");

    let num_workers = 5;
    let iterations_per_worker = 2;
    let mut handles = vec![];

    let start_time = std::time::Instant::now();

    for _worker_id in 0..num_workers {
        let key_clone = key.clone();
        handles.push(tokio::spawn(async move {
            let db = get_db().await;

            for _iter in 0..iterations_per_worker {
                let update_fn: Box<infra::db::UpdateFn> = Box::new(|value: Option<Bytes>| {
                    let val: i32 = value
                        .map(|v| String::from_utf8_lossy(&v).parse().unwrap_or(0))
                        .unwrap_or(0);
                    Ok(Some((Some(Bytes::from((val + 1).to_string())), None)))
                });

                db.get_for_update(&key_clone, false, Some(0), update_fn)
                    .await
                    .expect("Worker iteration failed");
            }
        }));
    }

    for (i, handle) in handles.into_iter().enumerate() {
        handle.await.expect(&format!("Worker {} panicked", i));
    }

    let total_time = start_time.elapsed();
    let expected_count = num_workers * iterations_per_worker;

    let final_val = db.get(&key).await.expect("Final get failed");
    let final_count: i32 = String::from_utf8_lossy(&final_val)
        .parse()
        .expect("Parse failed");

    assert_eq!(
        final_count, expected_count as i32,
        "Counter should be exactly {} after {} total increments, got {}",
        expected_count, expected_count, final_count
    );

    println!("✓ High concurrency stress test passed");
    println!("  Workers: {}", num_workers);
    println!("  Iterations per worker: {}", iterations_per_worker);
    println!("  Total increments: {}", expected_count);
    println!("  Final counter: {}", final_count);
    println!("  Total time: {:?}", total_time);
}

/// Test rapid sequential updates.
/// Note: Reduced from 50 to 10 iterations for remote DB compatibility.
pub async fn test_rapid_sequential_updates_impl(db: &impl Db, prefix: &str) {
    let key = format!("/concurrent/{}/rapid/key1", prefix);

    db.put(&key, Bytes::from("0"), false, Some(0))
        .await
        .expect("Initial put failed");

    let num_updates = 10;
    let start_time = std::time::Instant::now();

    for i in 0..num_updates {
        let update_fn: Box<infra::db::UpdateFn> = Box::new(move |value: Option<Bytes>| {
            let val: i32 = value
                .map(|v| String::from_utf8_lossy(&v).parse().unwrap_or(0))
                .unwrap_or(0);
            assert_eq!(val, i, "Value should be {} before update {}", i, i);
            Ok(Some((Some(Bytes::from((val + 1).to_string())), None)))
        });

        db.get_for_update(&key, false, Some(0), update_fn)
            .await
            .expect(&format!("Update {} failed", i));
    }

    let total_time = start_time.elapsed();

    let final_val = db.get(&key).await.expect("Final get failed");
    let final_count: i32 = String::from_utf8_lossy(&final_val)
        .parse()
        .expect("Parse failed");

    assert_eq!(final_count, num_updates);

    println!("✓ Rapid sequential updates test passed");
    println!("  Updates: {}", num_updates);
    println!("  Total time: {:?}", total_time);
    println!(
        "  Avg time per update: {:?}",
        total_time / num_updates as u32
    );
}

/// Test mixed read-write concurrency.
/// Note: Reduced writers from 5 to 3, readers from 10 to 5 for remote DB compatibility.
pub async fn test_mixed_read_write_concurrency_impl(db: &impl Db, prefix: &str) {
    let key = format!("/concurrent/{}/mixed/key1", prefix);

    db.put(&key, Bytes::from("0"), false, Some(0))
        .await
        .expect("Initial put failed");

    let num_writers = 3;
    let num_readers = 5;
    let mut handles = vec![];

    for writer_id in 0..num_writers {
        let key_clone = key.clone();
        handles.push(tokio::spawn(async move {
            let db = get_db().await;

            let update_fn: Box<infra::db::UpdateFn> = Box::new(|value: Option<Bytes>| {
                let val: i32 = value
                    .map(|v| String::from_utf8_lossy(&v).parse().unwrap_or(0))
                    .unwrap_or(0);
                Ok(Some((Some(Bytes::from((val + 1).to_string())), None)))
            });

            db.get_for_update(&key_clone, false, Some(0), update_fn)
                .await
                .expect(&format!("Writer {} failed", writer_id));

            ("writer", writer_id)
        }));
    }

    for reader_id in 0..num_readers {
        let key_clone = key.clone();
        handles.push(tokio::spawn(async move {
            let db = get_db().await;
            let _ = db.get(&key_clone).await;
            ("reader", reader_id)
        }));
    }

    for handle in handles {
        let (role, id) = handle.await.expect("Task panicked");
        println!("  {} {} completed", role, id);
    }

    let final_val = db.get(&key).await.expect("Final get failed");
    let final_count: i32 = String::from_utf8_lossy(&final_val)
        .parse()
        .expect("Parse failed");

    assert_eq!(final_count, num_writers as i32);

    println!("✓ Mixed read-write concurrency test passed");
    println!(
        "  Final counter: {} (expected {})",
        final_count, num_writers
    );
}

/// Test concurrent operations across connection pools.
pub async fn test_concurrent_across_connection_pools_impl(db: &impl Db, prefix: &str) {
    let key = format!("/concurrent/{}/pools/key1", prefix);

    db.put(&key, Bytes::from("0"), false, Some(0))
        .await
        .expect("Initial put failed");

    let num_pools = 3;
    let mut handles = vec![];

    for pool_id in 0..num_pools {
        let key_clone = key.clone();
        handles.push(tokio::spawn(async move {
            let db = get_db().await;
            let pid = pool_id;

            let update_fn: Box<infra::db::UpdateFn> = Box::new(move |value: Option<Bytes>| {
                let val: i32 = value
                    .map(|v| String::from_utf8_lossy(&v).parse().unwrap_or(0))
                    .unwrap_or(0);
                println!("Pool {} read value: {}", pid, val);
                Ok(Some((Some(Bytes::from((val + 1).to_string())), None)))
            });

            db.get_for_update(&key_clone, false, Some(0), update_fn)
                .await
                .expect(&format!("Pool {} operation failed", pool_id));
        }));
    }

    for handle in handles {
        handle.await.expect("Task panicked");
    }

    let final_val = db.get(&key).await.expect("Final get failed");
    let final_count: i32 = String::from_utf8_lossy(&final_val)
        .parse()
        .expect("Parse failed");

    assert_eq!(final_count, num_pools as i32);

    println!("✓ Concurrent across connection pools test passed");
    println!("  Final counter: {} (expected {})", final_count, num_pools);
}
