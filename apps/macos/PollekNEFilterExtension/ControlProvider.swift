import NetworkExtension
import os.log
import Foundation

final class ControlProvider: NEFilterControlProvider {
    private let logger = Logger(
        subsystem: "com.aecinfraconnect.pollek.dek.nefilter",
        category: "ControlProvider"
    )

    override func startFilter(completionHandler: @escaping (Error?) -> Void) {
        logger.info("Starting Pollek DEK NEFilterControlProvider")
        completionHandler(nil)
    }

    override func stopFilter(with reason: NEProviderStopReason, completionHandler: @escaping () -> Void) {
        logger.info("Stopping Pollek DEK NEFilterControlProvider: \(String(describing: reason))")
        completionHandler()
    }

    override func handleNewFlow(_ flow: NEFilterFlow, completionHandler: @escaping (NEFilterControlVerdict) -> Void) {
        let metadata = FlowMetadata.fromControlFlow(flow)

        // In production, this should query a local daemon through an allowed IPC channel
        // or use a pre-loaded signed policy snapshot.
        let decision = localFallbackDecision(metadata)

        SharedStore.shared.writeAuditEvent(metadata, decision: decision)

        switch decision.action {
        case .allow:
            completionHandler(.allow(withUpdateRules: false))
        case .block:
            completionHandler(.drop(withUpdateRules: false))
        case .needMoreRules:
            // Avoid infinite need-rules loops. Use conservative default.
            completionHandler(.drop(withUpdateRules: false))
        }
    }

    private func localFallbackDecision(_ flow: FlowMetadata) -> PolicyDecision {
        if let host = flow.remoteHostname?.lowercased(), host.hasSuffix(".webhook.site") {
            return PolicyDecision(
                action: .block,
                ruleId: "control-block-webhook-site",
                reason: "Blocked by ControlProvider fallback policy",
                auditRequired: true
            )
        }

        return PolicyDecision(
            action: .allow,
            ruleId: nil,
            reason: "Allowed by ControlProvider fallback policy",
            auditRequired: false
        )
    }
}

private extension FlowMetadata {
    static func fromControlFlow(_ flow: NEFilterFlow) -> FlowMetadata {
        var remoteHostname: String?
        var remoteAddress: String?
        var remotePort: Int?
        var direction = "unknown"

        if let socketFlow = flow as? NEFilterSocketFlow {
            direction = socketFlow.direction == .outbound ? "outbound" : "inbound"
            if let endpoint = socketFlow.remoteEndpoint as? NWHostEndpoint {
                remoteHostname = endpoint.hostname
                remoteAddress = endpoint.hostname
                remotePort = Int(endpoint.port)
            }
        }

        var sourceAppIdentifier: String?
        if let sourceApp = flow.sourceAppIdentifier {
            sourceAppIdentifier = String(data: sourceApp, encoding: .utf8)
        }

        return FlowMetadata(
            flowIdentifier: flow.identifier.uuidString,
            direction: direction,
            remoteHostname: remoteHostname,
            remoteAddress: remoteAddress,
            remotePort: remotePort,
            sourceAppIdentifier: sourceAppIdentifier,
            timestamp: Date()
        )
    }
}
