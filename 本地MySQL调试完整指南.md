# OpenObserve 本地 MySQL 调试完整指南(Windows)

## 目录

1. [前置准备](#1-前置准备)
2. [MySQL 配置](#2-mysql-配置)
3. [后端配置与启动](#3-后端配置与启动)
4. [前端配置与启动](#4-前端配置与启动)
5. [验证与调试](#5-验证与调试)
6. [常见问题与解决方案](#6-常见问题与解决方案)

---

## 1. 前置准备

### 1.1 安装必要的工具

#### ✅ Rust 开发环境

```powershell
# 1. 安装 Rust(如果尚未安装)
# 访问 https://rustup.rs/ 下载安装

# 2. 验证安装
rustc --version
cargo --version

# 3. 安装 C++ 编译工具(OpenObserve 需要)
# 下载并安装 Visual Studio Build Tools 2022
# https://visualstudio.microsoft.com/downloads/
# 选择 "Desktop development with C++"
```

#### ✅ Node.js 和 npm

```powershell
# 1. 安装 Node.js(推荐 LTS 版本)
# 访问 https://nodejs.org/ 下载安装

# 2. 验证安装
node --version  # 应该显示 v18.x 或更高
npm --version
```

#### ✅ MySQL 8.0+

```powershell
# 1. 下载 MySQL 8.0+ for Windows
# https://dev.mysql.com/downloads/mysql/

# 2. 安装 MySQL(推荐使用 MySQL Installer)
# 或使用 Chocolatey 安装:
choco install mysql

# 3. 验证安装
mysql --version
```

---

## 2. MySQL 配置

### 2.1 启动 MySQL 服务

```powershell
# 方法 1: 使用 Windows 服务
net start MySQL80  # 服务名称可能是 MySQL 或 MySQL80

# 方法 2: 通过 MySQL Workbench 启动
# 打开 MySQL Workbench,连接到本地实例
```

### 2.2 创建数据库和用户

```sql
-- 1. 登录 MySQL
mysql -u root -p

-- 2. 创建数据库
CREATE DATABASE openobserve CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- 3. 创建专用用户(可选,推荐)
CREATE USER 'o2_user'@'localhost' IDENTIFIED BY 'o2_password_123';

-- 4. 授予权限
GRANT ALL PRIVILEGES ON openobserve.* TO 'o2_user'@'localhost';
FLUSH PRIVILEGES;

-- 5. 验证
SHOW DATABASES;
USE openobserve;
SHOW TABLES;  -- 初始应该为空

-- 6. 退出
EXIT;
```

**重要提示:**
- ⚠️ MySQL 支持已被标记为 **DEPRECATED**(即将废弃)
- 📌 官方推荐迁移到 **PostgreSQL**
- ✅ 但目前仍可用于开发调试

---

## 3. 后端配置与启动

### 3.1 配置环境变量

在项目根目录创建 `.env` 文件:

```bash
# .env 文件内容

# ==================== 基础配置 ====================
# 本地模式(必须设置为 false 才能使用 MySQL)
ZO_LOCAL_MODE=false

# 节点角色(all = 所有组件在一个节点运行)
ZO_NODE_ROLE=all

# ==================== 用户配置 ====================
# Root 用户配置
ZO_ROOT_USER_EMAIL=admin@example.com
ZO_ROOT_USER_PASSWORD=Complexpass#123

# ==================== MySQL 配置 ====================
# 元数据存储类型(必须设置为 mysql)
ZO_META_STORE=mysql

# MySQL 连接字符串
ZO_META_MYSQL_DSN=mysql://o2_user:o2_password_123@localhost:3306/openobserve

# MySQL 只读连接(可选,用于读写分离)
# ZO_META_MYSQL_RO_DSN=mysql://o2_user:o2_password_123@localhost:3306/openobserve

# ==================== 存储配置 ====================
# 数据目录(WAL 和本地缓存)
ZO_DATA_DIR=C:/openobserve/data
ZO_DATA_WAL_DIR=C:/openobserve/data/wal

# 对象存储配置(可选,使用本地磁盘)
ZO_LOCAL_MODE_STORAGE=disk

# 或使用 MinIO(如果已部署)
# ZO_S3_BUCKET_NAME=openobserve
# ZO_S3_ENDPOINT=http://localhost:9000
# ZO_S3_ACCESS_KEY=minioadmin
# ZO_S3_SECRET_KEY=minioadmin

# ==================== 性能配置 ====================
# 文件大小限制(MB)
ZO_MAX_FILE_SIZE_ON_DISK=32

# 文件推送间隔(秒)
ZO_FILE_PUSH_INTERVAL=10

# MemTable 大小(MB)
ZO_MEM_TABLE_MAX_SIZE=256

# ==================== 日志配置 ====================
RUST_LOG=info,openobserve=debug

# ==================== HTTP/gRPC 配置 ====================
ZO_HTTP_PORT=5080
ZO_GRPC_PORT=5081

# ==================== 调试配置 ====================
# 启用调试模式
RUST_BACKTRACE=1
```

### 3.2 创建数据目录

```powershell
# 创建数据目录
New-Item -ItemType Directory -Force -Path C:\openobserve\data
New-Item -ItemType Directory -Force -Path C:\openobserve\data\wal
```

### 3.3 编译后端

```powershell
# 1. 进入项目目录
cd C:\Users\ltdhk\Documents\DevSpace\ClaudeCode\openobserve

# 2. 检查 Rust 工具链
rustup show

# 3. 编译(debug 模式,编译速度快)
cargo build

# 或编译 release 模式(性能更好但编译慢)
# cargo build --release

# 4. 等待编译完成(首次编译可能需要 10-30 分钟)
```

**编译优化建议:**

如果编译太慢,可以修改 `Cargo.toml`:

```toml
# 添加到 Cargo.toml
[profile.dev]
opt-level = 1  # 轻微优化,加快编译速度
```

### 3.4 运行数据库迁移(自动)

OpenObserve 会在启动时自动运行数据库迁移,无需手动操作。

### 3.5 启动后端

```powershell
# 方法 1: 直接运行编译后的二进制文件
.\target\debug\openobserve.exe

# 方法 2: 使用 cargo run(推荐用于开发)
cargo run

# 方法 3: 使用 release 版本(性能更好)
# cargo run --release
```

**启动成功标志:**

```
[INFO] OpenObserve is starting...
[INFO] Version: 0.17.0
[INFO] Node role: all
[INFO] Meta store: mysql
[INFO] MySQL connection established
[INFO] Running database migrations...
[INFO] Migrations completed successfully
[INFO] Job initialization complete
[INFO] HTTP server listening on 0.0.0.0:5080
[INFO] gRPC server listening on 0.0.0.0:5081
```

**⚠️ 如果看到 MySQL 废弃警告:**

```
╔════════════════════════════════════════════════════════════════════════════╗
║                              ⚠️  WARNING  ⚠️                                 ║
║                                                                            ║
║  MySQL support is DEPRECATED and will be removed in future.                ║
║  Please migrate to PostgreSQL.                                             ║
║                                                                            ║
╚════════════════════════════════════════════════════════════════════════════╝
```

这是正常的,MySQL 仍可用于开发调试。

---

## 4. 前端配置与启动

### 4.1 安装前端依赖

```powershell
# 1. 进入前端目录
cd web

# 2. 安装依赖(首次需要)
npm install

# 或使用 pnpm(更快)
# npm install -g pnpm
# pnpm install
```

### 4.2 配置前端环境变量(可选)

创建 `web/.env.local`:

```bash
# web/.env.local

# 后端 API 地址
VITE_API_BASE_URL=http://localhost:5080
```

### 4.3 启动前端开发服务器

```powershell
# 在 web 目录下运行
npm run dev

# 或使用 pnpm
# pnpm dev
```

**启动成功标志:**

```
VITE v5.x.x  ready in xxx ms

  ➜  Local:   http://localhost:8080/
  ➜  Network: http://192.168.x.x:8080/
  ➜  press h + enter to show help
```

### 4.4 访问前端界面

打开浏览器访问:

```
http://localhost:8080
```

**首次登录:**
- 用户名:`admin@example.com`
- 密码:`Complexpass#123`

---

## 5. 验证与调试

### 5.1 验证后端运行状态

```powershell
# 1. 检查 HTTP API
curl http://localhost:5080/healthz

# 或使用浏览器访问
# http://localhost:5080/healthz

# 2. 检查 MySQL 连接
mysql -u o2_user -p openobserve
# 输入密码: o2_password_123

# 查看已创建的表
SHOW TABLES;

# 应该看到类似:
# +---------------------------+
# | Tables_in_openobserve     |
# +---------------------------+
# | alerts                    |
# | dashboards                |
# | file_list                 |
# | functions                 |
# | meta                      |
# | pipelines                 |
# | scheduled_jobs            |
# | streams                   |
# | users                     |
# | ...                       |
# +---------------------------+
```

### 5.2 验证前端连接

```powershell
# 1. 打开浏览器开发者工具(F12)
# 2. 访问 http://localhost:8080
# 3. 查看 Network 标签,应该看到对 http://localhost:5080 的 API 请求
# 4. 查看 Console 标签,确认无错误
```

### 5.3 发送测试日志

```powershell
# 使用 curl 发送测试日志
curl -X POST http://localhost:5080/api/default/logs/_json `
  -H "Content-Type: application/json" `
  -u "admin@example.com:Complexpass#123" `
  -d '[{"timestamp": 1711094400000, "level": "INFO", "message": "Test log from Windows"}]'

# 或使用 PowerShell 的 Invoke-RestMethod
$headers = @{
    "Content-Type" = "application/json"
}
$body = '[{"timestamp": 1711094400000, "level": "INFO", "message": "Test log from PowerShell"}]'
$credential = [Convert]::ToBase64String([Text.Encoding]::ASCII.GetBytes("admin@example.com:Complexpass#123"))

Invoke-RestMethod -Uri "http://localhost:5080/api/default/logs/_json" `
    -Method POST `
    -Headers @{"Authorization" = "Basic $credential"; "Content-Type" = "application/json"} `
    -Body $body
```

### 5.4 查询日志

在前端界面:

1. 登录后,点击左侧菜单 **Logs**
2. 选择 Stream: `default`
3. 点击 **Run Query**
4. 应该能看到刚才发送的测试日志

---

## 6. 常见问题与解决方案

### 问题 1: 编译失败 - `link.exe not found`

**原因:** 缺少 C++ 编译工具

**解决方案:**

```powershell
# 1. 安装 Visual Studio Build Tools 2022
# https://visualstudio.microsoft.com/downloads/
# 选择 "Desktop development with C++"

# 2. 重启 PowerShell
# 3. 重新编译
cargo clean
cargo build
```

---

### 问题 2: MySQL 连接失败 - `Access denied`

**错误信息:**

```
Error: Meta store is MySQL, but connection failed: Access denied for user 'o2_user'@'localhost'
```

**解决方案:**

```sql
-- 1. 检查用户权限
mysql -u root -p

SHOW GRANTS FOR 'o2_user'@'localhost';

-- 2. 重新授权
GRANT ALL PRIVILEGES ON openobserve.* TO 'o2_user'@'localhost';
FLUSH PRIVILEGES;

-- 3. 或直接使用 root 用户(仅开发环境)
# 修改 .env 文件:
ZO_META_MYSQL_DSN=mysql://root:your_root_password@localhost:3306/openobserve
```

---

### 问题 3: 端口被占用

**错误信息:**

```
Error: Address already in use (os error 10048)
```

**解决方案:**

```powershell
# 1. 查看占用端口的进程
netstat -ano | findstr :5080

# 2. 杀掉占用进程
taskkill /PID <PID> /F

# 3. 或修改端口
# 编辑 .env 文件:
ZO_HTTP_PORT=5090
ZO_GRPC_PORT=5091
```

---

### 问题 4: 前端无法连接后端

**现象:** 前端页面空白或 API 请求失败

**解决方案:**

```powershell
# 1. 检查后端是否运行
curl http://localhost:5080/healthz

# 2. 检查 CORS 配置(通常自动配置)
# 后端日志应该显示:
# [INFO] CORS enabled for all origins in development mode

# 3. 检查浏览器控制台错误
# F12 -> Console 标签

# 4. 确认前端配置正确
# web/.env.local:
VITE_API_BASE_URL=http://localhost:5080
```

---

### 问题 5: 数据目录权限问题

**错误信息:**

```
Error: Permission denied (os error 5)
```

**解决方案:**

```powershell
# 1. 以管理员身份运行 PowerShell
# 右键 PowerShell -> "以管理员身份运行"

# 2. 修改数据目录权限
icacls C:\openobserve\data /grant Everyone:F /T

# 3. 或使用用户目录
# 修改 .env:
ZO_DATA_DIR=C:/Users/ltdhk/AppData/Local/openobserve/data
ZO_DATA_WAL_DIR=C:/Users/ltdhk/AppData/Local/openobserve/data/wal
```

---

### 问题 6: MySQL 迁移失败

**错误信息:**

```
Error: Migration failed: ...
```

**解决方案:**

```sql
-- 1. 清空数据库(重新开始)
mysql -u root -p

DROP DATABASE openobserve;
CREATE DATABASE openobserve CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- 2. 重新启动后端(会自动运行迁移)
cargo run
```

---

## 7. 调试技巧

### 7.1 后端调试

#### 使用 Visual Studio Code

创建 `.vscode/launch.json`:

```json
{
  "version": "0.2.0",
  "configurations": [
    {
      "type": "lldb",
      "request": "launch",
      "name": "Debug OpenObserve",
      "cargo": {
        "args": [
          "build",
          "--bin=openobserve",
          "--package=openobserve"
        ],
        "filter": {
          "name": "openobserve",
          "kind": "bin"
        }
      },
      "args": [],
      "cwd": "${workspaceFolder}",
      "env": {
        "RUST_LOG": "debug"
      }
    }
  ]
}
```

安装 VS Code 扩展:
- **rust-analyzer** - Rust 语言支持
- **CodeLLDB** - LLDB 调试器

#### 查看详细日志

```powershell
# 设置日志级别为 debug
$env:RUST_LOG="debug,openobserve=trace"
cargo run
```

### 7.2 前端调试

#### 使用 Vue DevTools

1. 安装 Chrome 扩展: **Vue.js devtools**
2. 打开浏览器开发者工具(F12)
3. 切换到 **Vue** 标签
4. 可以查看组件树、状态、事件等

#### 查看 API 请求

1. F12 打开开发者工具
2. 切换到 **Network** 标签
3. 过滤: `XHR` 或 `Fetch`
4. 点击请求查看详细信息

---

## 8. 快速启动脚本

### 启动脚本(PowerShell)

创建 `start-dev.ps1`:

```powershell
# start-dev.ps1 - 快速启动开发环境

Write-Host "========== 启动 OpenObserve 开发环境 ==========" -ForegroundColor Green

# 1. 检查 MySQL 服务
Write-Host "[1/4] 检查 MySQL 服务..." -ForegroundColor Yellow
$mysqlService = Get-Service -Name "MySQL80" -ErrorAction SilentlyContinue
if ($mysqlService.Status -ne "Running") {
    Write-Host "启动 MySQL 服务..." -ForegroundColor Yellow
    Start-Service -Name "MySQL80"
}

# 2. 启动后端
Write-Host "[2/4] 启动后端..." -ForegroundColor Yellow
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$PWD'; cargo run"

# 等待后端启动
Start-Sleep -Seconds 10

# 3. 启动前端
Write-Host "[3/4] 启动前端..." -ForegroundColor Yellow
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$PWD\web'; npm run dev"

# 4. 打开浏览器
Write-Host "[4/4] 打开浏览器..." -ForegroundColor Yellow
Start-Sleep -Seconds 5
Start-Process "http://localhost:8080"

Write-Host "========== 启动完成! ==========" -ForegroundColor Green
Write-Host "后端: http://localhost:5080" -ForegroundColor Cyan
Write-Host "前端: http://localhost:8080" -ForegroundColor Cyan
Write-Host "用户: admin@example.com / Complexpass#123" -ForegroundColor Cyan
```

**使用方法:**

```powershell
# 赋予执行权限(首次)
Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser

# 运行脚本
.\start-dev.ps1
```

---

## 9. 推荐的开发工具

| 工具 | 用途 | 下载地址 |
|------|------|---------|
| **Visual Studio Code** | 代码编辑器 | https://code.visualstudio.com/ |
| **rust-analyzer** | Rust 语言支持(VS Code 扩展) | VS Code 扩展市场 |
| **MySQL Workbench** | MySQL 数据库管理 | https://www.mysql.com/products/workbench/ |
| **DBeaver** | 通用数据库工具(可选) | https://dbeaver.io/ |
| **Postman** | API 测试工具 | https://www.postman.com/ |
| **Vue DevTools** | Vue.js 调试工具 | Chrome 扩展商店 |

---

## 10. 总结

### ✅ 完整启动流程

1. **启动 MySQL 服务**
   ```powershell
   net start MySQL80
   ```

2. **创建数据库和用户**
   ```sql
   CREATE DATABASE openobserve;
   CREATE USER 'o2_user'@'localhost' IDENTIFIED BY 'o2_password_123';
   GRANT ALL PRIVILEGES ON openobserve.* TO 'o2_user'@'localhost';
   ```

3. **配置 `.env` 文件**
   ```bash
   ZO_LOCAL_MODE=false
   ZO_META_STORE=mysql
   ZO_META_MYSQL_DSN=mysql://o2_user:o2_password_123@localhost:3306/openobserve
   ZO_ROOT_USER_EMAIL=admin@example.com
   ZO_ROOT_USER_PASSWORD=Complexpass#123
   ```

4. **启动后端**
   ```powershell
   cargo run
   ```

5. **启动前端**
   ```powershell
   cd web
   npm run dev
   ```

6. **访问前端**
   ```
   http://localhost:8080
   ```

### 📌 关键配置总结

| 配置项 | 值 | 说明 |
|--------|-------|------|
| `ZO_LOCAL_MODE` | `false` | 必须设置为 false 才能使用 MySQL |
| `ZO_META_STORE` | `mysql` | 元数据存储类型 |
| `ZO_META_MYSQL_DSN` | `mysql://user:pass@host:port/db` | MySQL 连接字符串 |
| `ZO_NODE_ROLE` | `all` | 所有组件在一个节点运行 |
| `ZO_HTTP_PORT` | `5080` | HTTP API 端口 |
| `ZO_GRPC_PORT` | `5081` | gRPC 端口 |

### ⚠️ 重要提示

1. **MySQL 支持已被废弃**,推荐生产环境使用 PostgreSQL
2. **本地开发**建议使用 `debug` 模式编译(速度快)
3. **数据目录**权限确保当前用户有读写权限
4. **端口冲突**检查 5080/5081/8080 端口是否被占用

---

**祝您调试顺利!** 🎉

如有问题,欢迎查看:
- 官方文档: https://openobserve.ai/docs/
- GitHub Issues: https://github.com/openobserve/openobserve/issues
