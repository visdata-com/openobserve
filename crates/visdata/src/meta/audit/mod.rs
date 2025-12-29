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

//! Audit logging module for Visdata.
//!
//! This module provides audit types, configuration, and service for audit logging.
//! Similar to `o2_enterprise::enterprise::common::auditor`.
//!
//! ## Architecture
//!
//! ```text
//! router/mod.rs:audit_middleware
//!     ↓ calls crate::service::self_reporting::audit()
//! self_reporting/mod.rs:audit(msg)
//!     ↓ calls visdata::meta::audit::service::audit()
//! visdata::meta::audit::service::audit()
//!     ↓ buffers messages internally
//!     ↓ periodic flush via publish_existing_audits()
//! publish_audit(IngestionRequest)  <- callback from main project
//!     ↓ calls ingestion_service::ingest()
//! _meta/audit stream
//! ```
//!
//! Audit logs are written to the `_meta/audit` stream.

pub mod config;
pub mod service;
pub mod types;

pub use config::{get_audit_interval, is_audit_enabled};
pub use service::{audit, flush_audit, publish_existing_audits};
pub use types::{AuditMessage, Protocol, ResponseMeta};

/// Audit stream name in _meta organization
pub const AUDIT_STREAM: &str = "audit";
