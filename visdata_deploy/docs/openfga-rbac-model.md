# OpenFGA RBAC 权限模型文档

## 概述

本文档描述 OpenObserve 基于 OpenFGA 实现的 RBAC（基于角色的访问控制）权限模型。该模型支持细粒度的资源权限控制、角色继承、组织隔离等企业级特性。

---

## 1. 核心概念

### 1.1 权限模型架构

```
┌─────────────────────────────────────────────────────────────────┐
│                         组织 (org)                               │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                      角色 (role)                         │    │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐      │    │
│  │  │   用户组    │  │    用户     │  │   权限集    │      │    │
│  │  │  (group)    │  │   (user)    │  │ (ALLOW_*)   │      │    │
│  │  └─────────────┘  └─────────────┘  └─────────────┘      │    │
│  └─────────────────────────────────────────────────────────┘    │
│                              │                                   │
│                              ▼                                   │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                     资源 (Resources)                     │    │
│  │  stream, dashboard, alert, function, pipeline, ...      │    │
│  └─────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
```

### 1.2 核心类型

| 类型 | 说明 | 用途 |
|------|------|------|
| `user` | 用户 | 系统中的个人用户 |
| `group` | 用户组 | 用户的集合，便于批量授权 |
| `role` | 角色 | 权限的集合，可分配给用户或组 |
| `org` | 组织 | 多租户隔离的顶级容器 |

### 1.3 权限类型

每个资源类型都支持以下标准权限：

| 权限 | HTTP 方法 | 说明 |
|------|-----------|------|
| `ALLOW_ALL` | * | 完全访问权限 |
| `ALLOW_GET` | GET | 读取单个资源 |
| `ALLOW_LIST` | GET (list) | 列出资源集合 |
| `ALLOW_POST` | POST | 创建新资源 |
| `ALLOW_PUT` | PUT | 更新现有资源 |
| `ALLOW_DELETE` | DELETE | 删除资源 |

---

## 2. 资源类型详解

### 2.1 数据流相关

| 类型 | 说明 | 父级关系 |
|------|------|----------|
| `stream` | 数据流（抽象父级） | - |
| `logs` | 日志流 | stream |
| `metrics` | 指标流 | stream |
| `traces` | 追踪流 | stream |
| `metadata` | 元数据流 | stream |
| `index` | 索引 | stream |

### 2.2 可视化相关

| 类型 | 说明 | 父级关系 |
|------|------|----------|
| `dashboard` | 仪表板 | dfolder |
| `dfolder` | 仪表板文件夹 | dfolder (自引用) |
| `report` | 报表 | rfolder |
| `rfolder` | 报表文件夹 | rfolder (自引用) |
| `savedviews` | 保存的视图 | - |

### 2.3 告警相关

| 类型 | 说明 | 父级关系 |
|------|------|----------|
| `alert` | 告警规则 | afolder |
| `afolder` | 告警文件夹 | afolder (自引用) |
| `template` | 告警模板 | - |
| `destination` | 告警目标 | - |

### 2.4 数据处理相关

| 类型 | 说明 |
|------|------|
| `function` | VRL 函数 |
| `pipeline` | 数据管道 |
| `enrichment_table` | 数据增强表 |
| `summary` | 数据摘要 |

### 2.5 系统管理相关

| 类型 | 说明 |
|------|------|
| `settings` | 系统设置 |
| `kv` | 键值存储 |
| `syslog-route` | Syslog 路由 |
| `ratelimit` | 速率限制 |
| `cipher_keys` | 加密密钥 |
| `license` | 许可证管理 |

### 2.6 用户与安全相关

| 类型 | 说明 |
|------|------|
| `passcode` | 访问密码 |
| `rumtoken` | RUM 令牌 |
| `service_accounts` | 服务账户 |
| `search_jobs` | 搜索任务 |
| `action_scripts` | 操作脚本 |

### 2.7 其他

| 类型 | 说明 |
|------|------|
| `ai` | AI 功能 |
| `re_patterns` | 正则表达式模式 |

---

## 3. 权限继承机制

### 3.1 继承来源

权限可以从以下来源获得：

```
┌─────────────────────────────────────────────────────────────┐
│                      权限来源优先级                          │
├─────────────────────────────────────────────────────────────┤
│ 1. 直接授权: role#has → ALLOW_* → resource                  │
│ 2. 父级继承: resource.selfParent → ALLOW_*                  │
│ 3. 层级继承: resource.parent → permission (folder结构)      │
│ 4. 组织角色: org.viewer/editor/admin                        │
└─────────────────────────────────────────────────────────────┘
```

### 3.2 权限计算规则

以 `GET` 权限为例：

