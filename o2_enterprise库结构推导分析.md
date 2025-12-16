# o2_enterprise 私有库结构推导分析报告

**分析日期:** 2025-01-27
**分析方法:** 通过代码库中的引用和使用方式逆向推导

---

## 1. 库总体结构

通过分析 OpenObserve 代码库中的所有引用（约 312+ 处），推导出 `o2_enterprise` 库的完整模块结构：

```
o2_enterprise/
└── enterprise/
    ├── auth/                    # 认证模块
    │   └── claim_parser/        # JWT Claims解析
    │
    ├── cipher/                  # 加密模块
    │   ├── algorithm/           # 加密算法
    │   ├── http_repr/           # HTTP表示层
    │   └── tink/                # Google Tink集成
    │
    ├── license/                 # 许可证模块
    │
    ├── ai/                      # AI功能模块
    │   ├── agent/               # AI Agent
    │   │   ├── meta/            # 元数据定义
    │   │   └── prompt/          # 提示词管理
    │   │       ├── prompts/     # 提示词模板
    │   │       ├── service/     # 提示词服务
    │   │       └── meta/        # 提示词元数据
    │   ├── mcp/                 # MCP协议支持
    │   └── rca/                 # 根因分析(RCA)
    │       └── integration/     # RCA集成
    │
    ├── common/                  # 通用模块
    │   ├── config/              # 配置管理
    │   ├── auditor/             # 审计日志
    │   └── streaming_agg_cache/ # 流式聚合缓存
    │
    ├── super_cluster/           # 超集群模块
    │   ├── queue/               # 消息队列
    │   ├── kv/                  # KV存储
    │   │   ├── cluster/         # 集群管理
    │   │   └── alert_manager/   # 告警管理器
    │   └── search/              # 分布式搜索
    │
    ├── actions/                 # 自动化操作模块
    │   ├── action_manager/      # 操作管理器
    │   ├── action_deployer/     # 操作部署器
    │   └── meta/                # 操作元数据
    │
    ├── search/                  # 搜索增强模块
    │   ├── cache/               # 搜索缓存
    │   │   └── streaming_agg/   # 流式聚合缓存
    │   ├── datafusion/          # DataFusion扩展
    │   │   └── distributed_plan/
    │   │       └── streaming_aggs_exec/
    │   ├── work_group/          # 工作组
    │   └── sampling/            # 采样
    │       ├── core/            # 核心采样
    │       └── execution/       # 采样执行
    │
    ├── cloud/                   # 云服务模块
    │   ├── billings/            # 计费
    │   ├── email/               # 邮件服务
    │   └── org_invites/         # 组织邀请
    │
    ├── drain/                   # 排空模块
    │   └── parquet/             # Parquet文件处理
    │
    ├── domain_management/       # 域管理模块
    │   ├── db/                  # 数据库操作
    │   └── meta/                # 元数据
    │
    ├── service_streams/         # 服务流模块
    │   ├── processor/           # 流处理器
    │   ├── batch_processor/     # 批处理器
    │   ├── sampler/             # 采样器
    │   ├── cache/               # 缓存
    │   └── storage/             # 存储
    │
    ├── pipeline/                # 管道模块
    │   ├── pipeline_job/        # 管道任务
    │   └── pipeline_file_server/# 管道文件服务
    │
    ├── recommendations/         # 推荐模块
    │   └── engine/              # 推荐引擎
    │
    ├── log_patterns/            # 日志模式模块
    │
    ├── re_patterns/             # 正则模式模块
    │
    ├── metering/                # 计量模块
    │
    ├── mmdb/                    # MaxMind数据库模块
    │   └── mmdb_downloader/     # MMDB下载器
    │
    └── alerts/                  # 告警增强模块
        └── semantic_config/     # 语义配置
```

---

## 2. 详细模块分析

### 2.1 认证模块 (enterprise::auth)

**路径:** `o2_enterprise::enterprise::auth`

**推导的结构:**
```rust
// o2_enterprise/src/enterprise/auth/mod.rs
pub mod claim_parser;

// o2_enterprise/src/enterprise/auth/claim_parser.rs
pub struct ClaimParserError;

pub async fn parse_claims<F1, F2, F3, F4>(
    claims: /* JWT Claims */,
    iam_settings_fn: F1,
    org_exists_fn: F2,
    /* ... */
) -> Result<ParsedClaims, ClaimParserError>;
```

