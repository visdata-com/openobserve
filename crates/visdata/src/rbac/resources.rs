// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Resource and permission definitions

use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

/// Permission types (matching frontend UI)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Permission {
    /// All permissions
    AllowAll,
    /// List permission (GET for collections)
    AllowList,
    /// Get permission (GET for single resource)
    AllowGet,
    /// Create permission (POST)
    AllowPost,
    /// Update permission (PUT)
    AllowPut,
    /// Delete permission (DELETE)
    AllowDelete,
}

impl Permission {
    /// Check if this permission grants the given action
    pub fn grants(&self, action: &Permission) -> bool {
        match self {
            Permission::AllowAll => true,
            other => other == action,
        }
    }

    /// Convert HTTP method to permission
    pub fn from_http_method(method: &str, is_list: bool) -> Option<Self> {
        match method.to_uppercase().as_str() {
            "GET" => Some(if is_list {
                Permission::AllowList
            } else {
                Permission::AllowGet
            }),
            "POST" => Some(Permission::AllowPost),
            "PUT" | "PATCH" => Some(Permission::AllowPut),
            "DELETE" => Some(Permission::AllowDelete),
            _ => None,
        }
    }
}

impl std::fmt::Display for Permission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Permission::AllowAll => write!(f, "AllowAll"),
            Permission::AllowList => write!(f, "AllowList"),
            Permission::AllowGet => write!(f, "AllowGet"),
            Permission::AllowPost => write!(f, "AllowPost"),
            Permission::AllowPut => write!(f, "AllowPut"),
            Permission::AllowDelete => write!(f, "AllowDelete"),
        }
    }
}

impl std::str::FromStr for Permission {
    type Err = crate::error::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "AllowAll" => Ok(Permission::AllowAll),
            "AllowList" => Ok(Permission::AllowList),
            "AllowGet" => Ok(Permission::AllowGet),
            "AllowPost" => Ok(Permission::AllowPost),
            "AllowPut" => Ok(Permission::AllowPut),
            "AllowDelete" => Ok(Permission::AllowDelete),
            _ => Err(crate::error::Error::InvalidPermission(s.to_string())),
        }
    }
}

/// Resource types (matching frontend UI)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResourceType {
    Stream,
    Logs,
    Metrics,
    Traces,
    Index,
    Functions,
    #[serde(rename = "dfolder")]
    DashboardFolders,
    Dashboard,
    Templates,
    Destinations,
    Alerts,
    #[serde(rename = "alertfolder")]
    AlertFolders,
    Organizations,
    Pipeline,
    Reports,
    #[serde(rename = "savedviews")]
    SavedViews,
    Groups,
    Roles,
    #[serde(rename = "serviceaccounts")]
    ServiceAccounts,
    #[serde(rename = "actionscripts")]
    ActionScripts,
    #[serde(rename = "cipherkeys")]
    CipherKeys,
}

impl std::fmt::Display for ResourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResourceType::Stream => write!(f, "stream"),
            ResourceType::Logs => write!(f, "logs"),
            ResourceType::Metrics => write!(f, "metrics"),
            ResourceType::Traces => write!(f, "traces"),
            ResourceType::Index => write!(f, "index"),
            ResourceType::Functions => write!(f, "functions"),
            ResourceType::DashboardFolders => write!(f, "dfolder"),
            ResourceType::Dashboard => write!(f, "dashboard"),
            ResourceType::Templates => write!(f, "templates"),
            ResourceType::Destinations => write!(f, "destinations"),
            ResourceType::Alerts => write!(f, "alerts"),
            ResourceType::AlertFolders => write!(f, "alertfolder"),
            ResourceType::Organizations => write!(f, "organizations"),
            ResourceType::Pipeline => write!(f, "pipeline"),
            ResourceType::Reports => write!(f, "reports"),
            ResourceType::SavedViews => write!(f, "savedviews"),
            ResourceType::Groups => write!(f, "groups"),
            ResourceType::Roles => write!(f, "roles"),
            ResourceType::ServiceAccounts => write!(f, "serviceaccounts"),
            ResourceType::ActionScripts => write!(f, "actionscripts"),
            ResourceType::CipherKeys => write!(f, "cipherkeys"),
        }
    }
}

