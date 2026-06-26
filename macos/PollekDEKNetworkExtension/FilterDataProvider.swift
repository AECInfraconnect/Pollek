// macos/PollekDEKNetworkExtension/FilterDataProvider.swift
import NetworkExtension

class FilterDataProvider: NEFilterDataProvider {
    // rule ที่ push มาจาก container app ผ่าน IPC
    private var blockedDomains: Set<String> = []
    private var blockedPorts: Set<UInt16> = []

    override func startFilter(completionHandler: @escaping (Error?) -> Void) {
        // listen UDS /var/run/pollek/nefilter.sock รับ rule จาก container
        startIpcListener()
        completionHandler(nil)
    }

    // ตัดสินทุก flow ใหม่ — allow หรือ drop (research: filter ให้ verdict ไม่ modify)
    override func handleNewFlow(_ flow: NEFilterFlow) -> NEFilterNewFlowVerdict {
        guard let socketFlow = flow as? NEFilterSocketFlow,
              let endpoint = socketFlow.remoteEndpoint as? NWHostEndpoint else {
            return .allow()
        }
        let host = endpoint.hostname
        let port = UInt16(endpoint.port) ?? 0

        // emit telemetry ทุก decision → เขียนลง shared spool / XPC
        let blocked = blockedDomains.contains(where: { host.contains($0) })
                      || blockedPorts.contains(port)
        emitTelemetry(host: host, port: port, decision: blocked ? "block" : "allow")

        return blocked ? .drop() : .allow()
    }
    
    private func startIpcListener() {
        // Stub: Implement Unix Domain Socket listener for /var/run/pollek/nefilter.sock
        // to receive rules from NeFilterClient in dek-core.
    }
    
    private func emitTelemetry(host: String, port: UInt16, decision: String) {
        // Stub: Write decision log to XPC or shared IPC to spool it.
    }
}
