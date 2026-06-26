import Foundation
import os.log

final class SharedStore {
    static let shared = SharedStore()

    private let appGroupId = "group.com.aecinfraconnect.pollek.dek"
    private let policyFileName = "pollek-policy-bundle.json"

    private let logger = Logger(
        subsystem: "com.aecinfraconnect.pollek.dek.nefilter",
        category: "SharedStore"
    )

    private var containerURL: URL? {
        FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: appGroupId)
    }

    func loadPolicyBundle() -> PollekPolicyBundle? {
        guard let url = containerURL?.appendingPathComponent(policyFileName) else {
            logger.error("App Group container is unavailable")
            return nil
        }

        do {
            let data = try Data(contentsOf: url)
            let decoder = JSONDecoder()
            decoder.dateDecodingStrategy = .iso8601
            return try decoder.decode(PollekPolicyBundle.self, from: data)
        } catch {
            logger.error("Failed to load policy bundle: \(error.localizedDescription)")
            return nil
        }
    }

    func writeAuditEvent(_ event: FlowMetadata, decision: PolicyDecision) {
        guard let url = containerURL?.appendingPathComponent("audit-events.ndjson") else {
            return
        }

        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601

        struct AuditEnvelope: Codable {
            let event: FlowMetadata
            let decision: PolicyDecision
        }

        do {
            let data = try encoder.encode(AuditEnvelope(event: event, decision: decision))
            var line = data
            line.append(0x0A) // newline

            if FileManager.default.fileExists(atPath: url.path) {
                let handle = try FileHandle(forWritingTo: url)
                try handle.seekToEnd()
                try handle.write(contentsOf: line)
                try handle.close()
            } else {
                try line.write(to: url, options: .atomic)
            }
        } catch {
            logger.error("Failed to write audit event: \(error.localizedDescription)")
        }
    }
}
