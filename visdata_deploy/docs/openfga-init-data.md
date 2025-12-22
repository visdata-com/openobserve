# OpenFGA 初始化数据文档

## 概述

本文档描述 OpenObserve RBAC 系统的初始化数据，包括默认组织、root 用户和基础资源的配置。

---

## 1. 组织结构

### 1.1 预置组织

| 组织 ID | 说明 | 用途 |
|---------|------|------|
| `default` | 默认组织 | 系统默认的业务组织 |
| `_meta` | 元数据组织 | 存储系统内部数据（审计日志等） |

---

## 2. Root 用户配置

### 2.1 Root 用户

Root 用户是系统的超级管理员，拥有所有组织的完全控制权限。

```yaml
用户标识: user:root@openobserve.ai
组织角色:
  - admin → org:default
  - admin → org:_meta
组织上下文:
  - org_context → org:default
  - org_context → org:_meta
```

### 2.2 Root 用户 Tuples

```yaml
# Root 用户归属于 default 组织
- user: org:default
  relation: owningOrg
  object: user:root@openobserve.ai

# Root 用户是 default 组织的管理员
- user: user:root@openobserve.ai
  relation: admin
  object: org:default

# Root 用户有 default 组织的上下文
- user: user:root@openobserve.ai
  relation: org_context
  object: org:default

# Root 用户是 _meta 组织的管理员
- user: user:root@openobserve.ai
  relation: admin
  object: org:_meta

# Root 用户有 _meta 组织的上下文
- user: user:root@openobserve.ai
  relation: org_context
  object: org:_meta
```

---

## 3. Default 组织资源初始化

### 3.1 资源所有权 (owningOrg)

为 default 组织创建所有资源类型的通配符资源：

```yaml
# 数据流相关
- user: org:default
  relation: owningOrg
  object: stream:_all_default

- user: org:default
  relation: owningOrg
  object: logs:_all_default

- user: org:default
  relation: owningOrg
  object: metrics:_all_default

- user: org:default
  relation: owningOrg
  object: traces:_all_default

- user: org:default
  relation: owningOrg
  object: metadata:_all_default

- user: org:default
  relation: owningOrg
  object: index:_all_default

# 可视化相关
- user: org:default
  relation: owningOrg
  object: dashboard:_all_default

- user: org:default
  relation: owningOrg
  object: dfolder:_all_default

- user: org:default
  relation: owningOrg
  object: savedviews:_all_default

- user: org:default
  relation: owningOrg
  object: report:_all_default

- user: org:default
  relation: owningOrg
  object: rfolder:_all_default

# 告警相关
- user: org:default
  relation: owningOrg
  object: alert:_all_default

- user: org:default
  relation: owningOrg
  object: afolder:_all_default

- user: org:default
  relation: owningOrg
  object: template:_all_default

- user: org:default
  relation: owningOrg
  object: destination:_all_default

# 数据处理相关
- user: org:default
  relation: owningOrg
  object: function:_all_default

- user: org:default
  relation: owningOrg
  object: pipeline:_all_default

- user: org:default
  relation: owningOrg
  object: enrichment_table:_all_default

- user: org:default
  relation: owningOrg
  object: summary:_all_default

# 系统管理相关
- user: org:default
  relation: owningOrg
  object: settings:_all_default

- user: org:default
  relation: owningOrg
  object: kv:_all_default

- user: org:default
  relation: owningOrg
  object: syslog-route:_all_default

- user: org:default
  relation: owningOrg
  object: ratelimit:_all_default

- user: org:default
  relation: owningOrg
  object: cipher_keys:_all_default

- user: org:default
  relation: owningOrg
  object: license:_all_default

# 用户与安全相关
- user: org:default
  relation: owningOrg
  object: user:_all_default

- user: org:default
  relation: owningOrg
  object: group:_all_default

- user: org:default
  relation: owningOrg
  object: role:_all_default

- user: org:default
  relation: owningOrg
  object: passcode:_all_default

- user: org:default
  relation: owningOrg
  object: rumtoken:_all_default

- user: org:default
  relation: owningOrg
  object: service_accounts:_all_default

- user: org:default
  relation: owningOrg
  object: search_jobs:_all_default

- user: org:default
  relation: owningOrg
  object: action_scripts:_all_default

# 其他
- user: org:default
  relation: owningOrg
  object: ai:_all_default

- user: org:default
  relation: owningOrg
  object: re_patterns:_all_default
```

### 3.2 文件夹层级结构

```yaml
# 默认 Dashboard 文件夹
- user: org:default
  relation: owningOrg
  object: dfolder:default

- user: dfolder:_all_default
  relation: selfParent
  object: dfolder:default

# 默认 Alert 文件夹
- user: org:default
  relation: owningOrg
  object: afolder:default

- user: afolder:_all_default
  relation: selfParent
  object: afolder:default
```

