// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use std::env;
use std::fs::File;
use std::path::Path;

fn main() {
    // Always create a dummy file to satisfy include_bytes_aligned! on all platforms
    // and in case the eBPF build fails (e.g. missing bpf-linker)
    if let Some(out_dir) = env::var_os("OUT_DIR") {
        let dest_path = Path::new(&out_dir).join("dek-ebpf-prog");
        let _ = File::create(&dest_path);
    }

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    // eBPF is only applicable on Linux
    if target_os != "linux" {
        return;
    }

    if let Err(e) = aya_build::build_ebpf(std::iter::empty::<cargo_metadata::Package>()) {
        println!("cargo:warning=Failed to build eBPF programs: {}", e);
    } else {
        // Find the compiled eBPF object and copy it to OUT_DIR/dek-ebpf-prog
        let target_dir = env::var_os("CARGO_MANIFEST_DIR").map(|dir| {
            Path::new(&dir).join("../../target/bpfel-unknown-none/release/dek-ebpf-prog")
        });
        if let Some(src) = target_dir {
            if src.exists() {
                if let Some(out_dir) = env::var_os("OUT_DIR") {
                    let dest = Path::new(&out_dir).join("dek-ebpf-prog");
                    let _ = std::fs::copy(&src, &dest);
                }
            }
        }
    }
}
