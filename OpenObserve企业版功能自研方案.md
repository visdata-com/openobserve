# OpenObserve 企业版功能自研方案与可行性分析

**文档版本:** 1.0
**编写日期:** 2025-01-27
**项目:** OpenObserve 企业版功能自研
**作者:** 技术团队

---

## 📑 目录

1. [项目概述](#1-项目概述)
2. [企业版功能分析](#2-企业版功能分析)
3. [功能模块可行性评估](#3-功能模块可行性评估)
4. [实施路线图](#4-实施路线图)
5. [详细实现方案](#5-详细实现方案)
   - 5.1 [SSO/OAuth2认证](#51-ssooauth2认证)
   - 5.2 [RBAC权限管理](#52-rbac权限管理)
   - 5.3 [加密密钥管理](#53-加密密钥管理)
   - 5.4 [审计日志系统](#54-审计日志系统)
   - 5.5 [许可证管理](#55-许可证管理)
   - 5.6 [AI功能集成](#56-ai功能集成)
6. [成本效益分析](#6-成本效益分析)
7. [技术栈与依赖](#7-技术栈与依赖)
8. [风险评估与应对](#8-风险评估与应对)
9. [总结与建议](#9-总结与建议)

---

## 1. 项目概述

### 1.1 背景

OpenObserve是一个开源的可观测性平台,提供日志、指标、追踪等功能。当前开源版本提供核心功能,但企业版功能(SSO、RBAC、加密等)依赖于闭源的`o2_enterprise`库。

### 1.2 目标

本方案旨在:
- ✅ 自主实现企业版核心功能
- ✅ 降低对第三方商业库的依赖
- ✅ 满足企业客户的安全与合规需求
- ✅ 为产品商业化提供技术基础

### 1.3 范围

**包含功能:**
- SSO/OAuth2单点登录
- RBAC基于角色的访问控制
- 数据加密与密钥管理
- 审计日志系统
- 许可证管理
- AI功能集成(可选)

**不包含:**
- 超集群队列系统(复杂度过高)
- 完整AI模型训练(建议使用第三方API)

---

## 2. 企业版功能分析

### 2.1 当前企业版功能概览

基于对OpenObserve代码库的深入分析,企业版主要包含以下模块:

| 功能模块 | 描述 | 企业版依赖 |
|---------|------|-----------|
| **认证系统** | SSO、SAML、LDAP、Dex集成 | `o2_enterprise::enterprise::auth` |
| **权限管理** | OpenFGA、RBAC、细粒度权限 | `o2_openfga`, `o2_dex` |
| **加密功能** | AES-256-SIV密钥管理 | `o2_enterprise::enterprise::cipher` |
| **审计日志** | 操作审计、合规记录 | `o2_enterprise::enterprise::common::auditor` |
| **AI功能** | AI聊天、推荐引擎 | `o2_enterprise::enterprise::ai` |
| **许可证** | License验证、使用量限制 | `o2_enterprise::enterprise::license` |
| **超集群** | 跨集群消息队列 | `o2_enterprise::enterprise::super_cluster` |
| **自动化** | Action系统、脚本部署 | `o2_enterprise::enterprise::actions` |

### 2.2 代码集成点分析

通过搜索代码库,发现企业版集成点分布:

```
企业版使用统计:
- cfg(feature = "enterprise"): 877处
- o2_enterprise调用: 312处
- 涉及文件: 30+个模块
```

**关键集成位置:**
- `src/handler/http/auth/` - 认证处理
- `src/service/db/` - 数据库操作
- `src/job/` - 后台任务
- `src/cipher/` - 加密功能
- `src/super_cluster_queue/` - 集群通信

### 2.3 依赖关系图

```
┌─────────────────────────────────────────────────────────┐
│                   OpenObserve 核心                       │
│  (开源,包含日志/指标/追踪/查询等核心功能)                  │
└────────────────────┬────────────────────────────────────┘
                     │
        ┌────────────┴────────────┐
        │                         │
┌───────▼────────┐      ┌─────────▼──────────┐
│  开源版功能     │      │   企业版功能        │
│                │      │  (o2_enterprise)   │
│ - 基础认证(JWT)│      │ - SSO/SAML        │
│ - MySQL/PG支持 │      │ - RBAC/OpenFGA    │
│ - 基础API      │      │ - 加密/审计        │
│ - Dashboard    │      │ - AI/推荐          │
└────────────────┘      │ - 许可证管理       │
                        │ - 超集群支持       │
                        └────────────────────┘
```

---

## 3. 功能模块可行性评估

### 3.1 评估矩阵

| 功能模块 | 复杂度 | 实现周期 | 可行性 | ROI | 优先级 |
|---------|-------|---------|-------|-----|-------|
| **SSO/OAuth2** | ⭐⭐⭐ | 2-3周 | ✅ 高 | 🔥🔥🔥 | **P0** |
| **RBAC权限** | ⭐⭐⭐⭐ | 3-4周 | ✅ 高 | 🔥🔥🔥 | **P0** |
| **加密管理** | ⭐⭐⭐ | 2周 | ✅ 高 | 🔥🔥 | **P1** |
| **审计日志** | ⭐⭐ | 1-2周 | ✅ 高 | 🔥🔥 | **P1** |
| **许可证** | ⭐⭐ | 1周 | ✅ 高 | 🔥 | **P2** |
| **AI聊天** | ⭐⭐⭐⭐⭐ | 6-8周 | ⚠️ 中 | 🔥 | **P3** |
| **查询管理** | ⭐⭐⭐⭐ | 4-6周 | ✅ 中 | 🔥🔥 | **P2** |
| **超集群** | ⭐⭐⭐⭐⭐ | 8-12周 | ⚠️ 低 | 🔥 | **P4** |
| **自动化操作** | ⭐⭐⭐⭐ | 4-5周 | ✅ 中 | 🔥 | **P3** |

**说明:**
- **复杂度:** ⭐越多越复杂
- **可行性:** ✅高 / ⚠️中 / ❌低
- **ROI:** 投资回报率,🔥越多价值越高
- **优先级:** P0(最高) → P4(最低)

### 3.2 详细评估

#### P0功能 - 必须实现

**SSO/OAuth2认证**
- **复杂度:** 中等
- **理由:** 企业客户的核心需求,集成现有OAuth2库即可
- **依赖:** `oauth2` crate(成熟稳定)
- **风险:** 低,技术成熟

**RBAC权限管理**
- **复杂度:** 较高
- **理由:** 安全核心,需要仔细设计
- **方案:** 简化版RBAC,不必完全实现OpenFGA
- **风险:** 中,需要充分测试

#### P1功能 - 强烈推荐

**加密密钥管理**
- **复杂度:** 中等
- **理由:** 数据安全基础
- **依赖:** `aes-siv` crate(已在项目中)
- **风险:** 低

**审计日志系统**
- **复杂度:** 简单
- **理由:** 合规要求
- **实现:** 中间件拦截+数据库记录
- **风险:** 低

#### P2-P4功能 - 可选实现

根据实际需求和资源情况决定。

---

## 4. 实施路线图

### 4.1 整体时间规划

```
┌─────────────────────────────────────────────────────────────┐
│                    总体时间轴(16周)                           │
└─────────────────────────────────────────────────────────────┘

第1-8周: Phase 1 - 核心认证与权限
├─ Week 1-3:  OAuth2/SSO实现
├─ Week 4-7:  RBAC权限系统
└─ Week 8:    集成测试与文档

第9-12周: Phase 2 - 数据安全
├─ Week 9-10:  加密密钥管理
├─ Week 11-12: 审计日志系统
└─ Week 12:    安全测试

第13-14周: Phase 3 - 商业化功能
└─ Week 13-14: 许可证管理系统

第15-16周: Phase 4 - 集成与发布
├─ Week 15:    全面集成测试
└─ Week 16:    文档完善与发布准备
```

### 4.2 里程碑

| 里程碑 | 时间点 | 交付物 |
|-------|-------|-------|
| **M1: 认证完成** | Week 3 | OAuth2登录可用 |
| **M2: 权限完成** | Week 7 | RBAC系统上线 |
| **M3: 安全完成** | Week 12 | 加密+审计就绪 |
| **M4: 商业就绪** | Week 14 | 许可证系统 |
| **M5: 正式发布** | Week 16 | 企业版1.0 |

### 4.3 人员配置建议

| 角色 | 人数 | 技能要求 |
|-----|------|---------|
| **Rust开发** | 2-3人 | 熟悉Rust、Actix-Web |
| **安全专家** | 1人 | 了解OAuth2、加密算法 |
| **测试工程师** | 1人 | 安全测试、集成测试 |
| **技术文档** | 1人 | 技术写作 |

**总计:** 5-6人团队,3-4个月完成核心功能

---

## 5. 详细实现方案

### 5.1 SSO/OAuth2认证

#### 5.1.1 架构设计

```
┌──────────────┐         ┌──────────────┐         ┌──────────────┐
│   浏览器      │         │  OpenObserve │         │ OAuth2提供商  │
│              │         │              │         │  (GitHub等)  │
└──────┬───────┘         └──────┬───────┘         └──────┬───────┘
       │                        │                        │
       │ 1. 访问/login         │                        │
       │─────────────────────>│                        │
       │                        │                        │
       │ 2. 重定向到OAuth2      │                        │
       │<─────────────────────│                        │
       │                        │                        │
       │ 3. 授权请求            │                        │
       │────────────────────────────────────────────>│
       │                        │                        │
       │ 4. 授权码              │                        │
       │<────────────────────────────────────────────│
       │                        │                        │
       │ 5. 回调+授权码         │                        │
       │─────────────────────>│                        │
       │                        │ 6. 交换token           │
       │                        │───────────────────────>│
       │                        │ 7. Access Token        │
       │                        │<───────────────────────│
       │                        │ 8. 获取用户信息         │
       │                        │───────────────────────>│
       │                        │ 9. 用户信息             │
       │                        │<───────────────────────│
       │ 10. JWT令牌            │                        │
       │<─────────────────────│                        │
       │                        │                        │
```

#### 5.1.2 核心代码实现

**配置结构:**

```rust
// src/config/src/auth.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OAuth2Config {
    pub client_id: String,
    pub client_secret: String,
    pub auth_url: String,
    pub token_url: String,
    pub redirect_url: String,
    pub user_info_url: String,
    pub scopes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AuthConfig {
    // 现有JWT配置
    pub jwt_secret: String,
    pub jwt_expiry_hours: i64,

    // OAuth2配置
    pub oauth2_enabled: bool,
    pub oauth2_providers: HashMap<String, OAuth2Config>,
}

impl AuthConfig {
    pub fn get_oauth2_provider(&self, name: &str) -> Option<&OAuth2Config> {
        self.oauth2_providers.get(name)
    }
}
```

**OAuth2服务实现:**

```rust
// src/service/auth/oauth2.rs

use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken,
    RedirectUrl, Scope, TokenResponse, TokenUrl,
    basic::BasicClient,
    reqwest::async_http_client,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct UserInfo {
    pub email: String,
    pub name: Option<String>,
    pub avatar: Option<String>,
}

pub struct OAuth2Service {
    client: BasicClient,
    user_info_url: String,
}

impl OAuth2Service {
    pub fn new(config: OAuth2Config) -> Result<Self> {
        let client = BasicClient::new(
            ClientId::new(config.client_id),
            Some(ClientSecret::new(config.client_secret)),
            AuthUrl::new(config.auth_url)
                .context("Invalid auth URL")?,
            Some(TokenUrl::new(config.token_url)
                .context("Invalid token URL")?),
        )
        .set_redirect_uri(
            RedirectUrl::new(config.redirect_url)
                .context("Invalid redirect URL")?
        );

        Ok(Self {
            client,
            user_info_url: config.user_info_url,
        })
    }

    /// 生成授权URL
    pub fn get_authorization_url(&self, scopes: &[String]) -> (String, String) {
        let mut auth_request = self.client.authorize_url(CsrfToken::new_random);

        for scope in scopes {
            auth_request = auth_request.add_scope(Scope::new(scope.clone()));
        }

        let (url, csrf_token) = auth_request.url();
        (url.to_string(), csrf_token.secret().clone())
    }

    /// 交换授权码获取token
    pub async fn exchange_code(&self, code: &str) -> Result<String> {
        let token = self.client
            .exchange_code(AuthorizationCode::new(code.to_string()))
            .request_async(async_http_client)
            .await
            .context("Failed to exchange authorization code")?;

        Ok(token.access_token().secret().clone())
    }

    /// 获取用户信息
    pub async fn get_user_info(&self, access_token: &str) -> Result<UserInfo> {
        let client = reqwest::Client::new();
        let response = client
            .get(&self.user_info_url)
            .bearer_auth(access_token)
            .send()
            .await
            .context("Failed to fetch user info")?;

        let user_info: UserInfo = response
            .json()
            .await
            .context("Failed to parse user info")?;

        Ok(user_info)
    }
}
```

**HTTP处理器:**

```rust
// src/handler/http/auth/oauth2.rs

use actix_web::{web, HttpResponse, Result as ActixResult};
use serde::{Deserialize, Serialize};
use crate::service::auth::oauth2::{OAuth2Service, UserInfo};
use crate::service::users;
use config::get_config;

#[derive(Deserialize)]
pub struct CallbackParams {
    code: String,
    state: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: UserInfo,
}

/// 发起OAuth2登录
pub async fn oauth2_login(
    provider: web::Path<String>,
) -> ActixResult<HttpResponse> {
    let config = get_config();
    let oauth2_config = config.auth.get_oauth2_provider(&provider)
        .ok_or_else(|| actix_web::error::ErrorNotFound("Provider not found"))?;

    let service = OAuth2Service::new(oauth2_config.clone())
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let (auth_url, csrf_token) = service.get_authorization_url(&oauth2_config.scopes);

    // TODO: 存储csrf_token到session中验证

    Ok(HttpResponse::Found()
        .append_header(("Location", auth_url))
        .finish())
}

/// OAuth2回调处理
pub async fn oauth2_callback(
    provider: web::Path<String>,
    params: web::Query<CallbackParams>,
) -> ActixResult<HttpResponse> {
    let config = get_config();
    let oauth2_config = config.auth.get_oauth2_provider(&provider)
        .ok_or_else(|| actix_web::error::ErrorNotFound("Provider not found"))?;

    // TODO: 验证csrf_token

    let service = OAuth2Service::new(oauth2_config.clone())
        .map_err(actix_web::error::ErrorInternalServerError)?;

    // 交换授权码
    let access_token = service.exchange_code(&params.code)
        .await
        .map_err(actix_web::error::ErrorUnauthorized)?;

    // 获取用户信息
    let user_info = service.get_user_info(&access_token)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    // 创建或更新用户
    let user = users::create_or_update_from_oauth(&user_info)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    // 生成JWT
    let jwt = crate::service::auth::generate_jwt(&user)
        .map_err(actix_web::error::ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().json(LoginResponse {
        token: jwt,
        user: user_info,
    }))
}
```

**路由注册:**

```rust
// src/handler/http/router/auth.rs

pub fn configure_auth_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/auth/oauth2")
            .route("/{provider}/login", web::get().to(oauth2::oauth2_login))
            .route("/{provider}/callback", web::get().to(oauth2::oauth2_callback))
    );
}
```

#### 5.1.3 配置示例

```yaml
# config/oauth2.yaml

auth:
  oauth2_enabled: true
  oauth2_providers:
    github:
      client_id: "your-github-client-id"
      client_secret: "your-github-client-secret"
      auth_url: "https://github.com/login/oauth/authorize"
      token_url: "https://github.com/login/oauth/access_token"
      redirect_url: "http://localhost:5080/api/auth/oauth2/github/callback"
      user_info_url: "https://api.github.com/user"
      scopes:
        - "user:email"
        - "read:user"

    google:
      client_id: "your-google-client-id"
      client_secret: "your-google-client-secret"
      auth_url: "https://accounts.google.com/o/oauth2/v2/auth"
      token_url: "https://oauth2.googleapis.com/token"
      redirect_url: "http://localhost:5080/api/auth/oauth2/google/callback"
      user_info_url: "https://www.googleapis.com/oauth2/v2/userinfo"
      scopes:
        - "openid"
        - "email"
        - "profile"
```

#### 5.1.4 测试计划

**单元测试:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_oauth2_service_creation() {
        let config = OAuth2Config {
            client_id: "test".to_string(),
            client_secret: "secret".to_string(),
            auth_url: "https://example.com/auth".to_string(),
            token_url: "https://example.com/token".to_string(),
            redirect_url: "http://localhost/callback".to_string(),
            user_info_url: "https://example.com/user".to_string(),
            scopes: vec!["email".to_string()],
        };

        let service = OAuth2Service::new(config);
        assert!(service.is_ok());
    }

    #[test]
    fn test_authorization_url_generation() {
        // ... 测试授权URL生成
    }
}
```

**集成测试:**
- GitHub OAuth2流程测试
- Google OAuth2流程测试
- 用户创建与更新测试
- JWT生成与验证测试

---

### 5.2 RBAC权限管理

#### 5.2.1 权限模型设计

**三层权限模型:**

```
组织层级 (Organization)
    ├─ 角色 (Roles)
    │   ├─ Admin (管理员)
    │   ├─ Editor (编辑者)
    │   └─ Viewer (查看者)
    │
    └─ 资源 (Resources)
        ├─ 流 (Streams)
        ├─ 仪表板 (Dashboards)
        ├─ 告警 (Alerts)
        └─ 用户 (Users)
```

**权限定义:**

```rust
// src/service/rbac/permission.rs

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceType {
    Organization,
    Stream,
    Dashboard,
    Alert,
    User,
    Function,
    Pipeline,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Action {
    Read,
    Write,
    Delete,
    Admin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Permission {
    pub resource_type: ResourceType,
    pub resource_id: Option<String>, // None表示所有资源
    pub action: Action,
}

impl Permission {
    pub fn new(resource_type: ResourceType, action: Action) -> Self {
        Self {
            resource_type,
            resource_id: None,
            action,
        }
    }

    pub fn for_resource(resource_type: ResourceType, resource_id: String, action: Action) -> Self {
        Self {
            resource_type,
            resource_id: Some(resource_id),
            action,
        }
    }

    /// 检查是否匹配
    pub fn matches(&self, other: &Permission) -> bool {
        if self.resource_type != other.resource_type {
            return false;
        }

        if self.action != other.action && self.action != Action::Admin {
            return false;
        }

        // 如果当前权限没有指定resource_id,表示对所有资源有权限
        if self.resource_id.is_none() {
            return true;
        }

        // 检查资源ID是否匹配
        self.resource_id == other.resource_id
    }
}
```

**角色定义:**

```rust
// src/service/rbac/role.rs

use serde::{Deserialize, Serialize};
use super::permission::{Permission, ResourceType, Action};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    pub id: String,
    pub name: String,
    pub description: String,
    pub permissions: Vec<Permission>,
    pub is_system: bool, // 系统预定义角色不可删除
    pub created_at: i64,
    pub updated_at: i64,
}

impl Role {
    /// 创建管理员角色
    pub fn admin() -> Self {
        Self {
            id: "admin".to_string(),
            name: "Administrator".to_string(),
            description: "Full access to all resources".to_string(),
            permissions: vec![
                Permission::new(ResourceType::Organization, Action::Admin),
                Permission::new(ResourceType::Stream, Action::Admin),
                Permission::new(ResourceType::Dashboard, Action::Admin),
                Permission::new(ResourceType::Alert, Action::Admin),
                Permission::new(ResourceType::User, Action::Admin),
            ],
            is_system: true,
            created_at: chrono::Utc::now().timestamp(),
            updated_at: chrono::Utc::now().timestamp(),
        }
    }

    /// 创建编辑者角色
    pub fn editor() -> Self {
        Self {
            id: "editor".to_string(),
            name: "Editor".to_string(),
            description: "Can read and write resources".to_string(),
            permissions: vec![
                Permission::new(ResourceType::Stream, Action::Write),
                Permission::new(ResourceType::Dashboard, Action::Write),
                Permission::new(ResourceType::Alert, Action::Write),
                Permission::new(ResourceType::User, Action::Read),
            ],
            is_system: true,
            created_at: chrono::Utc::now().timestamp(),
            updated_at: chrono::Utc::now().timestamp(),
        }
    }

    /// 创建查看者角色
    pub fn viewer() -> Self {
        Self {
            id: "viewer".to_string(),
            name: "Viewer".to_string(),
            description: "Read-only access to resources".to_string(),
            permissions: vec![
                Permission::new(ResourceType::Stream, Action::Read),
                Permission::new(ResourceType::Dashboard, Action::Read),
                Permission::new(ResourceType::Alert, Action::Read),
            ],
            is_system: true,
            created_at: chrono::Utc::now().timestamp(),
            updated_at: chrono::Utc::now().timestamp(),
        }
    }

    /// 检查角色是否有指定权限
    pub fn has_permission(&self, required: &Permission) -> bool {
        self.permissions.iter().any(|p| p.matches(required))
    }
}
```

#### 5.2.2 数据库Schema

```sql
-- 角色表
CREATE TABLE roles (
    id VARCHAR(255) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    permissions JSON NOT NULL,
    is_system BOOLEAN DEFAULT FALSE,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    INDEX idx_name (name)
);

-- 用户角色关联表
CREATE TABLE user_roles (
    id VARCHAR(255) PRIMARY KEY,
    user_email VARCHAR(255) NOT NULL,
    org_id VARCHAR(255) NOT NULL,
    role_id VARCHAR(255) NOT NULL,
    created_at BIGINT NOT NULL,
    created_by VARCHAR(255),
    UNIQUE KEY unique_user_org_role (user_email, org_id, role_id),
    INDEX idx_user_email (user_email),
    INDEX idx_org_id (org_id)
);

-- 资源权限表(可选,用于细粒度控制)
CREATE TABLE resource_permissions (
    id VARCHAR(255) PRIMARY KEY,
    resource_type VARCHAR(50) NOT NULL,
    resource_id VARCHAR(255) NOT NULL,
    role_id VARCHAR(255) NOT NULL,
    action VARCHAR(50) NOT NULL,
    created_at BIGINT NOT NULL,
    INDEX idx_resource (resource_type, resource_id),
    INDEX idx_role (role_id)
);
```

#### 5.2.3 RBAC服务实现

```rust
// src/service/rbac/service.rs

use anyhow::{Context, Result};
use sea_orm::*;
use super::{Role, Permission, ResourceType, Action};
use crate::infra::db::ORM_CLIENT;

pub struct RbacService;

impl RbacService {
    /// 初始化系统角色
    pub async fn init_system_roles() -> Result<()> {
        let db = ORM_CLIENT.get().context("Database not initialized")?;

        let roles = vec![
            Role::admin(),
            Role::editor(),
            Role::viewer(),
        ];

        for role in roles {
            // 检查是否已存在
            let existing = entity::roles::Entity::find_by_id(&role.id)
                .one(db)
                .await?;

            if existing.is_none() {
                let model = entity::roles::ActiveModel {
                    id: Set(role.id.clone()),
                    name: Set(role.name.clone()),
                    description: Set(role.description.clone()),
                    permissions: Set(serde_json::to_value(&role.permissions)?),
                    is_system: Set(role.is_system),
                    created_at: Set(role.created_at),
                    updated_at: Set(role.updated_at),
                };

                entity::roles::Entity::insert(model)
                    .exec(db)
                    .await?;
            }
        }

        Ok(())
    }

    /// 分配角色给用户
    pub async fn assign_role(
        user_email: &str,
        org_id: &str,
        role_id: &str,
        assigned_by: &str,
    ) -> Result<()> {
        let db = ORM_CLIENT.get().context("Database not initialized")?;

        // 检查角色是否存在
        let role = entity::roles::Entity::find_by_id(role_id)
            .one(db)
            .await?
            .context("Role not found")?;

        // 创建用户角色关联
        let id = format!("{}:{}:{}", user_email, org_id, role_id);
        let model = entity::user_roles::ActiveModel {
            id: Set(id),
            user_email: Set(user_email.to_string()),
            org_id: Set(org_id.to_string()),
            role_id: Set(role_id.to_string()),
            created_at: Set(chrono::Utc::now().timestamp()),
            created_by: Set(Some(assigned_by.to_string())),
        };

        entity::user_roles::Entity::insert(model)
            .on_conflict(
                OnConflict::columns([
                    entity::user_roles::Column::UserEmail,
                    entity::user_roles::Column::OrgId,
                    entity::user_roles::Column::RoleId,
                ])
                .do_nothing()
                .to_owned()
            )
            .exec(db)
            .await?;

        Ok(())
    }

    /// 获取用户在组织中的角色
    pub async fn get_user_roles(
        user_email: &str,
        org_id: &str,
    ) -> Result<Vec<Role>> {
        let db = ORM_CLIENT.get().context("Database not initialized")?;

        let user_roles = entity::user_roles::Entity::find()
            .filter(entity::user_roles::Column::UserEmail.eq(user_email))
            .filter(entity::user_roles::Column::OrgId.eq(org_id))
            .all(db)
            .await?;

        let mut roles = Vec::new();
        for ur in user_roles {
            if let Some(role_model) = entity::roles::Entity::find_by_id(&ur.role_id)
                .one(db)
                .await?
            {
                let permissions: Vec<Permission> = serde_json::from_value(role_model.permissions)?;
                roles.push(Role {
                    id: role_model.id,
                    name: role_model.name,
                    description: role_model.description,
                    permissions,
                    is_system: role_model.is_system,
                    created_at: role_model.created_at,
                    updated_at: role_model.updated_at,
                });
            }
        }

        Ok(roles)
    }

    /// 检查用户是否有指定权限
    pub async fn check_permission(
        user_email: &str,
        org_id: &str,
        required: &Permission,
    ) -> Result<bool> {
        let roles = Self::get_user_roles(user_email, org_id).await?;

        Ok(roles.iter().any(|role| role.has_permission(required)))
    }
}
```

#### 5.2.4 RBAC中间件

```rust
// src/handler/http/middleware/rbac.rs

use actix_web::{
    Error, HttpMessage, HttpRequest,
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    error::ErrorForbidden,
};
use futures_util::future::{ready, Ready};
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use crate::service::rbac::{Permission, RbacService, ResourceType, Action};

pub struct RbacMiddleware {
    pub resource_type: ResourceType,
    pub action: Action,
}

impl RbacMiddleware {
    pub fn new(resource_type: ResourceType, action: Action) -> Self {
        Self {
            resource_type,
            action,
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for RbacMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = RbacMiddlewareService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RbacMiddlewareService {
            service,
            resource_type: self.resource_type.clone(),
            action: self.action.clone(),
        }))
    }
}

pub struct RbacMiddlewareService<S> {
    service: S,
    resource_type: ResourceType,
    action: Action,
}

impl<S, B> Service<ServiceRequest> for RbacMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&self, ctx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let resource_type = self.resource_type.clone();
        let action = self.action.clone();

        Box::pin(async move {
            // 从请求中提取用户信息
            let user_email = req
                .extensions()
                .get::<String>()
                .cloned()
                .ok_or_else(|| ErrorForbidden("User not authenticated"))?;

            // 从路径中提取org_id
            let org_id = req
                .match_info()
                .get("org_id")
                .ok_or_else(|| ErrorForbidden("Organization ID not found"))?
                .to_string();

            // 可选:从路径提取resource_id
            let resource_id = req.match_info().get("resource_id").map(|s| s.to_string());

            // 构建所需权限
            let required_permission = if let Some(rid) = resource_id {
                Permission::for_resource(resource_type, rid, action)
            } else {
                Permission::new(resource_type, action)
            };

            // 检查权限
            let has_permission = RbacService::check_permission(
                &user_email,
                &org_id,
                &required_permission,
            )
            .await
            .map_err(|e| ErrorForbidden(format!("Permission check failed: {}", e)))?;

            if !has_permission {
                return Err(ErrorForbidden("Insufficient permissions"));
            }

            // 权限通过,继续处理请求
            let res = self.service.call(req).await?;
            Ok(res)
        })
    }
}
```

#### 5.2.5 使用示例

```rust
// 在路由中使用RBAC中间件

use crate::handler::http::middleware::rbac::RbacMiddleware;
use crate::service::rbac::{ResourceType, Action};

pub fn configure_stream_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/{org_id}/streams")
            // 读取流 - 需要Read权限
            .route("", web::get()
                .to(list_streams)
                .wrap(RbacMiddleware::new(ResourceType::Stream, Action::Read))
            )
            // 创建流 - 需要Write权限
            .route("", web::post()
                .to(create_stream)
                .wrap(RbacMiddleware::new(ResourceType::Stream, Action::Write))
            )
            // 删除流 - 需要Delete权限
            .route("/{stream_name}", web::delete()
                .to(delete_stream)
                .wrap(RbacMiddleware::new(ResourceType::Stream, Action::Delete))
            )
    );
}
```

---

### 5.3 加密密钥管理

#### 5.3.1 加密架构

```
┌──────────────────────────────────────────────────────────┐
│                    密钥管理架构                            │
└──────────────────────────────────────────────────────────┘

┌─────────────┐      ┌──────────────┐      ┌─────────────┐
│  Key Store  │      │ Key Registry │      │  Cipher     │
│  (DB/File)  │─────>│   (Memory)   │─────>│  Instance   │
└─────────────┘      └──────────────┘      └─────────────┘
       │                     │                      │
       │                     │                      │
       v                     v                      v
  持久化存储           运行时缓存              加密/解密
```

**支持的加密算法:**
- ✅ AES-256-SIV (主推荐)
- ✅ AES-256-GCM (可选)
- ✅ ChaCha20-Poly1305 (可选)

#### 5.3.2 核心实现

```rust
// src/service/encryption/cipher.rs

use aes_siv::{Aes256SivAead, KeyInit, Nonce};
use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde::{Deserialize, Serialize};

/// Cipher trait - 统一加密接口
pub trait Cipher: Send + Sync {
    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>>;
    fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>>;
    fn algorithm(&self) -> &str;
}

/// AES-256-SIV实现
pub struct AesSivCipher {
    cipher: Aes256SivAead,
}

impl AesSivCipher {
    /// 创建新的AES-SIV cipher
    /// 密钥必须是64字节(512位)
    pub fn new(key: &[u8]) -> Result<Self> {
        if key.len() != 64 {
            anyhow::bail!("AES-SIV key must be 64 bytes, got {}", key.len());
        }

        let cipher = Aes256SivAead::new_from_slice(key)
            .context("Failed to create AES-SIV cipher")?;

        Ok(Self { cipher })
    }

    /// 从base64编码的密钥创建
    pub fn from_base64(encoded_key: &str) -> Result<Self> {
        let key = BASE64.decode(encoded_key)
            .context("Failed to decode base64 key")?;
        Self::new(&key)
    }

    /// 生成随机密钥
    pub fn generate_key() -> Vec<u8> {
        use rand::RngCore;
        let mut key = vec![0u8; 64];
        rand::thread_rng().fill_bytes(&mut key);
        key
    }
}

impl Cipher for AesSivCipher {
    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let nonce = Nonce::default(); // SIV模式使用空nonce是安全的

        self.cipher
            .encrypt(&nonce, plaintext)
            .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))
    }

    fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        let nonce = Nonce::default();

        self.cipher
            .decrypt(&nonce, ciphertext)
            .map_err(|e| anyhow::anyhow!("Decryption failed: {}", e))
    }

    fn algorithm(&self) -> &str {
        "AES-256-SIV"
    }
}

/// 密钥元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyMetadata {
    pub name: String,
    pub algorithm: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub created_by: String,
    pub description: Option<String>,
}

/// 加密密钥(持久化格式)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionKey {
    pub metadata: KeyMetadata,
    pub key_data: String, // Base64编码的密钥
}

impl EncryptionKey {
    pub fn new(
        name: String,
        algorithm: String,
        key_data: Vec<u8>,
        created_by: String,
    ) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            metadata: KeyMetadata {
                name,
                algorithm,
                created_at: now,
                updated_at: now,
                created_by,
                description: None,
            },
            key_data: BASE64.encode(key_data),
        }
    }

    /// 创建AES-SIV密钥
    pub fn new_aes_siv(name: String, created_by: String) -> Self {
        let key = AesSivCipher::generate_key();
        Self::new(name, "AES-256-SIV".to_string(), key, created_by)
    }

    /// 转换为Cipher实例
    pub fn to_cipher(&self) -> Result<Box<dyn Cipher>> {
        match self.metadata.algorithm.as_str() {
            "AES-256-SIV" => {
                let cipher = AesSivCipher::from_base64(&self.key_data)?;
                Ok(Box::new(cipher))
            }
            _ => anyhow::bail!("Unsupported algorithm: {}", self.metadata.algorithm),
        }
    }
}
```

#### 5.3.3 密钥注册表

```rust
// src/service/encryption/registry.rs

use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use once_cell::sync::Lazy;
use anyhow::{Context, Result};
use super::{Cipher, EncryptionKey};

/// 全局密钥注册表
pub struct KeyRegistry {
    keys: Arc<RwLock<HashMap<String, Box<dyn Cipher>>>>,
}

impl KeyRegistry {
    pub fn new() -> Self {
        Self {
            keys: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 添加密钥
    pub async fn add(&self, name: String, cipher: Box<dyn Cipher>) {
        self.keys.write().await.insert(name, cipher);
    }

    /// 获取密钥
    pub async fn get(&self, name: &str) -> Option<Box<dyn Cipher>> {
        // Clone cipher trait object需要特殊处理
        // 这里简化处理,实际需要实现Clone trait
        self.keys.read().await.contains_key(name).then(|| {
            // 返回一个标记,实际使用时从registry获取
            unimplemented!("Need to implement cipher cloning")
        })
    }

    /// 移除密钥
    pub async fn remove(&self, name: &str) -> Option<Box<dyn Cipher>> {
        self.keys.write().await.remove(name)
    }

    /// 列出所有密钥名称
    pub async fn list_keys(&self) -> Vec<String> {
        self.keys.read().await.keys().cloned().collect()
    }

    /// 从数据库加载所有密钥
    pub async fn load_from_db(&self, org_id: &str) -> Result<()> {
        let keys = crate::service::db::cipher::list_all_keys(org_id)
            .await
            .context("Failed to load keys from database")?;

        for key in keys {
            let cipher = key.to_cipher()?;
            self.add(key.metadata.name.clone(), cipher).await;
        }

        Ok(())
    }
}

/// 全局单例
pub static KEY_REGISTRY: Lazy<KeyRegistry> = Lazy::new(|| KeyRegistry::new());

/// 便捷函数
pub async fn get_cipher(name: &str) -> Result<Box<dyn Cipher>> {
    KEY_REGISTRY
        .get(name)
        .await
        .context(format!("Key '{}' not found in registry", name))
}
```

#### 5.3.4 数据库操作

```rust
// src/service/db/cipher.rs

use anyhow::{Context, Result};
use sea_orm::*;
use crate::service::encryption::{EncryptionKey, KeyMetadata};
use crate::infra::table::entity::cipher_keys;

/// 保存加密密钥
pub async fn save_key(org_id: &str, key: &EncryptionKey) -> Result<()> {
    let db = crate::infra::db::ORM_CLIENT.get()
        .context("Database not initialized")?;

    let model = cipher_keys::ActiveModel {
        org_id: Set(org_id.to_string()),
        name: Set(key.metadata.name.clone()),
        algorithm: Set(key.metadata.algorithm.clone()),
        key_data: Set(key.key_data.clone()),
        description: Set(key.metadata.description.clone()),
        created_at: Set(key.metadata.created_at),
        updated_at: Set(key.metadata.updated_at),
        created_by: Set(key.metadata.created_by.clone()),
        ..Default::default()
    };

    cipher_keys::Entity::insert(model)
        .on_conflict(
            OnConflict::columns([
                cipher_keys::Column::OrgId,
                cipher_keys::Column::Name,
            ])
            .update_column(cipher_keys::Column::UpdatedAt)
            .to_owned()
        )
        .exec(db)
        .await?;

    Ok(())
}

/// 获取密钥
pub async fn get_key(org_id: &str, name: &str) -> Result<Option<EncryptionKey>> {
    let db = crate::infra::db::ORM_CLIENT.get()
        .context("Database not initialized")?;

    let model = cipher_keys::Entity::find()
        .filter(cipher_keys::Column::OrgId.eq(org_id))
        .filter(cipher_keys::Column::Name.eq(name))
        .one(db)
        .await?;

    Ok(model.map(|m| EncryptionKey {
        metadata: KeyMetadata {
            name: m.name,
            algorithm: m.algorithm,
            created_at: m.created_at,
            updated_at: m.updated_at,
            created_by: m.created_by,
            description: m.description,
        },
        key_data: m.key_data,
    }))
}

/// 列出所有密钥
pub async fn list_all_keys(org_id: &str) -> Result<Vec<EncryptionKey>> {
    let db = crate::infra::db::ORM_CLIENT.get()
        .context("Database not initialized")?;

    let models = cipher_keys::Entity::find()
        .filter(cipher_keys::Column::OrgId.eq(org_id))
        .all(db)
        .await?;

    Ok(models.into_iter().map(|m| EncryptionKey {
        metadata: KeyMetadata {
            name: m.name,
            algorithm: m.algorithm,
            created_at: m.created_at,
            updated_at: m.updated_at,
            created_by: m.created_by,
            description: m.description,
        },
        key_data: m.key_data,
    }).collect())
}

/// 删除密钥
pub async fn delete_key(org_id: &str, name: &str) -> Result<()> {
    let db = crate::infra::db::ORM_CLIENT.get()
        .context("Database not initialized")?;

    cipher_keys::Entity::delete_many()
        .filter(cipher_keys::Column::OrgId.eq(org_id))
        .filter(cipher_keys::Column::Name.eq(name))
        .exec(db)
        .await?;

    Ok(())
}
```

#### 5.3.5 HTTP API

```rust
// src/handler/http/request/cipher/mod.rs

use actix_web::{web, HttpResponse, Result as ActixResult};
use serde::{Deserialize, Serialize};
use crate::service::encryption::{EncryptionKey, KeyMetadata};

#[derive(Deserialize)]
pub struct CreateKeyRequest {
    pub name: String,
    pub algorithm: String, // "AES-256-SIV"
    pub description: Option<String>,
}

#[derive(Serialize)]
pub struct KeyResponse {
    pub metadata: KeyMetadata,
}

#[derive(Serialize)]
pub struct ListKeysResponse {
    pub keys: Vec<KeyMetadata>,
}

/// 创建密钥
pub async fn create_key(
    org_id: web::Path<String>,
    user: web::ReqData<String>, // 从认证中间件获取
    body: web::Json<CreateKeyRequest>,
) -> ActixResult<HttpResponse> {
    // 生成密钥
    let key = match body.algorithm.as_str() {
        "AES-256-SIV" => {
            EncryptionKey::new_aes_siv(
                body.name.clone(),
                user.into_inner(),
            )
        }
        _ => {
            return Ok(HttpResponse::BadRequest()
                .json(serde_json::json!({
                    "error": "Unsupported algorithm"
                })));
        }
    };

    // 保存到数据库
    crate::service::db::cipher::save_key(&org_id, &key)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    // 添加到注册表
    let cipher = key.to_cipher()
        .map_err(actix_web::error::ErrorInternalServerError)?;
    crate::service::encryption::KEY_REGISTRY
        .add(key.metadata.name.clone(), cipher)
        .await;

    Ok(HttpResponse::Created().json(KeyResponse {
        metadata: key.metadata,
    }))
}

/// 列出密钥
pub async fn list_keys(
    org_id: web::Path<String>,
) -> ActixResult<HttpResponse> {
    let keys = crate::service::db::cipher::list_all_keys(&org_id)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let metadata: Vec<KeyMetadata> = keys.into_iter()
        .map(|k| k.metadata)
        .collect();

    Ok(HttpResponse::Ok().json(ListKeysResponse {
        keys: metadata,
    }))
}

/// 删除密钥
pub async fn delete_key(
    path: web::Path<(String, String)>,
) -> ActixResult<HttpResponse> {
    let (org_id, key_name) = path.into_inner();

    // 从数据库删除
    crate::service::db::cipher::delete_key(&org_id, &key_name)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    // 从注册表移除
    crate::service::encryption::KEY_REGISTRY
        .remove(&key_name)
        .await;

    Ok(HttpResponse::NoContent().finish())
}

/// 配置路由
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/{org_id}/cipher_keys")
            .route("", web::post().to(create_key))
            .route("", web::get().to(list_keys))
            .route("/{key_name}", web::delete().to(delete_key))
    );
}
```

---

### 5.4 审计日志系统

#### 5.4.1 审计日志模型

```rust
// src/service/audit/model.rs

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditAction {
    // 用户操作
    UserLogin,
    UserLogout,
    UserCreate,
    UserUpdate,
    UserDelete,

    // 资源操作
    StreamCreate,
    StreamUpdate,
    StreamDelete,
    DashboardCreate,
    DashboardUpdate,
    DashboardDelete,
    AlertCreate,
    AlertUpdate,
    AlertDelete,

    // 配置操作
    ConfigUpdate,
    KeyCreate,
    KeyDelete,
    RoleAssign,
    RoleRevoke,

    // 数据操作
    DataIngest,
    DataQuery,
    DataExport,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditStatus {
    Success,
    Failed,
    Partial,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLog {
    pub id: String,
    pub timestamp: i64,
    pub org_id: String,
    pub user_email: String,
    pub action: AuditAction,
    pub resource_type: String,
    pub resource_id: Option<String>,
    pub status: AuditStatus,
    pub details: serde_json::Value,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub duration_ms: Option<i64>,
    pub error_message: Option<String>,
}

impl AuditLog {
    pub fn new(
        org_id: String,
        user_email: String,
        action: AuditAction,
        resource_type: String,
    ) -> Self {
        Self {
            id: crate::common::utils::ider::generate(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            org_id,
            user_email,
            action,
            resource_type,
            resource_id: None,
            status: AuditStatus::Success,
            details: serde_json::json!({}),
            ip_address: None,
            user_agent: None,
            duration_ms: None,
            error_message: None,
        }
    }

    pub fn with_resource_id(mut self, resource_id: String) -> Self {
        self.resource_id = Some(resource_id);
        self
    }

    pub fn with_status(mut self, status: AuditStatus) -> Self {
        self.status = status;
        self
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = details;
        self
    }

    pub fn with_ip(mut self, ip: String) -> Self {
        self.ip_address = Some(ip);
        self
    }

    pub fn with_user_agent(mut self, user_agent: String) -> Self {
        self.user_agent = Some(user_agent);
        self
    }
}
```

#### 5.4.2 审计服务

```rust
// src/service/audit/service.rs

use anyhow::{Context, Result};
use super::model::{AuditLog, AuditAction, AuditStatus};

pub struct AuditService;

impl AuditService {
    /// 记录审计日志
    pub async fn log(audit: AuditLog) -> Result<()> {
        // 1. 存储到数据库
        crate::service::db::audit::insert(&audit)
            .await
            .context("Failed to insert audit log to database")?;

        // 2. 可选:发送到专门的审计流用于长期存储
        if config::get_config().audit.stream_enabled {
            Self::send_to_audit_stream(&audit)
                .await
                .context("Failed to send audit to stream")?;
        }

        // 3. 可选:发送到外部SIEM系统
        if let Some(siem_url) = &config::get_config().audit.siem_url {
            Self::send_to_siem(&audit, siem_url)
                .await
                .ok(); // 不阻塞主流程
        }

        Ok(())
    }

    /// 发送到审计流
    async fn send_to_audit_stream(audit: &AuditLog) -> Result<()> {
        let stream_name = "_audit_logs";
        let org_id = &audit.org_id;

        // 转换为日志记录格式
        let log_record = serde_json::to_value(audit)?;

        // 写入流
        crate::service::logs::ingest::handle_json_ingestion(
            org_id,
            stream_name,
            vec![log_record],
        )
        .await?;

        Ok(())
    }

    /// 发送到外部SIEM系统
    async fn send_to_siem(audit: &AuditLog, siem_url: &str) -> Result<()> {
        let client = reqwest::Client::new();
        client
            .post(siem_url)
            .json(audit)
            .send()
            .await?;
        Ok(())
    }

    /// 查询审计日志
    pub async fn query(
        org_id: &str,
        start_time: i64,
        end_time: i64,
        user_email: Option<String>,
        action: Option<AuditAction>,
        limit: usize,
    ) -> Result<Vec<AuditLog>> {
        crate::service::db::audit::query(
            org_id,
            start_time,
            end_time,
            user_email,
            action,
            limit,
        )
        .await
    }
}
```

#### 5.4.3 审计中间件

```rust
// src/handler/http/middleware/audit.rs

use actix_web::{
    Error, HttpMessage, HttpRequest,
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
};
use futures_util::future::{ready, Ready};
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::SystemTime,
};
use crate::service::audit::{AuditLog, AuditAction, AuditStatus, AuditService};

pub struct AuditMiddleware;

impl<S, B> Transform<S, ServiceRequest> for AuditMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = AuditMiddlewareService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AuditMiddlewareService { service }))
    }
}

