// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! SDR (Structured Data Recognition) Type Recognizer
//!
//! Automatically identifies variable types in log patterns such as:
//! - IP addresses (IPv4/IPv6)
//! - Numbers (integers/floats)
//! - UUIDs
//! - Timestamps
//! - File paths
//! - URLs
//! - Email addresses
//! - Hexadecimal strings

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fmt;

/// SDR (Structured Data Recognition) types
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SdrType {
    /// IPv4 or IPv6 address
    Ip,
    /// Numeric value (integer or float)
    Num,
    /// UUID format
    Uuid,
    /// Timestamp format
    Timestamp,
    /// File path
    Path,
    /// URL
    Url,
    /// Email address
    Email,
    /// Hexadecimal string
    Hex,
    /// Generic string (fallback)
    String,
}

impl fmt::Display for SdrType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SdrType::Ip => write!(f, "IP"),
            SdrType::Num => write!(f, "NUM"),
            SdrType::Uuid => write!(f, "UUID"),
            SdrType::Timestamp => write!(f, "TIMESTAMP"),
            SdrType::Path => write!(f, "PATH"),
            SdrType::Url => write!(f, "URL"),
            SdrType::Email => write!(f, "EMAIL"),
            SdrType::Hex => write!(f, "HEX"),
            SdrType::String => write!(f, "STRING"),
        }
    }
}

impl SdrType {
    /// Get the placeholder format for this type
    pub fn placeholder(&self) -> String {
        format!("<:{}>", self)
    }

    /// Get the human-readable placeholder (simple wildcard)
    pub fn simple_placeholder(&self) -> &'static str {
        "<*>"
    }
}

// Pre-compiled regex patterns for SDR recognition
// Order matters: more specific patterns should come first

/// IPv4 address pattern
static RE_IPV4: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)$").unwrap()
});

/// IPv6 address pattern (simplified)
static RE_IPV6: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(?:[0-9a-fA-F]{1,4}:){7}[0-9a-fA-F]{1,4}$|^::(?:[0-9a-fA-F]{1,4}:){0,6}[0-9a-fA-F]{1,4}$|^(?:[0-9a-fA-F]{1,4}:){1,7}:$|^(?:[0-9a-fA-F]{1,4}:){1,6}:[0-9a-fA-F]{1,4}$").unwrap()
});

/// UUID pattern
static RE_UUID: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$")
        .unwrap()
});

/// ISO 8601 timestamp pattern
static RE_TIMESTAMP_ISO: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})?$")
        .unwrap()
});

/// Common timestamp pattern (YYYY-MM-DD HH:MM:SS)
static RE_TIMESTAMP_COMMON: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\d{4}[-/]\d{2}[-/]\d{2}[T ]?\d{2}:\d{2}:\d{2}$").unwrap()
});

/// Unix timestamp (10 or 13 digits)
static RE_TIMESTAMP_UNIX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^1[0-9]{9}(?:[0-9]{3})?$").unwrap());

/// URL pattern
static RE_URL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^https?://[^\s/$.?#].[^\s]*$").unwrap()
});

/// Email pattern
static RE_EMAIL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap()
});

/// File path pattern (Unix or Windows)
static RE_PATH: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(?:/[^/\s]+)+/?$|^[a-zA-Z]:\\(?:[^\\/:*?<>|\r\n]+\\)*[^\\/:*?<>|\r\n]*$").unwrap()
});

/// Hexadecimal pattern (with 0x prefix or pure hex)
static RE_HEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(?:0x)?[0-9a-fA-F]{8,}$").unwrap()
});

/// Integer pattern
static RE_INTEGER: Lazy<Regex> = Lazy::new(|| Regex::new(r"^-?\d+$").unwrap());

/// Float pattern
static RE_FLOAT: Lazy<Regex> = Lazy::new(|| Regex::new(r"^-?\d+\.\d+$").unwrap());

/// SDR Recognizer for identifying structured data types in tokens
#[derive(Clone, Debug)]
pub struct SdrRecognizer {
    /// Whether to use detailed SDR types or simple wildcards
    pub use_detailed_types: bool,
}

impl Default for SdrRecognizer {
    fn default() -> Self {
        Self::new()
    }
}

impl SdrRecognizer {
    /// Create a new SDR recognizer with detailed type recognition
    pub fn new() -> Self {
        Self {
            use_detailed_types: true,
        }
    }

    /// Create a recognizer that uses simple wildcards only
    pub fn simple() -> Self {
        Self {
            use_detailed_types: false,
        }
    }

