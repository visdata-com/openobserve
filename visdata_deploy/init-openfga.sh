#!/bin/bash
# OpenFGA Initialization Script for VisData
# Usage: ./init-openfga.sh -e root@example.com -o default

set -e

# Default values
OPENFGA_URL="http://localhost:8080"
STORE_NAME="openobserve"
ORG_ID="default"
ROOT_USER_EMAIL=""

# Parse arguments
while getopts "e:o:u:s:" opt; do
  case $opt in
    e) ROOT_USER_EMAIL="$OPTARG" ;;
    o) ORG_ID="$OPTARG" ;;
    u) OPENFGA_URL="$OPTARG" ;;
    s) STORE_NAME="$OPTARG" ;;
    *) echo "Usage: $0 -e <root_email> [-o <org_id>] [-u <openfga_url>] [-s <store_name>]"; exit 1 ;;
  esac
done

if [ -z "$ROOT_USER_EMAIL" ]; then
  echo "Error: Root user email is required (-e)"
  echo "Usage: $0 -e root@example.com [-o default] [-u http://localhost:8080] [-s openobserve]"
  exit 1
fi

echo "========================================"
echo "OpenFGA Initialization for VisData"
echo "========================================"
echo "OpenFGA URL: $OPENFGA_URL"
echo "Store Name: $STORE_NAME"
echo "Organization: $ORG_ID"
echo "Root User: $ROOT_USER_EMAIL"
echo "========================================"

# Step 1: Check if store exists or create new one
echo ""
echo "[Step 1] Checking/Creating Store..."

STORES_RESPONSE=$(curl -s "$OPENFGA_URL/stores")
STORE_ID=$(echo "$STORES_RESPONSE" | jq -r ".stores[] | select(.name==\"$STORE_NAME\") | .id")

if [ -n "$STORE_ID" ] && [ "$STORE_ID" != "null" ]; then
  echo "Store '$STORE_NAME' already exists with ID: $STORE_ID"
else
  CREATE_RESPONSE=$(curl -s -X POST "$OPENFGA_URL/stores" \
    -H "Content-Type: application/json" \
    -d "{\"name\": \"$STORE_NAME\"}")
  STORE_ID=$(echo "$CREATE_RESPONSE" | jq -r '.id')
  echo "Created new store '$STORE_NAME' with ID: $STORE_ID"
fi

# Step 2: Write Authorization Model
echo ""
echo "[Step 2] Writing Authorization Model..."

AUTH_MODEL='{
  "schema_version": "1.1",
  "type_definitions": [
    {
      "type": "user"
    },
    {
      "type": "group",
      "relations": {
        "member": {
          "this": {}
        }
      },
      "metadata": {
        "relations": {
          "member": {
            "directly_related_user_types": [
              { "type": "user" }
            ]
          }
        }
      }
    },
    {
      "type": "role",
      "relations": {
        "assignee": {
          "this": {}
        }
      },
      "metadata": {
        "relations": {
          "assignee": {
            "directly_related_user_types": [
              { "type": "user" },
              { "type": "group", "relation": "member" }
            ]
          }
        }
      }
    },
    {
      "type": "organization",
      "relations": {
        "owner": {
          "this": {}
        },
        "admin": {
          "union": {
            "child": [
              { "this": {} },
              { "computedUserset": { "relation": "owner" } }
            ]
          }
        },
        "member": {
          "union": {
            "child": [
              { "this": {} },
              { "computedUserset": { "relation": "admin" } }
            ]
          }
        }
      },
      "metadata": {
        "relations": {
          "owner": {
            "directly_related_user_types": [
              { "type": "user" },
              { "type": "role", "relation": "assignee" }
            ]
          },
          "admin": {
            "directly_related_user_types": [
              { "type": "user" },
              { "type": "role", "relation": "assignee" }
            ]
          },
          "member": {
            "directly_related_user_types": [
              { "type": "user" },
              { "type": "role", "relation": "assignee" }
            ]
          }
        }
      }
    },
    {
      "type": "resource",
      "relations": {
        "parent": {
          "this": {}
        },
        "owner": {
          "union": {
            "child": [
              { "this": {} },
              { "tupleToUserset": { "tupleset": { "relation": "parent" }, "computedUserset": { "relation": "owner" } } }
            ]
          }
        },
        "admin": {
          "union": {
            "child": [
              { "this": {} },
              { "computedUserset": { "relation": "owner" } },
              { "tupleToUserset": { "tupleset": { "relation": "parent" }, "computedUserset": { "relation": "admin" } } }
            ]
          }
        },
        "can_create": {
          "union": {
            "child": [
              { "this": {} },
              { "computedUserset": { "relation": "admin" } }
            ]
          }
        },
        "can_read": {
          "union": {
            "child": [
              { "this": {} },
              { "computedUserset": { "relation": "can_create" } },
              { "tupleToUserset": { "tupleset": { "relation": "parent" }, "computedUserset": { "relation": "member" } } }
            ]
          }
        },
        "can_list": {
          "union": {
            "child": [
              { "this": {} },
              { "computedUserset": { "relation": "can_read" } }
            ]
          }
        },
        "can_update": {
          "union": {
            "child": [
              { "this": {} },
              { "computedUserset": { "relation": "admin" } }
            ]
          }
        },
        "can_delete": {
          "union": {
            "child": [
              { "this": {} },
              { "computedUserset": { "relation": "owner" } }
            ]
          }
        }
      },
      "metadata": {
        "relations": {
          "parent": {
            "directly_related_user_types": [
              { "type": "organization" }
            ]
          },
          "owner": {
            "directly_related_user_types": [
              { "type": "user" },
              { "type": "role", "relation": "assignee" }
            ]
          },
          "admin": {
            "directly_related_user_types": [
              { "type": "user" },
              { "type": "role", "relation": "assignee" }
            ]
          },
          "can_create": {
            "directly_related_user_types": [
              { "type": "user" },
              { "type": "role", "relation": "assignee" }
            ]
          },
          "can_read": {
            "directly_related_user_types": [
              { "type": "user" },
              { "type": "role", "relation": "assignee" }
            ]
          },
          "can_list": {
            "directly_related_user_types": [
              { "type": "user" },
              { "type": "role", "relation": "assignee" }
            ]
          },
          "can_update": {
            "directly_related_user_types": [
              { "type": "user" },
              { "type": "role", "relation": "assignee" }
            ]
          },
          "can_delete": {
            "directly_related_user_types": [
              { "type": "user" },
              { "type": "role", "relation": "assignee" }
            ]
          }
        }
      }
    }
  ]
}'

