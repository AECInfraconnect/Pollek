# Policy-First / PEP-Transparent Desktop Flow

This document provides the Single Source of Truth for the Desktop Flow and Friendly Message Catalog.

## 1. Flow Overview (Scan -> Protect -> Timeline)

1. **Scan Session**: Automatically discovers agents via eBPF/WFP/browser session readers.
2. **Capability Snapshot**: Checks local capabilities (eBPF, WFP, MCP Stdio wrapping).
3. **Feasibility**: Assesses if the requested control level (Observe, Warn, Enforce) is achievable.
4. **Deployment Session**: Actuates the policy, configuring the underlying PEP transparently.
5. **Warm Check**: Tests if the PEP is actually healthy before marking Active.
6. **Observe**: Degrades to ObserveOnly if enforcement fails.

## 2. 3 Operating Modes

* **Simple Mode**: Focuses strictly on Data Protection and Agent Management. PEP configuration is fully hidden and auto-managed.
* **Enterprise Mode**: Unlocks all capabilities (Simulator, Advanced Auditing, Policy Suggestions, Entities) for power users and administrators.
* **Cloud Mode**: Connects to Pollek Cloud for centralized policy distribution and compliance reporting across an organization.

## 3. Friendly Message Catalog (TH/EN)

| Event / Status | EN Message | TH Message |
| --- | --- | --- |
| Agent Discovered | "New AI Agent discovered: {name}" | "พบ AI Agent ใหม่: {name}" |
| Policy Enforced | "Policy successfully enforced on {name}." | "บังคับใช้นโยบายกับ {name} สำเร็จ" |
| Capability Degraded | "Enforcement capability degraded. Falling back to Observe Mode." | "ความสามารถในการบังคับใช้ลดลง เปลี่ยนเป็นโหมดสังเกตการณ์" |
| Action Blocked | "Blocked action {action} by {name} due to policy." | "ระงับการกระทำ {action} โดย {name} ตามนโยบาย" |

