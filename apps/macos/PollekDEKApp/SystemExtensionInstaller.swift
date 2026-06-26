import Foundation
import SystemExtensions
import os.log

final class SystemExtensionInstaller: NSObject {
    static let shared = SystemExtensionInstaller()

    private let logger = Logger(
        subsystem: "com.aecinfraconnect.pollek.dek",
        category: "SystemExtensionInstaller"
    )

    private let extensionBundleIdentifier = "com.aecinfraconnect.pollek.dek.nefilter"

    func activate() {
        let request = OSSystemExtensionRequest.activationRequest(
            forExtensionWithIdentifier: extensionBundleIdentifier,
            queue: .main
        )
        request.delegate = self
        OSSystemExtensionManager.shared.submitRequest(request)
    }

    func deactivate() {
        let request = OSSystemExtensionRequest.deactivationRequest(
            forExtensionWithIdentifier: extensionBundleIdentifier,
            queue: .main
        )
        request.delegate = self
        OSSystemExtensionManager.shared.submitRequest(request)
    }
}

extension SystemExtensionInstaller: OSSystemExtensionRequestDelegate {
    func requestNeedsUserApproval(_ request: OSSystemExtensionRequest) {
        logger.info("System Extension requires user approval in System Settings")
    }

    func request(
        _ request: OSSystemExtensionRequest,
        actionForReplacingExtension existing: OSSystemExtensionProperties,
        withExtension ext: OSSystemExtensionProperties
    ) -> OSSystemExtensionRequest.ReplacementAction {
        logger.info("Replacing System Extension \(existing.bundleVersion) with \(ext.bundleVersion)")
        return .replace
    }

    func request(_ request: OSSystemExtensionRequest, didFinishWithResult result: OSSystemExtensionRequest.Result) {
        logger.info("System Extension activation finished: \(String(describing: result))")
    }

    func request(_ request: OSSystemExtensionRequest, didFailWithError error: Error) {
        logger.error("System Extension request failed: \(error.localizedDescription)")
    }
}
