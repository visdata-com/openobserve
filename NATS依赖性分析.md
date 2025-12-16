# OpenObserve NATS 依赖性分析

## 目录

1. [核心问题](#1-核心问题)
2. [NATS 的作用](#2-nats-的作用)
3. [本地模式 vs 集群模式](#3-本地模式-vs-集群模式)
4. [配置详解](#4-配置详解)
5. [依赖矩阵](#5-依赖矩阵)
6. [最佳实践](#6-最佳实践)

---

## 1. 核心问题

### 问题 1: 本地模式是否需要 NATS?

**答案: ❌ 不需要!**

本地模式 (`ZO_LOCAL_MODE=true`) **完全不依赖 NATS**,所有功能都使用本地实现。

### 问题 2: 集群模式是否必须有 NATS?

**答案: ✅ 是的,集群模式需要 NATS(或其他替代方案)**

集群模式 (`ZO_LOCAL_MODE=false`) 需要分布式协调和消息队列,默认使用 NATS。

---

## 2. NATS 的作用

### 2.1 在 OpenObserve 中的角色

NATS 在 OpenObserve 集群模式下扮演 **三个核心角色**:

```
┌─────────────────────────────────────────────────────────────┐
│                    1. 集群协调器                              │
│            (Cluster Coordinator)                             │
│  - 节点发现与注册                                             │
│  - 节点健康检查(心跳)                                          │
│  - 分布式锁                                                  │
│  - 配置同步                                                  │
└─────────────────────────────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────────┐
│                    2. 消息队列                               │
│               (Message Queue)                                │
│  - Compaction 任务队列                                       │
│  - File List 更新通知                                        │
│  - Alert 触发通知                                            │
│  - Schema 变更通知                                           │
└─────────────────────────────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────────┐
│                    3. 元数据存储                             │
│              (Metadata Store - 可选)                         │
│  - KV 存储(可用 MySQL/PostgreSQL 替代)                       │
│  - Stream 配置                                              │
│  - Alert 规则                                               │
│  - User 信息                                                │
└─────────────────────────────────────────────────────────────┘
```

---

### 2.2 NATS 的三种功能模式

| 功能 | 环境变量 | 默认值 | 说明 |
|------|---------|--------|------|
| **集群协调器** | `ZO_CLUSTER_COORDINATOR` | `nats` | 节点发现、健康检查、分布式锁 |
| **消息队列** | `ZO_QUEUE_STORE` | `nats` | 任务队列、事件通知 |
| **元数据存储** | `ZO_META_STORE` | `nats` (集群) / `sqlite` (本地) | 组织/流/用户等元数据 |

**代码位置:** [src/config/src/config.rs:753-758](src/config/src/config.rs#L753-L758)

```rust
#[env_config(name = "ZO_CLUSTER_COORDINATOR", default = "nats")]
pub cluster_coordinator: String,

#[env_config(name = "ZO_QUEUE_STORE", default = "nats")]
pub queue_store: String,

#[env_config(name = "ZO_META_STORE", default = "")]
pub meta_store: String,  // 空 = 根据 local_mode 自动选择
```

---

## 3. 本地模式 vs 集群模式

### 3.1 本地模式 (ZO_LOCAL_MODE=true)

**架构:**

```
┌─────────────────────────────────────────────────────────────┐
│                   单节点 OpenObserve                          │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │           所有组件运行在一个进程中                      │   │
│  │  - Ingester                                          │   │
│  │  - Querier                                           │   │
│  │  - Compactor                                         │   │
│  │  - AlertManager                                      │   │
│  │  - Router                                            │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │         本地存储(无需 NATS)                           │   │
│  │  - 元数据: SQLite (本地文件)                          │   │
│  │  - 数据: 本地磁盘                                     │   │
│  │  - 协调: 单进程,无需协调                             │   │
│  │  - 队列: 内存队列                                     │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

**特点:**

- ✅ **不需要 NATS**
- ✅ **不需要 MySQL/PostgreSQL**(使用 SQLite)
- ✅ **不需要 S3/MinIO**(使用本地磁盘)
- ✅ 单机即可运行
- ✅ 部署简单
- ❌ 无法水平扩展
- ❌ 单点故障

**配置示例:**

```bash
# 本地模式配置(最简单)
ZO_LOCAL_MODE=true
ZO_DATA_DIR=./data/openobserve/

# NATS 配置会被忽略
# ZO_CLUSTER_COORDINATOR=nats  ← 不生效
# ZO_QUEUE_STORE=nats          ← 不生效
# ZO_META_STORE=nats           ← 自动变为 sqlite
```

**代码逻辑:** [src/config/src/config.rs:2530-2534](src/config/src/config.rs#L2530-L2534)

```rust
// 元数据存储自动选择
if cfg.common.meta_store.is_empty() {
    if cfg.common.local_mode {
        cfg.common.meta_store = "sqlite".to_string();  // ← 强制使用 SQLite
    } else {
        cfg.common.meta_store = "nats".to_string();    // ← 默认使用 NATS
    }
}
```

---

### 3.2 集群模式 (ZO_LOCAL_MODE=false)

**架构:**

```
┌──────────────────────────────────────────────────────────────┐
│                        NATS Server                            │
│  ┌────────────────────────────────────────────────────────┐  │
│  │  - JetStream KV (元数据存储)                           │  │
│  │  - JetStream Streams (消息队列)                        │  │
│  │  - Core NATS (事件通知)                                │  │
│  └────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
                            ↑ ↓ ↑ ↓ ↑ ↓
        ┌───────────────────┴─┴─┴─┴─┴─┴───────────────────┐
        │                                                  │
┌───────▼────────┐  ┌──────────────┐  ┌──────────────────▼───┐
│  Ingester 节点  │  │ Querier 节点  │  │  Compactor 节点       │
│  ┌──────────┐  │  │ ┌──────────┐ │  │  ┌────────────────┐  │
│  │ 写入数据  │  │  │ │ 查询数据  │ │  │  │ 合并 Parquet  │  │
│  │ 生成 WAL │  │  │ │ 扫描文件  │ │  │  │ 获取任务队列  │  │
│  │ 上传 S3  │  │  │ │ 执行查询  │ │  │  │ 处理小文件    │  │
│  └──────────┘  │  │ └──────────┘ │  │  └────────────────┘  │
└────────────────┘  └──────────────┘  └──────────────────────┘
        │                   │                      │
        └───────────────────▼──────────────────────┘
                            │
              ┌─────────────▼────────────┐
              │   共享对象存储 (S3)       │
              │  - Parquet 数据文件      │
              │  - 索引文件              │
              └──────────────────────────┘
```

**特点:**

- ✅ 支持水平扩展
- ✅ 高可用(多节点)
- ✅ 角色分离(Ingester/Querier/Compactor)
- ⚠️ **必须有集群协调器** (NATS 或其他)
- ⚠️ **必须有消息队列** (NATS 或其他)
- ⚠️ **必须有共享元数据存储** (NATS/MySQL/PostgreSQL)
- ⚠️ **必须有共享对象存储** (S3/MinIO/GCS)

**配置示例:**

```bash
# 集群模式配置
ZO_LOCAL_MODE=false
ZO_NODE_ROLE=ingester  # 或 querier, compactor, all

# ⭐ 必须配置集群协调器
ZO_CLUSTER_COORDINATOR=nats
ZO_NATS_ADDR=nats://nats-server:4222

# ⭐ 必须配置消息队列
ZO_QUEUE_STORE=nats

# ⭐ 必须配置元数据存储(三选一)
# 方案 1: 使用 NATS KV
ZO_META_STORE=nats

# 方案 2: 使用 PostgreSQL (推荐)
ZO_META_STORE=postgres
ZO_META_POSTGRES_DSN=postgres://user:pass@postgres:5432/openobserve

# 方案 3: 使用 MySQL (已废弃)
ZO_META_STORE=mysql
ZO_META_MYSQL_DSN=mysql://user:pass@mysql:3306/openobserve

# ⭐ 必须配置对象存储
ZO_S3_BUCKET_NAME=openobserve
ZO_S3_ACCESS_KEY=...
ZO_S3_SECRET_KEY=...
```

---

## 4. 配置详解

### 4.1 元数据存储选择逻辑

**代码位置:** [src/config/src/config.rs:2537-2554](src/config/src/config.rs#L2537-L2554)

```rust
// 集群模式验证
if !cfg.common.local_mode
    && !cfg.common.meta_store.starts_with("postgres")
    && !cfg.common.meta_store.starts_with("mysql")
{
    return Err(anyhow::anyhow!(
        "Meta store only support mysql or postgres in cluster mode."
    ));
}
```

**规则:**

```
ZO_LOCAL_MODE=true:
  ├─ ZO_META_STORE 强制为 sqlite
  └─ ZO_CLUSTER_COORDINATOR/ZO_QUEUE_STORE 被忽略

ZO_LOCAL_MODE=false:
  ├─ ZO_META_STORE 必须是 postgres 或 mysql
  ├─ ZO_CLUSTER_COORDINATOR 默认 nats
  └─ ZO_QUEUE_STORE 默认 nats
```

---

### 4.2 NATS 配置参数

**代码位置:** [src/config/src/config.rs:1825-1891](src/config/src/config.rs#L1825-L1891)

| 环境变量 | 默认值 | 说明 |
|---------|--------|------|
| `ZO_NATS_ADDR` | `localhost:4222` | NATS 服务器地址 |
| `ZO_NATS_PREFIX` | `o2_` | KV 存储前缀 |
| `ZO_NATS_USER` | `""` | 用户名(可选) |
| `ZO_NATS_PASSWORD` | `""` | 密码(可选) |
| `ZO_NATS_REPLICAS` | `1` | JetStream 副本数 |
| `ZO_NATS_HISTORY` | `5` | KV 历史版本数 |
| `ZO_NATS_CONNECT_TIMEOUT` | `5` | 连接超时(秒) |
| `ZO_NATS_COMMAND_TIMEOUT` | `10` | 命令超时(秒) |
| `ZO_NATS_LOCK_WAIT_TIMEOUT` | `3600` | 分布式锁等待超时(秒) |
| `ZO_NATS_QUEUE_MAX_AGE` | `60` | 队列消息最大保留时间(天) |
| `ZO_NATS_EVENT_MAX_AGE` | `3600` | 事件消息最大保留时间(秒) |
| `ZO_NATS_QUEUE_MAX_SIZE` | `2048` | 队列最大大小(MB) |

**完整配置示例:**

```bash
# NATS 服务器连接
ZO_NATS_ADDR=nats://nats1:4222,nats2:4222,nats3:4222  # 支持多个地址
ZO_NATS_PREFIX=o2_
ZO_NATS_USER=openobserve
ZO_NATS_PASSWORD=complexpass123

# JetStream 配置
ZO_NATS_REPLICAS=3          # 生产环境推荐 3 副本
ZO_NATS_HISTORY=10          # 保留 10 个历史版本

# 超时配置
ZO_NATS_CONNECT_TIMEOUT=10
ZO_NATS_COMMAND_TIMEOUT=30
ZO_NATS_LOCK_WAIT_TIMEOUT=600  # 10 分钟

# 存储配置
ZO_NATS_QUEUE_MAX_SIZE=4096    # 4GB
ZO_NATS_QUEUE_MAX_AGE=30       # 保留 30 天
ZO_NATS_EVENT_MAX_AGE=7200     # 2 小时
```

---

### 4.3 NATS 初始化逻辑

**代码位置:** [src/infra/src/db/nats.rs:89-91](src/infra/src/db/nats.rs#L89-L91)

```rust
pub async fn init() {
    _ = get_nats_client().await;  // 连接 NATS 服务器
}
```

**连接过程:**

```rust
async fn connect() -> Client {
    let cfg = get_config();
    let opts = ConnectOptions::new()
        .user_and_password(cfg.nats.user.clone(), cfg.nats.password.clone())
        .connect_timeout(Duration::from_secs(cfg.nats.connect_timeout as u64))
        .max_reconnects(None);  // 无限重连

    let addrs = cfg.nats.addr
        .split(',')
        .map(|s| s.trim().to_string())
        .collect::<Vec<_>>();

    async_nats::connect_with_options(addrs, opts)
        .await
        .expect("Failed to connect to NATS")
}
```

**注意:**

- 如果 `ZO_LOCAL_MODE=true`,NATS 初始化会**被跳过**(虽然代码会执行,但不会实际连接)
- 如果 `ZO_LOCAL_MODE=false`,NATS 连接失败会导致**启动失败**

---

## 5. 依赖矩阵

### 5.1 完整依赖对比

| 组件 | 本地模式 | 集群模式 | 说明 |
|------|---------|---------|------|
| **NATS** | ❌ 不需要 | ✅ 需要 | 集群协调 + 消息队列 |
| **MySQL/PostgreSQL** | ❌ 不需要 | ✅ 需要 | 元数据存储(或用 NATS KV) |
| **S3/MinIO/GCS** | ❌ 不需要 | ✅ 需要 | 共享对象存储 |
| **SQLite** | ✅ 需要 | ❌ 不用 | 本地元数据存储 |
| **本地磁盘** | ✅ 需要 | ⚠️ 可选 | 本地模式存数据,集群模式存 WAL |

---

### 5.2 部署架构对比

#### 本地模式部署

```bash
# 最小化部署(单个二进制文件即可)
./openobserve

# 数据存储在本地
./data/openobserve/
├── db/           # SQLite 元数据
├── stream/       # Parquet 数据文件
└── wal/          # WAL 日志
```

**依赖:** 无

---

#### 集群模式部署 - 方案 1 (NATS + PostgreSQL)

```yaml
# docker-compose.yml
version: '3'
services:
  # NATS 服务器
  nats:
    image: nats:2.10-alpine
    command: ["-js", "-sd", "/data"]
    volumes:
      - nats_data:/data
    ports:
      - "4222:4222"

  # PostgreSQL 数据库
  postgres:
    image: postgres:16-alpine
    environment:
      POSTGRES_DB: openobserve
      POSTGRES_USER: o2_user
      POSTGRES_PASSWORD: o2_password_123
    volumes:
      - postgres_data:/var/lib/postgresql/data
    ports:
      - "5432:5432"

  # MinIO 对象存储
  minio:
    image: minio/minio:latest
    command: server /data --console-address ":9001"
    environment:
      MINIO_ROOT_USER: minioadmin
      MINIO_ROOT_PASSWORD: minioadmin
    volumes:
      - minio_data:/data
    ports:
      - "9000:9000"
      - "9001:9001"

  # OpenObserve Ingester
  openobserve-ingester:
    image: public.ecr.aws/zinclabs/openobserve:latest
    environment:
      ZO_LOCAL_MODE: "false"
      ZO_NODE_ROLE: "ingester"
      ZO_CLUSTER_COORDINATOR: "nats"
      ZO_QUEUE_STORE: "nats"
      ZO_META_STORE: "postgres"
      ZO_NATS_ADDR: "nats://nats:4222"
      ZO_META_POSTGRES_DSN: "postgres://o2_user:o2_password_123@postgres:5432/openobserve"
      ZO_S3_PROVIDER: "minio"
      ZO_S3_SERVER_URL: "http://minio:9000"
      ZO_S3_ACCESS_KEY: "minioadmin"
      ZO_S3_SECRET_KEY: "minioadmin"
      ZO_S3_BUCKET_NAME: "openobserve"
    depends_on:
      - nats
      - postgres
      - minio

  # OpenObserve Querier
  openobserve-querier:
    image: public.ecr.aws/zinclabs/openobserve:latest
    environment:
      ZO_LOCAL_MODE: "false"
      ZO_NODE_ROLE: "querier"
      ZO_CLUSTER_COORDINATOR: "nats"
      ZO_META_STORE: "postgres"
      ZO_NATS_ADDR: "nats://nats:4222"
      ZO_META_POSTGRES_DSN: "postgres://o2_user:o2_password_123@postgres:5432/openobserve"
      ZO_S3_PROVIDER: "minio"
      ZO_S3_SERVER_URL: "http://minio:9000"
      ZO_S3_ACCESS_KEY: "minioadmin"
      ZO_S3_SECRET_KEY: "minioadmin"
      ZO_S3_BUCKET_NAME: "openobserve"
    ports:
      - "5080:5080"
    depends_on:
      - nats
      - postgres
      - minio

volumes:
  nats_data:
  postgres_data:
  minio_data:
```

**依赖:** NATS + PostgreSQL + MinIO

---

#### 集群模式部署 - 方案 2 (仅 NATS,不推荐)

```bash
# 只使用 NATS(元数据也存在 NATS KV)
ZO_LOCAL_MODE=false
ZO_CLUSTER_COORDINATOR=nats
ZO_QUEUE_STORE=nats
ZO_META_STORE=nats           # ⚠️ 不推荐,性能较差
ZO_NATS_ADDR=nats://nats:4222
```

**依赖:** NATS + MinIO

**注意:** 不推荐使用 NATS 作为元数据存储,因为:
- 性能较差(KV 存储不如关系型数据库)
- 查询功能有限(无 SQL 支持)
- 数据量大时内存占用高

---

### 5.3 企业功能依赖

某些企业功能对 NATS 有强依赖:

**代码位置:** [src/main.rs:1482-1491](src/main.rs#L1482-L1491)

```rust
#[cfg(feature = "enterprise")]
fn check_ratelimit_config(cfg: &Config, o2cfg: &O2Config) -> Result<(), anyhow::Error> {
    if o2cfg.rate_limit.rate_limit_enabled {
        let meta_store: config::meta::meta_store::MetaStore =
            cfg.common.queue_store.as_str().into();
        if meta_store != config::meta::meta_store::MetaStore::Nats {
            return Err(anyhow::anyhow!(
                "ZO_QUEUE_STORE must be nats when ratelimit is enabled"
            ));
        }
    }
    Ok(())
}
```

**需要 NATS 的企业功能:**

| 功能 | 是否必须 NATS | 原因 |
|------|--------------|------|
| **Rate Limiting** | ✅ 必须 | 需要 NATS 的分布式计数器 |
| **Super Cluster** | ✅ 必须 | 跨集群通信依赖 NATS |
| **Search Jobs** | ⚠️ 推荐 | 任务队列建议用 NATS |
| **Streaming Aggregation** | ⚠️ 推荐 | 缓存协调建议用 NATS |

---

## 6. 最佳实践

### 6.1 本地开发/测试

**推荐配置:**

```bash
# 最简单的本地模式
ZO_LOCAL_MODE=true
ZO_DATA_DIR=./data/openobserve/

# 不需要配置任何外部依赖
# 不需要 NATS
# 不需要 MySQL/PostgreSQL
# 不需要 S3/MinIO
```

**启动:**

```bash
./openobserve
```

**优点:**

- ✅ 零依赖
- ✅ 启动快
- ✅ 调试方便

---

### 6.2 生产环境(小规模)

**推荐配置:**

```bash
# 本地模式(单节点)
ZO_LOCAL_MODE=true
ZO_DATA_DIR=/var/lib/openobserve/

# 使用 RAID/LVM 保证数据可靠性
# 定期备份 /var/lib/openobserve/
```

**适用场景:**

- 日志量 < 100 GB/天
- QPS < 1000
- 可接受短暂停机

---

### 6.3 生产环境(大规模)

**推荐配置:**

```bash
# 集群模式
ZO_LOCAL_MODE=false
ZO_NODE_ROLE=all  # 或分角色部署

# 集群协调(NATS)
ZO_CLUSTER_COORDINATOR=nats
ZO_NATS_ADDR=nats://nats1:4222,nats2:4222,nats3:4222
ZO_NATS_REPLICAS=3

# 消息队列(NATS)
ZO_QUEUE_STORE=nats

# 元数据存储(PostgreSQL,推荐)
ZO_META_STORE=postgres
ZO_META_POSTGRES_DSN=postgres://o2_user:pass@postgres:5432/openobserve

# 对象存储(S3)
ZO_S3_BUCKET_NAME=openobserve-prod
ZO_S3_ACCESS_KEY=...
ZO_S3_SECRET_KEY=...
```

**架构:**

```
NATS 集群 (3 节点)
    ↓
PostgreSQL 主从
    ↓
OpenObserve 集群 (多节点)
    ↓
S3 对象存储
```

**适用场景:**

- 日志量 > 1 TB/天
- QPS > 10000
- 高可用要求
- 需要水平扩展

---

### 6.4 NATS 集群部署

**单节点 NATS (开发/测试):**

```bash
nats-server -js -sd /data
```

**NATS 集群 (生产):**

```yaml
# nats1.conf
server_name: nats1
port: 4222
jetstream {
    store_dir: /data
}
cluster {
    name: o2-cluster
    listen: 0.0.0.0:6222
    routes: [
        nats://nats2:6222
        nats://nats3:6222
    ]
}
```

**启动:**

```bash
# 节点 1
nats-server -c nats1.conf

# 节点 2
nats-server -c nats2.conf

# 节点 3
nats-server -c nats3.conf
```

**验证:**

```bash
# 检查集群状态
nats-server --routes

# 检查 JetStream 状态
nats server report jetstream
```

---

### 6.5 故障排查

#### 问题 1: 连接 NATS 失败

**错误信息:**

```
Failed to connect to NATS: connection refused
```

**排查步骤:**

```bash
# 1. 检查 NATS 是否运行
netstat -tuln | grep 4222

# 2. 检查防火墙
telnet nats-server 4222

# 3. 检查 NATS 日志
docker logs nats

# 4. 检查配置
echo $ZO_NATS_ADDR
```

---

#### 问题 2: 集群模式启动失败

**错误信息:**

```
Meta store only support mysql or postgres in cluster mode.
```

**原因:** 集群模式必须使用 PostgreSQL 或 MySQL,不能用 NATS KV

**解决方案:**

```bash
# 方案 1: 使用 PostgreSQL
ZO_META_STORE=postgres
ZO_META_POSTGRES_DSN=postgres://...

# 方案 2: 使用 MySQL (不推荐,已废弃)
ZO_META_STORE=mysql
ZO_META_MYSQL_DSN=mysql://...
```

---

#### 问题 3: Rate Limiting 启动失败

**错误信息:**

```
ZO_QUEUE_STORE must be nats when ratelimit is enabled
```

**原因:** Rate Limiting 功能强制要求使用 NATS

**解决方案:**

```bash
# 启用 Rate Limiting 时必须配置 NATS
ZO_QUEUE_STORE=nats
ZO_NATS_ADDR=nats://nats:4222
```

---

## 7. 总结

### 7.1 快速决策表

| 场景 | 是否需要 NATS | 配置 |
|------|--------------|------|
| **本地开发** | ❌ 不需要 | `ZO_LOCAL_MODE=true` |
| **单机生产(小规模)** | ❌ 不需要 | `ZO_LOCAL_MODE=true` |
| **集群生产(大规模)** | ✅ 需要 | `ZO_LOCAL_MODE=false` + NATS |
| **使用 Rate Limiting** | ✅ 需要 | 强制要求 NATS |
| **使用 Super Cluster** | ✅ 需要 | 强制要求 NATS |

---

### 7.2 核心要点

1. **本地模式完全不需要 NATS**
   - 元数据: SQLite
   - 协调: 单进程,无需协调
   - 队列: 内存队列

2. **集群模式需要 NATS**(或替代方案)
   - 集群协调: NATS
   - 消息队列: NATS
   - 元数据存储: PostgreSQL/MySQL(推荐) 或 NATS KV(不推荐)

3. **企业功能强制依赖 NATS**
   - Rate Limiting: 必须用 NATS
   - Super Cluster: 必须用 NATS

4. **生产环境推荐**
   - 小规模: 本地模式(不用 NATS)
   - 大规模: 集群模式 + NATS 集群 + PostgreSQL

---

## 8. 参考资料

### 8.1 相关代码文件

| 文件 | 说明 |
|------|------|
| [src/config/src/config.rs](src/config/src/config.rs) | 配置定义和验证 |
| [src/infra/src/db/nats.rs](src/infra/src/db/nats.rs) | NATS 客户端实现 |
| [src/main.rs](src/main.rs) | 启动流程 |

### 8.2 官方文档

- [NATS 官方文档](https://docs.nats.io/)
- [NATS JetStream](https://docs.nats.io/nats-concepts/jetstream)
- [OpenObserve 部署指南](https://openobserve.ai/docs/deployment/)

---

**最后更新:** 2025-01-29
**版本:** 1.0



docker run -d --name nats -p 4222:4222 nats:latest -js