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

//! Audit configuration for Visdata.
//!
//! Supports two environment variable naming conventions:
//! - `ZO_AUDIT_*` - Standard OpenObserve prefix (compatible with enterprise)
//! - `VISDATA_AUDIT_*` - Visdata-specific prefix
//!
//! Priority: `ZO_AUDIT_*` > `VISDATA_AUDIT_*` > default values

/// Check if audit logging is enabled.
///
/// Reads from environment variables:
/// - `ZO_AUDIT_ENABLED` (priority)
/// - `VISDATA_AUDIT_ENABLED` (fallback)
///
/// Returns `false` by default if neither is set.
pub fn is_audit_enabled() -> bool {
    std::env::var("ZO_AUDIT_ENABLED")
        .or_else(|_| std::env::var("VISDATA_AUDIT_ENABLED"))
        .map(|v| v.to_lowercase() == "true" || v == "1")
        .unwrap_or(false)
}

/// Get the audit publish interval in seconds.
///
/// Reads from environment variables:
/// - `ZO_AUDIT_PUBLISH_INTERVAL` (priority)
/// - `VISDATA_AUDIT_PUBLISH_INTERVAL` (fallback)
///
/// Returns `30` seconds by default if neither is set.
pub fn get_audit_interval() -> u64 {
    std::env::var("ZO_AUDIT_PUBLISH_INTERVAL")
        .or_else(|_| std::env::var("VISDATA_AUDIT_PUBLISH_INTERVAL"))
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_audit_disabled() {
        // Clear any existing env vars
        std::env::remove_var("ZO_AUDIT_ENABLED");
        std::env::remove_var("VISDATA_AUDIT_ENABLED");

        assert!(!is_audit_enabled());
    }

    #[test]
    fn test_default_audit_interval() {
        // Clear any existing env vars
        std::env::remove_var("ZO_AUDIT_PUBLISH_INTERVAL");
        std::env::remove_var("VISDATA_AUDIT_PUBLISH_INTERVAL");

        assert_eq!(get_audit_interval(), 30);
    }
}