**使用示例（来自源码）:**
```rust
// src/handler/http/auth/jwt.rs:1032
|error: o2_enterprise::enterprise::auth::claim_parser::ClaimParserError| { ... }

// src/handler/http/auth/jwt.rs:1060
match o2_enterprise::enterprise::auth::claim_parser::parse_claims(
    claims,
    iam_settings_fn,
    org_exists_fn,
    ...
)
```

---

### 2.2 加密模块 (enterprise::cipher)

**路径:** `o2_enterprise::enterprise::cipher`

**推导的结构:**
```rust
// o2_enterprise/src/enterprise/cipher/mod.rs
pub mod algorithm;
pub mod http_repr;
pub mod tink;

/// 加密器 Trait - 核心接口
pub trait Cipher: Send + Sync {
    fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>>;
    fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>>;
    fn clone_self(&self) -> Box<dyn Cipher>;
}

/// 加密密钥
pub struct Key {
    // 内部实现
}

impl Key {
    pub fn try_new(key_data: String, algorithm: Algorithm) -> Result<Self>;
}

/// 加密数据
pub struct CipherData {
    // 加密后的数据结构
}

// o2_enterprise/src/enterprise/cipher/algorithm.rs
pub enum Algorithm {
    Aes256Siv,
    // 其他算法...
}

// o2_enterprise/src/enterprise/cipher/http_repr.rs
pub struct HttpKey {
    // HTTP请求/响应中的密钥表示
}

pub struct HttpMechanism {
    // 加密机制HTTP表示
}

pub struct HttpStore {
    // 密钥存储HTTP表示
}

pub fn merge_updates(/* ... */) -> Result</* ... */>;

// o2_enterprise/src/enterprise/cipher/tink.rs
pub fn decode_tink_key(/* ... */) -> Result</* ... */>;
```

**使用示例:**
```rust
// src/cipher/registry.rs:17
use o2_enterprise::enterprise::cipher::Cipher;

// src/cipher/registry.rs:48
use o2_enterprise::enterprise::cipher::{Key, algorithm::Algorithm};

// 测试代码显示:
Box::new(Key::try_new("dGVzdC1rZQ==".to_string(), Algorithm::Aes256Siv).unwrap())
```

---

### 2.3 许可证模块 (enterprise::license)

**路径:** `o2_enterprise::enterprise::license`

**推导的结构:**
```rust
// o2_enterprise/src/enterprise/license/mod.rs

/// 许可证数据库键
pub const LICENSE_DB_KEY: &str = "...";

/// 许可证结构
pub struct License {
    // 许可证字段
}

impl License {
    pub fn load_from_str(key: &str) -> Result<Self>;
}

/// 检查许可证有效性
pub async fn check_license(license: &License) -> Result<()>;

/// 获取当前许可证
pub async fn get_license() -> Option<(String, License)>;

/// 获取过期时间
pub async fn get_expiry_time() -> Option<i64>;

/// 检查许可证是否过期
pub async fn license_expired() -> bool;

/// 获取摄取使用量 (0.0 - 1.0)
pub fn ingestion_used() -> f64;

/// 获取摄取超限次数
pub fn ingestion_limit_exceeded_count() -> u8;

/// 检查搜索是否允许
pub fn search_allowed() -> bool;

/// 更新许可证
pub async fn update_license<F>(get_usage: F) -> Result<()>
where
    F: Fn() -> /* usage data */;

/// 启动许可证检查任务
pub async fn start_license_check<F>(
    get_usage: F,
    is_router: bool,
) -> Result<()>;
```

**使用示例:**
```rust
// src/handler/http/request/license/mod.rs:17-19
use o2_enterprise::enterprise::license::{
    LICENSE_DB_KEY, License, check_license, get_license, ingestion_limit_exceeded_count,
    ingestion_used, license_expired,
};

// src/service/search/mod.rs:1566
if !o2_enterprise::enterprise::license::search_allowed() { ... }
```

---

### 2.4 AI模块 (enterprise::ai)

**路径:** `o2_enterprise::enterprise::ai`

