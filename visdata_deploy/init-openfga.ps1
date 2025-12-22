# OpenFGA Initialization Script for VisData
# Usage: .\init-openfga.ps1 -RootUserEmail "root@example.com" -OrgId "default"

param(
    [Parameter(Mandatory=$true)]
    [string]$RootUserEmail,

    [Parameter(Mandatory=$false)]
    [string]$OrgId = "default",

    [Parameter(Mandatory=$false)]
    [string]$OpenFGAUrl = "http://localhost:8080",

    [Parameter(Mandatory=$false)]
    [string]$StoreName = "openobserve"
)

$ErrorActionPreference = "Stop"

Write-Host "========================================"
Write-Host "OpenFGA Initialization for VisData"
Write-Host "========================================"
Write-Host "OpenFGA URL: $OpenFGAUrl"
Write-Host "Store Name: $StoreName"
Write-Host "Organization: $OrgId"
Write-Host "Root User: $RootUserEmail"
Write-Host "========================================"

# Step 1: Check if store exists or create new one
Write-Host "`n[Step 1] Checking/Creating Store..."

$storesResponse = Invoke-RestMethod -Uri "$OpenFGAUrl/stores" -Method Get
$existingStore = $storesResponse.stores | Where-Object { $_.name -eq $StoreName }

if ($existingStore) {
    $storeId = $existingStore.id
    Write-Host "Store '$StoreName' already exists with ID: $storeId"
} else {
    $createStoreBody = @{ name = $StoreName } | ConvertTo-Json
    $newStore = Invoke-RestMethod -Uri "$OpenFGAUrl/stores" -Method Post -ContentType "application/json" -Body $createStoreBody
    $storeId = $newStore.id
    Write-Host "Created new store '$StoreName' with ID: $storeId"
}

# Step 2: Write Authorization Model
Write-Host "`n[Step 2] Writing Authorization Model..."

$authModel = @'
{
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
}
'@

try {
    $modelResponse = Invoke-RestMethod -Uri "$OpenFGAUrl/stores/$storeId/authorization-models" -Method Post -ContentType "application/json" -Body $authModel
    $modelId = $modelResponse.authorization_model_id
    Write-Host "Authorization model created with ID: $modelId"
} catch {
    Write-Host "Warning: Could not create authorization model (may already exist)"
    Write-Host $_.Exception.Message
}

# Step 3: Initialize System Role Permissions (REQUIRED)
# This is the critical part - maps system roles to OpenFGA relations
# Without this, non-root users (Admin/Editor/Viewer) won't have permissions
Write-Host "`n[Step 3] Initializing System Role Mappings (Required for non-root users)..."

$systemRoleTuples = @{
    writes = @{
        tuple_keys = @(
            # ============================================
            # System Role: admin
            # Users with UserRole::Admin -> organization admin
            # admin relation grants: can_create, can_update on resources
            # ============================================
            @{
                user = "role:${OrgId}_admin#assignee"
                relation = "admin"
                object = "organization:$OrgId"
            },

            # ============================================
            # System Role: editor
            # Users with UserRole::Editor -> organization member
            # member relation grants: can_read, can_list on resources
            # Editor needs explicit grants for write operations
            # ============================================
            @{
                user = "role:${OrgId}_editor#assignee"
                relation = "member"
                object = "organization:$OrgId"
            },

            # ============================================
            # System Role: viewer
            # Users with UserRole::Viewer -> organization member
            # member relation grants: can_read, can_list on resources
            # ============================================
            @{
                user = "role:${OrgId}_viewer#assignee"
                relation = "member"
                object = "organization:$OrgId"
            }
        )
    }
} | ConvertTo-Json -Depth 10

try {
    Invoke-RestMethod -Uri "$OpenFGAUrl/stores/$storeId/write" -Method Post -ContentType "application/json" -Body $systemRoleTuples
    Write-Host "  [OK] System role 'admin' -> organization admin (can create/update/delete)"
    Write-Host "  [OK] System role 'editor' -> organization member (can read/list)"
    Write-Host "  [OK] System role 'viewer' -> organization member (can read/list)"
} catch {
    Write-Host "Warning: Could not write system role tuples (may already exist)"
    Write-Host $_.Exception.Message
}