pub struct AuditMiddlewareService<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for AuditMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&self, ctx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let start_time = SystemTime::now();

        // 提取审计信息
        let user_email = req.extensions().get::<String>().cloned()
            .unwrap_or_else(|| "anonymous".to_string());

        let org_id = req.match_info().get("org_id")
            .map(|s| s.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let action = extract_action(&req);
        let resource_type = extract_resource_type(&req);
        let resource_id = req.match_info().get("resource_id").map(|s| s.to_string());

        let ip = extract_ip(&req);
        let user_agent = extract_user_agent(&req);

        Box::pin(async move {
            // 执行请求
            let response = self.service.call(req).await;

            // 计算耗时
            let duration = start_time.elapsed()
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);

            // 确定状态
            let status = match &response {
                Ok(res) => {
                    if res.status().is_success() {
                        AuditStatus::Success
                    } else {
                        AuditStatus::Failed
                    }
                }
                Err(_) => AuditStatus::Failed,
            };

            // 创建审计日志
            let mut audit = AuditLog::new(org_id, user_email, action, resource_type);
            audit.status = status;
            audit.duration_ms = Some(duration);

            if let Some(rid) = resource_id {
                audit.resource_id = Some(rid);
            }
            if let Some(ip_addr) = ip {
                audit.ip_address = Some(ip_addr);
            }
            if let Some(ua) = user_agent {
                audit.user_agent = Some(ua);
            }

            // 异步记录审计日志(不阻塞响应)
            tokio::spawn(async move {
                if let Err(e) = AuditService::log(audit).await {
                    log::error!("Failed to log audit: {}", e);
                }
            });

            response
        })
    }
}

