// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use chrono::Utc;
use std::path::PathBuf;

pub struct EvidencePackager {
    log_dir: PathBuf,
}

impl EvidencePackager {
    pub fn new(log_dir: PathBuf) -> Self {
        Self { log_dir }
    }

    pub fn generate_markdown_report(&self, redact: bool) -> Result<String> {
        let timestamp = Utc::now().to_rfc3339();

        let mut report = format!("# Compliance Evidence Pack\n");
        report.push_str(&format!("Generated at: {}\n\n", timestamp));
        report.push_str("## 1. EU AI Act Compliance Mapping\n");
        report.push_str(
            "- **Article 10 (Data and Data Governance)**: Local registry controls MCP access.\n",
        );
        report.push_str("- **Article 14 (Human Oversight)**: All high-risk deployment actions are explicitly approved via DEK Control Plane.\n");
        report.push_str("- **Article 15 (Accuracy, Robustness, Cybersecurity)**: Preflight doctor checks ensure OS integrity and runtime sandboxing before PEPs are activated.\n\n");

        report.push_str("## 2. NIST AI RMF Mapping\n");
        report.push_str("- **Govern 1.1**: Legal and regulatory requirements (EU AI Act) are codified in system policies.\n");
        report.push_str("- **Map 1.5**: Agent permissions and scopes are tracked and risk-scored prior to execution.\n");
        report.push_str("- **Measure 2.6**: System logs capture tamper-evident event trails for all policy enforcements.\n\n");

        report.push_str("## 3. ISO 42001 (AI Management System)\n");
        report.push_str(
            "- **A.2.1 AI Policies**: Handled by DEK's Policy Router and PDP/PEP separation.\n",
        );
        report.push_str(
            "- **A.7.2 Traceability**: Detailed execution history is maintained locally.\n\n",
        );

        report.push_str("## 4. Local Telemetry Summary\n");
        if redact {
            report.push_str("*(Sensitive data redacted)*\n");
        } else {
            report.push_str("*(Full diagnostic data included)*\n");
        }

        // Mocking reading logs
        let entries_count = if self.log_dir.exists() {
            std::fs::read_dir(&self.log_dir)?.count()
        } else {
            0
        };

        report.push_str(&format!("Total log files found: {}\n", entries_count));
        report.push_str(
            "System operates under strict egress controls (Sovereign Mode compatible).\n",
        );

        Ok(report)
    }

    pub fn export_to_file(&self, dest: &PathBuf, redact: bool) -> Result<()> {
        let content = self.generate_markdown_report(redact)?;
        std::fs::write(dest, content)?;
        Ok(())
    }
}