MODEL_RESPONSE=$(curl -s -X POST "$OPENFGA_URL/stores/$STORE_ID/authorization-models" \
  -H "Content-Type: application/json" \
  -d "$AUTH_MODEL")

MODEL_ID=$(echo "$MODEL_RESPONSE" | jq -r '.authorization_model_id')
if [ -n "$MODEL_ID" ] && [ "$MODEL_ID" != "null" ]; then
  echo "Authorization model created with ID: $MODEL_ID"
else
  echo "Warning: Could not create authorization model (may already exist)"
  echo "$MODEL_RESPONSE"
fi

# Step 3: Initialize System Role Permissions (REQUIRED)
# This is the critical part - maps system roles to OpenFGA relations
# Without this, non-root users (Admin/Editor/Viewer) won't have permissions
echo ""
echo "[Step 3] Initializing System Role Mappings (Required for non-root users)..."

SYSTEM_ROLE_TUPLES="{
  \"writes\": {
    \"tuple_keys\": [
      {
        \"user\": \"role:${ORG_ID}_admin#assignee\",
        \"relation\": \"admin\",
        \"object\": \"organization:$ORG_ID\"
      },
      {
        \"user\": \"role:${ORG_ID}_editor#assignee\",
        \"relation\": \"member\",
        \"object\": \"organization:$ORG_ID\"
      },
      {
        \"user\": \"role:${ORG_ID}_viewer#assignee\",
        \"relation\": \"member\",
        \"object\": \"organization:$ORG_ID\"
      }
    ]
  }
}"

WRITE_RESPONSE=$(curl -s -X POST "$OPENFGA_URL/stores/$STORE_ID/write" \
  -H "Content-Type: application/json" \
  -d "$SYSTEM_ROLE_TUPLES")

if echo "$WRITE_RESPONSE" | jq -e '.code' > /dev/null 2>&1; then
  echo "Warning: Could not write system role tuples (may already exist)"
  echo "$WRITE_RESPONSE"
else
  echo "  [OK] System role 'admin' -> organization admin (can create/update/delete)"
  echo "  [OK] System role 'editor' -> organization member (can read/list)"
  echo "  [OK] System role 'viewer' -> organization member (can read/list)"
fi

# Step 4: (Optional) Initialize Root User in OpenFGA
# NOTE: Root users (UserRole::Root) bypass OpenFGA checks in code,
# so this is only for consistency/auditing purposes
echo ""
if [ "$ROOT_USER_EMAIL" != "skip" ]; then
  echo "[Step 4] (Optional) Setting up root user in OpenFGA..."
  echo "  Note: Root users bypass OpenFGA checks in code, this is for consistency only"

  ROOT_USER_TUPLES="{
    \"writes\": {
      \"tuple_keys\": [
        {
          \"user\": \"user:$ROOT_USER_EMAIL\",
          \"relation\": \"owner\",
          \"object\": \"organization:$ORG_ID\"
        },
        {
          \"user\": \"user:$ROOT_USER_EMAIL\",
          \"relation\": \"assignee\",
          \"object\": \"role:${ORG_ID}_admin\"
        }
      ]
    }
  }"

  ROLE_RESPONSE=$(curl -s -X POST "$OPENFGA_URL/stores/$STORE_ID/write" \
    -H "Content-Type: application/json" \
    -d "$ROOT_USER_TUPLES")

  if echo "$ROLE_RESPONSE" | jq -e '.code' > /dev/null 2>&1; then
    echo "Warning: Could not write root user tuples (may already exist)"
  else
    echo "  [OK] Root user '$ROOT_USER_EMAIL' -> organization owner"
    echo "  [OK] Root user assigned to 'admin' role"
  fi