/// 从请求中提取操作类型
fn extract_action(req: &ServiceRequest) -> AuditAction {
    let method = req.method();
    let path = req.path();

    // 根据HTTP方法和路径推断操作类型
    match (method.as_str(), path) {
        ("POST", p) if p.contains("/streams") => AuditAction::StreamCreate,
        ("PUT", p) if p.contains("/streams") => AuditAction::StreamUpdate,
        ("DELETE", p) if p.contains("/streams") => AuditAction::StreamDelete,
        ("POST", p) if p.contains("/dashboards") => AuditAction::DashboardCreate,
        ("PUT", p) if p.contains("/dashboards") => AuditAction::DashboardUpdate,
        ("DELETE", p) if p.contains("/dashboards") => AuditAction::DashboardDelete,
        // ... 更多操作映射
        _ => AuditAction::DataQuery, // 默认
    }
}

/// 提取资源类型
fn extract_resource_type(req: &ServiceRequest) -> String {
    let path = req.path();

    if path.contains("/streams") {
        "stream".to_string()
    } else if path.contains("/dashboards") {
        "dashboard".to_string()
    } else if path.contains("/alerts") {
        "alert".to_string()
    } else {
        "unknown".to_string()
    }
}

/// 提取IP地址
fn extract_ip(req: &ServiceRequest) -> Option<String> {
    req.connection_info()
        .realip_remote_addr()
        .map(|s| s.to_string())
}

