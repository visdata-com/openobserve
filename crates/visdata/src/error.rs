// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Error types for VisData module

use actix_web::{HttpResponse, ResponseError};
use std::fmt;

/// Result type alias for VisData operations
pub type Result<T> = std::result::Result<T, Error>;

/// Error types for VisData module
#[derive(Debug)]
pub enum Error {
    /// Module already initialized
    AlreadyInitialized,
    /// Module not initialized
    NotInitialized,
    /// Database error
    Database(sea_orm::DbErr),
    /// Configuration error
    Config(String),
    /// Generic not found error
    NotFound(String),
    /// Role not found
    RoleNotFound(String),
    /// Group not found
    GroupNotFound(String),
    /// User not found
    UserNotFound(String),
    /// Permission denied
    PermissionDenied(String),
    /// Invalid permission
    InvalidPermission(String),
    /// Invalid resource type
    InvalidResourceType(String),
    /// Duplicate entry
    DuplicateEntry(String),
    /// SSO provider error
    SSOProvider(String),
    /// OIDC error
    OIDC(String),
    /// LDAP error
    LDAP(String),
    /// JWT error
    JWT(String),
    /// Encryption error
    Encryption(String),
    /// Validation error
    Validation(String),
    /// Internal error
    Internal(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::AlreadyInitialized => write!(f, "VisData module already initialized"),
            Error::NotInitialized => write!(f, "VisData module not initialized"),
            Error::Database(e) => write!(f, "Database error: {}", e),
            Error::Config(msg) => write!(f, "Configuration error: {}", msg),
            Error::NotFound(msg) => write!(f, "Not found: {}", msg),
            Error::RoleNotFound(name) => write!(f, "Role not found: {}", name),
            Error::GroupNotFound(name) => write!(f, "Group not found: {}", name),
            Error::UserNotFound(email) => write!(f, "User not found: {}", email),
            Error::PermissionDenied(msg) => write!(f, "Permission denied: {}", msg),
            Error::InvalidPermission(perm) => write!(f, "Invalid permission: {}", perm),
            Error::InvalidResourceType(rt) => write!(f, "Invalid resource type: {}", rt),
            Error::DuplicateEntry(msg) => write!(f, "Duplicate entry: {}", msg),
            Error::SSOProvider(msg) => write!(f, "SSO provider error: {}", msg),
            Error::OIDC(msg) => write!(f, "OIDC error: {}", msg),
            Error::LDAP(msg) => write!(f, "LDAP error: {}", msg),
            Error::JWT(msg) => write!(f, "JWT error: {}", msg),
            Error::Encryption(msg) => write!(f, "Encryption error: {}", msg),
            Error::Validation(msg) => write!(f, "Validation error: {}", msg),
            Error::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Database(e) => Some(e),
            _ => None,
        }
    }
}

impl From<sea_orm::DbErr> for Error {
    fn from(err: sea_orm::DbErr) -> Self {
        Error::Database(err)
    }
}

impl From<jsonwebtoken::errors::Error> for Error {
    fn from(err: jsonwebtoken::errors::Error) -> Self {
        Error::JWT(err.to_string())
    }
}

impl From<aes_gcm::Error> for Error {
    fn from(err: aes_gcm::Error) -> Self {
        Error::Encryption(err.to_string())
    }
}

impl ResponseError for Error {
    fn error_response(&self) -> HttpResponse {
        match self {
            Error::NotFound(_) | Error::RoleNotFound(_) | Error::GroupNotFound(_) | Error::UserNotFound(_) => {
                HttpResponse::NotFound().json(serde_json::json!({
                    "message": self.to_string()
                }))
            }
            Error::PermissionDenied(_) => {
                HttpResponse::Forbidden().json(serde_json::json!({
                    "message": self.to_string()
                }))
            }
            Error::DuplicateEntry(_) => {
                HttpResponse::Conflict().json(serde_json::json!({
                    "message": self.to_string()
                }))
            }
            Error::InvalidPermission(_)
            | Error::InvalidResourceType(_)
            | Error::Validation(_)
            | Error::Config(_) => {
                HttpResponse::BadRequest().json(serde_json::json!({
                    "message": self.to_string()
                }))
            }
            _ => {
                HttpResponse::InternalServerError().json(serde_json::json!({
                    "message": self.to_string()
                }))
            }
        }
    }
}
