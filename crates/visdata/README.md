# VisData 企业版 SSO 和 RBAC 模块

## 概述

VisData 是 OpenObserve 的自研企业版 SSO 和 RBAC 模块，提供：
- **SSO (Single Sign-On)**: 支持 OIDC 和 LDAP 认证
- **RBAC (Role-Based Access Control)**: 细粒度权限控制

### 与官方企业版的区别

| 特性 | 官方企业版 | VisData |
|------|-----------|---------|
| RBAC 后端 | OpenFGA (需独立部署) | 内嵌实现 (无需外部服务) |
| SSO 后端 | Dex (需独立部署) | 内嵌 OIDC/LDAP 库 |
| 前端兼容性 | 原生 | 完全兼容 (API 一致) |
| 数据库表前缀 | - | `vd_` |
| Feature Flag | `enterprise` | `visdata` |

## 快速开始

### 编译

```bash
# 启用 visdata 功能编译
cargo build --release --features visdata

# 检查编译
cargo check --features visdata
```

### 配置

VisData 模块会在启动时自动初始化，使用与主应用相同的数据库连接。

## 架构设计

### 目录结构

```
crates/visdata/
├── Cargo.toml                    # 包配置
└── src/
    ├── lib.rs                    # 模块入口，全局单例
    ├── error.rs                  # 错误类型定义
    ├── config.rs                 # 配置结构
    ├── entity/                   # sea-orm 数据库实体
    │   ├── mod.rs
    │   ├── vd_roles.rs           # 角色表
    │   ├── vd_role_permissions.rs # 角色权限表
    │   ├── vd_role_users.rs      # 角色用户关联表
    │   ├── vd_groups.rs          # 用户组表
    │   ├── vd_group_roles.rs     # 组角色关联表
    │   ├── vd_group_users.rs     # 组用户关联表
    │   ├── vd_sso_providers.rs   # SSO 提供商表
    │   └── vd_sso_user_mappings.rs # SSO 用户映射表
    ├── rbac/
    │   ├── mod.rs
    │   ├── engine.rs             # RBAC 权限检查引擎
    │   ├── cache.rs              # DashMap 权限缓存
    │   └── resources.rs          # 资源类型定义
    ├── sso/
    │   ├── mod.rs
    │   ├── oidc.rs               # OIDC 认证实现
    │   ├── ldap.rs               # LDAP 认证实现
    │   └── provider.rs           # SSO 提供商管理
    ├── meta/
    │   └── mod.rs                # API 请求/响应结构
    ├── service/
    │   ├── mod.rs
    │   ├── role.rs               # 角色业务逻辑
    │   └── group.rs              # 用户组业务逻辑
    └── handler/
        ├── mod.rs
        ├── roles.rs              # 角色管理 API
        ├── groups.rs             # 用户组管理 API
        ├── users.rs              # 用户查询 API
        ├── resources.rs          # 资源定义 API
        └── sso.rs                # SSO API
```

### 数据库表结构

#### vd_roles - 角色表
```sql
CREATE TABLE vd_roles (
    id VARCHAR(27) PRIMARY KEY,      -- KSUID
    org_id VARCHAR(100) NOT NULL,
    name VARCHAR(100) NOT NULL,
    display_name VARCHAR(200),
    description TEXT,
    is_system BOOLEAN DEFAULT FALSE,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    UNIQUE(org_id, name)
);
```

#### vd_role_permissions - 角色权限表
```sql
CREATE TABLE vd_role_permissions (
    id VARCHAR(27) PRIMARY KEY,
    role_id VARCHAR(27) NOT NULL,
    org_id VARCHAR(100) NOT NULL,
    object VARCHAR(500) NOT NULL,     -- 格式: "resource:entity"
    permission VARCHAR(50) NOT NULL,   -- AllowAll/AllowList/AllowGet/AllowPost/AllowPut/AllowDelete
    created_at BIGINT NOT NULL,
    UNIQUE(role_id, object, permission)
);
```

#### vd_role_users - 角色用户关联表
```sql
CREATE TABLE vd_role_users (
    id VARCHAR(27) PRIMARY KEY,
    role_id VARCHAR(27) NOT NULL,
    org_id VARCHAR(100) NOT NULL,
    user_email VARCHAR(100) NOT NULL,
    created_at BIGINT NOT NULL,
    UNIQUE(role_id, org_id, user_email)
);
```

#### vd_groups - 用户组表
```sql
CREATE TABLE vd_groups (
    id VARCHAR(27) PRIMARY KEY,
    org_id VARCHAR(100) NOT NULL,
    name VARCHAR(100) NOT NULL,
    display_name VARCHAR(200),
    description TEXT,
    external_id VARCHAR(256),         -- SSO 同步用
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    UNIQUE(org_id, name)
);
```

#### vd_group_roles - 组角色关联表
```sql
CREATE TABLE vd_group_roles (
    id VARCHAR(27) PRIMARY KEY,
    group_id VARCHAR(27) NOT NULL,
    org_id VARCHAR(100) NOT NULL,
    role_id VARCHAR(27) NOT NULL,
    created_at BIGINT NOT NULL,
    UNIQUE(group_id, role_id)
);
```