```
GET = ALLOW_ALL
   or ALLOW_GET
   or ALLOW_ALL from selfParent
   or ALLOW_GET from selfParent
   or viewer from owningOrg (部分资源)
```

### 3.3 角色权限生效条件

角色的 `has` 关系是权限生效的关键：

```
has = members AND org_context from owningOrg

其中:
- members = assigned (直接分配的用户) OR member from grp_assigned (组成员)
- org_context = 用户必须在组织上下文中
```

**重要**: 用户必须同时满足：
1. 是角色成员（直接分配或通过组）
2. 拥有组织的 `org_context` 关系

---

## 4. 组织权限模型

### 4.1 组织内置角色

| 关系 | 说明 | 权限范围 |
|------|------|----------|
| `admin` | 管理员 | 完全控制组织及所有资源 |
| `editor` | 编辑者 | 读写组织资源 |
| `viewer` | 查看者 | 只读访问 |
| `allowed_user` | 允许用户 | 基本访问权限 |
| `org_context` | 组织上下文 | 权限生效的前提条件 |

### 4.2 组织权限计算

```
org.GET = ALLOW_ALL or ALLOW_GET or allowed_user or admin or editor or viewer
org.POST = ALLOW_ALL or ALLOW_POST or admin or editor
org.PUT = ALLOW_ALL or ALLOW_PUT or admin or editor
org.DELETE = ALLOW_ALL or ALLOW_DELETE or admin or editor
org.LIST = ALLOW_ALL or ALLOW_LIST or allowed_user or admin or editor or viewer
```

---

## 5. 特殊资源权限模型

### 5.1 Dashboard/Alert（支持文件夹层级）

```
dashboard.GET = ALLOW_ALL
             or ALLOW_GET
             or GET from parent (dfolder)
             or GET_INDIVIDUAL_FROM_ROLE from selfContext

dashboard.DELETE = ALLOW_ALL
                or ALLOW_DELETE
                or DELETE from parent (dfolder)
                or DELETE_INDIVIDUAL_FROM_ROLE from selfContext
```

### 5.2 Stream 子类型（日志/指标/追踪）

```
logs.GET = ALLOW_ALL
        or ALLOW_GET
        or ALLOW_ALL from selfParent
        or ALLOW_GET from selfParent
        or GET from parent (stream)
```

---

## 6. 权限检查流程

### 6.1 API 请求权限检查

```
1. 解析请求路径和方法
   └── GET /api/{org_id}/streams/{stream_name}

2. 确定资源类型和权限
   └── 资源: stream:{org_id}/{stream_name}
   └── 权限: GET

3. 构建 OpenFGA 检查请求
   └── user: user:{user_email}
   └── relation: GET
   └── object: stream:{resource_id}

4. 执行权限检查
   └── OpenFGA Check API

5. 返回结果
   └── allowed: true/false
```

### 6.2 批量权限检查

对于 LIST 操作，使用 OpenFGA 的 ListObjects API：

```
ListObjects(
    user: "user:admin@example.com",
    relation: "LIST",
    type: "stream"
)
→ 返回用户有权访问的所有 stream 列表
```

---

## 7. 模型定义 (DSL)

### 7.1 用户类型

```fga
type user
  relations
    define ALLOW_ALL: [role#has]
    define ALLOW_DELETE: [role#has]
    define ALLOW_GET: [role#has]
    define ALLOW_LIST: [role#has]
    define ALLOW_POST: [role#has]
    define ALLOW_PUT: [role#has]
    define DELETE: ALLOW_ALL or ALLOW_DELETE or ALLOW_ALL from selfParent or ALLOW_DELETE from selfParent
    define GET: ALLOW_ALL or ALLOW_GET or viewer from owningOrg or ALLOW_ALL from selfParent or ALLOW_GET from selfParent
    define LIST: ALLOW_ALL or ALLOW_LIST or viewer from owningOrg or LIST from selfParent or allowed_user from owningOrg
    define POST: ALLOW_ALL or ALLOW_POST or ALLOW_ALL from selfParent or ALLOW_POST from selfParent
    define PUT: ALLOW_ALL or ALLOW_PUT or ALLOW_ALL from selfParent or ALLOW_PUT from selfParent
    define owningOrg: [org]
    define selfParent: [user]
```

### 7.2 组织类型

