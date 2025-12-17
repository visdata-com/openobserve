// Copyright 2025 VisData Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Compile Dex gRPC proto
    tonic_prost_build::configure()
        .build_server(false)
        .build_client(true)
        .compile_protos(&["proto/dex/api.proto"], &["proto"])?;

    println!("cargo:rerun-if-changed=proto/dex/api.proto");

    Ok(())
}
