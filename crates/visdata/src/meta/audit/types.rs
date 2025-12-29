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

//! Audit message types for Visdata.
//!
//! These types are aligned with the enterprise version's `o2_enterprise::enterprise::common::auditor`
//! to ensure compatibility and consistent audit log format.

use serde::{Deserialize, Serialize};

/// Audit message structure aligned with enterprise version.
///
/// This struct captures all relevant information about an API request
/// for audit logging purposes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditMessage {
    /// Email of the user who made the request
    pub user_email: String,
    /// Organization ID the request was made against
    pub org_id: String,
    /// Timestamp in microseconds
    pub _timestamp: i64,
    /// Protocol used (HTTP or gRPC)
    pub protocol: Protocol,
    /// Response metadata containing request/response details
    pub response_meta: ResponseMeta,
}

/// Protocol type for the audit message.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Protocol {
    Http,
    Grpc,
}

/// Response metadata containing HTTP request and response details.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResponseMeta {
    /// HTTP method (GET, POST, PUT, DELETE, etc.)
    pub http_method: String,
    /// API path (without base URI prefix)
    pub http_path: String,
    /// Request body (JSON string or base64 for binary)
    pub http_body: String,
    /// Query parameters string
    pub http_query_params: String,
    /// HTTP response status code
    pub http_response_code: u16,
    /// Error message if the request failed
    pub error_msg: Option<String>,
    /// Trace ID for distributed tracing
    pub trace_id: Option<String>,
}
