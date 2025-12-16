# Visdata 代码修改清单

本文档记录了在 OpenObserve 开源代码中添加 visdata（自研 SSO + RBAC）功能时所做的所有修改，以便在更新开源代码时能够正确处理合并冲突。

---

## 一、新增的文件/目录（需要保留）

| 路径 | 说明 |
|------|------|
| `crates/visdata/` | 整个 visdata crate（SSO + RBAC 实现） |
| `src/infra/src/table/migration/m20251214_000001_create_visdata_tables.rs` | 数据库迁移文件 |
| `web/.env.local` | 前端配置（添加了 `VITE_OPENOBSERVE_ENTERPRISE=true`） |

---

## 二、修改的 Cargo 配置文件

### 1. Cargo.toml (根目录)

```toml
# [features] 部分添加:
visdata = ["dep:visdata", "infra/visdata"]

# [dependencies] 部分添加:
visdata = { workspace = true, optional = true }

# [workspace] members 部分添加:
"crates/visdata",

# [workspace.dependencies] 部分添加:
visdata = { path = "crates/visdata" }
```

### 2. src/infra/Cargo.toml

```toml
# [features] 部分添加:
visdata = []
```

---

## 三、修改的 Rust 源文件

### 1. src/main.rs

**位置 1** (~第 311 行) - 初始化 visdata 模块:

```rust
#[cfg(feature = "visdata")]
{
    if let Err(e) = visdata::init(&config).await {
        panic!("Failed to initialize visdata: {}", e);
    }
}
```

**位置 2** (~第 1500 行) - 优雅关闭:

```rust
#[cfg(feature = "visdata")]
visdata::shutdown();
```

### 2. src/handler/http/router/mod.rs

**位置 1** (~第 46 行) - 导入:

```rust
#[cfg(feature = "visdata")]
use visdata::handler as visdata_handler;
```

**位置 2** (~第 582-612 行) - 路由注册:

```rust
#[cfg(feature = "visdata")]
{
    // Visdata RBAC routes
    cfg.service(
        web::scope("/api")
            .service(visdata_handler::roles::create_role)
            .service(visdata_handler::roles::list_roles)
            .service(visdata_handler::roles::update_role)
            .service(visdata_handler::roles::delete_role)
            .service(visdata_handler::roles::get_role_permissions)
            .service(visdata_handler::roles::get_role_users)
            .service(visdata_handler::groups::create_group)
            .service(visdata_handler::groups::list_groups)
            .service(visdata_handler::groups::get_group)
            .service(visdata_handler::groups::update_group)
            .service(visdata_handler::groups::delete_group)
            .service(visdata_handler::groups::add_role)
            .service(visdata_handler::groups::remove_role)
            .service(visdata_handler::groups::add_user)
            .service(visdata_handler::groups::remove_user)
            .service(visdata_handler::resources::get_resources)
            .service(visdata_handler::sso::list_providers)
            .service(visdata_handler::sso::create_provider)
            .service(visdata_handler::sso::update_provider)
            .service(visdata_handler::sso::delete_provider)
            .service(visdata_handler::sso::oidc_callback),
    );
}
#[cfg(not(feature = "visdata"))]
{
    // Original routes without visdata
}
```

### 3. src/handler/http/request/status/mod.rs

**位置** (~第 260-264 行) - 返回 `rbac_enabled`:

```rust
#[cfg(feature = "enterprise")]
let rbac_enabled = openfga_cfg.enabled;
#[cfg(all(not(feature = "enterprise"), feature = "visdata"))]
let rbac_enabled = true; // visdata provides its own RBAC
#[cfg(all(not(feature = "enterprise"), not(feature = "visdata")))]
let rbac_enabled = false;
```

### 4. src/handler/http/auth/validator.rs

**位置** (~第 1010-1030 行) - RBAC 验证函数:

```rust
#[cfg(all(not(feature = "enterprise"), not(feature = "visdata")))]
async fn check_permissions(...) -> Result<bool, Error> {
    // Original implementation - no RBAC check
    Ok(true)
}

#[cfg(feature = "visdata")]
async fn check_permissions(
    org_id: &str,
    user_email: &str,
    resource_type: &str,
    resource_id: Option<&str>,
    permission: &str,
) -> Result<bool, Error> {
    // Visdata RBAC check implementation
    let resource = match resource_id {
        Some(id) => format!("{}:{}", resource_type, id),
        None => format!("{}:_all_{}", resource_type, org_id),
    };

    let perm: visdata::rbac::Permission = permission.parse()
        .map_err(|_| Error::Forbidden(format!("Invalid permission: {}", permission)))?;

    visdata::Visdata::global()
        .rbac()
        .check_permission(org_id, user_email, &resource, perm)
        .await
        .map_err(|e| Error::Forbidden(e.to_string()))
}
```

### 5. src/migration/mod.rs

**位置** (~第 94-105 行) - 即使版本匹配也运行 visdata 迁移:

```rust
if db_schema_version == config::DB_SCHEMA_VERSION {
    // if version matches, we do not need to run update commands
    log::info!("DB_SCHEMA_VERSION match, skipping db upgrade");
    // Still run sea-orm migrations for visdata tables (they use IF NOT EXISTS)
    #[cfg(feature = "visdata")]
    {
        ORM_CLIENT_DDL.get_or_init(connect_to_orm_ddl).await;
        infra::table::migrate().await?;
    }
    return Ok(());
}
```

### 6. src/infra/src/table/migration/mod.rs

**位置 1** (~第 79 行) - 导入迁移模块:

```rust
#[cfg(feature = "visdata")]
mod m20251214_000001_create_visdata_tables;
```

**位置 2** (~第 150 行) - 注册迁移:

```rust
#[cfg(feature = "visdata")]
vec.push(Box::new(m20251214_000001_create_visdata_tables::Migration));
```

---

## 四、前端配置修改

### web/.env.local

```env
VITE_OPENOBSERVE_ENDPOINT=http://localhost:5080
VITE_OPENOBSERVE_ENTERPRISE=true
```

> **注意**: `VITE_OPENOBSERVE_ENTERPRISE=true` 使前端显示 IAM 菜单（Roles、Groups 等）

---

## 五、数据库表结构

visdata 会创建以下数据库表（前缀 `vd_`）：

| 表名 | 说明 |
|------|------|
| `vd_roles` | 角色定义 |
| `vd_role_permissions` | 角色权限 |
| `vd_role_users` | 角色-用户关联 |
| `vd_groups` | 用户组 |
| `vd_group_roles` | 组-角色关联 |
| `vd_group_users` | 组-用户关联 |
| `vd_sso_providers` | SSO 提供者配置 |
| `vd_sso_user_mappings` | SSO 用户映射 |

---

## 六、更新开源代码时的合并策略

### 冲突风险评估

| 文件 | 冲突风险 | 说明 |
|------|----------|------|
| `Cargo.toml` | **高** | 每次都需要添加 visdata 相关配置 |
| `src/infra/Cargo.toml` | 低 | 只需添加 `visdata = []` feature |
| `src/main.rs` | **中** | 初始化和关闭代码 |
| `src/handler/http/router/mod.rs` | **中** | 路由注册 |
| `src/handler/http/request/status/mod.rs` | 低 | rbac_enabled 返回值 |
| `src/handler/http/auth/validator.rs` | **中** | RBAC 验证逻辑 |
| `src/migration/mod.rs` | 低 | 迁移执行逻辑 |
| `src/infra/src/table/migration/mod.rs` | 低 | 迁移注册 |

### 建议的 Git 工作流

```bash
# 1. 创建 visdata 分支保存当前更改
git checkout -b visdata-features
git add -A
git commit -m "feat: add visdata SSO and RBAC"

# 2. 更新主分支
git checkout main
git pull upstream main

# 3. 将 visdata 功能合并回来
git merge visdata-features

# 4. 解决冲突（主要在 Cargo.toml 和 router/mod.rs）

# 5. 验证编译
cargo check --features visdata
```