/// 提取User-Agent
fn extract_user_agent(req: &ServiceRequest) -> Option<String> {
    req.headers()
        .get("User-Agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}
```

#### 5.4.4 数据库Schema

```sql
CREATE TABLE audit_logs (
    id VARCHAR(255) PRIMARY KEY,
    timestamp BIGINT NOT NULL,
    org_id VARCHAR(255) NOT NULL,
    user_email VARCHAR(255) NOT NULL,
    action VARCHAR(50) NOT NULL,
    resource_type VARCHAR(50) NOT NULL,
    resource_id VARCHAR(255),
    status VARCHAR(20) NOT NULL,
    details JSON,
    ip_address VARCHAR(50),
    user_agent TEXT,
    duration_ms BIGINT,
    error_message TEXT,

    INDEX idx_org_timestamp (org_id, timestamp DESC),
    INDEX idx_user_timestamp (user_email, timestamp DESC),
    INDEX idx_action (action),
    INDEX idx_resource (resource_type, resource_id)
);

-- 分区表(可选,用于大量数据)
-- 按月分区
ALTER TABLE audit_logs
PARTITION BY RANGE (timestamp) (
    PARTITION p202501 VALUES LESS THAN (UNIX_TIMESTAMP('2025-02-01')),
    PARTITION p202502 VALUES LESS THAN (UNIX_TIMESTAMP('2025-03-01')),
    -- ...
);
```

---

### 5.5 许可证管理

#### 5.5.1 许可证模型

```rust
// src/service/license/model.rs

use serde::{Deserialize, Serialize};
use rsa::{RsaPublicKey, RsaPrivateKey, pkcs8::DecodePublicKey};
use sha2::{Sha256, Digest};
use anyhow::{Context, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct License {
    pub customer_name: String,
    pub customer_email: String,
    pub license_type: LicenseType,
    pub max_ingestion_gb_per_day: f64,
    pub max_users: u32,
    pub features: Vec<String>,
    pub valid_from: i64,
    pub valid_until: i64,
    pub issued_at: i64,
    pub signature: String, // RSA签名(Base64)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LicenseType {
    Trial,
    Standard,
    Professional,
    Enterprise,
}

impl License {
    /// 验证许可证签名
    pub fn verify(&self, public_key_pem: &str) -> Result<bool> {
        // 解析公钥
        let public_key = RsaPublicKey::from_public_key_pem(public_key_pem)
            .context("Invalid public key")?;

        // 构建待签名的消息
        let message = self.get_signing_message();

        // 计算消息哈希
        let mut hasher = Sha256::new();
        hasher.update(message.as_bytes());
        let hash = hasher.finalize();

        // 解码签名
        use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
        let signature = BASE64.decode(&self.signature)
            .context("Invalid signature format")?;

        // 验证签名
        use rsa::Pkcs1v15Sign;
        let scheme = Pkcs1v15Sign::new::<Sha256>();
        public_key.verify(scheme, &hash, &signature)
            .map(|_| true)
            .or(Ok(false))
    }

    /// 检查许可证是否有效
    pub fn is_valid(&self) -> bool {
        let now = chrono::Utc::now().timestamp();
        now >= self.valid_from && now <= self.valid_until
    }

    /// 检查是否过期
    pub fn is_expired(&self) -> bool {
        let now = chrono::Utc::now().timestamp();
        now > self.valid_until
    }

    /// 检查是否包含指定功能
    pub fn has_feature(&self, feature: &str) -> bool {
        self.features.contains(&feature.to_string())
    }

    /// 获取剩余天数
    pub fn days_remaining(&self) -> i64 {
        let now = chrono::Utc::now().timestamp();
        (self.valid_until - now) / 86400
    }

    /// 构建签名消息
    fn get_signing_message(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}|{}|{}",
            self.customer_name,
            self.customer_email,
            self.max_ingestion_gb_per_day,
            self.max_users,
            self.features.join(","),
            self.valid_from,
            self.valid_until
        )
    }
}

/// 许可证使用情况
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseUsage {
    pub daily_ingestion_gb: f64,
    pub active_users: u32,
    pub last_updated: i64,
}
```

#### 5.5.2 许可证服务

```rust
// src/service/license/service.rs

use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use tokio::sync::RwLock;
use super::model::{License, LicenseUsage};

/// 许可证管理服务
pub struct LicenseService {
    current_license: RwLock<Option<License>>,
    usage: RwLock<LicenseUsage>,
    public_key: String,
}

impl LicenseService {
    pub fn new(public_key: String) -> Self {
        Self {
            current_license: RwLock::new(None),
            usage: RwLock::new(LicenseUsage {
                daily_ingestion_gb: 0.0,
                active_users: 0,
                last_updated: chrono::Utc::now().timestamp(),
            }),
            public_key,
        }
    }

    /// 加载许可证
    pub async fn load_license(&self, license_key: &str) -> Result<()> {
        // 解析许可证
        let license: License = serde_json::from_str(license_key)
            .context("Invalid license format")?;

        // 验证签名
        if !license.verify(&self.public_key)? {
            anyhow::bail!("Invalid license signature");
        }

        // 检查是否有效
        if !license.is_valid() {
            if license.is_expired() {
                anyhow::bail!("License has expired");
            } else {
                anyhow::bail!("License is not yet valid");
            }
        }

        // 保存许可证
        *self.current_license.write().await = Some(license.clone());

        // 持久化到数据库
        crate::service::db::license::save(&license).await?;

        Ok(())
    }

    /// 获取当前许可证
    pub async fn get_license(&self) -> Option<License> {
        self.current_license.read().await.clone()
    }

    /// 检查功能是否可用
    pub async fn check_feature(&self, feature: &str) -> Result<bool> {
        let license = self.current_license.read().await;

        match license.as_ref() {
            Some(lic) => {
                if !lic.is_valid() {
                    anyhow::bail!("License expired");
                }
                Ok(lic.has_feature(feature))
            }
            None => Ok(false), // 无许可证,默认为开源功能
        }
    }

    /// 更新使用量
    pub async fn update_usage(&self, ingestion_gb: f64, active_users: u32) {
        let mut usage = self.usage.write().await;
        usage.daily_ingestion_gb = ingestion_gb;
        usage.active_users = active_users;
        usage.last_updated = chrono::Utc::now().timestamp();
    }

    /// 检查是否超限
    pub async fn check_limits(&self) -> Result<()> {
        let license = self.current_license.read().await;
        let usage = self.usage.read().await;

        if let Some(lic) = license.as_ref() {
            // 检查数据摄取量
            if usage.daily_ingestion_gb > lic.max_ingestion_gb_per_day {
                anyhow::bail!(
                    "Ingestion limit exceeded: {:.2} GB / {:.2} GB",
                    usage.daily_ingestion_gb,
                    lic.max_ingestion_gb_per_day
                );
            }

            // 检查用户数
            if usage.active_users > lic.max_users {
                anyhow::bail!(
                    "User limit exceeded: {} / {}",
                    usage.active_users,
                    lic.max_users
                );
            }
        }

        Ok(())
    }
}

/// 全局许可证服务
pub static LICENSE_SERVICE: Lazy<LicenseService> = Lazy::new(|| {
    let public_key = include_str!("../../keys/license_public_key.pem");
    LicenseService::new(public_key.to_string())
});

/// 便捷函数
pub async fn check_feature(feature: &str) -> Result<bool> {
    LICENSE_SERVICE.check_feature(feature).await
}

pub async fn check_limits() -> Result<()> {
    LICENSE_SERVICE.check_limits().await
}
```

#### 5.5.3 许可证中间件

```rust
// src/handler/http/middleware/license.rs

use actix_web::{
    Error, HttpMessage,
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    error::ErrorForbidden,
};
use futures_util::future::{ready, Ready};
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

/// 许可证功能检查中间件
pub struct LicenseFeatureCheck {
    pub feature: String,
}

impl LicenseFeatureCheck {
    pub fn new(feature: impl Into<String>) -> Self {
        Self {
            feature: feature.into(),
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for LicenseFeatureCheck
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = LicenseFeatureCheckService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(LicenseFeatureCheckService {
            service,
            feature: self.feature.clone(),
        }))
    }
}

pub struct LicenseFeatureCheckService<S> {
    service: S,
    feature: String,
}

impl<S, B> Service<ServiceRequest> for LicenseFeatureCheckService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&self, ctx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let feature = self.feature.clone();

        Box::pin(async move {
            // 检查功能是否启用
            let enabled = crate::service::license::check_feature(&feature)
                .await
                .map_err(|e| ErrorForbidden(format!("License check failed: {}", e)))?;

            if !enabled {
                return Err(ErrorForbidden(format!(
                    "Feature '{}' requires a valid license",
                    feature
                )));
            }

            // 功能已启用,继续处理
            let res = self.service.call(req).await?;
            Ok(res)
        })
    }
}
```

#### 5.5.4 使用示例

```rust
// 在需要许可证的路由上使用中间件

