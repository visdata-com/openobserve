use async_trait::async_trait;
use config::{get_config, metrics::DB_QUERY_NUMS};

use crate::{dist_lock, errors::Result, file_list::mysql, local_lock};

pub struct OceanbaseFileList {
    legacy: bool,
    mysql_file_list: mysql::MysqlFileList,
}

impl OceanbaseFileList {
    pub fn new() -> Self {
        Self {
            legacy: false,
            mysql_file_list: mysql::MysqlFileList::new(),
        }
    }

    pub fn new_legacy() -> Self {
        Self {
            legacy: true,
            mysql_file_list: mysql::MysqlFileList::new(),
        }
    }
}

impl Default for OceanbaseFileList {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl super::FileList for OceanbaseFileList {
    async fn create_table(&self) -> Result<()> {
        self.mysql_file_list.create_table().await
    }

    async fn create_table_index(&self) -> Result<()> {
        self.mysql_file_list.create_table_index().await
    }

    async fn add(&self, account: &str, file: &str, meta: &super::FileMeta) -> Result<i64> {
        self.mysql_file_list.add(account, file, meta).await
    }

    async fn add_history(&self, account: &str, file: &str, meta: &super::FileMeta) -> Result<i64> {
        self.mysql_file_list.add_history(account, file, meta).await
    }

    async fn remove(&self, file: &str) -> Result<()> {
        self.mysql_file_list.remove(file).await
    }

    async fn batch_add(&self, files: &[super::FileKey]) -> Result<()> {
        self.mysql_file_list.batch_add(files).await
    }

    async fn batch_add_with_id(&self, files: &[super::FileKey]) -> Result<()> {
        self.mysql_file_list.batch_add_with_id(files).await
    }

    async fn batch_add_history(&self, files: &[super::FileKey]) -> Result<()> {
        self.mysql_file_list.batch_add_history(files).await
    }

    async fn update_dump_records(
        &self,
        dump_file: &super::FileKey,
        dumped_ids: &[i64],
    ) -> Result<()> {
        self.mysql_file_list
            .update_dump_records(dump_file, dumped_ids)
            .await
    }

    async fn batch_process(&self, files: &[super::FileKey]) -> Result<()> {
        self.mysql_file_list.batch_process(files).await
    }

    async fn batch_add_deleted(
        &self,
        org_id: &str,
        created_at: i64,
        files: &[super::FileListDeleted],
    ) -> Result<()> {
        self.mysql_file_list
            .batch_add_deleted(org_id, created_at, files)
            .await
    }

    async fn batch_remove_deleted(&self, files: &[super::FileKey]) -> Result<()> {
        self.mysql_file_list.batch_remove_deleted(files).await
    }

    async fn get(&self, file: &str) -> Result<super::FileMeta> {
        self.mysql_file_list.get(file).await
    }

    async fn contains(&self, file: &str) -> Result<bool> {
        self.mysql_file_list.contains(file).await
    }

    async fn update_flattened(&self, file: &str, flattened: bool) -> Result<()> {
        self.mysql_file_list.update_flattened(file, flattened).await
    }

    async fn update_compressed_size(&self, file: &str, size: i64) -> Result<()> {
        self.mysql_file_list
            .update_compressed_size(file, size)
            .await
    }

    async fn list(&self) -> Result<Vec<super::FileKey>> {
        self.mysql_file_list.list().await
    }

    async fn query(
        &self,
        org_id: &str,
        stream_type: super::StreamType,
        stream_name: &str,
        time_level: super::PartitionTimeLevel,
        time_range: (i64, i64),
        flattened: Option<bool>,
    ) -> Result<Vec<super::FileKey>> {
        self.mysql_file_list
            .query(
                org_id,
                stream_type,
                stream_name,
                time_level,
                time_range,
                flattened,
            )
            .await
    }

    async fn query_for_merge(
        &self,
        org_id: &str,
        stream_type: super::StreamType,
        stream_name: &str,
        date_range: (String, String),
    ) -> Result<Vec<super::FileKey>> {
        self.mysql_file_list
            .query_for_merge(org_id, stream_type, stream_name, date_range)
            .await
    }

    async fn query_for_dump(
        &self,
        org_id: &str,
        stream_type: super::StreamType,
        stream_name: &str,
        time_range: (i64, i64),
    ) -> Result<Vec<super::FileRecord>> {
        self.mysql_file_list
            .query_for_dump(org_id, stream_type, stream_name, time_range)
            .await
    }

    async fn query_for_dump_by_updated_at(
        &self,
        time_range: (i64, i64),
    ) -> Result<Vec<super::FileRecord>> {
        self.mysql_file_list
            .query_for_dump_by_updated_at(time_range)
            .await
    }

    async fn query_by_ids(&self, ids: &[i64]) -> Result<Vec<super::FileKey>> {
        self.mysql_file_list.query_by_ids(ids).await
    }

    async fn query_ids(
        &self,
        org_id: &str,
        stream_type: super::StreamType,
        stream_name: &str,
        time_range: (i64, i64),
    ) -> Result<Vec<super::FileId>> {
        self.mysql_file_list
            .query_ids(org_id, stream_type, stream_name, time_range)
            .await
    }

