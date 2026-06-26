import Cocoa

@main
class AppDelegate: NSObject, NSApplicationDelegate {

    func applicationDidFinishLaunching(_ aNotification: Notification) {
        // Activate the System Extension on launch
        SystemExtensionInstaller.shared.activate()
    }

    func applicationWillTerminate(_ aNotification: Notification) {
        // Optional: Deactivate on terminate or keep it running? 
        // Typically, security products keep the extension running.
    }

    func applicationSupportsSecureRestorableState(_ app: NSApplication) -> Bool {
        return true
    }
}