use crate::handler::http::middleware::license::LicenseFeatureCheck;

pub fn configure_enterprise_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/{org_id}/enterprise")
            // SSO功能需要许可证
            .route("/sso/config", web::get()
                .to(get_sso_config)
                .wrap(LicenseFeatureCheck::new("sso"))
            )
            // AI功能需要许可证
            .route("/ai/chat", web::post()
                .to(ai_chat)
                .wrap(LicenseFeatureCheck::new("ai"))
            )
    );
}
```

---

### 5.6 AI功能集成

#### 5.6.1 集成方案(推荐)

**不建议完全自研AI模型,推荐集成第三方API:**

```rust
// src/service/ai/mod.rs

use serde::{Deserialize, Serialize};
use anyhow::{Context, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String, // "system", "user", "assistant"
    pub content: String,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Message,
}

pub struct AiService {
    client: reqwest::Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl AiService {
    pub fn new_openai(api_key: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            model: "gpt-4".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
        }
    }

    pub fn new_anthropic(api_key: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            model: "claude-3-sonnet-20240229".to_string(),
            base_url: "https://api.anthropic.com/v1".to_string(),
        }
    }

    /// 发送聊天请求
    pub async fn chat(&self, messages: Vec<Message>) -> Result<String> {
        let url = format!("{}/chat/completions", self.base_url);

        let request = ChatRequest {
            model: self.model.clone(),
            messages,
            temperature: 0.7,
            max_tokens: 2000,
        };

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send chat request")?;

        let chat_response: ChatResponse = response
            .json()
            .await
            .context("Failed to parse chat response")?;

        Ok(chat_response.choices[0].message.content.clone())
    }
}

