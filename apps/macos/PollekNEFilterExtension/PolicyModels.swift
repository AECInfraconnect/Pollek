import Foundation

enum PollekVerdictAction: String, Codable {
    case allow
    case block
    case needMoreRules
}

struct PollekPolicyBundle: Codable {
    let version: String
    let generatedAt: Date
    let defaultAction: PollekVerdictAction
    let rules: [PollekRule]
}

struct PollekRule: Codable {
    let id: String
    let action: PollekVerdictAction
    let remoteHostSuffixes: [String]
    let remotePorts: [Int]
    let processBundleIds: [String]
    let reason: String
}

struct FlowMetadata: Codable {
    let flowIdentifier: String
    let direction: String
    let remoteHostname: String?
    let remoteAddress: String?
    let remotePort: Int?
    let sourceAppIdentifier: String?
    let timestamp: Date
}

struct PolicyDecision: Codable {
    let action: PollekVerdictAction
    let ruleId: String?
    let reason: String
    let auditRequired: Bool
}
