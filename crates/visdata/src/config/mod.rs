// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Configuration module (compatible with o2_openfga::config)
//!
//! This module provides configuration management for the VisData module.

mod types;
mod get_config;

pub use types::*;
pub use get_config::*;
