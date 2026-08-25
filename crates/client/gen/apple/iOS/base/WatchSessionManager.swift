//
//  WatchSessionManager.swift
//  bevy_in_iOS
//
//  Sends this client's per-frame kill count + player HP to the Watch
//  companion app (see gen/apple/watchOS/bevy_watch_Watch_App's
//  WatchConnectivityManager.swift, the receiving half of this same
//  "killCount"/"hp" dictionary contract). Uses updateApplicationContext
//  rather than sendMessage: it's fire-and-forget, coalesces automatically
//  (only the latest context is ever delivered), and — unlike
//  sendMessage — doesn't require the Watch app to be reachable right now,
//  so background sync (the "auto-launch" ask, scoped down to "always have
//  fresh data waiting" rather than force-opening the Watch UI, which
//  WatchConnectivity has no public API for) works even before the user
//  has ever opened the Watch app.
//
//  Also owns the "you got a kill" local notification: fired once per
//  actual kill-count increase (never on the initial sync, so installing
//  the app / reconnecting doesn't spam one), and mirrored to the Watch
//  automatically by the system — no separate Watch-side notification
//  handling needed for that half.
//

import Foundation
import WatchConnectivity
import UserNotifications

final class WatchSessionManager: NSObject, WCSessionDelegate {
    static let shared = WatchSessionManager()

    // nil until the first real syncHud() call — distinguishes "never
    // synced yet" from "synced at 0 kills", so a fresh app launch never
    // fires a bogus "first kill" notification for a value that was
    // already 0.
    private var lastKillCount: Int?

    private override init() {
        super.init()
        guard WCSession.isSupported() else { return }
        WCSession.default.delegate = self
        WCSession.default.activate()
        UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound]) { _, _ in }
    }

    func session(_ session: WCSession, activationDidCompleteWith activationState: WCSessionActivationState, error: Error?) {}

    #if os(iOS)
    func sessionDidBecomeInactive(_ session: WCSession) {}
    func sessionDidDeactivate(_ session: WCSession) {
        session.activate()
    }
    #endif

    // Called once per frame from ViewController's syncHud (same cadence
    // as the on-screen HUD read) — updateApplicationContext is cheap to
    // call every frame since WatchConnectivity itself coalesces to "only
    // the latest wins", but throttling here would just add complexity for
    // no real benefit at this call frequency.
    func update(hp: Float, killCount: UInt32) {
        guard WCSession.isSupported(), WCSession.default.activationState == .activated else { return }
        let context: [String: Any] = ["hp": Double(hp), "killCount": Int(killCount)]
        try? WCSession.default.updateApplicationContext(context)

        let killCount = Int(killCount)
        if let last = lastKillCount, killCount > last {
            notifyKill(totalKills: killCount)
        }
        lastKillCount = killCount
    }

    private func notifyKill(totalKills: Int) {
        let content = UNMutableNotificationContent()
        content.title = "Zombie down!"
        content.body = "Total kills: \(totalKills)"
        content.sound = .default
        let request = UNNotificationRequest(identifier: "kill-\(totalKills)", content: content, trigger: nil)
        UNUserNotificationCenter.current().add(request)
    }
}
