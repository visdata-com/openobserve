// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! ID generation utilities using KSUID

use svix_ksuid::{Ksuid, KsuidLike};

/// Generate a new KSUID
pub fn generate_id() -> String {
    Ksuid::new(None, None).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_id() {
        let id1 = generate_id();
        let id2 = generate_id();

        // IDs should be unique
        assert_ne!(id1, id2);

        // KSUID is 27 characters
        assert_eq!(id1.len(), 27);
    }
}