# Step 4: (Optional) Initialize Root User in OpenFGA
# NOTE: Root users (UserRole::Root) bypass OpenFGA checks in code,
# so this is only for consistency/auditing purposes
if ($RootUserEmail -ne "skip") {
    Write-Host "`n[Step 4] (Optional) Setting up root user in OpenFGA..."
    Write-Host "  Note: Root users bypass OpenFGA checks in code, this is for consistency only"

    $rootUserTuples = @{
        writes = @{
            tuple_keys = @(
                @{
                    user = "user:$RootUserEmail"
                    relation = "owner"
                    object = "organization:$OrgId"
                },
                @{
                    user = "user:$RootUserEmail"
                    relation = "assignee"
                    object = "role:${OrgId}_admin"
                }
            )
        }
    } | ConvertTo-Json -Depth 10

    try {
        Invoke-RestMethod -Uri "$OpenFGAUrl/stores/$storeId/write" -Method Post -ContentType "application/json" -Body $rootUserTuples
        Write-Host "  [OK] Root user '$RootUserEmail' -> organization owner"
        Write-Host "  [OK] Root user assigned to 'admin' role"
    } catch {
        Write-Host "Warning: Could not write root user tuples (may already exist)"
        Write-Host $_.Exception.Message
    }
} else {
    Write-Host "`n[Step 4] Skipping root user setup (not needed for Root users)"
}

# Step 5: Verify Setup
Write-Host "`n[Step 5] Verifying Setup..."

# Test 1: Root user has owner access
$checkOwner = @{
    tuple_key = @{
        user = "user:$RootUserEmail"
        relation = "owner"
        object = "organization:$OrgId"
    }
} | ConvertTo-Json -Depth 5

try {
    $response = Invoke-RestMethod -Uri "$OpenFGAUrl/stores/$storeId/check" -Method Post -ContentType "application/json" -Body $checkOwner
    if ($response.allowed) {
        Write-Host "  [PASS] Root user has 'owner' access to organization"
    } else {
        Write-Host "  [FAIL] Root user does NOT have 'owner' access"
    }
} catch {
    Write-Host "  [ERROR] Could not verify owner access"
}

# Test 2: Root user has admin access (inherited from owner)
$checkAdmin = @{
    tuple_key = @{
        user = "user:$RootUserEmail"
        relation = "admin"
        object = "organization:$OrgId"
    }
} | ConvertTo-Json -Depth 5

try {
    $response = Invoke-RestMethod -Uri "$OpenFGAUrl/stores/$storeId/check" -Method Post -ContentType "application/json" -Body $checkAdmin
    if ($response.allowed) {
        Write-Host "  [PASS] Root user has 'admin' access to organization"
    } else {
        Write-Host "  [FAIL] Root user does NOT have 'admin' access"
    }
} catch {
    Write-Host "  [ERROR] Could not verify admin access"
}

# Test 3: Root user has member access (inherited from admin)
$checkMember = @{
    tuple_key = @{
        user = "user:$RootUserEmail"
        relation = "member"
        object = "organization:$OrgId"
    }
} | ConvertTo-Json -Depth 5

try {
    $response = Invoke-RestMethod -Uri "$OpenFGAUrl/stores/$storeId/check" -Method Post -ContentType "application/json" -Body $checkMember
    if ($response.allowed) {
        Write-Host "  [PASS] Root user has 'member' access to organization"
    } else {
        Write-Host "  [FAIL] Root user does NOT have 'member' access"
    }
} catch {
    Write-Host "  [ERROR] Could not verify member access"
}

Write-Host "`n========================================"
Write-Host "Initialization Complete!"
Write-Host "========================================"
Write-Host "Store ID: $storeId"
Write-Host ""
Write-Host "Permission Structure:"
Write-Host "  organization:$OrgId"
Write-Host "    |-- admin: role:${OrgId}_admin#assignee"
Write-Host "    |-- member: role:${OrgId}_editor#assignee"
Write-Host "    |-- member: role:${OrgId}_viewer#assignee"
if ($RootUserEmail -ne "skip") {
    Write-Host "    |-- owner: user:$RootUserEmail (optional)"
}
Write-Host ""
Write-Host "System Roles Mapping (CRITICAL):"
Write-Host "  UserRole.Root   -> bypasses OpenFGA (checked in code)"
Write-Host "  UserRole.Admin  -> role:${OrgId}_admin  -> org admin  -> full access"
Write-Host "  UserRole.Editor -> role:${OrgId}_editor -> org member -> read + explicit writes"
Write-Host "  UserRole.Viewer -> role:${OrgId}_viewer -> org member -> read only"
Write-Host ""
Write-Host "Environment Variables:"
Write-Host "  VISDATA_OPENFGA_URL=$OpenFGAUrl"
Write-Host "  VISDATA_OPENFGA_STORE=$StoreName"
Write-Host ""
Write-Host "Usage:"
Write-Host "  # With root user (optional):"
Write-Host "  .\init-openfga.ps1 -RootUserEmail 'root@example.com'"
Write-Host ""
Write-Host "  # Without root user (system roles only):"
Write-Host "  .\init-openfga.ps1 -RootUserEmail 'skip'"
Write-Host "========================================"
