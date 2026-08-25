//
//  WatchConnectivityManager.swift
//  bevy_watch Watch App
//
//  Receives the iPhone app's per-frame HUD snapshot (kill count + player
//  HP) via WCSession.updateApplicationContext, which — unlike
//  sendMessage — persists across restarts and is delivered even if this
//  app wasn't in the foreground, so raising the wrist always shows the
//  latest values rather than stale ones from whenever the app last woke
//  up. See the iOS side's WatchSessionManager.swift for the sender half of
//  this same contract (the two files share the "killCount"/"hp" key names
//  by convention, not a shared Swift type — there's no cross-target Swift
//  module boundary to share one across, same as the FFI boundary's own
//  "one getter per field" style in libbevy_in_app.h).
//

import Foundation
import WatchConnectivity

final class WatchConnectivityManager: NSObject, ObservableObject, WCSessionDelegate {
    static let shared = WatchConnectivityManager()

    // -1 = "no data yet" (matches native_menu::player_hp's own sentinel on
    // the Rust side) so the UI can show "--" instead of a misleading 0.
    @Published var hp: Double = -1
    @Published var killCount: Int = 0
    @Published var reachable: Bool = false

    private override init() {
        super.init()
        guard WCSession.isSupported() else { return }
        let session = WCSession.default
        session.delegate = self
        session.activate()
    }

    func session(_ session: WCSession, activationDidCompleteWith activationState: WCSessionActivationState, error: Error?) {
        DispatchQueue.main.async {
            self.reachable = session.isReachable
            self.apply(context: session.receivedApplicationContext)
        }
    }

    func sessionReachabilityDidChange(_ session: WCSession) {
        DispatchQueue.main.async {
            self.reachable = session.isReachable
        }
    }

    func session(_ session: WCSession, didReceiveApplicationContext applicationContext: [String: Any]) {
        DispatchQueue.main.async {
            self.apply(context: applicationContext)
        }
    }

    private func apply(context: [String: Any]) {
        if let hp = context["hp"] as? Double {
            self.hp = hp
        }
        if let killCount = context["killCount"] as? Int {
            self.killCount = killCount
        }
    }
}