**推导的结构:**
```rust
// o2_enterprise/src/enterprise/ai/mod.rs
pub mod agent;
pub mod mcp;
pub mod rca;

/// 初始化AI组件
pub fn init_ai_components(api: OpenApi) -> Result<()>;

// === Agent子模块 ===
// o2_enterprise/src/enterprise/ai/agent/mod.rs
pub mod meta;
pub mod prompt;

// o2_enterprise/src/enterprise/ai/agent/meta.rs
#[derive(Debug, Serialize, Deserialize)]
pub struct AiMessage {
    pub role: Role,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Role {
    System,
    User,
    Assistant,
}

// o2_enterprise/src/enterprise/ai/agent/prompt/mod.rs
pub mod prompts;
pub mod service;
pub mod meta;

// o2_enterprise/src/enterprise/ai/agent/prompt/prompts.rs
pub async fn load_system_prompt() -> Result<()>;

// o2_enterprise/src/enterprise/ai/agent/prompt/service.rs
pub async fn update_prompt_in_memory() -> Result<()>;

// o2_enterprise/src/enterprise/ai/agent/prompt/meta.rs
pub struct UpdatePromptRequest {
    // 更新提示词的请求
}

// === MCP子模块 (Model Context Protocol) ===
// o2_enterprise/src/enterprise/ai/mcp.rs
pub struct MCPRequest {
    // MCP请求
}

pub struct OAuthServerMetadata {
    // OAuth服务器元数据
}

impl OAuthServerMetadata {
    pub fn build(base_url: &str) -> Self;
}

pub async fn handle_mcp_request(
    request: MCPRequest,
    auth_token: Option<String>,
) -> Result<MCPResponse>;

pub async fn handle_mcp_request_stream(
    request: MCPRequest,
    auth_token: Option<String>,
) -> Result<impl Stream<Item = Result<Bytes>>>;

// === RCA子模块 (Root Cause Analysis) ===
// o2_enterprise/src/enterprise/ai/rca/mod.rs
pub mod integration;

pub fn register_batch(trace_id: &str, count: usize);
pub fn mark_alert_completed(trace_id: &str) -> bool;

// o2_enterprise/src/enterprise/ai/rca/integration.rs
pub fn is_rca_enabled_for_org(org: &str) -> bool;
pub async fn collect_alert_event(...) -> Result<()>;
pub async fn process_batch_and_create_incidents(trace_id: &str) -> Result<()>;
```

---

### 2.5 通用模块 (enterprise::common)

**路径:** `o2_enterprise::enterprise::common`

**推导的结构:**
```rust
// o2_enterprise/src/enterprise/common/mod.rs
pub mod config;
pub mod auditor;
pub mod streaming_agg_cache;

// === 配置子模块 ===
// o2_enterprise/src/enterprise/common/config.rs
pub struct O2Config {
    pub common: CommonConfig,
    pub super_cluster: SuperClusterConfig,
    pub ai: AiConfig,
    pub service_streams: ServiceStreamsConfig,
    pub rate_limit: RateLimitConfig,
}

pub struct CommonConfig {
    pub enable_enterprise_mmdb: bool,
    pub license_server_url: String,
}

pub struct SuperClusterConfig {
    pub enabled: bool,
}

pub struct AiConfig {
    pub enabled: bool,
}

pub struct ServiceStreamsConfig {
    // 服务流配置
}

impl ServiceStreamsConfig {
    pub fn get_fqn_priority_dimensions(&self) -> Vec<String>;
}

pub const GEO_IP_ENTERPRISE_ENRICHMENT_TABLE: &str = "...";

pub fn get_config() -> &'static O2Config;
pub fn refresh_config() -> Result<()>;

// === 审计子模块 ===
// o2_enterprise/src/enterprise/common/auditor.rs
pub struct AuditMessage {
    // 审计消息结构
}

pub enum Protocol {
    Http,
    Grpc,
    // ...
}

pub struct ResponseMeta {
    // 响应元数据
}

// === 流式聚合缓存子模块 ===
// o2_enterprise/src/enterprise/common/streaming_agg_cache.rs
pub struct StreamingAggsCacheResultRecordBatch {
    // 缓存结果批次
}

pub fn calculate_record_batches_deltas(...) -> Result<...>;
```

---

### 2.6 超集群模块 (enterprise::super_cluster)

**路径:** `o2_enterprise::enterprise::super_cluster`

