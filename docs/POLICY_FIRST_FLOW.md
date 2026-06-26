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
* **Advance Mode**: Unlocks local power-user capabilities such as Simulator, detailed auditing, Policy Suggestions, Entities, Tools, Identities, and control-method diagnostics.
* **Enterprise Cloud Mode**: Unlocks only after Pollek Cloud is configured and the connection probe succeeds. It enables centralized policy distribution, hot reload, telemetry sync, SPIFFE/OAuth-backed workload tracing, and compliance reporting across an organization.

## 3. Friendly Message Catalog (TH/EN)

| Event / Status | EN Message | TH Message |
| --- | --- | --- |
| Agent Discovered | "New AI Agent discovered: {name}" | "�� AI Agent ����: {name}" |
| Policy Enforced | "Policy successfully enforced on {name}." | "�ѧ�Ѻ���º�¡Ѻ {name} �����" |
| Capability Degraded | "Enforcement capability degraded. Falling back to Observe Mode." | "��������ö㹡�úѧ�Ѻ��Ŵŧ ����¹�������ѧࡵ��ó�" |
| Action Blocked | "Blocked action {action} by {name} due to policy." | "�ЧѺ��á�з� {action} �� {name} �����º��" |
