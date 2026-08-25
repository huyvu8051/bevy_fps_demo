//
//  GameControllerInput.swift
//  client_iOS_CADisplayLink
//
//  Bluetooth/hardware keyboard + mouse support via Apple's GameController
//  framework (GCKeyboard/GCMouse) — the standard way to get hardware
//  keyboard/mouse events in a custom Metal/CADisplayLink app that doesn't
//  go through UIKit's normal responder chain for this (README previously
//  called this out as a known gap: "Chuột/bàn phím Bluetooth không điều
//  khiển được game trên iOS"). Same FFI-bridge shape as ViewController's
//  own touch handling: capture the native event in Swift, forward it to
//  Rust/Bevy through the C functions declared in libbevy_in_app.h.
//

import GameController
import UIKit

// GCKeyboardInput/GCMouse require iOS 14+; the app's own deployment target
// is 13.0 (see IPHONEOS_DEPLOYMENT_TARGET in project.pbxproj), so this
// whole class is gated — ViewController only constructs it behind an
// #available(iOS 14.0, *) check.
@available(iOS 14.0, *)
final class GameControllerInput {
    private weak var viewController: ViewController?
    private let notificationCenter = NotificationCenter.default

    init(viewController: ViewController) {
        self.viewController = viewController
        notificationCenter.addObserver(self, selector: #selector(keyboardConnected), name: .GCKeyboardDidConnect, object: nil)
        notificationCenter.addObserver(self, selector: #selector(mouseConnected), name: .GCMouseDidConnect, object: nil)
        notificationCenter.addObserver(self, selector: #selector(mouseDisconnected), name: .GCMouseDidDisconnect, object: nil)
        // In case a keyboard/mouse was already connected before this
        // object was created (e.g. app relaunched while a Bluetooth
        // keyboard was already paired).
        if let keyboardInput = GCKeyboard.coalesced?.keyboardInput {
            wire(keyboard: keyboardInput)
        }
        if let mouse = GCMouse.current {
            wire(mouse: mouse)
        }
    }

    @objc private func keyboardConnected(_ notification: Notification) {
        guard let keyboard = notification.object as? GCKeyboard, let keyboardInput = keyboard.keyboardInput else { return }
        wire(keyboard: keyboardInput)
    }

    @objc private func mouseConnected(_ notification: Notification) {
        guard let mouse = notification.object as? GCMouse else { return }
        wire(mouse: mouse)
    }

    @objc private func mouseDisconnected(_ notification: Notification) {
        Self.mouseConnected(on: viewController, false)
    }

    private func wire(keyboard: GCKeyboardInput) {
        keyboard.keyChangedHandler = { [weak self] _, _, keyCode, pressed in
            guard let keyId = Self.keyId(for: keyCode) else { return }
            Self.keyChanged(on: self?.viewController, keyId, pressed)
        }
    }

    private func wire(mouse: GCMouse) {
        Self.mouseConnected(on: viewController, true)
        guard let input = mouse.mouseInput else { return }
        input.mouseMovedHandler = { [weak self] _, deltaX, deltaY in
            // GCMouseInput reports +Y as up; Bevy's MouseMotion (like
            // desktop mouse deltas generally) expects +Y as down, same
            // sign flip touch_look/mouse_look already assume for drag
            // deltas — flip here so look direction matches touch/desktop.
            Self.mouseMoved(on: self?.viewController, deltaX, -deltaY)
        }
        input.leftButton.pressedChangedHandler = { [weak self] _, _, pressed in
            Self.mouseButtonChanged(on: self?.viewController, 0, pressed)
        }
        input.rightButton?.pressedChangedHandler = { [weak self] _, _, pressed in
            Self.mouseButtonChanged(on: self?.viewController, 1, pressed)
        }
        input.middleButton?.pressedChangedHandler = { [weak self] _, _, pressed in
            Self.mouseButtonChanged(on: self?.viewController, 2, pressed)
        }
    }

    // Dispatch to the active native Metal probe (`rustMetalProbe` — both
    // the plain probe demo and, when USE_REAL_GAME_NATIVE_RENDERER is set,
    // the real game's own headless App via native_metal_probe_*). The old
    // bevy_render-on-CADisplayLink fallback (`bevyApp`) was removed —
    // dead code once USE_RUST_METAL_DEMO (which gated it) was always true.
    private static func keyChanged(on vc: ViewController?, _ keyId: UInt16, _ pressed: Bool) {
        if let probe = vc?.rustMetalProbe {
            native_metal_probe_key_changed(probe, keyId, pressed)
        }
    }

    private static func mouseConnected(on vc: ViewController?, _ connected: Bool) {
        if let probe = vc?.rustMetalProbe {
            native_metal_probe_mouse_connected(probe, connected)
        }
    }

    private static func mouseMoved(on vc: ViewController?, _ deltaX: Float, _ deltaY: Float) {
        if let probe = vc?.rustMetalProbe {
            native_metal_probe_mouse_moved(probe, deltaX, deltaY)
        }
    }

    private static func mouseButtonChanged(on vc: ViewController?, _ button: UInt8, _ pressed: Bool) {
        if let probe = vc?.rustMetalProbe {
            native_metal_probe_mouse_button_changed(probe, button, pressed)
        }
    }

    // Maps the handful of GCKeyCodes this game actually reads (see
    // crates/client/src/lib.rs's KeyCode:: usages) to the small numbering
    // map_key_id in crates/client/src/cadisplaylink/ffi.rs expects:
    // letters 0-25 (A-Z), digits 26-35 (0-9), then named keys — deliberately
    // NOT GCKeyCode's own rawValue, so this file is the one place both
    // sides' numbering has to agree.
    private static func keyId(for code: GCKeyCode) -> UInt16? {
        switch code {
        case .keyA: return 0
        case .keyB: return 1
        case .keyC: return 2
        case .keyD: return 3
        case .keyE: return 4
        case .keyF: return 5
        case .keyG: return 6
        case .keyH: return 7
        case .keyI: return 8
        case .keyJ: return 9
        case .keyK: return 10
        case .keyL: return 11
        case .keyM: return 12
        case .keyN: return 13
        case .keyO: return 14
        case .keyP: return 15
        case .keyQ: return 16
        case .keyR: return 17
        case .keyS: return 18
        case .keyT: return 19
        case .keyU: return 20
        case .keyV: return 21
        case .keyW: return 22
        case .keyX: return 23
        case .keyY: return 24
        case .keyZ: return 25
        case .zero: return 26
        case .one: return 27
        case .two: return 28
        case .three: return 29
        case .four: return 30
        case .five: return 31
        case .six: return 32
        case .seven: return 33
        case .eight: return 34
        case .nine: return 35
        case .spacebar: return 36
        case .escape: return 37
        case .leftShift: return 38
        case .rightShift: return 39
        case .leftControl: return 40
        case .rightControl: return 41
        case .leftAlt: return 42
        case .rightAlt: return 43
        case .openBracket: return 44
        case .closeBracket: return 45
        case .F1: return 46
        case .tab: return 47
        case .returnOrEnter: return 48
        case .upArrow: return 49
        case .downArrow: return 50
        case .leftArrow: return 51
        case .rightArrow: return 52
        default: return nil
        }
    }

    deinit {
        notificationCenter.removeObserver(self)
    }
}
