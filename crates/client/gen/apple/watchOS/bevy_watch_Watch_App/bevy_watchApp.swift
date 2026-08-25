//
//  bevy_watchApp.swift
//  bevy_watch Watch App
//
//  Apple Watch companion for the iOS game (bevy_in_iOS). Shows the local
//  player's current kill count and HP, kept in sync via WatchConnectivity
//  (see WatchConnectivityManager.swift) rather than talking to the Rust
//  game engine directly — the watch app never links libbevy_in_app.a, it's
//  a pure display surface for state the iPhone app already computes every
//  frame (ViewController.swift's syncHud, and the new syncWatch call next
//  to it).
//

import SwiftUI

@main
struct bevy_watchApp: App {
    @StateObject private var connectivity = WatchConnectivityManager.shared

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(connectivity)
        }
    }
}
