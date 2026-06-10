# Pollen DEK Windows WFP Driver

## Overview
This directory contains the skeleton for the Windows Filtering Platform (WFP) Callout Driver (`.sys`). 
Its purpose is to provide `KernelEnforced` capability on Windows by intercepting egress network traffic at the ALE (Application Layer Enforcement) layer, matching the functionality currently provided by eBPF on Linux.

## Future R&D Scope
- **FwpsCalloutRegister**: Register callouts at `FWPM_LAYER_ALE_AUTH_CONNECT_V4` and `FWPM_LAYER_ALE_AUTH_CONNECT_V6`.
- **Honesty**: Currently, Windows enforcement uses `RedirectAdvisory` (user-mode proxy). Once this driver is active, `enforcement_level()` will report `KernelEnforced`.
- **EV/WHQL Signing**: To run on 64-bit Windows systems, the `.sys` file must be countersigned by Microsoft via the Windows Hardware Developer Center dashboard.

## Next Steps
- Port the eBPF matching logic to a WFP C/C++ driver or use `windows-kernel-rs` for a Rust-based driver.
- Establish a user-to-kernel communication channel (DeviceIoControl) to sync rules.