**推导的结构:**
```rust
// o2_enterprise/src/enterprise/super_cluster/mod.rs
pub mod queue;
pub mod kv;
pub mod search;

// === Queue子模块 (消息队列) ===
// o2_enterprise/src/enterprise/super_cluster/queue.rs

/// 消息类型枚举
pub enum MessageType {
    Put,
    Delete,
    // ...
}

/// 通用消息结构
pub struct Message {
    pub message_type: MessageType,
    pub payload: Bytes,
    // ...
}

// 各种专用消息类型
pub struct AlertMessage { ... }
pub struct DashboardMessage { ... }
pub struct TemplateMessage { ... }
pub struct ReportMessage { ... }
pub struct KeysMessage { ... }
pub struct AiPromptMessage { ... }
pub struct ActionScriptsMessage { ... }
pub struct FolderMessage { ... }
pub struct DestinationMessage { ... }

// 队列类型
pub struct AlertsQueue;
pub struct DashboardsQueue;
pub struct TemplatesQueue;
pub struct SchemasQueue;
pub struct PipelinesQueue;
pub struct FoldersQueue;
pub struct DestinationsQueue;
pub struct MetaQueue;
pub struct OrgUsersQueue;
pub struct SchedulerQueue;
pub struct ActionScriptsQueue;
pub struct AiSystemPromptQueue;

// 队列操作函数
pub async fn put(message: Message) -> Result<()>;
pub async fn ai_prompt_put(message: AiPromptMessage) -> Result<()>;

// === KV子模块 ===
// o2_enterprise/src/enterprise/super_cluster/kv/mod.rs
pub mod cluster;
pub mod alert_manager;

pub async fn init() -> Result<()>;

// o2_enterprise/src/enterprise/super_cluster/kv/cluster.rs
pub fn get_grpc_token() -> String;
pub async fn list_by_role_group(role_group: Option<RoleGroup>) -> Result<Vec<ClusterInfo>>;

// o2_enterprise/src/enterprise/super_cluster/kv/alert_manager.rs
pub async fn get_job_cluster() -> Result<String>;
pub async fn register_job_cluster(cluster_name: &str) -> Result<()>;

// === Search子模块 ===
// o2_enterprise/src/enterprise/super_cluster/search.rs
pub async fn get_cluster_nodes(
    trace_id: &str,
    regions: Vec<String>,
) -> Result<Vec<ClusterNode>>;

pub async fn cancel_query(org_id: &str, trace_id: &str) -> Result<()>;
```

---

### 2.7 Actions自动化模块 (enterprise::actions)

**路径:** `o2_enterprise::enterprise::actions`

**推导的结构:**
```rust
// o2_enterprise/src/enterprise/actions/mod.rs
pub mod action_manager;
pub mod action_deployer;
pub mod meta;

// === action_manager子模块 ===
// o2_enterprise/src/enterprise/actions/action_manager.rs
pub struct ActionEndpoint {
    // Action端点定义
}

pub fn init_client() -> Result<()>;

pub async fn trigger_action(request: TriggerActionRequest) -> Result<ActionTriggerResult>;
pub async fn get_actions(org_id: &str) -> Result<Vec<Action>>;
pub async fn get_action_details(org_id: &str, action_id: &str) -> Result<Action>;
pub async fn register_app(/* ... */) -> Result<()>;
pub async fn update_app_on_target_cluster(/* ... */) -> Result<()>;
pub async fn delete_app_from_target_cluster(/* ... */) -> Result<()>;
pub async fn serve_file_from_s3(/* ... */) -> Result<Response>;

// === action_deployer子模块 ===
// o2_enterprise/src/enterprise/actions/action_deployer.rs
pub static ACTION_DEPLOYER: Lazy<ActionDeployer>;

pub async fn init() -> Result<()>;

// === meta子模块 ===
// o2_enterprise/src/enterprise/actions/meta.rs
pub struct TriggerActionRequest {
    // 触发Action的请求
}

pub struct TriggerSource {
    // 触发源
}

pub struct ActionTriggerResult {
    // Action触发结果
}
```

---

### 2.8 搜索增强模块 (enterprise::search)

**路径:** `o2_enterprise::enterprise::search`

