import NetworkExtension
import os.log
import Foundation

final class DataProvider: NEFilterDataProvider {
    private let logger = Logger(
        subsystem: "com.aecinfraconnect.pollek.dek.nefilter",
        category: "DataProvider"
    )

    private var policyEngine = PolicyEngine()

    override func startFilter(completionHandler: @escaping (Error?) -> Void) {
        logger.info("Starting Pollek DEK NEFilterDataProvider")
        policyEngine.reload()
        completionHandler(nil)
    }

    override func stopFilter(with reason: NEProviderStopReason, completionHandler: @escaping () -> Void) {
        logger.info("Stopping Pollek DEK NEFilterDataProvider: \(String(describing: reason))")
        completionHandler()
    }

    override func handleNewFlow(_ flow: NEFilterFlow) -> NEFilterNewFlowVerdict {
        let metadata = FlowMetadata.from(flow: flow)
        let decision = policyEngine.decide(flow: metadata)

        if decision.auditRequired {
            SharedStore.shared.writeAuditEvent(metadata, decision: decision)
        }

        switch decision.action {
        case .allow:
            logger.debug("Allow flow: \(metadata.remoteHostname ?? "unknown")")
            return .allow()

        case .block:
            logger.info("Block flow: \(metadata.remoteHostname ?? "unknown"), reason: \(decision.reason)")
            return .drop()

        case .needMoreRules:
            logger.debug("Need more rules for flow: \(metadata.remoteHostname ?? "unknown")")
            return .needRules()
        }
    }

    override func handleInboundDataFromFlow(
        _ flow: NEFilterFlow,
        readBytesStartOffset offset: Int,
        readBytes: Data
    ) -> NEFilterDataVerdict {
        // Keep content inspection minimal in v1.
        // Prefer metadata-level enforcement to reduce privacy and performance risk.
        return .allow()
    }

    override func handleOutboundDataFromFlow(
        _ flow: NEFilterFlow,
        readBytesStartOffset offset: Int,
        readBytes: Data
    ) -> NEFilterDataVerdict {
        // Optional: implement small signature checks here only if required.
        // Avoid remote calls, disk-heavy operations, or full payload logging.
        return .allow()
    }
}

private extension FlowMetadata {
    static func from(flow: NEFilterFlow) -> FlowMetadata {
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
