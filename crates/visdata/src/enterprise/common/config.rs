// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Enterprise configuration utilities (compatible with o2_enterprise::common::config)

/// Get the enterprise configuration
///
/// This is a re-export of the main config getter for compatibility.
pub use crate::config::get_config;

/// Check if enterprise features are enabled
pub fn is_enterprise_enabled() -> bool {
    // In VisData, enterprise features are always enabled
    true
}

/// Get OpenFGA URL from configuration
pub fn get_openfga_url() -> String {
    // Use environment variable or default
    std::env::var("VISDATA_OPENFGA_URL")
        .unwrap_or_else(|_| "http://localhost:8080".to_string())
}

/// Get Dex URL from configuration
pub fn get_dex_url() -> String {
    // Use environment variable or default
    std::env::var("VISDATA_DEX_GRPC_URL")
        .unwrap_or_else(|_| "http://localhost:5557".to_string())
}

/// Get Dex issuer URL from configuration
pub fn get_dex_issuer_url() -> String {
    std::env::var("VISDATA_DEX_ISSUER_URL")
        .unwrap_or_else(|_| "http://localhost:5556".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_enterprise_enabled() {
        assert!(is_enterprise_enabled());
    }

    #[test]
    fn test_default_urls() {
        // These should return defaults when env vars are not set
        let openfga_url = get_openfga_url();
        assert!(!openfga_url.is_empty());

        let dex_url = get_dex_url();
        assert!(!dex_url.is_empty());
    }
}
