// Postal's mailto: handler on macOS: only an app receives a link from the
// system. It hands each link to `postal open-in-terminal` as one argument,
// through no shell; postal picks the terminal. See docs/adr/0034.
import AppKit

let defaults = UserDefaults.standard
let mailto = URL(string: "mailto:")!

final class Opener: NSObject, NSApplicationDelegate {
    private var quitting: Timer?

    // Installed before launch ends, so the link that launched the app arrives.
    func applicationWillFinishLaunching(_: Notification) {
        NSAppleEventManager.shared().setEventHandler(
            self, andSelector: #selector(open(_:reply:)),
            forEventClass: AEEventClass(kInternetEventClass), andEventID: AEEventID(kAEGetURL))
        linger()
    }

    // The link as the event carries it: a URL would re-encode what it holds.
    @objc func open(_ event: NSAppleEventDescriptor, reply _: NSAppleEventDescriptor) {
        defer { linger() }
        guard let link = event.paramDescriptor(forKeyword: keyDirectObject)?.stringValue,
            let postal = defaults.string(forKey: "postal")
        else {
            NSLog("postal: a link came, but `postal setup-mailto` named no postal")
            return
        }

        let process = Process()
        process.executableURL = URL(fileURLWithPath: postal)
        process.arguments = ["open-in-terminal", link]
        do {
            try process.run()
            process.waitUntilExit()
        } catch {
            NSLog("postal: cannot run \(postal): \(error)")
        }
    }

    // Quits once no link has come for a while, as links arrive one event each.
    private func linger() {
        quitting?.invalidate()
        quitting = Timer.scheduledTimer(withTimeInterval: 2, repeats: false) { _ in
            NSApp.terminate(nil)
        }
    }
}

/// Makes this app the handler, `postal` the binary it runs, and keeps the
/// handler it replaces for `unregister`.
func register(postal: String) -> Int32 {
    let app = Bundle.main.bundleURL
    let previous = NSWorkspace.shared.urlForApplication(toOpen: mailto)
    if let previous, previous.standardizedFileURL != app.standardizedFileURL {
        defaults.set(previous.path, forKey: "previous")
    }

    defaults.set(postal, forKey: "postal")
    LSRegisterURL(app as CFURL, true)
    return setDefault(app)
}

/// Gives the links back to the handler `register` replaced, Mail without one.
func unregister() -> Int32 {
    let previous = defaults.string(forKey: "previous").map { URL(fileURLWithPath: $0) }
    let mail = NSWorkspace.shared.urlForApplication(withBundleIdentifier: "com.apple.mail")
    let status = (previous ?? mail).map(setDefault) ?? 0
    defaults.removeObject(forKey: "postal")
    defaults.removeObject(forKey: "previous")
    return status
}

// The main run loop turns while waiting: the answer may come on it.
func setDefault(_ app: URL) -> Int32 {
    var status: Int32?
    NSWorkspace.shared.setDefaultApplication(at: app, toOpenURLsWithScheme: "mailto") { error in
        if let error {
            FileHandle.standardError.write(Data("\(error.localizedDescription)\n".utf8))
        }
        DispatchQueue.main.async { status = error == nil ? 0 : 1 }
    }
    while status == nil {
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.05))
    }
    return status!
}

let arguments = CommandLine.arguments.dropFirst()
switch (arguments.first, arguments.count) {
case ("register", 2): exit(register(postal: arguments.last!))
case ("unregister", 1): exit(unregister())
default:
    let app = NSApplication.shared
    let opener = Opener()
    app.delegate = opener
    app.setActivationPolicy(.prohibited)
    app.run()
}
