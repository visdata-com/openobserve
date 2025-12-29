// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Authorization model definitions

pub mod resources;
pub mod schema;

pub use resources::{RESOURCE_TYPES, get_resource, get_all_resources};
pub use schema::{get_authorization_model_json, get_initial_tuples};
