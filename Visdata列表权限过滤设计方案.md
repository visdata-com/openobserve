# Visdata 列表权限过滤设计方案

本文档描述如何为 Visdata RBAC 系统实现列表接口的权限过滤功能，使其达到与 Enterprise 版本相同的用户体验。

---

## 一、背景与目标

### 1.1 当前状态

| 功能 | Enterprise | Visdata (当前) |
|------|------------|----------------|
| 单个资源权限检查 | ✅ OpenFGA | ✅ RBAC Engine |
| 403 错误处理 | ✅ | ✅ |
| **列表接口过滤** | ✅ | ❌ 未实现 |

### 1.2 问题描述

当前 Visdata 模式下：
- 用户请求资源列表时，返回所有资源
- 用户点击某个资源时，后端才检查权限并返回 403
- 用户体验不佳：可以看到无权限的资源，点击后才知道没权限

### 1.3 目标

实现列表接口的权限过滤，让用户只能看到有权限访问的资源。

---

## 二、Enterprise 版本实现分析

### 2.1 核心函数

Enterprise 版本使用 `list_objects_for_user()` 函数获取用户有权限访问的资源列表：

```rust
// src/handler/http/auth/validator.rs:1071-1107
#[cfg(feature = "enterprise")]
pub(crate) async fn list_objects_for_user(
    org_id: &str,
    user_id: &str,
    permission: &str,    // "GET", "PUT", "DELETE"
    object_type: &str,   // "logs", "alerts", "dashboard"
) -> Result<Option<Vec<String>>, Error>
```

**返回值说明：**
- `None`: Root 用户或 RBAC 未启用，不需要过滤
- `Some(Vec<String>)`: 允许访问的资源 ID 列表

### 2.2 两种过滤模式

| 模式 | 使用场景 | 实现位置 |
|------|----------|----------|
| **Pre-Query** | Streams, Pipelines, Functions | Handler 层调用，传入 Service 层 |
| **Service-Layer** | Alerts, Dashboards | 直接传 user_id，Service 层内部过滤 |

### 2.3 已实现过滤的资源

| 资源类型 | 文件位置 | Object Key |
|----------|----------|------------|
| Streams | `stream/mod.rs` | logs, metrics, traces |
| Alerts | `alerts/mod.rs` | alerts |
| Dashboards | `dashboards/mod.rs` | dashboard |
| Pipelines | `pipeline.rs` | pipelines |
| Functions | `functions/mod.rs` | function |

---

## 三、Visdata 设计方案

### 3.1 新增 RBAC 引擎函数

在 `crates/visdata/src/rbac/engine.rs` 中添加：

```rust
/// 获取用户有权限访问的资源列表
/// 返回 None 表示允许访问所有资源（Root 用户或无限制）
/// 返回 Some(Vec) 表示只能访问指定的资源
pub async fn list_permitted_objects(
    &self,
    org_id: &str,
    user_email: &str,
    resource_type: &str,  // "logs", "alerts", "dashboard" 等
    permission: &str,     // "AllowGet", "AllowList", "AllowPost" 等
) -> Result<Option<Vec<String>>>
```

### 3.2 过滤逻辑流程

```
用户请求列表 → 获取用户所有角色 → 收集权限 → 返回允许的资源 ID

权限匹配规则:
1. 如果有 "resource:_all_{org_id}" → 返回 None (允许所有)
2. 如果有 "resource:specific_name" → 收集到列表
3. 返回 Some(collected_list)
```

### 3.3 新增公共 API 函数

在 `src/handler/http/auth/validator.rs` 中添加 Visdata 版本：

```rust
#[cfg(all(not(feature = "enterprise"), feature = "visdata"))]
pub(crate) async fn list_objects_for_user(
    org_id: &str,
    user_id: &str,
    permission: &str,
    object_type: &str,
) -> Result<Option<Vec<String>>, Error> {
    // Root 用户跳过
    if is_root_user(user_id) {
        return Ok(None);
    }

    // 调用 Visdata RBAC
    visdata::Visdata::global()
        .rbac()
        .list_permitted_objects(org_id, user_id, object_type, permission)
        .await
        .map_err(|e| ErrorForbidden(e.to_string()))
}
```

---

## 四、需要修改的接口清单

### 4.1 高优先级（核心数据资源）