/// 带上下文的智能聊天
pub async fn chat_with_logs_context(
    org_id: &str,
    query: &str,
    ai_service: &AiService,
) -> Result<String> {
    // 1. 从OpenObserve查询相关日志
    let logs = search_relevant_logs(org_id, query, 10).await?;

    // 2. 构建上下文
    let context = logs.iter()
        .map(|log| format!("- {}", serde_json::to_string(log).unwrap_or_default()))
        .collect::<Vec<_>>()
        .join("\n");

    // 3. 构建消息
    let messages = vec![
        Message {
            role: "system".to_string(),
            content: "You are a log analysis assistant. Analyze the provided logs and answer user questions.".to_string(),
        },
        Message {
            role: "user".to_string(),
            content: format!(
                "Based on these logs:\n{}\n\nQuestion: {}",
                context, query
            ),
        },
    ];

    // 4. 调用AI
    ai_service.chat(messages).await
}

/// 搜索相关日志
async fn search_relevant_logs(
    org_id: &str,
    query: &str,
    limit: usize,
) -> Result<Vec<serde_json::Value>> {
    // 使用OpenObserve的搜索功能
    // 这里简化,实际需要调用search API
    Ok(vec![])
}
```

#### 5.6.2 HTTP API

```rust
// src/handler/http/request/ai/mod.rs