    async fn query_ids_by_files(
        &self,
        files: &[super::FileKey],
    ) -> Result<std::collections::HashMap<String, i64>> {
        self.mysql_file_list.query_ids_by_files(files).await
    }

    async fn query_old_data_hours(
        &self,
        org_id: &str,
        stream_type: super::StreamType,
        stream_name: &str,
        time_range: (i64, i64),
    ) -> Result<Vec<String>> {
        self.mysql_file_list
            .query_old_data_hours(org_id, stream_type, stream_name, time_range)
            .await
    }

    async fn query_deleted(
        &self,
        org_id: &str,
        time_max: i64,
        limit: i64,
    ) -> Result<Vec<super::FileListDeleted>> {
        if !self.legacy {
            return self
                .mysql_file_list
                .query_deleted(org_id, time_max, limit)
                .await;
        }

        if time_max == 0 {
            return Ok(Vec::new());
        }

        let cfg = get_config();
        let lock_key = format!("/lock/meta/{}", "file_list_deleted:query_deleted");

        DB_QUERY_NUMS.with_label_values(&["get_lock", ""]).inc();
        if cfg.common.local_mode {
            let locker = local_lock::lock(&lock_key).await?;
            let guard = locker.lock().await;
            let res = self
                .mysql_file_list
                .query_deleted_inner(org_id, time_max, limit)
                .await;
            DB_QUERY_NUMS.with_label_values(&["release_lock", ""]).inc();
            drop(guard);
            res
        } else {
            let timeout = cfg.limit.meta_transaction_lock_timeout as u64;
            let locker = dist_lock::lock(&lock_key, timeout).await?;
            let result = self
                .mysql_file_list
                .query_deleted_inner(org_id, time_max, limit)
                .await;
            DB_QUERY_NUMS.with_label_values(&["release_lock", ""]).inc();
            if let Err(e) = dist_lock::unlock(&locker).await {
                log::error!(
                    "[OCEANBASE] Failed to unlock NATS lock for {}: {}",
                    lock_key,
                    e
                );
            }
            result
        }
    }

    async fn list_deleted(&self) -> Result<Vec<super::FileListDeleted>> {
        self.mysql_file_list.list_deleted().await
    }

    async fn get_min_date(
        &self,
        org_id: &str,
        stream_type: super::StreamType,
        stream_name: &str,
        date_range: Option<(String, String)>,
    ) -> Result<String> {
        self.mysql_file_list
            .get_min_date(org_id, stream_type, stream_name, date_range)
            .await
    }

    async fn get_min_update_at(&self) -> Result<i64> {
        self.mysql_file_list.get_min_update_at().await
    }

    async fn get_max_update_at(&self) -> Result<i64> {
        self.mysql_file_list.get_max_update_at().await
    }

    async fn clean_by_min_update_at(&self, val: i64) -> Result<()> {
        self.mysql_file_list.clean_by_min_update_at(val).await
    }

    async fn get_updated_streams(&self, time_range: (i64, i64)) -> Result<Vec<String>> {
        self.mysql_file_list.get_updated_streams(time_range).await
    }

    async fn stats_by_date_range(
        &self,
        org_id: &str,
        stream_type: super::StreamType,
        stream_name: &str,
        date_range: (String, String),
    ) -> Result<super::StreamStats> {
        self.mysql_file_list
            .stats_by_date_range(org_id, stream_type, stream_name, date_range)
            .await
    }

    async fn get_stream_stats(
        &self,
        org_id: &str,
        stream_type: Option<super::StreamType>,
        stream_name: Option<&str>,
    ) -> Result<Vec<(String, super::StreamStats)>> {
        self.mysql_file_list
            .get_stream_stats(org_id, stream_type, stream_name)
            .await
    }

    async fn del_stream_stats(
        &self,
        org_id: &str,
        stream_type: super::StreamType,
        stream_name: &str,
    ) -> Result<()> {
        self.mysql_file_list
            .del_stream_stats(org_id, stream_type, stream_name)
            .await
    }

    async fn set_stream_stats(
        &self,
        org_id: &str,
        stream_type: super::StreamType,
        stream_name: &str,
        stats: &super::StreamStats,
        is_recent: bool,
    ) -> Result<()> {
        self.mysql_file_list
            .set_stream_stats(org_id, stream_type, stream_name, stats, is_recent)
            .await
    }

    async fn reset_stream_stats(&self) -> Result<()> {
        self.mysql_file_list.reset_stream_stats().await
    }

    async fn reset_stream_stats_min_ts(
        &self,
        _org_id: &str,
        stream: &str,
        min_ts: i64,
    ) -> Result<()> {
        self.mysql_file_list
            .reset_stream_stats_min_ts(_org_id, stream, min_ts)
            .await
    }

    async fn len(&self) -> usize {
        self.mysql_file_list.len().await
    }

    async fn is_empty(&self) -> bool {
        self.mysql_file_list.is_empty().await
    }

