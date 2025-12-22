# OpenFGA 初始化脚本

本目录包含 OpenObserve RBAC 系统的 OpenFGA 初始化文件。

## 文件说明

| 文件 | 说明 |
|------|------|
| `model.fga` | OpenFGA 授权模型定义（DSL 格式） |
| `tuples.yaml` | 初始化数据（关系元组） |
| `store.yaml` | 完整的 Store 配置（模型 + 数据，可一次性导入） |
| `init.sh` | 初始化脚本（Linux/Mac） |

## 快速开始

### 方式一：使用 fga CLI 一次性导入

```bash
# 安装 fga CLI
go install github.com/openfga/cli/cmd/fga@latest

# 一次性导入 Store、模型和数据
fga store import --file store.yaml

# 或指定 API 地址
fga store import --api-url http://localhost:8080 --file store.yaml
```

### 方式二：分步导入

```bash
# 1. 创建 Store
fga store create --name openobserve
# 输出: store_id: 01HXXXXXX

# 2. 设置环境变量
export FGA_STORE_ID=01HXXXXXX

# 3. 导入授权模型
fga model write --file model.fga
# 输出: authorization_model_id: 01HXXXXXX

# 4. 导入初始化数据
fga tuple write --file tuples.yaml
```

### 方式三：使用 init.sh 脚本

```bash
chmod +x init.sh
./init.sh http://localhost:8080
```

## 验证

```bash
# 设置环境变量
export FGA_STORE_ID=<your-store-id>

# 检查 root 用户对 default 组织的管理权限
fga query check user:root@openobserve.ai GET org:default
# 预期: allowed: true

# 检查 root 用户对 stream 的访问权限
fga query check user:root@openobserve.ai GET stream:_all_default
# 预期: allowed: true

# 列出 root 用户可访问的所有组织
fga query list-objects user:root@openobserve.ai GET org
```

## 初始化内容

### 组织

| 组织 ID | 说明 |
|---------|------|
| `default` | 默认业务组织 |
| `_meta` | 系统元数据组织 |

### Root 用户

- **用户标识**: `user:root@openobserve.ai`
- **权限**: 同时是 `default` 和 `_meta` 组织的管理员
- **org_context**: 已配置，权限立即生效

### 资源类型

为每个组织初始化了以下资源的通配符权限：

- 数据流: `stream`, `logs`, `metrics`, `traces`, `metadata`, `index`
- 可视化: `dashboard`, `dfolder`, `savedviews`, `report`, `rfolder`
- 告警: `alert`, `afolder`, `template`, `destination`
- 数据处理: `function`, `pipeline`, `enrichment_table`, `summary`
- 系统管理: `settings`, `kv`, `syslog-route`, `ratelimit`, `cipher_keys`, `license`
- 用户安全: `user`, `group`, `role`, `passcode`, `rumtoken`, `service_accounts`, `search_jobs`, `action_scripts`
- 其他: `ai`, `re_patterns`

## 相关文档

- [权限模型说明](../docs/openfga-rbac-model.md)
- [初始化数据说明](../docs/openfga-init-data.md)
- [OpenFGA 官方文档](https://openfga.dev/docs)
