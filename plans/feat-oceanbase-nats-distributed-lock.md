# feat: OceanBase MetaStore 使用 NATS 分布式锁替代 GET_LOCK

## 概述

为 OceanBase MetaStore 实现使用 NATS 分布式锁替代 MySQL GET_LOCK 函数。这是因为 OceanBase 低版本（V4.2.0 之前）不支持 GET_LOCK 函数，而 GET_LOCK 是 OpenObserve 中实现分布式锁的关键机制。

**类型**: enhancement
**模块**: infra/db, infra/file_list, infra/scheduler
**影响范围**: OceanBase 用户，特别是使用低版本的用户
**状态**: 计划已通过深度研究增强

---

## 问题陈述

### 现状

当前 OpenObserve 使用 MySQL 的 `GET_LOCK` 函数在多个关键位置实现分布式锁：

| 位置 | 文件 | 行号 | 锁用途 |
|------|------|------|--------|
| MetaStore | `src/infra/src/db/mysql.rs` | 240 | 元数据的原子读取-修改-写入 |
| FileList | `src/infra/src/file_list/mysql.rs` | 729 | 查询已删除文件列表 |
| FileList | `src/infra/src/file_list/mysql.rs` | 1305 | 获取待处理合并作业 |
| FileList | `src/infra/src/file_list/mysql.rs` | 1629 | 获取待处理转储作业 |
| Scheduler | `src/infra/src/scheduler/mysql.rs` | 548 | 拉取调度任务 |

### 问题

1. **OceanBase 兼容性**: OceanBase V4.2.0 之前的版本不支持 `GET_LOCK` 函数
2. **架构耦合**: OceanBase 当前完全复用 MySQL 实现（`mysql.rs`），无法针对性处理
3. **部署限制**: 低版本 OceanBase 用户无法正常使用 OpenObserve 的集群功能

### 参考