use actix_web::{web, HttpResponse, Result as ActixResult};
use serde::{Deserialize, Serialize};
use crate::service::ai::{AiService, Message, chat_with_logs_context};

#[derive(Deserialize)]
pub struct ChatRequest {
    pub query: String,
    pub messages: Vec<Message>,
    pub use_context: Option<bool>,
}

#[derive(Serialize)]
pub struct ChatResponse {
    pub response: String,
}

pub async fn chat(
    org_id: web::Path<String>,
    body: web::Json<ChatRequest>,
) -> ActixResult<HttpResponse> {
    // 获取AI服务配置
    let config = config::get_config();
    let ai_service = if let Some(openai_key) = &config.ai.openai_api_key {
        AiService::new_openai(openai_key.clone())
    } else if let Some(claude_key) = &config.ai.anthropic_api_key {
        AiService::new_anthropic(claude_key.clone())
    } else {
        return Ok(HttpResponse::ServiceUnavailable()
            .json(serde_json::json!({
                "error": "AI service not configured"
            })));
    };

    // 处理请求
    let response = if body.use_context.unwrap_or(true) {
        // 使用日志上下文
        chat_with_logs_context(&org_id, &body.query, &ai_service)
            .await
            .map_err(actix_web::error::ErrorInternalServerError)?
    } else {
        // 直接聊天
        ai_service.chat(body.messages.clone())
            .await
            .map_err(actix_web::error::ErrorInternalServerError)?
    };

    Ok(HttpResponse::Ok().json(ChatResponse { response }))
}
```

---

## 6. 成本效益分析

### 6.1 开发成本估算

| 项目 | 人天 | 成本(假设¥1000/天) |
|------|------|-------------------|
| **SSO/OAuth2** | 15天 | ¥15,000 |
| **RBAC权限** | 20天 | ¥20,000 |
| **加密管理** | 10天 | ¥10,000 |
| **审计日志** | 8天 | ¥8,000 |
| **许可证管理** | 5天 | ¥5,000 |
| **AI集成** | 15天 | ¥15,000 |
| **测试与文档** | 15天 | ¥15,000 |
| **项目管理** | 10天 | ¥10,000 |
| **总计** | **98天** | **¥98,000** |

**说明:** 以2-3人团队计算,实际工期约3-4个月。

### 6.2 运维成本

| 项目 | 年成本 |
|------|--------|
| **AI API费用** | ¥20,000 - ¥50,000 |
| **额外服务器** | ¥10,000 - ¥30,000 |
| **维护人力** | ¥100,000 - ¥200,000 |
| **总计** | **¥130,000 - ¥280,000/年** |

### 6.3 对比分析

| 方案 | 初期投入 | 年运维成本 | 3年总成本 | 优势 | 劣势 |
|-----|---------|-----------|----------|------|------|
| **完全自研** | ¥100,000 | ¥200,000 | ¥700,000 | 完全掌控 | 开发周期长 |
| **部分自研** | ¥70,000 | ¥150,000 | ¥520,000 | 平衡性好 | 依赖第三方 |
| **购买企业版** | ¥50,000 | ¥100,000 | ¥350,000 | 快速上线 | 受限于供应商 |

**推荐:** 部分自研方案(SSO+RBAC+加密自研,AI集成第三方)

---

## 7. 技术栈与依赖

### 7.1 Rust依赖

```toml
[dependencies]
# 核心框架
actix-web = "4.12"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1.0"
thiserror = "2.0"

