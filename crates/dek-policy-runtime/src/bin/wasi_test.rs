#![allow(clippy::print_stdout)]
// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect
#![allow(unused)]
use std::io::Cursor;
use wasmtime::*;
use wasmtime_wasi::pipe::{MemoryInputPipe, MemoryOutputPipe};
use wasmtime_wasi::WasiCtxBuilder;

// ...
fn main() {
    let input_str = r#"{"allow": true}"#;
    let stdin = MemoryInputPipe::new(bytes::Bytes::from(input_str.to_string().into_bytes()));
    let stdout = MemoryOutputPipe::new(1024 * 1024);

    let mut builder = WasiCtxBuilder::new();
    builder.stdin(stdin.clone());
    builder.stdout(stdout.clone());

    let wasi = builder.build_p1();
    let bytes = stdout.contents();
    println!("bytes len: {}", bytes.len());
}
