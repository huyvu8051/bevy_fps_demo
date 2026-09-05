//
//  ViewController.swift
//  client_iOS_CADisplayLink
//
//  CADisplayLink-driven entry point for the real game's Rust+Metal
//  renderer (crates/client's cadisplaylink module) — no CoreMotion, no
//  accelerometer, no WinitPlugin. The pure-Swift classic-Metal demo scene
//  (a hand-drawn Sponza/box-arena room, once used as a control experiment
//  for measuring wgpu's own CPU encoding cost against a "real" Metal app
//  doing the same geometry) has been removed — that question was answered
//  long ago (see README.md's own "Build & chạy trên iOS" section) and the
//  demo scene had no path left to ever become reachable (USE_NATIVE_METAL_
//  DEMO was permanently false); see git history if it's ever worth
//  resurrecting.
//

import UIKit
import QuartzCore

// The Rust+Metal renderer (`rustMetalProbe`) draws the real game's own
// level (spawn_test_arena's walls+floor, the ak47_scope gun), camera
// driven by the game's own FpsCamera — see native_metal_probe_create_
// real_game in ffi.rs.

private extension Comparable {
    func clamped(to range: ClosedRange<Self>) -> Self {
        min(max(self, range.lowerBound), range.upperBound)
    }
}

// Phase 4 of the wgpu-replacement plan: HUD as plain UIKit views layered
// over metalV, instead of reimplementing bevy_ui's text/layout engine —
// presentation-only, no gameplay logic lives here.
//
// The joystick + all 10 touch-control buttons (fire/jump/aim/crouch/
// sprint/reload/menu/lean×2/prone) that used to live here as native UIKit
// controls have been REMOVED — they're now real bevy_ui nodes
// (TouchJoystickBase/Knob, TouchFireButton, ... — crates/client/src/lib.rs)
// drawn directly by the cadisplaylink Metal renderer (see
// native_metal_probe.rs's touch-control draw loop) and driven by Bevy's
// own touch-handling systems, reading the exact same forwarded
// `native_metal_probe_touch_*` events ViewController already sends every
// frame (see touchesBegan/Moved/Ended/Cancelled below) — no native
// hit-testing or key/mouse-event synthesis needed for them anymore. This
// was a genuine duplicate input path (native Swift hit-test + FFI key/
// mouse synthesis running ALONGSIDE bevy_ui's own Interaction-based touch
// handling, both driven by the same underlying touches) — removing the
// native half is what actually achieves one consistent control path across
// desktop and iOS, not just a second copy of the same buttons.
//
// HP/ammo/ping text readouts + crosshair used to stay native here (no
// bevy_ui/Metal-renderer equivalent yet) — now that native_metal_probe.rs
// draws HealthUi/AmmoUi/PingUi/CrosshairLine directly (same real ECS state
// the desktop/wgpu path renders, see HudTextSnapshot/CrosshairLineSnapshot
// in native_game_extract.rs), the `TouchHUD` class that used to hold this
// is gone entirely — nothing native-only is left in this file.
class ViewController: UIViewController {
    @IBOutlet var metalV: MetalView!

    // Not `private`: GameControllerInput.swift (Bluetooth keyboard/mouse)
    // reads this the same way it already reads `bevyApp`.
    var rustMetalProbe: OpaquePointer?
    // Left half of screen = move (drag vector -> forward/strafe), right
    // half = look (drag delta -> yaw/pitch), tracked by touch identity.
    private var moveTouch: UITouch?
    private var moveTouchStart: CGPoint = .zero
    private var moveTouchCurrent: CGPoint = .zero
    private var lookTouch: UITouch?
    private var lookTouchLast: CGPoint = .zero
    private var lastFrameTime: CFTimeInterval = CACurrentMediaTime()
    // Bluetooth keyboard/mouse (GameController framework) — see
    // GameControllerInput.swift. Created after rustMetalProbe so its "wire
    // already-connected devices" check in init() has a real probe pointer
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

    // Polled once per frame (see enterFrame) — HP/ammo/ping/crosshair are
    // now drawn straight off real ECS state by native_metal_probe.rs (see
    // ViewController's own class-level doc comment), so this only still
    // exists for the Apple Watch companion sync below.
    private func syncHud() {
        guard let probe = self.rustMetalProbe else { return }
        let hp = native_metal_probe_get_player_hp(probe)

        // Apple Watch companion app sync — see WatchSessionManager's own
        // doc comment for why this runs every frame alongside the rest of
        // the HUD instead of on its own timer.
        let killCount = native_metal_probe_get_kill_count(probe)
        WatchSessionManager.shared.update(hp: hp, killCount: killCount)
    }