**推导的结构:**
```rust
// o2_enterprise/src/enterprise/search/mod.rs
pub mod cache;
pub mod datafusion;
pub mod work_group;
pub mod sampling;

pub struct QueryManager;
pub struct TaskStatus;
pub struct WorkGroup;

pub async fn init() -> Result<()>;

// === cache子模块 ===
// o2_enterprise/src/enterprise/search/cache/mod.rs
pub mod streaming_agg;

// o2_enterprise/src/enterprise/search/cache/streaming_agg.rs
pub struct StreamingAggsPartitionStrategy;
pub struct CacheDiscoveryResult;

impl CacheDiscoveryResult {
    pub fn empty(start_time: i64, end_time: i64) -> Self;
}

// === datafusion子模块 ===
// o2_enterprise/src/enterprise/search/datafusion/mod.rs
pub mod distributed_plan;

// o2_enterprise/src/enterprise/search/datafusion/distributed_plan/mod.rs
pub mod streaming_aggs_exec;

// o2_enterprise/src/enterprise/search/datafusion/distributed_plan/streaming_aggs_exec.rs
pub static GLOBAL_CACHE: Lazy<...>;

// === work_group子模块 ===
// o2_enterprise/src/enterprise/search/work_group.rs
pub fn predict(nodes: Vec<Node>, /* ... */) -> WorkGroup;

// === sampling子模块 ===
// o2_enterprise/src/enterprise/search/sampling/mod.rs
pub mod core;
pub mod execution;

// o2_enterprise/src/enterprise/search/sampling/core.rs
pub fn parse_sampling_config(query: &str, /* ... */) -> Option<SamplingConfig>;

// o2_enterprise/src/enterprise/search/sampling/execution.rs
pub fn apply_sampling_to_files(files: &mut Vec<FileInfo>, config: &SamplingConfig);
```

---

### 2.9 云服务模块 (enterprise::cloud)

**路径:** `o2_enterprise::enterprise::cloud`

**推导的结构:**
```rust
// o2_enterprise/src/enterprise/cloud/mod.rs
pub mod billings;
pub mod email;
pub mod org_invites;

pub enum OrgInviteStatus {
    Pending,
    Accepted,
    Rejected,
    // ...
}

pub struct InvitationRecord {
    // 邀请记录
}

pub fn is_ofga_migrations_done() -> bool;
pub async fn migrate() -> Result<()>;
pub async fn ofga_migrate() -> Result<()>;

// === billings子模块 ===
// o2_enterprise/src/enterprise/cloud/billings.rs
pub async fn watch();

// === email子模块 ===
// o2_enterprise/src/enterprise/cloud/email.rs
pub async fn check_email(email: &str) -> Result<()>;

// === org_invites子模块 ===
// o2_enterprise/src/enterprise/cloud/org_invites.rs
// 组织邀请相关功能
```

---

### 2.10 其他模块

#### 2.10.1 Drain排空模块
```rust
// o2_enterprise/src/enterprise/drain/mod.rs
pub mod parquet;

pub fn is_draining() -> bool;
pub fn set_draining(value: bool);
pub fn get_drain_status(is_ingester: bool) -> DrainStatus;

// o2_enterprise/src/enterprise/drain/parquet.rs
pub fn check_has_pending_files(processing_count: usize, /* ... */) -> bool;
```

#### 2.10.2 Domain Management域管理模块
```rust
// o2_enterprise/src/enterprise/domain_management/mod.rs
pub mod db;
pub mod meta;

pub fn is_email_allowed(email: &str, /* ... */) -> Result<bool>;

// o2_enterprise/src/enterprise/domain_management/db.rs
pub async fn watch();
pub async fn cache() -> Result<()>;

// o2_enterprise/src/enterprise/domain_management/meta.rs
pub struct DomainManagementRequest {
    // 域管理请求
}
```

