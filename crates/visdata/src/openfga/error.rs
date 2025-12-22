// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! RBAC module error types

use actix_web::{HttpResponse, ResponseError};
use std::fmt;

/// Result type alias for RBAC operations
pub type Result<T> = std::result::Result<T, Error>;

/// RBAC error types
#[derive(Debug)]
pub enum Error {
    /// Not initialized
    NotInitialized(String),

    /// OpenFGA API error
    OpenFGA(String),

    /// HTTP client error
    Http(reqwest::Error),

    /// Store not found or not created
    StoreNotFound,

    /// Model not found or not created
    ModelNotFound,

    /// Role not found
    RoleNotFound(String),

    /// Group not found
    GroupNotFound(String),

    /// User not found
    UserNotFound(String),

    /// Permission denied
    PermissionDenied(String),

    /// Invalid permission type
    InvalidPermission(String),

    /// Invalid resource type
    InvalidResourceType(String),

    /// Duplicate entry
    DuplicateEntry(String),

    /// Validation error
    Validation(String),

    /// Serialization error
    Serialization(String),

    /// Configuration error
    Config(String),

    /// Internal error
    Internal(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotInitialized(msg) => write!(f, "Not initialized: {}", msg),
            Error::OpenFGA(msg) => write!(f, "OpenFGA error: {}", msg),
            Error::Http(e) => write!(f, "HTTP error: {}", e),
            Error::StoreNotFound => write!(f, "OpenFGA store not found"),
            Error::ModelNotFound => write!(f, "OpenFGA authorization model not found"),
            Error::RoleNotFound(name) => write!(f, "Role not found: {}", name),
            Error::GroupNotFound(name) => write!(f, "Group not found: {}", name),
            Error::UserNotFound(email) => write!(f, "User not found: {}", email),
            Error::PermissionDenied(msg) => write!(f, "Permission denied: {}", msg),
            Error::InvalidPermission(perm) => write!(f, "Invalid permission: {}", perm),
            Error::InvalidResourceType(rt) => write!(f, "Invalid resource type: {}", rt),
            Error::DuplicateEntry(msg) => write!(f, "Duplicate entry: {}", msg),
            Error::Validation(msg) => write!(f, "Validation error: {}", msg),
            Error::Serialization(msg) => write!(f, "Serialization error: {}", msg),
            Error::Config(msg) => write!(f, "Configuration error: {}", msg),
            Error::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Http(e) => Some(e),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Error::Http(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Serialization(err.to_string())
    }
}

impl ResponseError for Error {
    fn error_response(&self) -> HttpResponse {
        match self {
            Error::NotInitialized(_) => HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "message": self.to_string()
            })),
            Error::RoleNotFound(_) | Error::GroupNotFound(_) | Error::UserNotFound(_) => {
                HttpResponse::NotFound().json(serde_json::json!({
                    "message": self.to_string()
                }))
            }
            Error::PermissionDenied(_) => HttpResponse::Forbidden().json(serde_json::json!({
                "message": self.to_string()
            })),
            Error::DuplicateEntry(_) => HttpResponse::Conflict().json(serde_json::json!({
                "message": self.to_string()
            })),
            Error::InvalidPermission(_) | Error::InvalidResourceType(_) | Error::Validation(_) => {
                HttpResponse::BadRequest().json(serde_json::json!({
                    "message": self.to_string()
                }))
            }
            _ => HttpResponse::InternalServerError().json(serde_json::json!({
                "message": self.to_string()
            })),
        }
    }
}
