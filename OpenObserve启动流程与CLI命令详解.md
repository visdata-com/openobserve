# OpenObserve 启动流程与 CLI 命令详解

## 目录

1. [Main 方法启动流程](#main-方法启动流程)
2. [三运行时架构](#三运行时架构)
3. [启动序列详解](#启动序列详解)
4. [CLI 命令完整参考](#cli-命令完整参考)
5. [常用操作场景](#常用操作场景)
6. [配置文件与环境变量](#配置文件与环境变量)

---

## Main 方法启动流程

### 概览

OpenObserve 使用 Tokio 异步运行时，采用**三运行时架构**设计：

```
┌─────────────────────────────────────────────────────────┐
│                    Main 方法入口                         │
│                  #[tokio::main]                         │
└─────────────────────────────────────────────────────────┘
                            │
                            ▼
        ┌───────────────────────────────────────┐
        │   1. CLI 参数解析 (cli::cli())        │
        │      如果是工具命令，直接执行并退出      │
        └───────────────────────────────────────┘
                            │
                            ▼
        ┌───────────────────────────────────────┐
        │   2. 分析器初始化 (可选)               │
        │      - pprof (性能分析)                │
        │      - pyroscope (持续性能监控)        │
        └───────────────────────────────────────┘
                            │
                            ▼
        ┌───────────────────────────────────────┐
        │   3. 日志与追踪系统初始化              │
        │      - 标准日志 (tracing-subscriber)  │
        │      - OpenTelemetry 追踪             │
        │      - 企业版 AI 追踪                 │
        └───────────────────────────────────────┘
                            │
        ┌───────────────────┴────────────────────┐
        │                                        │
        ▼                                        ▼
┌──────────────────┐              ┌──────────────────────┐
│  Job Runtime     │              │   gRPC Runtime       │
│  (后台任务线程)   │              │   (集群通信线程)      │
└──────────────────┘              └──────────────────────┘
        │                                        │
        │                                        │
        └────────────────┬───────────────────────┘
                         │
                         ▼
              ┌──────────────────────┐
              │   HTTP Runtime       │
              │   (主线程 - API)      │
              └──────────────────────┘
                         │
                         ▼
              ┌──────────────────────┐
              │   优雅关闭流程        │
              │   (信号处理)          │
              └──────────────────────┘
```

### 代码位置

- **主入口**: [src/main.rs:118-570](src/main.rs#L118-L570)
- **Job 运行时初始化**: [src/main.rs:250-341](src/main.rs#L250-L341)
- **gRPC 运行时初始化**: [src/main.rs:354-386](src/main.rs#L354-L386)
- **HTTP 服务器启动**: [src/main.rs:478-485](src/main.rs#L478-L485)

---

## 三运行时架构

### 1. Job Runtime (后台任务运行时)

**目的**: 处理所有后台任务，与 HTTP/gRPC 服务隔离，避免阻塞用户请求

**独立线程**: 使用 `std::thread::spawn` 创建独立线程

**工作线程数**: 由 `cfg.limit.job_runtime_worker_num` 配置

```rust
// 代码位置: src/main.rs:254-341
let job_rt_handle = std::thread::spawn(move || {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(cfg.limit.job_runtime_worker_num)
        .thread_name("job_runtime")
        .max_blocking_threads(cfg.limit.job_runtime_blocking_worker_num)
        .build()
        .unwrap();

    rt.block_on(async move {
        // 初始化任务...
    });
});
```

**主要初始化步骤**:

1. **集群注册**: `cluster::register_and_keep_alive()` - 向集群注册节点并保持心跳
2. **配置初始化**: `config::init()` - 从数据库加载所有配置
3. **数据库迁移**: `migration::init_db()` - 运行 SeaORM 迁移
4. **基础设施初始化**: `infra::init()` - 初始化对象存储、缓存等
5. **公共基础设施**: `common_infra::init()` - 初始化共享组件
6. **企业版初始化**: `init_enterprise()` (仅在 `feature = "enterprise"` 时)
7. **摄取器初始化**: `ingester::init()` - 初始化 WAL 和数据摄取组件
8. **后台任务初始化**: `job::init()` - 启动所有后台任务

**后台任务类型** (位于 `src/job/` 模块):

- **文件压缩**: 合并小文件到大 Parquet 文件
- **文件列表管理**: 维护文件元数据
- **告警调度**: 定时检查告警规则
- **数据保留**: 删除过期数据
- **使用统计**: 收集并报告使用指标
- **缓存预热**: 预加载热数据

**关闭流程**:

```rust
// 位置: src/main.rs:330-340
job_shutdown_rx.await.ok();  // 等待关闭信号

// 刷新所有缓存到磁盘
metadata::close().await;          // 刷新 distinct values
ingester::flush_all().await;      // 刷新 WAL 缓存
db::compact::files::sync_cache_to_db().await;  // 刷新压缩偏移量
db.close().await;                 // 关闭数据库连接
```

### 2. gRPC Runtime (集群通信运行时)

**目的**: 处理节点间通信和 OpenTelemetry 协议摄取

**独立线程**: 使用 `std::thread::spawn` 创建独立线程

**工作线程数**: 由 `cfg.limit.grpc_runtime_worker_num` 配置

```rust
// 代码位置: src/main.rs:358-386
let grpc_rt_handle = std::thread::spawn(move || {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(cfg.limit.grpc_runtime_worker_num)
        .thread_name("grpc_runtime")
        .max_blocking_threads(cfg.limit.grpc_runtime_blocking_worker_num)
        .build()
        .expect("grpc runtime init failed");

    rt.block_on(async move {
        if config::cluster::LOCAL_NODE.is_router() {
            init_router_grpc_server(...).await
        } else {
            init_common_grpc_server(...).await
        }
    });
});
```

**服务类型**:

#### 普通节点服务 (init_common_grpc_server)

代码位置: [src/main.rs:572-685](src/main.rs#L572-L685)

| 服务名称 | 用途 | 端口 |
|---------|-----|------|
| **EventServer** | 集群事件通知 (缓存失效、配置更新) | `cfg.grpc.port` |
| **SearchServer** | 分布式查询协调 | 同上 |
| **MetricsServer** | 内部指标查询 | 同上 |
| **MetricsServiceServer** | OpenTelemetry Metrics 摄取 | 同上 |
| **LogsServiceServer** | OpenTelemetry Logs 摄取 | 同上 |
| **TraceServiceServer** | OpenTelemetry Traces 摄取 | 同上 |
| **QueryCacheServer** | 查询结果缓存同步 | 同上 |
| **IngestServer** | 数据摄取转发 | 同上 |
| **StreamsServer** | 数据流管理 | 同上 |
| **FlightServiceServer** | Arrow Flight 数据传输 | 同上 |
| **NodeServiceServer** | 节点管理 (上线/下线/刷新) | 同上 |
| **ClusterInfoServiceServer** | 集群信息查询 | 同上 |

#### 路由节点服务 (init_router_grpc_server)

代码位置: [src/main.rs:687-741](src/main.rs#L687-L741)

路由节点仅提供简化的摄取服务:

- **LogsServiceServer**: 接收日志并转发到摄取节点
- **MetricsServiceServer**: 接收指标并转发
- **TraceServiceServer**: 接收追踪数据并转发

**TLS 支持**:

```rust
if cfg.grpc.tls_enabled {
    let cert = std::fs::read_to_string(&cfg.grpc.tls_cert_path)?;
    let key = std::fs::read_to_string(&cfg.grpc.tls_key_path)?;
    let identity = Identity::from_pem(cert, key);
    tonic::transport::Server::builder()
        .tls_config(ServerTlsConfig::new().identity(identity))?
}
```

**压缩配置**:

所有服务都启用 Gzip 压缩:

```rust
.send_compressed(CompressionEncoding::Gzip)
.accept_compressed(CompressionEncoding::Gzip)
.max_decoding_message_size(cfg.grpc.max_message_size * 1024 * 1024)
.max_encoding_message_size(cfg.grpc.max_message_size * 1024 * 1024)
```

### 3. HTTP Runtime (Web API 运行时)

**目的**: 处理所有外部 HTTP 请求 (REST API, Web UI)

**主线程**: 使用主 Tokio 运行时 (`#[tokio::main]`)

**工作进程数**: 由 `cfg.limit.http_worker_num` 配置 (默认值 = CPU 核心数)

```rust
// 代码位置: src/main.rs:743-844
let server = HttpServer::new(move || {
    let mut app = App::new();

    if config::cluster::LOCAL_NODE.is_router() {
        // 路由节点配置 (简化路由 + 限流)
        app.service(...)
    } else {
        // 普通节点配置 (完整 API)
        app.service(...)
    }

    app
        .app_data(web::JsonConfig::default().limit(cfg.limit.req_json_limit))
        .app_data(web::PayloadConfig::new(cfg.limit.req_payload_limit))
        .wrap(middlewares::Compress::default())
        .wrap(middleware::Logger::new(&get_http_access_log_format()))
        .wrap(RequestTracing::new())
})
.workers(cfg.limit.http_worker_num)
.worker_max_blocking_threads(cfg.limit.http_worker_max_blocking)
.bind(haddr)?
.run()
```

**路由配置**:

- **路由节点**: 仅提供数据摄取和转发 API
- **普通节点**: 提供完整的 REST API (查询、管理、配置)

**中间件链** (按执行顺序):

1. **RequestTracing**: OpenTelemetry 请求追踪
2. **Logger**: HTTP 访问日志
3. **Compress**: Gzip/Brotli 响应压缩
4. **SlowLog**: 慢查询日志 (超过阈值时记录)
5. **RateLimitController** (企业版): 限流控制

**TLS/HTTPS 支持**:

```rust
if cfg.http.tls_enabled {
    let sc = http_tls_config()?;
    server.bind_rustls_0_23(haddr, sc)?
} else {
    server.bind(haddr)?
}
```

**超时配置**:

```rust
.keep_alive(KeepAlive::Timeout(Duration::from_secs(cfg.limit.http_keep_alive)))
.client_request_timeout(Duration::from_secs(cfg.limit.http_request_timeout))
.shutdown_timeout(cfg.limit.http_shutdown_timeout)
```

---

## 启动序列详解

### 完整启动时序图

```
时间轴
  │
  ├── 1. CLI 参数解析 (0-50ms)
  │    ├── 解析命令行参数
  │    ├── 加载配置文件 (如果指定 -c/--config)
  │    └── 执行工具命令并退出 (如果是工具命令)
  │
  ├── 2. 分析器设置 (50-100ms, 仅在启用时)
  │    ├── pprof 初始化
  │    └── pyroscope 代理启动
  │
  ├── 3. 日志系统初始化 (100-200ms)
  │    ├── 检测配置 (events_enabled, tracing_enabled)
  │    ├── 初始化 tracing_subscriber
  │    ├── 配置 OpenTelemetry 导出器 (如果启用)
  │    └── 设置日志输出 (stdout 或文件)
  │
  ├── 4. Job Runtime 启动 (200ms-5s)
  │    ├── 创建独立线程和 Tokio 运行时
  │    ├── 集群注册和心跳 (200-500ms)
  │    ├── 配置初始化 (500-1000ms)
  │    │    ├── 从数据库加载组织/流/函数配置
  │    │    └── 构建缓存
  │    ├── 数据库迁移 (1-2s)
  │    │    └── 运行 SeaORM 迁移 (仅在首次启动或升级时)
  │    ├── Infra 初始化 (2-3s)
  │    │    ├── 连接对象存储 (S3/MinIO/GCS/Azure)
  │    │    ├── 初始化文件列表缓存
  │    │    ├── 初始化内存缓存
  │    │    └── 初始化磁盘缓存
  │    ├── 企业版初始化 (3-4s, 仅企业版)
  │    │    ├── 加载 OpenFGA 配置
  │    │    ├── 初始化 AI 组件
  │    │    └── 启动 Pipeline 文件服务器
  │    ├── Ingester 初始化 (4-5s)
  │    │    ├── 加载 WAL 文件
  │    │    └── 恢复未完成的批次
  │    └── Job 初始化 (5s)
  │         ├── 文件压缩调度器
  │         ├── 告警调度器
  │         ├── 数据保留调度器
  │         └── 使用统计收集器
  │    ├── 发送初始化成功信号 (job_init_tx.send(true))
  │
  ├── 5. gRPC Runtime 启动 (5-6s)
  │    ├── 创建独立线程和 Tokio 运行时
  │    ├── 根据节点角色选择服务类型
  │    │    ├── 路由节点: 启动简化摄取服务
  │    │    └── 普通节点: 启动完整 gRPC 服务
  │    ├── 配置 TLS (如果启用)
  │    ├── 绑定端口并开始监听
  │    └── 发送初始化成功信号 (grpc_init_tx.send())
  │
  ├── 6. 运行时指标注册 (6s)
  │    ├── 注册 Job Runtime 指标收集器
  │    ├── 注册 gRPC Runtime 指标收集器
  │    ├── 注册 HTTP Runtime 指标收集器
  │    └── 启动指标收集器后台任务
  │
  ├── 7. 节点上线 (6-7s)
  │    ├── cluster::set_online() - 在元数据中标记节点为在线
  │    ├── job::init_deferred() - 启动延迟任务
  │    │    └── 仅在 gRPC 服务启动后执行的任务
  │    └── cluster::set_schedulable() - 标记节点可调度
  │         └── 重试最多 10 次 (每次间隔 1 秒)
  │
  ├── 8. 辅助服务启动 (7s)
  │    ├── 启动事件日志发送任务 (如果启用)
  │    ├── 发送遥测事件 (如果启用)
  │    └── 设置查询推荐定时触发器
  │
  ├── 9. HTTP Server 启动 (7-8s)
  │    ├── 解析监听地址
  │    ├── 配置 HTTP 工作线程
  │    ├── 设置路由和中间件
  │    ├── 配置 TLS (如果启用)
  │    ├── 绑定端口并开始监听
  │    ├── 注册优雅关闭处理器
  │    └── 等待运行 (阻塞直到收到关闭信号)
  │
  └── 10. 服务就绪 (8s+)
       ├── 所有服务正常运行
       ├── 接受 HTTP/gRPC 请求
       └── 后台任务定期执行
```

### 详细步骤解释

#### 步骤 1: CLI 参数解析

代码位置: [src/main.rs:126-128](src/main.rs#L126-L128)

```rust
if cli::cli().await? {
    return Ok(());  // 如果是工具命令,执行后退出
}
```

**工具命令** (直接执行并退出,不启动服务器):

- `reset` - 重置组件
- `import`/`export` - 数据导入导出
- `migrate-*` - 数据库迁移工具
- `sql` - 执行 SQL 查询
- `parse-id` - 解析雪花 ID
- 等等 (详见 CLI 命令章节)

**配置文件加载**:

如果指定 `-c/--config` 参数,配置文件会在此步骤加载,覆盖环境变量。

#### 步骤 2: 分析器设置 (可选)

代码位置: [src/main.rs:146-182](src/main.rs#L146-L182)

**pprof 分析器** (需要 `feature = "profiling"`):

```rust
let pprof_guard = pprof::ProfilerGuardBuilder::default()
    .frequency(1000)  // 1000 Hz 采样频率
    .blocklist(&["libc", "libgcc", "pthread", "vdso"])
    .build()
    .unwrap();
```

环境变量:
- `ZO_PROFILING_PPROF_ENABLED=true`
- `ZO_PROFILING_PPROF_FLAMEGRAPH_PATH=/path/to/flamegraph.svg`

**pyroscope 分析器** (需要 `feature = "pyroscope"`):

```rust
let agent = PyroscopeAgent::builder(
    &cfg.profiling.pyroscope_server_url,
    &cfg.profiling.pyroscope_project_name,
)
.tags([
    ("role", cfg.common.node_role.as_str()),
    ("instance", cfg.common.instance_name.as_str()),
    ("version", config::VERSION),
])
.backend(pprof_backend(PprofConfig::new().sample_rate(100)))
.build()
.expect("Failed to setup pyroscope agent");
```

环境变量:
- `ZO_PROFILING_PYROSCOPE_ENABLED=true`
- `ZO_PROFILING_PYROSCOPE_SERVER_URL=http://pyroscope:4040`
- `ZO_PROFILING_PYROSCOPE_PROJECT_NAME=openobserve`

#### 步骤 3: 日志系统初始化

代码位置: [src/main.rs:186-225](src/main.rs#L186-L225)

**三种日志模式**:

1. **事件日志模式** (`cfg.log.events_enabled = true`):
   - 使用自定义 `ZoLogger` 将日志发送到内部队列
   - 日志会被摄取到 OpenObserve 自身

2. **OpenTelemetry 追踪模式** (`cfg.common.tracing_enabled = true`):
   - 使用 `tracing-opentelemetry` 将日志作为追踪发送
   - 支持分布式追踪上下文传播

3. **标准日志模式**:
   - 使用 `tracing-subscriber` 输出到文件或 stdout
   - 支持 JSON 格式和纯文本格式

**日志输出配置**:

```rust
let (writer, guard) = if cfg.log.file_dir.is_empty() {
    // 输出到 stdout
    let (non_blocking, _guard) = tracing_appender::non_blocking(std::io::stdout());
    (BoxMakeWriter::new(non_blocking), _guard)
} else {
    // 输出到滚动日志文件 (每日轮转)
    let file_appender = tracing_appender::rolling::daily(
        &cfg.log.file_dir,
        file_name_prefix
    );
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    (BoxMakeWriter::new(non_blocking), _guard)
};
```

环境变量:
- `ZO_LOG_LEVEL=info` (debug, info, warn, error)
- `ZO_LOG_FILE_DIR=/var/log/openobserve`
- `ZO_LOG_FILE_NAME_PREFIX=o2.log`
- `ZO_LOG_JSON_FORMAT=false`
- `ZO_LOG_EVENTS_ENABLED=false`

#### 步骤 4: Job Runtime 初始化

代码位置: [src/main.rs:250-341](src/main.rs#L250-L341)

**4.1 集群注册** (代码位置: src/main.rs:269-272):

```rust
if let Err(e) = cluster::register_and_keep_alive().await {
    job_init_tx.send(false).ok();
    panic!("cluster init failed: {e}");
}
```

功能:
- 向元数据存储注册节点
- 启动心跳任务 (每 30 秒发送一次)
- 加载集群节点列表

**4.2 配置初始化** (代码位置: src/main.rs:274-277):

```rust
if let Err(e) = config::init().await {
    job_init_tx.send(false).ok();
    panic!("config init failed: {e}");
}
```

加载内容:
- 组织 (Organizations)
- 数据流 (Streams)
- 函数 (Functions)
- 告警规则 (Alerts)
- 仪表板 (Dashboards)
- 模板 (Templates)

**4.3 数据库迁移** (代码位置: src/main.rs:280-283):

```rust
if let Err(e) = migration::init_db().await {
    job_init_tx.send(false).ok();
    panic!("db init failed: {e}");
}
```

功能:
- 连接数据库 (MySQL/PostgreSQL/SQLite)
- 运行 SeaORM 迁移
- 创建或更新表结构

**4.4 基础设施初始化** (代码位置: src/main.rs:286-294):

```rust
if let Err(e) = infra::init().await {
    job_init_tx.send(false).ok();
    panic!("infra init failed: {e}");
}

if let Err(e) = common_infra::init().await {
    job_init_tx.send(false).ok();
    panic!("common infra init failed: {e}");
}
```

初始化组件:
- 对象存储连接 (S3/MinIO/GCS/Azure/本地)
- 文件列表缓存 (从数据库或对象存储加载)
- 内存缓存 (LRU 缓存,用于查询结果)
- 磁盘缓存 (用于热数据)
- WAL 目录创建

**4.5 企业版初始化** (代码位置: src/main.rs:297-301):

```rust
#[cfg(feature = "enterprise")]
if let Err(e) = crate::init_enterprise().await {
    job_init_tx.send(false).ok();
    panic!("enterprise init failed: {e}");
}
```

企业版功能初始化 (详见 [src/main.rs:1441-1479](src/main.rs#L1441-L1479)):
- OpenFGA 权限系统
- 限流规则加载
- AI 组件初始化
- Pipeline 文件服务器
- Super Cluster 初始化 (如果启用)

**4.6 摄取器初始化** (代码位置: src/main.rs:304-307):

```rust
if let Err(e) = ingester::init().await {
    job_init_tx.send(false).ok();
    panic!("ingester init failed: {e}");
}
```

功能:
- 加载 WAL (Write-Ahead Log) 文件
- 恢复未完成的批次
- 初始化数据分区

**4.7 后台任务初始化** (代码位置: src/main.rs:310-313):

```rust
if let Err(e) = job::init().await {
    job_init_tx.send(false).ok();
    panic!("job init failed: {e}");
}
```

启动的后台任务:
- **文件压缩** (`compact::run_merge`): 合并小文件
- **文件列表同步** (`file_list::run`): 同步对象存储文件列表
- **告警检查** (`alert::run`): 定时评估告警规则
- **数据保留** (`retention::run`): 删除过期数据
- **使用统计** (`stats::run`): 收集使用指标
- **缓存预热** (`cache::run`): 预加载热数据
- **流统计更新** (`stream_stats::run`): 更新流统计信息

#### 步骤 5: gRPC Runtime 启动

代码位置: [src/main.rs:354-390](src/main.rs#L354-L390)

**节点角色判断**:

```rust
let ret = if config::cluster::LOCAL_NODE.is_router() {
    init_router_grpc_server(...)  // 路由节点
} else {
    init_common_grpc_server(...)  // 普通节点 (all/ingester/querier/compactor)
};
```

**普通节点 gRPC 服务**:

12 个服务 (详见"三运行时架构 - gRPC Runtime"章节)

**路由节点 gRPC 服务**:

仅 3 个摄取服务:
- LogsServiceServer
- MetricsServiceServer
- TraceServiceServer

#### 步骤 6: 运行时指标注册

代码位置: [src/main.rs:391-397](src/main.rs#L391-L397)

```rust
// 注册 HTTP Runtime
if let Ok(handle) = tokio::runtime::Handle::try_current() {
    openobserve::service::runtime_metrics::register_runtime("http".to_string(), handle);
}

// 启动指标收集器
openobserve::service::runtime_metrics::start_metrics_collector().await;
```

收集的指标:
- 运行时工作线程数
- 阻塞线程数
- 任务队列深度
- CPU 使用率
- 内存使用量

#### 步骤 7: 节点上线

代码位置: [src/main.rs:399-434](src/main.rs#L399-L434)

**7.1 节点标记为在线**:

```rust
let _ = cluster::set_online().await;
```

**7.2 启动延迟任务**:

```rust
job::init_deferred()
    .await
    .expect("Deferred jobs failed to init");
```

延迟任务 (仅在 gRPC 服务启动后执行):
- 分布式查询协调器
- 集群事件订阅

**7.3 节点标记为可调度**:

```rust
for _ in 0..10 {
    match cluster::set_schedulable().await {
        Ok(_) => {
            start_ok = true;
            break;
        }
        Err(e) => {
            log::error!("set node schedulable failed: {e}");
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
}
```

可调度状态允许调度器将任务分配到该节点。

#### 步骤 8: 辅助服务启动

代码位置: [src/main.rs:407-476](src/main.rs#L407-L476)

**8.1 事件日志发送** (如果启用):

```rust
if cfg.log.events_enabled {
    tokio::task::spawn(zo_logger::send_logs());
}
```

**8.2 遥测事件** (如果启用):

```rust
if cfg.common.telemetry_enabled {
    tokio::task::spawn(async move {
        meta::telemetry::Telemetry::new()
            .send_track_event("OpenObserve - Starting server", None, true, false)
            .await;
    });
}
```

**8.3 查询推荐触发器**:

```rust
// 检查是否存在查询推荐触发器,不存在则创建
match db::scheduler::list(Some(QueryRecommendations)).await {
    Ok(list) if list.len() == 1 => {}  // 已存在
    _ => {
        // 删除旧的并创建新的
        let trigger = Trigger {
            org: META_ORG_ID.to_string(),
            module: TriggerModule::QueryRecommendations,
            module_key: "QueryRecommendations".to_string(),
            next_run_at: next_minute,  // 下一个整分钟
            status: TriggerStatus::Waiting,
            retries: 3,
            ..Default::default()
        };
        db::scheduler::push(trigger).await
    }
}
```

#### 步骤 9: HTTP Server 启动

代码位置: [src/main.rs:478-486](src/main.rs#L478-L486)

**条件启动**:

```rust
if !cfg.common.tracing_enabled && cfg.common.tracing_search_enabled {
    // 仅搜索追踪启用时,使用无追踪的 HTTP 服务器
    if let Err(e) = init_http_server_without_tracing().await {
        log::error!("HTTP server runs failed: {e}");
    }
} else {
    // 正常启动
    if let Err(e) = init_http_server().await {
        log::error!("HTTP server runs failed: {e}");
    }
}
```

**服务器配置** (详见"三运行时架构 - HTTP Runtime"章节)

**优雅关闭处理器**:

```rust
let handle = server.handle();
tokio::task::spawn(graceful_shutdown(handle));
server.await?;  // 阻塞直到收到关闭信号
```

---

## 优雅关闭流程

### 关闭触发信号

代码位置: [src/main.rs:949-991](src/main.rs#L949-L991)

#### Unix/Linux 系统:

```rust
use tokio::signal::unix::{SignalKind, signal};

let mut sigquit = signal(SignalKind::quit()).unwrap();    // SIGQUIT (Ctrl+\)
let mut sigterm = signal(SignalKind::terminate()).unwrap(); // SIGTERM (kill)
let mut sigint = signal(SignalKind::interrupt()).unwrap();  // SIGINT (Ctrl+C)

tokio::select! {
    _ = sigquit.recv() =>  log::info!("SIGQUIT received"),
    _ = sigterm.recv() =>  log::info!("SIGTERM received"),
    _ = sigint.recv() =>   log::info!("SIGINT received"),
}
```

**推荐信号**:
- **SIGTERM** (15): 优雅关闭,允许清理资源
- **SIGINT** (2): Ctrl+C,同样优雅关闭
- **SIGQUIT** (3): 立即关闭并生成 core dump

#### Windows 系统:

```rust
use tokio::signal::windows::*;

let mut sigbreak = ctrl_break().unwrap();    // Ctrl+Break
let mut sigint = ctrl_c().unwrap();          // Ctrl+C
let mut sigquit = ctrl_close().unwrap();     // 关闭窗口
let mut sigterm = ctrl_shutdown().unwrap();  // 系统关闭

tokio::select! {
    _ = sigbreak.recv() =>  log::info!("ctrl-break received"),
    _ = sigquit.recv() =>  log::info!("ctrl-c received"),
    _ = sigterm.recv() =>  log::info!("ctrl-close received"),
    _ = sigint.recv() =>   log::info!("ctrl-shutdown received"),
}
```

### 关闭顺序

代码位置: [src/main.rs:483-569](src/main.rs#L483-L569)

```
关闭信号接收
    │
    ▼
┌────────────────────────────────┐
│ 1. 节点下线                     │
│    cluster::set_offline()      │
│    (停止接收新任务)              │
└────────────────────────────────┘
    │
    ▼
┌────────────────────────────────┐
│ 2. HTTP 服务器停止              │
│    handle.stop(true).await     │
│    (停止接受新连接,等待现有请求)  │
└────────────────────────────────┘
    │
    ▼
┌────────────────────────────────┐
│ 3. OpenTelemetry 追踪停止      │
│    tracer_provider.shutdown()  │
└────────────────────────────────┘
    │
    ▼
┌────────────────────────────────┐
│ 4. 刷新使用报告                │
│    self_reporting::flush()     │
└────────────────────────────────┘
    │
    ▼
┌────────────────────────────────┐
│ 5. 离开集群                    │
│    cluster::leave()            │
│    (取消节点注册)               │
└────────────────────────────────┘
    │
    ▼
┌────────────────────────────────┐
│ 6. gRPC 服务器停止             │
│    grpc_shutdown_tx.send(())   │
│    grpc_stopped_rx.await       │
│    grpc_rt_handle.join()       │
└────────────────────────────────┘
    │
    ▼
┌────────────────────────────────┐
│ 7. 后台任务停止                │
│    job_shutdown_tx.send(())    │
│    job_stopped_rx.await        │
│                                │
│    清理工作:                    │
│    - metadata::close()         │
│      刷新 distinct values      │
│    - ingester::flush_all()     │
│      刷新 WAL 到磁盘           │
│    - db::compact::sync_cache   │
│      刷新压缩偏移量             │
│    - db.close()                │
│      关闭数据库连接             │
│                                │
│    job_rt_handle.join()        │
└────────────────────────────────┘
    │
    ▼
┌────────────────────────────────┐
│ 8. 发送停止遥测事件 (如果启用)  │
│    Telemetry::send_track_event │
│    ("Server stopped")          │
└────────────────────────────────┘
    │
    ▼
┌────────────────────────────────┐
│ 9. 保存分析数据 (如果启用)      │
│    - pprof flamegraph 保存     │
│    - pyroscope agent 停止      │
└────────────────────────────────┘
    │
    ▼
┌────────────────────────────────┐
│ 10. 程序退出                   │
│     Ok(())                     │
└────────────────────────────────┘
```

**关键点**:

1. **节点下线优先**: 确保调度器不再分配新任务
2. **HTTP 先于 gRPC**: 停止外部请求,内部通信继续工作
3. **缓存刷新**: 确保所有数据持久化到磁盘
4. **数据库最后关闭**: 确保所有写入完成

---

## CLI 命令完整参考

### CLI 文件位置

- **主 CLI 解析器**: [src/cli/basic/cli.rs](src/cli/basic/cli.rs)
- **命令实现**: `src/cli/basic/` 目录下的各个模块

### 通用参数

所有命令支持的全局参数:

```bash
openobserve [OPTIONS] [SUBCOMMAND]
```

**全局选项**:

| 参数 | 短参数 | 说明 | 默认值 |
|-----|-------|-----|-------|
| `--config <FILE>` | `-c` | 配置文件路径 | `.env` 或环境变量 |

### 命令分类

#### 1. 数据重置命令

**命令**: `reset`

**用途**: 重置系统组件到初始状态

**子命令**:

```bash
# 重置 root 用户密码
openobserve reset root

# 删除指定用户
openobserve reset user --org <组织名> --email <用户邮箱>

# 删除所有告警规则
openobserve reset alert --org <组织名>

# 删除所有仪表板
openobserve reset dashboard --org <组织名>

# 删除所有函数
openobserve reset function --org <组织名>

# 重置流统计信息
openobserve reset stream-stats --org <组织名> --stream <流名> --type <logs|metrics|traces>
```

**示例**:

```bash
# 重置 root 密码 (交互式输入)
openobserve reset root

# 删除 default 组织中的用户
openobserve reset user --org default --email user@example.com

# 删除 default 组织的所有告警
openobserve reset alert --org default
```

**代码位置**: [src/cli/basic/reset.rs](src/cli/basic/reset.rs)

#### 2. 数据导入导出命令

**命令**: `import` / `export`

**用途**: 在 OpenObserve 实例间迁移数据

**导出**:

```bash
openobserve export \
  --org <组织名> \
  --file <输出文件路径> \
  [--type <alerts|dashboards|functions|streams|all>]
```

**导入**:

```bash
openobserve import \
  --org <组织名> \
  --file <输入文件路径>
```

**示例**:

```bash
# 导出 default 组织的所有配置
openobserve export --org default --file /tmp/backup.json

# 仅导出仪表板
openobserve export --org default --file /tmp/dashboards.json --type dashboards

# 导入配置到新组织
openobserve import --org new_org --file /tmp/backup.json
```

**代码位置**: [src/cli/basic/import.rs](src/cli/basic/import.rs), [src/cli/basic/export.rs](src/cli/basic/export.rs)

#### 3. 数据库迁移命令

**命令**: `migrate-file-list`

**用途**: 将文件列表从对象存储迁移到数据库 (或反之)

```bash
openobserve migrate-file-list \
  [--from <s3|db>] \
  [--to <s3|db>]
```

**示例**:

```bash
# 从对象存储迁移到数据库
openobserve migrate-file-list --from s3 --to db

# 从数据库迁移到对象存储
openobserve migrate-file-list --from db --to s3
```

**命令**: `migrate-meta`

**用途**: 迁移元数据存储后端

```bash
openobserve migrate-meta \
  [--from <mysql|postgres|sqlite>] \
  [--to <mysql|postgres|sqlite>]
```

**命令**: `migrate-dashboards`

**用途**: 迁移旧版本仪表板格式

```bash
openobserve migrate-dashboards
```

**命令**: `migrate-pipeline`

**用途**: 迁移 Pipeline 配置

```bash
openobserve migrate-pipeline
```

**代码位置**: [src/cli/basic/migrate.rs](src/cli/basic/migrate.rs)

#### 4. 节点管理命令

**命令**: `node`

**用途**: 管理集群节点

**子命令**:

```bash
# 将节点标记为离线
openobserve node offline --name <节点名>

# 将节点标记为在线
openobserve node online --name <节点名>

# 刷新节点缓存
openobserve node flush --name <节点名>

# 查看节点状态
openobserve node status --name <节点名>

# 列出所有节点
openobserve node list

# 查看节点指标
openobserve node metrics --name <节点名>
```

**示例**:

```bash
# 列出所有节点
openobserve node list

# 将节点 node-1 设为离线
openobserve node offline --name node-1

# 刷新节点 node-2 的缓存
openobserve node flush --name node-2
```

**代码位置**: [src/cli/basic/node.rs](src/cli/basic/node.rs)

#### 5. 查询命令

**命令**: `sql`

**用途**: 执行 SQL 查询

```bash
openobserve sql \
  --org <组织名> \
  --query <SQL 查询语句> \
  [--start-time <开始时间>] \
  [--end-time <结束时间>]
```

**示例**:

```bash
# 查询日志
openobserve sql \
  --org default \
  --query "SELECT * FROM logs WHERE level = 'error' LIMIT 100" \
  --start-time "2025-01-01T00:00:00Z" \
  --end-time "2025-01-02T00:00:00Z"
```

**代码位置**: [src/cli/basic/sql.rs](src/cli/basic/sql.rs)

#### 6. 工具命令

**命令**: `parse-id`

**用途**: 解析雪花 ID (Snowflake ID) 为时间戳

```bash
openobserve parse-id <雪花ID>
```

**示例**:

```bash
# 解析 ID
openobserve parse-id 123456789012345678

# 输出:
# Timestamp: 2025-01-15 10:23:45 UTC
# Worker ID: 1
# Sequence: 123
```

**命令**: `consistent-hash`

**用途**: 测试一致性哈希算法

```bash
openobserve consistent-hash \
  --key <哈希键> \
  --nodes <节点数>
```

**命令**: `query-optimiser`

**用途**: 分析和优化 SQL 查询

```bash
openobserve query-optimiser \
  --org <组织名> \
  --query <SQL 查询>
```

**代码位置**: [src/cli/basic/tools.rs](src/cli/basic/tools.rs)

#### 7. 数据库命令

**命令**: `seaorm-rollback`

**用途**: 回滚 SeaORM 数据库迁移

```bash
openobserve seaorm-rollback [--steps <回滚步数>]
```

**示例**:

```bash
# 回滚最近一次迁移
openobserve seaorm-rollback

# 回滚最近 3 次迁移
openobserve seaorm-rollback --steps 3
```

**命令**: `upgrade-db`

**用途**: 手动运行数据库升级

```bash
openobserve upgrade-db
```

**代码位置**: [src/cli/basic/db.rs](src/cli/basic/db.rs)

#### 8. 维护命令

**命令**: `delete-parquet`

**用途**: 删除 Parquet 文件及其元数据

```bash
openobserve delete-parquet \
  --org <组织名> \
  --stream <流名> \
  --type <logs|metrics|traces> \
  --file <文件路径>
```

**命令**: `recover-file-list`

**用途**: 从对象存储恢复文件列表

```bash
openobserve recover-file-list \
  [--org <组织名>] \
  [--stream <流名>] \
  [--type <logs|metrics|traces>]
```

**示例**:

```bash
# 恢复所有文件列表
openobserve recover-file-list

# 恢复指定流的文件列表
openobserve recover-file-list --org default --stream nginx_logs --type logs
```

**命令**: `init-dir`

**用途**: 初始化数据目录结构

```bash
openobserve init-dir
```

**代码位置**: [src/cli/basic/maintenance.rs](src/cli/basic/maintenance.rs)

#### 9. 测试命令

**命令**: `test`

**用途**: 运行内部测试和基准测试

```bash
openobserve test [--type <unit|integration|benchmark>]
```

**代码位置**: [src/cli/basic/test.rs](src/cli/basic/test.rs)

#### 10. 视图命令

**命令**: `view`

**用途**: 查看系统信息

**子命令**:

```bash
# 查看版本信息
openobserve view version

# 查看配置
openobserve view config

# 查看集群状态
openobserve view cluster

# 查看统计信息
openobserve view stats --org <组织名>
```

**示例**:

```bash
# 查看版本
openobserve view version
# 输出: OpenObserve v0.17.0

# 查看当前配置
openobserve view config

# 查看集群状态
openobserve view cluster
```

**代码位置**: [src/cli/basic/view.rs](src/cli/basic/view.rs)

---

## 常用操作场景

### 场景 1: 首次部署

```bash
# 1. 准备配置文件
cat > /etc/openobserve/.env << EOF
ZO_ROOT_USER_EMAIL=admin@example.com
ZO_ROOT_USER_PASSWORD=Complexpass#123
ZO_DATA_DIR=/var/openobserve/data
ZO_DATA_WAL_DIR=/var/openobserve/wal
ZO_META_STORE=mysql
ZO_META_MYSQL_DSN=mysql://user:pass@localhost:3306/openobserve
ZO_S3_BUCKET_NAME=openobserve-data
ZO_S3_REGION_NAME=us-east-1
EOF

# 2. 初始化目录结构
openobserve init-dir

# 3. 启动服务
openobserve
```

### 场景 2: 数据备份与恢复

```bash
# 备份配置
openobserve export --org default --file /backup/config-$(date +%Y%m%d).json

# 恢复到新实例
openobserve import --org default --file /backup/config-20250115.json
```

### 场景 3: 集群维护

```bash
# 1. 查看所有节点
openobserve node list

# 2. 将节点标记为离线 (维护前)
openobserve node offline --name node-2

# 3. 执行维护...

# 4. 将节点标记为在线
openobserve node online --name node-2

# 5. 刷新节点缓存
openobserve node flush --name node-2
```

### 场景 4: 故障排查

```bash
# 1. 查看节点状态
openobserve node status --name node-1

# 2. 查看节点指标
openobserve node metrics --name node-1

# 3. 执行查询测试
openobserve sql \
  --org default \
  --query "SELECT COUNT(*) FROM logs WHERE _timestamp > NOW() - INTERVAL '1 hour'" \
  --start-time "2025-01-15T10:00:00Z" \
  --end-time "2025-01-15T11:00:00Z"

# 4. 恢复文件列表 (如果元数据损坏)
openobserve recover-file-list --org default
```

### 场景 5: 数据库迁移

```bash
# 从 SQLite 迁移到 MySQL
# 1. 停止服务
pkill openobserve

# 2. 修改配置
export ZO_META_STORE=mysql
export ZO_META_MYSQL_DSN="mysql://user:pass@localhost:3306/openobserve"

# 3. 运行迁移
openobserve migrate-meta --from sqlite --to mysql

# 4. 启动服务
openobserve
```

### 场景 6: 性能分析

```bash
# 启动时启用 pprof 分析
export ZO_PROFILING_PPROF_ENABLED=true
export ZO_PROFILING_PPROF_FLAMEGRAPH_PATH=/tmp/flamegraph.svg

# 启动服务
openobserve

# 运行一段时间后 (例如 1 小时)
# Ctrl+C 优雅关闭

# 查看火焰图
firefox /tmp/flamegraph.svg
```

### 场景 7: 重置密码

```bash
# 重置 root 用户密码
openobserve reset root

# 交互式提示:
# Enter new password: ********
# Confirm password: ********
# Password reset successfully!
```

### 场景 8: 清理数据

```bash
# 删除所有告警 (谨慎使用!)
openobserve reset alert --org default

# 删除所有仪表板
openobserve reset dashboard --org default

# 重置流统计
openobserve reset stream-stats --org default --stream nginx_logs --type logs
```

---

## 配置文件与环境变量

### 配置文件优先级

1. **命令行参数** (`-c/--config`) - 最高优先级
2. **环境变量**
3. **`.env` 文件** (当前目录)
4. **默认值**

### 核心配置项

#### 服务器配置

```bash
# HTTP 服务器
ZO_HTTP_ADDR=0.0.0.0           # 监听地址
ZO_HTTP_PORT=5080              # 监听端口
ZO_HTTP_IPV6_ENABLED=false     # 启用 IPv6
ZO_HTTP_TLS_ENABLED=false      # 启用 HTTPS
ZO_HTTP_TLS_CERT_PATH=         # TLS 证书路径
ZO_HTTP_TLS_KEY_PATH=          # TLS 私钥路径

# gRPC 服务器
ZO_GRPC_ADDR=0.0.0.0           # 监听地址
ZO_GRPC_PORT=5081              # 监听端口
ZO_GRPC_TLS_ENABLED=false      # 启用 TLS
ZO_GRPC_MAX_MESSAGE_SIZE=16    # 最大消息大小 (MB)
```

#### 数据库配置

```bash
# 元数据存储
ZO_META_STORE=sqlite           # mysql, postgres, sqlite
ZO_META_MYSQL_DSN=             # MySQL 连接字符串
ZO_META_POSTGRES_DSN=          # PostgreSQL 连接字符串

# SQLite (默认)
# 文件位置: ${ZO_DATA_DIR}/metadata/metadata.db
```

#### 对象存储配置

```bash
# S3 兼容存储
ZO_S3_BUCKET_NAME=             # 存储桶名称
ZO_S3_REGION_NAME=us-east-1    # 区域
ZO_S3_PROVIDER=aws             # aws, minio, gcs, azure
ZO_S3_SERVER_URL=              # MinIO URL (MinIO 时需要)
ZO_S3_ACCESS_KEY=              # 访问密钥
ZO_S3_SECRET_KEY=              # 密钥

# 本地存储
ZO_LOCAL_MODE=true             # 启用本地模式
ZO_DATA_DIR=/data              # 数据目录
```

#### 运行时配置

```bash
# Job Runtime
ZO_JOB_RUNTIME_WORKER_NUM=4               # 工作线程数
ZO_JOB_RUNTIME_BLOCKING_WORKER_NUM=8      # 阻塞线程数

# gRPC Runtime
ZO_GRPC_RUNTIME_WORKER_NUM=4
ZO_GRPC_RUNTIME_BLOCKING_WORKER_NUM=8

# HTTP Runtime
ZO_HTTP_WORKER_NUM=0                      # 0 = CPU 核心数
ZO_HTTP_WORKER_MAX_BLOCKING=512
```

#### 集群配置

```bash
ZO_CLUSTER_NAME=openobserve       # 集群名称
ZO_NODE_ROLE=all                  # all, ingester, querier, compactor, router
ZO_INSTANCE_NAME=instance1        # 实例名称
ZO_CLUSTER_COORDINATOR=nats       # nats, etcd
```

#### 日志配置

```bash
ZO_LOG_LEVEL=info                 # debug, info, warn, error
ZO_LOG_FILE_DIR=                  # 日志文件目录 (空=stdout)
ZO_LOG_FILE_NAME_PREFIX=o2.log    # 日志文件前缀
ZO_LOG_JSON_FORMAT=false          # JSON 格式
ZO_LOG_EVENTS_ENABLED=false       # 发送日志到自身
```

#### 性能分析配置

```bash
# pprof
ZO_PROFILING_PPROF_ENABLED=false
ZO_PROFILING_PPROF_FLAMEGRAPH_PATH=/tmp/flamegraph.svg

# pyroscope
ZO_PROFILING_PYROSCOPE_ENABLED=false
ZO_PROFILING_PYROSCOPE_SERVER_URL=
ZO_PROFILING_PYROSCOPE_PROJECT_NAME=openobserve
```

#### OpenTelemetry 追踪配置

```bash
ZO_TRACING_ENABLED=false                    # 启用追踪
ZO_OTEL_OTLP_URL=                          # OTLP HTTP 端点
ZO_OTEL_OTLP_GRPC_URL=                     # OTLP gRPC 端点
ZO_TRACING_HEADER_KEY=Authorization        # 认证头名称
ZO_TRACING_HEADER_VALUE=                   # 认证头值

# 企业版 AI 追踪
O2_AI_TRACES_ENABLED=false                 # 启用 AI 追踪
O2_AI_EVAL_OTLP_ENDPOINT=                  # AI 评估端点
```

#### 限制配置

```bash
ZO_LIMIT_REQ_JSON_LIMIT=209715200          # JSON 请求大小限制 (200MB)
ZO_LIMIT_REQ_PAYLOAD_LIMIT=209715200       # 总 payload 限制 (200MB)
ZO_LIMIT_HTTP_REQUEST_TIMEOUT=600          # HTTP 请求超时 (秒)
ZO_LIMIT_HTTP_KEEP_ALIVE=30                # Keep-Alive 超时 (秒)
ZO_MEMORY_CACHE_MAX_SIZE=1073741824        # 内存缓存大小 (1GB)
ZO_DISK_CACHE_MAX_SIZE=10737418240         # 磁盘缓存大小 (10GB)
```

### 完整配置示例

```bash
# /etc/openobserve/.env

# === 基础配置 ===
ZO_ROOT_USER_EMAIL=admin@example.com
ZO_ROOT_USER_PASSWORD=Complexpass#123
ZO_INSTANCE_NAME=prod-node-1
ZO_NODE_ROLE=all
ZO_CLUSTER_NAME=openobserve-prod

# === 网络配置 ===
ZO_HTTP_ADDR=0.0.0.0
ZO_HTTP_PORT=5080
ZO_GRPC_PORT=5081

# === 数据库配置 ===
ZO_META_STORE=mysql
ZO_META_MYSQL_DSN=mysql://o2user:password@mysql-server:3306/openobserve

# === 对象存储 ===
ZO_S3_PROVIDER=aws
ZO_S3_BUCKET_NAME=openobserve-data-prod
ZO_S3_REGION_NAME=us-east-1
ZO_S3_ACCESS_KEY=AKIAXXXXXXXXXXXXXXXX
ZO_S3_SECRET_KEY=xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx

# === 本地数据目录 ===
ZO_DATA_DIR=/var/openobserve/data
ZO_DATA_WAL_DIR=/var/openobserve/wal

# === 运行时配置 ===
ZO_JOB_RUNTIME_WORKER_NUM=8
ZO_GRPC_RUNTIME_WORKER_NUM=8
ZO_HTTP_WORKER_NUM=0  # 自动检测 CPU 核心数

# === 日志配置 ===
ZO_LOG_LEVEL=info
ZO_LOG_FILE_DIR=/var/log/openobserve
ZO_LOG_JSON_FORMAT=true

# === 集群协调器 ===
ZO_CLUSTER_COORDINATOR=nats
ZO_NATS_ADDR=nats://nats-server:4222

# === 性能优化 ===
ZO_MEMORY_CACHE_MAX_SIZE=4294967296      # 4GB
ZO_DISK_CACHE_MAX_SIZE=107374182400      # 100GB
ZO_LIMIT_CPU_NUM=0                       # 0 = 自动检测

# === OpenTelemetry 追踪 ===
ZO_TRACING_ENABLED=true
ZO_OTEL_OTLP_URL=http://otel-collector:4318/v1/traces
ZO_TRACING_HEADER_KEY=Authorization
ZO_TRACING_HEADER_VALUE=Basic <base64-token>

# === 企业版功能 (如果编译时启用) ===
O2_RATE_LIMIT_ENABLED=true
O2_AI_TRACES_ENABLED=true
```

---

## 附录: 完整启动日志示例

```
2025-01-15T10:00:00.123Z  INFO openobserve: Starting OpenObserve v0.17.0
2025-01-15T10:00:00.125Z  INFO openobserve: System info: CPU cores 16, MEM total 64.00 GB, Disk total 1.00 TB, free 800.00 GB
2025-01-15T10:00:00.127Z  INFO openobserve: Caches info: Disk max size 100.00 GB, MEM max size 4.00 GB, Datafusion pool size: 8.00 GB
2025-01-15T10:00:00.500Z  INFO cluster: Node registered: prod-node-1, role: all
2025-01-15T10:00:01.200Z  INFO config: Loaded 5 organizations, 120 streams, 45 functions
2025-01-15T10:00:02.100Z  INFO migration: Running database migrations...
2025-01-15T10:00:02.500Z  INFO migration: Database is up to date
2025-01-15T10:00:03.000Z  INFO infra::storage: Connected to S3: openobserve-data-prod (us-east-1)
2025-01-15T10:00:03.500Z  INFO infra::cache: File list cache loaded: 15000 entries
2025-01-15T10:00:04.000Z  INFO infra::cache: Memory cache initialized: 4.00 GB
2025-01-15T10:00:04.200Z  INFO infra::cache: Disk cache initialized: 100.00 GB
2025-01-15T10:00:04.800Z  INFO ingester: WAL directory initialized: /var/openobserve/wal
2025-01-15T10:00:05.000Z  INFO job: File compactor started
2025-01-15T10:00:05.050Z  INFO job: Alert scheduler started
2025-01-15T10:00:05.100Z  INFO job: Retention scheduler started
2025-01-15T10:00:05.150Z  INFO job: Usage reporter started
2025-01-15T10:00:05.200Z  INFO backend job init success
2025-01-15T10:00:05.500Z  INFO starting gRPC server at 0.0.0.0:5081
2025-01-15T10:00:06.000Z  INFO cluster: Node set to online: prod-node-1
2025-01-15T10:00:06.500Z  INFO job: Deferred jobs initialized
2025-01-15T10:00:07.000Z  INFO cluster: Node set to schedulable: prod-node-1
2025-01-15T10:00:07.500Z  INFO Starting HTTP server at: 0.0.0.0:5080, thread_id: 0
2025-01-15T10:00:07.501Z  INFO Starting HTTP server at: 0.0.0.0:5080, thread_id: 1
2025-01-15T10:00:07.502Z  INFO Starting HTTP server at: 0.0.0.0:5080, thread_id: 2
2025-01-15T10:00:07.503Z  INFO Starting HTTP server at: 0.0.0.0:5080, thread_id: 3
2025-01-15T10:00:07.504Z  INFO Starting HTTP server at: 0.0.0.0:5080, thread_id: 4
2025-01-15T10:00:07.505Z  INFO Starting HTTP server at: 0.0.0.0:5080, thread_id: 5
2025-01-15T10:00:07.506Z  INFO Starting HTTP server at: 0.0.0.0:5080, thread_id: 6
2025-01-15T10:00:07.507Z  INFO Starting HTTP server at: 0.0.0.0:5080, thread_id: 7
```

**关闭日志示例**:

```
2025-01-15T15:30:00.000Z  INFO SIGTERM received
2025-01-15T15:30:00.100Z  INFO cluster: Node set to offline: prod-node-1
2025-01-15T15:30:00.200Z  INFO Node is offline
2025-01-15T15:30:01.000Z  INFO HTTP server stopped
2025-01-15T15:30:01.100Z  INFO Tracer provider shutdown result: Ok(())
2025-01-15T15:30:01.200Z  INFO Usage report flushed
2025-01-15T15:30:01.300Z  INFO Node left cluster
2025-01-15T15:30:01.400Z  INFO gRPC server starts shutting down
2025-01-15T15:30:02.000Z  INFO gRPC server stopped
2025-01-15T15:30:02.100Z  INFO metadata: Flushed 1500 distinct values
2025-01-15T15:30:02.500Z  INFO ingester: Flushed 250 MB WAL data
2025-01-15T15:30:02.700Z  INFO db::compact: Synced file list cache
2025-01-15T15:30:02.800Z  INFO db: Database connections closed
2025-01-15T15:30:03.000Z  INFO backend job stopped
2025-01-15T15:30:03.100Z  INFO server stopped
```

---

## 总结

本文档详细介绍了 OpenObserve 的启动流程和 CLI 命令使用方法:

1. **三运行时架构**: Job、gRPC、HTTP 独立运行,职责分离
2. **10 步启动序列**: 从 CLI 解析到服务就绪的完整流程
3. **优雅关闭**: 确保数据完整性的有序关闭流程
4. **完整 CLI 参考**: 涵盖数据管理、维护、故障排查等场景
5. **配置指南**: 环境变量和配置文件的完整说明

通过理解这些内容,您可以:
- 深入了解 OpenObserve 的启动和运行机制
- 有效使用 CLI 工具进行运维和故障排查
- 根据需求调整配置以优化性能
- 安全地执行数据迁移和系统维护

**相关文档**:
- [代码库分析报告.md](代码库分析报告.md) - 完整的代码库结构分析
- [OpenObserve企业版功能自研方案.md](OpenObserve企业版功能自研方案.md) - 企业版功能实现指南