impl std::str::FromStr for ResourceType {
    type Err = crate::error::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "stream" => Ok(ResourceType::Stream),
            "logs" => Ok(ResourceType::Logs),
            "metrics" => Ok(ResourceType::Metrics),
            "traces" => Ok(ResourceType::Traces),
            "index" => Ok(ResourceType::Index),
            "functions" => Ok(ResourceType::Functions),
            "dfolder" | "dashboardfolders" => Ok(ResourceType::DashboardFolders),
            "dashboard" => Ok(ResourceType::Dashboard),
            "templates" => Ok(ResourceType::Templates),
            "destinations" => Ok(ResourceType::Destinations),
            "alerts" => Ok(ResourceType::Alerts),
            "alertfolder" | "alertfolders" => Ok(ResourceType::AlertFolders),
            "organizations" => Ok(ResourceType::Organizations),
            "pipeline" | "pipelines" => Ok(ResourceType::Pipeline),
            "reports" => Ok(ResourceType::Reports),
            "savedviews" => Ok(ResourceType::SavedViews),
            "groups" => Ok(ResourceType::Groups),
            "roles" => Ok(ResourceType::Roles),
            "serviceaccounts" => Ok(ResourceType::ServiceAccounts),
            "actionscripts" => Ok(ResourceType::ActionScripts),
            "cipherkeys" => Ok(ResourceType::CipherKeys),
            _ => Err(crate::error::Error::InvalidResourceType(s.to_string())),
        }
    }
}

/// Resource definition for API response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDefinition {
    pub key: String,
    /// Display name for UI
    pub display_name: String,
    #[serde(default)]
    pub has_entities: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<String>>,
    /// Sort order for UI display
    #[serde(default)]
    pub order: i32,
    /// Whether this resource is visible in UI
    #[serde(default = "default_visible")]
    pub visible: bool,
    /// Whether this is a top-level resource
    #[serde(default)]
    pub top_level: bool,
}

fn default_visible() -> bool {
    true
}

/// Static resource definitions (matching frontend expectations)
pub static RESOURCE_DEFINITIONS: LazyLock<Vec<ResourceDefinition>> = LazyLock::new(|| {
    vec![
        ResourceDefinition {
            key: "stream".to_string(),
            display_name: "Streams".to_string(),
            has_entities: true,
            parent: None,
            children: Some(vec![
                "logs".to_string(),
                "metrics".to_string(),
                "traces".to_string(),
                "index".to_string(),
            ]),
            order: 1,
            visible: true,
            top_level: true,
        },
        ResourceDefinition {
            key: "logs".to_string(),
            display_name: "Logs".to_string(),
            has_entities: true,
            parent: Some("stream".to_string()),
            children: None,
            order: 2,
            visible: true,
            top_level: false,
        },
        ResourceDefinition {
            key: "metrics".to_string(),
            display_name: "Metrics".to_string(),
            has_entities: true,
            parent: Some("stream".to_string()),
            children: None,
            order: 3,
            visible: true,
            top_level: false,
        },
        ResourceDefinition {
            key: "traces".to_string(),
            display_name: "Traces".to_string(),
            has_entities: true,
            parent: Some("stream".to_string()),
            children: None,
            order: 4,
            visible: true,
            top_level: false,
        },
        ResourceDefinition {
            key: "index".to_string(),
            display_name: "Index".to_string(),
            has_entities: true,
            parent: Some("stream".to_string()),
            children: None,
            order: 5,
            visible: true,
            top_level: false,
        },
        ResourceDefinition {
            key: "function".to_string(),
            display_name: "Functions".to_string(),
            has_entities: true,
            parent: None,
            children: None,
            order: 10,
            visible: true,
            top_level: true,
        },
        ResourceDefinition {
            key: "dfolder".to_string(),
            display_name: "Dashboard Folders".to_string(),
            has_entities: true,
            parent: None,
            children: Some(vec!["dashboard".to_string()]),
            order: 20,
            visible: true,
            top_level: true,
        },
        ResourceDefinition {
            key: "dashboard".to_string(),
            display_name: "Dashboards".to_string(),
            has_entities: true,
            parent: Some("dfolder".to_string()),
            children: None,
            order: 21,
            visible: false,  // Child of dfolder, not shown at top level
            top_level: false,
        },
        ResourceDefinition {
            key: "template".to_string(),
            display_name: "Templates".to_string(),
            has_entities: true,
            parent: None,
            children: None,
            order: 30,
            visible: true,
            top_level: true,
        },
        ResourceDefinition {
            key: "destination".to_string(),
            display_name: "Destinations".to_string(),
            has_entities: true,
            parent: None,
            children: None,
            order: 31,
            visible: true,
            top_level: true,
        },
        ResourceDefinition {
            key: "afolder".to_string(),
            display_name: "Alert Folders".to_string(),
            has_entities: true,
            parent: None,
            children: Some(vec!["alert".to_string()]),
            order: 40,
            visible: true,
            top_level: true,
        },
        ResourceDefinition {
            key: "alert".to_string(),
            display_name: "Alerts".to_string(),
            has_entities: true,
            parent: Some("afolder".to_string()),
            children: None,
            order: 41,
            visible: false,  // Child of afolder, not shown at top level
            top_level: false,
        },
        ResourceDefinition {
            key: "org".to_string(),
            display_name: "Organizations".to_string(),
            has_entities: false,
            parent: None,
            children: None,
            order: 50,
            visible: true,
            top_level: true,
        },
        ResourceDefinition {
            key: "pipeline".to_string(),
            display_name: "Pipelines".to_string(),
            has_entities: true,
            parent: None,
            children: None,
            order: 60,
            visible: true,
            top_level: true,
        },
        ResourceDefinition {
            key: "report".to_string(),
            display_name: "Reports".to_string(),
            has_entities: true,
            parent: None,
            children: None,
            order: 70,
            visible: true,
            top_level: true,
        },
        ResourceDefinition {
            key: "savedviews".to_string(),
            display_name: "Saved Views".to_string(),
            has_entities: true,
            parent: None,
            children: None,
            order: 80,
            visible: true,
            top_level: true,
        },
        ResourceDefinition {
            key: "enrichment_table".to_string(),
            display_name: "Enrichment Tables".to_string(),
            has_entities: true,
            parent: None,
            children: None,
            order: 85,
            visible: true,
            top_level: true,
        },
        ResourceDefinition {
            key: "group".to_string(),
            display_name: "Groups".to_string(),
            has_entities: true,
            parent: None,
            children: None,
            order: 90,
            visible: true,
            top_level: true,
        },
        ResourceDefinition {
            key: "role".to_string(),
            display_name: "Roles".to_string(),
            has_entities: true,
            parent: None,
            children: None,
            order: 91,
            visible: true,
            top_level: true,
        },
        ResourceDefinition {
            key: "service_accounts".to_string(),
            display_name: "Service Accounts".to_string(),
            has_entities: true,
            parent: None,
            children: None,
            order: 92,
            visible: true,
            top_level: true,
        },
        ResourceDefinition {
            key: "action_scripts".to_string(),
            display_name: "Action Scripts".to_string(),
            has_entities: true,
            parent: None,
            children: None,
            order: 100,
            visible: true,
            top_level: true,
        },
        ResourceDefinition {
            key: "cipher_keys".to_string(),
            display_name: "Cipher Keys".to_string(),
            has_entities: true,
            parent: None,
            children: None,
            order: 110,
            visible: true,
            top_level: true,
        },
        ResourceDefinition {
            key: "metadata".to_string(),
            display_name: "Metadata".to_string(),
            has_entities: true,
            parent: None,
            children: None,
            order: 120,
            visible: true,
            top_level: true,
        },
        ResourceDefinition {
            key: "settings".to_string(),
            display_name: "Settings".to_string(),
            has_entities: false,
            parent: None,
            children: None,
            order: 130,
            visible: true,
            top_level: true,
        },
    ]
});