### 3.3 Stream 父子关系

```yaml
# logs 继承自 stream
- user: stream:_all_default
  relation: parent
  object: logs:_all_default

# metrics 继承自 stream
- user: stream:_all_default
  relation: parent
  object: metrics:_all_default

# traces 继承自 stream
- user: stream:_all_default
  relation: parent
  object: traces:_all_default

# index 继承自 stream
- user: stream:_all_default
  relation: parent
  object: index:_all_default

# metadata 继承自 stream
- user: stream:_all_default
  relation: parent
  object: metadata:_all_default
```

---

## 4. _meta 组织资源初始化

### 4.1 资源所有权 (owningOrg)

```yaml
# 审计日志（特殊资源）
- user: org:_meta
  relation: owningOrg
  object: logs:audit

# 数据流相关
- user: org:_meta
  relation: owningOrg
  object: stream:_all__meta

- user: org:_meta
  relation: owningOrg
  object: logs:_all__meta

- user: org:_meta
  relation: owningOrg
  object: metrics:_all__meta

- user: org:_meta
  relation: owningOrg
  object: traces:_all__meta

- user: org:_meta
  relation: owningOrg
  object: metadata:_all__meta

- user: org:_meta
  relation: owningOrg
  object: index:_all__meta

# 可视化相关
- user: org:_meta
  relation: owningOrg
  object: dashboard:_all__meta

- user: org:_meta
  relation: owningOrg
  object: dfolder:_all__meta

- user: org:_meta
  relation: owningOrg
  object: savedviews:_all__meta

- user: org:_meta
  relation: owningOrg
  object: report:_all__meta

- user: org:_meta
  relation: owningOrg
  object: rfolder:_all__meta

# 告警相关
- user: org:_meta
  relation: owningOrg
  object: alert:_all__meta

- user: org:_meta
  relation: owningOrg
  object: afolder:_all__meta

- user: org:_meta
  relation: owningOrg
  object: template:_all__meta

- user: org:_meta
  relation: owningOrg
  object: destination:_all__meta

# 数据处理相关
- user: org:_meta
  relation: owningOrg
  object: function:_all__meta

- user: org:_meta
  relation: owningOrg
  object: pipeline:_all__meta

- user: org:_meta
  relation: owningOrg
  object: enrichment_table:_all__meta

- user: org:_meta
  relation: owningOrg
  object: summary:_all__meta

# 系统管理相关
- user: org:_meta
  relation: owningOrg
  object: settings:_all__meta

- user: org:_meta
  relation: owningOrg
  object: kv:_all__meta

- user: org:_meta
  relation: owningOrg
  object: syslog-route:_all__meta

- user: org:_meta
  relation: owningOrg
  object: ratelimit:_all__meta

- user: org:_meta
  relation: owningOrg
  object: cipher_keys:_all__meta

- user: org:_meta
  relation: owningOrg
  object: license:_all__meta

# 用户与安全相关
- user: org:_meta
  relation: owningOrg
  object: user:_all__meta

- user: org:_meta
  relation: owningOrg
  object: group:_all__meta

- user: org:_meta
  relation: owningOrg
  object: role:_all__meta

- user: org:_meta
  relation: owningOrg
  object: passcode:_all__meta

- user: org:_meta
  relation: owningOrg
  object: rumtoken:_all__meta

- user: org:_meta
  relation: owningOrg
  object: service_accounts:_all__meta

- user: org:_meta
  relation: owningOrg
  object: search_jobs:_all__meta

- user: org:_meta
  relation: owningOrg
  object: action_scripts:_all__meta

# 其他
- user: org:_meta
  relation: owningOrg
  object: ai:_all__meta

- user: org:_meta
  relation: owningOrg
  object: re_patterns:_all__meta
```

### 4.2 文件夹层级结构

```yaml
# 默认 Dashboard 文件夹
- user: org:_meta
  relation: owningOrg
  object: dfolder:default

- user: dfolder:_all__meta
  relation: selfParent
  object: dfolder:default

# 默认 Alert 文件夹
- user: org:_meta
  relation: owningOrg
  object: afolder:default

- user: afolder:_all__meta
  relation: selfParent
  object: afolder:default
```

### 4.3 Stream 父子关系

```yaml
- user: stream:_all__meta
  relation: parent
  object: logs:_all__meta

- user: stream:_all__meta
  relation: parent
  object: metrics:_all__meta

- user: stream:_all__meta
  relation: parent
  object: traces:_all__meta

- user: stream:_all__meta
  relation: parent
  object: index:_all__meta

- user: stream:_all__meta
  relation: parent
  object: metadata:_all__meta
```

---

## 5. 完整初始化 YAML

以下是可直接导入 OpenFGA 的完整初始化数据：

