# OpenObserve 数据丢失防护与 Compactor 工作机制详解

## 目录

1. [Ingester 节点崩溃时的数据丢失防护机制](#1-ingester-节点崩溃时的数据丢失防护机制)
2. [Compactor 组件工作机制详解](#2-compactor-组件工作机制详解)
3. [Ingester 与 Compactor 的交互方式](#3-ingester-与-compactor-的交互方式)
4. [完整的数据流转时间线](#4-完整的数据流转时间线)

---

## 1. Ingester 节点崩溃时的数据丢失防护机制

### 问题背景

用户关心的核心问题是:**因为数据只会发送到一个 Ingester 节点,如果这个节点处理过程中崩溃了,怎么保证这条数据不丢失呢?**

### 1.1 数据丢失防护的三层保障

OpenObserve 通过**三层保障机制**确保即使 Ingester 节点崩溃,数据也不会丢失:

#### **第一层:WAL 持久化写入(最重要)**

在数据写入 Ingester 时,采用 **WAL-First 策略** - 数据先写入 WAL(Write-Ahead Log)文件,再写入内存 MemTable。

**关键代码** ([src/ingester/src/writer.rs](src/ingester/src/writer.rs#L370-L400)):

```rust
async fn consume_processed(&self, batch: crate::ProcessedBatch, fsync: bool) -> Result<()> {
    // 1. 检查是否需要 rotation
    self.rotate(batch.entries_json_size, batch.entries_arrow_size).await?;

    // 2. 先写入 WAL (磁盘) - 这是第一优先级
    let mut wal = self.wal.write().await;
    for entry in batch.bytes_entries {
        wal.write(&entry).context(WalSnafu)?;  // 写入本地磁盘
    }

    // 3. 再写入 MemTable (内存)
    let mut mem = self.memtable.write().await;
    for (entry, batch_entry) in batch.entries.into_iter().zip(batch.batch_entries) {
        mem.write(entry.schema.clone().unwrap(), entry, batch_entry)?;
    }

    // 4. 可选的强制 fsync 刷盘
    if fsync {
        wal.sync().context(WalSnafu)?;  // 强制刷新到磁盘
    }

    Ok(())
}
```

**WAL 文件格式** (带 CRC32 校验和 Snappy 压缩) - [src/wal/src/writer.rs](src/wal/src/writer.rs#L50-L70):

```rust
pub fn write(&mut self, data: &[u8]) -> Result<()> {
    // 1. 写入 8 字节头部占位符
    self.buffer.write_u64::<BigEndian>(0).expect("cannot fail");

    // 2. Snappy 压缩 + CRC32 校验
    let mut encoder = snap::write::FrameEncoder::new(HasherWrapper::new(&mut self.buffer));
    encoder.write_all(data).context(UnableToCompressDataSnafu)?;
    let (checksum, buf) = encoder.into_inner().unwrap().finalize();

    // 3. 写入头部: CRC32 (4 字节) + compressed_len (4 字节)
    buf.write_u32::<BigEndian>(checksum)?;
    buf.write_u32::<BigEndian>(compressed_len as u32)?;

    // 4. 写入文件
    self.f.write_all(buf).context(WriteDataSnafu)?;

    Ok(())
}
```

**关键特性:**
- **CRC32 校验码**:确保数据完整性,能检测磁盘损坏
- **Snappy 压缩**:减少磁盘 I/O,同时保证数据可逆
- **fsync 可选**:重要数据可强制刷盘,确保操作系统缓存全部落盘

#### **第二层:节点重启时的 WAL Replay 机制**

当 Ingester 节点崩溃重启后,会自动**重放所有未处理的 WAL 文件**,恢复崩溃前的数据。

**WAL Replay 流程** ([src/ingester/src/wal.rs](src/ingester/src/wal.rs#L108-L237)):

```rust
// 在 ingester init 时自动调用
pub(crate) async fn replay_wal_files(wal_dir: PathBuf, wal_files: Vec<PathBuf>) -> Result<()> {
    if wal_files.is_empty() {
        return Ok(());
    }

    for wal_file in wal_files.iter() {
        log::warn!("replay wal file: {wal_file:?} starting...");

        // 1. 解析 WAL 文件路径,提取 org_id/stream_type 等元数据
        let file_str = wal_file.strip_prefix(&wal_dir).unwrap()...;
        let file_columns = file_str.split('/').collect::<Vec<_>>();
        let stream_type = file_columns[file_columns.len() - 2];
        let org_id = file_columns[file_columns.len() - 3];

        // 2. 创建 MemTable 准备重建内存数据
        let mut memtable = memtable::MemTable::new();
        let mut reader = match wal::Reader::from_path(wal_file) {
            Ok(v) => v,
            Err(e) => {
                log::error!("Unable to open the wal file err: {e}, skip");
                continue;  // 跳过损坏的 WAL 文件
            }
        };

        // 3. 循环读取 WAL 每条 Entry
        loop {
            let entry = match reader.read_entry() {
                Ok(entry) => entry,
                // 容错处理:数据损坏时跳过该条记录,继续处理下一条
                Err(wal::Error::UnableToReadData { source }) => {
                    log::error!("Unable to read entry from: {source}, skip the entry");
                    continue;
                }
                Err(wal::Error::ChecksumMismatch { expected, actual }) => {
                    log::error!("Checksum mismatch: expected {expected}, actual {actual}, skip");
                    continue;
                }
                Err(e) => return Err(Error::WalError { source: e }),
            };

            let Some(entry_bytes) = entry else { break; };
            let mut entry = Entry::from_bytes(&entry_bytes)?;

            // 4. 获取最新 schema 并写入 MemTable
            let latest_schema = infra::schema::get_cache(org_id, &entry.stream, stream_type).await?;
            entry.schema_key = latest_schema.hash_key().into();
            let batch = entry.into_batch(key.stream_type.clone(), infer_schema.clone())?;
            memtable.write(infer_schema, entry, batch)?;
        }

        // 5. 直接将 MemTable dump 到本地 Parquet 文件
        let immutable = immutable::Immutable::new(idx, key, memtable);
        let stat = immutable.persist(&wal_path).await?;

        log::warn!("replay wal file: {:?} done, json_size: {}, file_num: {}",
            wal_path, stat.json_size, stat.file_num);
    }

    Ok(())
}
```

**启动时自动触发 WAL Replay** ([src/ingester/src/lib.rs](src/ingester/src/lib.rs#L87-L103)):

```rust
pub async fn init() -> errors::Result<()> {
    // 1. 检查未完成的 Parquet 文件(处理崩溃时的中间状态)
    wal::check_uncompleted_parquet_files().await?;

    // 2. 扫描所有 .wal 文件
    let wal_dir = PathBuf::from(&config::get_config().common.data_wal_dir).join("logs");
    create_dir_all(&wal_dir).context(OpenDirSnafu { path: wal_dir.clone() })?;
    let wal_files = wal::wal_scan_files(&wal_dir, "wal").await.unwrap_or_default();

    // 3. 后台异步重放所有 WAL 文件
    tokio::task::spawn(async move {
        if let Err(e) = wal::replay_wal_files(wal_dir, wal_files).await {
            log::error!("replay wal files error: {e}");
        }
    });

    Ok(())
}
```

**容错机制:**
- **跳过损坏的 Entry**:即使部分数据损坏,也能恢复其他完好的数据
- **CRC32 校验**:精确检测哪些 Entry 损坏
- **自动清理中间状态**:处理崩溃时留下的 `.par`、`.lock` 文件

#### **第三层:Lock File 保护机制(防止并发冲突)**

在 WAL 转换为 Parquet 文件的过程中,使用 **Lock File** 确保原子性操作。

**Lock File 检查流程** ([src/ingester/src/wal.rs](src/ingester/src/wal.rs#L34-L106)):

```rust
// WAL 文件转换过程的 5 步流程:
// 1. 将内存数据写入磁盘,扩展名为 .par
// 2. 创建 lock 文件,记录所有 .par 文件名
// 3. 删除 .wal 文件
// 4. 将 .par 文件重命名为 .parquet
// 5. 删除 lock 文件
//
// 崩溃恢复场景:
// - 崩溃发生在步骤 2 之前:有 .par 文件但无 lock 文件 -> 删除这些 .par 文件
// - 崩溃发生在步骤 2-5 之间:有 .par/.parquet 文件和 lock 文件 -> 继续完成后续步骤

pub(crate) async fn check_uncompleted_parquet_files() -> Result<()> {
    let cfg = config::get_config();
    let wal_dir = PathBuf::from(&cfg.common.data_wal_dir).join(crate::WAL_DIR_DEFAULT_PREFIX);

    // 1. 扫描所有 .lock 文件
    let lock_files = wal_scan_files(wal_dir, "lock").await.unwrap_or_default();

    // 2. 对每个 lock 文件,检查对应的 .wal 文件并完成转换
    for lock_file in lock_files.iter() {
        log::warn!("found uncompleted wal file: {lock_file:?}");

        let wal_file = lock_file.with_extension("wal");
        if wal_file.exists() {
            // 删除已处理的 .wal 文件
            log::warn!("delete processed wal file: {wal_file:?}");
            std::fs::remove_file(&wal_file)?;
        }

        // 读取 lock 文件中记录的所有 .par 文件
        let mut file = File::open(lock_file)?;
        let mut par_files = Vec::new();
        for line in BufReader::new(&mut file).lines() {
            let line = line?;
            par_files.push(line);
        }

        // 将所有 .par 文件重命名为 .parquet
        for par_file in par_files.iter() {
            let par_file = PathBuf::from(par_file);
            let parquet_file = par_file.with_extension("parquet");
            log::warn!("rename par file: {par_file:?} to parquet");
            if par_file.exists() {
                std::fs::rename(&par_file, &parquet_file)?;
            }
        }

        // 删除 lock 文件
        log::warn!("delete lock file: {lock_file:?}");
        std::fs::remove_file(lock_file)?;
    }

    // 3. 删除所有孤立的 .par 文件(无对应 lock 文件)
    let parquet_dir = PathBuf::from(&cfg.common.data_wal_dir).join("files");
    let par_files = wal_scan_files(parquet_dir, "par").await.unwrap_or_default();
    for par_file in par_files.iter() {
        log::warn!("delete uncompleted par file: {par_file:?}");
        std::fs::remove_file(par_file)?;
    }

    Ok(())
}
```

**保护机制总结:**
- **Lock File 记录**:确保知道哪些文件是一组需要原子操作的
- **自动恢复**:重启时自动检测并完成未完成的转换
- **垃圾清理**:删除无效的中间文件

### 1.2 客户端重试机制(HTTP 层面)

除了 Ingester 节点本身的数据持久化,客户端也有重试机制:

#### HTTP 5xx 错误自动重试

如果 Ingester 节点在处理数据时崩溃,会返回 **HTTP 5xx 错误**(如 503 Service Unavailable),客户端应实现重试逻辑:

```python
# Python 客户端示例(使用 requests 库的 Retry 机制)
from requests.adapters import HTTPAdapter
from requests.packages.urllib3.util.retry import Retry
import requests

# 配置重试策略
retry_strategy = Retry(
    total=3,                          # 最多重试 3 次
    status_forcelist=[500, 502, 503, 504],  # 这些状态码触发重试
    method_whitelist=["POST"],        # POST 请求也可重试(幂等性)
    backoff_factor=1                  # 指数退避:1s, 2s, 4s
)

adapter = HTTPAdapter(max_retries=retry_strategy)
http = requests.Session()
http.mount("http://", adapter)
http.mount("https://", adapter)

# 发送日志数据
response = http.post(
    "http://ingester-node:5080/api/default/logs/_json",
    json={"message": "test log"},
    timeout=10
)
```

#### Router 节点的负载均衡与故障转移

如果集群部署了 **Router 节点**,Router 会自动检测 Ingester 节点的健康状态,将请求路由到健康的节点:

```
客户端 -> Router -> [Ingester-1 (健康), Ingester-2 (崩溃), Ingester-3 (健康)]
                     ↑ 选择健康节点         ↑ 自动跳过           ↑ 选择健康节点
```

Router 通过定期的健康检查(如 gRPC ping 或 HTTP health check)维护可用节点列表,确保流量只发送到健康节点。

### 1.3 完整的数据不丢失保障流程图

```
┌──────────────────────────────────────────────────────────────────────────┐
│ 客户端发送数据                                                             │
└────────────┬─────────────────────────────────────────────────────────────┘
             │
             ▼
┌────────────────────────────────────────────────────────────────────────┐
│ Router 节点(可选)                                                        │
│  - 检测 Ingester 健康状态                                                │
│  - 将请求路由到健康节点                                                   │
└────────────┬───────────────────────────────────────────────────────────┘
             │
             ▼
┌────────────────────────────────────────────────────────────────────────┐
│ Ingester 节点接收数据                                                    │
│  ┌─────────────────────────────────────────────────────────────┐       │
│  │ 步骤 1: 写入 WAL (磁盘)                                       │       │
│  │  - CRC32 校验                                                │       │
│  │  - Snappy 压缩                                               │       │
│  │  - fsync 刷盘(可选)                                          │       │
│  └─────────────────────────────────────────────────────────────┘       │
│                        │                                                │
│                        ▼                                                │
│  ┌─────────────────────────────────────────────────────────────┐       │
│  │ 步骤 2: 写入 MemTable (内存)                                 │       │
│  │  - Arrow 格式                                                │       │
│  │  - 高性能查询                                                │       │
│  └─────────────────────────────────────────────────────────────┘       │
│                        │                                                │
│                        ▼                                                │
│  ┌─────────────────────────────────────────────────────────────┐       │
│  │ 步骤 3: 返回 HTTP 200 OK                                     │       │
│  │  - 只有 WAL 和 MemTable 都写入成功才返回 200                  │       │
│  └─────────────────────────────────────────────────────────────┘       │
└────────────┬───────────────────────────────────────────────────────────┘
             │
             ▼
┌────────────────────────────────────────────────────────────────────────┐
│ 如果节点崩溃...                                                          │
│  ┌─────────────────────────────────────────────────────────────┐       │
│  │ 节点重启时:                                                   │       │
│  │  1. 扫描 WAL 目录                                            │       │
│  │  2. Replay 所有 .wal 文件                                    │       │
│  │  3. 重建 MemTable                                            │       │
│  │  4. 生成本地 Parquet 文件                                    │       │
│  │  5. 正常服务恢复                                              │       │
│  └─────────────────────────────────────────────────────────────┘       │
└────────────────────────────────────────────────────────────────────────┘

数据丢失风险窗口分析:

❌ 客户端崩溃(未发送成功) -> 客户端负责重试
❌ 网络丢包 -> TCP/IP 层自动重传或客户端 HTTP 重试
✅ Ingester 接收到数据后崩溃 -> WAL Replay 恢复
✅ Ingester 写入 WAL 后崩溃 -> WAL Replay 恢复
✅ Ingester 写入 MemTable 后崩溃 -> WAL Replay 恢复
✅ 磁盘数据损坏 -> CRC32 检测,跳过损坏部分,恢复完好数据
```

### 1.4 总结:Ingester 崩溃后的数据安全保障

| 保障层级 | 机制 | 覆盖场景 | 数据丢失风险 |
|---------|------|---------|-------------|
| **第 1 层** | WAL 持久化 + CRC32 校验 | 节点崩溃、断电 | ✅ 0% (已写入磁盘) |
| **第 2 层** | WAL Replay 机制 | 节点重启后数据恢复 | ✅ 0% (自动重放) |
| **第 3 层** | Lock File 原子性保护 | 文件转换过程中崩溃 | ✅ 0% (自动完成转换) |
| **辅助层** | 客户端 HTTP 重试 | 节点崩溃时请求失败 | ⚠️ 依赖客户端实现 |
| **辅助层** | Router 负载均衡 | 集群多节点故障转移 | ✅ 0% (自动路由到健康节点) |

**结论:**只要数据成功写入 WAL(Ingester 返回 HTTP 200),即使节点立即崩溃,数据也不会丢失。重启后会自动通过 WAL Replay 机制恢复所有数据。

---

## 2. Compactor 组件工作机制详解

### 问题背景

用户关心的问题:**Compactor 组件是定时任务吗?是在哪个环节起作用的,上传到对象存储之前?他跟 Ingester 节点有数据交互吗?如果不在一台机器上时时如何处理的?**

### 2.1 Compactor 的核心定位

**Compactor 是一个定时任务组件**,但它运行的是**多个独立的定时任务**,每个任务负责不同的数据优化工作。

**关键要点:**
1. ✅ Compactor **是定时任务**(由多个 `spawn_pausable_job!` 宏创建)
2. ✅ Compactor 在**对象存储上传之后**起作用(不是之前)
3. ❌ Compactor **不与 Ingester 直接交互**,通过共享存储层(S3/数据库)间接协作
4. ✅ Compactor 可以在**不同机器**上运行,通过对象存储 + 数据库协调

### 2.2 Compactor 的定时任务结构

Compactor 组件在 [src/job/compactor.rs](src/job/compactor.rs) 中定义了 **11 个独立的定时任务**:

```rust
pub async fn run() -> Result<(), anyhow::Error> {
    // 只有节点角色为 Compactor 时才运行
    if !LOCAL_NODE.is_compactor() {
        return Ok(());
    }

    let cfg = get_config();
    if !cfg.compact.enabled {
        return Ok(());
    }
    log::info!("[COMPACTOR::JOB] Compactor is enabled");

    // 启动 Merge Worker(多线程工作池)
    let mut worker = compact::worker::MergeWorker::new(cfg.limit.file_merge_thread_num);
    worker.run()?;

    // 启动 Job Scheduler(任务调度器)
    let mut scheduler = compact::worker::JobScheduler::new(
        cfg.limit.file_merge_thread_num,
        worker.tx()
    );
    scheduler.run()?;

    // ========== 定时任务 1: 生成当前数据合并任务 ==========
    // 间隔: compact.interval (默认 60 秒)
    spawn_pausable_job!("run_generate_job", get_config().compact.interval, {
        log::debug!("[COMPACTOR::JOB] Running generate merge job");
        if let Err(e) = compact::run_generate_job(CompactionJobType::Current).await {
            log::error!("[COMPACTOR::JOB] run generate merge job error: {e}");
        }
    });

    // ========== 定时任务 2: 生成历史数据合并任务 ==========
    // 间隔: compact.old_data_interval (默认 300 秒)
    spawn_pausable_job!(
        "run_generate_old_data_job",
        get_config().compact.old_data_interval.saturating_add(1),
        {
            log::debug!("[COMPACTOR::JOB] Running generate merge job for old data");
            if let Err(e) = compact::run_generate_job(CompactionJobType::Historical).await {
                log::error!("[COMPACTOR::JOB] run generate merge job for old data error: {e}");
            }
        }
    );

    // ========== 定时任务 3: 降采样任务(企业版) ==========
    #[cfg(feature = "enterprise")]
    spawn_pausable_job!(
        "compactor_downsampling",
        get_o2_config().downsampling.downsampling_interval,
        {
            if get_o2_config().downsampling.metrics_downsampling_rules.is_empty() {
                continue;
            }
            log::debug!("[COMPACTOR::JOB] Running generate downsampling job");
            if let Err(e) = compact::run_generate_downsampling_job().await {
                log::error!("[COMPACTOR::JOB] run generate downsampling job error: {e}");
            }
        }
    );

    // ========== 定时任务 4: 执行数据合并 ==========
    // 间隔: compact.interval + 2 (默认 62 秒)
    spawn_pausable_job!("run_merge", get_config().compact.interval + 2, {
        log::debug!("[COMPACTOR::JOB] Running data merge");
        if let Err(e) = compact::run_merge(scheduler.tx().clone()).await {
            log::error!("[COMPACTOR::JOB] run data merge error: {e}");
        }
    });

    // ========== 定时任务 5: 数据保留期清理 ==========
    // 间隔: compact.interval + 3 (默认 63 秒)
    spawn_pausable_job!("run_retention", get_config().compact.interval + 3, {
        log::debug!("[COMPACTOR::JOB] Running data retention");
        if let Err(e) = compact::run_retention().await {
            log::error!("[COMPACTOR::JOB] run data retention error: {e}");
        }
    });

    // ========== 定时任务 6: 延迟删除 ==========
    // 间隔: compact.interval + 4 (默认 64 秒)
    spawn_pausable_job!("run_delay_deletion", get_config().compact.interval + 4, {
        log::debug!("[COMPACTOR::JOB] Running data delay deletion");
        if let Err(e) = compact::run_delay_deletion().await {
            log::error!("[COMPACTOR::JOB] run data delay deletion error: {e}");
        }
    });

    // ========== 定时任务 7: 同步 Offset 到数据库 ==========
    // 间隔: compact.sync_to_db_interval (默认 300 秒)
    spawn_pausable_job!(
        "compactor_sync_to_db",
        get_config().compact.sync_to_db_interval,
        {
            log::debug!("[COMPACTOR::JOB] Running sync cached compact offset to db");
            if let Err(e) = crate::service::db::compact::files::sync_cache_to_db().await {
                log::error!("[COMPACTOR::JOB] run sync cached compact offset to db error: {e}");
            }
        }
    );

    // ========== 定时任务 8: 降采样 Offset 同步(企业版) ==========
    #[cfg(feature = "enterprise")]
    spawn_pausable_job!(
        "compactor_downsampling_sync_to_db",
        get_config().compact.sync_to_db_interval,
        {
            // ...
        }
    );

    // ========== 定时任务 9: 检查运行中的任务超时 ==========
    // 间隔: compact.job_run_timeout (默认 3600 秒)
    spawn_pausable_job!(
        "compactor_check_running_jobs",
        get_config().compact.job_run_timeout,
        {
            log::debug!("[COMPACTOR::JOB] Running check running jobs");
            let timeout = get_config().compact.job_run_timeout;
            let updated_at = config::utils::time::now_micros() - (timeout * 1000 * 1000);
            if let Err(e) = infra::file_list::check_running_jobs(updated_at).await {
                log::error!("[COMPACTOR::JOB] run check running jobs error: {e}");
            }
        },
        sleep_after
    );

    // ========== 定时任务 10: 清理已完成的任务 ==========
    // 间隔: compact.job_clean_wait_time (默认 7200 秒)
    spawn_pausable_job!(
        "compactor_clean_done_jobs",
        get_config().compact.job_clean_wait_time,
        {
            log::debug!("[COMPACTOR::JOB] Running clean done jobs");
            let wait_time = get_config().compact.job_clean_wait_time;
            let updated_at = config::utils::time::now_micros() - (wait_time * 1000 * 1000);
            if let Err(e) = infra::file_list::clean_done_jobs(updated_at).await {
                log::error!("[COMPACTOR::JOB] run clean done jobs error: {e}");
            }
        },
        sleep_after
    );

    // ========== 定时任务 11: 待处理任务指标上报 ==========
    // 间隔: compact.pending_jobs_metric_interval (默认 60 秒)
    spawn_pausable_job!(
        "run_compactor_pending_jobs_metric",
        get_config().compact.pending_jobs_metric_interval,
        {
            log::debug!("[COMPACTOR::JOB] Running compactor pending jobs to report metric");
            let job_status = match infra::file_list::get_pending_jobs_count().await {
                Ok(status) => status,
                Err(e) => {
                    log::error!("[COMPACTOR::JOB] run compactor pending jobs metric error: {e}");
                    continue;
                }
            };

            // 更新 Prometheus 指标
            for ((org_id, stream_type), counter) in job_status {
                metrics::COMPACT_PENDING_JOBS
                    .with_label_values(&[org_id.as_str(), stream_type.as_str()])
                    .set(counter);
            }
        }
    );

    // ========== 额外任务: EnrichmentTable 合并 ==========
    tokio::task::spawn(async move { run_enrichment_table_merge().await });

    Ok(())
}
```

### 2.3 Compactor 定时任务详细说明

| 任务编号 | 任务名称 | 默认间隔 | 功能描述 |
|---------|---------|---------|---------|
| **1** | `run_generate_job` | 60 秒 | 扫描对象存储,为**当前数据**生成合并任务(小文件合并为大文件) |
| **2** | `run_generate_old_data_job` | 300 秒 | 为**历史数据**生成合并任务(通常合并更大的时间范围) |
| **3** | `compactor_downsampling` | 300 秒 | 企业版功能,对指标数据进行降采样 |
| **4** | `run_merge` | 62 秒 | 从任务队列拉取并**执行合并任务**(核心任务) |
| **5** | `run_retention` | 63 秒 | 根据数据保留策略**删除过期数据** |
| **6** | `run_delay_deletion` | 64 秒 | 执行延迟删除(标记为删除的文件) |
| **7** | `compactor_sync_to_db` | 300 秒 | 将内存中的 Offset 缓存同步到数据库 |
| **8** | `compactor_downsampling_sync_to_db` | 300 秒 | 降采样 Offset 同步到数据库(企业版) |
| **9** | `compactor_check_running_jobs` | 3600 秒 | 检查超时任务,标记为失败并重新调度 |
| **10** | `compactor_clean_done_jobs` | 7200 秒 | 清理数据库中已完成的任务记录 |
| **11** | `run_compactor_pending_jobs_metric` | 60 秒 | 统计待处理任务数量,上报到 Prometheus |

### 2.4 Compactor 的工作时机:对象存储上传**之后**

**重要结论:Compactor 操作的是已经上传到对象存储(S3/MinIO/GCS)的文件,不是本地文件。**

#### 数据流转时间线:

```
Ingester 节点:
  数据到达
    ↓
  写入 WAL (本地磁盘)
    ↓
  写入 MemTable (内存)
    ↓
  MemTable 达到阈值/超时
    ↓
  转换为本地 Parquet 文件 (files/ 目录)
    ↓
  【关键步骤】上传到对象存储 (S3/MinIO)  ← Ingester 的职责结束
    ↓
  写入 file_list 表(记录文件元数据)
    ↓
  删除本地 Parquet 文件

================================================

Compactor 节点:
  【等待】定时任务触发(如每 60 秒)
    ↓
  从数据库 file_list 表查询文件列表
    ↓
  分析哪些文件需要合并(小文件、碎片文件)
    ↓
  【关键步骤】从对象存储下载文件到本地  ← Compactor 的工作开始
    ↓
  合并多个小 Parquet 文件为大文件
    ↓
  上传合并后的大文件到对象存储
    ↓
  更新 file_list 表(删除旧文件记录,添加新文件记录)
    ↓
  删除对象存储中的旧文件
```

**核心区别:**
- **Ingester 的合并**:合并本地 WAL 目录下的 Parquet 文件,**然后上传到对象存储**
- **Compactor 的合并**:从对象存储**下载已有文件**,合并后重新上传

### 2.5 Compactor 与 Ingester 的交互方式:通过共享存储层

Compactor 和 Ingester **不直接通信**,而是通过以下两个共享层间接协作:

#### 1. 对象存储(S3/MinIO/GCS)

**Ingester 写入:**

```rust
// src/job/files/parquet.rs - Ingester 上传文件到对象存储
async fn move_files(thread_id: usize, prefix: &str, files: Vec<FileKey>) -> Result<(), anyhow::Error> {
    // 1. 获取最新 schema
    let latest_schema = infra::schema::get(&org_id, &stream_name, stream_type).await?;

    // 2. 合并本地 Parquet 文件
    let (account, new_file_name, new_file_meta, new_file_list) =
        merge_files(thread_id, latest_schema.clone(), &wal_dir, &files_with_size, num_uds_fields).await?;

    // 3. 上传到对象存储(S3/MinIO)
    // 注意:这里的 set 操作会自动触发上传到对象存储
    db::file_list::set(&account, &new_file_name, Some(new_file_meta), false).await?;

    // 4. 删除本地文件
    for file in new_file_list.iter() {
        WAL_PARQUET_METADATA.write().await.remove(&file.key);
        remove_file(wal_dir.join(&file.key)).await?;
        PROCESSING_FILES.remove_async(&file.key).await;
    }

    Ok(())
}
```

**Compactor 读取:**

```rust
// src/service/compact/merge.rs - Compactor 从对象存储读取文件
pub async fn generate_job_by_stream(
    org_id: &str,
    stream_type: StreamType,
    stream_name: &str
) -> Result<(), anyhow::Error> {
    // 1. 获取 offset(记录上次处理到哪个时间点)
    let (mut offset, node) = db::compact::files::get_offset(
        org_id, stream_type, stream_name
    ).await;

    // 2. 检查是否有其他节点正在处理这个 stream
    if !node.is_empty()
        && LOCAL_NODE.uuid.ne(&node)
        && get_node_by_uuid(&node).await.is_some()
    {
        return Ok(()); // 其他节点正在处理,退出
    }

    // 3. 获取分布式锁(防止多个 Compactor 同时处理)
    let lock_key = format!("/compact/merge/{org_id}/{stream_type}/{stream_name}");
    let locker = dist_lock::lock(&lock_key, 0).await?;

    // 4. 标记当前节点正在处理
    db::compact::files::set_offset(
        org_id, stream_type, stream_name,
        offset,
        Some(&LOCAL_NODE.uuid)  // 记录当前节点 UUID
    ).await?;

    // 5. 从对象存储查询文件列表(通过 file_list 表)
    let files = db::file_list::query_by_stream(
        org_id, stream_type, stream_name,
        offset, limit
    ).await?;

    // 6. 分析哪些文件需要合并(小文件、碎片文件)
    let merge_jobs = analyze_merge_jobs(files)?;

    // 7. 创建合并任务(写入数据库任务队列)
    for job in merge_jobs {
        db::compact::create_job(job).await?;
    }

    dist_lock::unlock(&locker).await?;
    Ok(())
}
```

#### 2. 数据库(MySQL/PostgreSQL/SQLite)

Compactor 和 Ingester 通过**数据库表**进行协调:

**关键数据库表:**

1. **`file_list` 表** - 记录所有对象存储中的文件
   ```sql
   CREATE TABLE file_list (
       id BIGINT PRIMARY KEY,
       org_id VARCHAR(100),
       stream_type VARCHAR(50),
       stream_name VARCHAR(256),
       file_name VARCHAR(512),      -- 对象存储中的文件路径
       file_size BIGINT,
       min_ts BIGINT,               -- 文件最小时间戳
       max_ts BIGINT,               -- 文件最大时间戳
       records BIGINT,              -- 记录数
       compressed_size BIGINT,
       created_at TIMESTAMP
   );
   ```

2. **`file_list_jobs` 表** - Compactor 的任务队列
   ```sql
   CREATE TABLE file_list_jobs (
       id BIGINT PRIMARY KEY,
       org_id VARCHAR(100),
       stream_type VARCHAR(50),
       stream_name VARCHAR(256),
       files TEXT,                  -- 需要合并的文件列表(JSON 数组)
       status VARCHAR(20),          -- Waiting, Running, Done, Failed
       node_uuid VARCHAR(50),       -- 哪个 Compactor 节点在处理
       started_at TIMESTAMP,
       completed_at TIMESTAMP
   );
   ```

3. **`compact_offset` 表** - 记录每个 stream 的处理进度
   ```sql
   CREATE TABLE compact_offset (
       org_id VARCHAR(100),
       stream_type VARCHAR(50),
       stream_name VARCHAR(256),
       offset BIGINT,               -- 已处理到的时间戳
       node_uuid VARCHAR(50),       -- 当前处理的节点 UUID
       updated_at TIMESTAMP,
       PRIMARY KEY (org_id, stream_type, stream_name)
   );
   ```

**工作流程:**

```
Ingester:
  1. 上传 Parquet 文件到 S3
  2. INSERT INTO file_list (file_name, file_size, min_ts, max_ts, ...)

Compactor:
  1. SELECT * FROM file_list WHERE min_ts > offset ORDER BY min_ts LIMIT 1000
  2. 分析哪些文件需要合并(如 5 个小文件合并为 1 个大文件)
  3. INSERT INTO file_list_jobs (files=['file1', 'file2', ...], status='Waiting')
  4. UPDATE file_list_jobs SET status='Running', node_uuid='xxx' WHERE id=xxx
  5. 从 S3 下载 file1, file2, ..., 合并为 merged_file
  6. 上传 merged_file 到 S3
  7. INSERT INTO file_list (file_name=merged_file, ...)
  8. DELETE FROM file_list WHERE file_name IN ('file1', 'file2', ...)
  9. 从 S3 删除 file1, file2, ...
  10. UPDATE file_list_jobs SET status='Done', completed_at=NOW()
```

### 2.6 跨机器运行时的协调机制

**关键问题:如果 Compactor 和 Ingester 不在同一台机器上,如何协调?**

**答案:通过分布式锁 + 数据库 + 对象存储**

#### 1. 分布式锁(防止重复处理)

```rust
// Compactor 获取分布式锁
let lock_key = format!("/compact/merge/{org_id}/{stream_type}/{stream_name}");
let locker = dist_lock::lock(&lock_key, 0).await?;

// 处理完成后释放锁
dist_lock::unlock(&locker).await?;
```

**锁的作用:**
- 确保同一时刻只有一个 Compactor 节点处理某个 stream
- 即使有 10 个 Compactor 节点,也不会重复合并同一批文件

#### 2. 节点 UUID 标记

```rust
// 在数据库中记录当前节点 UUID
db::compact::files::set_offset(
    org_id, stream_type, stream_name,
    offset,
    Some(&LOCAL_NODE.uuid)  // 标记当前节点
).await?;

// 其他节点检查是否有节点正在处理
let (offset, node) = db::compact::files::get_offset(org_id, stream_type, stream_name).await;
if !node.is_empty()
    && LOCAL_NODE.uuid.ne(&node)  // 不是当前节点
    && get_node_by_uuid(&node).await.is_some()  // 节点仍然在线
{
    return Ok(()); // 跳过,其他节点正在处理
}
```

#### 3. 对象存储统一访问

所有节点(Ingester/Compactor)访问同一个对象存储:

```yaml
# 配置示例
ZO_S3_ENDPOINT: https://s3.us-west-1.amazonaws.com
ZO_S3_BUCKET: my-openobserve-bucket
ZO_S3_ACCESS_KEY: AKIAIOSFODNN7EXAMPLE
ZO_S3_SECRET_KEY: wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY
```

**所有节点看到同一份数据:**
- Ingester-1 上传的文件,Compactor-5 可以立即看到
- Compactor-3 合并后的文件,Querier-2 可以立即查询

### 2.7 Compactor 工作机制总结

| 问题 | 答案 |
|------|------|
| **是定时任务吗?** | ✅ 是,运行 11 个独立的定时任务(每 60 秒到 7200 秒不等) |
| **在哪个环节起作用?** | ✅ 在对象存储上传**之后**,操作已上传的文件 |
| **跟 Ingester 有数据交互吗?** | ❌ 没有直接交互,通过对象存储 + 数据库间接协作 |
| **不在一台机器上如何处理?** | ✅ 通过分布式锁 + 节点 UUID + 对象存储统一访问 |

---

## 3. Ingester 与 Compactor 的交互方式

### 3.1 两者的职责边界

| 组件 | 职责 | 操作对象 | 时机 |
|------|------|---------|------|
| **Ingester** | 实时数据摄入 → 本地 Parquet → 上传对象存储 | 本地 WAL 目录下的文件 | 实时(数据到达后立即处理) |
| **Compactor** | 优化存储 → 合并小文件 → 删除过期数据 | 对象存储中的文件 | 定时(每 60 秒检查一次) |

### 3.2 两者的文件合并区别

#### Ingester 的文件合并(Local Merge)

**目的:**将 WAL 目录下同一 stream 的多个 Parquet 文件合并为一个,减少对象存储的上传次数。

**流程:**
```
WAL 目录:
  files/org1/logs/stream1/0/2025/03/20/12/abc123/file1.parquet  (1 MB)
  files/org1/logs/stream1/0/2025/03/20/12/abc123/file2.parquet  (1.2 MB)
  files/org1/logs/stream1/0/2025/03/20/12/abc123/file3.parquet  (0.8 MB)

合并后:
  merged_file_20250320120000.parquet  (3 MB)

上传到 S3:
  s3://bucket/files/org1/logs/stream1/2025/03/20/12/merged_file_20250320120000.parquet
```

**代码位置:**[src/job/files/parquet.rs](src/job/files/parquet.rs) - `move_files()` 函数

#### Compactor 的文件合并(Remote Merge)

**目的:**将对象存储中碎片化的小文件合并为大文件,提升查询性能。

**流程:**
```
S3 中的文件(假设每小时有 10 个小文件):
  s3://bucket/files/org1/logs/stream1/2025/03/20/10/file1.parquet  (5 MB)
  s3://bucket/files/org1/logs/stream1/2025/03/20/10/file2.parquet  (4 MB)
  s3://bucket/files/org1/logs/stream1/2025/03/20/10/file3.parquet  (6 MB)
  ...
  s3://bucket/files/org1/logs/stream1/2025/03/20/10/file10.parquet (5 MB)

合并后(假设合并为 2 个大文件):
  s3://bucket/files/org1/logs/stream1/2025/03/20/10/compacted_file1.parquet  (25 MB)
  s3://bucket/files/org1/logs/stream1/2025/03/20/10/compacted_file2.parquet  (25 MB)

删除原文件:
  file1.parquet, file2.parquet, ..., file10.parquet
```

**代码位置:**[src/service/compact/merge.rs](src/service/compact/merge.rs) - `generate_job_by_stream()` 函数

### 3.3 完整的数据流图

```
┌─────────────────────────────────────────────────────────────────┐
│ 客户端                                                            │
└───────────┬─────────────────────────────────────────────────────┘
            │ HTTP POST /api/org1/logs/_json
            ▼
┌─────────────────────────────────────────────────────────────────┐
│ Ingester 节点                                                     │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ 步骤 1: 写入 WAL (磁盘)                                    │   │
│  │  files/org1/logs/stream1/0/2025/03/20/12/abc/wal1.wal     │   │
│  └──────────────────────────────────────────────────────────┘   │
│                     │                                            │
│                     ▼                                            │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ 步骤 2: 写入 MemTable (内存 Arrow 格式)                    │   │
│  └──────────────────────────────────────────────────────────┘   │
│                     │                                            │
│                     ▼                                            │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ 步骤 3: MemTable 达到阈值/超时                             │   │
│  │  - 转换为本地 Parquet 文件                                 │   │
│  │  files/org1/logs/stream1/0/2025/03/20/12/abc/file1.parquet│   │
│  └──────────────────────────────────────────────────────────┘   │
│                     │                                            │
│                     ▼                                            │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ 步骤 4: Local Merge (Ingester 的合并)                     │   │
│  │  - 合并同一目录下的多个 Parquet 文件                       │   │
│  │  - 生成 merged_file.parquet                               │   │
│  └──────────────────────────────────────────────────────────┘   │
│                     │                                            │
│                     ▼                                            │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ 步骤 5: 上传到对象存储 (S3/MinIO)                          │   │
│  │  s3://bucket/files/org1/logs/stream1/2025/03/20/12/       │   │
│  │         merged_file_20250320120000.parquet                │   │
│  └──────────────────────────────────────────────────────────┘   │
│                     │                                            │
│                     ▼                                            │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ 步骤 6: 写入 file_list 表                                  │   │
│  │  INSERT INTO file_list (file_name, file_size, min_ts, ...)│   │
│  └──────────────────────────────────────────────────────────┘   │
│                     │                                            │
│                     ▼                                            │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ 步骤 7: 删除本地 Parquet 文件                              │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘

                     ║ 对象存储 + 数据库 作为共享层
                     ▼

┌─────────────────────────────────────────────────────────────────┐
│ Compactor 节点(定时任务:每 60 秒)                                  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ 步骤 1: 查询 file_list 表                                  │   │
│  │  SELECT * FROM file_list WHERE min_ts > offset            │   │
│  └──────────────────────────────────────────────────────────┘   │
│                     │                                            │
│                     ▼                                            │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ 步骤 2: 分析哪些文件需要合并                                │   │
│  │  - 小文件(<10MB)                                           │   │
│  │  - 碎片文件(同一时间段有多个文件)                           │   │
│  └──────────────────────────────────────────────────────────┘   │
│                     │                                            │
│                     ▼                                            │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ 步骤 3: 获取分布式锁                                        │   │
│  │  dist_lock::lock("/compact/merge/org1/logs/stream1")      │   │
│  └──────────────────────────────────────────────────────────┘   │
│                     │                                            │
│                     ▼                                            │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ 步骤 4: 从 S3 下载文件                                      │   │
│  │  - file1.parquet, file2.parquet, ...                      │   │
│  └──────────────────────────────────────────────────────────┘   │
│                     │                                            │
│                     ▼                                            │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ 步骤 5: Remote Merge (Compactor 的合并)                    │   │
│  │  - 合并多个小文件为大文件                                   │   │
│  │  - 生成 compacted_file.parquet                            │   │
│  └──────────────────────────────────────────────────────────┘   │
│                     │                                            │
│                     ▼                                            │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ 步骤 6: 上传合并后的文件到 S3                               │   │
│  │  s3://bucket/files/org1/logs/stream1/2025/03/20/          │   │
│  │         compacted_file_20250320100000.parquet             │   │
│  └──────────────────────────────────────────────────────────┘   │
│                     │                                            │
│                     ▼                                            │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ 步骤 7: 更新 file_list 表                                  │   │
│  │  - INSERT 新文件记录                                       │   │
│  │  - DELETE 旧文件记录                                       │   │
│  └──────────────────────────────────────────────────────────┘   │
│                     │                                            │
│                     ▼                                            │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ 步骤 8: 从 S3 删除旧文件                                    │   │
│  │  - file1.parquet, file2.parquet, ...                      │   │
│  └──────────────────────────────────────────────────────────┘   │
│                     │                                            │
│                     ▼                                            │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ 步骤 9: 释放分布式锁                                        │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘

                     ║ 对象存储 + 数据库 作为共享层
                     ▼

┌─────────────────────────────────────────────────────────────────┐
│ Querier 节点                                                      │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ 查询时从 S3 读取 Parquet 文件                               │   │
│  │  - 优先读取大文件(Compactor 合并后的文件)                   │   │
│  │  - 减少 S3 API 调用次数,提升查询性能                       │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

---

## 4. 完整的数据流转时间线

### 4.1 数据从摄入到长期存储的完整过程

假设场景:一条日志从客户端发送到 OpenObserve,最终被存储和查询的完整过程。

#### **T0 时刻:客户端发送日志**

```bash
curl -X POST http://ingester-1:5080/api/org1/logs/_json \
  -H "Content-Type: application/json" \
  -d '{"timestamp": 1711094400000, "message": "User login successful", "user_id": 12345}'
```

#### **T0 + 1ms:Ingester 接收并写入 WAL**

```rust
// Ingester 节点:ingester-1
// 1. 数据到达 HTTP Handler
// 2. 解析 JSON,验证 schema
// 3. 写入 WAL (优先级最高)
wal.write(&entry).await?;  // 写入本地磁盘
                           // 文件:files/org1/logs/access/0/2025/03/20/12/abc123/wal1.wal

// 4. 写入 MemTable
memtable.write(schema, entry, batch)?;  // 写入内存

// 5. 返回 HTTP 200 OK
```

**此时的数据状态:**
- ✅ WAL 文件:已持久化到磁盘
- ✅ MemTable:已加载到内存
- ❌ 对象存储:尚未上传
- ❌ 可查询:仅从本地 MemTable 查询(如果查询刚好命中这个 Ingester 节点)

#### **T0 + 5分钟:MemTable 达到阈值,转换为 Parquet**

```rust
// Ingester 后台任务(每 5 分钟检查一次 TTL)
// 检测到 MemTable 大小超过阈值或超时
immutable::persist().await?;

// 1. 将 MemTable 转换为 Immutable
// 2. 写入本地 Parquet 文件
//    文件:files/org1/logs/access/0/2025/03/20/12/abc123/file1.parquet

// 3. 删除对应的 WAL 文件
std::fs::remove_file(wal_file)?;
```

**此时的数据状态:**
- ❌ WAL 文件:已删除
- ✅ 本地 Parquet:已生成
- ❌ 对象存储:尚未上传

#### **T0 + 10分钟:Ingester 上传到对象存储**

```rust
// Ingester 后台任务(每 file_push_interval 秒扫描一次)
// src/job/files/parquet.rs - move_files()

// 1. 扫描 files/ 目录,发现多个 Parquet 文件
let parquet_files = wal_scan_files("files/", "parquet").await?;

// 2. 按 stream 分组,合并同一 stream 的文件
let merged_file = merge_files(thread_id, schema, &wal_dir, &files).await?;
// 生成:files/org1/logs/access/2025/03/20/12/merged_file_20250320120000.parquet

// 3. 上传到 S3
db::file_list::set(&account, &new_file_name, Some(new_file_meta), false).await?;
// S3 路径:s3://my-bucket/files/org1/logs/access/2025/03/20/12/merged_file_20250320120000.parquet

// 4. 写入 file_list 表
INSERT INTO file_list (
    org_id, stream_type, stream_name, file_name, file_size, min_ts, max_ts, records
) VALUES (
    'org1', 'logs', 'access',
    'files/org1/logs/access/2025/03/20/12/merged_file_20250320120000.parquet',
    1024000, 1711094400000, 1711095000000, 5000
);

// 5. 删除本地 Parquet 文件
remove_file(wal_dir.join(&file.key)).await?;
```

**此时的数据状态:**
- ✅ 对象存储:已上传
- ✅ file_list 表:已记录元数据
- ❌ 本地 Parquet:已删除
- ✅ 可查询:全局可查询(所有 Querier 节点都能查询)

#### **T0 + 1小时:Compactor 检测到可合并的文件**

```rust
// Compactor 节点:compactor-1
// 定时任务(每 60 秒)触发

// 1. 查询 file_list 表
SELECT * FROM file_list
WHERE org_id = 'org1'
  AND stream_type = 'logs'
  AND stream_name = 'access'
  AND min_ts > last_offset
ORDER BY min_ts ASC
LIMIT 1000;

// 假设返回 10 个小文件(每个 1-5 MB)
// file1.parquet (2 MB), file2.parquet (3 MB), ..., file10.parquet (4 MB)

// 2. 分析合并策略
let merge_jobs = analyze_merge_jobs(files)?;
// 决定将 10 个小文件合并为 2 个大文件

// 3. 创建合并任务
db::compact::create_job(merge_job).await?;
INSERT INTO file_list_jobs (
    org_id, stream_type, stream_name,
    files, status, node_uuid
) VALUES (
    'org1', 'logs', 'access',
    '["file1.parquet", "file2.parquet", ..., "file5.parquet"]',
    'Waiting', NULL
);
```

#### **T0 + 1小时 + 2秒:Compactor 执行合并任务**

```rust
// Compactor 的 run_merge 定时任务(每 62 秒)

// 1. 从任务队列拉取任务
let jobs = db::compact::pull_jobs(10).await?;
UPDATE file_list_jobs
SET status = 'Running', node_uuid = 'compactor-1-uuid'
WHERE id IN (...);

// 2. 获取分布式锁
let locker = dist_lock::lock("/compact/merge/org1/logs/access", 0).await?;

// 3. 从 S3 下载文件
for file in files {
    let data = storage::get(&file).await?;
    local_files.push(data);
}

// 4. 合并 Parquet 文件
let compacted_file = merge_parquet_files(local_files)?;
// 生成:compacted_file_20250320120000_part1.parquet (15 MB)

// 5. 上传到 S3
storage::put("files/org1/logs/access/2025/03/20/12/compacted_file_20250320120000_part1.parquet",
             compacted_file).await?;

// 6. 更新 file_list 表
INSERT INTO file_list (...) VALUES (...);  -- 新文件
DELETE FROM file_list WHERE file_name IN ('file1.parquet', ..., 'file5.parquet');  -- 旧文件

// 7. 从 S3 删除旧文件
for file in old_files {
    storage::delete(&file).await?;
}

// 8. 更新任务状态
UPDATE file_list_jobs SET status = 'Done', completed_at = NOW() WHERE id = ...;

// 9. 释放分布式锁
dist_lock::unlock(&locker).await?;
```

**此时的数据状态:**
- ✅ 对象存储:只有合并后的大文件(旧文件已删除)
- ✅ file_list 表:只记录大文件的元数据
- ✅ 可查询:查询性能提升(减少 S3 API 调用次数)

#### **T0 + 30天:Compactor 清理过期数据**

```rust
// Compactor 的 run_retention 定时任务(每 63 秒)

// 1. 查询数据保留策略
let retention_days = stream_settings.get_retention_days();  // 假设 30 天

// 2. 计算删除阈值
let threshold_ts = now() - (retention_days * 86400 * 1000000);

// 3. 查询过期文件
SELECT * FROM file_list
WHERE max_ts < threshold_ts;

// 4. 删除对象存储中的文件
for file in expired_files {
    storage::delete(&file.file_name).await?;
}

// 5. 删除 file_list 表记录
DELETE FROM file_list WHERE max_ts < threshold_ts;
```

**此时的数据状态:**
- ❌ 对象存储:过期数据已删除
- ❌ file_list 表:过期文件记录已删除
- ❌ 不可查询:数据已永久删除

### 4.2 完整时间线总结

| 时刻 | 事件 | 数据位置 | 是否可查询 | 责任组件 |
|------|------|---------|-----------|---------|
| **T0** | 客户端发送日志 | - | ❌ | - |
| **T0 + 1ms** | 写入 WAL + MemTable | WAL 文件 + 内存 | ⚠️ 仅本地 | Ingester |
| **T0 + 5分钟** | 转换为本地 Parquet | 本地 Parquet | ⚠️ 仅本地 | Ingester |
| **T0 + 10分钟** | 上传到对象存储 | S3 + file_list 表 | ✅ 全局 | Ingester |
| **T0 + 1小时** | Compactor 合并文件 | S3(大文件) | ✅ 全局(性能更好) | Compactor |
| **T0 + 30天** | Compactor 删除过期数据 | - | ❌ 已删除 | Compactor |

---

## 附录:配置参数说明

### Ingester 相关配置

```bash
# WAL 目录
ZO_DATA_WAL_DIR=/data/openobserve/wal

# MemTable 大小阈值(超过后触发 rotation)
ZO_MEM_TABLE_MAX_SIZE=256  # MB

# MemTable 最大保留时间(超过后强制转换为 Parquet)
ZO_MAX_FILE_RETENTION_TIME=300  # 秒

# 本地 Parquet 文件上传间隔
ZO_FILE_PUSH_INTERVAL=60  # 秒

# fsync 策略
ZO_WAL_FSYNC_ENABLED=true  # 是否强制 fsync(性能 vs 可靠性权衡)
```

### Compactor 相关配置

```bash
# 是否启用 Compactor
ZO_COMPACT_ENABLED=true

# 基础间隔(所有定时任务的基础周期)
ZO_COMPACT_INTERVAL=60  # 秒

# 历史数据合并间隔
ZO_COMPACT_OLD_DATA_INTERVAL=300  # 秒

# Offset 同步到数据库间隔
ZO_COMPACT_SYNC_TO_DB_INTERVAL=300  # 秒

# 任务超时时间(超过后标记为失败)
ZO_COMPACT_JOB_RUN_TIMEOUT=3600  # 秒

# 已完成任务清理等待时间
ZO_COMPACT_JOB_CLEAN_WAIT_TIME=7200  # 秒

# 合并线程数
ZO_FILE_MERGE_THREAD_NUM=4
```

### 对象存储配置

```bash
# S3 配置
ZO_S3_ENDPOINT=https://s3.us-west-1.amazonaws.com
ZO_S3_BUCKET=my-openobserve-bucket
ZO_S3_ACCESS_KEY=AKIAIOSFODNN7EXAMPLE
ZO_S3_SECRET_KEY=wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY
ZO_S3_REGION=us-west-1

# MinIO 配置示例
ZO_S3_ENDPOINT=http://minio:9000
ZO_S3_BUCKET=openobserve
ZO_S3_ACCESS_KEY=minioadmin
ZO_S3_SECRET_KEY=minioadmin
```

### 数据库配置

```bash
# MySQL 示例
ZO_META_STORE=mysql
ZO_META_MYSQL_DSN=mysql://user:password@localhost:3306/openobserve

# PostgreSQL 示例
ZO_META_STORE=postgres
ZO_META_POSTGRES_DSN=postgres://user:password@localhost:5432/openobserve

# SQLite 示例(本地开发)
ZO_META_STORE=sqlite
ZO_META_SQLITE_DSN=/data/openobserve/metadata.db
```

---

## 总结

### 核心要点回顾

1. **Ingester 崩溃数据不丢失的保障:**
   - ✅ WAL 持久化 + CRC32 校验
   - ✅ 节点重启时自动 WAL Replay
   - ✅ Lock File 保护文件转换原子性
   - ⚠️ 客户端应实现 HTTP 重试机制

2. **Compactor 工作机制:**
   - ✅ 是定时任务(11 个独立任务)
   - ✅ 在对象存储上传**之后**工作
   - ❌ 不与 Ingester 直接交互
   - ✅ 通过分布式锁 + 数据库 + 对象存储跨机器协调

3. **两者的文件合并区别:**
   - **Ingester**:合并本地 WAL 文件 → 上传到对象存储
   - **Compactor**:从对象存储下载文件 → 合并 → 重新上传

4. **数据流转完整时间线:**
   - T0 + 1ms:写入 WAL + MemTable
   - T0 + 5分钟:转换为本地 Parquet
   - T0 + 10分钟:上传到对象存储
   - T0 + 1小时:Compactor 合并优化
   - T0 + 30天:Compactor 清理过期数据

---

**文档版本:** 1.0
**最后更新:** 2025-03-20
**适用版本:** OpenObserve v0.10.x+
