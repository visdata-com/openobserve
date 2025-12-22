// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Authentication error types

use actix_web::{HttpResponse, ResponseError};
use std::fmt;

/// Result type for auth operations
pub type Result<T> = std::result::Result<T, Error>;

/// Auth-specific error types
#[derive(Debug)]
pub enum Error {
    /// Invalid credentials
    InvalidCredentials(String),
    /// Token validation failed
    InvalidToken(String),
    /// Token expired
    TokenExpired,
    /// User not found
    UserNotFound(String),
    /// Connector not found
    ConnectorNotFound(String),
    /// Connector already exists
    ConnectorExists(String),
    /// Invalid connector configuration
    InvalidConnector(String),
    /// gRPC communication error
    GrpcError(String),
    /// HTTP communication error
    HttpError(String),
    /// Configuration error
    ConfigError(String),
    /// Internal error
    Internal(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidCredentials(msg) => write!(f, "Invalid credentials: {}", msg),
            Error::InvalidToken(msg) => write!(f, "Invalid token: {}", msg),
            Error::TokenExpired => write!(f, "Token has expired"),
            Error::UserNotFound(user) => write!(f, "User not found: {}", user),
            Error::ConnectorNotFound(id) => write!(f, "Connector not found: {}", id),
            Error::ConnectorExists(id) => write!(f, "Connector already exists: {}", id),
            Error::InvalidConnector(msg) => write!(f, "Invalid connector configuration: {}", msg),
            Error::GrpcError(msg) => write!(f, "gRPC error: {}", msg),
            Error::HttpError(msg) => write!(f, "HTTP error: {}", msg),
            Error::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
            Error::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for Error {}

impl ResponseError for Error {
    fn error_response(&self) -> HttpResponse {
        let (status, code) = match self {
            Error::InvalidCredentials(_) => (actix_web::http::StatusCode::UNAUTHORIZED, 401),
            Error::InvalidToken(_) => (actix_web::http::StatusCode::UNAUTHORIZED, 401),
            Error::TokenExpired => (actix_web::http::StatusCode::UNAUTHORIZED, 401),
            Error::UserNotFound(_) => (actix_web::http::StatusCode::NOT_FOUND, 404),
            Error::ConnectorNotFound(_) => (actix_web::http::StatusCode::NOT_FOUND, 404),
            Error::ConnectorExists(_) => (actix_web::http::StatusCode::CONFLICT, 409),
            Error::InvalidConnector(_) => (actix_web::http::StatusCode::BAD_REQUEST, 400),
            Error::GrpcError(_) => (actix_web::http::StatusCode::BAD_GATEWAY, 502),
            Error::HttpError(_) => (actix_web::http::StatusCode::BAD_GATEWAY, 502),
            Error::ConfigError(_) => (actix_web::http::StatusCode::INTERNAL_SERVER_ERROR, 500),
            Error::Internal(_) => (actix_web::http::StatusCode::INTERNAL_SERVER_ERROR, 500),
        };

        HttpResponse::build(status).json(serde_json::json!({
            "code": code,
            "message": self.to_string()
        }))
    }
}

impl From<tonic::Status> for Error {
    fn from(status: tonic::Status) -> Self {
        Error::GrpcError(status.message().to_string())
    }
}

impl From<tonic::transport::Error> for Error {
    fn from(err: tonic::transport::Error) -> Self {
        Error::GrpcError(err.to_string())
    }
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Error::HttpError(err.to_string())
    }
}

impl From<jsonwebtoken::errors::Error> for Error {
    fn from(err: jsonwebtoken::errors::Error) -> Self {
        Error::InvalidToken(err.to_string())
    }
}