---

## 七、快速验证清单

更新开源代码后，运行以下命令验证 visdata 是否正常工作:

```powershell
# 1. 检查编译
cargo check --features visdata

# 2. 运行测试
cargo test --features visdata -p visdata

# 3. 构建发布版本
cargo build --release --features visdata

# 4. 验证启动
# 启动后检查日志是否有:
# - "Initializing VisData (SSO & RBAC) module..."
# - "VisData module initialized successfully"
# - "Created default role 'Admin' in org 'xxx'"
```

---

## 八、权限格式说明

### 权限对象格式

前端期望的权限格式为 `{resource}:_all_{org_id}`，例如：
- `stream:_all_default` - 所有 stream 的权限
- `dashboard:_all_default` - 所有 dashboard 的权限
- `logs:my_stream_name` - 特定 stream 的权限

### 权限类型

| 权限 | 说明 | HTTP 方法 |
|------|------|-----------|
| `AllowAll` | 完全访问 | 所有 |
| `AllowList` | 列表查看 | GET (列表) |
| `AllowGet` | 单个查看 | GET (单个) |
| `AllowPost` | 创建 | POST |
| `AllowPut` | 更新 | PUT/PATCH |
| `AllowDelete` | 删除 | DELETE |

---

## 九、默认角色

系统启动时会为每个组织创建以下默认角色：

| 角色 | 说明 |
|------|------|
| `Admin` | 组织管理员，拥有所有权限 |
| `Editor` | 编辑者，可以查看和编辑大部分资源 |
| `Viewer` | 查看者，只读权限 |
| `Ingester` | 数据摄入者，只能向 stream 写入数据 |

---

*文档更新时间: 2025-12-14*



1）） c:\Users\ltdhk\Documents\DevSpace\ClaudeCode\openobserve\web\src\components\iam\users\AddUser.vue

 在 AddUser.vue 的 defaultValue 中添加了 custom_role: []，确保默认值是空数组而不是 undefined。


2））前端修改（User.vue）
导入新的 getCustomRoles 函数：
```javascript
import { getCustomRoles as getIamCustomRoles } from "@/services/iam";
```
修改 getCustomRoles 使用正确的函数：
```javascript
const getCustomRoles = async () => {
  await getIamCustomRoles(store.state.selectedOrganization.identifier)
    .then((res) => {
      customRoles.value = res.data;
    })
};
```
在 onBeforeMount 中调用获取自定义角色：
```javascript
if (isEnterprise.value) {
  await getCustomRoles();
}
```

### 7. src/service/users.rs

此文件修改较多，用于支持 custom roles 功能。

**位置 1** (~第 69-130 行) - 允许 visdata 使用 custom roles:

```rust
if usr_req.role.custom_role.is_some() {
    #[cfg(all(not(feature = "enterprise"), not(feature = "visdata")))]
    return Ok(HttpResponse::BadRequest().json(MetaHttpResponse::message(
        http::StatusCode::BAD_REQUEST,
        "Custom roles not allowed",
    )));
    // ... enterprise block ...

    // Visdata: Validate custom roles exist
    #[cfg(all(not(feature = "enterprise"), feature = "visdata"))]
    {
        if visdata::is_initialized() {
            let custom_roles = usr_req.role.custom_role.as_ref().unwrap();
            for custom_role in custom_roles {
                match visdata::Visdata::global()
                    .rbac()
                    .get_role_by_name(org_id, custom_role)
                    .await
                {
                    Ok(Some(_)) => {} // Role exists, continue
                    Ok(None) => {
                        return Ok(HttpResponse::BadRequest().json(MetaHttpResponse::message(
                            http::StatusCode::BAD_REQUEST,
                            format!("Custom role not found: {}", custom_role),
                        )));
                    }
                    Err(e) => {
                        log::error!("Error fetching custom role during post user: {e}");
                        return Ok(HttpResponse::BadRequest().json(MetaHttpResponse::message(
                            http::StatusCode::BAD_REQUEST,
                            "Custom role not found",
                        )));
                    }
                }
            }
        }
    }
}
```