#### vd_group_users - 组用户关联表
```sql
CREATE TABLE vd_group_users (
    id VARCHAR(27) PRIMARY KEY,
    group_id VARCHAR(27) NOT NULL,
    org_id VARCHAR(100) NOT NULL,
    user_email VARCHAR(100) NOT NULL,
    created_at BIGINT NOT NULL,
    UNIQUE(group_id, user_email)
);
```

#### vd_sso_providers - SSO 提供商表
```sql
CREATE TABLE vd_sso_providers (
    id VARCHAR(27) PRIMARY KEY,
    org_id VARCHAR(100) NOT NULL,
    provider_type VARCHAR(50) NOT NULL,  -- oidc/ldap
    name VARCHAR(100) NOT NULL,
    is_enabled BOOLEAN DEFAULT TRUE,
    is_default BOOLEAN DEFAULT FALSE,
    config_json TEXT NOT NULL,            -- 加密的配置
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    UNIQUE(org_id, name)
);
```

#### vd_sso_user_mappings - SSO 用户映射表
```sql
CREATE TABLE vd_sso_user_mappings (
    id VARCHAR(27) PRIMARY KEY,
    provider_id VARCHAR(27) NOT NULL,
    external_id VARCHAR(256) NOT NULL,
    user_email VARCHAR(100) NOT NULL,
    external_groups TEXT,                 -- JSON 数组
    last_sync_at BIGINT,
    created_at BIGINT NOT NULL,
    UNIQUE(provider_id, external_id)
);
```

## 权限模型

### 权限类型 (6种)

| 权限名 | 前端显示 | HTTP 方法映射 |
|--------|----------|--------------|
| AllowAll | All | 所有权限 |
| AllowList | List | GET (列表) |
| AllowGet | Get | GET (详情) |
| AllowPost | Create | POST |
| AllowPut | Update | PUT |
| AllowDelete | Delete | DELETE |

### 资源类型

```rust
pub enum ResourceType {
    Stream,           // Streams (父级)
    Logs,             // -> 日志流
    Metrics,          // -> 指标流
    Traces,           // -> 追踪流
    Index,            // -> 索引
    Functions,        // Functions
    DashboardFolders, // Dashboard Folders
    Dashboard,        // -> 具体仪表板
    Templates,        // Templates
    Destinations,     // Destinations
    Alerts,           // Alerts
    AlertFolders,     // Alert Folders
    Organizations,    // Organizations
    Pipeline,         // Pipelines
    Reports,          // Reports
    SavedViews,       // Saved Views
    Groups,           // Groups
    Roles,            // Roles
    ServiceAccounts,  // Service Accounts
    ActionScripts,    // Action Scripts
    CipherKeys,       // Cipher Keys
}
```

### 权限对象命名规则

```
格式: {resource}:{entity}

示例:
- "logs:_all_org123"              # 所有日志流 (_all_ 前缀表示全部)
- "logs:my_app_logs"              # 特定日志流
- "dashboard:folder1/dash1"       # 文件夹下的仪表板
- "alert:alertfolder/alert1"      # 告警
- "stream:_all_org123"            # 所有 streams 类型
```

### 权限检查流程

```
HTTP 请求进入
    ↓
1. 提取用户信息和请求的资源/操作
    ↓
2. 是 Root 用户? → 直接放行
    ↓ 否
3. 检查缓存 → 命中则返回
    ↓ 未命中
4. 获取用户有效角色
   ├── 用户直接分配的角色 (vd_role_users)
   └── 用户所属组的角色 (vd_group_users → vd_group_roles)
    ↓
5. 对每个角色，检查权限 (vd_role_permissions)
   ├── 检查精确匹配: object = "logs:my_stream"
   ├── 检查通配符: object = "logs:_all_{org_id}"
   └── 检查 AllowAll 权限
    ↓
6. 缓存结果并返回
```

## API 端点

### 角色管理

| 方法 | 端点 | 说明 |
|------|------|------|
| POST | `/{org_id}/roles` | 创建角色 |
| GET | `/{org_id}/roles` | 列出角色 |
| PUT | `/{org_id}/roles/{role_id}` | 更新角色 (权限+用户) |
| DELETE | `/{org_id}/roles/{role_id}` | 删除角色 |
| GET | `/{org_id}/roles/{role_id}/permissions/{resource}` | 获取角色权限 |
| GET | `/{org_id}/roles/{role_id}/users` | 获取角色的用户 |

### 用户组管理

| 方法 | 端点 | 说明 |
|------|------|------|
| POST | `/{org_id}/groups` | 创建用户组 |
| GET | `/{org_id}/groups` | 列出用户组 |
| GET | `/{org_id}/groups/{group_name}` | 获取组详情 |
| PUT | `/{org_id}/groups/{group_name}` | 更新组 (角色+用户) |
| DELETE | `/{org_id}/groups/{group_name}` | 删除组 |

### 用户查询