/// Get resource definitions for API response
pub fn get_resource_definitions() -> Vec<ResourceDefinition> {
    RESOURCE_DEFINITIONS.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_grants() {
        // AllowAll grants everything
        assert!(Permission::AllowAll.grants(&Permission::AllowAll));
        assert!(Permission::AllowAll.grants(&Permission::AllowGet));
        assert!(Permission::AllowAll.grants(&Permission::AllowList));
        assert!(Permission::AllowAll.grants(&Permission::AllowPost));
        assert!(Permission::AllowAll.grants(&Permission::AllowPut));
        assert!(Permission::AllowAll.grants(&Permission::AllowDelete));

        // Specific permissions only grant themselves
        assert!(Permission::AllowGet.grants(&Permission::AllowGet));
        assert!(!Permission::AllowGet.grants(&Permission::AllowPost));
        assert!(!Permission::AllowGet.grants(&Permission::AllowAll));

        assert!(Permission::AllowList.grants(&Permission::AllowList));
        assert!(!Permission::AllowList.grants(&Permission::AllowGet));

        assert!(Permission::AllowPost.grants(&Permission::AllowPost));
        assert!(!Permission::AllowPost.grants(&Permission::AllowPut));

        assert!(Permission::AllowPut.grants(&Permission::AllowPut));
        assert!(!Permission::AllowPut.grants(&Permission::AllowDelete));

        assert!(Permission::AllowDelete.grants(&Permission::AllowDelete));
        assert!(!Permission::AllowDelete.grants(&Permission::AllowGet));
    }

    #[test]
    fn test_permission_from_http_method() {
        // GET methods
        assert_eq!(
            Permission::from_http_method("GET", false),
            Some(Permission::AllowGet)
        );
        assert_eq!(
            Permission::from_http_method("GET", true),
            Some(Permission::AllowList)
        );
        assert_eq!(
            Permission::from_http_method("get", false),
            Some(Permission::AllowGet)
        );

        // POST
        assert_eq!(
            Permission::from_http_method("POST", false),
            Some(Permission::AllowPost)
        );

        // PUT and PATCH
        assert_eq!(
            Permission::from_http_method("PUT", false),
            Some(Permission::AllowPut)
        );
        assert_eq!(
            Permission::from_http_method("PATCH", false),
            Some(Permission::AllowPut)
        );

        // DELETE
        assert_eq!(
            Permission::from_http_method("DELETE", false),
            Some(Permission::AllowDelete)
        );

        // Unknown methods
        assert_eq!(Permission::from_http_method("OPTIONS", false), None);
        assert_eq!(Permission::from_http_method("HEAD", false), None);
    }

    #[test]
    fn test_permission_parse() {
        assert_eq!("AllowAll".parse::<Permission>().unwrap(), Permission::AllowAll);
        assert_eq!("AllowGet".parse::<Permission>().unwrap(), Permission::AllowGet);
        assert_eq!("AllowList".parse::<Permission>().unwrap(), Permission::AllowList);
        assert_eq!("AllowPost".parse::<Permission>().unwrap(), Permission::AllowPost);
        assert_eq!("AllowPut".parse::<Permission>().unwrap(), Permission::AllowPut);
        assert_eq!("AllowDelete".parse::<Permission>().unwrap(), Permission::AllowDelete);
        assert!("InvalidPermission".parse::<Permission>().is_err());
    }

    #[test]
    fn test_permission_display() {
        assert_eq!(Permission::AllowAll.to_string(), "AllowAll");
        assert_eq!(Permission::AllowGet.to_string(), "AllowGet");
        assert_eq!(Permission::AllowList.to_string(), "AllowList");
        assert_eq!(Permission::AllowPost.to_string(), "AllowPost");
        assert_eq!(Permission::AllowPut.to_string(), "AllowPut");
        assert_eq!(Permission::AllowDelete.to_string(), "AllowDelete");
    }

    #[test]
    fn test_resource_type_parse() {
        assert_eq!("logs".parse::<ResourceType>().unwrap(), ResourceType::Logs);
        assert_eq!("LOGS".parse::<ResourceType>().unwrap(), ResourceType::Logs);
        assert_eq!("metrics".parse::<ResourceType>().unwrap(), ResourceType::Metrics);
        assert_eq!("traces".parse::<ResourceType>().unwrap(), ResourceType::Traces);
        assert_eq!("stream".parse::<ResourceType>().unwrap(), ResourceType::Stream);
        assert_eq!("functions".parse::<ResourceType>().unwrap(), ResourceType::Functions);
        assert_eq!("dashboard".parse::<ResourceType>().unwrap(), ResourceType::Dashboard);
        assert_eq!("dfolder".parse::<ResourceType>().unwrap(), ResourceType::DashboardFolders);
        assert_eq!("alerts".parse::<ResourceType>().unwrap(), ResourceType::Alerts);
        assert_eq!("alertfolder".parse::<ResourceType>().unwrap(), ResourceType::AlertFolders);
        assert_eq!("pipeline".parse::<ResourceType>().unwrap(), ResourceType::Pipeline);
        assert_eq!("pipelines".parse::<ResourceType>().unwrap(), ResourceType::Pipeline);
        assert!("invalid_resource".parse::<ResourceType>().is_err());
    }

    #[test]
    fn test_resource_type_display() {
        assert_eq!(ResourceType::Logs.to_string(), "logs");
        assert_eq!(ResourceType::Metrics.to_string(), "metrics");
        assert_eq!(ResourceType::Stream.to_string(), "stream");
        assert_eq!(ResourceType::Dashboard.to_string(), "dashboard");
        assert_eq!(ResourceType::DashboardFolders.to_string(), "dfolder");
        assert_eq!(ResourceType::Alerts.to_string(), "alerts");
        assert_eq!(ResourceType::AlertFolders.to_string(), "alertfolder");
    }

    #[test]
    fn test_resource_definitions_not_empty() {
        let definitions = get_resource_definitions();
        assert!(!definitions.is_empty());

        // Check some expected resources exist
        let keys: Vec<&str> = definitions.iter().map(|d| d.key.as_str()).collect();
        assert!(keys.contains(&"logs"));
        assert!(keys.contains(&"metrics"));
        assert!(keys.contains(&"stream"));
        assert!(keys.contains(&"dashboard"));
        assert!(keys.contains(&"alert"));
    }

    #[test]
    fn test_resource_definitions_hierarchy() {
        let definitions = get_resource_definitions();

        // Stream should have children
        let stream = definitions.iter().find(|d| d.key == "stream").unwrap();
        assert!(stream.children.is_some());
        let children = stream.children.as_ref().unwrap();
        assert!(children.contains(&"logs".to_string()));
        assert!(children.contains(&"metrics".to_string()));
        assert!(children.contains(&"traces".to_string()));

        // Logs should have stream as parent
        let logs = definitions.iter().find(|d| d.key == "logs").unwrap();
        assert_eq!(logs.parent, Some("stream".to_string()));
    }
}