| 接口 | 文件 | 修改内容 |
|------|------|----------|
| `GET /{org}/streams` | `src/handler/http/request/stream/mod.rs` | 添加 visdata 过滤 |
| `GET /{org}/alerts` | `src/handler/http/request/alerts/mod.rs` | 添加 visdata 过滤 |
| `GET /{org}/dashboards` | `src/handler/http/request/dashboards/mod.rs` | 添加 visdata 过滤 |

### 4.2 中优先级（功能资源）

| 接口 | 文件 | 修改内容 |
|------|------|----------|
| `GET /{org}/pipelines` | `src/handler/http/request/pipeline.rs` | 添加 visdata 过滤 |
| `GET /{org}/functions` | `src/handler/http/request/functions/mod.rs` | 添加 visdata 过滤 |
| `GET /{org}/reports` | `src/handler/http/request/reports/mod.rs` | 添加 visdata 过滤 |

### 4.3 低优先级（辅助资源）

| 接口 | 文件 | 修改内容 |
|------|------|----------|
| `GET /{org}/alerts/templates` | alerts 相关 | 添加 visdata 过滤 |
| `GET /{org}/alerts/destinations` | alerts 相关 | 添加 visdata 过滤 |
| `GET /{org}/saved_views` | saved_views 相关 | 添加 visdata 过滤 |

---

## 五、详细实现步骤

### Step 1: 添加 RBAC 引擎函数

**文件**: `crates/visdata/src/rbac/engine.rs`

```rust
/// List objects that user has permission to access
pub async fn list_permitted_objects(
    &self,
    org_id: &str,
    user_email: &str,
    resource_type: &str,
    permission: &str,
) -> Result<Option<Vec<String>>> {
    // 1. 获取用户所有角色
    let user_roles = self.get_user_roles(org_id, user_email).await?;
    let all_role_ids: Vec<&str> = user_roles
        .direct_role_ids.iter()
        .chain(user_roles.group_role_ids.iter())
        .map(|s| s.as_str())
        .collect();

    if all_role_ids.is_empty() {
        return Ok(Some(vec![])); // 无角色 = 无权限
    }

    // 2. 收集所有权限
    let mut permitted_objects: Vec<String> = vec![];
    let all_wildcard = format!("{}:_all_{}", resource_type, org_id);
    let required_permission: Permission = permission.parse()?;

    for role_id in &all_role_ids {
        let permissions = self.get_role_permissions(role_id).await?;

        for (perm_object, perm_str) in permissions {
            // 检查是否是该资源类型
            if !perm_object.starts_with(&format!("{}:", resource_type)) {
                continue;
            }

            // 检查权限是否匹配
            if let Ok(perm) = perm_str.parse::<Permission>() {
                if !perm.grants(&required_permission) {
                    continue;
                }
            }

            // 如果有通配符权限，返回 None (允许所有)
            if perm_object == all_wildcard {
                return Ok(None);
            }

            // 提取资源 ID
            if let Some(id) = perm_object.strip_prefix(&format!("{}:", resource_type)) {
                if !permitted_objects.contains(&id.to_string()) {
                    permitted_objects.push(id.to_string());
                }
            }
        }
    }

    Ok(Some(permitted_objects))
}
```

### Step 2: 添加公共 API

**文件**: `src/handler/http/auth/validator.rs`

```rust
/// Visdata 版本: 获取用户有权限的对象列表
#[cfg(all(not(feature = "enterprise"), feature = "visdata"))]
pub(crate) async fn list_objects_for_user(
    org_id: &str,
    user_id: &str,
    permission: &str,
    object_type: &str,
) -> Result<Option<Vec<String>>, Error> {
    use crate::common::meta::user::UserRole;

    // Root 用户跳过过滤
    if let Some(user) = users::get_user(Some(org_id), user_id).await {
        if user.role == UserRole::Root {
            return Ok(None);
        }
    }

    // 检查 visdata 是否初始化
    if !visdata::is_initialized() {
        return Ok(None);
    }

    // 将 HTTP permission 映射到 Visdata permission
    let vd_permission = match permission {
        "GET" => "AllowGet",
        "LIST" => "AllowList",
        "POST" => "AllowPost",
        "PUT" => "AllowPut",
        "DELETE" => "AllowDelete",
        _ => "AllowGet",
    };

    visdata::Visdata::global()
        .rbac()
        .list_permitted_objects(org_id, user_id, object_type, vd_permission)
        .await
        .map_err(|e| ErrorForbidden(e.to_string()))
}

/// 开源版本: 无过滤
#[cfg(all(not(feature = "enterprise"), not(feature = "visdata")))]
pub(crate) async fn list_objects_for_user(
    _org_id: &str,
    _user_id: &str,
    _permission: &str,
    _object_type: &str,
) -> Result<Option<Vec<String>>, Error> {
    Ok(None) // 不过滤
}
```