| 方法 | 端点 | 说明 |
|------|------|------|
| GET | `/{org_id}/users/{user_email}/roles` | 获取用户角色 |
| GET | `/{org_id}/users/{user_email}/groups` | 获取用户所属组 |

### 资源定义

| 方法 | 端点 | 说明 |
|------|------|------|
| GET | `/{org_id}/resources` | 获取所有资源类型定义 |

### SSO 管理

| 方法 | 端点 | 说明 |
|------|------|------|
| GET | `/{org_id}/sso/providers` | 列出 SSO 提供商 |
| POST | `/{org_id}/sso/providers/oidc` | 创建 OIDC 提供商 |
| POST | `/{org_id}/sso/providers/ldap` | 创建 LDAP 提供商 |
| PUT | `/{org_id}/sso/providers/{provider_id}` | 更新 SSO 提供商 |
| DELETE | `/{org_id}/sso/providers/{provider_id}` | 删除 SSO 提供商 |
| GET | `/{org_id}/sso/login` | 发起 SSO 登录 |
| GET | `/{org_id}/sso/callback` | SSO 回调 |

## API 请求/响应格式

### 创建角色

```json
// POST /{org_id}/roles
// Request:
{ "role": "custom_role_name" }

// Response:
{ "message": "Role created successfully" }
```

### 更新角色

```json
// PUT /{org_id}/roles/{role_id}
// Request:
{
  "add": [
    { "object": "logs:_all_org123", "permission": "AllowGet" },
    { "object": "dashboard:folder1/dash1", "permission": "AllowAll" }
  ],
  "remove": [
    { "object": "metrics:_all_org123", "permission": "AllowList" }
  ],
  "add_users": ["user@example.com"],
  "remove_users": ["olduser@example.com"]
}

// Response:
{ "message": "Role updated successfully" }
```

### 更新用户组

```json
// PUT /{org_id}/groups/{group_name}
// Request:
{
  "add_roles": ["custom_role", "viewer"],
  "remove_roles": ["editor"],
  "add_users": ["user@example.com"],
  "remove_users": ["old@example.com"]
}

// Response:
{ "message": "Group updated successfully" }
```

### 获取资源定义

```json
// GET /{org_id}/resources
// Response:
[
  {
    "key": "stream",
    "name": "Streams",
    "has_entities": true,
    "children": ["logs", "metrics", "traces", "index"]
  },
  {
    "key": "dashboard",
    "name": "Dashboards",
    "has_entities": true,
    "parent": "dfolder"
  }
  // ... 更多资源类型
]
```

## SSO 配置

### OIDC 配置

```rust
pub struct OIDCConfig {
    pub issuer_url: String,
    pub client_id: String,
    pub client_secret: String,  // 加密存储
    pub scopes: Vec<String>,
    pub redirect_uri: String,
    pub email_claim: String,
    pub name_claim: String,
    pub groups_claim: Option<String>,
    pub group_role_mappings: HashMap<String, String>,
}
```

### LDAP 配置

```rust
pub struct LDAPConfig {
    pub server_url: String,
    pub bind_dn: String,
    pub bind_password: String,  // 加密存储
    pub user_base_dn: String,
    pub user_filter: String,
    pub user_attr_email: String,
    pub user_attr_name: String,
    pub group_base_dn: String,
    pub group_filter: String,
    pub group_attr_name: String,
    pub group_role_mappings: HashMap<String, String>,
}
```

## 主项目集成点

### 修改的文件

| 文件 | 修改内容 |
|------|----------|
| `Cargo.toml` | 添加 `visdata` feature 和依赖 |
| `src/handler/http/auth/validator.rs` | 添加 visdata 权限检查分支 |
| `src/handler/http/router/mod.rs` | 条件编译注册 visdata 路由 |
| `src/main.rs` | 添加 visdata 初始化 |

### Feature Flag 互斥

- 当启用 `visdata` 时，使用 VisData 的 RBAC/SSO handlers
- 当未启用 `visdata` 时，使用现有的 `authz::fga` handlers
- `visdata` 和 `enterprise` 可以同时启用，但 RBAC 路由会使用 visdata

## 依赖

```toml
[dependencies]
# SSO - OIDC
openidconnect = "3.5"
oauth2 = "4.4"
jsonwebtoken = "9.3"

# SSO - LDAP
ldap3 = "0.11"

# Database
sea-orm = { workspace = true }

# Web framework
actix-web = { workspace = true }
actix-web-httpauth = "0.8"

# Cache
dashmap = { workspace = true }

# Encryption
aes-gcm = "0.10"
```

## 后续开发计划

### 待实现功能

1. **SSO Handler 完整实现**
   - SSO 登录流程
   - SSO 回调处理
   - 用户自动创建/同步

2. **数据库迁移**
   - 创建 sea-orm 迁移脚本
   - 自动创建表结构

3. **权限缓存优化**
   - TTL 过期清理
   - 分布式缓存同步

4. **管理界面**
   - SSO 提供商配置 UI
   - 角色权限矩阵 UI（复用现有企业版 UI）

## License

GNU Affero General Public License v3.0 (AGPL-3.0)

Copyright 2025 VisData Inc.
