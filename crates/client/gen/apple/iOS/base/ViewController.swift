//
//  ViewController.swift
//  client_iOS_CADisplayLink
//
//  CADisplayLink-driven entry point for our real probe (crates/client's
//  cadisplaylink module) — no CoreMotion, no accelerometer, just the
//  camera+Sponza+touch-controls probe scene, same as the winit-driven
//  MINIMAL_MAP_PROBE, minus WinitPlugin.
//

import UIKit

class ViewController: UIViewController {
    @IBOutlet var metalV: MetalView!
    var bevyApp: OpaquePointer?
    // Bluetooth keyboard/mouse (GameController framework) — see
    // GameControllerInput.swift. Created after bevyApp so its "wire
    // already-connected devices" check in init() has a real app pointer
    // to send the initial GCMouseDidConnect-equivalent state to. Typed
    // Any? since GameControllerInput itself is @available(iOS 14, *) and
    // this property must be declarable on iOS 13 too (deployment target).
    private var gameControllerInput: Any?

    lazy var displayLink: CADisplayLink = {
        CADisplayLink.init(target: self, selector: #selector(enterFrame))
    }()

    override func viewDidLoad() {
        super.viewDidLoad()
        self.displayLink.add(to: .current, forMode: .default)
        self.displayLink.isPaused = true
        // AssistiveTouch's system pointer (needed on iPhone to make a
        // Bluetooth mouse usable at all — see GameControllerInput.swift's
        // own comment) stays on-screen by default even once GCMouse deltas
        // are driving the in-game camera, which looks wrong for an FPS.
        // UIPointerInteraction lets us tell UIKit to just not draw it
        // while it's hovering our view.
        if #available(iOS 13.4, *) {
            self.metalV.addInteraction(UIPointerInteraction(delegate: self))
        }
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        self.view.backgroundColor = .black
        if bevyApp == nil {
            self.createBevyApp()
        }
        self.displayLink.isPaused = false
    }

    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        displayLink.isPaused = true
    }

    func createBevyApp() {
        let viewPointer = Unmanaged.passUnretained(self.metalV).toOpaque()
        // Defaults to the real game. MINIMAL_MAP_PROBE=1 (pass via
        // `devicectl device process launch -e '{"MINIMAL_MAP_PROBE":"1"}'`)
        // switches to the stripped-down probe scene instead — same env var
        // name/purpose as the (now legacy) winit-driven app's own toggle,
        // for a consistent debugging workflow.
        if ProcessInfo.processInfo.environment["MINIMAL_MAP_PROBE"] != nil {
            bevyApp = cadisplaylink_create_app(viewPointer, Float(UIScreen.main.nativeScale))
        } else {
            bevyApp = cadisplaylink_create_game_app(viewPointer, Float(UIScreen.main.nativeScale))
        }
        if #available(iOS 15.0, *) {
            // Explicit ProMotion opt-in, mirrors MINIMAL_MAP_PROBE's own
            // CADisableMinimumFrameDurationOnPhone + WinitSettings::reactive
            // 120Hz config, for an apples-to-apples comparison.
            displayLink.preferredFrameRateRange = CAFrameRateRange(minimum: 80, maximum: 120, preferred: 120)
        }
        if #available(iOS 14.0, *) {
            gameControllerInput = GameControllerInput(viewController: self)
        }
    }

    @objc func enterFrame() {
        guard let bevy = self.bevyApp else { return }
        cadisplaylink_enter_frame(bevy)
    }

    // MARK: touch — every active UITouch is forwarded with its own stable
    // id (its own pointer address), so two simultaneous touches (joystick
    // drag + look drag) both keep working, unlike upstream bevy-in-app's
    // touches.first-only handling.
    private func touchId(for touch: UITouch) -> UInt64 {
        UInt64(UInt(bitPattern: Unmanaged.passUnretained(touch).toOpaque()))
    }

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let bevy = self.bevyApp else { return }
        for touch in touches {
            let location = touch.location(in: self.metalV)
            cadisplaylink_touch_started(bevy, touchId(for: touch), Float(location.x), Float(location.y))
        }
    }

    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let bevy = self.bevyApp else { return }
        for touch in touches {
            let location = touch.location(in: self.metalV)
            cadisplaylink_touch_moved(bevy, touchId(for: touch), Float(location.x), Float(location.y))
        }
    }

    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let bevy = self.bevyApp else { return }
        for touch in touches {
            let location = touch.location(in: self.metalV)
            cadisplaylink_touch_ended(bevy, touchId(for: touch), Float(location.x), Float(location.y))
        }
    }

    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let bevy = self.bevyApp else { return }
        for touch in touches {
            let location = touch.location(in: self.metalV)
            cadisplaylink_touch_cancelled(bevy, touchId(for: touch), Float(location.x), Float(location.y))
        }
    }

    deinit {
        if let bevy = bevyApp {
            cadisplaylink_release_app(bevy)
        }
    }

    // The actual "hide the system pointer and give the app raw relative
    // mouse deltas" API for games (WWDC20 "Bring keyboard and mouse gaming
    // to iPad") — UIPointerInteraction's .hidden() style above only hides
    // the hover-pointer while over a region, it doesn't stop the system
    // from drawing the AssistiveTouch pointer overlay on iPhone. This is
    // the real switch; requires the scene to stay full-screen and active,
    // which UIRequiresFullScreen (Info.plist) plus this being the app's
    // only screen already guarantee.
    @available(iOS 14.0, *)
    override var prefersPointerLocked: Bool {
        true
    }
}

@available(iOS 13.4, *)
extension ViewController: UIPointerInteractionDelegate {
    func pointerInteraction(_ interaction: UIPointerInteraction, styleFor region: UIPointerRegion) -> UIPointerStyle? {
        .hidden()
    }
}
