// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! OpenFGA configuration

use serde::{Deserialize, Serialize};

/// OpenFGA configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenFGAConfig {
    /// OpenFGA HTTP API URL (e.g., "http://localhost:8080")
    pub api_url: String,

    /// Store ID (will be created if not exists)
    #[serde(default)]
    pub store_id: String,

    /// Authorization Model ID (will be created if not exists)
    #[serde(default)]
    pub model_id: Option<String>,

    /// Store name for auto-creation
    #[serde(default = "default_store_name")]
    pub store_name: String,

    /// Enable permission checking
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Only return objects the user has permission to access in list operations
    #[serde(default = "default_true")]
    pub list_only_permitted: bool,

    /// Request timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
}

fn default_store_name() -> String {
    "openobserve".to_string()
}

fn default_true() -> bool {
    true
}

fn default_timeout() -> u64 {
    30
}

impl Default for OpenFGAConfig {
    fn default() -> Self {
        Self {
            api_url: "http://localhost:8080".to_string(),
            store_id: String::new(),
            model_id: None,
            store_name: default_store_name(),
            enabled: true,
            list_only_permitted: true,
            timeout_seconds: default_timeout(),
        }
    }
}

impl OpenFGAConfig {
    /// Set the API URL
    pub fn with_api_url(mut self, url: &str) -> Self {
        self.api_url = url.to_string();
        self
    }

    /// Set the store name
    pub fn with_store_name(mut self, name: &str) -> Self {
        self.store_name = name.to_string();
        self
    }

    /// Set the store ID
    pub fn with_store_id(mut self, id: &str) -> Self {
        self.store_id = id.to_string();
        self
    }

    /// Set the model ID
    pub fn with_model_id(mut self, id: &str) -> Self {
        self.model_id = Some(id.to_string());
        self
    }
}
