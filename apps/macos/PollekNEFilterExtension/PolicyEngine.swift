import Foundation

final class PolicyEngine {
    private var bundle: PollekPolicyBundle?
    private let lock = NSLock()

    init() {
        reload()
    }

    func reload() {
        let loaded = SharedStore.shared.loadPolicyBundle()
        lock.lock()
        self.bundle = loaded
        lock.unlock()
    }

    func decide(flow: FlowMetadata) -> PolicyDecision {
        lock.lock()
        let current = bundle
        lock.unlock()

        guard let current = current else {
            return PolicyDecision(
                action: .needMoreRules,
                ruleId: nil,
                reason: "No local policy bundle loaded",
                auditRequired: true
            )
        }

        for rule in current.rules {
            if matches(rule: rule, flow: flow) {
                return PolicyDecision(
                    action: rule.action,
                    ruleId: rule.id,
                    reason: rule.reason,
                    auditRequired: true
                )
            }
        }

        return PolicyDecision(
            action: current.defaultAction,
            ruleId: nil,
            reason: "Default policy action",
            auditRequired: current.defaultAction != .allow
        )
    }

    private func matches(rule: PollekRule, flow: FlowMetadata) -> Bool {
        if !rule.remoteHostSuffixes.isEmpty {
            guard let host = flow.remoteHostname?.lowercased() else { return false }
            let matched = rule.remoteHostSuffixes.contains { suffix in
                host.hasSuffix(suffix.lowercased())
            }
            if !matched { return false }
        }

        if !rule.remotePorts.isEmpty {
            guard let port = flow.remotePort else { return false }
            if !rule.remotePorts.contains(port) { return false }
        }

        if !rule.processBundleIds.isEmpty {
            guard let bundleId = flow.sourceAppIdentifier else { return false }
            if !rule.processBundleIds.contains(bundleId) { return false }
        }

        return true
    }
}
