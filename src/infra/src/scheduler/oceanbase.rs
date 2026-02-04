use async_trait::async_trait;
use config::{get_config, metrics::DB_QUERY_NUMS};

use crate::{dist_lock, errors::Result, local_lock, scheduler::mysql};

pub struct OceanBaseScheduler {
    legacy: bool,
    mysql_scheduler: mysql::MySqlScheduler,
}

impl OceanBaseScheduler {
    pub fn new() -> Self {
        Self {
            legacy: false,
            mysql_scheduler: mysql::MySqlScheduler::new(),
        }
    }

    pub fn new_legacy() -> Self {
        Self {
            legacy: true,
            mysql_scheduler: mysql::MySqlScheduler::new(),
        }
    }
}

impl Default for OceanBaseScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl super::Scheduler for OceanBaseScheduler {
    async fn create_table(&self) -> Result<()> {
        self.mysql_scheduler.create_table().await
    }

    async fn create_table_index(&self) -> Result<()> {
        self.mysql_scheduler.create_table_index().await
    }

    async fn push(&self, trigger: super::Trigger) -> Result<()> {
        self.mysql_scheduler.push(trigger).await
    }

    async fn delete(&self, org: &str, module: super::TriggerModule, key: &str) -> Result<()> {
        self.mysql_scheduler.delete(org, module, key).await
    }

    async fn update_status(
        &self,
        org: &str,
        module: super::TriggerModule,
        key: &str,
        status: super::TriggerStatus,
        retries: i32,
        data: Option<&str>,
    ) -> Result<()> {
        self.mysql_scheduler
            .update_status(org, module, key, status, retries, data)
            .await
    }

    async fn update_trigger(&self, trigger: super::Trigger, clone: bool) -> Result<()> {
        self.mysql_scheduler.update_trigger(trigger, clone).await
    }

    async fn bulk_update_triggers(&self, triggers: Vec<super::Trigger>) -> Result<()> {
        self.mysql_scheduler.bulk_update_triggers(triggers).await
    }

    async fn bulk_update_status(
        &self,
        updates: Vec<(
            String,
            super::TriggerModule,
            String,
            super::TriggerStatus,
            i32,
            Option<String>,
        )>,
    ) -> Result<()> {
        self.mysql_scheduler.bulk_update_status(updates).await
    }

    async fn keep_alive(&self, ids: &[i64], alert_timeout: i64, report_timeout: i64) -> Result<()> {
        self.mysql_scheduler
            .keep_alive(ids, alert_timeout, report_timeout)
            .await
    }

    async fn pull(
        &self,
        concurrency: i64,
        alert_timeout: i64,
        report_timeout: i64,
    ) -> Result<Vec<super::Trigger>> {
        if !self.legacy {
            return self
                .mysql_scheduler
                .pull(concurrency, alert_timeout, report_timeout)
                .await;
        }

        // legacy OceanBase: use NATS/local lock and call pull_inner
        log::debug!("Start pulling scheduled_job (oceanbase legacy)");
        let now = config::utils::time::now_micros();
        let report_max_time = now
            + chrono::Duration::try_seconds(report_timeout)
                .unwrap()
                .num_microseconds()
                .unwrap();
        let alert_max_time = now
            + chrono::Duration::try_seconds(alert_timeout)
                .unwrap()
                .num_microseconds()
                .unwrap();

        let cfg = get_config();
        let lock_key = "/lock/meta/scheduler_pull_lock".to_string();
        DB_QUERY_NUMS
            .with_label_values(&["get_lock", "scheduled_jobs"])
            .inc();
        if cfg.common.local_mode {
            let locker = local_lock::lock(&lock_key).await?;
            let guard = locker.lock().await;
            let res = self
                .mysql_scheduler
                .pull_inner(concurrency, now, report_max_time, alert_max_time)
                .await;
            DB_QUERY_NUMS
                .with_label_values(&["release_lock", "scheduled_jobs"])
                .inc();
            drop(guard);
            res
        } else {
            let timeout = cfg.limit.meta_transaction_lock_timeout as u64;
            let locker = dist_lock::lock(&lock_key, timeout).await?;
            let result = self
                .mysql_scheduler
                .pull_inner(concurrency, now, report_max_time, alert_max_time)
                .await;
            DB_QUERY_NUMS
                .with_label_values(&["release_lock", "scheduled_jobs"])
                .inc();
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

    async fn get(
        &self,
        org: &str,
        module: super::TriggerModule,
        key: &str,
    ) -> Result<super::Trigger> {
        self.mysql_scheduler.get(org, module, key).await
    }

    async fn list(&self, module: Option<super::TriggerModule>) -> Result<Vec<super::Trigger>> {
        self.mysql_scheduler.list(module).await
    }

    async fn list_by_org(
        &self,
        org: &str,
        module: Option<super::TriggerModule>,
    ) -> Result<Vec<super::Trigger>> {
        self.mysql_scheduler.list_by_org(org, module).await
    }

    async fn clean_complete(&self) -> Result<()> {
        self.mysql_scheduler.clean_complete().await
    }

    async fn watch_timeout(&self) -> Result<()> {
        self.mysql_scheduler.watch_timeout().await
    }

    async fn len_module(&self, module: super::TriggerModule) -> usize {
        self.mysql_scheduler.len_module(module).await
    }

    async fn len(&self) -> usize {
        self.mysql_scheduler.len().await
    }

    async fn is_empty(&self) -> bool {
        self.mysql_scheduler.is_empty().await
    }

    async fn clear(&self) -> Result<()> {
        self.mysql_scheduler.clear().await
    }
}
