// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! OpenFGA authorization model schema

/// Get the OpenFGA authorization model in JSON format
///
/// This model defines:
/// - user: Individual users
/// - group: User groups with member relation
/// - role: Roles that can be assigned to users or groups
/// - organization: Multi-tenant organization
/// - Various resource types (stream, dashboard, etc.) with parent relation to organization
pub fn get_authorization_model_json() -> &'static str {
    r#"{
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
}"#
}

/// Generate organization-scoped type name
pub fn org_type(org_id: &str) -> String {
    format!("organization:{}", org_id)
}

/// Generate user type name
pub fn user_type(user_email: &str) -> String {
    format!("user:{}", user_email)
}

/// Generate role type name
pub fn role_type(org_id: &str, role_name: &str) -> String {
    format!("role:{}_{}", org_id, role_name)
}

/// Generate group type name
pub fn group_type(org_id: &str, group_name: &str) -> String {
    format!("group:{}_{}", org_id, group_name)
}

/// Generate resource object name
/// Format: "{resource_type}:{org_id}_{entity_id}"
/// e.g., "dfolder:default_my_folder" or "logs:default_my_stream"
pub fn resource_object(org_id: &str, resource_type: &str, entity_id: &str) -> String {
    format!("{}:{}_{}", resource_type, org_id, entity_id)
}

/// Generate "all org" resource object name (for type-level permissions)
/// Format: "{resource_type}:{org_id}_all"
/// e.g., "dfolder:default_all" means all dfolder resources in org "default"
pub fn resource_object_all(org_id: &str, resource_type: &str) -> String {
    format!("{}:{}_all", resource_type, org_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_authorization_model_is_valid_json() {
        let model = get_authorization_model_json();
        let parsed: serde_json::Value = serde_json::from_str(model).unwrap();
        assert!(parsed.get("schema_version").is_some());
        assert!(parsed.get("type_definitions").is_some());
    }

    #[test]
    fn test_type_generation() {
        assert_eq!(org_type("default"), "organization:default");
        assert_eq!(user_type("alice@example.com"), "user:alice@example.com");
        assert_eq!(role_type("default", "admin"), "role:default_admin");
        assert_eq!(group_type("default", "developers"), "group:default_developers");
        assert_eq!(
            resource_object("default", "logs", "my_stream"),
            "logs:default_my_stream"
        );
        assert_eq!(
            resource_object_all("default", "dfolder"),
            "dfolder:default_all"
        );
    }
}
