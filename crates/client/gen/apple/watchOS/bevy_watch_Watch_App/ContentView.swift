//
//  ContentView.swift
//  bevy_watch Watch App
//
//  Minimal always-useful glance: kill count front and center (this is
//  the stat a player actually checks mid-match) with player HP below it,
//  colored to read as a danger cue at a glance the same way the iPhone
//  HUD's own health readout does (see TouchHUD.swift's health coloring).
//

import SwiftUI

struct ContentView: View {
    @EnvironmentObject var connectivity: WatchConnectivityManager

    private var hpColor: Color {
        guard connectivity.hp >= 0 else { return .gray }
        switch connectivity.hp {
        case ..<25: return .red
        case ..<50: return .orange
        default: return .green
        }
    }

    private var hpText: String {
        connectivity.hp >= 0 ? "\(Int(connectivity.hp.rounded()))" : "--"
    }

    var body: some View {
        VStack(spacing: 8) {
            VStack(spacing: 0) {
                Text("\(connectivity.killCount)")
                    .font(.system(size: 46, weight: .bold, design: .rounded))
                Text("KILLS")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }

            HStack(spacing: 4) {
                Image(systemName: "heart.fill")
                    .foregroundStyle(hpColor)
                Text("\(hpText) HP")
                    .font(.headline)
                    .foregroundStyle(hpColor)
            }

            if !connectivity.reachable {
                Text("Waiting for iPhone…")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
        }
        .padding()
    }
}

#Preview {
    ContentView().environmentObject(WatchConnectivityManager.shared)
}