# 认证
oauth2 = "4.4"
jsonwebtoken = "9.3"

# 加密
aes-siv = "0.7"
rsa = "0.9"
sha2 = "0.10"
base64 = "0.22"
rand = "0.9"

# 数据库
sea-orm = { version = "1.1", features = ["sqlx-all", "runtime-tokio-rustls"] }
sqlx = { version = "0.8", features = ["mysql", "postgres", "sqlite"] }

# HTTP客户端
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }

# 日期时间
chrono = { version = "0.4", features = ["serde"] }

# 日志
log = "0.4"
tracing = "0.1"
```

### 7.2 外部服务依赖

| 服务 | 用途 | 必需性 |
|-----|------|--------|
| **MySQL/PostgreSQL** | 元数据存储 | 必需 |
| **OAuth2提供商** | SSO认证 | 可选 |
| **OpenAI/Claude API** | AI功能 | 可选 |
| **SIEM系统** | 审计日志转发 | 可选 |

---

## 8. 风险评估与应对

### 8.1 技术风险

| 风险 | 可能性 | 影响 | 应对措施 |
|-----|-------|------|---------|
| **加密算法漏洞** | 低 | 高 | 使用成熟的crate,定期更新 |
| **权限绕过** | 中 | 高 | 充分测试,安全审计 |
| **性能问题** | 中 | 中 | 性能测试,优化查询 |
| **数据库迁移** | 低 | 中 | 使用SeaORM migration |

### 8.2 业务风险

| 风险 | 应对 |
|-----|------|
| **功能不完整** | 分阶段交付,优先核心功能 |
| **兼容性问题** | 充分测试,保持向后兼容 |
| **文档不足** | 同步编写文档,API文档自动生成 |

### 8.3 合规风险

| 风险 | 应对 |
|-----|------|
| **数据隐私** | 遵循GDPR/CCPA,实现数据删除 |
| **审计要求** | 完整的审计日志,不可篡改 |
| **加密标准** | 使用业界标准算法(AES-256) |

---

## 9. 总结与建议

### 9.1 实施建议

#### ✅ **推荐实施的功能 (P0-P1)**

1. **SSO/OAuth2认证** - 企业基础需求,实现简单
2. **RBAC权限管理** - 安全核心,分阶段实现
3. **加密密钥管理** - 数据安全基础
4. **审计日志系统** - 合规必需

**预期效果:**
- 🎯 满足90%企业客户的基本需求
- ⏱️ 3个月内可交付
- 💰 投入产出比高

#### ⚠️ **可选实施的功能 (P2-P3)**

1. **许可证管理** - 商业化需要
2. **AI功能集成** - 差异化功能,建议使用第三方API
3. **查询管理器** - 性能优化

**建议:**
- 根据客户反馈决定优先级
- AI功能优先集成,不自研

#### ❌ **不建议自研的功能 (P4)**

1. **超集群队列系统** - 复杂度极高,投入产出比低
2. **完整AI训练系统** - 专业性强,维护成本高

**替代方案:**
- 超集群:使用成熟的消息队列(NATS/Kafka)
- AI:集成OpenAI/Claude等商业API

### 9.2 成功关键因素

1. ✅ **团队技能:** 需要熟悉Rust、安全编程的工程师
2. ✅ **测试覆盖:** 安全功能必须有高测试覆盖率(>80%)
3. ✅ **文档完善:** API文档、部署文档、用户手册
4. ✅ **安全审计:** 第三方安全审计,漏洞扫描
5. ✅ **渐进交付:** 分阶段发布,快速迭代

### 9.3 预期收益

**技术收益:**
- 🔐 完整的企业级安全体系
- 🚀 自主可控的核心功能
- 📈 可定制化的权限系统

**商业收益:**
- 💰 支持商业化,License管理
- 🏢 满足企业客户需求
- 🌟 产品差异化竞争力

**时间收益:**
- ⏱️ 3-4个月完成核心功能
- 📅 6个月达到企业版80%功能
- 🎯 12个月功能完整

### 9.4 最终建议

**基于ROI分析,建议采用"混合策略":**

```
┌─────────────────────────────────────────┐
│         自研 + 集成 混合策略              │
└─────────────────────────────────────────┘

自研部分 (70%):
  ├─ SSO/OAuth2认证      ✅ 自研
  ├─ RBAC权限管理        ✅ 自研
  ├─ 加密密钥管理        ✅ 自研
  ├─ 审计日志系统        ✅ 自研
  └─ 许可证管理          ✅ 自研

集成部分 (30%):
  ├─ AI聊天功能          🔌 集成OpenAI/Claude
  ├─ 高级分析            🔌 集成第三方分析工具
  └─ SIEM集成            🔌 对接现有SIEM系统

暂缓部分:
  ├─ 超集群队列          ⏸️ 使用NATS等成熟方案
  └─ AI模型训练          ⏸️ 优先级低
```

**预期交付:**
- **3个月:** 核心功能(SSO+RBAC+加密+审计)可用
- **6个月:** 完整企业版功能,支持商业化
- **12个月:** 功能成熟,用户反馈优化

**投资回报:**
- **初期投入:** ¥70,000 - ¥100,000
- **年运维成本:** ¥150,000 - ¥200,000
- **预期收益:** 支持企业客户,提升产品竞争力

---

## 附录

### A. 参考资料

1. **OAuth2.0规范:** https://oauth.net/2/
2. **RBAC最佳实践:** https://www.nist.gov/publications/rbac
3. **AES-SIV加密:** https://tools.ietf.org/html/rfc5297
4. **审计日志标准:** https://www.iso.org/standard/42506.html

### B. 开发工具

1. **Rust工具链:** rustup, cargo
2. **数据库工具:** SeaORM CLI, sqlx-cli
3. **API测试:** Postman, curl
4. **安全扫描:** cargo-audit, cargo-deny

### C. 联系方式

**技术支持:** tech@example.com
**项目管理:** pm@example.com
**安全团队:** security@example.com

---

**文档结束**
