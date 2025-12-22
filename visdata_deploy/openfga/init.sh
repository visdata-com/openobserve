#!/bin/bash
# OpenFGA 初始化脚本
# 用法: ./init.sh [OPENFGA_URL]

set -e

OPENFGA_URL="${1:-http://localhost:8080}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
STORE_NAME="openobserve"

echo "=========================================="
echo "OpenFGA 初始化脚本"
echo "=========================================="
echo "OpenFGA URL: $OPENFGA_URL"
echo "Store Name: $STORE_NAME"
echo ""

# 等待 OpenFGA 服务就绪
echo "等待 OpenFGA 服务就绪..."
for i in {1..30}; do
    if curl -s "${OPENFGA_URL}/healthz" > /dev/null 2>&1; then
        echo "OpenFGA 服务已就绪"
        break
    fi
    if [ $i -eq 30 ]; then
        echo "错误: OpenFGA 服务未就绪"
        exit 1
    fi
    echo "等待中... ($i/30)"
    sleep 2
done

echo ""
echo "步骤 1: 清理现有 Store"
echo "------------------------------------------"

# 查找并删除名为 openobserve 的 Store
echo "查找现有的 '$STORE_NAME' Store..."
STORES_RESPONSE=$(curl -s -X GET "${OPENFGA_URL}/stores")
EXISTING_STORE_IDS=$(echo "$STORES_RESPONSE" | grep -o '"id":"[^"]*"' | cut -d'"' -f4)

if [ -n "$EXISTING_STORE_IDS" ]; then
    # 遍历所有 store，查找名为 openobserve 的
    for STORE_ID in $EXISTING_STORE_IDS; do
        STORE_INFO=$(curl -s -X GET "${OPENFGA_URL}/stores/${STORE_ID}")
        STORE_NAME_CHECK=$(echo "$STORE_INFO" | grep -o '"name":"[^"]*"' | cut -d'"' -f4)

        if [ "$STORE_NAME_CHECK" = "$STORE_NAME" ]; then
            echo "发现现有 Store: $STORE_ID (name: $STORE_NAME_CHECK)"
            echo "删除 Store: $STORE_ID..."
            DELETE_RESPONSE=$(curl -s -X DELETE "${OPENFGA_URL}/stores/${STORE_ID}")
            echo "Store 已删除"
        fi
    done
else
    echo "未发现现有的 '$STORE_NAME' Store"
fi

echo ""
echo "步骤 2: 创建新 Store"
echo "------------------------------------------"

STORE_RESPONSE=$(curl -s -X POST "${OPENFGA_URL}/stores" \
    -H "Content-Type: application/json" \
    -d '{"name": "openobserve"}')

STORE_ID=$(echo "$STORE_RESPONSE" | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)

if [ -z "$STORE_ID" ]; then
    echo "错误: 无法创建 Store"
    echo "响应: $STORE_RESPONSE"
    exit 1
fi

echo "Store 创建成功: $STORE_ID"

echo ""
echo "步骤 3: 写入授权模型"
echo "------------------------------------------"

# 读取模型文件并转换为 JSON
MODEL_FILE="${SCRIPT_DIR}/model.fga"
if [ ! -f "$MODEL_FILE" ]; then
    echo "错误: 模型文件不存在: $MODEL_FILE"
    exit 1
fi

# 使用 fga model transform 转换模型（如果安装了 fga CLI）
if command -v fga &> /dev/null; then
    echo "使用 fga CLI 转换模型..."
    MODEL_JSON=$(fga model transform --file "$MODEL_FILE")

    MODEL_RESPONSE=$(curl -s -X POST "${OPENFGA_URL}/stores/${STORE_ID}/authorization-models" \
        -H "Content-Type: application/json" \
        -d "$MODEL_JSON")
else
    echo "警告: fga CLI 未安装，请手动转换模型"
    echo "安装: go install github.com/openfga/cli/cmd/fga@latest"
    echo ""
    echo "或使用以下命令转换模型:"
    echo "  fga model transform --file $MODEL_FILE > model.json"
    echo "  然后运行: curl -X POST ${OPENFGA_URL}/stores/${STORE_ID}/authorization-models -d @model.json"
    exit 1
fi

MODEL_ID=$(echo "$MODEL_RESPONSE" | grep -o '"authorization_model_id":"[^"]*"' | head -1 | cut -d'"' -f4)

if [ -z "$MODEL_ID" ]; then
    echo "错误: 无法创建授权模型"
    echo "响应: $MODEL_RESPONSE"
    exit 1
fi

echo "授权模型创建成功: $MODEL_ID"

echo ""
echo "步骤 4: 写入初始化 Tuples"
echo "------------------------------------------"

TUPLES_FILE="${SCRIPT_DIR}/tuples.yaml"
if [ ! -f "$TUPLES_FILE" ]; then
    echo "错误: Tuples 文件不存在: $TUPLES_FILE"
    exit 1
fi

# 使用 fga tuple write 写入 tuples
if command -v fga &> /dev/null; then
    echo "写入 tuples..."
    fga tuple write --store-id "$STORE_ID" --file "$TUPLES_FILE"
    echo "Tuples 写入成功"
else
    echo "警告: fga CLI 未安装，无法写入 tuples"
    exit 1
fi

echo ""
echo "=========================================="
echo "初始化完成!"
echo "=========================================="
echo ""
echo "Store ID: $STORE_ID"
echo "Model ID: $MODEL_ID"
echo ""
echo "环境变量配置:"
echo "  export OPENFGA_STORE_ID=$STORE_ID"
echo "  export OPENFGA_MODEL_ID=$MODEL_ID"
echo ""
echo "验证命令:"
echo "  # 检查 root 用户对 default 组织的管理权限"
echo "  fga query check --store-id $STORE_ID user:root@openobserve.ai GET org:default"
echo ""
