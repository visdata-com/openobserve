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

//! OceanBase MetaStore implementation
//!
//! This module provides an OceanBase-specific implementation of the Db trait.
//! OceanBase versions prior to V4.2.0 do not support MySQL's GET_LOCK function,
//! so this implementation uses NATS distributed locks instead.
//!
//! The implementation delegates most methods to MysqlDb, only overriding
//! `get_for_update` to use NATS distributed locks.

use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use hashbrown::HashMap;
use tokio::sync::mpsc;

use super::mysql::MysqlDb;
use crate::{dist_lock, errors::Result, local_lock};

/// OceanBase MetaStore implementation
///
/// Uses NATS distributed locks instead of MySQL GET_LOCK for the `get_for_update`
/// operation, while delegating all other operations to the underlying MysqlDb.
pub struct OceanBaseDb {
    mysql_db: MysqlDb,
    legacy: bool,
}

impl OceanBaseDb {
    pub fn new() -> Self {
        Self {
            mysql_db: MysqlDb::new(),
            legacy: false,
        }
    }

    pub fn new_legacy() -> Self {
        Self {
            mysql_db: MysqlDb::new(),
            legacy: true,
        }
    }
}

impl Default for OceanBaseDb {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl super::Db for OceanBaseDb {
    async fn create_table(&self) -> Result<()> {
        self.mysql_db.create_table().await
    }

    async fn stats(&self) -> Result<super::Stats> {
        self.mysql_db.stats().await
    }

    async fn get(&self, key: &str) -> Result<Bytes> {
        self.mysql_db.get(key).await
    }

    async fn put(
        &self,
        key: &str,
        value: Bytes,
        need_watch: bool,
        start_dt: Option<i64>,
    ) -> Result<()> {
        self.mysql_db.put(key, value, need_watch, start_dt).await
    }

    /// Get for update with distributed or local lock
    ///
    /// This method uses NATS distributed lock instead of MySQL GET_LOCK
    /// to ensure compatibility with OceanBase versions prior to V4.2.0.
    /// In local_mode, it uses a local mutex lock instead.
    async fn get_for_update(
        &self,
        key: &str,
        need_watch: bool,
        start_dt: Option<i64>,
        update_fn: Box<super::UpdateFn>,
    ) -> Result<()> {
        let cfg = config::get_config();
        let lock_key = format!("/lock/meta/{}", key);

        if self.legacy {
            if cfg.common.local_mode {
                // In local mode, use local lock instead of NATS distributed lock
                let locker = local_lock::lock(&lock_key).await?;
                let _guard = locker.lock().await;

                // Execute the inner operation while holding the lock
                self.mysql_db
                    .get_for_update_inner(key, need_watch, start_dt, update_fn)
                    .await
            } else {
                // In cluster mode, use NATS distributed lock
                let timeout = cfg.limit.meta_transaction_lock_timeout as u64;
                let locker = dist_lock::lock(&lock_key, timeout).await?;

                // Execute the inner operation while holding the lock
                let result = self
                    .mysql_db
                    .get_for_update_inner(key, need_watch, start_dt, update_fn)
                    .await;

                // Release the lock
                if let Err(e) = dist_lock::unlock(&locker).await {
                    log::error!(
                        "[OCEANBASE] Failed to unlock NATS lock for key {}: {}",
                        key,
                        e
                    );
                }

                result
            }
        } else {
            // For non-legacy OceanBase versions, delegate to MysqlDb's get_for_update
            self.mysql_db
                .get_for_update(key, need_watch, start_dt, update_fn)
                .await
        }
    }

    async fn delete(
        &self,
        key: &str,
        with_prefix: bool,
        need_watch: bool,
        start_dt: Option<i64>,
    ) -> Result<()> {
        self.mysql_db
            .delete(key, with_prefix, need_watch, start_dt)
            .await
    }

    async fn list(&self, prefix: &str) -> Result<HashMap<String, Bytes>> {
        self.mysql_db.list(prefix).await
    }

    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>> {
        self.mysql_db.list_keys(prefix).await
    }

    async fn list_values(&self, prefix: &str) -> Result<Vec<Bytes>> {
        self.mysql_db.list_values(prefix).await
    }

    async fn list_values_by_start_dt(
        &self,
        prefix: &str,
        start_dt: Option<(i64, i64)>,
    ) -> Result<Vec<(i64, Bytes)>> {
        self.mysql_db
            .list_values_by_start_dt(prefix, start_dt)
            .await
    }

    async fn count(&self, prefix: &str) -> Result<i64> {
        self.mysql_db.count(prefix).await
    }

    async fn watch(&self, prefix: &str) -> Result<Arc<mpsc::Receiver<super::Event>>> {
        self.mysql_db.watch(prefix).await
    }

    async fn close(&self) -> Result<()> {
        self.mysql_db.close().await
    }

    async fn add_start_dt_column(&self) -> Result<()> {
        self.mysql_db.add_start_dt_column().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oceanbase_db_new() {
        let db = OceanBaseDb::new();
        // Verify it contains a MysqlDb
        assert_eq!(std::mem::size_of_val(&db.mysql_db), 0);
    }

    #[test]
    fn test_oceanbase_db_default() {
        let db = OceanBaseDb::default();
        assert_eq!(std::mem::size_of_val(&db.mysql_db), 0);
    }
}
