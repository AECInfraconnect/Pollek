// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use std::io::{self, Read};

fn main() {
    let mut input = String::new();
    let _ = io::stdin().read_to_string(&mut input); // Read inputs (optional for dummy)

    let allow = true;
    let reason = "Allowed by Dummy In-Memory WASM Policy";

    // Write JSON to stdout
    let output = format!(r#"{{"allow": {}, "reason": "{}"}}"#, allow, reason);
    print!("{}", output);
}