**位置 2** (~第 236-258 行) - 新建用户时分配 custom roles:

```rust
// Update Visdata RBAC - assign custom roles to user
#[cfg(all(not(feature = "enterprise"), feature = "visdata"))]
{
    if visdata::is_initialized() {
        if let Some(ref custom_roles) = usr_req.role.custom_role {
            for role_name in custom_roles {
                if let Err(e) = visdata::service::role::add_user(
                    &org_id,
                    role_name,
                    &usr_req.email,
                )
                .await
                {
                    log::error!(
                        "Error assigning custom role '{}' to user '{}': {e}",
                        role_name,
                        usr_req.email
                    );
                }
            }
        }
    }
}
```

**位置 3** (~第 360-363 行) - 声明 custom_roles 变量:

```rust
#[cfg(any(feature = "enterprise", feature = "visdata"))]
let mut custom_roles: Vec<String> = vec![];
#[cfg(any(feature = "enterprise", feature = "visdata"))]
let mut custom_roles_need_change = false;
```

**位置 4** (~第 469-475 行) - 更新用户时标记 custom roles 变更:

```rust
} else if local_user.role.ne(&new_user.role) {
    #[cfg(any(feature = "enterprise", feature = "visdata"))]
    if new_org_role.custom_role.is_some() {
        custom_roles_need_change = true;
        custom_roles.extend(new_org_role.custom_role.clone().unwrap());
    }
    is_org_updated = true;
}
```

**位置 5** (~第 617-665 行) - 更新用户时同步 custom roles 到 Visdata:

```rust
// Update Visdata RBAC - sync custom roles for user
#[cfg(all(not(feature = "enterprise"), feature = "visdata"))]
{
    if visdata::is_initialized() && custom_roles_need_change {
        // Get existing roles for user (returns Vec<vd_roles::Model>)
        let existing_roles: Vec<String> = match visdata::Visdata::global()
            .rbac()
            .get_user_direct_roles(org_id, email)
            .await
        {
            Ok(roles) => roles.into_iter().map(|r| r.name).collect(),
            Err(e) => {
                log::error!("Error fetching user roles: {e}");
                vec![]
            }
        };

        // Add new roles that user doesn't have
        for role_name in &custom_roles {
            if !existing_roles.contains(role_name) {
                if let Err(e) =
                    visdata::service::role::add_user(org_id, role_name, email).await
                {
                    log::error!(
                        "Error adding custom role '{}' to user '{}': {e}",
                        role_name,
                        email
                    );
                }
            }
        }

        // Remove roles that are no longer assigned
        for existing_role in &existing_roles {
            if !custom_roles.contains(existing_role) {
                if let Err(e) =
                    visdata::service::role::remove_user(org_id, existing_role, email)
                        .await
                {
                    log::error!(
                        "Error removing custom role '{}' from user '{}': {e}",
                        existing_role,
                        email
                    );
                }
            }
        }
    }
}
```

**位置 6** (~第 828-846 行) - 添加用户到组织时分配 custom roles:

```rust
// Update Visdata RBAC - assign custom roles to user
#[cfg(all(not(feature = "enterprise"), feature = "visdata"))]
{
    if visdata::is_initialized() {
        if let Some(ref custom_roles) = role.custom_role {
            for role_name in custom_roles {
                if let Err(e) =
                    visdata::service::role::add_user(org_id, role_name, &email).await
                {
                    log::error!(
                        "Error assigning custom role '{}' to user '{}': {e}",
                        role_name,
                        email
                    );
                }
            }
        }
    }
}
```

---

## 十、冲突风险更新

| 文件 | 冲突风险 | 说明 |
|------|----------|------|
| `src/service/users.rs` | **高** | 多处修改支持 custom roles |

---

*文档更新时间: 2025-12-14*