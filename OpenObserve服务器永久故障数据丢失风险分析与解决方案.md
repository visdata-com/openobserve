# OpenObserve 服务器永久故障数据丢失风险分析与解决方案

## 核心问题

**用户提问:**如果数据写入了某个节点的 WAL,这时候这台服务器崩溃并且**无法恢复**(如磁盘物理损坏、服务器永久故障),因为 WAL 在那个服务器上,所以是不是这个时候数据就会**永久丢失**?

**答案:是的,这种情况下数据会丢失!** ⚠️

---

## 1. 数据丢失风险窗口分析

### 1.1 数据流转时间线

```
T0: 客户端发送数据
  ↓
T1: Ingester 写入 WAL (本地磁盘)
  ↓
T2: Ingester 返回 HTTP 200 OK
  ↓
[🔴 数据丢失风险窗口开始] ⚠️
  ↓
T3: MemTable 达到阈值/超时 (默认 5 分钟)
  ↓
T4: 转换为本地 Parquet 文件
  ↓
T5: 上传到对象存储 (S3/MinIO)
  ↓
[🟢 数据丢失风险窗口结束] ✅
  ↓
T6: 删除本地 Parquet 文件

数据丢失风险窗口 = T1 到 T5
通常持续 5-15 分钟

在这个窗口内:
✅ 数据已写入本地 WAL/MemTable/Parquet 文件
❌ 数据尚未上传到对象存储
❌ 没有任何副本或备份
⚠️ 如果服务器永久故障(磁盘损坏、机房火灾等),数据会丢失
```

### 1.2 风险量化计算

**假设配置:**
```bash
ZO_MEM_TABLE_MAX_SIZE=256        # MemTable 大小阈值 256MB
ZO_MAX_FILE_RETENTION_TIME=300   # 5 分钟强制转换
ZO_FILE_PUSH_INTERVAL=60         # 每分钟上传一次
```

**最坏情况:**
- 数据在 WAL 中停留:0-5 分钟(等待 MemTable 达到阈值)
- 数据在本地 Parquet 中停留:0-1 分钟(等待上传任务)
- **总风险窗口:最长 6 分钟**

**丢失数据量估算:**

假设日志流量为 **10GB/小时**:
```
风险窗口内的数据量 = 10GB ÷ 60分钟 × 6分钟 = 1GB
```

**如果服务器永久故障,丢失约 1GB 数据**

假设日志流量为 **100GB/小时**(高流量场景):
```
风险窗口内的数据量 = 100GB ÷ 60分钟 × 6分钟 = 10GB
```

**如果服务器永久故障,丢失约 10GB 数据**

### 1.3 为什么会有这个风险?

OpenObserve 的默认部署模式是 **Local Mode**(本地模式),**没有内置 WAL 复制机制**。这是一个**性能与可靠性的权衡**。

