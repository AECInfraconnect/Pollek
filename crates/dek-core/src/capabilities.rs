// SPDX-License-Identifier: Apache-2.0

use dek_bundle_format::OsModulesConfig;

pub fn collect_runtime_capabilities() -> OsModulesConfig {
    #[cfg(target_os = "linux")]
    let linux_w = if dek_ebpfd::probe_available() {
        vec!["ebpfd.v1".to_string()]
    } else {
        vec!["ebpfd.stub".to_string()]
    };
    #[cfg(not(target_os = "linux"))]
    let linux_w = vec!["ebpfd.stub".to_string()];

    #[cfg(target_os = "windows")]
    let windows_w = if dek_windows_wfp::probe_available() {
        vec!["wfp.v1".to_string()]
    } else {
        vec!["wfp.stub".to_string()]
    };
    #[cfg(not(target_os = "windows"))]
    let windows_w = vec!["wfp.stub".to_string()];

    #[cfg(target_os = "macos")]
    let macos_w = if dek_macos_nefilter::probe_available() {
        vec!["nefilter.v1".to_string()]
    } else {
        vec!["nefilter.stub".to_string()]
    };
    #[cfg(not(target_os = "macos"))]
    let macos_w = vec!["nefilter.stub".to_string()];

    OsModulesConfig {
        linux: linux_w,
        windows: windows_w,
        macos: macos_w,
    }
}