    // Safe to call more than once — the rustMetalProbe == nil guard makes
    // every call after the first a no-op. Only ever called once now (from
    // viewDidAppear, unconditionally — see its own doc comment), but kept
    // as its own function since createGameProbeIfNeeded's own set of
    // one-time side effects (frame-rate range, GameControllerInput) reads
    // more clearly named than inlined.
    private func createGameProbeIfNeeded() {
        guard rustMetalProbe == nil else { return }
        let viewPointer = Unmanaged.passUnretained(self.metalV).toOpaque()
        // The renderer's own standalone Sponza/box-arena/patrol demo World
        // (native_metal_probe_create, real_game=false) was removed from the
        // Rust side once this real-game path became the only one ever
        // built — see git history if a standalone demo entry point is ever
        // worth resurrecting.
        rustMetalProbe = native_metal_probe_create_real_game(viewPointer, Float(UIScreen.main.nativeScale))
        if #available(iOS 15.0, *) {
            displayLink.preferredFrameRateRange = CAFrameRateRange(minimum: 80, maximum: 120, preferred: 120)
        }
        if #available(iOS 14.0, *), gameControllerInput == nil {
            gameControllerInput = GameControllerInput(viewController: self)
        }
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        // Keeps the screen from auto-locking/dimming during gameplay —
        // ViewController is the app's only screen (UIRequiresFullScreen,
        // Info.plist), so there's no other view to restore this for on
        // exit; set once here for the whole process lifetime. Doesn't
        // block the user manually pressing the power button, only the
        // idle-timeout auto-lock.
        UIApplication.shared.isIdleTimerDisabled = true
        self.view.backgroundColor = .black
        // Always creates the probe (and the real headless game App inside
        // it) immediately, whether or not MAP_ID is set yet —
        // MapSelectionPending (crate::lib) holds off connect_to_server and
        // shows a real bevy_ui map-select screen, drawn right here by
        // native_metal_probe.rs's own draw_panel, until map_select_button
        // picks one. No native map-select view left to gate this on
        // anymore.
        createGameProbeIfNeeded()
        self.displayLink.isPaused = false
    }

    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        displayLink.isPaused = true
    }

    @objc func enterFrame() {
        guard let probe = self.rustMetalProbe else { return }
        let now = CACurrentMediaTime()
        let deltaTime = Float(now - lastFrameTime)
        lastFrameTime = now

        var lookDx: Float = 0
        var lookDy: Float = 0
        if let lookTouch = self.lookTouch {
            let current = lookTouch.location(in: self.metalV)
            lookDx = Float(current.x - lookTouchLast.x)
            lookDy = Float(current.y - lookTouchLast.y)
            lookTouchLast = current
        }
        var forward: Float = 0
        var strafe: Float = 0
        if self.moveTouch != nil {
            let dx = Float(moveTouchCurrent.x - moveTouchStart.x)
            let dy = Float(moveTouchCurrent.y - moveTouchStart.y)
            let maxDrag: Float = 60.0
            strafe = (dx / maxDrag).clamped(to: -1...1)
            forward = (-dy / maxDrag).clamped(to: -1...1)
        }
        native_metal_probe_update_camera(probe, forward, strafe, lookDx, lookDy, deltaTime)
        native_metal_probe_render_frame(probe)
        syncHud()
    }

    // MARK: touch — every active UITouch is forwarded with its own stable
    // id (its own pointer address), so two simultaneous touches (joystick
    // drag + look drag) both keep working, unlike upstream bevy-in-app's
    // touches.first-only handling.
    private func touchId(for touch: UITouch) -> UInt64 {
        UInt64(UInt(bitPattern: Unmanaged.passUnretained(touch).toOpaque()))
    }

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        // Phase 6 Step 3: in real-game-renderer mode, ALSO forward every
        // touch to the real game's own headless App (native_metal_probe_
        // touch_started) so its own touch_move_axis/touch_look systems
        // drive movement/look — alongside (not instead of) the existing
        // moveTouch/lookTouch tracking below, which render_frame's camera
        // pose gets overwritten from the real FpsCamera anyway once one
        // exists (see native_metal_probe.rs's render_frame).
        if let probe = self.rustMetalProbe {
            for touch in touches {
                let location = touch.location(in: self.metalV)
                native_metal_probe_touch_started(probe, touchId(for: touch), Float(location.x), Float(location.y))
            }
        }
        for touch in touches {
            let location = touch.location(in: self.metalV)
            let isLeftHalf = location.x < self.metalV.bounds.width / 2.0
            if isLeftHalf, moveTouch == nil {
                moveTouch = touch
                moveTouchStart = location
                moveTouchCurrent = location
            } else if !isLeftHalf, lookTouch == nil {
                lookTouch = touch
                lookTouchLast = location
            }
        }
    }

    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        if let probe = self.rustMetalProbe {
            for touch in touches {
                let location = touch.location(in: self.metalV)
                native_metal_probe_touch_moved(probe, touchId(for: touch), Float(location.x), Float(location.y))
            }
        }
        for touch in touches {
            if touch == moveTouch {
                moveTouchCurrent = touch.location(in: self.metalV)
            }
            // lookTouchLast is updated in updateCamera() each frame
            // from the touch's live location, not here.
        }
    }

    private func endNativeTouch(_ touches: Set<UITouch>) {
        for touch in touches {
            if touch == moveTouch {
                moveTouch = nil
            }
            if touch == lookTouch {
                lookTouch = nil
            }
        }
    }

    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        if let probe = self.rustMetalProbe {
            for touch in touches {
                let location = touch.location(in: self.metalV)
                native_metal_probe_touch_ended(probe, touchId(for: touch), Float(location.x), Float(location.y))
            }
        }
        endNativeTouch(touches)
    }

    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        if let probe = self.rustMetalProbe {
            for touch in touches {
                let location = touch.location(in: self.metalV)
                native_metal_probe_touch_cancelled(probe, touchId(for: touch), Float(location.x), Float(location.y))
            }
        }
        endNativeTouch(touches)
    }

    deinit {
        if let probe = rustMetalProbe {
            native_metal_probe_release(probe)
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