**代码依据** ([src/config/src/config.rs:747-751](src/config/src/config.rs#L747-L751)):

```rust
#[env_config(name = "ZO_LOCAL_MODE", default = true)]
pub local_mode: bool,

// ZO_LOCAL_MODE_STORAGE is ignored when ZO_LOCAL_MODE is set to false
#[env_config(name = "ZO_LOCAL_MODE_STORAGE", default = "disk")]
pub local_mode_storage: String,
```

**Local Mode 的设计理念:**
```rust
// src/config/src/config.rs:2230-2235
if cfg.common.local_mode {
    cfg.common.node_role = "all".to_string();  // 单节点模式
    cfg.common.node_role_group = "".to_string();
}
cfg.common.is_local_storage = cfg.common.local_mode
    && (cfg.common.local_mode_storage == "disk" || cfg.common.local_mode_storage == "local");
```

**Local Mode 的特点:**
- ✅ 数据只写入本地磁盘 WAL
- ✅ 无需跨节点网络通信,写入性能更高
- ❌ 没有跨节点复制
- ❌ 单点故障风险
- ⚠️ 适合开发/测试环境或对数据丢失容忍的场景

---

## 2. 解决方案:如何避免数据丢失

以下是 **5 种解决方案**,按可靠性从低到高排列。

### 方案 1:缩短数据上传间隔 ⚡ (治标不治本)

#### 原理
减少数据在本地停留的时间,尽快上传到对象存储。

#### 配置示例

```bash
# 减少 MemTable 保留时间(从 5 分钟降到 1 分钟)
ZO_MAX_FILE_RETENTION_TIME=60

# 增加文件上传频率(从 60 秒降到 10 秒)
ZO_FILE_PUSH_INTERVAL=10

# 减小 MemTable 大小阈值(从 256MB 降到 64MB)
ZO_MEM_TABLE_MAX_SIZE=64
```

#### 效果分析

**风险窗口变化:**
```
原配置:5-6 分钟
新配置:1-2 分钟
```

**丢失数据量变化:**
```
假设流量 10GB/小时:
原配置:1GB
新配置:200-300MB
```

#### 优缺点

| 优点 | 缺点 |
|------|------|
| ✅ 配置简单,无需修改架构 | ⚠️ 仍然存在数据丢失风险(只是减少) |
| ✅ 立即生效 | ⚠️ 增加对象存储 API 调用次数(成本上升) |
| | ⚠️ 增加小文件数量(需要 Compactor 更频繁合并) |
| | ⚠️ 可能影响性能(更频繁的 I/O 操作) |

#### 适用场景
- 对数据丢失容忍度较高的场景(如非关键日志)
- 临时缓解措施
- 开发/测试环境

---

### 方案 2:使用 RAID/LVM 镜像 💾 (硬件层面保护)

#### 原理
在服务器本地使用 RAID 1/10 或 LVM 镜像,确保磁盘冗余。

#### 实施步骤

**RAID 1 (镜像模式):**

```bash
# 1. 创建 RAID 1 镜像(2 块磁盘)
mdadm --create --verbose /dev/md0 \
  --level=1 \
  --raid-devices=2 \
  /dev/sda1 /dev/sdb1

# 2. 格式化
mkfs.ext4 /dev/md0

# 3. 挂载 WAL 目录到 RAID 设备
mkdir -p /data/openobserve/wal
mount /dev/md0 /data/openobserve/wal

# 4. 配置自动挂载
echo '/dev/md0 /data/openobserve/wal ext4 defaults 0 0' >> /etc/fstab

# 5. 配置 OpenObserve
export ZO_DATA_WAL_DIR=/data/openobserve/wal
```

**LVM 镜像:**

```bash
# 1. 创建物理卷
pvcreate /dev/sda1 /dev/sdb1

# 2. 创建卷组
vgcreate vg_wal /dev/sda1 /dev/sdb1

# 3. 创建镜像逻辑卷
lvcreate --type mirror -m 1 -L 500G -n lv_wal vg_wal

# 4. 格式化并挂载
mkfs.ext4 /dev/vg_wal/lv_wal
mount /dev/vg_wal/lv_wal /data/openobserve/wal
```

#### 效果分析

**保护范围:**
- ✅ 单块磁盘损坏 → 数据不丢失
- ❌ 整机故障(主板、电源) → 数据丢失
- ❌ 机房级故障(火灾、断电) → 数据丢失
- ❌ 人为误删除 → 数据丢失

**性能影响:**
- 写入性能:降低约 5-10%(需要写入 2 块磁盘)
- 读取性能:提升约 50%(可以从 2 块磁盘并行读取)

#### 优缺点

| 优点 | 缺点 |
|------|------|
| ✅ 保护单块磁盘损坏 | ❌ 无法防止服务器级别故障 |
| ✅ 透明,应用无需修改 | ❌ 无法防止人为误删除 |
| ✅ 成本较低(只需额外一块磁盘) | ⚠️ 写入性能略有下降 |

#### 适用场景
- 物理服务器部署
- 对磁盘故障有防护需求但不需要跨服务器冗余
- 预算有限

---

### 方案 3:使用网络附加存储 (NAS/NFS/EBS) 🌐 (共享存储)

#### 原理
将 WAL 目录挂载到网络存储(如 AWS EBS、Azure Disk、NFS),自动获得存储层的冗余。

#### 实施示例

**AWS EBS (推荐):**

```bash
# 1. 创建 EBS 卷(gp3 类型,自动跨 3 个可用区复制)
aws ec2 create-volume \
  --availability-zone us-west-2a \
  --volume-type gp3 \
  --size 500 \
  --tag-specifications 'ResourceType=volume,Tags=[{Key=Name,Value=openobserve-wal}]'

# 2. 挂载到 EC2 实例
aws ec2 attach-volume \
  --volume-id vol-xxxxx \
  --instance-id i-xxxxx \
  --device /dev/xvdf

# 3. 在 EC2 实例内格式化并挂载
sudo mkfs.ext4 /dev/xvdf
sudo mkdir -p /data/openobserve/wal
sudo mount /dev/xvdf /data/openobserve/wal

# 4. 配置 OpenObserve
export ZO_DATA_WAL_DIR=/data/openobserve/wal
```

**Kubernetes StatefulSet + AWS EBS CSI:**

```yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: openobserve-ingester
  namespace: openobserve
spec:
  serviceName: "openobserve-ingester"
  replicas: 3
  selector:
    matchLabels:
      app: openobserve-ingester
  template:
    metadata:
      labels:
        app: openobserve-ingester
    spec:
      containers:
      - name: openobserve
        image: public.ecr.aws/zinclabs/openobserve:latest
        env:
        - name: ZO_DATA_WAL_DIR
          value: /data/openobserve/wal
        volumeMounts:
        - name: wal-storage
          mountPath: /data/openobserve/wal
  volumeClaimTemplates:
  - metadata:
      name: wal-storage
    spec:
      accessModes: ["ReadWriteOnce"]
      storageClassName: gp3  # AWS EBS gp3 (内置跨可用区复制)
      resources:
        requests:
          storage: 100Gi
```

**Azure Disk (Premium SSD LRS):**

```yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: openobserve-ingester
spec:
  volumeClaimTemplates:
  - metadata:
      name: wal-storage
    spec:
      accessModes: ["ReadWriteOnce"]
      storageClassName: managed-premium  # Azure Premium SSD LRS
      resources:
        requests:
          storage: 100Gi
```

**NFS (本地网络存储):**

```bash
# 1. 在 NFS 服务器上配置
mkdir -p /export/openobserve/wal
echo '/export/openobserve/wal *(rw,sync,no_subtree_check)' >> /etc/exports
exportfs -ra

# 2. 在 OpenObserve 节点上挂载
sudo apt-get install nfs-common
sudo mkdir -p /data/openobserve/wal
sudo mount -t nfs nas-server:/export/openobserve/wal /data/openobserve/wal

# 3. 配置自动挂载
echo 'nas-server:/export/openobserve/wal /data/openobserve/wal nfs defaults 0 0' >> /etc/fstab
```

#### 效果分析

**保护范围:**
- ✅ 单块磁盘损坏 → 数据不丢失
- ✅ 服务器故障 → 数据不丢失(可在新服务器上挂载同一磁盘)
- ✅ AWS EBS/Azure Disk 有内置跨可用区复制
- ❌ 区域级故障(整个 AWS Region 故障) → 数据丢失

**性能影响:**
- AWS EBS gp3:延迟约 1-3ms(网络 I/O)
- NFS:延迟约 5-10ms(取决于网络)
- 吞吐量:AWS EBS gp3 最高 1000 MiB/s

#### 优缺点

| 优点 | 缺点 |
|------|------|
| ✅ 保护服务器级别故障 | ⚠️ 增加延迟(网络 I/O) |
| ✅ AWS EBS/Azure Disk 有内置复制 | ⚠️ 需要存储系统支持并发读写 |
| ✅ 透明,应用无需修改 | ⚠️ 成本较高(云存储费用) |
| ✅ 易于备份和快照 | |

#### 适用场景
- 云环境部署(AWS、Azure、GCP)
- 对可用性要求高但不希望修改应用代码
- 中等规模生产环境

#### 成本估算 (AWS EBS gp3)

```
100GB EBS gp3 卷:
- 存储成本:$8/月
- IOPS 成本(如果超过 3000 IOPS):每 1000 IOPS $6.5/月
- 吞吐量成本(如果超过 125 MiB/s):每 MiB/s $0.04/月

假设 3 个 Ingester 节点,每个 100GB:
总成本 = $8 × 3 = $24/月
```

---

### 方案 4:部署多个 Ingester + 客户端双写 🔄 (应用层面复制)

#### 原理
客户端同时向多个 Ingester 节点发送数据,确保至少一个节点成功写入。

#### 架构示意图

```
客户端
  ├─> Ingester-1 (WAL-1)  ──> S3
  ├─> Ingester-2 (WAL-2)  ──> S3
  └─> Ingester-3 (WAL-3)  ──> S3

只要任意一个 Ingester 成功,数据就不会丢失
即使 2 个节点永久故障,第 3 个节点仍有完整数据
```

#### 客户端实现示例

**Python 客户端:**

```python
import requests
import concurrent.futures
import hashlib
import time

INGESTER_NODES = [
    "http://ingester-1:5080",
    "http://ingester-2:5080",
    "http://ingester-3:5080"
]

def send_to_ingester(node_url, data):
    """发送数据到单个 Ingester 节点"""
    try:
        response = requests.post(
            f"{node_url}/api/default/logs/_json",
            json=data,
            timeout=5
        )
        return response.status_code == 200
    except Exception as e:
        print(f"❌ {node_url} 失败: {e}")
        return False

def send_logs_with_replication(log_data, min_success=2):
    """
    发送日志到多个 Ingester,至少 min_success 个成功才认为成功

    参数:
        log_data: 日志数据
        min_success: 最少需要成功的节点数
    """
    # 1. 为每条日志生成唯一 ID (用于去重)
    unique_id = hashlib.sha256(
        f"{log_data.get('timestamp', time.time())}{log_data}".encode()
    ).hexdigest()[:16]

    log_data["_id"] = unique_id

    # 2. 并发发送到所有 Ingester 节点
    with concurrent.futures.ThreadPoolExecutor(max_workers=len(INGESTER_NODES)) as executor:
        futures = [
            executor.submit(send_to_ingester, node, log_data)
            for node in INGESTER_NODES
        ]

        # 等待所有请求完成
        results = []
        for future in concurrent.futures.as_completed(futures):
            results.append(future.result())

    # 3. 检查成功的节点数
    success_count = sum(results)
    if success_count >= min_success:
        print(f"✅ 数据已写入 {success_count}/{len(INGESTER_NODES)} 个节点")
        return True
    else:
        print(f"❌ 只有 {success_count}/{len(INGESTER_NODES)} 个节点成功,需要 {min_success} 个")
        return False

# 使用示例
log_data = {
    "timestamp": int(time.time() * 1000),
    "level": "INFO",
    "message": "User login successful",
    "user_id": 12345
}

# 至少 2 个节点成功才认为写入成功
if send_logs_with_replication(log_data, min_success=2):
    print("日志写入成功")
else:
    print("日志写入失败,需要重试")
```

**Go 客户端:**

```go
package main

import (
    "bytes"
    "encoding/json"
    "net/http"
    "sync"
    "time"
)

var IngesterNodes = []string{
    "http://ingester-1:5080",
    "http://ingester-2:5080",
    "http://ingester-3:5080",
}

func sendToIngester(nodeURL string, data map[string]interface{}) bool {
    jsonData, _ := json.Marshal(data)

    resp, err := http.Post(
        nodeURL+"/api/default/logs/_json",
        "application/json",
        bytes.NewBuffer(jsonData),
    )
    if err != nil {
        return false
    }
    defer resp.Body.Close()

    return resp.StatusCode == 200
}

func sendLogsWithReplication(logData map[string]interface{}, minSuccess int) bool {
    var wg sync.WaitGroup
    results := make(chan bool, len(IngesterNodes))

    // 添加唯一 ID
    logData["_id"] = generateUniqueID(logData)

    // 并发发送到所有节点
    for _, node := range IngesterNodes {
        wg.Add(1)
        go func(nodeURL string) {
            defer wg.Done()
            results <- sendToIngester(nodeURL, logData)
        }(node)
    }

    wg.Wait()
    close(results)

    // 统计成功的节点数
    successCount := 0
    for success := range results {
        if success {
            successCount++
        }
    }

    return successCount >= minSuccess
}

func main() {
    logData := map[string]interface{}{
        "timestamp": time.Now().UnixMilli(),
        "level":     "INFO",
        "message":   "User login successful",
        "user_id":   12345,
    }

    if sendLogsWithReplication(logData, 2) {
        println("✅ 日志写入成功")
    } else {
        println("❌ 日志写入失败")
    }
}
```

#### 查询时去重

由于数据写入了多个 Ingester,可能产生重复数据。需要在查询时去重:

**方法 1:使用 `_id` 字段去重**

```sql
-- OpenObserve SQL 查询
SELECT DISTINCT * FROM logs
WHERE timestamp >= '2025-03-20T00:00:00Z'
```

**方法 2:使用 GROUP BY**

```sql
SELECT
    _id,
    ANY_VALUE(timestamp) as timestamp,
    ANY_VALUE(message) as message,
    ANY_VALUE(user_id) as user_id
FROM logs
WHERE timestamp >= '2025-03-20T00:00:00Z'
GROUP BY _id
```

#### 效果分析

**保护范围:**
- ✅ 单个服务器永久故障 → 数据不丢失
- ✅ 2 个服务器永久故障 → 数据不丢失(假设 3 副本)
- ✅ 区域级故障 → 如果 Ingester 部署在不同区域,数据不丢失
- ❌ 客户端崩溃(发送前) → 数据丢失

**可靠性计算:**

假设单个服务器的年故障率为 **1%**:
```
单副本:数据丢失概率 = 1%
双副本:数据丢失概率 = 1% × 1% = 0.01%
三副本:数据丢失概率 = 1% × 1% × 1% = 0.0001%
```

假设单个服务器的可用性为 **99.9%**(3 个 9):
```
单副本:可用性 = 99.9%
双副本:可用性 = 1 - (1 - 0.999)² = 99.9999% (5 个 9)
三副本:可用性 = 1 - (1 - 0.999)³ = 99.9999999% (8 个 9)
```

#### 优缺点

| 优点 | 缺点 |
|------|------|
| ✅ 防止单个/多个服务器永久故障 | ⚠️ 增加客户端复杂度 |
| ✅ 灵活的复制因子(2/3/5 副本可配置) | ⚠️ 增加网络流量(写入 N 倍数据) |
| ✅ 可跨区域部署(防止区域级故障) | ⚠️ 可能产生重复数据(需要去重) |
| ✅ 无需修改 OpenObserve 代码 | ⚠️ 客户端需要维护 Ingester 节点列表 |

#### 适用场景
- 对数据丢失零容忍的场景(如金融交易日志)
- 客户端可控(可修改日志发送逻辑)
- 网络带宽充足
- 中等规模生产环境

---

### 方案 5:使用外部 WAL 复制系统 (如 Kafka/Pulsar) 📨 (最可靠)

#### 原理
在 OpenObserve 之前部署一个消息队列(Kafka/Pulsar/NATS JetStream),利用其内置的复制机制保证数据不丢失。

#### 架构示意图

```
客户端
  ↓
Kafka Topic: openobserve-logs (复制因子 = 3)
  ├─> Partition 0 Replica-1 (Broker-1)  ← Leader
  ├─> Partition 0 Replica-2 (Broker-2)  ← Follower
  └─> Partition 0 Replica-3 (Broker-3)  ← Follower
  ↓
OpenObserve Ingester (Kafka Consumer Group)
  ├─> Ingester-1 消费 Partition 0-2
  ├─> Ingester-2 消费 Partition 3-5
  └─> Ingester-3 消费 Partition 6-9
  ↓
写入 WAL (本地)
  ↓
上传到 S3

数据在 Kafka 中有 3 个副本,即使 OpenObserve Ingester 全部故障,数据仍在 Kafka 中
```

#### Kafka 配置示例

**1. 创建 Kafka Topic:**

```bash
kafka-topics.sh --create \
  --bootstrap-server kafka:9092 \
  --topic openobserve-logs \
  --partitions 10 \
  --replication-factor 3 \
  --config min.insync.replicas=2 \
  --config retention.ms=86400000  # 保留 24 小时
```

**配置说明:**
- `partitions=10`:支持 10 个 Ingester 并行消费
- `replication-factor=3`:每条消息有 3 个副本
- `min.insync.replicas=2`:至少 2 个副本确认才算写入成功

**2. Producer 配置 (客户端):**

```python
from kafka import KafkaProducer
import json

producer = KafkaProducer(
    bootstrap_servers=['kafka:9092'],
    acks='all',  # 等待所有副本确认
    retries=3,   # 失败重试 3 次
    value_serializer=lambda v: json.dumps(v).encode('utf-8')
)

# 发送日志
log_data = {
    "timestamp": 1711094400000,
    "message": "User login successful",
    "user_id": 12345
}

future = producer.send('openobserve-logs', value=log_data)
record_metadata = future.get(timeout=10)  # 阻塞等待确认

print(f"✅ 消息已写入 Kafka: partition={record_metadata.partition}, offset={record_metadata.offset}")
```

**3. OpenObserve Ingester 作为 Kafka Consumer:**

```rust
// 伪代码:从 Kafka 消费数据并写入 OpenObserve
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::config::ClientConfig;

async fn consume_from_kafka() {
    let consumer: StreamConsumer = ClientConfig::new()
        .set("group.id", "openobserve-ingester")
        .set("bootstrap.servers", "kafka:9092")
        .set("enable.auto.commit", "false")  // 手动提交 offset
        .set("auto.offset.reset", "earliest")
        .create()
        .unwrap();

    consumer.subscribe(&["openobserve-logs"]).unwrap();

    loop {
        match consumer.recv().await {
            Ok(message) => {
                let payload = message.payload().unwrap();

                // 1. 解析 JSON
                let log_data: serde_json::Value = serde_json::from_slice(payload).unwrap();

                // 2. 写入 OpenObserve (WAL + MemTable)
                let result = write_to_openobserve(log_data).await;

                // 3. 只有成功写入后才提交 offset
                if result.is_ok() {
                    consumer.commit_message(&message, CommitMode::Async).unwrap();
                } else {
                    // 写入失败,不提交 offset,下次重新消费
                    log::error!("Failed to write to OpenObserve, will retry");
                }
            }
            Err(e) => eprintln!("Kafka error: {}", e),
        }
    }
}
```

**4. 部署示例 (Docker Compose):**

```yaml
version: '3.8'

services:
  zookeeper:
    image: confluentinc/cp-zookeeper:latest
    environment:
      ZOOKEEPER_CLIENT_PORT: 2181
      ZOOKEEPER_TICK_TIME: 2000

  kafka:
    image: confluentinc/cp-kafka:latest
    depends_on:
      - zookeeper
    environment:
      KAFKA_BROKER_ID: 1
      KAFKA_ZOOKEEPER_CONNECT: zookeeper:2181
      KAFKA_ADVERTISED_LISTENERS: PLAINTEXT://kafka:9092
      KAFKA_OFFSETS_TOPIC_REPLICATION_FACTOR: 3
      KAFKA_MIN_INSYNC_REPLICAS: 2

  openobserve-ingester-1:
    image: public.ecr.aws/zinclabs/openobserve:latest
    environment:
      ZO_NODE_ROLE: ingester
      KAFKA_BOOTSTRAP_SERVERS: kafka:9092
      KAFKA_TOPIC: openobserve-logs
      KAFKA_GROUP_ID: openobserve-ingester
    depends_on:
      - kafka

  openobserve-ingester-2:
    image: public.ecr.aws/zinclabs/openobserve:latest
    environment:
      ZO_NODE_ROLE: ingester
      KAFKA_BOOTSTRAP_SERVERS: kafka:9092
      KAFKA_TOPIC: openobserve-logs
      KAFKA_GROUP_ID: openobserve-ingester
    depends_on:
      - kafka
```

#### 效果分析

**保护范围:**
- ✅ 单个 Kafka Broker 故障 → 数据不丢失(自动切换到其他副本)
- ✅ 单个 OpenObserve Ingester 故障 → 数据不丢失(其他消费者接管)
- ✅ 所有 OpenObserve Ingester 故障 → 数据不丢失(Kafka 中有完整数据)
- ✅ 支持数据回溯(重置 offset 重新消费)
- ❌ 整个 Kafka 集群故障 → 数据丢失

**可靠性计算:**

假设单个 Kafka Broker 可用性为 **99.9%**:
```
单副本:可用性 = 99.9%
三副本:可用性 = 1 - (1 - 0.999)³ = 99.9999999% (8 个 9)
```

#### 优缺点

| 优点 | 缺点 |
|------|------|
| ✅ 数据在 Kafka 中有 N 个副本 | ⚠️ 增加架构复杂度(需要运维 Kafka 集群) |
| ✅ 即使所有 Ingester 崩溃,数据仍在 Kafka 中 | ⚠️ 增加延迟(数据需要先写入 Kafka) |
| ✅ 支持数据回溯(重新消费) | ⚠️ 增加成本(Kafka 集群资源) |
| ✅ 支持横向扩展(增加 partition 和消费者) | ⚠️ 需要维护 offset 管理 |
| ✅ 成熟的生态系统(监控、管理工具) | |

#### 适用场景
- 大规模生产环境
- 对数据丢失零容忍
- 已有 Kafka/Pulsar 基础设施
- 需要支持数据回溯(replay)
- 金融、电商等关键业务

#### 成本估算 (AWS MSK - Managed Kafka)

```
AWS MSK 集群配置:
- 3 个 Broker (kafka.m5.large)
- 500GB 存储 × 3
- 复制因子 = 3

成本:
- Broker 成本:$0.21/小时 × 3 = $0.63/小时 = $460/月
- 存储成本:$0.10/GB/月 × 500GB × 3 = $150/月
- 总成本:$460 + $150 = $610/月
```

---

## 3. 方案对比总结

| 方案 | 可靠性 | 复杂度 | 成本 | 性能影响 | 数据丢失概率 | 适用场景 |
|------|--------|--------|------|---------|-------------|---------|
| **1. 缩短上传间隔** | ⭐ | ⭐ | ⭐ | ⚠️ 中 | ~0.1% | 临时缓解 |
| **2. RAID/LVM** | ⭐⭐ | ⭐⭐ | ⭐⭐ | ✅ 低 | ~0.01% | 物理服务器 |
| **3. NAS/EBS** | ⭐⭐⭐ | ⭐⭐ | ⭐⭐⭐ | ⚠️ 中 | ~0.001% | 云环境 |
| **4. 客户端双写** | ⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐ | ⚠️ 高 | ~0.0001% | 客户端可控 |
| **5. Kafka/Pulsar** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⚠️ 中 | ~0.00001% | 大规模生产 |

---

## 4. 推荐方案

根据不同场景的推荐:

### 小规模/测试环境
- **方案 1**(缩短上传间隔)+ **方案 2**(RAID)
- 成本低,配置简单
- 可接受小概率数据丢失

### 云环境(AWS/Azure/GCP)
- **方案 3**(EBS/Azure Disk)
- 利用云平台内置的高可用性
- 成本适中,无需修改应用

### 中等规模生产环境
- **方案 4**(客户端双写到 2-3 个 Ingester)
- 灵活的复制因子
- 可跨区域部署

### 大规模/金融级可靠性
- **方案 5**(Kafka/Pulsar)
- 最高可靠性
- 支持数据回溯和横向扩展

---

## 5. 监控和告警

无论采用哪种方案,都应该配置监控和告警:

### 5.1 监控 WAL 目录大小

```bash
# Prometheus 告警规则
- alert: WALDirectoryTooLarge
  expr: node_filesystem_size_bytes{mountpoint="/data/openobserve/wal"} > 10737418240
  for: 5m
  annotations:
    summary: "WAL directory size > 10GB, potential upload issue"
    description: "WAL directory on {{ $labels.instance }} has grown to {{ humanize $value }}GB"

- alert: WALGrowingTooFast
  expr: rate(node_filesystem_size_bytes{mountpoint="/data/openobserve/wal"}[5m]) > 104857600
  for: 10m
  annotations:
    summary: "WAL directory growing faster than 100MB/5min"
```

### 5.2 监控文件上传成功率

```bash
# OpenObserve 内部指标
- alert: FileUploadFailureRate
  expr: rate(openobserve_file_upload_failures_total[5m]) > 0.01
  for: 5m
  annotations:
    summary: "File upload failure rate > 1%"
```

### 5.3 监控复制因子 (方案 4/5)

```bash
# Kafka 复制因子监控
- alert: KafkaUnderReplicatedPartitions
  expr: kafka_topic_partition_under_replicated_partition > 0
  for: 5m
  annotations:
    summary: "Kafka topic has under-replicated partitions"
```

---

## 6. 总结

### 核心结论

1. **是的,服务器永久故障会导致 WAL 中的数据丢失。**
2. **OpenObserve 默认没有 WAL 复制机制,这是性能与可靠性的权衡。**
3. **数据丢失风险窗口:5-15 分钟(从写入 WAL 到上传对象存储)。**

### 最佳实践建议

1. **短期:**缩短上传间隔 + RAID/LVM
   ```bash
   ZO_FILE_PUSH_INTERVAL=30
   ZO_MAX_FILE_RETENTION_TIME=120
   ```

2. **中期(云环境):**使用 AWS EBS/Azure Disk
   - 自动跨可用区复制
   - 成本适中

3. **长期(生产环境):**
   - 部署 3 个 Ingester + 客户端双写
   - 或使用 Kafka(复制因子 = 3)

4. **监控:**
   - WAL 目录大小
   - 文件上传成功率
   - 复制因子健康度

### 选择建议

| 场景 | 推荐方案 | 预期可靠性 |
|------|---------|-----------|
| 开发/测试 | 方案 1 + 方案 2 | 99.9% |
| 云环境 | 方案 3 (EBS/Azure Disk) | 99.999% |
| 生产环境 | 方案 4 (客户端双写) | 99.9999% |
| 关键业务 | 方案 5 (Kafka/Pulsar) | 99.99999% |

---

**文档版本:** 1.0
**最后更新:** 2025-03-20
**适用版本:** OpenObserve v0.10.x+
