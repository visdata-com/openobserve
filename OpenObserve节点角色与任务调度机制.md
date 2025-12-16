# OpenObserve 节点角色与任务调度机制

## 目录

1. [核心问题](#核心问题)
2. [节点角色类型](#节点角色类型)
3. [任务分配机制](#任务分配机制)
4. [重复处理防护机制](#重复处理防护机制)
5. [节点角色配置](#节点角色配置)
6. [最佳实践](#最佳实践)

---

## 核心问题

**问题**: 每个节点上运行的 OpenObserve 组件 (ingest、compactor、alert 等) 功能是完全一致的吗? 会不会有重复处理数据的情况?

**简短回答**:
- ❌ **不一致**: 每个节点根据配置的 `NODE_ROLE` 运行**不同的组件**
- ✅ **不会重复**: 通过**分布式锁**和**数据库任务调度器**确保每个任务**只被一个节点处理**

---

## 节点角色类型

### 节点角色枚举

**代码位置**: [src/config/src/meta/cluster.rs:195-205](src/config/src/meta/cluster.rs#L195-L205)

```rust
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, ToSchema)]
pub enum Role {
    All,              // 所有功能 (单机模式)
    Ingester,         // 数据摄取
    Querier,          // 数据查询
    Compactor,        // 数据压缩
    Router,           // 路由转发
    AlertManager,     // 告警管理
    FlattenCompactor, // 扁平化压缩
    ScriptServer,     // 脚本服务器 (企业版)
}
```

### 节点角色判断方法

**代码位置**: [src/config/src/meta/cluster.rs:102-130](src/config/src/meta/cluster.rs#L102-L130)

```rust
impl Node {
    pub fn is_router(&self) -> bool {
        self.role.contains(&Role::Router)
    }

    pub fn is_ingester(&self) -> bool {
        self.role.contains(&Role::Ingester) || self.role.contains(&Role::All)
    }

    pub fn is_querier(&self) -> bool {
        self.role.contains(&Role::Querier) || self.role.contains(&Role::All)
    }

    pub fn is_compactor(&self) -> bool {
        self.role.contains(&Role::Compactor) || self.role.contains(&Role::All)
    }

    pub fn is_alert_manager(&self) -> bool {
        self.role.contains(&Role::AlertManager) || self.role.contains(&Role::All)
    }

    pub fn is_flatten_compactor(&self) -> bool {
        self.role.contains(&Role::FlattenCompactor)
    }

    pub fn is_script_server(&self) -> bool {
        self.role.contains(&Role::ScriptServer) || self.role.contains(&Role::All)
    }
}
```

### 节点角色分组

**代码位置**: [src/config/src/meta/cluster.rs:244-250](src/config/src/meta/cluster.rs#L244-L250)

```rust
/// 将节点分为不同的优先级组
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, Default, ToSchema)]
pub enum RoleGroup {
    #[default]
    None,          // 所有任务
    Interactive,   // 高优先级任务 (用户交互查询)
    Background,    // 低优先级任务 (定时报表、后台作业)
}
```

**用途**: 分离交互式查询和后台任务,避免后台任务影响用户查询性能。

---

## 任务分配机制

### 1. 组件启动条件判断

**代码位置**: [src/job/mod.rs:79-400](src/job/mod.rs#L79-L400)

每个组件在启动时都会**检查当前节点的角色**,只有满足条件才会启动:

#### 1.1 Ingester 组件

```rust
// 代码位置: src/job/mod.rs:290-295
if LOCAL_NODE.is_ingester() {
    // 创建 WAL 目录
    if let Err(e) = std::fs::create_dir_all(&cfg.common.data_wal_dir) {
        log::error!("Failed to create wal dir: {e}");
    }
}
```

**功能**:
- 接收数据写入请求
- 写入 WAL (Write-Ahead Log)
- 维护 MemTable
- 持久化到 Parquet 文件

**启动条件**: `role = "ingester"` 或 `role = "all"`

#### 1.2 Querier 组件

```rust
// 代码位置: src/job/mod.rs:196-206
if cfg.common.telemetry_enabled && LOCAL_NODE.is_querier() {
    spawn_pausable_job!(
        "telemetry",
        config::get_config().common.telemetry_heartbeat,
        {
            crate::common::meta::telemetry::Telemetry::new()
                .heart_beat("OpenObserve - heartbeat", None)
                .await;
        }
    );
}
```

**功能**:
- 接收查询请求
- 协调分布式查询
- 从 MemTable、Parquet 和对象存储读取数据
- 返回查询结果

**启动条件**: `role = "querier"` 或 `role = "all"`

#### 1.3 Compactor 组件

**代码位置**: [src/job/compactor.rs:29-32](src/job/compactor.rs#L29-L32)

```rust
pub async fn run() -> Result<(), anyhow::Error> {
    if !LOCAL_NODE.is_compactor() {
        return Ok(());  // 不是 compactor 角色,直接返回
    }

    let cfg = get_config();
    if !cfg.compact.enabled {
        return Ok(());  // 压缩功能未启用,直接返回
    }
    log::info!("[COMPACTOR::JOB] Compactor is enabled");

    // 启动各种压缩任务...
}
```

**功能** (代码位置: [src/job/compactor.rs:51-168](src/job/compactor.rs#L51-L168)):

```rust
// 1. 生成合并任务 (当前数据)
spawn_pausable_job!("run_generate_job", get_config().compact.interval, {
    compact::run_generate_job(CompactionJobType::Current).await
});

// 2. 生成合并任务 (历史数据)
spawn_pausable_job!("run_generate_old_data_job", get_config().compact.old_data_interval, {
    compact::run_generate_job(CompactionJobType::Historical).await
});

// 3. 执行数据合并
spawn_pausable_job!("run_merge", get_config().compact.interval + 2, {
    compact::run_merge(scheduler.tx().clone()).await
});

// 4. 数据保留 (删除过期数据)
spawn_pausable_job!("run_retention", get_config().compact.interval + 3, {
    compact::run_retention().await
});

// 5. 延迟删除
spawn_pausable_job!("run_delay_deletion", get_config().compact.interval + 4, {
    compact::run_delay_deletion().await
});

// 6. 同步压缩偏移量到数据库
spawn_pausable_job!("compactor_sync_to_db", get_config().compact.sync_to_db_interval, {
    crate::service::db::compact::files::sync_cache_to_db().await
});

// 7. 检查运行中的任务 (超时处理)
spawn_pausable_job!("compactor_check_running_jobs", get_config().compact.job_run_timeout, {
    let timeout = get_config().compact.job_run_timeout;
    let updated_at = config::utils::time::now_micros() - (timeout * 1000 * 1000);
    infra::file_list::check_running_jobs(updated_at).await
});

// 8. 清理已完成的任务
spawn_pausable_job!("compactor_clean_done_jobs", get_config().compact.job_clean_wait_time, {
    let wait_time = get_config().compact.job_clean_wait_time;
    let updated_at = config::utils::time::now_micros() - (wait_time * 1000 * 1000);
    infra::file_list::clean_done_jobs(updated_at).await
});
```

**启动条件**: `role = "compactor"` 或 `role = "all"` **且** `compact.enabled = true`

#### 1.4 AlertManager 组件

**代码位置**: [src/job/alert_manager.rs:22-25](src/job/alert_manager.rs#L22-L25)

```rust
pub async fn run() -> Result<(), anyhow::Error> {
    if !LOCAL_NODE.is_alert_manager() {
        return Ok(());  // 不是 alert_manager 角色,直接返回
    }

    // 启动告警调度器...
}
```

**功能**:

```rust
// 1. 报表服务器 (如果启用)
if cfg.report_server.enable_report_server {
    tokio::task::spawn(async move {
        report_server::spawn_server().await
    });
}

// 2. 调度任务执行器
tokio::task::spawn(async move { run_schedule_jobs().await });

// 3. 监控超时任务
spawn_pausable_job!("alert_manager_watch_timeout", get_config().limit.scheduler_watch_interval, {
    infra::scheduler::watch_timeout().await
});

// 4. 搜索作业执行器 (企业版)
#[cfg(feature = "enterprise")]
for i in 0..cfg.limit.search_job_workers {
    spawn_pausable_job!(format!("search_job_worker_{}", i), ..., {
        service::search_jobs::run(i).await
    });
}

// 5. 告警去重状态清理
spawn_pausable_job!("alert_dedup_cleanup", 3600, {
    cleanup_alert_dedup_state().await
});
```

**启动条件**: `role = "alert_manager"` 或 `role = "all"`

#### 1.5 Router 组件

**代码位置**: [src/job/mod.rs:190-193](src/job/mod.rs#L190-L193)

```rust
// Router 不需要初始化后台任务
if LOCAL_NODE.is_router() && LOCAL_NODE.is_single_role() {
    return Ok(());
}
```

**功能**:
- 接收摄取请求
- 转发到 Ingester 节点
- 负载均衡
- 不执行任何后台任务

**启动条件**: `role = "router"`

**gRPC 服务**: 仅启动简化的 OpenTelemetry 摄取服务 (代码位置: [src/main.rs:687-741](src/main.rs#L687-L741))

### 2. Pipeline 任务选择性启动

**代码位置**: [src/job/mod.rs:233-244](src/job/mod.rs#L233-L244)

```rust
// Pipeline 不用于 compactor
if LOCAL_NODE.is_ingester() || LOCAL_NODE.is_querier() || LOCAL_NODE.is_alert_manager() {
    tokio::task::spawn(async move { db::pipeline::watch().await });
}

// Session 管理 (企业版)
#[cfg(feature = "enterprise")]
if LOCAL_NODE.is_ingester() || LOCAL_NODE.is_querier() {
    tokio::task::spawn(db::session::watch());
}

// Enrichment Table 监控
if LOCAL_NODE.is_ingester() || LOCAL_NODE.is_querier() || LOCAL_NODE.is_alert_manager() {
    tokio::task::spawn(db::enrichment_table::watch());
}
```

**启动逻辑**: 不同的元数据监控任务根据节点角色选择性启动,避免不必要的资源消耗。

---

## 重复处理防护机制

### 1. 数据库调度器 + 分布式锁

#### 调度器架构

**代码位置**: [src/service/alerts/scheduler/worker.rs](src/service/alerts/scheduler/worker.rs)

```
┌────────────────────────────────────────────────────────┐
│              Scheduler (调度器主控)                      │
│                                                        │
│  - SchedulerJobPuller (任务拉取器)                      │
│  - SchedulerWorker x N (工作线程池)                     │
└────────────────────────────────────────────────────────┘
                            │
                            ▼
┌────────────────────────────────────────────────────────┐
│            MySQL/PostgreSQL/SQLite Database            │
│                                                        │
│  Table: scheduled_jobs                                 │
│  ┌──────────────────────────────────────────────┐     │
│  │ id  org  module  module_key  status  next_run│     │
│  ├──────────────────────────────────────────────┤     │
│  │ 1   org1 Alert   alert_key1  Waiting  ...    │     │
│  │ 2   org1 Report  report_1    Running  ...    │     │
│  │ 3   org2 Alert   alert_key2  Waiting  ...    │     │
│  └──────────────────────────────────────────────┘     │
│                                                        │
│  分布式锁机制: 使用 UPDATE + WHERE 子句                  │
└────────────────────────────────────────────────────────┘
                            │
                            ▼
┌────────────────────────────────────────────────────────┐
│              任务执行流程                                │
│                                                        │
│  1. JobPuller 定期拉取 (每 N 秒)                        │
│  2. 使用 SQL 原子操作获取任务                            │
│  3. 更新状态为 "Running"                                │
│  4. 分发到 Worker                                       │
│  5. 执行完成后更新状态                                   │
└────────────────────────────────────────────────────────┘
```

#### 1.1 任务拉取 (Pull) - 原子操作

**代码位置**: [src/service/alerts/scheduler/worker.rs:173-195](src/service/alerts/scheduler/worker.rs#L173-L195)

```rust
pub async fn run(&self) -> Result<()> {
    let interval = self.config.poll_interval_secs;
    let mut interval = time::interval(time::Duration::from_secs(interval));
    interval.tick().await; // 第一次 tick

    loop {
        let trace_id = config::ider::uuid();
        log::debug!("[SCHEDULER][JobPuller-{trace_id}] Pulling jobs");

        // 从数据库拉取任务 (原子操作)
        let triggers = match self.pull().await {
            Ok(triggers) => triggers,
            Err(e) => {
                log::error!("[SCHEDULER][JobPuller-{trace_id}] Error pulling triggers: {e}");
                continue;
            }
        };

        log::debug!(
            "[SCHEDULER][JobPuller-{}] Pulled {} jobs from scheduler",
            trace_id,
            triggers.len()
        );

        // ... 处理任务
    }
}
```

**SQL 实现** (代码位置: src/infra/src/scheduler/mysql.rs):

```sql
-- 原子获取并锁定任务
UPDATE scheduled_jobs
SET
    status = 'Running',        -- 更新为运行中
    start_time = ?,            -- 记录开始时间
    retries = retries + 1      -- 增加重试计数
WHERE
    id IN (
        SELECT id FROM (
            SELECT id
            FROM scheduled_jobs
            WHERE
                status = 'Waiting'              -- 只拉取等待中的任务
                AND next_run_at <= ?            -- 到期的任务
                AND retries < ?                 -- 重试次数未超限
            ORDER BY next_run_at ASC            -- 按到期时间排序
            LIMIT ?                             -- 限制拉取数量
            FOR UPDATE SKIP LOCKED              -- 跳过已锁定的行 (PostgreSQL/MySQL 8+)
        ) AS subquery
    );
```

**关键点**:

1. **`FOR UPDATE SKIP LOCKED`**: 跳过已被其他事务锁定的行,确保不同节点不会拉取相同的任务
2. **`status = 'Waiting'`**: 只拉取等待中的任务
3. **`UPDATE` 原子操作**: 在同一个事务中更新状态,确保原子性
4. **`LIMIT ?`**: 根据可用 Worker 数量限制拉取任务数

#### 1.2 Keep-Alive 机制

**代码位置**: [src/service/alerts/scheduler/worker.rs:258-293](src/service/alerts/scheduler/worker.rs#L258-L293)

```rust
tokio::task::spawn(async move {
    let start_time = tokio::time::Instant::now();
    let max_lifetime = tokio::time::Duration::from_secs(max_lifetime_secs as u64);

    loop {
        tokio::select! {
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(ttl)) => {}
            _ = rx.recv() => {
                // 任务完成,停止 keep-alive
                return;
            }
        }

        // 检查最大生命周期
        if start_time.elapsed() >= max_lifetime {
            log::warn!(
                "[SCHEDULER][JobPuller-{trace_id_keep_alive}] keep_alive for job[{job_id}] exceeded maximum lifetime, stopping"
            );
            return;
        }

        // 定期更新任务的 start_time (证明任务还活着)
        if let Err(e) = infra::scheduler::keep_alive(
            &[job_id],
            alert_timeout,
            report_timeout
        ).await {
            log::error!(
                "[SCHEDULER][JobPuller-{trace_id_keep_alive}] keep_alive failed: {e}"
            );
        }
    }
});
```

**目的**: 防止任务执行节点崩溃后,任务永远卡在 "Running" 状态。

**机制**:
1. 每 `ttl` 秒更新一次任务的 `start_time`
2. 如果节点崩溃,`start_time` 不再更新
3. 超时监控器检测到 `start_time` 过旧,将任务重置为 "Waiting"

#### 1.3 超时监控

**代码位置**: [src/job/alert_manager.rs:66-71](src/job/alert_manager.rs#L66-L71)

```rust
spawn_pausable_job!(
    "alert_manager_watch_timeout",
    get_config().limit.scheduler_watch_interval,
    {
        if let Err(e) = infra::scheduler::watch_timeout().await {
            log::error!("[SCHEDULER] watch timeout jobs error: {e}");
        }
    }
);
```

**SQL 实现**:

```sql
-- 查找超时的任务
SELECT id, org, module, module_key
FROM scheduled_jobs
WHERE
    status = 'Running'                               -- 运行中的任务
    AND start_time < (NOW() - INTERVAL '? seconds')  -- 开始时间过旧

-- 重置为等待状态
UPDATE scheduled_jobs
SET
    status = 'Waiting',
    start_time = NULL
WHERE id IN (超时任务的 ID);
```

**超时时间**:
- Alert: `alert_schedule_timeout` (默认 300 秒)
- Report: `report_schedule_timeout` (默认 3600 秒)

### 2. Compactor 任务分配

#### 文件列表锁机制

**代码位置**: src/infra/src/file_list/mysql.rs (推测)

```rust
// 1. 生成压缩任务
pub async fn add_job(org: &str, stream_type: &str, stream: &str, file: &str) -> Result<()> {
    let sql = r#"
    INSERT INTO file_list_jobs (org, stream_type, stream, file, status, created_at)
    VALUES (?, ?, ?, ?, 'Pending', NOW())
    ON DUPLICATE KEY UPDATE status = 'Pending';
    "#;
    // ...
}

// 2. 拉取任务 (原子操作)
pub async fn pull_jobs(limit: usize) -> Result<Vec<CompactionJob>> {
    let sql = r#"
    UPDATE file_list_jobs
    SET
        status = 'Running',
        node_id = ?,           -- 记录执行节点
        start_time = NOW()
    WHERE id IN (
        SELECT id FROM (
            SELECT id
            FROM file_list_jobs
            WHERE status = 'Pending'
            ORDER BY created_at ASC
            LIMIT ?
            FOR UPDATE SKIP LOCKED
        ) AS subquery
    )
    RETURNING *;
    "#;
    // ...
}
```

**关键点**:
- 每个文件的压缩任务**只会被一个 Compactor 节点拉取**
- 使用 `FOR UPDATE SKIP LOCKED` 避免锁等待
- `node_id` 记录执行节点,便于故障排查

### 3. Ingester 数据分片

#### 数据分片机制

虽然多个 Ingester 节点可以同时写入,但**不会重复处理**同一条数据:

```
客户端请求
    │
    ▼
┌─────────────────────────────────┐
│  Router/Load Balancer           │
│  (根据某种规则路由到 Ingester)    │
└─────────────────────────────────┘
    │         │         │
    ▼         ▼         ▼
┌────────┐ ┌────────┐ ┌────────┐
│Ingester│ │Ingester│ │Ingester│
│  节点1  │ │  节点2  │ │  节点3  │
└────────┘ └────────┘ └────────┘
```

**路由策略**:

1. **轮询 (Round Robin)**: 请求均匀分布到所有 Ingester
2. **哈希 (Hash)**: 根据 `org_id` 或 `stream_name` 哈希到固定节点
3. **一致性哈希**: 节点增减时减少数据迁移

**重要**: 每条数据只会被发送到**一个** Ingester 节点,因此**不会重复处理**。

### 4. 元数据同步机制

#### 集群协调器 (Cluster Coordinator)

**代码位置**: src/infra/src/db/mod.rs

OpenObserve 使用分布式协调器同步元数据更新:

```rust
pub async fn get_coordinator() -> Arc<dyn Coordinator> {
    match config::get_config().common.cluster_coordinator.as_str() {
        "nats" => Arc::new(NatsCoordinator::new()),
        "etcd" => Arc::new(EtcdCoordinator::new()),
        _ => panic!("Unknown cluster coordinator"),
    }
}
```

**机制**:

```
节点 1 修改告警规则
    │
    ▼
┌─────────────────────────────────┐
│  1. 更新 MySQL 数据库            │
└─────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────┐
│  2. 发送 PUT 事件到 NATS/Etcd    │
│     Key: /alerts/{org}/{key}    │
└─────────────────────────────────┘
    │
    ├──────────────┬───────────────┐
    │              │               │
    ▼              ▼               ▼
┌────────┐    ┌────────┐      ┌────────┐
│ 节点 1  │    │ 节点 2  │      │ 节点 3  │
│ Watch  │    │ Watch  │      │ Watch  │
│ 事件   │    │ 事件   │      │ 事件   │
└────────┘    └────────┘      └────────┘
    │              │               │
    ▼              ▼               ▼
重新加载缓存    重新加载缓存    重新加载缓存
```

**示例** (代码位置: [src/infra/src/scheduler/mysql.rs:186-196](src/infra/src/scheduler/mysql.rs#L186-L196)):

```rust
// 推送实时告警触发器
if trigger.module == TriggerModule::Alert && trigger.is_realtime {
    let key = format!(
        "{TRIGGERS_KEY}{}/{}/{}",
        trigger.module, &trigger.org, &trigger.module_key
    );
    let cluster_coordinator = db::get_coordinator().await;
    cluster_coordinator
        .put(&key, Bytes::from(""), true, None)  // 发送 PUT 事件
        .await?;
}
```

**结果**: 所有节点的缓存**最终一致**,无需担心数据不同步。

---

## 节点角色配置

### 1. 单节点模式 (All-in-One)

**适用场景**: 开发、测试、小规模部署

**配置**:

```bash
ZO_NODE_ROLE=all
```

**行为**:
- 单个进程运行所有组件
- Ingester + Querier + Compactor + AlertManager
- 无需集群协调器
- 性能受限于单机资源

**优点**:
- 部署简单
- 无需外部依赖 (NATS/Etcd)
- 适合快速开发

**缺点**:
- 无法水平扩展
- 单点故障
- 资源争用 (查询和压缩抢 CPU)

### 2. 多节点集群模式

#### 2.1 推荐架构 (生产环境)

```
┌─────────────────────────────────────────────────────────┐
│                    Load Balancer                        │
│                  (Nginx/HAProxy)                        │
└─────────────────────────────────────────────────────────┘
            │                    │
            ▼                    ▼
┌──────────────────┐    ┌──────────────────┐
│  Router Node 1   │    │  Router Node 2   │
│  角色: router     │    │  角色: router     │
│  功能: 路由转发   │    │  功能: 路由转发   │
└──────────────────┘    └──────────────────┘
            │                    │
    ┌───────┼────────┬───────────┼────────┐
    │       │        │           │        │
    ▼       ▼        ▼           ▼        ▼
┌────────┐┌────────┐┌────────┐┌────────┐┌────────┐
│Ingester││Ingester││Querier ││Querier ││Querier │
│ Node 1 ││ Node 2 ││ Node 1 ││ Node 2 ││ Node 3 │
│        ││        ││        ││        ││        │
│写入数据││写入数据││查询数据││查询数据││查询数据│
└────────┘└────────┘└────────┘└────────┘└────────┘

┌────────────────┐  ┌────────────────┐  ┌────────────────┐
│ Compactor      │  │ AlertManager   │  │ FlattenComp    │
│ Node 1         │  │ Node 1         │  │ Node 1         │
│                │  │                │  │                │
│ 数据压缩       │  │ 告警调度       │  │ 扁平化压缩     │
└────────────────┘  └────────────────┘  └────────────────┘

┌─────────────────────────────────────────────────────────┐
│              共享基础设施                                 │
│                                                         │
│  - MySQL/PostgreSQL (元数据)                            │
│  - S3/MinIO (对象存储)                                  │
│  - NATS/Etcd (集群协调)                                 │
└─────────────────────────────────────────────────────────┘
```

#### 2.2 节点配置示例

**Router 节点**:

```bash
# 环境变量
ZO_NODE_ROLE=router
ZO_INSTANCE_NAME=router-1
ZO_HTTP_PORT=5080
ZO_GRPC_PORT=5081
```

**Ingester 节点**:

```bash
ZO_NODE_ROLE=ingester
ZO_INSTANCE_NAME=ingester-1
ZO_HTTP_PORT=5080
ZO_GRPC_PORT=5081
ZO_DATA_WAL_DIR=/data/wal
```

**Querier 节点**:

```bash
ZO_NODE_ROLE=querier
ZO_INSTANCE_NAME=querier-1
ZO_HTTP_PORT=5080
ZO_GRPC_PORT=5081
ZO_LIMIT_CPU_NUM=16              # 分配更多 CPU
```

**Querier 节点 (Interactive)**:

```bash
ZO_NODE_ROLE=querier
ZO_NODE_ROLE_GROUP=interactive    # 高优先级,处理用户查询
ZO_INSTANCE_NAME=querier-interactive-1
```

**Querier 节点 (Background)**:

```bash
ZO_NODE_ROLE=querier
ZO_NODE_ROLE_GROUP=background     # 低优先级,处理定时报表
ZO_INSTANCE_NAME=querier-background-1
```

**Compactor 节点**:

```bash
ZO_NODE_ROLE=compactor
ZO_INSTANCE_NAME=compactor-1
ZO_COMPACT_ENABLED=true
ZO_LIMIT_FILE_MERGE_THREAD_NUM=8
```

**AlertManager 节点**:

```bash
ZO_NODE_ROLE=alert_manager
ZO_INSTANCE_NAME=alert-manager-1
ZO_LIMIT_ALERT_SCHEDULE_CONCURRENCY=10
```

#### 2.3 多角色节点 (组合角色)

**示例 1: Ingester + Querier**:

```bash
ZO_NODE_ROLE=ingester,querier     # 逗号分隔
ZO_INSTANCE_NAME=hybrid-1
```

**用途**: 小规模集群,节省节点数量

**示例 2: Querier + AlertManager**:

```bash
ZO_NODE_ROLE=querier,alert_manager
ZO_INSTANCE_NAME=hybrid-query-alert-1
```

**用途**: AlertManager 通常负载较低,可与 Querier 共享节点

### 3. 集群协调器配置

**NATS (推荐)**:

```bash
# 每个节点配置
ZO_CLUSTER_COORDINATOR=nats
ZO_NATS_ADDR=nats://nats-server:4222
```

**Etcd**:

```bash
ZO_CLUSTER_COORDINATOR=etcd
ZO_ETCD_ADDR=http://etcd-server:2379
```

### 4. 数据库配置

**MySQL**:

```bash
ZO_META_STORE=mysql
ZO_META_MYSQL_DSN=mysql://user:password@mysql-server:3306/openobserve
```

**PostgreSQL**:

```bash
ZO_META_STORE=postgres
ZO_META_POSTGRES_DSN=postgres://user:password@postgres-server:5432/openobserve
```

**SQLite** (仅单节点):

```bash
ZO_META_STORE=sqlite
# 文件位置: ${ZO_DATA_DIR}/metadata/metadata.db
```

---

## 重复处理防护总结表

| 组件 | 任务类型 | 防护机制 | 实现方式 |
|-----|---------|---------|---------|
| **Ingester** | 数据写入 | 路由分片 | 每条数据只发送到一个 Ingester |
| **Compactor** | 文件压缩 | 数据库锁 | `FOR UPDATE SKIP LOCKED` + 状态机 |
| **AlertManager** | 告警调度 | 数据库锁 | `FOR UPDATE SKIP LOCKED` + Keep-Alive |
| **AlertManager** | 定时报表 | 数据库锁 | `FOR UPDATE SKIP LOCKED` + Keep-Alive |
| **所有组件** | 元数据同步 | 分布式协调器 | NATS/Etcd Pub/Sub + Watch |
| **所有组件** | 后台任务 | 节点角色判断 | `if LOCAL_NODE.is_xxx()` |

---

## 最佳实践

### 1. 节点角色规划

#### 1.1 小规模集群 (< 100GB/天)

**推荐配置**:

```
2 x All-in-One 节点 (高可用)
```

**配置**:

```bash
# 节点 1 和节点 2 相同配置
ZO_NODE_ROLE=all
ZO_INSTANCE_NAME=node-1  # 或 node-2
```

**优点**:
- 简单易维护
- 自动高可用
- 成本低

**缺点**:
- 资源争用
- 扩展性有限

#### 1.2 中规模集群 (100GB-1TB/天)

**推荐配置**:

```
2 x Router (负载均衡)
3 x Ingester (写入)
3 x Querier (查询)
2 x Compactor (压缩)
1 x AlertManager (告警)
```

**总节点**: 11 个

#### 1.3 大规模集群 (> 1TB/天)

**推荐配置**:

```
3 x Router (负载均衡)
5-10 x Ingester (写入)
5-10 x Querier-Interactive (交互查询)
2-3 x Querier-Background (后台报表)
3-5 x Compactor (压缩)
2 x AlertManager (告警,高可用)
1 x FlattenCompactor (扁平化压缩)
```

**总节点**: 21-34 个

### 2. 资源分配建议

| 角色 | CPU | 内存 | 磁盘 | 网络 |
|-----|-----|------|------|------|
| **Router** | 2-4 核 | 4-8 GB | 50 GB | 高带宽 |
| **Ingester** | 8-16 核 | 16-32 GB | 500 GB-2 TB (WAL) | 高带宽 |
| **Querier** | 16-32 核 | 32-64 GB | 200 GB (缓存) | 高带宽 |
| **Compactor** | 8-16 核 | 16-32 GB | 100 GB | 中等带宽 |
| **AlertManager** | 4-8 核 | 8-16 GB | 50 GB | 低带宽 |

### 3. 高可用配置

#### 3.1 Router 高可用

```
┌─────────────────────────────────┐
│      Load Balancer (VIP)        │
│      (Keepalived/HAProxy)       │
└─────────────────────────────────┘
            │        │
            ▼        ▼
    ┌──────────┐ ┌──────────┐
    │ Router 1 │ │ Router 2 │
    └──────────┘ └──────────┘
```

**配置**:
- HAProxy 健康检查: `GET /healthz`
- 故障转移时间: < 5 秒

#### 3.2 Ingester 高可用

**策略**: 无需特殊配置,Router 自动负载均衡

**故障处理**:
1. Ingester 节点故障
2. Router 检测到 HTTP 错误或超时
3. Router 自动重试到其他 Ingester
4. WAL 重放机制恢复未持久化数据

#### 3.3 Querier 高可用

**策略**: 查询协调器自动分配任务到多个 Querier

**故障处理**:
1. Querier 节点故障
2. 查询协调器检测超时
3. 重新分配任务到其他 Querier

#### 3.4 Compactor 高可用

**策略**: 数据库调度器自动容错

**故障处理**:
1. Compactor 节点故障
2. 超时监控器检测任务超时
3. 重置任务为 "Waiting"
4. 其他 Compactor 节点自动拉取

#### 3.5 AlertManager 高可用

**部署**: 2 个 AlertManager 节点

**故障处理**:
1. AlertManager 1 故障
2. AlertManager 2 继续调度
3. 超时监控器重置被 AlertManager 1 锁定的任务

**注意**: 告警可能有轻微延迟 (< 超时时间),但**不会丢失**。

### 4. 监控指标

#### 4.1 节点健康指标

```promql
# 节点在线状态
openobserve_cluster_node_status{role="ingester"}

# 节点 CPU 使用率
openobserve_node_cpu_usage{instance="ingester-1"}

# 节点内存使用率
openobserve_node_memory_usage{instance="ingester-1"}
```

#### 4.2 任务调度指标

```promql
# 待处理任务数
openobserve_scheduler_pending_jobs{module="alert"}

# 运行中任务数
openobserve_scheduler_running_jobs{module="alert"}

# 任务执行延迟
histogram_quantile(0.95,
  rate(openobserve_scheduler_job_duration_seconds_bucket[5m])
)

# 任务失败率
rate(openobserve_scheduler_job_failures_total[5m]) /
rate(openobserve_scheduler_job_total[5m])
```

#### 4.3 Compactor 指标

```promql
# 待压缩文件数
openobserve_compact_pending_files{org="default"}

# 压缩吞吐量
rate(openobserve_compact_files_total[5m])

# 压缩延迟
openobserve_compact_lag_seconds{org="default"}
```

### 5. 故障排查

#### 5.1 任务重复执行

**症状**: 相同的告警发送多次

**原因**:
1. 超时时间设置过短
2. 任务执行时间过长
3. 网络延迟

**解决**:

```bash
# 增加超时时间
ZO_LIMIT_ALERT_SCHEDULE_TIMEOUT=600  # 10 分钟

# 增加 Keep-Alive 频率
# (自动计算,无需手动配置)
```

#### 5.2 任务积压

**症状**: `openobserve_scheduler_pending_jobs` 持续增长

**原因**:
1. AlertManager 节点不足
2. 并发度不足
3. 任务执行时间过长

**解决**:

```bash
# 增加并发度
ZO_LIMIT_ALERT_SCHEDULE_CONCURRENCY=20  # 默认 10

# 增加 AlertManager 节点
ZO_NODE_ROLE=alert_manager
```

#### 5.3 数据丢失

**症状**: 写入的数据查询不到

**排查**:

```bash
# 1. 检查 Ingester 节点状态
curl http://ingester-1:5080/healthz

# 2. 检查 WAL 目录
ls -lh /data/wal/logs/

# 3. 检查 MemTable 指标
curl http://ingester-1:5080/metrics | grep memtable

# 4. 检查 Parquet 文件
curl http://ingester-1:5080/metrics | grep parquet
```

**常见原因**:
1. 熔断器触发 (磁盘满、内存满)
2. WAL 写入失败
3. Parquet 持久化失败

---

## 总结

### 核心回答

**问题 1**: 每个节点上运行的组件功能是否一致?

**回答**: ❌ **不一致**

- 每个节点根据 `ZO_NODE_ROLE` 配置运行**不同的组件**
- `role = "all"`: 运行所有组件 (单机模式)
- `role = "ingester"`: 仅运行 Ingester 组件
- `role = "querier"`: 仅运行 Querier 组件
- `role = "compactor"`: 仅运行 Compactor 组件
- `role = "alert_manager"`: 仅运行 AlertManager 组件

**问题 2**: 会不会有重复处理数据的情况?

**回答**: ✅ **不会重复处理**

**防护机制**:

1. **数据写入**: 每条数据只路由到一个 Ingester 节点
2. **文件压缩**: 使用数据库 `FOR UPDATE SKIP LOCKED` 锁机制
3. **告警调度**: 使用数据库原子操作 + Keep-Alive 机制
4. **元数据同步**: 使用 NATS/Etcd 分布式协调器
5. **组件启动**: 根据节点角色判断是否启动组件

### 关键设计

1. **角色分离**: 不同节点运行不同组件,避免资源争用
2. **分布式锁**: 使用数据库事务保证任务唯一性
3. **超时容错**: Keep-Alive + 超时监控,自动恢复故障任务
4. **最终一致**: 元数据通过分布式协调器同步,保证一致性

### 推荐配置

| 场景 | 节点配置 | 说明 |
|-----|---------|-----|
| **开发/测试** | 1 x All | 单机模式,快速部署 |
| **小规模生产** | 2 x All | 高可用,简单维护 |
| **中规模生产** | Router + Ingester + Querier + Compactor + AlertManager | 角色分离,性能优化 |
| **大规模生产** | 多节点 + 角色分组 + 资源隔离 | 高性能,高可用 |

---

**文档版本**: 1.0
**最后更新**: 2025-01-15
**适用版本**: OpenObserve v0.17.0+
