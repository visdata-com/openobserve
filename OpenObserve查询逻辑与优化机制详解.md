# OpenObserve 查询逻辑与优化机制详解

## 目录

1. [查询流程概览](#1-查询流程概览)
2. [文件列表过滤(File List Pruning)](#2-文件列表过滤file-list-pruning)
3. [分区策略(Partitioning)](#3-分区策略partitioning)
4. [索引条件提取(Index Condition)](#4-索引条件提取index-condition)
5. [Parquet 文件扫描优化](#5-parquet-文件扫描优化)
6. [布隆过滤器应用](#6-布隆过滤器应用)
7. [行级过滤(Row-Level Filtering)](#7-行级过滤row-level-filtering)
8. [完整示例](#8-完整示例)

---

## 1. 查询流程概览

### 1.1 查询生命周期

```
┌─────────────────────────────────────────────────────────────────┐
│          1. HTTP/gRPC Request                                    │
│          - src/handler/http/request/search/mod.rs                │
│          - src/handler/grpc/request/search/mod.rs                │
└──────────────────────────┬──────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────────────┐
│          2. SQL 解析与重写                                        │
│          - src/service/search/sql/mod.rs                         │
│          - 提取时间范围、过滤条件、排序、聚合等                      │
│          - 注入 _timestamp 过滤                                  │
└──────────────────────────┬──────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────────────┐
│          3. 文件列表查询与过滤 (⭐ 第一层剪枝)                       │
│          - src/service/file_list.rs::query()                     │
│          - 基于时间范围过滤文件                                    │
│          - SQL: WHERE max_ts >= start AND min_ts <= end          │
└──────────────────────────┬──────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────────────┐
│          4. 分区生成 (Partition Generation)                       │
│          - src/service/search/partition.rs                       │
│          - 生成时间分区 [start, end] 数组                         │
│          - 支持 Mini Partition(快速返回)                          │
└──────────────────────────┬──────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────────────┐
│          5. 索引条件提取 (Index Condition)                        │
│          - src/service/search/index.rs                           │
│          - 提取可用于倒排索引的条件                                │
│          - 例如: field = 'value', field IN (...)                 │
└──────────────────────────┬──────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────────────┐
│          6. DataFusion 查询计划                                  │
│          - src/service/search/datafusion/exec.rs                 │
│          - 逻辑计划 → 物理计划                                    │
└──────────────────────────┬──────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────────────┐
│          7. Table Provider - 文件扫描 (⭐ 第二层剪枝)              │
│          - src/service/search/datafusion/table_provider/mod.rs   │
│          - list_files_for_scan(): 列举文件                       │
│          - 收集 Parquet 元数据统计                                │
│          - 生成 ParquetAccessPlan(行组/行级过滤)                  │
└──────────────────────────┬──────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────────────┐
│          8. Parquet 扫描执行 (⭐ 第三层剪枝)                       │
│          - DataFusion ParquetExec                                │
│          - 应用布隆过滤器(Bloom Filter)                           │
│          - 应用行组统计(Row Group Statistics)                     │
│          - 应用谓词下推(Predicate Pushdown)                       │
└──────────────────────────┬──────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────────────┐
│          9. 结果聚合与返回                                        │
│          - 多分区结果合并                                         │
│          - 应用 LIMIT、OFFSET                                    │
│          - 返回给客户端                                           │
└─────────────────────────────────────────────────────────────────┘
```

### 1.2 三层剪枝策略

| 剪枝层级 | 位置 | 基于什么剪枝 | 剪枝粒度 | 性能影响 |
|---------|------|-------------|---------|---------|
| **第一层** | 文件列表查询 | 文件级 min_ts/max_ts | 文件级 | ⭐⭐⭐⭐⭐ 最大 |
| **第二层** | Parquet 元数据 | 行组统计、布隆过滤器 | 行组级 | ⭐⭐⭐⭐ 很大 |
| **第三层** | 行级扫描 | 谓词过滤、段选择位图 | 行级 | ⭐⭐⭐ 中等 |

---

## 2. 文件列表过滤(File List Pruning)

### 2.1 核心逻辑

文件列表过滤是**第一层也是最重要的剪枝**,发生在查询执行之前。

**代码位置:** [src/service/file_list.rs:40-102](src/service/file_list.rs#L40-L102)

```rust
pub async fn query(
    trace_id: &str,
    org_id: &str,
    stream_name: &str,
    stream_type: StreamType,
    time_level: PartitionTimeLevel,
    time_min: i64,    // 查询起始时间(微秒)
    time_max: i64,    // 查询结束时间(微秒)
) -> Result<Vec<FileKey>> {
    // 1. 从 file_list 表查询(持久化的文件)
    let mut files = file_list::query(
        org_id,
        stream_type,
        stream_name,
        time_level,
        Some((time_min, time_max)),  // ← 时间范围过滤
        None,
    ).await?;

    // 2. 从 file_list_dump 查询(最近写入的文件)
    let dumped_files = file_list_dump::query(
        trace_id,
        org_id,
        stream_name,
        stream_type,
        (time_min, time_max),        // ← 时间范围过滤
        None,
    ).await?;

    // 3. 合并并去重
    files.extend(dumped_files);
    files.par_sort_unstable_by(|a, b| a.key.cmp(&b.key));
    files.dedup_by(|a, b| a.key == b.key);

    Ok(files)
}
```

---

### 2.2 时间范围过滤 SQL

**代码位置:** [src/infra/src/file_list/sqlite.rs:377-397](src/infra/src/file_list/sqlite.rs#L377-L397)

```rust
async fn query(
    &self,
    org_id: &str,
    stream_type: StreamType,
    stream_name: &str,
    _time_level: PartitionTimeLevel,
    time_range: Option<(i64, i64)>,
    flattened: Option<bool>,
) -> Result<Vec<FileKey>> {
    let stream_key = format!("{org_id}/{stream_type}/{stream_name}");
    let (time_start, time_end) = time_range.unwrap_or((0, 0));
    let max_ts_upper_bound = calculate_max_ts_upper_bound(time_end, stream_type);

    // ⭐ 核心过滤 SQL
    sqlx::query_as::<_, FileRecord>(
        r#"
        SELECT id, account, stream, date, file, deleted,
               min_ts, max_ts, records, original_size, compressed_size, index_size, flattened
        FROM file_list
        WHERE stream = $1
          AND max_ts >= $2      -- 文件最大时间 >= 查询起始时间
          AND max_ts <= $3      -- 文件最大时间 <= 上界(加缓冲)
          AND min_ts <= $4;     -- 文件最小时间 <= 查询结束时间
        "#,
    )
    .bind(stream_key)
    .bind(time_start)           // $2: 查询起始时间
    .bind(max_ts_upper_bound)   // $3: 上界(time_end + 缓冲)
    .bind(time_end)             // $4: 查询结束时间
    .fetch_all(&pool)
    .await
}
```

### 2.3 时间范围过滤原理

**关键条件:**

```sql
WHERE max_ts >= time_start  -- 文件结束时间 >= 查询开始时间
  AND min_ts <= time_end    -- 文件开始时间 <= 查询结束时间
```

**示意图:**

```
查询时间范围:    [========= time_start --------- time_end =========]

文件 A:  [====]                                            ❌ 过滤掉(max_ts < time_start)
文件 B:         [====]                                     ✅ 保留(有重叠)
文件 C:                [====]                              ✅ 保留(有重叠)
文件 D:                       [====]                       ✅ 保留(完全包含)
文件 E:                              [====]                ✅ 保留(有重叠)
文件 F:                                     [====]         ❌ 过滤掉(min_ts > time_end)
```

**过滤效果:**

- 如果查询范围是 1 小时,而数据跨度是 30 天
- 理论上可以过滤掉 **约 99.86%** 的文件 (23小时/24小时 × 29天/30天)
- 实际过滤率取决于数据分布

---

### 2.4 FileMeta 结构

**代码位置:** [src/config/src/meta/stream.rs](src/config/src/meta/stream.rs)

```rust
pub struct FileMeta {
    pub min_ts: i64,            // 文件中最小时间戳(微秒)
    pub max_ts: i64,            // 文件中最大时间戳(微秒)
    pub records: i64,           // 记录数
    pub original_size: i64,     // 原始大小(字节)
    pub compressed_size: i64,   // 压缩后大小(字节)
    pub index_size: i64,        // 索引大小(字节)
    pub flattened: bool,        // 是否已扁平化
}
```

**这些元数据在哪里生成:**

1. **写入时** - [src/ingester/src/writer.rs](src/ingester/src/writer.rs)
   - MemTable 跟踪 min_ts/max_ts
   - 上传到 S3 前计算文件元数据

2. **存储位置** - 双重存储
   - **数据库表** `file_list` (SQLite/MySQL/PostgreSQL)
   - **Parquet 文件元数据** (KeyValue metadata)

---

## 3. 分区策略(Partitioning)

### 3.1 分区生成器

**代码位置:** [src/service/search/partition.rs:22-87](src/service/search/partition.rs#L22-L87)

```rust
pub struct PartitionGenerator {
    /// 最小步长(微秒) - 通常基于直方图间隔
    min_step: i64,
    /// Mini 分区持续时间(秒)
    mini_partition_duration_secs: u64,
    /// 是否为直方图查询
    is_histogram: bool,
}

impl PartitionGenerator {
    /// 生成分区
    pub fn generate_partitions(
        &self,
        start_time: i64,        // 起始时间
        end_time: i64,          // 结束时间
        step: i64,              // 分区步长
        order_by: OrderBy,      // 排序顺序(ASC/DESC)
        is_aggregate: bool,     // 是否聚合查询
        add_mini_partition: bool, // 是否添加 mini 分区
    ) -> Vec<[i64; 2]> {
        if self.is_histogram {
            // 直方图查询 - 按间隔对齐分区
            self.generate_partitions_aligned_with_histogram_interval(...)
        } else if is_aggregate {
            // 聚合查询 - 单个分区(不分片)
            vec![[start_time, end_time]]
        } else {
            // 普通查询 - 生成带 mini 分区的分区列表
            self.generate_partitions_with_mini_partition(...)
        }
    }
}
```

### 3.2 Mini Partition 机制

**目的:** 快速返回部分结果,提升用户体验

**原理:**

```
查询范围: 2025-01-29 00:00:00 → 2025-01-29 23:59:59 (24小时)
ORDER BY _timestamp DESC LIMIT 100

传统分区:
  Part 1: [00:00 - 23:59]  ← 扫描整天数据

优化后(Mini Partition):
  Part 0: [23:00 - 23:59]  ← ⭐ Mini 分区(1小时,优先扫描)
  Part 1: [22:00 - 23:00]
  Part 2: [21:00 - 22:00]
  ...
  Part 23: [00:00 - 01:00]
```

**优势:**

- **快速首屏** - Mini 分区通常能满足 LIMIT 要求,无需扫描全部数据
- **降低延迟** - 减少 99% 的数据扫描量(如果 LIMIT 较小)
- **提升体验** - 用户更快看到结果

**代码位置:** [src/service/search/partition.rs:98-148](src/service/search/partition.rs#L98-L148)

---

### 3.3 分区示例

**场景:** 查询最近 1 小时日志,按时间降序,取前 100 条

```rust
// 查询参数
start_time = now - 1_hour = 1738166400000000  // 2025-01-29 15:00:00
end_time   = now            = 1738170000000000  // 2025-01-29 16:00:00
order_by   = DESC
limit      = 100

// 生成分区
partitions = [
    [1738169400000000, 1738170000000000],  // Part 0: 15:50 - 16:00 (Mini, 10分钟)
    [1738168800000000, 1738169400000000],  // Part 1: 15:40 - 15:50
    [1738168200000000, 1738168800000000],  // Part 2: 15:30 - 15:40
    [1738167600000000, 1738168200000000],  // Part 3: 15:20 - 15:30
    [1738167000000000, 1738167600000000],  // Part 4: 15:10 - 15:20
    [1738166400000000, 1738167000000000],  // Part 5: 15:00 - 15:10
]

// 执行顺序: Part 0 → Part 1 → Part 2 → ...
// 如果 Part 0 返回了 100 条记录,后续分区可能不执行(早停)
```

---

## 4. 索引条件提取(Index Condition)

### 4.1 什么是索引条件

**索引条件**是从 SQL WHERE 子句中提取出来的、可以利用**倒排索引(Inverted Index)**加速的条件。

**支持的条件类型:**

| SQL 条件 | 索引类型 | 示例 |
|---------|---------|------|
| `field = 'value'` | Term Query | `level = 'ERROR'` |
| `field IN ('a', 'b')` | Terms Query | `status IN ('200', '404')` |
| `field LIKE 'prefix%'` | Prefix Query | `url LIKE '/api/%'` |
| `field ~ 'regex'` | Regex Query | `message ~ '.*error.*'` |
| `match_all('search term')` | Full-text | `match_all('login failed')` |
| `str_match(field, 'val')` | Term Query | `str_match(user_id, '12345')` |

**代码位置:** [src/service/search/index.rs:65-88](src/service/search/index.rs#L65-L88)

```rust
pub fn get_index_condition_from_expr(
    index_fields: &HashSet<String>,
    expr: &Expr,
) -> (Option<IndexCondition>, Option<Expr>) {
    let mut other_expr = Vec::new();
    let expr_list = split_conjunction(expr);  // 拆分 AND 连接的条件
    let mut index_condition = IndexCondition::default();

    for e in expr_list {
        if !is_expr_valid_for_index(e, index_fields) {
            other_expr.push(e);  // 不支持索引的条件保留
            continue;
        }

        // 提取为索引条件
        let multi_condition = Condition::from_expr(e);
        index_condition.add_condition(multi_condition);
    }

    let new_expr = conjunction(other_expr);
    if index_condition.is_empty() {
        (None, new_expr)  // 没有可用索引
    } else {
        (Some(index_condition), new_expr)  // 返回索引条件 + 剩余条件
    }
}
```

---

### 4.2 IndexCondition 结构

**代码位置:** [src/service/search/index.rs:91-194](src/service/search/index.rs#L91-L194)

```rust
#[derive(Default, Clone, Hash, Eq, PartialEq)]
pub struct IndexCondition {
    pub conditions: Vec<Condition>,  // AND 连接的条件列表
}

impl IndexCondition {
    /// 转换为 Tantivy 查询
    pub fn to_tantivy_query(
        &self,
        schema: Schema,
        default_field: Option<Field>,
    ) -> anyhow::Result<Box<dyn Query>> {
        let queries = self
            .conditions
            .iter()
            .map(|condition| {
                condition
                    .to_tantivy_query(&schema, default_field)
                    .map(|condition| (Occur::Must, condition))  // 所有条件 MUST 满足
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(Box::new(BooleanQuery::from(queries)))
    }

    /// 获取需要的字段(用于投影)
    pub fn get_schema_fields(&self, fst_fields: &[String]) -> HashSet<String> {
        self.conditions
            .iter()
            .fold(HashSet::new(), |mut acc, condition| {
                acc.extend(condition.get_schema_fields(fst_fields));
                acc
            })
    }

    /// 转换为物理表达式(用于数据验证)
    pub fn to_physical_expr(
        &self,
        schema: &arrow_schema::Schema,
        fst_fields: &[String],
    ) -> Result<Arc<dyn PhysicalExpr>, anyhow::Error> {
        // 将所有条件转换为物理表达式并用 AND 连接
        Ok(conjunction(
            self.conditions
                .iter()
                .map(|condition| condition.to_physical_expr(schema, fst_fields))
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }
}
```

---

### 4.3 索引条件示例

**SQL 查询:**

```sql
SELECT * FROM logs
WHERE level = 'ERROR'
  AND status IN ('500', '503')
  AND message LIKE '%timeout%'
  AND timestamp >= 1738166400000000
  AND timestamp <= 1738170000000000
```

**提取的索引条件:**

```rust
IndexCondition {
    conditions: [
        Condition::Eq { field: "level", value: "ERROR" },
        Condition::In { field: "status", values: ["500", "503"] },
        Condition::Like { field: "message", pattern: "%timeout%" },
    ]
}
```

**剩余条件(不使用索引):**

```sql
timestamp >= 1738166400000000 AND timestamp <= 1738170000000000
```

**查询优化流程:**

1. **倒排索引扫描** - 找到满足 `level='ERROR' AND status IN (...) AND message LIKE ...` 的文件/行组
2. **文件剪枝** - 结合时间戳条件过滤文件
3. **Parquet 扫描** - 只扫描通过索引筛选的文件
4. **数据验证** - 读取数据后再次应用所有条件(确保正确性)

---

## 5. Parquet 文件扫描优化

### 5.1 list_files_for_scan 方法

**代码位置:** [src/service/search/datafusion/table_provider/mod.rs:158-221](src/service/search/datafusion/table_provider/mod.rs#L158-L221)

```rust
async fn list_files_for_scan<'a>(
    &'a self,
    ctx: &'a SessionState,
    limit: Option<usize>,
) -> Result<(Vec<FileGroup>, Statistics)> {
    let store = ctx.runtime_env().object_store(url)?;

    // 1. 列举文件
    let file_list = future::try_join_all(
        self.table_paths
            .iter()
            .map(|table_path| list_files(store.as_ref(), table_path)),
    ).await?;

    // 2. 收集统计信息(并行)
    let files = file_list
        .map(|part_file| async {
            let part_file = part_file?;
            let statistics = if self.options.collect_stat {
                // ⭐ 读取 Parquet 元数据
                self.do_collect_statistics(ctx, &store, &part_file).await?
            } else {
                Arc::new(Statistics::new_unknown(&self.file_schema))
            };
            Ok(part_file.with_statistics(statistics))
        })
        .buffer_unordered(ctx.config_options().execution.meta_fetch_concurrency);

    // 3. 应用 LIMIT 提前停止
    let (file_group, inexact_stats) =
        get_files_with_limit(files, limit, self.options.collect_stat).await?;

    // 4. 生成访问计划(ParquetAccessPlan)
    let semaphore = Arc::new(Semaphore::new(get_config().limit.cpu_num));
    let mut tasks = Vec::with_capacity(file_group.files().len());

    for mut file in file_group.into_inner().into_iter() {
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let task = tokio::task::spawn(async move {
            // ⭐ 生成行组/行级访问计划
            let access_plan = generate_access_plan(&file);
            if let Some(access_plan) = access_plan {
                file.extensions = Some(access_plan as _);
            }
            drop(permit);
            file
        });
        tasks.push(task);
    }

    let files = try_join_all(tasks).await?;
    Ok((vec![FileGroup::new(files)], statistics))
}
```

---

### 5.2 Parquet 统计信息收集

**代码位置:** [src/service/search/datafusion/table_provider/mod.rs:228-266](src/service/search/datafusion/table_provider/mod.rs#L228-L266)

```rust
async fn do_collect_statistics(
    &self,
    ctx: &SessionState,
    store: &Arc<dyn ObjectStore>,
    part_file: &PartitionedFile,
) -> Result<Arc<Statistics>> {
    // 1. 检查缓存
    match self.collected_statistics.get(part_file.path().as_ref()) {
        Some(statistics) => Ok(statistics),
        None => {
            // 2. 从 Parquet 文件推断统计信息
            let statistics = self
                .options
                .format
                .as_ref()
                .unwrap()
                .infer_stats(
                    ctx,
                    store,
                    self.file_schema.clone(),
                    part_file.object_meta(),
                )
                .await?;

            // 3. 缓存统计信息
            let statistics = Arc::new(statistics);
            self.collected_statistics.put_with_extra(
                part_file.path().as_ref().to_string(),
                Arc::clone(&statistics),
                part_file.range.clone(),
            );
            Ok(statistics)
        }
    }
}
```

**收集的统计信息包括:**

1. **行数** (num_rows)
2. **每列的最小/最大值** (column statistics)
3. **NULL 值数量**
4. **不同值数量** (distinct count)

**这些统计信息用于:**

- **谓词下推** - 如果列的最大值 < 查询条件,跳过整个行组
- **投影下推** - 只读取需要的列
- **聚合优化** - 使用统计信息估算结果

---

### 5.3 ParquetAccessPlan 生成

**代码位置:** [src/service/search/datafusion/table_provider/helpers.rs:152-199](src/service/search/datafusion/table_provider/helpers.rs#L152-L199)

```rust
pub fn generate_access_plan(file: &PartitionedFile) -> Option<Arc<ParquetAccessPlan>> {
    // 1. 获取段选择位图(来自倒排索引)
    let row_ids = storage::file_list::get_segment_ids(file.path().as_ref())?;

    // 2. 获取文件总行数
    let stats = file.statistics.as_ref()?;
    let Precision::Exact(num_rows) = stats.num_rows else {
        return None;
    };

    // 3. 计算行组数量
    let row_group_count = num_rows.div_ceil(PARQUET_MAX_ROW_GROUP_SIZE);
    let mut access_plan = ParquetAccessPlan::new_none(row_group_count);

    // 4. 为每个行组生成行选择器(RowSelector)
    for (row_group_id, chunk) in row_ids.chunks(PARQUET_MAX_ROW_GROUP_SIZE).enumerate() {
        let mut selection = Vec::new();
        let mut current_count = 0;
        let mut current_select = false;

        // 将连续的 true/false 压缩为 RowSelector
        for val in chunk.iter() {
            if *val == current_select {
                current_count += 1;
            } else {
                if current_count > 0 {
                    if current_select {
                        selection.push(RowSelector::select(current_count));
                    } else {
                        selection.push(RowSelector::skip(current_count));
                    }
                }
                current_select = *val;
                current_count = 1;
            }
        }

        // 处理最后一批
        if current_count > 0 {
            if current_select {
                selection.push(RowSelector::select(current_count));
            } else {
                selection.push(RowSelector::skip(current_count));
            }
        }

        // 如果行组有任何需要读取的行,标记为扫描
        if selection.iter().any(|s| !s.skip) {
            access_plan.scan(row_group_id);
            access_plan.scan_selection(row_group_id, RowSelection::from(selection));
        }
    }

    Some(Arc::new(access_plan))
}
```

**ParquetAccessPlan 作用:**

- **跳过整个行组** - 如果行组不包含任何匹配行
- **行级跳过** - 在行组内跳过不匹配的行
- **性能提升** - 避免解压和解码不需要的数据

**示例:**

```
文件: logs_2025_01_29.parquet (10,000,000 行)
索引结果: 只有第 1000-2000 行匹配

行组划分 (每行组 1,048,576 行):
  Row Group 0: [0 - 1,048,575]      ← 包含匹配行,扫描
    RowSelector::skip(999)          ← 跳过前 999 行
    RowSelector::select(1001)       ← 读取 1000-2000 行
    RowSelector::skip(1,047,575)    ← 跳过后续行
  Row Group 1: [1,048,576 - ...]    ← 不包含匹配行,完全跳过
  ...

性能提升: 只解码 0.01% 的数据(1001 行 / 10,000,000 行)
```

---

## 6. 布隆过滤器应用

### 6.1 布隆过滤器配置

**OpenObserve 使用 Parquet 内置的布隆过滤器**,而非传统的独立布隆过滤器。

**代码位置:** [src/config/src/utils/parquet.rs:37-95](src/config/src/utils/parquet.rs#L37-L95)

```rust
pub fn new_parquet_writer<'a>(
    buf: &'a mut Vec<u8>,
    schema: &'a Arc<Schema>,
    bloom_filter_fields: &'a [String],  // ⭐ 需要布隆过滤器的字段
    metadata: &'a FileMeta,
    write_metadata: bool,
    compression: Option<&str>,
) -> AsyncArrowWriter<&'a mut Vec<u8>> {
    let cfg = get_config();
    let mut writer_props = WriterProperties::builder()
        .set_write_batch_size(PARQUET_BATCH_SIZE)
        .set_max_row_group_size(PARQUET_MAX_ROW_GROUP_SIZE)
        .set_compression(get_parquet_compression(compression));

    // ⭐ 布隆过滤器配置
    // NDV (Number of Distinct Values) 用于控制布隆过滤器大小
    let mut bf_ndv = min(metadata.records as u64, PARQUET_MAX_ROW_GROUP_SIZE as u64);
    if bf_ndv > 1000 {
        // 降低 NDV 以减少内存占用
        bf_ndv = max(1000, bf_ndv / cfg.common.bloom_filter_ndv_ratio);
    }

    if cfg.common.bloom_filter_enabled {
        // 合并用户指定字段 + 默认字段
        let mut fields = bloom_filter_fields.to_vec();
        fields.extend(BLOOM_FILTER_DEFAULT_FIELDS.clone());  // [_timestamp, ...]
        fields.sort();
        fields.dedup();

        for field in fields {
            writer_props = writer_props
                .set_column_bloom_filter_enabled(field.as_str().into(), true)
                .set_column_bloom_filter_fpp(field.as_str().into(), DEFAULT_BLOOM_FILTER_FPP)  // 0.05
                .set_column_bloom_filter_ndv(field.into(), bf_ndv);
        }
    }

    let writer_props = writer_props.build();
    AsyncArrowWriter::try_new(buf, schema.clone(), Some(writer_props)).unwrap()
}
```

**关键参数:**

| 参数 | 默认值 | 说明 |
|------|--------|------|
| `bloom_filter_enabled` | `true` | 是否启用布隆过滤器 |
| `bloom_filter_ndv_ratio` | `10` | NDV 缩减比例 |
| `DEFAULT_BLOOM_FILTER_FPP` | `0.05` | 假阳性率(5%) |
| `BLOOM_FILTER_DEFAULT_FIELDS` | `[_timestamp]` | 默认启用的字段 |

---

### 6.2 布隆过滤器字段配置

**通过流设置(Stream Settings)配置:**

```json
{
  "stream_name": "logs",
  "bloom_filter_fields": ["user_id", "trace_id", "request_id"]
}
```

**代码位置:** [src/service/stream.rs:641-649](src/service/stream.rs#L641-L649)

```rust
// 添加布隆过滤器字段
if !new_settings.bloom_filter_fields.add.is_empty() {
    existing_settings
        .bloom_filter_fields
        .extend(new_settings.bloom_filter_fields.add);
}

// 移除布隆过滤器字段
if !new_settings.bloom_filter_fields.remove.is_empty() {
    existing_settings
        .bloom_filter_fields
        .retain(|field| !new_settings.bloom_filter_fields.remove.contains(field));
}
```

---

### 6.3 布隆过滤器读取配置

**代码位置:** [src/service/search/datafusion/exec.rs:412-416](src/service/search/datafusion/exec.rs#L412-L416)

```rust
// DataFusion SessionConfig 配置
let mut config = SessionConfig::new()
    .with_batch_size(get_config().limit.datafusion_batch_size);

// 启用布隆过滤器读取
if cfg.common.bloom_filter_enabled {
    config = config.set_bool("datafusion.execution.parquet.bloom_filter_on_read", true);
}

// 禁用布隆过滤器(用于调试)
if cfg.common.bloom_filter_disabled_on_search {
    config = config.set_bool("datafusion.execution.parquet.bloom_filter_on_read", false);
}
```

**环境变量:**

```bash
# 启用布隆过滤器(默认)
ZO_BLOOM_FILTER_ENABLED=true

# 禁用搜索时的布隆过滤器
ZO_BLOOM_FILTER_DISABLED_ON_SEARCH=true

# NDV 缩减比例
ZO_BLOOM_FILTER_NDV_RATIO=10
```

---

### 6.4 布隆过滤器工作原理

**1. 写入阶段(Ingester):**

```
数据写入 → Parquet Writer
               ↓
  为每个 Row Group 的指定列创建布隆过滤器
               ↓
  存储在 Parquet 文件 ColumnMetaData 中
```

**2. 查询阶段(Querier):**

```
查询条件: user_id = '12345'
               ↓
  读取 Parquet 文件元数据
               ↓
  检查 Row Group 0 的 user_id 布隆过滤器
    - bloom_filter.contains('12345') → true
    - 扫描 Row Group 0
               ↓
  检查 Row Group 1 的 user_id 布隆过滤器
    - bloom_filter.contains('12345') → false  ← ⭐ 跳过!
    - 不扫描 Row Group 1
```

**性能提升:**

- **精确匹配查询** - 可跳过 90%+ 的行组
- **IN 查询** - 对每个值检查布隆过滤器
- **高基数字段** - 如 user_id, trace_id, request_id 效果最佳

**注意:**

- **假阳性** - 5% 概率误报(FPP=0.05),不会漏数据
- **假阴性** - 0% 概率,不会漏数据
- **内存占用** - 每个行组每个字段约 10-50 KB

---

### 6.5 布隆过滤器 vs 倒排索引

| 特性 | 布隆过滤器 | 倒排索引 |
|------|-----------|---------|
| **存储位置** | Parquet 文件内 | 独立索引文件(.idx) |
| **粒度** | 行组级 | 行级 |
| **查询类型** | 精确匹配、IN | 精确匹配、全文搜索、正则 |
| **准确性** | 概率性(5% 误报) | 精确 |
| **内存占用** | 小(10-50 KB/列/行组) | 大(取决于基数) |
| **写入开销** | 低 | 中 |
| **查询速度** | 快(行组剪枝) | 非常快(直接定位) |
| **适用场景** | 高基数字段过滤 | 全文搜索、复杂查询 |

**最佳实践:**

- **组合使用** - 倒排索引找文件 → 布隆过滤器过滤行组 → 谓词过滤行
- **布隆过滤器字段选择** - 高基数、经常过滤的字段(user_id, trace_id)
- **倒排索引字段选择** - 需要全文搜索或正则的字段(message, log)

---

## 7. 行级过滤(Row-Level Filtering)

### 7.1 段选择位图(Segment Selection Bitmap)

**概念:** 使用位图(BitVec)标记哪些行需要读取,哪些行跳过。

**数据流:**

```
倒排索引查询 (Tantivy)
       ↓
返回匹配的文档 ID 列表
       ↓
转换为位图 (Vec<bool>)
  [true, true, false, false, true, ...]
       ↓
存储在内存缓存
       ↓
generate_access_plan() 读取
       ↓
生成 ParquetAccessPlan
       ↓
ParquetExec 应用行级跳过
```

**代码位置:** [src/service/search/datafusion/storage/file_list.rs](src/service/search/datafusion/storage/file_list.rs)

```rust
pub fn get_segment_ids(file_key: &str) -> Option<Vec<bool>> {
    // 从缓存获取该文件的段选择位图
    let cache = FILE_LIST_CACHE.read();
    cache.get(file_key).map(|entry| entry.segment_ids.clone())
}

pub fn set_segment_ids(file_key: &str, segment_ids: Vec<bool>) {
    let mut cache = FILE_LIST_CACHE.write();
    cache.insert(
        file_key.to_string(),
        CacheEntry {
            segment_ids,
            created_at: now_micros(),
        },
    );
}
```

---

### 7.2 RowSelector 压缩

**问题:** 如果文件有 100 万行,位图就需要 100 万个布尔值,占用约 1 MB 内存。

**解决方案:** 使用 `RowSelector` 压缩连续的 true/false 段。

**示例:**

```rust
// 原始位图 (1,000,000 个布尔值)
[false, false, false, ...(999次)..., false, true, true, ...(1000次)..., true, false, ...]

// 压缩为 RowSelector
[
    RowSelector::skip(1000),      // 跳过前 1000 行
    RowSelector::select(1000),    // 读取接下来 1000 行
    RowSelector::skip(998000),    // 跳过后续 998000 行
]

// 内存占用: 1 MB → ~24 字节(3 个 RowSelector)
```

**代码位置:** [src/service/search/datafusion/table_provider/helpers.rs:164-192](src/service/search/datafusion/table_provider/helpers.rs#L164-L192)

---

### 7.3 行级过滤示例

**查询:**

```sql
SELECT * FROM logs
WHERE level = 'ERROR'
  AND user_id = '12345'
  AND timestamp >= 1738166400000000
  AND timestamp <= 1738170000000000
```

**优化过程:**

1. **倒排索引查询** (level='ERROR' AND user_id='12345')
   ```
   返回文件列表:
   - file_1.parquet: 匹配行 [100, 150, 200, 500-600]
   - file_2.parquet: 匹配行 [1000-1100, 2000]
   ```

2. **生成段选择位图**
   ```
   file_1.parquet:
   [false×100, true, false×49, true, false×49, true, false×299, true×101, false×...]

   压缩为:
   [skip(100), select(1), skip(49), select(1), skip(49), select(1), skip(299), select(101), ...]
   ```

3. **Parquet 扫描**
   ```
   Row Group 0: [0-1,048,575]
     - 应用 RowSelector
     - 只解码行 [100, 150, 200, 500-600]
     - 跳过其他 1,048,475 行
   ```

4. **谓词过滤**
   ```
   在解码的 102 行中应用时间戳过滤
   最终返回 85 行
   ```

**性能提升:**

- 原始扫描: 1,048,576 行
- 优化后扫描: 102 行
- **减少 99.99%** 的数据解码

---

## 8. 完整示例

### 8.1 查询场景

**业务需求:** 查询最近 1 小时内,ERROR 级别的日志,涉及用户 12345,按时间倒序,取前 100 条。

**SQL:**

```sql
SELECT _timestamp, level, user_id, message
FROM logs
WHERE _timestamp >= 1738166400000000
  AND _timestamp <= 1738170000000000
  AND level = 'ERROR'
  AND user_id = '12345'
ORDER BY _timestamp DESC
LIMIT 100
```

---

### 8.2 执行计划分析

#### 阶段 1: 文件列表查询

**输入:**

```rust
org_id = "default"
stream_name = "logs"
stream_type = Logs
time_min = 1738166400000000  // 2025-01-29 15:00:00
time_max = 1738170000000000  // 2025-01-29 16:00:00
```

**SQL 查询:**

```sql
SELECT id, account, stream, date, file, min_ts, max_ts, records, ...
FROM file_list
WHERE stream = 'default/logs/logs'
  AND max_ts >= 1738166400000000
  AND min_ts <= 1738170000000000;
```

**结果:**

```
假设数据库中有 30 天的数据,每小时生成 10 个文件:
- 总文件数: 30 × 24 × 10 = 7,200 个文件
- 过滤后: 1 小时 × 10 = 10 个文件
- 过滤率: 99.86% (7,190 / 7,200)
```

---

#### 阶段 2: 分区生成

**输入:**

```rust
start_time = 1738166400000000
end_time   = 1738170000000000
order_by   = DESC
limit      = 100
is_histogram = false
is_aggregate = false
```

**生成分区:**

```rust
vec![
    [1738169400000000, 1738170000000000],  // Part 0: 15:50-16:00 (Mini, 10min)
    [1738168800000000, 1738169400000000],  // Part 1: 15:40-15:50
    [1738168200000000, 1738168800000000],  // Part 2: 15:30-15:40
    [1738167600000000, 1738168200000000],  // Part 3: 15:20-15:30
    [1738167000000000, 1738167600000000],  // Part 4: 15:10-15:20
    [1738166400000000, 1738167000000000],  // Part 5: 15:00-15:10
]
```

---

#### 阶段 3: 索引条件提取

**输入:**

```sql
WHERE level = 'ERROR' AND user_id = '12345'
```

**提取结果:**

```rust
index_condition = Some(IndexCondition {
    conditions: [
        Condition::Eq { field: "level", value: "ERROR" },
        Condition::Eq { field: "user_id", value: "12345" },
    ]
})

other_expr = Some(
    _timestamp >= 1738166400000000 AND _timestamp <= 1738170000000000
)
```

---

#### 阶段 4: 倒排索引查询

**Tantivy 查询:**

```rust
BooleanQuery {
    clauses: [
        (Occur::Must, TermQuery { term: "level:ERROR" }),
        (Occur::Must, TermQuery { term: "user_id:12345" }),
    ]
}
```

**索引扫描结果:**

```
索引返回匹配的文件和行 ID:
- file_1.parquet (15:52-15:58): 行 [1000, 1050, 2000-2100, 5000]
- file_2.parquet (15:45-15:51): 行 [500-550, 3000]
- file_3.parquet (15:38-15:44): 无匹配
- ...
```

**段选择位图生成:**

```rust
// file_1.parquet (假设 100,000 行)
segment_ids = vec![
    false×1000, true, false×49, true, false×899, true×101, false×2899, true, false×...
]

// 压缩为 RowSelector
access_plan = ParquetAccessPlan {
    row_group_0: RowSelection {
        selectors: [
            skip(1000), select(1), skip(49), select(1), skip(899), select(101), skip(2899), select(1), ...
        ]
    }
}
```

---

#### 阶段 5: Parquet 文件扫描

**文件:** `file_1.parquet` (15:52-15:58, 100,000 rows, 1 row group)

**步骤 1: 检查文件级元数据**

```rust
FileMeta {
    min_ts: 1738169520000000,  // 15:52:00
    max_ts: 1738169880000000,  // 15:58:00
    records: 100000,
    ...
}

// 时间范围检查
if max_ts >= time_min && min_ts <= time_max {
    // ✅ 通过,继续扫描
}
```

**步骤 2: 应用布隆过滤器**

```rust
// Row Group 0 的 user_id 列布隆过滤器
bloom_filter.contains("12345") → true  // ✅ 可能包含,继续扫描

// 如果返回 false,则跳过整个行组
```

**步骤 3: 应用行组统计**

```rust
// Row Group 0 的 _timestamp 列统计
ColumnStatistics {
    min: 1738169520000000,
    max: 1738169880000000,
}

// 范围检查
if max >= time_min && min <= time_max {
    // ✅ 通过,继续扫描
}
```

**步骤 4: 应用 ParquetAccessPlan**

```rust
// 只解码匹配的行
rows_to_decode = [1000, 1050, 2000-2100, 5000]
total_rows_decoded = 104  // 而非 100,000
```

**步骤 5: 谓词过滤**

```rust
// 在解码的 104 行中应用完整谓词
filtered_rows = rows.filter(|row| {
    row.timestamp >= time_min
    && row.timestamp <= time_max
    && row.level == "ERROR"
    && row.user_id == "12345"
});

// 最终返回 98 行(6 行时间戳不在范围内)
```

---

#### 阶段 6: 结果聚合

**Part 0 (15:50-16:00) 返回 98 行**

```
已满足 LIMIT 100,提前终止后续分区扫描
```

**最终结果:**

```json
{
  "took": 45,  // 毫秒
  "hits": {
    "total": 98,
    "records": [
      {
        "_timestamp": 1738169850000000,
        "level": "ERROR",
        "user_id": "12345",
        "message": "Connection timeout"
      },
      // ... 97 more
    ]
  },
  "scan_stats": {
    "files_scanned": 2,        // 只扫描了 2 个文件(而非 10 个)
    "rows_scanned": 104,       // 只扫描了 104 行
    "rows_returned": 98,
    "bytes_scanned": 8192      // 只读取了 8 KB(而非 100 MB)
  }
}
```

---

### 8.3 性能对比

| 指标 | 无优化 | 有优化 | 提升 |
|------|--------|--------|------|
| **文件扫描** | 7,200 个 | 2 个 | 99.97% |
| **数据读取** | 7.2 GB | 8 KB | 99.9999% |
| **行扫描** | 720,000,000 | 104 | 99.99998% |
| **查询时间** | ~30 秒 | ~45 毫秒 | **666× 加速** |

---

### 8.4 优化总结

**第一层 - 文件列表过滤 (99.86% 减少)**

```
7,200 files → 10 files
```

**第二层 - 索引条件过滤 (80% 减少)**

```
10 files → 2 files (8 个文件不包含 level='ERROR' AND user_id='12345')
```

**第三层 - 布隆过滤器 (行组剪枝)**

```
2 files × 1 row group = 2 row groups
布隆过滤器检查通过,继续扫描
```

**第四层 - 行级过滤 (99.9% 减少)**

```
200,000 rows → 104 rows (ParquetAccessPlan)
```

**第五层 - 谓词过滤 (6% 减少)**

```
104 rows → 98 rows (时间戳精确过滤)
```

---

## 9. 配置参数总结

### 9.1 布隆过滤器配置

| 环境变量 | 默认值 | 说明 |
|---------|--------|------|
| `ZO_BLOOM_FILTER_ENABLED` | `true` | 启用布隆过滤器 |
| `ZO_BLOOM_FILTER_DISABLED_ON_SEARCH` | `false` | 搜索时禁用布隆过滤器(调试) |
| `ZO_BLOOM_FILTER_NDV_RATIO` | `10` | NDV 缩减比例 |

**代码位置:**

- 写入: [src/config/src/utils/parquet.rs:74-92](src/config/src/utils/parquet.rs#L74-L92)
- 读取: [src/service/search/datafusion/exec.rs:412-416](src/service/search/datafusion/exec.rs#L412-L416)

---

### 9.2 分区配置

| 环境变量 | 默认值 | 说明 |
|---------|--------|------|
| `ZO_MINI_PARTITION_DURATION_SECS` | `600` | Mini 分区持续时间(秒) |
| `ZO_QUERY_PARTITION_STRATEGY` | `auto` | 分区策略(auto/histogram/fixed) |

**代码位置:**

- [src/service/search/partition.rs:22-87](src/service/search/partition.rs#L22-L87)

---

### 9.3 文件列表配置

| 环境变量 | 默认值 | 说明 |
|---------|--------|------|
| `ZO_FILE_LIST_DUMP_ENABLED` | `true` | 启用文件列表转储 |
| `ZO_FILE_LIST_DUMP_DUAL_WRITE` | `false` | 双写模式 |
| `ZO_FILE_LIST_ID_BATCH_SIZE` | `100` | ID 批次大小 |

**代码位置:**

- [src/service/file_list.rs:40-102](src/service/file_list.rs#L40-L102)
- [src/infra/src/file_list/sqlite.rs:345-397](src/infra/src/file_list/sqlite.rs#L345-L397)

---

### 9.4 DataFusion 配置

| 环境变量 | 默认值 | 说明 |
|---------|--------|------|
| `ZO_DATAFUSION_BATCH_SIZE` | `8192` | 批次大小 |
| `ZO_PARQUET_MAX_ROW_GROUP_SIZE` | `1048576` | 最大行组大小 |
| `ZO_META_FETCH_CONCURRENCY` | `16` | 元数据并发数 |

**代码位置:**

- [src/service/search/datafusion/exec.rs:410-450](src/service/search/datafusion/exec.rs#L410-L450)

---

## 10. 最佳实践

### 10.1 字段选择

**布隆过滤器字段:**

- ✅ **高基数字段** - user_id, trace_id, request_id, session_id
- ✅ **经常等值查询** - status_code, error_code
- ❌ **低基数字段** - level (只有 DEBUG/INFO/WARN/ERROR)
- ❌ **范围查询字段** - timestamp, duration

**倒排索引字段:**

- ✅ **全文搜索字段** - message, log, error_message
- ✅ **精确匹配字段** - user_id, trace_id
- ✅ **正则查询字段** - url, path

---

### 10.2 查询优化技巧

**1. 缩小时间范围**

```sql
-- ❌ 差
SELECT * FROM logs WHERE level = 'ERROR';

-- ✅ 好
SELECT * FROM logs
WHERE _timestamp >= now() - INTERVAL 1 HOUR
  AND level = 'ERROR';
```

**2. 利用索引字段**

```sql
-- ❌ 差 (无法使用索引)
SELECT * FROM logs WHERE message LIKE '%error%';

-- ✅ 好 (使用全文索引)
SELECT * FROM logs WHERE match_all('error');
```

**3. 使用 LIMIT**

```sql
-- ❌ 差
SELECT * FROM logs ORDER BY _timestamp DESC;

-- ✅ 好
SELECT * FROM logs ORDER BY _timestamp DESC LIMIT 100;
```

**4. 避免 SELECT ***

```sql
-- ❌ 差
SELECT * FROM logs WHERE user_id = '12345';

-- ✅ 好
SELECT _timestamp, level, message
FROM logs
WHERE user_id = '12345';
```

---

### 10.3 监控指标

**关键指标:**

- `file_list_cache_hit_count` - 文件列表缓存命中数
- `file_list_id_select_count` - ID 查询数量
- `scan_stats.files_scanned` - 扫描文件数
- `scan_stats.rows_scanned` - 扫描行数
- `scan_stats.bytes_scanned` - 扫描字节数

**优化目标:**

- 文件扫描比例 < 1% (扫描文件 / 总文件)
- 行扫描比例 < 0.1% (扫描行 / 总行)
- 查询延迟 < 100ms (P95)

---

## 11. 参考资料

### 11.1 相关代码文件

| 文件 | 说明 |
|------|------|
| [src/service/file_list.rs](src/service/file_list.rs) | 文件列表查询服务 |
| [src/infra/src/file_list/sqlite.rs](src/infra/src/file_list/sqlite.rs) | SQLite 文件列表实现 |
| [src/service/search/partition.rs](src/service/search/partition.rs) | 分区生成器 |
| [src/service/search/index.rs](src/service/search/index.rs) | 索引条件提取 |
| [src/service/search/datafusion/table_provider/mod.rs](src/service/search/datafusion/table_provider/mod.rs) | Table Provider |
| [src/service/search/datafusion/table_provider/helpers.rs](src/service/search/datafusion/table_provider/helpers.rs) | ParquetAccessPlan 生成 |
| [src/config/src/utils/parquet.rs](src/config/src/utils/parquet.rs) | Parquet 布隆过滤器配置 |

### 11.2 相关技术

- **Apache Parquet** - 列式存储格式,支持布隆过滤器和统计信息
- **Apache DataFusion** - Rust 实现的查询引擎,支持谓词下推和投影下推
- **Tantivy** - Rust 实现的全文搜索引擎,类似 Lucene
- **Bloom Filter** - 概率数据结构,用于快速判断元素是否存在

---

**最后更新:** 2025-01-29
**版本:** 1.0
**作者:** Claude + OpenObserve Codebase Analysis