```yaml
name: openobserve
tuples:
  # ============================================
  # Root 用户配置
  # ============================================
  - user: org:default
    relation: owningOrg
    object: user:root@openobserve.ai
  - user: user:root@openobserve.ai
    relation: admin
    object: org:default
  - user: user:root@openobserve.ai
    relation: org_context
    object: org:default
  - user: user:root@openobserve.ai
    relation: admin
    object: org:_meta
  - user: user:root@openobserve.ai
    relation: org_context
    object: org:_meta

  # ============================================
  # Default 组织 - 资源所有权
  # ============================================
  - user: org:default
    relation: owningOrg
    object: cipher_keys:_all_default
  - user: org:default
    relation: owningOrg
    object: summary:_all_default
  - user: org:default
    relation: owningOrg
    object: template:_all_default
  - user: org:default
    relation: owningOrg
    object: action_scripts:_all_default
  - user: org:default
    relation: owningOrg
    object: syslog-route:_all_default
  - user: org:default
    relation: owningOrg
    object: group:_all_default
  - user: org:default
    relation: owningOrg
    object: re_patterns:_all_default
  - user: org:default
    relation: owningOrg
    object: user:_all_default
  - user: org:default
    relation: owningOrg
    object: destination:_all_default
  - user: org:default
    relation: owningOrg
    object: rumtoken:_all_default
  - user: org:default
    relation: owningOrg
    object: metadata:_all_default
  - user: org:default
    relation: owningOrg
    object: search_jobs:_all_default
  - user: org:default
    relation: owningOrg
    object: ratelimit:_all_default
  - user: org:default
    relation: owningOrg
    object: stream:_all_default
  - user: org:default
    relation: owningOrg
    object: ai:_all_default
  - user: org:default
    relation: owningOrg
    object: traces:_all_default
  - user: org:default
    relation: owningOrg
    object: dfolder:_all_default
  - user: org:default
    relation: owningOrg
    object: passcode:_all_default
  - user: org:default
    relation: owningOrg
    object: savedviews:_all_default
  - user: org:default
    relation: owningOrg
    object: afolder:_all_default
  - user: org:default
    relation: owningOrg
    object: kv:_all_default
  - user: org:default
    relation: owningOrg
    object: logs:_all_default
  - user: org:default
    relation: owningOrg
    object: metrics:_all_default
  - user: org:default
    relation: owningOrg
    object: function:_all_default
  - user: org:default
    relation: owningOrg
    object: dashboard:_all_default
  - user: org:default
    relation: owningOrg
    object: rfolder:_all_default
  - user: org:default
    relation: owningOrg
    object: settings:_all_default
  - user: org:default
    relation: owningOrg
    object: enrichment_table:_all_default
  - user: org:default
    relation: owningOrg
    object: service_accounts:_all_default
  - user: org:default
    relation: owningOrg
    object: license:_all_default
  - user: org:default
    relation: owningOrg
    object: alert:_all_default
  - user: org:default
    relation: owningOrg
    object: pipeline:_all_default
  - user: org:default
    relation: owningOrg
    object: report:_all_default
  - user: org:default
    relation: owningOrg
    object: role:_all_default
  - user: org:default
    relation: owningOrg
    object: index:_all_default

  # ============================================
  # Default 组织 - 文件夹层级
  # ============================================
  - user: org:default
    relation: owningOrg
    object: dfolder:default
  - user: dfolder:_all_default
    relation: selfParent
    object: dfolder:default
  - user: org:default
    relation: owningOrg
    object: afolder:default
  - user: afolder:_all_default
    relation: selfParent
    object: afolder:default

  # ============================================
  # Default 组织 - Stream 父子关系
  # ============================================
  - user: stream:_all_default
    relation: parent
    object: logs:_all_default
  - user: stream:_all_default
    relation: parent
    object: metrics:_all_default
  - user: stream:_all_default
    relation: parent
    object: traces:_all_default
  - user: stream:_all_default
    relation: parent
    object: index:_all_default
  - user: stream:_all_default
    relation: parent
    object: metadata:_all_default

  # ============================================
  # _meta 组织 - 资源所有权
  # ============================================
  - user: org:_meta
    relation: owningOrg
    object: logs:audit
  - user: org:_meta
    relation: owningOrg
    object: enrichment_table:_all__meta
  - user: org:_meta
    relation: owningOrg
    object: ratelimit:_all__meta
  - user: org:_meta
    relation: owningOrg
    object: service_accounts:_all__meta
  - user: org:_meta
    relation: owningOrg
    object: dfolder:_all__meta
  - user: org:_meta
    relation: owningOrg
    object: destination:_all__meta
  - user: org:_meta
    relation: owningOrg
    object: alert:_all__meta
  - user: org:_meta
    relation: owningOrg
    object: search_jobs:_all__meta
  - user: org:_meta
    relation: owningOrg
    object: settings:_all__meta
  - user: org:_meta
    relation: owningOrg
    object: summary:_all__meta
  - user: org:_meta
    relation: owningOrg
    object: role:_all__meta
  - user: org:_meta
    relation: owningOrg
    object: ai:_all__meta
  - user: org:_meta
    relation: owningOrg
    object: afolder:_all__meta
  - user: org:_meta
    relation: owningOrg
    object: rfolder:_all__meta
  - user: org:_meta
    relation: owningOrg
    object: re_patterns:_all__meta
  - user: org:_meta
    relation: owningOrg
    object: index:_all__meta
  - user: org:_meta
    relation: owningOrg
    object: action_scripts:_all__meta
  - user: org:_meta
    relation: owningOrg
    object: pipeline:_all__meta
  - user: org:_meta
    relation: owningOrg
    object: function:_all__meta
  - user: org:_meta
    relation: owningOrg
    object: license:_all__meta
  - user: org:_meta
    relation: owningOrg
    object: report:_all__meta
  - user: org:_meta
    relation: owningOrg
    object: syslog-route:_all__meta
  - user: org:_meta
    relation: owningOrg
    object: stream:_all__meta
  - user: org:_meta
    relation: owningOrg
    object: template:_all__meta
  - user: org:_meta
    relation: owningOrg
    object: group:_all__meta
  - user: org:_meta
    relation: owningOrg
    object: passcode:_all__meta
  - user: org:_meta
    relation: owningOrg
    object: cipher_keys:_all__meta
  - user: org:_meta
    relation: owningOrg
    object: traces:_all__meta
  - user: org:_meta
    relation: owningOrg
    object: dashboard:_all__meta
  - user: org:_meta
    relation: owningOrg
    object: metadata:_all__meta
  - user: org:_meta
    relation: owningOrg
    object: rumtoken:_all__meta
  - user: org:_meta
    relation: owningOrg
    object: user:_all__meta
  - user: org:_meta
    relation: owningOrg
    object: savedviews:_all__meta
  - user: org:_meta
    relation: owningOrg
    object: metrics:_all__meta
  - user: org:_meta
    relation: owningOrg
    object: logs:_all__meta
  - user: org:_meta
    relation: owningOrg
    object: kv:_all__meta

  # ============================================
  # _meta 组织 - 文件夹层级
  # ============================================
  - user: org:_meta
    relation: owningOrg
    object: dfolder:default
  - user: dfolder:_all__meta
    relation: selfParent
    object: dfolder:default
  - user: org:_meta
    relation: owningOrg
    object: afolder:default
  - user: afolder:_all__meta
    relation: selfParent
    object: afolder:default

  # ============================================
  # _meta 组织 - Stream 父子关系
  # ============================================
  - user: stream:_all__meta
    relation: parent
    object: logs:_all__meta
  - user: stream:_all__meta
    relation: parent
    object: metrics:_all__meta
  - user: stream:_all__meta
    relation: parent
    object: traces:_all__meta
  - user: stream:_all__meta
    relation: parent
    object: index:_all__meta
  - user: stream:_all__meta
    relation: parent
    object: metadata:_all__meta
```