else
  echo "[Step 4] Skipping root user setup (not needed for Root users)"
fi

# Step 5: Verify Setup
echo ""
echo "[Step 5] Verifying Setup..."

# Test 1: Root user has owner access
CHECK_OWNER="{
  \"tuple_key\": {
    \"user\": \"user:$ROOT_USER_EMAIL\",
    \"relation\": \"owner\",
    \"object\": \"organization:$ORG_ID\"
  }
}"

OWNER_RESPONSE=$(curl -s -X POST "$OPENFGA_URL/stores/$STORE_ID/check" \
  -H "Content-Type: application/json" \
  -d "$CHECK_OWNER")

OWNER_ALLOWED=$(echo "$OWNER_RESPONSE" | jq -r '.allowed')
if [ "$OWNER_ALLOWED" = "true" ]; then
  echo "  [PASS] Root user has 'owner' access to organization"
else
  echo "  [FAIL] Root user does NOT have 'owner' access"
fi

# Test 2: Root user has admin access (inherited from owner)
CHECK_ADMIN="{
  \"tuple_key\": {
    \"user\": \"user:$ROOT_USER_EMAIL\",
    \"relation\": \"admin\",
    \"object\": \"organization:$ORG_ID\"
  }
}"

ADMIN_RESPONSE=$(curl -s -X POST "$OPENFGA_URL/stores/$STORE_ID/check" \
  -H "Content-Type: application/json" \
  -d "$CHECK_ADMIN")

ADMIN_ALLOWED=$(echo "$ADMIN_RESPONSE" | jq -r '.allowed')
if [ "$ADMIN_ALLOWED" = "true" ]; then
  echo "  [PASS] Root user has 'admin' access to organization"
else
  echo "  [FAIL] Root user does NOT have 'admin' access"
fi

# Test 3: Root user has member access (inherited from admin)
CHECK_MEMBER="{
  \"tuple_key\": {
    \"user\": \"user:$ROOT_USER_EMAIL\",
    \"relation\": \"member\",
    \"object\": \"organization:$ORG_ID\"
  }
}"

MEMBER_RESPONSE=$(curl -s -X POST "$OPENFGA_URL/stores/$STORE_ID/check" \
  -H "Content-Type: application/json" \
  -d "$CHECK_MEMBER")

MEMBER_ALLOWED=$(echo "$MEMBER_RESPONSE" | jq -r '.allowed')
if [ "$MEMBER_ALLOWED" = "true" ]; then
  echo "  [PASS] Root user has 'member' access to organization"
else
  echo "  [FAIL] Root user does NOT have 'member' access"
fi

echo ""
echo "========================================"
echo "Initialization Complete!"
echo "========================================"
echo "Store ID: $STORE_ID"
echo ""
echo "Permission Structure:"
echo "  organization:$ORG_ID"
echo "    |-- admin: role:${ORG_ID}_admin#assignee"
echo "    |-- member: role:${ORG_ID}_editor#assignee"
echo "    |-- member: role:${ORG_ID}_viewer#assignee"
if [ "$ROOT_USER_EMAIL" != "skip" ]; then
  echo "    |-- owner: user:$ROOT_USER_EMAIL (optional)"
fi
echo ""
echo "System Roles Mapping (CRITICAL):"
echo "  UserRole.Root   -> bypasses OpenFGA (checked in code)"
echo "  UserRole.Admin  -> role:${ORG_ID}_admin  -> org admin  -> full access"
echo "  UserRole.Editor -> role:${ORG_ID}_editor -> org member -> read + explicit writes"
echo "  UserRole.Viewer -> role:${ORG_ID}_viewer -> org member -> read only"
echo ""
echo "Environment Variables:"
echo "  VISDATA_OPENFGA_URL=$OPENFGA_URL"
echo "  VISDATA_OPENFGA_STORE=$STORE_NAME"
echo ""
echo "Usage:"
echo "  # With root user (optional):"
echo "  ./init-openfga.sh -e root@example.com"
echo ""
echo "  # Without root user (system roles only):"
echo "  ./init-openfga.sh -e skip"
echo "========================================"