```fga
type org
  relations
    define ALLOW_ALL: [role#has]
    define ALLOW_DELETE: [role#has]
    define ALLOW_GET: [role#has]
    define ALLOW_LIST: [role#has]
    define ALLOW_POST: [role#has]
    define ALLOW_PUT: [role#has]
    define DELETE: ALLOW_ALL or ALLOW_DELETE or ALLOW_ALL from selfParent or ALLOW_DELETE from selfParent or admin or editor
    define GET: ALLOW_ALL or ALLOW_GET or ALLOW_ALL from selfParent or ALLOW_GET from selfParent or allowed_user or admin or editor or viewer
    define LIST: ALLOW_ALL or ALLOW_LIST or LIST from selfParent or allowed_user or admin or editor or viewer
    define POST: ALLOW_ALL or ALLOW_POST or ALLOW_ALL from selfParent or ALLOW_POST from selfParent or admin or editor
    define PUT: ALLOW_ALL or ALLOW_PUT or ALLOW_ALL from selfParent or ALLOW_PUT from selfParent or admin or editor
    define admin: [user] and org_context
    define allowed_user: [user] and org_context
    define editor: [user] and org_context
    define org_context: [user]
    define selfParent: [org]
    define viewer: [user] and org_context
```

### 7.3 角色类型

```fga
type role
  relations
    define ALLOW_ALL: [role#has]
    define ALLOW_DELETE: [role#has]
    define ALLOW_GET: [role#has]
    define ALLOW_LIST: [role#has]
    define ALLOW_POST: [role#has]
    define ALLOW_PUT: [role#has]
    define DELETE: ALLOW_ALL or ALLOW_DELETE or ALLOW_ALL from selfParent or ALLOW_DELETE from selfParent
    define GET: ALLOW_ALL or ALLOW_GET or viewer from owningOrg or ALLOW_ALL from selfParent or ALLOW_GET from selfParent
    define LIST: ALLOW_ALL or ALLOW_LIST or viewer from owningOrg or LIST from selfParent
    define POST: ALLOW_ALL or ALLOW_POST or ALLOW_ALL from selfParent or ALLOW_POST from selfParent
    define PUT: ALLOW_ALL or ALLOW_PUT or ALLOW_ALL from selfParent or ALLOW_PUT from selfParent
    define assigned: [user]
    define grp_assigned: [group]
    define has: members and org_context from owningOrg
    define members: member from grp_assigned or assigned
    define owningOrg: [org]
    define selfParent: [role]
    define super_user: [user]
```

### 7.4 标准资源类型模板

```fga
type {resource_type}
  relations
    define ALLOW_ALL: [role#has]
    define ALLOW_DELETE: [role#has]
    define ALLOW_GET: [role#has]
    define ALLOW_LIST: [role#has]
    define ALLOW_POST: [role#has]
    define ALLOW_PUT: [role#has]
    define DELETE: ALLOW_ALL or ALLOW_DELETE or ALLOW_ALL from selfParent or ALLOW_DELETE from selfParent
    define GET: ALLOW_ALL or ALLOW_GET or ALLOW_ALL from selfParent or ALLOW_GET from selfParent
    define LIST: ALLOW_ALL or ALLOW_LIST or ALLOW_ALL from selfParent or ALLOW_LIST from selfParent
    define POST: ALLOW_ALL or ALLOW_POST or ALLOW_ALL from selfParent or ALLOW_POST from selfParent
    define PUT: ALLOW_ALL or ALLOW_PUT or ALLOW_ALL from selfParent or ALLOW_PUT from selfParent
    define owningOrg: [org]
    define selfParent: [{resource_type}]
```

---

## 8. 最佳实践

### 8.1 资源命名约定

```
{resource_type}:{org_id}/{resource_name}

示例:
- stream:default/access_logs
- dashboard:default/system_overview
- role:default/developer
```

### 8.2 通配符资源

使用 `_all_{org_id}` 表示组织下的所有资源：

```
stream:_all_default     - default 组织的所有 stream
dashboard:_all_default  - default 组织的所有 dashboard
```

### 8.3 权限分配建议

1. **最小权限原则**: 只授予必要的权限
2. **使用角色**: 通过角色批量管理权限，避免直接授权
3. **组织隔离**: 确保跨组织资源隔离
4. **定期审计**: 定期检查权限分配是否合理

---

## 9. 附录

### 9.1 完整资源类型列表

```
user, group, role, org,
function, dashboard, dfolder, template, destination,
alert, enrichment_table, settings, kv, syslog-route,
summary, stream, logs, metrics, traces, metadata, index,
passcode, rumtoken, savedviews, report, pipeline,
service_accounts, search_jobs, cipher_keys, action_scripts,
afolder, rfolder, ratelimit, ai, re_patterns, license
```

### 9.2 相关文档

- [OpenFGA 官方文档](https://openfga.dev/docs)
- [OpenObserve RBAC API 文档](./rbac-api.md)
- [初始化数据说明](./openfga-init-data.md)