    /// Recognize the SDR type of a token
    ///
    /// Returns the most specific matching type, or `SdrType::String` as fallback.
    /// Recognition order (most specific first):
    /// 1. UUID
    /// 2. Timestamp (ISO, common, Unix)
    /// 3. URL
    /// 4. Email
    /// 5. IPv4/IPv6
    /// 6. Hex (with 0x prefix)
    /// 7. Path
    /// 8. Float
    /// 9. Integer
    /// 10. String (fallback)
    pub fn recognize(&self, token: &str) -> SdrType {
        if token.is_empty() {
            return SdrType::String;
        }

        // UUID (most specific)
        if RE_UUID.is_match(token) {
            return SdrType::Uuid;
        }

        // Timestamps (various formats)
        if RE_TIMESTAMP_ISO.is_match(token)
            || RE_TIMESTAMP_COMMON.is_match(token)
            || RE_TIMESTAMP_UNIX.is_match(token)
        {
            return SdrType::Timestamp;
        }

        // URL
        if RE_URL.is_match(token) {
            return SdrType::Url;
        }

        // Email
        if RE_EMAIL.is_match(token) {
            return SdrType::Email;
        }

        // IP addresses
        if RE_IPV4.is_match(token) || RE_IPV6.is_match(token) {
            return SdrType::Ip;
        }

        // Hex (with 0x prefix, at least 8 chars)
        if token.starts_with("0x") && RE_HEX.is_match(token) {
            return SdrType::Hex;
        }

        // File paths
        if RE_PATH.is_match(token) {
            return SdrType::Path;
        }

        // Numbers (float before integer)
        if RE_FLOAT.is_match(token) {
            return SdrType::Num;
        }

        if RE_INTEGER.is_match(token) {
            return SdrType::Num;
        }

        // Pure hex strings (without 0x, but long enough)
        if token.len() >= 16 && RE_HEX.is_match(token) {
            return SdrType::Hex;
        }

        // Default fallback
        SdrType::String
    }

    /// Get the placeholder for a token based on its recognized type
    pub fn get_placeholder(&self, token: &str) -> String {
        let sdr_type = self.recognize(token);
        if self.use_detailed_types {
            sdr_type.placeholder()
        } else {
            sdr_type.simple_placeholder().to_string()
        }
    }

    /// Check if a token is a variable (non-constant) based on common patterns
    ///
    /// Variables are tokens that are likely to change between log instances,
    /// such as IDs, timestamps, numbers, etc.
    pub fn is_variable(&self, token: &str) -> bool {
        let sdr_type = self.recognize(token);
        // String type might still be a variable if it looks like an ID
        match sdr_type {
            SdrType::String => {
                // Check if it looks like a variable (mixed case/numbers, or very long)
                let has_digits = token.chars().any(|c| c.is_ascii_digit());
                let is_long = token.len() > 20;
                has_digits || is_long
            }
            _ => true, // All other types are considered variables
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipv4_recognition() {
        let sdr = SdrRecognizer::new();
        assert_eq!(sdr.recognize("192.168.1.1"), SdrType::Ip);
        assert_eq!(sdr.recognize("10.0.0.1"), SdrType::Ip);
        assert_eq!(sdr.recognize("255.255.255.255"), SdrType::Ip);
    }

    #[test]
    fn test_uuid_recognition() {
        let sdr = SdrRecognizer::new();
        assert_eq!(
            sdr.recognize("550e8400-e29b-41d4-a716-446655440000"),
            SdrType::Uuid
        );
        assert_eq!(
            sdr.recognize("A550E840-E29B-41D4-A716-446655440000"),
            SdrType::Uuid
        );
    }

    #[test]
    fn test_timestamp_recognition() {
        let sdr = SdrRecognizer::new();
        assert_eq!(sdr.recognize("2025-01-01T00:00:00Z"), SdrType::Timestamp);
        assert_eq!(
            sdr.recognize("2025-01-01T00:00:00+08:00"),
            SdrType::Timestamp
        );
        assert_eq!(sdr.recognize("2025-01-01 12:30:45"), SdrType::Timestamp);
        assert_eq!(sdr.recognize("1735689600000"), SdrType::Timestamp); // Unix ms
    }

    #[test]
    fn test_number_recognition() {
        let sdr = SdrRecognizer::new();
        assert_eq!(sdr.recognize("123"), SdrType::Num);
        assert_eq!(sdr.recognize("-456"), SdrType::Num);
        assert_eq!(sdr.recognize("3.14159"), SdrType::Num);
        assert_eq!(sdr.recognize("-2.718"), SdrType::Num);
    }

    #[test]
    fn test_url_recognition() {
        let sdr = SdrRecognizer::new();
        assert_eq!(sdr.recognize("https://example.com"), SdrType::Url);
        assert_eq!(sdr.recognize("http://localhost:8080/api"), SdrType::Url);
    }

    #[test]
    fn test_email_recognition() {
        let sdr = SdrRecognizer::new();
        assert_eq!(sdr.recognize("user@example.com"), SdrType::Email);
        assert_eq!(sdr.recognize("test.user@domain.org"), SdrType::Email);
    }

    #[test]
    fn test_path_recognition() {
        let sdr = SdrRecognizer::new();
        assert_eq!(sdr.recognize("/var/log/app.log"), SdrType::Path);
        assert_eq!(sdr.recognize("/usr/local/bin"), SdrType::Path);
    }

    #[test]
    fn test_hex_recognition() {
        let sdr = SdrRecognizer::new();
        assert_eq!(sdr.recognize("0x1a2b3c4d5e6f7890"), SdrType::Hex);
    }

    #[test]
    fn test_string_fallback() {
        let sdr = SdrRecognizer::new();
        assert_eq!(sdr.recognize("hello"), SdrType::String);
        assert_eq!(sdr.recognize("ERROR"), SdrType::String);
        assert_eq!(sdr.recognize("user_logged_in"), SdrType::String);
    }

    #[test]
    fn test_placeholder_format() {
        assert_eq!(SdrType::Ip.placeholder(), "<:IP>");
        assert_eq!(SdrType::Num.placeholder(), "<:NUM>");
        assert_eq!(SdrType::Uuid.placeholder(), "<:UUID>");
        assert_eq!(SdrType::String.placeholder(), "<:STRING>");
    }
}
