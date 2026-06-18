// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use std::env;
use std::fs::File;
use std::path::Path;

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("dek-ebpf-prog");
    
    // Always create a dummy file to satisfy include_bytes_aligned! on all platforms
    // and in case the eBPF build fails (e.g. missing bpf-linker)
    let _ = File::create(&dest_path);

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let profile = env::var("PROFILE").unwrap_or_default();

    if target_os != "linux" {
        return;
    }

    // Try reading from DEK_EBPF_OBJECT first
    if let Some(dek_ebpf_object) = env::var_os("DEK_EBPF_OBJECT") {
        if Path::new(&dek_ebpf_object).exists() {
            let _ = std::fs::copy(&dek_ebpf_object, &dest_path);
            return;
        } else if profile == "release" {
            panic!("DEK_EBPF_OBJECT was set but file not found: {:?}", dek_ebpf_object);
        }
    }

    if let Err(e) = aya_build::build_ebpf(std::iter::empty::<cargo_metadata::Package>()) {
        println!("cargo:warning=Failed to build eBPF programs: {}", e);
        if profile == "release" {
            panic!("Failed to build eBPF programs in release profile: {}", e);
        }
    } else {
        // Find the compiled eBPF object and copy it to OUT_DIR/dek-ebpf-prog
        // `aya_build` uses the same profile as the host build (release or debug)
        let src = env::var_os("CARGO_MANIFEST_DIR").map(|dir| {
            Path::new(&dir).join(format!("../../target/bpfel-unknown-none/{}/dek-ebpf-prog", profile))
        }).unwrap_or_else(|| Path::new("").to_path_buf());
        
        if src.exists() {
            let _ = std::fs::copy(&src, &dest_path);
        } else {
            // fallback
            let fallback = env::var_os("CARGO_MANIFEST_DIR").map(|dir| {
                Path::new(&dir).join("../../target/bpfel-unknown-none/release/dek-ebpf-prog")
            }).unwrap();
            
            if fallback.exists() {
                let _ = std::fs::copy(&fallback, &dest_path);
            } else if profile == "release" {
                panic!("eBPF build succeeded but target object not found at {:?}", src);
            }
        }
    }
}