#### 2.10.3 Service Streams服务流模块
```rust
// o2_enterprise/src/enterprise/service_streams/mod.rs
pub mod processor;
pub mod batch_processor;
pub mod sampler;
pub mod cache;
pub mod storage;

// o2_enterprise/src/enterprise/service_streams/processor.rs
pub struct StreamProcessor;

impl StreamProcessor {
    pub fn new(org_id: String, /* ... */) -> Self;
}

// o2_enterprise/src/enterprise/service_streams/batch_processor.rs
pub fn queue_services(org_id: String, /* ... */);
pub async fn run();
pub async fn flush_all() -> Result<()>;

// o2_enterprise/src/enterprise/service_streams/sampler.rs
pub fn should_process_file(org_id: &str, /* ... */) -> bool;

// o2_enterprise/src/enterprise/service_streams/cache.rs
pub async fn watch();
pub async fn init_cache() -> Result<()>;

// o2_enterprise/src/enterprise/service_streams/storage.rs
pub struct ServiceStorage;

impl ServiceStorage {
    pub fn calculate_dimension_analytics(org_id: &str) -> Result<...>;
    pub fn correlate(/* ... */) -> Result<...>;
    pub fn list_grouped_by_fqn(org_id: &str) -> Result<...>;
}
```

#### 2.10.4 Pipeline管道模块
```rust
// o2_enterprise/src/enterprise/pipeline/mod.rs
pub mod pipeline_job;
pub mod pipeline_file_server;

// o2_enterprise/src/enterprise/pipeline/pipeline_job.rs
pub async fn run();

// o2_enterprise/src/enterprise/pipeline/pipeline_file_server.rs
pub struct PipelineFileServer;

impl PipelineFileServer {
    pub async fn run() -> Result<()>;
}
```

#### 2.10.5 Recommendations推荐模块
```rust
// o2_enterprise/src/enterprise/recommendations/mod.rs
pub mod engine;

// o2_enterprise/src/enterprise/recommendations/engine.rs
pub fn get_recommendations(/* ... */) -> Vec<Recommendation>;
```

#### 2.10.6 Log Patterns日志模式模块
```rust
// o2_enterprise/src/enterprise/log_patterns.rs
pub struct PatternExtractionConfig {
    pub max_logs_for_extraction: usize,
    // ...
}

pub struct PatternAccumulator;

impl PatternAccumulator {
    pub fn new(config: PatternExtractionConfig) -> Self;
}

pub fn extract_patterns_from_logs(log_messages: &[String], /* ... */) -> Vec<Pattern>;
pub async fn extract_patterns_from_stream(accumulator: PatternAccumulator, /* ... */) -> Result<...>;
```

#### 2.10.7 RE Patterns正则模式模块
```rust
// o2_enterprise/src/enterprise/re_patterns.rs
pub static PATTERN_MANAGER: Lazy<PatternManager>;

pub async fn get_pattern_manager() -> &'static PatternManager;

pub struct PatternManager {
    // 正则模式管理器
}
```

#### 2.10.8 Metering计量模块
```rust
// o2_enterprise/src/enterprise/metering.rs
pub async fn init<F1, F2>(
    get_usage: F1,
    ingest_data_retention_usages: F2,
) -> Result<()>;
```

#### 2.10.9 MMDB模块
```rust
// o2_enterprise/src/enterprise/mmdb/mod.rs
pub mod mmdb_downloader;

// o2_enterprise/src/enterprise/mmdb/mmdb_downloader.rs
pub async fn run_download_files();
```

#### 2.10.10 Alerts告警增强模块
```rust
// o2_enterprise/src/enterprise/alerts/mod.rs
pub mod semantic_config;

// o2_enterprise/src/enterprise/alerts/semantic_config.rs
pub struct SemanticFieldGroup;

impl SemanticFieldGroup {
    pub fn load_defaults_from_file() -> Vec<SemanticFieldGroup>;
}
```

---

## 3. 依赖关系图

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           o2_enterprise 模块依赖图                            │
└─────────────────────────────────────────────────────────────────────────────┘

                              ┌──────────────┐
                              │   license    │
                              │  (许可证控制) │
                              └──────┬───────┘
                                     │ 控制访问
         ┌───────────────────────────┼───────────────────────────┐
         │                           │                           │
         ▼                           ▼                           ▼
┌─────────────────┐        ┌─────────────────┐        ┌─────────────────┐
│      auth       │        │     cipher      │        │       ai        │
│   (认证解析)     │        │   (加密服务)    │        │   (AI功能)      │
└─────────────────┘        └─────────────────┘        └─────────────────┘
                                                              │
                                                              ├── agent
                                                              ├── mcp
                                                              └── rca
         │
         ▼