---

## 6. 初始化命令

### 6.1 使用 FGA CLI 导入

```bash
# 创建 Store
fga store create --name openobserve

# 导入模型
fga model write --store-id <STORE_ID> --file model.fga

# 导入 Tuples
fga tuple write --store-id <STORE_ID> --file tuples.yaml
```

### 6.2 使用 API 导入

```bash
# 创建 Store
curl -X POST http://localhost:8080/stores \
  -H "Content-Type: application/json" \
  -d '{"name": "openobserve"}'

# 写入授权模型
curl -X POST http://localhost:8080/stores/{store_id}/authorization-models \
  -H "Content-Type: application/json" \
  -d @model.json

# 写入 Tuples
curl -X POST http://localhost:8080/stores/{store_id}/write \
  -H "Content-Type: application/json" \
  -d @tuples.json
```

---

## 7. 验证

### 7.1 验证 Root 用户权限

```bash
# 检查 root 用户对 default 组织的管理权限
fga query check \
  --store-id <STORE_ID> \
  --model-id <MODEL_ID> \
  user:root@openobserve.ai GET org:default

# 预期结果: allowed: true
```

### 7.2 验证资源访问

```bash
# 检查 root 用户对 stream 的访问权限
fga query check \
  --store-id <STORE_ID> \
  --model-id <MODEL_ID> \
  user:root@openobserve.ai GET stream:_all_default

# 预期结果: allowed: true
```

---

## 8. 相关文档

- [权限模型说明](./openfga-rbac-model.md)
- [OpenFGA 官方文档](https://openfga.dev/docs)
