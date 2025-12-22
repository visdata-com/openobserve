# Docker 部署 OpenFGA 和 Dex 指南

本文档介绍如何通过 Docker 部署 OpenObserve 企业版功能所需的 OpenFGA (RBAC) 和 Dex (SSO) 服务。

## 目录

- [架构概述](#架构概述)
- [前置条件](#前置条件)
- [快速开始](#快速开始)
- [配置文件详解](#配置文件详解)
- [初始化步骤](#初始化步骤)
- [OpenObserve 集成配置](#openobserve-集成配置)
- [生产环境注意事项](#生产环境注意事项)
- [常见问题](#常见问题)

## 架构概述

```
┌─────────────────────────────────────────────────────────────┐
│                      OpenObserve                             │
│                                                              │
│  ┌──────────────┐              ┌──────────────┐             │
│  │   RBAC       │              │    SSO       │             │
│  │  (权限控制)   │              │  (单点登录)   │             │
│  └──────┬───────┘              └──────┬───────┘             │
└─────────┼────────────────────────────┼──────────────────────┘
          │                            │
          ▼                            ▼
┌─────────────────┐          ┌─────────────────┐
│    OpenFGA      │          │      Dex        │
│   :8080 (HTTP)  │          │  :5556 (OIDC)   │
│   :8081 (gRPC)  │          │  :5557 (gRPC)   │
└────────┬────────┘          └────────┬────────┘
         │                            │
         └──────────┬─────────────────┘
                    ▼
          ┌─────────────────┐
          │   PostgreSQL    │
          │     :5432       │
          └─────────────────┘
```

### 组件说明

| 组件 | 用途 | 端口 |
|------|------|------|
| OpenFGA | 细粒度权限控制 (RBAC) | 8080 (HTTP), 8081 (gRPC) |
| Dex | 身份认证与 SSO | 5556 (OIDC), 5557 (gRPC) |
| PostgreSQL | 数据持久化存储 | 5432 |

## 前置条件

- Docker Engine 20.10+
- Docker Compose v2.0+
- 至少 2GB 可用内存
- 网络端口：5432, 5556, 5557, 8080, 8081

## 快速开始

### 1. 创建部署目录

```bash
mkdir visdata_deploy && cd visdata_deploy
```

### 2. 创建配置文件

需要创建以下文件：
- `docker-compose.visdata.yml` - Docker Compose 主配置
- `dex-config.yaml` - Dex 服务配置
- `init-multiple-dbs.sh` - PostgreSQL 初始化脚本

### 3. 启动服务

```bash
# 启动所有服务
docker-compose -f docker-compose.visdata.yml up -d

# 查看服务状态
docker-compose -f docker-compose.visdata.yml ps

# 查看日志
docker-compose -f docker-compose.visdata.yml logs -f
```

### 4. 初始化 OpenFGA

```bash
# 创建 Store
curl -X POST http://localhost:8080/stores \
  -H "Content-Type: application/json" \
  -d "{\"name\": \"openobserve\"}"
```

## 配置文件详解

### docker-compose.visdata.yml

```yaml
version: '3.8'

services:
  # ==========================================
  # PostgreSQL - 共享数据库
  # ==========================================
  postgres:
    image: postgres:15-alpine
    container_name: visdata-postgres
    environment:
      - POSTGRES_USER=postgres
      - POSTGRES_PASSWORD=postgres
    volumes:
      - ./init-multiple-dbs.sh:/docker-entrypoint-initdb.d/init-multiple-dbs.sh:ro
      - postgres_data:/var/lib/postgresql/data
    ports:
      - "5432:5432"
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres"]
      interval: 5s
      timeout: 5s
      retries: 5
    networks:
      - visdata-network

  # ==========================================
  # OpenFGA 数据库迁移 (一次性)
  # ==========================================
  openfga-migrate:
    image: openfga/openfga:latest
    container_name: openfga-migrate
    command: migrate
    environment:
      - OPENFGA_DATASTORE_ENGINE=postgres
      - OPENFGA_DATASTORE_URI=postgres://openfga:openfga@postgres:5432/openfga?sslmode=disable
    depends_on:
      postgres:
        condition: service_healthy
    networks:
      - visdata-network

  # ==========================================
  # OpenFGA - RBAC 权限管理
  # ==========================================
  openfga:
    image: openfga/openfga:latest
    container_name: openfga
    command: run
    environment:
      - OPENFGA_DATASTORE_ENGINE=postgres
      - OPENFGA_DATASTORE_URI=postgres://openfga:openfga@postgres:5432/openfga?sslmode=disable
      - OPENFGA_HTTP_ADDR=0.0.0.0:8080
      - OPENFGA_GRPC_ADDR=0.0.0.0:8081
      - OPENFGA_LOG_FORMAT=json
    ports:
      - "8080:8080"
      - "8081:8081"
    depends_on:
      openfga-migrate:
        condition: service_completed_successfully
      postgres:
        condition: service_healthy
    healthcheck:
      test: ["CMD", "wget", "-q", "--spider", "http://localhost:8080/healthz"]
      interval: 10s
      timeout: 5s
      retries: 5
    networks:
      - visdata-network

  # ==========================================
  # Dex - SSO 身份认证
  # ==========================================
  dex:
    image: dexidp/dex:latest
    container_name: dex
    command: ["dex", "serve", "/etc/dex/config.yaml"]
    volumes:
      - ./dex-config.yaml:/etc/dex/config.yaml:ro
    ports:
      - "5556:5556"
      - "5557:5557"
    depends_on:
      postgres:
        condition: service_healthy
    healthcheck:
      test: ["CMD", "wget", "-q", "--spider", "http://localhost:5556/healthz"]
      interval: 10s
      timeout: 5s
      retries: 5
    networks:
      - visdata-network

networks:
  visdata-network:
    driver: bridge

volumes:
  postgres_data:
```

### init-multiple-dbs.sh

```bash
#!/bin/bash
set -e

# 创建 OpenFGA 数据库和用户
psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" <<-EOSQL
    CREATE USER openfga WITH PASSWORD 'openfga';
    CREATE DATABASE openfga;
    GRANT ALL PRIVILEGES ON DATABASE openfga TO openfga;

    CREATE USER dex WITH PASSWORD 'dex';
    CREATE DATABASE dex;
    GRANT ALL PRIVILEGES ON DATABASE dex TO dex;
EOSQL

# 授权 schema 权限 (PostgreSQL 15+)
psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" -d openfga <<-EOSQL
    GRANT ALL ON SCHEMA public TO openfga;
EOSQL

psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" -d dex <<-EOSQL
    GRANT ALL ON SCHEMA public TO dex;
EOSQL

echo "=== 数据库初始化完成 ==="
```

> **Windows 用户注意**: 该脚本必须使用 LF 换行符 (Unix 格式)，否则会执行失败。

### dex-config.yaml

```yaml
# ==================================================
# Dex 身份认证服务配置
# ==================================================

# OIDC Issuer URL - 客户端用于发现 OIDC 配置
# 生产环境需要改为实际的外部访问地址
issuer: http://localhost:5556

# --------------------------------------------------
# 存储配置
# --------------------------------------------------
storage:
  type: postgres
  config:
    host: postgres
    port: 5432
    database: dex
    user: dex
    password: dex
    ssl:
      mode: disable

# --------------------------------------------------
# Web 服务配置
# --------------------------------------------------
web:
  http: 0.0.0.0:5556
  # 生产环境启用 HTTPS:
  # https: 0.0.0.0:5556
  # tlsCert: /etc/dex/tls/tls.crt
  # tlsKey: /etc/dex/tls/tls.key

# --------------------------------------------------
# gRPC API 配置 (管理接口)
# --------------------------------------------------
grpc:
  addr: 0.0.0.0:5557
  reflection: true
  # 生产环境启用 TLS:
  # tlsCert: /etc/dex/tls/tls.crt
  # tlsKey: /etc/dex/tls/tls.key

# --------------------------------------------------
# OAuth2 配置
# --------------------------------------------------
oauth2:
  skipApprovalScreen: true
  alwaysShowLoginScreen: false

# --------------------------------------------------
# 静态客户端配置 (OpenObserve)
# --------------------------------------------------
staticClients:
  - id: openobserve
    name: 'OpenObserve'
    # 重要: 生产环境必须更换为强密钥
    secret: openobserve-secret-change-me
    redirectURIs:
      - 'http://localhost:5080/auth/callback'
      - 'http://localhost:5080/web/cb'
    public: false

# --------------------------------------------------
# 启用本地密码登录
# --------------------------------------------------
enablePasswordDB: true

# --------------------------------------------------
# 身份提供商连接器 (Connectors)
# --------------------------------------------------
connectors:
  # GitHub OAuth 示例
  # - type: github
  #   id: github
  #   name: GitHub
  #   config:
  #     clientID: $GITHUB_CLIENT_ID
  #     clientSecret: $GITHUB_CLIENT_SECRET
  #     redirectURI: http://localhost:5556/callback
  #     orgs:
  #       - name: your-organization

  # LDAP 连接器示例
  # - type: ldap
  #   id: ldap
  #   name: LDAP
  #   config:
  #     host: ldap.example.com:636
  #     insecureNoSSL: false
  #     bindDN: cn=admin,dc=example,dc=com
  #     bindPW: admin-password
  #     userSearch:
  #       baseDN: ou=users,dc=example,dc=com
  #       filter: "(objectClass=person)"
  #       username: uid
  #       emailAttr: mail
  #       nameAttr: cn
  #     groupSearch:
  #       baseDN: ou=groups,dc=example,dc=com
  #       filter: "(objectClass=groupOfNames)"
  #       userMatchers:
  #         - userAttr: DN
  #           groupAttr: member
  #       nameAttr: cn

  # OIDC 连接器示例 (连接其他 IdP)
  # - type: oidc
  #   id: google
  #   name: Google
  #   config:
  #     issuer: https://accounts.google.com
  #     clientID: $GOOGLE_CLIENT_ID
  #     clientSecret: $GOOGLE_CLIENT_SECRET
  #     redirectURI: http://localhost:5556/callback

# --------------------------------------------------
# 静态用户 (仅用于测试)
# --------------------------------------------------
staticPasswords:
  - email: "admin@example.com"
    # 密码: password
    # 生成方式: htpasswd -nbBC 10 "" password | tr -d ':\n'
    hash: "$2a$10$2b2cU8CPhOTaGrs1HRQuAueS7JTT5ZHsHSzYiFPm1leZck7Mc8T4W"
    username: "admin"
    userID: "admin-user-id"

# --------------------------------------------------
# Token 过期配置
# --------------------------------------------------
expiry:
  signingKeys: "6h"
  idTokens: "24h"
  refreshTokens:
    validIfNotUsedFor: "168h"   # 7 天未使用则过期
    absoluteLifetime: "720h"    # 30 天绝对过期

# --------------------------------------------------
# 日志配置
# --------------------------------------------------
logger:
  level: "info"
  format: "json"
```

## 初始化步骤

### 1. 设置脚本权限 (Linux/macOS)

```bash
chmod +x init-multiple-dbs.sh
```

### 2. 启动服务

```bash
docker-compose -f docker-compose.visdata.yml up -d
```

### 3. 验证服务健康状态

```bash
# 检查所有服务状态
docker-compose -f docker-compose.visdata.yml ps

# 验证 OpenFGA
curl http://localhost:8080/healthz

# 验证 Dex
curl http://localhost:5556/healthz

# 获取 Dex OIDC 配置
curl http://localhost:5556/.well-known/openid-configuration
```

### 4. 初始化 OpenFGA Store

#### Linux/macOS:

```bash
curl -X POST http://localhost:8080/stores \
  -H "Content-Type: application/json" \
  -d '{"name": "openobserve"}'
```

#### Windows PowerShell:

```powershell
curl -X POST http://localhost:8080/stores -H "Content-Type: application/json" -d "{\"name\": \"openobserve\"}"
```

#### Windows (使用文件):

创建 `store.json`:
```json
{"name": "openobserve"}
```

执行:
```powershell
curl -X POST http://localhost:8080/stores -H "Content-Type: application/json" -d @store.json
```

### 5. 记录返回的 Store ID

```json
{
  "id": "01JFXXXXXXXXXXXXXX",
  "name": "openobserve",
  "created_at": "2024-01-01T00:00:00.000000Z",
  "updated_at": "2024-01-01T00:00:00.000000Z"
}
```

> 记录 `id` 字段的值，后续配置 OpenObserve 时需要使用。

## OpenObserve 集成配置

### 环境变量配置

```bash
# ========================================
# OpenFGA 配置
# ========================================
ZO_OPENFGA_URL=http://localhost:8080
ZO_OPENFGA_STORE_NAME=openobserve
# 如果已知 Store ID，可以直接配置
# ZO_OPENFGA_STORE_ID=01JFXXXXXXXXXXXXXX

# ========================================
# Dex 配置
# ========================================
ZO_DEX_GRPC_URL=http://localhost:5557
ZO_DEX_ISSUER_URL=http://localhost:5556
ZO_DEX_CLIENT_ID=openobserve
ZO_DEX_CLIENT_SECRET=openobserve-secret-change-me
ZO_DEX_REDIRECT_URI=http://localhost:5080/auth/callback

# ========================================
# 企业功能开关
# ========================================
ZO_RBAC_ENABLED=true
ZO_SSO_ENABLED=true
```

### 默认端口对照表

| 配置项 | 默认值 | 说明 |
|--------|--------|------|
| `openfga_url` | `http://localhost:8080` | OpenFGA HTTP API |
| `dex_grpc_url` | `http://localhost:5557` | Dex gRPC 管理接口 |
| `dex_issuer_url` | `http://localhost:5556` | Dex OIDC 发现端点 |
| `dex_client_id` | `openobserve` | OAuth2 客户端 ID |
| `dex_redirect_uri` | `http://localhost:5080/auth/callback` | OAuth2 回调地址 |

## 生产环境注意事项

### 安全配置清单

| 项目 | 开发环境 | 生产环境 |
|------|---------|---------|
| Dex HTTPS | HTTP | **必须启用 HTTPS** |
| OpenFGA TLS | 无 | 推荐启用 |
| PostgreSQL SSL | disable | **必须启用** |
| 静态密码 | 可用 | **必须禁用** |
| Client Secret | 简单值 | **使用强随机密钥** |
| 数据库密码 | 简单值 | **使用强随机密码** |

### 1. 启用 Dex HTTPS

```yaml
# dex-config.yaml
web:
  https: 0.0.0.0:5556
  tlsCert: /etc/dex/tls/tls.crt
  tlsKey: /etc/dex/tls/tls.key

grpc:
  addr: 0.0.0.0:5557
  tlsCert: /etc/dex/tls/tls.crt
  tlsKey: /etc/dex/tls/tls.key
```

### 2. 使用 Secrets 管理敏感信息

```yaml
# docker-compose.visdata.yml
services:
  dex:
    environment:
      - DEX_CLIENT_SECRET_FILE=/run/secrets/dex_client_secret
    secrets:
      - dex_client_secret

secrets:
  dex_client_secret:
    file: ./secrets/dex_client_secret.txt
```

### 3. 配置持久化存储

```yaml
volumes:
  postgres_data:
    driver: local
    driver_opts:
      type: none
      o: bind
      device: /data/postgres
```

### 4. 配置资源限制

```yaml
services:
  openfga:
    deploy:
      resources:
        limits:
          cpus: '2'
          memory: 2G
        reservations:
          cpus: '0.5'
          memory: 512M
```

### 5. 配置日志轮转

```yaml
services:
  openfga:
    logging:
      driver: "json-file"
      options:
        max-size: "100m"
        max-file: "3"
```

## 常见问题

### Q1: OpenFGA 报错 "relation store does not exist"

**原因**: 数据库迁移未执行

**解决方案**:
```bash
# 手动执行迁移
docker exec -it openfga /openfga migrate \
  --datastore-engine postgres \
  --datastore-uri "postgres://openfga:openfga@postgres:5432/openfga?sslmode=disable"
```

### Q2: Windows 下 curl 命令 JSON 报错

**原因**: Windows 命令行不支持单引号

**解决方案**: 使用双引号并转义内部引号
```powershell
curl -X POST http://localhost:8080/stores -H "Content-Type: application/json" -d "{\"name\": \"openobserve\"}"
```

### Q3: init-multiple-dbs.sh 执行失败

**原因**: Windows 换行符 (CRLF) 导致脚本执行失败

**解决方案**:
1. 使用支持 LF 的编辑器 (如 VS Code)
2. 转换换行符: `dos2unix init-multiple-dbs.sh`
3. 或在 Git 中配置: `git config core.autocrlf input`

### Q4: Dex 无法连接 PostgreSQL

**原因**: 网络或配置问题

**检查步骤**:
```bash
# 检查网络连通性
docker exec dex ping postgres

# 检查数据库连接
docker exec -it visdata-postgres psql -U dex -d dex -c "SELECT 1"

# 查看 Dex 日志
docker logs dex
```

### Q5: 如何重置所有数据

```bash
# 停止服务
docker-compose -f docker-compose.visdata.yml down

# 删除数据卷
docker volume rm visdata_deploy_postgres_data

# 重新启动
docker-compose -f docker-compose.visdata.yml up -d
```

### Q6: 如何备份数据

```bash
# 备份 PostgreSQL
docker exec visdata-postgres pg_dumpall -U postgres > backup.sql

# 恢复
docker exec -i visdata-postgres psql -U postgres < backup.sql
```

## 参考链接

- [OpenFGA 官方文档](https://openfga.dev/docs)
- [Dex 官方文档](https://dexidp.io/docs/)
- [OpenObserve 文档](https://openobserve.ai/docs/)

---

## 版本信息

| 组件 | 推荐版本 |
|------|---------|
| OpenFGA | v1.5.0+ |
| Dex | v2.37.0+ |
| PostgreSQL | 15+ |

---

*最后更新: 2025-01*
