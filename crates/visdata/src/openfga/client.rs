// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! OpenFGA HTTP client implementation

use reqwest::Client;
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use super::config::OpenFGAConfig;
use super::error::{Error, Result};
use super::types::*;

/// OpenFGA HTTP client
pub struct OpenFGAClient {
    http: Client,
    config: Arc<RwLock<OpenFGAConfig>>,
}

impl OpenFGAClient {
    /// Create a new OpenFGA client
    pub async fn new(config: &OpenFGAConfig) -> Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()?;

        let client = Self {
            http,
            config: Arc::new(RwLock::new(config.clone())),
        };

        // Initialize store if not configured
        if config.store_id.is_empty() {
            client.init_store().await?;
        }

        Ok(client)
    }

    /// Get the current configuration
    pub async fn config(&self) -> OpenFGAConfig {
        self.config.read().await.clone()
    }

    /// Get the store ID
    pub async fn store_id(&self) -> String {
        self.config.read().await.store_id.clone()
    }

    /// Get the model ID
    pub async fn model_id(&self) -> Option<String> {
        self.config.read().await.model_id.clone()
    }

    /// Initialize store (create if not exists, write model and initial tuples)
    async fn init_store(&self) -> Result<()> {
        use super::model::schema::{get_authorization_model_json, get_initial_tuples};

        let config = self.config.read().await;
        let store_name = config.store_name.clone();
        let api_url = config.api_url.clone();
        drop(config);

        // Try to find existing store
        let stores = self.list_stores().await?;
        let is_new_store;

        if let Some(store) = stores.iter().find(|s| s.name == store_name) {
            let mut config = self.config.write().await;
            config.store_id = store.id.clone();
            tracing::info!("[OpenFGA] Using existing store: {}", store.id);
            is_new_store = false;
        } else {
            // Create new store
            let url = format!("{}/stores", api_url);
            let req = CreateStoreRequest { name: store_name.clone() };

            let resp = self.http.post(&url).json(&req).send().await?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                return Err(Error::OpenFGA(format!(
                    "Failed to create store: {} - {}",
                    status, body
                )));
            }

            let store: CreateStoreResponse = resp.json().await?;
            let mut config = self.config.write().await;
            config.store_id = store.id.clone();
            tracing::info!("[OpenFGA] Created new store: {}", store.id);
            is_new_store = true;
        }

        // Check if model exists, write if not
        let model_id = self.get_latest_model_id().await?;
        if model_id.is_none() {
            tracing::info!("[OpenFGA] No authorization model found, writing default model...");
            let model_json = get_authorization_model_json();
            self.write_authorization_model(model_json).await?;
        } else {
            // Update config with existing model ID
            let mut config = self.config.write().await;
            config.model_id = model_id;
            tracing::info!("[OpenFGA] Using existing authorization model");
        }

        // Write initial tuples only for new store
        if is_new_store {
            tracing::info!("[OpenFGA] Writing initial tuples...");
            let initial_tuples = get_initial_tuples();

            // Write tuples in batches (OpenFGA has a limit per request)
            const BATCH_SIZE: usize = 50;
            for chunk in initial_tuples.chunks(BATCH_SIZE) {
                if let Err(e) = self.write(chunk.to_vec(), vec![]).await {
                    tracing::warn!("[OpenFGA] Failed to write some initial tuples: {}", e);
                    // Continue with other batches, some tuples might already exist
                }
            }
            tracing::info!("[OpenFGA] Initial tuples written successfully");
        }

        Ok(())
    }

    /// List all stores
    pub async fn list_stores(&self) -> Result<Vec<Store>> {
        let config = self.config.read().await;
        let url = format!("{}/stores", config.api_url);
        drop(config);

        let resp = self.http.get(&url).send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::OpenFGA(format!(
                "Failed to list stores: {} - {}",
                status, body
            )));
        }

        let response: ListStoresResponse = resp.json().await?;
        Ok(response.stores)
    }

    /// Check if a user has permission on an object
    pub async fn check(&self, tuple_key: &TupleKey) -> Result<bool> {
        let config = self.config.read().await;
        if config.store_id.is_empty() {
            return Err(Error::StoreNotFound);
        }

        let url = format!("{}/stores/{}/check", config.api_url, config.store_id);
        let req = CheckRequest {
            tuple_key: tuple_key.clone(),
            authorization_model_id: config.model_id.clone(),
        };
        drop(config);

        let resp = self.http.post(&url).json(&req).send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::OpenFGA(format!(
                "Check failed: {} - {}",
                status, body
            )));
        }

        let response: CheckResponse = resp.json().await?;
        Ok(response.allowed)
    }

    /// Write tuples (add and/or delete)
    pub async fn write(
        &self,
        writes: Vec<TupleKey>,
        deletes: Vec<TupleKey>,
    ) -> Result<()> {
        let config = self.config.read().await;
        if config.store_id.is_empty() {
            return Err(Error::StoreNotFound);
        }

        let url = format!("{}/stores/{}/write", config.api_url, config.store_id);

        let req = WriteRequest {
            writes: if writes.is_empty() {
                None
            } else {
                Some(TupleKeys { tuple_keys: writes })
            },
            deletes: if deletes.is_empty() {
                None
            } else {
                Some(TupleKeys { tuple_keys: deletes })
            },
            authorization_model_id: config.model_id.clone(),
        };
        drop(config);

        let resp = self.http.post(&url).json(&req).send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::OpenFGA(format!(
                "Write failed: {} - {}",
                status, body
            )));
        }

        Ok(())
    }

    /// Read tuples with optional filter (handles pagination automatically)
    /// Note: OpenFGA requires object type in filter. If filter is incomplete,
    /// we read all tuples and filter in memory.
    pub async fn read(&self, filter: Option<TupleKeyFilter>) -> Result<Vec<Tuple>> {
        let config = self.config.read().await;
        if config.store_id.is_empty() {
            return Err(Error::StoreNotFound);
        }

        let url = format!("{}/stores/{}/read", config.api_url, config.store_id);
        drop(config);

        // Check if filter is valid for OpenFGA API
        // OpenFGA requires: if tuple_key is provided, object must have type info
        let (api_filter, memory_filter) = match &filter {
            Some(f) if f.object.is_none() => {
                // Invalid filter for API - read all and filter in memory
                (None, filter)
            }
            _ => (filter, None),
        };

        let mut all_tuples = Vec::new();
        let mut continuation_token: Option<String> = None;

        loop {
            let req = ReadRequest {
                tuple_key: api_filter.clone(),
                page_size: Some(100), // OpenFGA max is 100
                continuation_token: continuation_token.clone(),
            };

            let resp = self.http.post(&url).json(&req).send().await?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                return Err(Error::OpenFGA(format!(
                    "Read failed: {} - {}",
                    status, body
                )));
            }

            let response: ReadResponse = resp.json().await?;
            all_tuples.extend(response.tuples);

            // Check if there are more pages
            if response.continuation_token.is_none() || response.continuation_token.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
                break;
            }
            continuation_token = response.continuation_token;
        }

        // Apply memory filter if needed
        if let Some(f) = memory_filter {
            all_tuples.retain(|t| {
                let user_match = f.user.as_ref().map_or(true, |u| t.key.user == *u);
                let relation_match = f.relation.as_ref().map_or(true, |r| t.key.relation == *r);
                let object_match = f.object.as_ref().map_or(true, |o| t.key.object == *o);
                user_match && relation_match && object_match
            });
        }

        Ok(all_tuples)
    }

    /// List objects that a user can access with a specific relation
    pub async fn list_objects(
        &self,
        user: &str,
        relation: &str,
        object_type: &str,
    ) -> Result<Vec<String>> {
        let config = self.config.read().await;
        if config.store_id.is_empty() {
            return Err(Error::StoreNotFound);
        }

        let url = format!("{}/stores/{}/list-objects", config.api_url, config.store_id);
        let req = ListObjectsRequest {
            user: user.to_string(),
            relation: relation.to_string(),
            type_: object_type.to_string(),
            authorization_model_id: config.model_id.clone(),
        };
        drop(config);

        let resp = self.http.post(&url).json(&req).send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::OpenFGA(format!(
                "List objects failed: {} - {}",
                status, body
            )));
        }

        let response: ListObjectsResponse = resp.json().await?;
        Ok(response.objects)
    }

    /// Write authorization model
    pub async fn write_authorization_model(&self, model_json: &str) -> Result<String> {
        let config = self.config.read().await;
        if config.store_id.is_empty() {
            return Err(Error::StoreNotFound);
        }

        let url = format!(
            "{}/stores/{}/authorization-models",
            config.api_url, config.store_id
        );
        drop(config);

        // Parse model JSON
        let model: serde_json::Value = serde_json::from_str(model_json)?;

        let resp = self.http.post(&url).json(&model).send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::OpenFGA(format!(
                "Write model failed: {} - {}",
                status, body
            )));
        }

        #[derive(Deserialize)]
        struct WriteModelResponse {
            authorization_model_id: String,
        }

        let response: WriteModelResponse = resp.json().await?;
        let model_id = response.authorization_model_id;

        // Update config with new model ID
        let mut config = self.config.write().await;
        config.model_id = Some(model_id.clone());

        tracing::info!("[OpenFGA] Created authorization model: {}", model_id);
        Ok(model_id)
    }

    /// Get the latest authorization model ID
    pub async fn get_latest_model_id(&self) -> Result<Option<String>> {
        let config = self.config.read().await;
        if config.store_id.is_empty() {
            return Err(Error::StoreNotFound);
        }

        let url = format!(
            "{}/stores/{}/authorization-models",
            config.api_url, config.store_id
        );
        drop(config);

        let resp = self.http.get(&url).send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::OpenFGA(format!(
                "Get models failed: {} - {}",
                status, body
            )));
        }

        #[derive(Deserialize)]
        struct AuthModel {
            id: String,
        }

        #[derive(Deserialize)]
        struct ListModelsResponse {
            authorization_models: Vec<AuthModel>,
        }

        let response: ListModelsResponse = resp.json().await?;

        Ok(response.authorization_models.first().map(|m| m.id.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tuple_key() {
        let key = TupleKey::new("user:alice", "viewer", "document:doc1");
        assert_eq!(key.user, "user:alice");
        assert_eq!(key.relation, "viewer");
        assert_eq!(key.object, "document:doc1");
    }
}