    async fn clear(&self) -> Result<()> {
        self.mysql_file_list.clear().await
    }

    async fn add_job(
        &self,
        org_id: &str,
        stream_type: super::StreamType,
        stream: &str,
        offset: i64,
    ) -> Result<i64> {
        self.mysql_file_list
            .add_job(org_id, stream_type, stream, offset)
            .await
    }

    async fn get_pending_jobs(&self, node: &str, limit: i64) -> Result<Vec<super::MergeJobRecord>> {
        if !self.legacy {
            return self.mysql_file_list.get_pending_jobs(node, limit).await;
        }

        let cfg = get_config();
        let lock_key = format!("/lock/meta/{}", "file_list_jobs:get_pending_jobs");

        DB_QUERY_NUMS.with_label_values(&["get_lock", ""]).inc();
        if cfg.common.local_mode {
            let locker = local_lock::lock(&lock_key).await?;
            let guard = locker.lock().await;
            let res = self
                .mysql_file_list
                .get_pending_jobs_inner(node, limit)
                .await;
            DB_QUERY_NUMS.with_label_values(&["release_lock", ""]).inc();
            drop(guard);
            res
        } else {
            let timeout = cfg.limit.meta_transaction_lock_timeout as u64;
            let locker = dist_lock::lock(&lock_key, timeout).await?;
            let result = self
                .mysql_file_list
                .get_pending_jobs_inner(node, limit)
                .await;
            DB_QUERY_NUMS.with_label_values(&["release_lock", ""]).inc();
            if let Err(e) = dist_lock::unlock(&locker).await {
                log::error!(
                    "[OCEANBASE] Failed to unlock NATS lock for {}: {}",
                    lock_key,
                    e
                );
            }
            result
        }
    }

    async fn get_pending_jobs_count(
        &self,
    ) -> Result<std::collections::HashMap<String, std::collections::HashMap<String, i64>>> {
        self.mysql_file_list.get_pending_jobs_count().await
    }

    async fn set_job_pending(&self, ids: &[i64]) -> Result<()> {
        self.mysql_file_list.set_job_pending(ids).await
    }

    async fn set_job_done(&self, ids: &[i64]) -> Result<()> {
        self.mysql_file_list.set_job_done(ids).await
    }

    async fn update_running_jobs(&self, ids: &[i64]) -> Result<()> {
        self.mysql_file_list.update_running_jobs(ids).await
    }

    async fn check_running_jobs(&self, before_date: i64) -> Result<()> {
        self.mysql_file_list.check_running_jobs(before_date).await
    }

    async fn clean_done_jobs(&self, before_date: i64) -> Result<()> {
        self.mysql_file_list.clean_done_jobs(before_date).await
    }

    async fn get_pending_dump_jobs(
        &self,
        node: &str,
        limit: i64,
    ) -> Result<Vec<(i64, String, i64)>> {
        if !self.legacy {
            return self
                .mysql_file_list
                .get_pending_dump_jobs(node, limit)
                .await;
        }

        let cfg = get_config();
        let lock_key = format!("/lock/meta/{}", "file_list_jobs:get_pending_dump_jobs");

        DB_QUERY_NUMS.with_label_values(&["get_lock", ""]).inc();
        if cfg.common.local_mode {
            let locker = local_lock::lock(&lock_key).await?;
            let guard = locker.lock().await;
            let res = self
                .mysql_file_list
                .get_pending_dump_jobs_inner(node, limit)
                .await;
            DB_QUERY_NUMS.with_label_values(&["release_lock", ""]).inc();
            drop(guard);
            res
        } else {
            let timeout = cfg.limit.meta_transaction_lock_timeout as u64;
            let locker = dist_lock::lock(&lock_key, timeout).await?;
            let result = self
                .mysql_file_list
                .get_pending_dump_jobs_inner(node, limit)
                .await;
            DB_QUERY_NUMS.with_label_values(&["release_lock", ""]).inc();
            if let Err(e) = dist_lock::unlock(&locker).await {
                log::error!(
                    "[OCEANBASE] Failed to unlock NATS lock for {}: {}",
                    lock_key,
                    e
                );
            }
            result
        }
    }

    async fn set_job_dumped_status(&self, ids: &[i64], dumped: bool) -> Result<()> {
        self.mysql_file_list
            .set_job_dumped_status(ids, dumped)
            .await
    }

    async fn insert_dump_stats(&self, file: &str, stats: &super::StreamStats) -> Result<()> {
        self.mysql_file_list.insert_dump_stats(file, stats).await
    }

    async fn delete_dump_stats(&self, file: &str) -> Result<()> {
        self.mysql_file_list.delete_dump_stats(file).await
    }

    async fn query_dump_stats_by_date_range(
        &self,
        org_id: &str,
        stream_type: super::StreamType,
        stream_name: &str,
        date_range: (String, String),
    ) -> Result<super::StreamStats> {
        self.mysql_file_list
            .query_dump_stats_by_date_range(org_id, stream_type, stream_name, date_range)
            .await
    }
}
