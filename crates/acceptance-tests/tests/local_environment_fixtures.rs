// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use dek_domain_schema::capabilities::{DeviceCapabilityReport, OsProfile};

pub fn mock_windows_11_environment() -> DeviceCapabilityReport {
    DeviceCapabilityReport {
        device_id: "local-win11-fixture".into(),
        os: OsProfile {
            r#type: "windows".into(),
            version: "11".into(),
            arch: "x86_64".into(),
        },
        peps: vec![],
        pdps: vec![],
        scanned_at: chrono::Utc::now(),
    }
}

pub fn mock_linux_environment() -> DeviceCapabilityReport {
    DeviceCapabilityReport {
        device_id: "local-linux-fixture".into(),
        os: OsProfile {
            r#type: "linux".into(),
            version: "Ubuntu 24.04".into(),
            arch: "x86_64".into(),
        },
        peps: vec![],
        pdps: vec![],
        scanned_at: chrono::Utc::now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixtures_valid() {
        let win = mock_windows_11_environment();
        assert_eq!(win.os.r#type, "windows");

        let lin = mock_linux_environment();
        assert_eq!(lin.os.r#type, "linux");
    }
}
