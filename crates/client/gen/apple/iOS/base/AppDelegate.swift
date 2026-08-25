//
//  AppDelegate.swift
//  bevy_in_iOS
//
//  Created by Jinlei Li on 2022/12/20.
//

import UIKit

@main
class AppDelegate: UIResponder, UIApplicationDelegate {
    var window: UIWindow?
        
    func application(_ application: UIApplication, didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?) -> Bool {
        window = UIWindow(frame: UIScreen.main.bounds)
        let mainStroryBoard = UIStoryboard(name: "Main", bundle: nil)
        window?.rootViewController = mainStroryBoard.instantiateInitialViewController()

        window?.makeKeyAndVisible()

        // Activate the Watch session (and ask for notification permission)
        // as soon as the app launches, rather than waiting for the first
        // syncHud() call — so the Watch companion app already has fresh
        // data the moment the game starts, matching the "always synced in
        // the background" behavior WatchSessionManager itself provides.
        _ = WatchSessionManager.shared

        return true
    }

}