- OceanBase GET_LOCK 支持: [V4.2.0+ 文档](https://en.oceanbase.com/docs/common-oceanbase-database-10000000001379158)
- 现有 NATS Locker 实现: `src/infra/src/db/nats.rs:646-745`

---

## 锁机制语义差异分析

### GET_LOCK vs NATS Lock 对比

| 特性 | MySQL GET_LOCK | NATS 分布式锁 |
|------|----------------|---------------|
| **锁绑定** | 数据库 Session（TCP 连接） | TTL + Keep-alive |
| **崩溃释放** | 连接断开立即释放 | 等待 TTL 过期 |
| **重入性** | 同一 Session 可重入 | 默认不可重入 |
| **跨节点** | 单实例有效 | 原生分布式 |

### 关键风险：不可用窗口期

当应用崩溃时：
- **GET_LOCK**: 数据库连接断开，锁**立即释放**
- **NATS Lock**: 锁必须等待 **TTL 过期**才能被其他节点获取

**缓解措施**: Keep-alive 协程必须与持有锁的任务生命周期绑定

---

## 提议的解决方案

### 高层设计

当 MetaStore 配置为 OceanBase 时，使用已有的 NATS 分布式锁（`dist_lock` 模块）替代 MySQL `GET_LOCK`。

```
┌─────────────────────────────────────────────────────────────┐
│                      Application                             │
└──────────────────────────┬──────────────────────────────────┘
                           │
           ┌───────────────┼───────────────┐
           ▼               ▼               ▼
   ┌─────────────┐  ┌─────────────┐  ┌─────────────┐
   │ OceanBaseDb │  │ OceanBase   │  │ OceanBase   │
   │             │  │ FileList    │  │ Scheduler   │
   └──────┬──────┘  └──────┬──────┘  └──────┬──────┘
          │                │                │
          │   delegate!    │   delegate!    │   delegate!
          ▼                ▼                ▼
   ┌─────────────┐  ┌─────────────┐  ┌─────────────┐
   │  MysqlDb    │  │ MysqlFile   │  │ MysqlSched  │
   │  (无锁方法)  │  │ List        │  │ uler        │
   └─────────────┘  └─────────────┘  └─────────────┘
          │
          └────────────────┬────────────────┘
                           ▼
              ┌────────────────────────┐
              │   NatsLockGuard        │
              │   (带中断机制)          │
              └────────────────────────┘
```

### 核心设计决策

| 决策点 | 选择 | 理由 |
|--------|------|------|
| 锁策略选择 | 基于 MetaStore 类型自动选择 | OceanBase 用 NATS，MySQL 用 GET_LOCK |
| 代码组织 | 创建独立 OceanBase 实现 | 隔离变更，便于维护，支持未来 OceanBase 特定优化 |
| 样板代码 | 使用 `delegate` crate | 减少手动委托代码，只重写锁相关方法 |
| NATS 依赖 | OceanBase 用户必须配置 NATS | 保证分布式锁可用 |
| 向后兼容 | MySQL 用户行为不变 | 无破坏性更改 |

---

## 技术方案

### 依赖添加

在 `Cargo.toml` 中添加：

```toml
[dependencies]
delegate = "0.12"  # 简化委托模式样板代码
```

### 架构变更

#### Phase 1: 创建带中断机制的锁 Guard

**文件**: `src/infra/src/db/nats_lock_guard.rs` (新增)

```rust
use tokio::sync::mpsc;
use std::future::Future;

/// 带中断机制的 NATS 锁 Guard
/// 当 Keep-alive 失败时，能够中断正在进行的操作
pub struct NatsLockGuard {
    locker: Option<dist_lock::Locker>,
    cancel_tx: mpsc::Sender<()>,
}

impl NatsLockGuard {
    /// 获取锁并执行操作，支持 Keep-alive 失败时中断
    pub async fn with_lock<F, T>(
        key: &str,
        timeout: u64,
        operation: F,
    ) -> Result<T>
    where
        F: Future<Output = Result<T>>,
    {
        let (cancel_tx, mut cancel_rx) = mpsc::channel(1);

        // 获取 NATS 锁
        let locker = dist_lock::lock(key, timeout).await?;

        // 使用 select 监听 Keep-alive 失败
        tokio::select! {
            result = operation => {
                // 操作完成，释放锁
                dist_lock::unlock(&locker).await?;
                result
            }
            _ = cancel_rx.recv() => {
                // Keep-alive 失败，中断操作
                dist_lock::unlock(&locker).await.ok(); // 尽力释放
                Err(Error::LockLost("NATS lock lost during operation".into()))
            }
        }
    }
}
```

#### Phase 2: 创建 OceanBaseDb 实现

**文件**: `src/infra/src/db/oceanbase.rs` (新增)

```rust
use delegate::delegate;
use super::mysql::MysqlDb;
use super::nats_lock_guard::NatsLockGuard;

pub struct OceanBaseDb {
    mysql_db: MysqlDb,
}

impl OceanBaseDb {
    pub fn new() -> Self {
        Self {
            mysql_db: MysqlDb::default(),
        }
    }
}

impl Db for OceanBaseDb {
    // 使用 delegate 宏自动委托非锁相关方法
    delegate! {
        to self.mysql_db {
            async fn get(&self, key: &str) -> Result<Bytes>;
            async fn put(&self, key: &str, value: Bytes, need_watch: bool, start_dt: Option<i64>) -> Result<()>;
            async fn delete(&self, key: &str, with_prefix: bool, need_watch: bool, start_dt: Option<i64>) -> Result<()>;
            async fn list(&self, prefix: &str) -> Result<HashMap<String, Bytes>>;
            async fn list_keys(&self, prefix: &str) -> Result<Vec<String>>;
            async fn list_values(&self, prefix: &str) -> Result<Vec<Bytes>>;
            async fn list_values_by_start_dt(&self, prefix: &str, start_dt: Option<(i64, i64)>) -> Result<Vec<(i64, Bytes)>>;
            async fn count(&self, prefix: &str) -> Result<i64>;
            async fn watch(&self, prefix: &str) -> Result<Arc<mpsc::Receiver<Event>>>;
            async fn close(&self) -> Result<()>;
            async fn add_start_dt_column(&self) -> Result<()>;
        }
    }

    /// 使用 NATS 分布式锁替代 GET_LOCK
    async fn get_for_update(
        &self,
        key: &str,
        need_watch: bool,
        start_dt: Option<i64>,
        update_fn: Box<super::UpdateFn>,
    ) -> Result<()> {
        let lock_key = format!("/lock/meta/{}", key);
        let timeout = config::get_config().limit.meta_transaction_lock_timeout as u64;

        NatsLockGuard::with_lock(&lock_key, timeout, async {
            self.mysql_db.get_for_update_inner(key, need_watch, start_dt, update_fn).await
        }).await
    }
}
```

**修改**: `src/infra/src/db/mysql.rs`

```rust
impl MysqlDb {
    /// 提取无锁版本的 get_for_update 核心逻辑
    /// 供 OceanBaseDb 调用
    pub(crate) async fn get_for_update_inner(
        &self,
        key: &str,
        need_watch: bool,
        start_dt: Option<i64>,
        update_fn: Box<super::UpdateFn>,
    ) -> Result<()> {
        // 原有的数据库操作逻辑，不包含 GET_LOCK/RELEASE_LOCK
        // ...
    }
}
```

#### Phase 3: FileList 和 Scheduler 模块适配

**文件**: `src/infra/src/file_list/oceanbase.rs` (新增)

```rust
use delegate::delegate;

pub struct OceanBaseFileList {
    mysql_impl: MysqlFileList,
}

impl FileList for OceanBaseFileList {
    delegate! {
        to self.mysql_impl {
            // 委托所有非锁相关方法
            async fn add(&self, ...) -> Result<()>;
            async fn remove(&self, ...) -> Result<()>;
            // ... 其他方法
        }
    }

    async fn query_deleted(&self, org_id: &str, ...) -> Result<Vec<...>> {
        let lock_key = "/lock/file_list/deleted";
        NatsLockGuard::with_lock(lock_key, 0, async {
            self.mysql_impl.query_deleted_inner(org_id, ...).await
        }).await
    }

    async fn get_pending_jobs(&self, ...) -> Result<Vec<...>> {
        let lock_key = "/lock/file_list/pending_jobs";
        NatsLockGuard::with_lock(lock_key, 0, async {
            self.mysql_impl.get_pending_jobs_inner(...).await
        }).await
    }

    async fn get_pending_dump_jobs(&self, ...) -> Result<Vec<...>> {
        let lock_key = "/lock/file_list/pending_dump_jobs";
        NatsLockGuard::with_lock(lock_key, 0, async {
            self.mysql_impl.get_pending_dump_jobs_inner(...).await
        }).await
    }
}
```

**文件**: `src/infra/src/scheduler/oceanbase.rs` (新增)

```rust
use delegate::delegate;

pub struct OceanBaseScheduler {
    mysql_impl: MysqlScheduler,
}

impl Scheduler for OceanBaseScheduler {
    delegate! {
        to self.mysql_impl {
            // 委托所有非锁相关方法
        }
    }

    async fn pull(&self, ...) -> Result<Vec<...>> {
        let lock_key = "/lock/scheduler/pull";
        let timeout = config::get_config().limit.scheduler_lock_timeout as u64;
        NatsLockGuard::with_lock(lock_key, timeout, async {
            self.mysql_impl.pull_inner(...).await
        }).await
    }
}
```

### 锁 Key 命名规范

统一使用 `/lock/{module}/{operation}` 格式：

| 模块 | 锁 Key |
|------|--------|
| MetaStore | `/lock/meta/{key}` |
| FileList deleted | `/lock/file_list/deleted` |
| FileList jobs | `/lock/file_list/pending_jobs` |
| FileList dump | `/lock/file_list/pending_dump_jobs` |
| Scheduler | `/lock/scheduler/pull` |

### 配置建议

建议为不同模块配置不同的 TTL：

| 模块 | 建议 TTL | 理由 |
|------|----------|------|
| MetaStore | 60s | 短操作，快速恢复 |
| FileList | 300s | 批量操作，需要较长时间 |
| Scheduler | 120s | 中等操作 |

可通过环境变量配置：

```bash
ZO_NATS_LOCK_TTL_META=60
ZO_NATS_LOCK_TTL_FILE_LIST=300
ZO_NATS_LOCK_TTL_SCHEDULER=120
```

---

## 分布式锁关键风险与缓解

### 风险 1: 脑裂与僵尸进程（CRITICAL）

**场景**:
```
1. Client A 获取 NATS 锁
2. Client A 发生 Full GC 或进程假死，Keep-alive 停止
3. NATS 锁超时失效
4. Client B 获取锁，开始写入数据库
5. Client A 苏醒，认为自己还持有锁，继续写入
→ 数据损坏！
```

**缓解措施**:

1. **Keep-alive 与任务绑定**: 使用 `tokio::select!` 确保 Keep-alive 失败时中断操作
2. **版本号校验 (Fencing Token)**: 在关键数据变更中引入版本号

```rust
// 示例：带版本号的更新
async fn update_with_fencing(key: &str, update_fn: impl FnOnce(&mut Value)) -> Result<()> {
    let lock = NatsLockGuard::acquire(key).await?;
    let fencing_token = lock.revision();  // NATS KV revision 作为 fencing token

    // 在数据库操作中验证 fencing token
    let affected = sqlx::query!(
        "UPDATE meta SET value = ?, fencing_token = ?
         WHERE key = ? AND fencing_token < ?",
        new_value, fencing_token, key, fencing_token
    ).execute(&pool).await?;

    if affected.rows_affected() == 0 {
        return Err(Error::StaleWrite);
    }
    Ok(())
}
```

**接受的风险**: 对于非关键操作，接受极端 GC 暂停下的数据竞争风险

### 风险 2: 锁的重入性（HIGH）

**问题**: MySQL `GET_LOCK` 在同一 Session 内可重入，NATS `create` 不支持。

**检查点**: 需确认以下调用链是否存在嵌套加锁：
- [ ] `Scheduler::pull` → 是否调用 `MetaStore::get_for_update`?
- [ ] `FileList::query_deleted` → 是否嵌套调用其他锁方法?

**解决方案** (如需要):

```rust
use std::cell::RefCell;
use std::collections::HashSet;

thread_local! {
    static HELD_LOCKS: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
}

pub async fn acquire_reentrant_lock(key: &str, timeout: u64) -> Result<ReentrantLockGuard> {
    let already_held = HELD_LOCKS.with(|locks| locks.borrow().contains(key));

    if already_held {
        // 已持有此锁，返回空 guard（重入）
        return Ok(ReentrantLockGuard::Reentrant(key.to_string()));
    }

    // 实际获取锁
    let locker = dist_lock::lock(key, timeout).await?;
    HELD_LOCKS.with(|locks| locks.borrow_mut().insert(key.to_string()));
    Ok(ReentrantLockGuard::Real(locker, key.to_string()))
}
```

### 风险 3: 原子性破坏（MEDIUM）

**场景**: `DB Commit` 成功后、`NATS Unlock` 前崩溃

**后果**: 数据已更新，锁未释放，阻塞至 TTL 过期

**缓解措施**:
1. 合理设置 TTL（不要太长）
2. 这是**已知限制**，接受恢复延迟换取一致性

### 风险 4: NATS 抖动时的事务中断（HIGH）

**问题**: Keep-alive 失败时，正在进行的 DB 事务应立即终止

**解决方案**: 通过 `NatsLockGuard::with_lock` 中的 `tokio::select!` 实现（见技术方案）

---

## 实现阶段

### Phase 1: 基础设施 (Foundation)

**任务**:
- [ ] 添加 `delegate` crate 依赖
- [ ] 创建 `src/infra/src/db/nats_lock_guard.rs`
- [ ] 实现带中断机制的 `NatsLockGuard`
- [ ] 添加 `LockLost` 错误类型到 `DbError`

**验收标准**:
- `NatsLockGuard` 单元测试通过
- Keep-alive 失败时能正确中断操作

### Phase 2: OceanBaseDb 实现 (Core)

**任务**:
- [ ] 创建 `src/infra/src/db/oceanbase.rs`
- [ ] 使用 `delegate!` 宏委托非锁方法
- [ ] 实现 `get_for_update` 使用 `NatsLockGuard`
- [ ] 修改 `src/infra/src/db/mod.rs` 路由
- [ ] 在 `MysqlDb` 中提取 `get_for_update_inner`

**验收标准**:
- OceanBase MetaStore 可以启动
- `get_for_update` 使用 NATS 锁
- MySQL MetaStore 行为不变

### Phase 3: FileList 和 Scheduler 适配

**任务**:
- [ ] 创建 `src/infra/src/file_list/oceanbase.rs`
- [ ] 创建 `src/infra/src/scheduler/oceanbase.rs`
- [ ] 使用 `delegate!` 宏减少样板代码
- [ ] 在 MySQL 实现中提取 `*_inner` 方法

**验收标准**:
- 5 个锁点全部替换为 NATS 锁
- 并发测试通过

### Phase 4: 测试与文档

**任务**:
- [ ] 添加单元测试（见测试计划）
- [ ] 添加集成测试（见测试计划）
- [ ] 确认重入性需求（检查嵌套调用）
- [ ] 更新部署文档说明 NATS 依赖
- [ ] 添加 TTL 配置说明

**验收标准**:
- 所有测试通过
- 文档完整

---

## 验收标准

### 功能要求

- [ ] OceanBase MetaStore 使用 NATS 分布式锁
- [ ] MySQL MetaStore 行为不变（向后兼容）
- [ ] 锁的获取和释放正确，无资源泄漏
- [ ] 锁超时后自动释放
- [ ] **Keep-alive 失败时能中断正在进行的操作**
- [ ] **支持锁的重入性（或确认无嵌套调用场景）**

### 非功能要求

- [ ] 性能: NATS 锁延迟 < 100ms（P99）
- [ ] 可靠性: NATS 不可用时返回明确错误 `LockServiceUnavailable`
- [ ] 可观测性: 复用现有 NATS Locker 日志
- [ ] **TTL 可按业务模块独立配置**

### 质量门槛

- [ ] 关键路径测试覆盖
- [ ] `cargo clippy` 无警告
- [ ] 文档更新完成

---

## 测试计划

### 单元测试

**文件**: `src/infra/src/db/nats_lock_guard.rs`

```rust
#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_lock_acquire_success() { ... }

    #[tokio::test]
    async fn test_lock_timeout() { ... }

    #[tokio::test]
    async fn test_lock_released_on_operation_complete() { ... }

    #[tokio::test]
    async fn test_operation_cancelled_on_lock_lost() { ... }
}
```

### 集成测试

**文件**: `tests/db_oceanbase_nats_lock_tests.rs`

| 测试 | 描述 |
|------|------|
| `test_concurrent_two_clients_nats_lock` | 两个客户端并发获取锁，只有一个成功 |
| `test_lock_timeout_with_nats` | NATS 锁超时行为 |
| `test_lock_released_on_error` | 操作失败后锁正确释放 |
| `test_get_for_update_with_nats_lock` | get_for_update 完整流程 |
| `test_lock_lost_during_operation` | Keep-alive 失败时操作被中断 |
| `test_reentrant_lock` | 同一线程嵌套获取锁（如需要） |
| `test_crash_recovery` | 模拟崩溃后锁 TTL 过期释放 |

### 边界情况测试

| 测试 | 描述 |
|------|------|
| `test_nats_reconnect` | NATS 短暂断连后恢复 |
| `test_long_gc_pause` | 模拟长时间 GC 暂停 |
| `test_mixed_mysql_oceanbase` | MySQL 和 OceanBase 实例共存 |

---

## 考虑过的替代方案

### 方案 A: 运行时检测 OceanBase 版本

**描述**: 启动时检测 OceanBase 版本，V4.2+ 使用 GET_LOCK，低版本使用 NATS 锁

**拒绝理由**:
- 增加运行时复杂度
- 版本检测可能不准确
- 混合锁机制难以调试

### 方案 B: 使用 SELECT FOR UPDATE 替代

**描述**: 使用数据库的行级锁替代命名锁

**拒绝理由**:
- `FOR UPDATE` 需要目标行存在
- 与 `GET_LOCK` 语义不完全等价
- 无法锁定任意名称

### 方案 C: 在 mysql.rs 中条件分支

**描述**: 在现有 mysql.rs 中根据 MetaStore 类型使用不同锁

**拒绝理由**:
- 虽然代码更简洁，但 OceanBase 可能有更多不兼容变更
- 隔离到独立文件便于未来针对性优化
- 使用 `delegate` crate 可减少样板代码

---

## 风险分析与缓解

| 风险 | 可能性 | 影响 | 缓解措施 |
|------|--------|------|----------|
| **脑裂/僵尸进程** | 低 | 高 | Keep-alive 绑定任务生命周期 + Fencing Token |
| **锁重入失败** | 中 | 中 | 检查嵌套调用，必要时实现重入支持 |
| **NATS 抖动** | 中 | 高 | `tokio::select!` 中断操作 |
| **TTL 过短** | 中 | 中 | 可配置 TTL，按模块调优 |
| NATS 成为单点故障 | 中 | 高 | 文档要求高可用 NATS 集群 |
| 滚动升级时锁不兼容 | 中 | 高 | 版本文档说明，建议全量升级 |

---

## 依赖与前置条件

### 必须依赖

- NATS 服务可用（`ZO_CLUSTER_COORDINATOR=nats`）
- async-nats 0.42.0+（当前版本满足）
- delegate crate 0.12+
- 现有 `dist_lock` 模块

### 前置条件

- 用户必须配置 NATS 作为 cluster coordinator
- NATS 集群应为高可用部署（生产环境）

### 实施前检查清单

- [ ] 确认 Scheduler/FileList 是否存在嵌套加锁调用
- [ ] 确认 `delegate` crate 与现有 Rust 版本兼容
- [ ] 确认 NATS Locker 的 Keep-alive 机制是否满足需求

---

## 参考资源

### 内部参考

- 现有 NATS Locker: `src/infra/src/db/nats.rs:646-745`
- 分布式锁 API: `src/infra/src/dist_lock.rs`
- MySQL GET_LOCK 实现: `src/infra/src/db/mysql.rs:228-471`
- MetaStore 配置: `src/config/src/meta/meta_store.rs`

### 外部参考

- [NATS JetStream Key-Value Store](https://docs.nats.io/nats-concepts/jetstream/key-value-store)
- [OceanBase MySQL 兼容性](https://en.oceanbase.com/docs/common-oceanbase-database-10000000001028992)
- [Martin Kleppmann: How to do distributed locking](https://martin.kleppmann.com/2016/02/08/how-to-do-distributed-locking.html)
- [async-nats 文档](https://docs.rs/async-nats)
- [delegate crate](https://crates.io/crates/delegate)

### 相关 PR/Issue

- MySQL/OceanBase 测试: feat/mysql-oceanbase-integration-tests 分支

---

## 审阅反馈整合

本计划已整合以下审阅意见：

1. **DHH 审阅**: 原方案过于复杂 → 使用 `delegate` crate 简化样板代码
2. **Kieran 审阅**: 缺少错误处理细节 → 增加 `LockLost` 错误和中断机制
3. **简洁性审阅**: 锁 Key 映射不必要 → 统一使用 `/lock/{module}/{operation}` 格式
4. **专家审阅**:
   - 增加脑裂风险分析和 Fencing Token 建议
   - 增加锁重入性检查点
   - 增加 NATS 抖动时的事务中断机制
   - 增加按模块配置 TTL 的建议

---

## 深度研究增强 (Deepen-Plan)

以下是通过多个专业代理并行研究获得的增强内容：

### 1. Rust 分布式锁最佳实践 (Best Practices Researcher)

#### RAII Guard 模式

```rust
/// 推荐实现：基于 RAII 的自动释放机制
pub struct NatsLockGuard {
    key: String,
    locker: Option<dist_lock::Locker>,
    keep_alive_handle: Option<JoinHandle<()>>,
    cancel_tx: Option<mpsc::Sender<()>>,
}

impl Drop for NatsLockGuard {
    fn drop(&mut self) {
        if let Some(locker) = self.locker.take() {
            // 停止 keep-alive
            if let Some(tx) = self.cancel_tx.take() {
                let _ = tx.try_send(());
            }
            // 尽力释放锁（同步上下文中）
            // 注意：Drop 中无法 await，需要使用 spawn_blocking 或接受锁超时释放
        }
    }
}
```

#### Fencing Token 最佳实践

```rust
/// Fencing Token 应存储于数据库，每次更新时验证
pub struct FencedOperation {
    lock_revision: u64,  // 从 NATS KV revision 获取
}

impl FencedOperation {
    pub async fn execute_with_fencing<F, T>(
        &self,
        pool: &Pool<MySql>,
        key: &str,
        operation: F,
    ) -> Result<T>
    where
        F: FnOnce() -> Result<T>,
    {
        // 1. 开始事务
        let mut tx = pool.begin().await?;

        // 2. 验证 fencing token
        let current_revision: Option<u64> = sqlx::query_scalar!(
            "SELECT lock_revision FROM meta WHERE key = ? FOR UPDATE",
            key
        )
        .fetch_optional(&mut *tx)
        .await?;

        if let Some(rev) = current_revision {
            if rev >= self.lock_revision {
                return Err(Error::StaleLock("Lock was acquired by another process"));
            }
        }

        // 3. 执行操作并更新 fencing token
        let result = operation()?;

        sqlx::query!(
            "UPDATE meta SET lock_revision = ? WHERE key = ?",
            self.lock_revision, key
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(result)
    }
}
```

### 2. 架构建议 (Architecture Strategist)

#### LockStrategy 抽象层

为支持未来可能的锁实现扩展（如 Redis、Consul），建议引入抽象层：

```rust
/// 锁策略 trait - 统一接口
#[async_trait]
pub trait LockStrategy: Send + Sync {
    /// 获取锁
    async fn acquire(&self, key: &str, timeout: Duration) -> Result<Box<dyn LockGuard>>;

    /// 尝试获取锁（非阻塞）
    async fn try_acquire(&self, key: &str) -> Result<Option<Box<dyn LockGuard>>>;
}

/// 锁 Guard trait
#[async_trait]
pub trait LockGuard: Send + Sync {
    /// 获取 fencing token
    fn fencing_token(&self) -> u64;

    /// 检查锁是否仍然有效
    fn is_valid(&self) -> bool;

    /// 释放锁
    async fn release(self: Box<Self>) -> Result<()>;
}

/// 锁策略工厂
pub fn create_lock_strategy(meta_store: &MetaStore) -> Box<dyn LockStrategy> {
    match meta_store {
        MetaStore::MySQL => Box::new(MysqlGetLockStrategy::new()),
        MetaStore::OceanBase => Box::new(NatsLockStrategy::new()),
        MetaStore::PostgreSQL => Box::new(PostgresAdvisoryLockStrategy::new()),
        _ => Box::new(NatsLockStrategy::new()),  // 默认使用 NATS
    }
}
```

**权衡**: 这增加了一层抽象，但考虑到：
- 未来可能支持更多数据库类型
- PostgreSQL 也有 Advisory Lock 需要类似处理
- 便于测试（可以 mock）

**建议**: Phase 1 先不引入，但设计时预留扩展点。

### 3. 性能优化建议 (Performance Oracle)

#### P99 < 100ms 可行性分析

| 操作 | 预期延迟 | 优化建议 |
|------|----------|----------|
| NATS KV create | 5-15ms | 使用本地 NATS 服务器 |
| Keep-alive interval | 每 5s | 合理，不影响性能 |
| Lock release | 3-8ms | 异步释放可降低 |
| 网络往返 | 1-3ms | 确保低延迟网络 |

**总计**: 10-30ms（正常情况），P99 < 100ms 可达成

#### Scheduler 锁分片优化

对于高并发 Scheduler 场景，考虑锁分片：

```rust
/// 锁分片策略 - 提高并发度
pub struct ShardedLock {
    shard_count: usize,
}

impl ShardedLock {
    pub fn get_lock_key(&self, org_id: &str, module: &str) -> String {
        let shard = self.hash(org_id) % self.shard_count;
        format!("/lock/{}/{}/shard_{}", module, org_id, shard)
    }

    fn hash(&self, s: &str) -> usize {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        s.hash(&mut hasher);
        hasher.finish() as usize
    }
}
```

**注意**: 仅在确认 Scheduler 锁成为瓶颈后实施。

#### Keep-alive 合并优化

多个锁的 keep-alive 可以合并到单个协程：

```rust
/// 多锁 Keep-alive 管理器
pub struct KeepAliveManager {
    locks: Arc<RwLock<HashMap<String, LockerInfo>>>,
    interval: Duration,
}

impl KeepAliveManager {
    pub async fn run(&self) {
        loop {
            tokio::time::sleep(self.interval).await;

            let locks = self.locks.read().await;
            for (key, info) in locks.iter() {
                if let Err(e) = self.renew_lock(key, info).await {
                    // 通知锁持有者
                    let _ = info.cancel_tx.send(()).await;
                }
            }
        }
    }
}
```

### 4. 安全加固建议 (Security Sentinel)

#### 风险矩阵

| 风险 | 严重性 | 发生概率 | 缓解措施 |
|------|--------|----------|----------|
| 脑裂导致数据损坏 | 高 | 低 | Fencing Token + DB 验证 |
| TOCTOU 竞争条件 | 中 | 中 | 事务内验证 fencing token |
| 锁 Key 暴露信息 | 低 | 高 | 使用 hash 而非明文 key |
| Keep-alive 被劫持 | 高 | 极低 | NATS 认证 + TLS |
| 锁超时配置错误 | 中 | 中 | 默认安全值 + 文档 |

#### 锁 Key 哈希化（可选）

```rust
/// 避免锁 key 泄露业务信息
fn hash_lock_key(key: &str) -> String {
    use sha2::{Sha256, Digest};
    let hash = Sha256::digest(key.as_bytes());
    format!("/lock/{}", hex::encode(&hash[..16]))
}
```

**权衡**: 增加调试难度，但提高安全性。建议作为可选配置。

#### 审计日志

```rust
/// 锁操作审计日志
pub fn log_lock_operation(op: &str, key: &str, result: &str, duration_ms: u64) {
    tracing::info!(
        target: "lock_audit",
        operation = op,
        lock_key = key,
        result = result,
        duration_ms = duration_ms,
        "Lock operation completed"
    );
}
```

### 5. 数据完整性增强 (Data Integrity Guardian)

#### 数据库 Schema 变更

需要为 `meta` 表添加 `lock_revision` 列以支持 Fencing Token：

```sql
-- 迁移脚本
ALTER TABLE meta ADD COLUMN lock_revision BIGINT UNSIGNED DEFAULT 0;
CREATE INDEX idx_meta_lock_revision ON meta(lock_revision);
```

#### 完整的 Fencing Token 流程

```
1. 获取 NATS 锁 → 获得 revision（如 42）
2. BEGIN TRANSACTION
3. SELECT lock_revision FROM meta WHERE key = ? FOR UPDATE
4. IF db_revision >= 42 THEN ROLLBACK (锁已过期)
5. 执行业务操作
6. UPDATE meta SET value = ?, lock_revision = 42 WHERE key = ?
7. COMMIT
8. 释放 NATS 锁
```

#### 迁移注意事项

- **现有数据**: `lock_revision = 0` 作为初始值
- **混合部署**: 新版本必须等待旧版本完全下线后才能使用 fencing token
- **回滚**: 如需回滚，新版本的 fencing token 数据对旧版本透明

### 6. 模式识别发现 (Pattern Recognition Specialist)

#### delegate crate 异步兼容性

经验证，`delegate` crate 支持异步方法，但需要注意：

```rust
// ✅ 支持
delegate! {
    to self.inner {
        async fn get(&self, key: &str) -> Result<Bytes>;
    }
}

// ⚠️ 注意：返回 impl Trait 需要特殊处理
// delegate 不支持直接返回 impl Future
```

#### 现有代码模式

项目中已有类似模式：
- `src/infra/src/db/mod.rs`: Db trait 的多实现路由
- `src/infra/src/scheduler/mod.rs`: **发现问题** - OceanBase 当前 fallthrough 到 SQLite

```rust
// scheduler/mod.rs 中的问题代码
pub async fn init() -> Result<()> {
    match db {
        Db::MySQL => mysql::init().await,
        Db::PostgreSQL => postgres::init().await,
        // OceanBase 没有显式处理！
        _ => sqlite::init().await,  // ⚠️ 错误：OceanBase 用了 SQLite scheduler
    }
}
```

**P0 修复**: 在实现 NATS 锁之前，必须先修复 scheduler 路由。

### 7. async-nats/tokio 关键知识 (Framework Docs Researcher)

#### tokio::select! 使用要点

```rust
// 正确用法：确保操作可取消
tokio::select! {
    result = operation => {
        // 操作完成
        handle_result(result)
    }
    _ = cancel_rx.recv() => {
        // 收到取消信号
        // 注意：operation 会被 drop，确保其实现了正确的取消逻辑
        return Err(Error::Cancelled);
    }
}
```

**关键点**:
- `operation` 必须是可取消的（在 await 点可以被 drop）
- 如果 `operation` 包含数据库事务，需要确保取消时事务能正确回滚

#### NATS JetStream KV 锁模式

```rust
use async_nats::jetstream::kv;

// 创建或获取 KV bucket
let kv = jetstream.get_or_create_key_value(kv::Config {
    bucket: "locks".to_string(),
    history: 1,
    ..Default::default()
}).await?;

// 使用 create 实现互斥锁
match kv.create(&lock_key, value).await {
    Ok(revision) => {
        // 获取锁成功，revision 可作为 fencing token
        Ok(LockGuard { key: lock_key, revision })
    }
    Err(e) if e.kind() == ErrorKind::AlreadyExists => {
        // 锁已被持有
        Err(Error::LockHeld)
    }
    Err(e) => Err(e.into()),
}
```

---

## P0 修复项

在开始主要实现之前，必须先修复以下问题：

### 1. Scheduler 模块 OceanBase 路由

**位置**: `src/infra/src/scheduler/mod.rs`

**问题**: OceanBase 类型 fallthrough 到 SQLite 实现

**修复**:
```rust
match db {
    Db::MySQL | Db::OceanBase => mysql::init().await,  // OceanBase 暂用 MySQL impl
    Db::PostgreSQL => postgres::init().await,
    _ => sqlite::init().await,
}
```

---

## 实施顺序更新

基于深度研究，更新实施顺序：

1. **P0**: 修复 scheduler/mod.rs OceanBase 路由问题
2. **Phase 1**: 创建 `NatsLockGuard` 和基础设施
3. **Phase 2**: 添加 `lock_revision` 数据库迁移
4. **Phase 3**: 实现 OceanBaseDb/FileList/Scheduler
5. **Phase 4**: 测试和文档

---

*Generated with Claude Code*

*Deepened with parallel research agents: Best Practices, Framework Docs, Architecture, Performance, Security, Data Integrity, Pattern Recognition*