┌─────────────────┐        ┌─────────────────┐        ┌─────────────────┐
│     common      │◄───────│  super_cluster  │───────►│     search      │
│  (通用工具)      │        │   (跨集群通信)   │        │   (搜索增强)     │
└─────────────────┘        └─────────────────┘        └─────────────────┘
         │                          │
         ├── config                 ├── queue (消息队列)
         ├── auditor                ├── kv (KV存储)
         └── streaming_agg_cache    └── search (分布式搜索)

┌─────────────────┐        ┌─────────────────┐        ┌─────────────────┐
│     actions     │        │     cloud       │        │     drain       │
│  (自动化操作)    │        │   (云服务)      │        │   (排空控制)     │
└─────────────────┘        └─────────────────┘        └─────────────────┘
         │                          │
         ├── action_manager         ├── billings
         ├── action_deployer        ├── email
         └── meta                   └── org_invites

┌─────────────────┐        ┌─────────────────┐        ┌─────────────────┐
│ service_streams │        │    pipeline     │        │ domain_mgmt     │
│  (服务流处理)    │        │   (管道系统)    │        │  (域管理)        │
└─────────────────┘        └─────────────────┘        └─────────────────┘

┌─────────────────┐        ┌─────────────────┐        ┌─────────────────┐
│  log_patterns   │        │   re_patterns   │        │  recommendations│
│  (日志模式提取)  │        │   (正则模式)    │        │   (推荐引擎)     │
└─────────────────┘        └─────────────────┘        └─────────────────┘
```

---

## 4. 关键接口签名汇总

### 4.1 Cipher Trait
```rust
pub trait Cipher: Send + Sync {
    fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>>;
    fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>>;
    fn clone_self(&self) -> Box<dyn Cipher>;
}
```

### 4.2 License API
```rust
pub async fn check_license(license: &License) -> Result<()>;
pub async fn get_license() -> Option<(String, License)>;
pub fn ingestion_used() -> f64;
pub fn search_allowed() -> bool;
```

### 4.3 AI Agent API
```rust
pub struct AiMessage { pub role: Role, pub content: String }
pub enum Role { System, User, Assistant }
pub async fn handle_mcp_request(req: MCPRequest, token: Option<String>) -> Result<Response>;
```

### 4.4 Super Cluster Queue API
```rust
pub async fn put(message: Message) -> Result<()>;
pub struct Message { pub message_type: MessageType, pub payload: Bytes }
```

### 4.5 Common Config API
```rust
pub fn get_config() -> &'static O2Config;
pub struct O2Config {
    pub common: CommonConfig,
    pub super_cluster: SuperClusterConfig,
    pub ai: AiConfig,
    pub service_streams: ServiceStreamsConfig,
}
```

---

## 5. 自研替代建议

基于上述分析，以下是自研替代方案的优先级建议：

| 模块 | 复杂度 | 自研难度 | 优先级 | 建议方案 |
|------|--------|----------|--------|----------|
| cipher | 中 | 低 | P0 | 使用标准 `aes-siv` crate 实现 |
| license | 低 | 低 | P1 | RSA签名 + JSON格式许可证 |
| auth::claim_parser | 中 | 中 | P0 | 实现JWT Claims解析器 |
| common::config | 低 | 低 | P0 | 标准配置结构体 |
| common::auditor | 低 | 低 | P1 | 中间件 + 日志记录 |
| ai::agent | 高 | 高 | P3 | 集成第三方LLM API |
| ai::mcp | 高 | 高 | P3 | 实现MCP协议 |
| super_cluster | 很高 | 很高 | P4 | 建议使用现有消息队列 |
| actions | 高 | 中 | P2 | 脚本执行 + 调度系统 |
| search增强 | 高 | 高 | P2 | DataFusion扩展 |

---

## 6. 总结

`o2_enterprise` 是一个功能丰富的企业版扩展库，包含约 **20+ 个主要模块**，涵盖：

1. **安全性**: 认证(auth)、加密(cipher)、许可证(license)、审计(auditor)
2. **可扩展性**: 超集群(super_cluster)、分布式搜索(search)
3. **智能化**: AI功能(ai)、推荐引擎(recommendations)、日志模式(log_patterns)
4. **商业化**: 计费(billings)、计量(metering)、域管理(domain_management)
5. **自动化**: Actions系统、Pipeline管道

通过代码引用分析，本报告提供了各模块的接口签名推导，可作为自研替代方案的参考依据。
