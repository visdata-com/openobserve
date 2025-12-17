// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Connector management service

use crate::Visdata;
use super::super::error::{Error, Result};
use super::super::types::{
    CreateOidcConnectorRequest, CreateLdapConnectorRequest, CreateSamlConnectorRequest,
    SsoProvider,
};

/// Create an OIDC connector
pub async fn create_oidc_connector(req: CreateOidcConnectorRequest) -> Result<()> {
    let visdata = Visdata::global();
    let mut dex = visdata.dex().write().await;

    // Build OIDC connector config
    let config = serde_json::json!({
        "issuer": req.issuer,
        "clientID": req.client_id,
        "clientSecret": req.client_secret,
        "redirectURI": req.redirect_uri,
        "scopes": req.scopes,
        "insecureSkipEmailVerified": false,
        "insecureEnableGroups": true,
        "getUserInfo": true,
        "claimMapping": {
            "groups": req.groups_claim.unwrap_or_else(|| "groups".to_string()),
            "email": req.email_claim.unwrap_or_else(|| "email".to_string()),
        }
    });

    dex.create_connector(&req.id, "oidc", &req.name, &config.to_string())
        .await?;

    Ok(())
}

/// Create an LDAP connector
pub async fn create_ldap_connector(req: CreateLdapConnectorRequest) -> Result<()> {
    let visdata = Visdata::global();
    let mut dex = visdata.dex().write().await;

    // Build LDAP connector config
    let config = serde_json::json!({
        "host": format!("{}:{}", req.host, req.port),
        "insecureNoSSL": !req.use_ssl,
        "startTLS": req.start_tls,
        "insecureSkipVerify": req.insecure_skip_verify,
        "bindDN": req.bind_dn,
        "bindPW": req.bind_password,
        "userSearch": {
            "baseDN": req.user_search_base_dn,
            "filter": req.user_search_filter.unwrap_or_else(|| "(objectClass=person)".to_string()),
            "username": req.user_search_username.unwrap_or_else(|| "uid".to_string()),
            "idAttr": req.user_search_id_attr.unwrap_or_else(|| "uid".to_string()),
            "emailAttr": req.user_search_email_attr.unwrap_or_else(|| "mail".to_string()),
            "nameAttr": req.user_search_name_attr.unwrap_or_else(|| "cn".to_string()),
        },
        "groupSearch": req.group_search_base_dn.map(|base_dn| {
            serde_json::json!({
                "baseDN": base_dn,
                "filter": req.group_search_filter.unwrap_or_else(|| "(objectClass=groupOfNames)".to_string()),
                "userMatchers": [
                    {"userAttr": "DN", "groupAttr": "member"}
                ],
                "nameAttr": "cn"
            })
        })
    });

    dex.create_connector(&req.id, "ldap", &req.name, &config.to_string())
        .await?;

    Ok(())
}

/// Create a SAML connector
pub async fn create_saml_connector(req: CreateSamlConnectorRequest) -> Result<()> {
    let visdata = Visdata::global();
    let mut dex = visdata.dex().write().await;

    // Build SAML connector config
    let config = serde_json::json!({
        "ssoURL": req.sso_url,
        "entityIssuer": req.entity_issuer,
        "ssoIssuer": req.sso_issuer,
        "ca": req.ca,
        "redirectURI": req.redirect_uri,
        "nameIDPolicyFormat": "emailAddress",
        "usernameAttr": req.name_attr.unwrap_or_else(|| "name".to_string()),
        "emailAttr": req.email_attr.unwrap_or_else(|| "email".to_string()),
        "groupsAttr": req.groups_attr.unwrap_or_else(|| "groups".to_string()),
    });

    dex.create_connector(&req.id, "saml", &req.name, &config.to_string())
        .await?;

    Ok(())
}

/// List all connectors
pub async fn list_connectors() -> Result<Vec<SsoProvider>> {
    let visdata = Visdata::global();
    let mut dex = visdata.dex().write().await;

    let connectors = dex.list_connectors().await?;

    let providers: Vec<SsoProvider> = connectors
        .into_iter()
        .map(|c| SsoProvider {
            id: c.id,
            provider_type: c.connector_type,
            name: c.name,
            enabled: true, // Dex doesn't have enabled flag, assume all are enabled
        })
        .collect();

    Ok(providers)
}

/// Get connector details
pub async fn get_connector(id: &str) -> Result<SsoProvider> {
    let providers = list_connectors().await?;

    providers
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| Error::ConnectorNotFound(id.to_string()))
}

/// Delete a connector
pub async fn delete_connector(id: &str) -> Result<()> {
    let visdata = Visdata::global();
    let mut dex = visdata.dex().write().await;

    dex.delete_connector(id).await
}

/// Update a connector
pub async fn update_connector(
    id: &str,
    connector_type: &str,
    name: &str,
    config_json: &str,
) -> Result<()> {
    let visdata = Visdata::global();
    let mut dex = visdata.dex().write().await;

    dex.update_connector(id, connector_type, name, config_json)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oidc_config_serialization() {
        let config = serde_json::json!({
            "issuer": "https://accounts.google.com",
            "clientID": "test-client",
            "clientSecret": "test-secret",
        });

        let serialized = config.to_string();
        assert!(serialized.contains("accounts.google.com"));
    }
}
