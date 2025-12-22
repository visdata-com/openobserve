// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Configuration getter functions (compatible with o2_openfga::config)

use crate::Visdata;
use super::VisdataConfig;

/// Get the current configuration (compatible with o2_openfga::config::get_config)
///
/// This function returns the current VisData configuration.
/// If the module is not initialized, it returns the default configuration.
pub async fn get_config() -> VisdataConfig {
    match Visdata::try_global() {
        Some(visdata) => visdata.config().await,
        None => VisdataConfig::default(),
    }
}

/// Get the OpenFGA configuration as a reference
pub fn get_openfga_config() -> Option<&'static crate::openfga::OpenFGAConfig> {
    Visdata::try_global().map(|v| v.openfga_config())
}

/// Get the Dex configuration as a reference
pub fn get_dex_config() -> Option<&'static crate::dex::DexConfig> {
    Visdata::try_global().map(|v| v.dex_config())
}

/// Check if RBAC is enabled
pub async fn is_rbac_enabled() -> bool {
    get_config().await.rbac_enabled
}

/// Check if SSO is enabled
pub async fn is_sso_enabled() -> bool {
    get_config().await.sso_enabled
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_config_default() {
        // Without initialization, should return default config
        let config = get_config().await;
        assert!(config.rbac_enabled);
        assert!(config.sso_enabled);
    }
}
