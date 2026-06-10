# Pollen DEK macOS System Extension

## Overview
This directory contains the skeleton for the macOS Network Extension (`NEFilterDataProvider`).
Its purpose is to provide `KernelEnforced` capability on macOS by intercepting egress network traffic using Apple's Endpoint Security / Network Extension frameworks, matching the functionality currently provided by eBPF on Linux.

## Future R&D Scope
- **NEFilterDataProvider**: Implement a content filter provider to inspect and drop unauthorized egress flows.
- **Honesty**: Currently, macOS enforcement uses `RedirectAdvisory` (user-mode proxy/ext_authz). Once this system extension is active, `enforcement_level()` will report `KernelEnforced`.
- **Notarization**: macOS requires System Extensions to be signed with a Developer ID Application certificate with the Network Extension entitlement and notarized by Apple.

## Next Steps
- Create an XCode project building the `.systemextension` bundle.
- Provide a Swift/Objective-C bridge to communicate with the Rust `dek-core` user-space daemon via XPC.
