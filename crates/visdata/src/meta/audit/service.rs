// Copyright 2025 OpenObserve Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

//! Audit service for Visdata.
//!
//! This module provides the audit buffer and publishing logic, similar to
//! `o2_enterprise::enterprise::common::auditor`.
//!
//! The main project provides a `publish_audit` callback function that calls
//! `ingestion_service::ingest()` to write audit logs to `_meta/audit` stream.

use std::{future::Future, sync::Arc};

use once_cell::sync::Lazy;
use proto::cluster_rpc::{IngestionData, IngestionRequest, IngestionResponse};
use tokio::sync::Mutex;

use super::{types::AuditMessage, AUDIT_STREAM};

/// Audit buffer (similar to o2_enterprise::auditor internal buffer)
static AUDIT_BUFFER: Lazy<Arc<Mutex<Vec<AuditMessage>>>> =
    Lazy::new(|| Arc::new(Mutex::new(Vec::new())));

/// Record an audit message to the buffer.
///
/// This is the main entry point for audit logging, similar to
/// `o2_enterprise::auditor::audit()`.
///
/// # Arguments
/// * `org_id` - The organization ID (used for routing, typically `_meta`)
/// * `msg` - The audit message to record
/// * `publish_fn` - Callback function to publish the audit (provided by main project)
///
/// Note: The `publish_fn` is not called immediately; messages are buffered
/// and published periodically via `flush_audit()` or `publish_existing_audits()`.
pub async fn audit<F, Fut>(_org_id: &str, msg: AuditMessage, _publish_fn: F)
where
    F: Fn(IngestionRequest) -> Fut + Send + Sync,
    Fut: Future<Output = Result<IngestionResponse, anyhow::Error>> + Send,
{
    let mut buffer = AUDIT_BUFFER.lock().await;
    buffer.push(msg);
}

/// Flush the audit buffer immediately.
///
/// Similar to `o2_enterprise::auditor::flush_audit()`.
///
/// # Arguments
/// * `org_id` - The organization ID to write to (typically `_meta`)
/// * `publish_fn` - Callback function to publish the audit batch
pub async fn flush_audit<F, Fut>(org_id: &str, publish_fn: F)
where
    F: Fn(IngestionRequest) -> Fut + Send + Sync,
    Fut: Future<Output = Result<IngestionResponse, anyhow::Error>> + Send,
{
    let messages = {
        let mut buffer = AUDIT_BUFFER.lock().await;
        std::mem::take(&mut *buffer)
    };
    if !messages.is_empty() {
        publish_audit_batch(org_id, messages, publish_fn).await;
    }
}

/// Publish existing audits from the buffer.
///
/// Similar to `o2_enterprise::auditor::publish_existing_audits()`.
/// Called by the periodic audit publish job.
///
/// # Arguments
/// * `org_id` - The organization ID to write to (typically `_meta`)
/// * `publish_fn` - Callback function to publish the audit batch
pub async fn publish_existing_audits<F, Fut>(org_id: &str, publish_fn: F)
where
    F: Fn(IngestionRequest) -> Fut + Send + Sync,
    Fut: Future<Output = Result<IngestionResponse, anyhow::Error>> + Send,
{
    flush_audit(org_id, publish_fn).await;
}

/// Internal function to publish a batch of audit messages.
async fn publish_audit_batch<F, Fut>(org_id: &str, messages: Vec<AuditMessage>, publish_fn: F)
where
    F: Fn(IngestionRequest) -> Fut + Send + Sync,
    Fut: Future<Output = Result<IngestionResponse, anyhow::Error>> + Send,
{
    let json_records: Vec<serde_json::Value> = messages
        .into_iter()
        .filter_map(|m| serde_json::to_value(m).ok())
        .collect();

    if json_records.is_empty() {
        return;
    }

    let req = IngestionRequest {
        org_id: org_id.to_string(),
        stream_name: AUDIT_STREAM.to_string(),
        stream_type: "logs".to_string(),
        data: Some(IngestionData {
            data: serde_json::to_vec(&json_records).unwrap_or_default(),
        }),
        ingestion_type: Some(proto::cluster_rpc::IngestionType::Json as i32),
        ..Default::default()
    };

    if let Err(e) = publish_fn(req).await {
        log::error!("[VISDATA] Failed to publish audit logs: {}", e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::audit::types::{Protocol, ResponseMeta};

    #[tokio::test]
    async fn test_audit_buffer() {
        // Clear buffer first
        {
            let mut buffer = AUDIT_BUFFER.lock().await;
            buffer.clear();
        }

        // Create a mock publish function
        async fn mock_publish(
            _req: IngestionRequest,
        ) -> Result<IngestionResponse, anyhow::Error> {
            Ok(IngestionResponse::default())
        }

        // Add an audit message
        let msg = AuditMessage {
            user_email: "test@example.com".to_string(),
            org_id: "test_org".to_string(),
            _timestamp: 1234567890,
            protocol: Protocol::Http,
            response_meta: ResponseMeta {
                http_method: "GET".to_string(),
                http_path: "/api/test".to_string(),
                http_body: "".to_string(),
                http_query_params: "".to_string(),
                http_response_code: 200,
                error_msg: None,
                trace_id: None,
            },
        };

        audit("_meta", msg, mock_publish).await;

        // Check buffer has one message
        {
            let buffer = AUDIT_BUFFER.lock().await;
            assert_eq!(buffer.len(), 1);
        }

        // Flush the buffer
        flush_audit("_meta", mock_publish).await;

        // Check buffer is empty
        {
            let buffer = AUDIT_BUFFER.lock().await;
            assert!(buffer.is_empty());
        }
    }
}