### Step 3: 修改 Stream 列表接口 (示例)

**文件**: `src/handler/http/request/stream/mod.rs`

```rust
// 现有 enterprise 代码块后添加 visdata 版本
#[cfg(all(not(feature = "enterprise"), feature = "visdata"))]
{
    let stream_type_key = match stream_type {
        StreamType::Logs => "logs",
        StreamType::Metrics => "metrics",
        StreamType::Traces => "traces",
        StreamType::EnrichmentTables => "enrichment_tables",
        StreamType::Index => "index",
        _ => "logs",
    };

    match crate::handler::http::auth::validator::list_objects_for_user(
        &org_id,
        &user_id,
        "GET",
        stream_type_key,
    ).await {
        Ok(list) => {
            _stream_list_from_rbac = list;
        }
        Err(e) => {
            return Ok(MetaHttpResponse::forbidden(e.to_string()));
        }
    }
}
```

---

## 六、资源类型映射表

Visdata 使用的资源类型标识符：

| 资源 | object_type | 通配符格式 | 具体格式 |
|------|-------------|------------|----------|
| Logs Stream | `logs` | `logs:_all_{org}` | `logs:{stream_name}` |
| Metrics Stream | `metrics` | `metrics:_all_{org}` | `metrics:{stream_name}` |
| Traces Stream | `traces` | `traces:_all_{org}` | `traces:{stream_name}` |
| Alerts | `alerts` | `alerts:_all_{org}` | `alerts:{alert_id}` |
| Dashboards | `dashboard` | `dashboard:_all_{org}` | `dashboard:{dashboard_id}` |
| Pipelines | `pipelines` | `pipelines:_all_{org}` | `pipelines:{pipeline_id}` |
| Functions | `functions` | `functions:_all_{org}` | `functions:{function_name}` |
| Reports | `reports` | `reports:_all_{org}` | `reports:{report_id}` |

---

## 七、修改文件汇总

### 新增文件
无

### 修改文件

| 文件 | 修改内容 | 优先级 |
|------|----------|--------|
| `crates/visdata/src/rbac/engine.rs` | 添加 `list_permitted_objects()` | 高 |
| `src/handler/http/auth/validator.rs` | 添加 visdata 版本的 `list_objects_for_user()` | 高 |
| `src/handler/http/request/stream/mod.rs` | 添加 visdata 过滤条件 | 高 |
| `src/handler/http/request/alerts/mod.rs` | 添加 visdata 过滤条件 | 高 |
| `src/handler/http/request/dashboards/mod.rs` | 添加 visdata 过滤条件 | 高 |
| `src/handler/http/request/pipeline.rs` | 添加 visdata 过滤条件 | 中 |
| `src/handler/http/request/functions/mod.rs` | 添加 visdata 过滤条件 | 中 |

---

## 八、实现顺序建议

### 第一阶段: 核心框架
- 实现 `list_permitted_objects()` 函数
- 实现 `list_objects_for_user()` 公共 API
- 添加单元测试

### 第二阶段: 核心资源
- 修改 Stream 列表接口
- 修改 Alert 列表接口
- 修改 Dashboard 列表接口

### 第三阶段: 扩展资源
- 修改 Pipeline 列表接口
- 修改 Function 列表接口
- 修改其他列表接口

---

## 九、注意事项

1. **性能考虑**: `list_permitted_objects()` 结果应该利用现有缓存
2. **兼容性**: 确保 Root 用户始终返回 None (不过滤)
3. **错误处理**: 权限检查失败应返回 403，而不是空列表
4. **条件编译**: 使用 `#[cfg(all(not(feature = "enterprise"), feature = "visdata"))]` 确保不与 Enterprise 冲突

---

## 十、测试验证

### 10.1 测试场景

| 场景 | 预期结果 |
|------|----------|
| Root 用户请求列表 | 返回所有资源 |
| Admin 角色用户 (有 `_all_` 权限) | 返回所有资源 |
| Viewer 角色用户 (有部分权限) | 只返回有权限的资源 |
| 无角色用户 | 返回空列表 |

### 10.2 验证命令

```bash
# 编译
cargo check --features visdata

# 运行测试
cargo test --features visdata -p visdata

# 构建
cargo build --release --features visdata
```

---

*文档创建时间: 2025-12-14*
