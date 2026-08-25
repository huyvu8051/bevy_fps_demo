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
import Metal
import QuartzCore
import simd
import ModelIO
import MetalKit

// Control experiment for the Bevy/wgpu "high CPU encoding cost" Metal HUD
// warning: a pure Swift + classic Metal (MTLCommandQueue/
// MTLRenderCommandEncoder, no wgpu, no Bevy, no Rust at all) render loop
// that draws the same 5-cuboid room (1 floor + 4 walls) as fps_probe.rs's
// original scene, with the same dimensions/camera. If this ALSO trips the
// warning, the cost is inherent to doing this little geometry on this
// device/OS. If it doesn't, that confirms the cost really is Bevy/wgpu-
// specific overhead, not something a "real" Metal app pays for the same
// scene.
let USE_NATIVE_METAL_DEMO = false
// Phase 6 Step 1 feasibility spike (temporary): boots the REAL game's
// gameplay App (physics/networking/AI/prediction) with no window/render
// backend at all and ticks it repeatedly, printing zombie/player counts —
// see native_game_app_create/tick in cadisplaylink/ffi.rs. No renderer
// reads from it yet. Remove once Phase 6 Step 1 is confirmed working.
let RUN_HEADLESS_GAME_APP_SPIKE = false
// Phase 6 Step 4: when true, the Rust+Metal probe (`rustMetalProbe`,
// always the active renderer now — see the retired USE_RUST_METAL_DEMO
// flag's removal) draws the REAL game's own level (spawn_test_arena's
// walls+floor, the ak47_scope gun) instead of its own Sponza/box-arena
// demo scene, with the camera driven by the game's own FpsCamera instead
// of touch input — see native_metal_probe_create_real_game in ffi.rs.
// Separate from RUN_HEADLESS_GAME_APP_SPIKE (that one boots its OWN
// headless App with no renderer at all; this one boots the renderer WITH
// a headless App wired in) — only one of the two should be true at once,
// they'd otherwise run two independent, un-networked copies of the game.
let USE_REAL_GAME_NATIVE_RENDERER = true

private let arenaHalfSize: Float = 10.0
private let arenaWallHeight: Float = 4.0
private let arenaWallThickness: Float = 0.3

private struct NativeVertex {
    var position: SIMD3<Float>
    var normal: SIMD3<Float>
    var color: SIMD3<Float>
}

// Appends 36 flat-shaded vertices (6 faces * 2 triangles * 3 verts) for an
// axis-aligned box centered at `center` with the given half-extents.
private func appendBox(_ verts: inout [NativeVertex], center: SIMD3<Float>, half: SIMD3<Float>, color: SIMD3<Float>) {
    let x = half.x, y = half.y, z = half.z
    // 8 corners
    let c000 = center + SIMD3<Float>(-x, -y, -z)
    let c001 = center + SIMD3<Float>(-x, -y, z)
    let c010 = center + SIMD3<Float>(-x, y, -z)
    let c011 = center + SIMD3<Float>(-x, y, z)
    let c100 = center + SIMD3<Float>(x, -y, -z)
    let c101 = center + SIMD3<Float>(x, -y, z)
    let c110 = center + SIMD3<Float>(x, y, -z)
    let c111 = center + SIMD3<Float>(x, y, z)

    func face(_ a: SIMD3<Float>, _ b: SIMD3<Float>, _ c: SIMD3<Float>, _ d: SIMD3<Float>, _ n: SIMD3<Float>) {
        verts.append(NativeVertex(position: a, normal: n, color: color))
        verts.append(NativeVertex(position: b, normal: n, color: color))
        verts.append(NativeVertex(position: c, normal: n, color: color))
        verts.append(NativeVertex(position: a, normal: n, color: color))
        verts.append(NativeVertex(position: c, normal: n, color: color))
        verts.append(NativeVertex(position: d, normal: n, color: color))
    }

    face(c001, c101, c111, c011, SIMD3<Float>(0, 0, 1))   // +Z
    face(c100, c000, c010, c110, SIMD3<Float>(0, 0, -1))  // -Z
    face(c101, c100, c110, c111, SIMD3<Float>(1, 0, 0))   // +X
    face(c000, c001, c011, c010, SIMD3<Float>(-1, 0, 0))  // -X
    face(c010, c011, c111, c110, SIMD3<Float>(0, 1, 0))   // +Y
    face(c001, c000, c100, c101, SIMD3<Float>(0, -1, 0))  // -Y
}

private func buildArenaVertices() -> [NativeVertex] {
    var verts: [NativeVertex] = []
    let floorColor = SIMD3<Float>(0.35, 0.35, 0.38)
    let wallColor = SIMD3<Float>(0.55, 0.5, 0.45)
    let floorSize = arenaHalfSize * 2.0

    appendBox(&verts,
              center: SIMD3<Float>(0, -arenaWallThickness / 2.0, 0),
              half: SIMD3<Float>(floorSize / 2.0, arenaWallThickness / 2.0, floorSize / 2.0),
              color: floorColor)

    let wallSpan = floorSize + arenaWallThickness
    let walls: [(SIMD3<Float>, Float, Float)] = [
        (SIMD3<Float>(0, arenaWallHeight / 2.0, -arenaHalfSize), wallSpan, arenaWallThickness),
        (SIMD3<Float>(0, arenaWallHeight / 2.0, arenaHalfSize), wallSpan, arenaWallThickness),
        (SIMD3<Float>(-arenaHalfSize, arenaWallHeight / 2.0, 0), arenaWallThickness, wallSpan),
        (SIMD3<Float>(arenaHalfSize, arenaWallHeight / 2.0, 0), arenaWallThickness, wallSpan),
    ]
    for (position, sizeX, sizeZ) in walls {
        appendBox(&verts,
                  center: position,
                  half: SIMD3<Float>(sizeX / 2.0, arenaWallHeight / 2.0, sizeZ / 2.0),
                  color: wallColor)
    }
    return verts
}

// SIMD3<Float> already has a 16-byte Swift stride (matches MSL's float3
// alignment in the constant address space), so no manual padding fields
// are needed between vector members for the layouts to agree.
private struct NativeUniforms {
    var viewProjection: simd_float4x4
    var lightDir: SIMD3<Float>
    var tintColor: SIMD3<Float> = SIMD3<Float>(0.6, 0.58, 0.55)
}

private func perspective(fovYRadians: Float, aspect: Float, near: Float, far: Float) -> simd_float4x4 {
    let yScale = 1 / tan(fovYRadians * 0.5)
    let xScale = yScale / aspect
    let zRange = far - near
    let zScale = -(far + near) / zRange
    let wzScale = -2 * far * near / zRange
    return simd_float4x4(columns: (
        SIMD4<Float>(xScale, 0, 0, 0),
        SIMD4<Float>(0, yScale, 0, 0),
        SIMD4<Float>(0, 0, zScale, -1),
        SIMD4<Float>(0, 0, wzScale, 0)
    ))
}

private func lookAt(eye: SIMD3<Float>, center: SIMD3<Float>, up: SIMD3<Float>) -> simd_float4x4 {
    let f = normalize(center - eye)
    let s = normalize(cross(f, up))
    let u = cross(s, f)
    return simd_float4x4(columns: (
        SIMD4<Float>(s.x, u.x, -f.x, 0),
        SIMD4<Float>(s.y, u.y, -f.y, 0),
        SIMD4<Float>(s.z, u.z, -f.z, 0),
        SIMD4<Float>(-dot(s, eye), -dot(u, eye), dot(f, eye), 1)
    ))
}

private let nativeShaderSource = """
#include <metal_stdlib>
using namespace metal;

struct Vertex {
    float3 position [[attribute(0)]];
    float3 normal [[attribute(1)]];
    float3 color [[attribute(2)]];
};

struct Uniforms {
    float4x4 viewProjection;
    float3 lightDir;
};

struct VertexOut {
    float4 position [[position]];
    float3 normal;
    float3 color;
};

vertex VertexOut vertex_main(uint vid [[vertex_id]],
                              const device Vertex *verts [[buffer(0)]],
                              constant Uniforms &uniforms [[buffer(1)]]) {
    VertexOut out;
    Vertex v = verts[vid];
    out.position = uniforms.viewProjection * float4(v.position, 1.0);
    out.normal = v.normal;
    out.color = v.color;
    return out;
}

fragment float4 fragment_main(VertexOut in [[stage_in]],
                               constant Uniforms &uniforms [[buffer(1)]]) {
    float diffuse = max(dot(normalize(in.normal), -normalize(uniforms.lightDir)), 0.0);
    float3 lit = in.color * (0.35 + 0.65 * diffuse);
    return float4(lit, 1.0);
}
"""

// Sponza mesh variant (Model I/O-imported .obj: position + normal only, no
// texcoords/textures — trimesh's gltf->obj conversion didn't preserve
// materials, see loadSponza's own comment) — same lighting model as the
// box demo above, flat per-draw tint color via a uniform instead of a
// per-vertex color attribute.
private let sponzaShaderSource = """
#include <metal_stdlib>
using namespace metal;

struct Vertex {
    float3 position [[attribute(0)]];
    float3 normal [[attribute(1)]];
};

struct Uniforms {
    float4x4 viewProjection;
    float3 lightDir;
    float3 tintColor;
};

struct VertexOut {
    float4 position [[position]];
    float3 normal;
};

vertex VertexOut sponza_vertex(Vertex in [[stage_in]],
                                constant Uniforms &uniforms [[buffer(1)]]) {
    VertexOut out;
    out.position = uniforms.viewProjection * float4(in.position, 1.0);
    out.normal = in.normal;
    return out;
}

fragment float4 sponza_fragment(VertexOut in [[stage_in]],
                                 constant Uniforms &uniforms [[buffer(1)]]) {
    float diffuse = max(dot(normalize(in.normal), -normalize(uniforms.lightDir)), 0.0);
    float3 lit = uniforms.tintColor * (0.35 + 0.65 * diffuse);
    return float4(lit, 1.0);
}
"""

private struct NativeSubmesh {
    var mtkSubmesh: MTKSubmesh
    var texture: MTLTexture?
}

private struct NativeMeshEntry {
    var mesh: MTKMesh
    var submeshes: [NativeSubmesh]
}

private extension Comparable {
    func clamped(to range: ClosedRange<Self>) -> Self {
        min(max(self, range.lowerBound), range.upperBound)
    }
}

// Phase 4 of the wgpu-replacement plan: HUD/joystick/crosshair as plain
// UIKit views layered over metalV, instead of reimplementing bevy_ui's
// text/layout engine — presentation-only, no gameplay logic lives here.
// Deliberately dumb: the joystick knob just visualizes the SAME
// moveTouch/lookTouch state ViewController already tracks for the camera
// (see updateTouchUI(), called from enterFrame each frame); the fire
// button decrements a local ammo counter with no actual shooting wired to
// the renderer yet — this phase is about the UI layer existing and being
// visible/interactive, not full gameplay integration.
// Phase 6 Step 5 cont'd: the extra action buttons (jump/aim/crouch/sprint/
// reload) beyond fire — each maps 1:1 to a desktop key/mouse binding the
// real game already reads (see crates/client/src/lib.rs), so pressing one
// just synthesizes that same KeyboardInput/MouseButtonInput via the
// existing FFI (no new Rust code, same technique as FIRE/Step 5):
//   .jump      -> KeyCode::Space,      just_pressed edge (send_player_input)
//   .aim       -> MouseButton::Right,  held            (is_aiming)
//   .crouch    -> KeyCode::KeyC,       just_pressed toggle (toggle_player_stance)
//   .sprint    -> KeyCode::ShiftLeft,  held            (send_player_input)
//   .reload    -> KeyCode::KeyR,       just_pressed edge (reload_gun)
//   .menu      -> KeyCode::F1,         just_pressed toggle (toggle_settings_menu)
//   .leanLeft  -> KeyCode::KeyQ,       held            (apply_lean_input)
//   .leanRight -> KeyCode::KeyE,       held            (apply_lean_input)
//   .prone     -> KeyCode::KeyX,       just_pressed toggle (Standing<->Prone)
// Not wired: gun recoil (ViewRecoil) needs no button at all — shoot()
// already applies it automatically on every real shot, same as desktop.
enum TouchHUDButton: Int {
    case jump = 0, aim = 1, crouch = 2, sprint = 3, reload = 4
    case menu = 5, leanLeft = 6, leanRight = 7, prone = 8
}

final class TouchHUD {
    let joystickBase = UIView()
    let joystickKnob = UIView()
    let fireButton = UIButton(type: .custom)
    let jumpButton = UIButton(type: .custom)
    let aimButton = UIButton(type: .custom)
    let crouchButton = UIButton(type: .custom)
    let sprintButton = UIButton(type: .custom)
    let reloadButton = UIButton(type: .custom)
    let menuButton = UIButton(type: .custom)
    let leanLeftButton = UIButton(type: .custom)
    let leanRightButton = UIButton(type: .custom)
    let proneButton = UIButton(type: .custom)
    let healthLabel = UILabel()
    let ammoLabel = UILabel()
    // Same real bevy_ui PingUi readout the desktop HUD shows — invisible on
    // iOS for the same reason HP/ammo needed native wiring (see
    // ViewController's own syncHud). "--" until the first real reading
    // arrives (see setPing(_:) below).
    let pingLabel = UILabel()
    let crosshairH = UIView()
    let crosshairV = UIView()

    private let joystickRadius: CGFloat = 55
    private let knobRadius: CGFloat = 24
    var ammo = 30 {
        didSet { ammoLabel.text = "\(ammo) / 90" }
    }
    var health = 100 {
        didSet { healthLabel.text = "HP \(health)" }
    }

    // Not a didSet-observed stored property like health/ammo above: -1
    // ("not connected yet") should keep showing "--" rather than "-1 ms",
    // so this needs its own formatting branch instead of a straight
    // string interpolation.
    func setPing(_ ms: Float) {
        // Re-anchored to its own right edge on every call (not just at
        // layout time, unlike health/ammo's fixed-width labels) since the
        // digit count genuinely varies more here (single-digit LAN ping
        // vs 3-digit ping over a Tailscale tunnel) — sizeToFit alone would
        // otherwise grow/shrink from the LEFT and drift into the
        // CROUCH/SPRINT/RELOAD row's own space.
        let rightEdge = pingLabel.frame.maxX
        pingLabel.text = ms >= 0 ? "Ping: \(Int(ms.rounded())) ms" : "Ping: -- ms"
        pingLabel.sizeToFit()
        pingLabel.frame.origin.x = rightEdge - pingLabel.bounds.width
    }

    func install(
        in parent: UIView,
        fireAction: @escaping () -> Void,
        fireUpAction: @escaping () -> Void = {},
        buttonDownAction: @escaping (TouchHUDButton) -> Void = { _ in },
        buttonUpAction: @escaping (TouchHUDButton) -> Void = { _ in }
    ) {
        // Joystick — bottom-left, semi-transparent circles.
        joystickBase.frame = CGRect(x: 0, y: 0, width: joystickRadius * 2, height: joystickRadius * 2)
        joystickBase.layer.cornerRadius = joystickRadius
        joystickBase.backgroundColor = UIColor.white.withAlphaComponent(0.12)
        joystickBase.layer.borderColor = UIColor.white.withAlphaComponent(0.35).cgColor
        joystickBase.layer.borderWidth = 1
        joystickBase.isUserInteractionEnabled = false
        parent.addSubview(joystickBase)

        joystickKnob.frame = CGRect(x: 0, y: 0, width: knobRadius * 2, height: knobRadius * 2)
        joystickKnob.layer.cornerRadius = knobRadius
        joystickKnob.backgroundColor = UIColor.white.withAlphaComponent(0.5)
        joystickKnob.isUserInteractionEnabled = false
        parent.addSubview(joystickKnob)

        // Fire button — bottom-right, semi-transparent red circle.
        let fireRadius: CGFloat = 40
        fireButton.frame = CGRect(x: 0, y: 0, width: fireRadius * 2, height: fireRadius * 2)
        fireButton.layer.cornerRadius = fireRadius
        fireButton.backgroundColor = UIColor.systemRed.withAlphaComponent(0.55)
        fireButton.setTitle("FIRE", for: .normal)
        fireButton.setTitleColor(.white, for: .normal)
        fireButton.titleLabel?.font = .boldSystemFont(ofSize: 14)
        // Phase 6 Step 5 cont'd: FIRE (and AIM, see configureExtraButton's
        // own note) is deliberately NOT a real interactive UIControl —
        // UIButton's own touch tracking would exclusively claim any touch
        // that starts on it, so ViewController's touchesBegan/Moved/Ended
        // (which already drives look-around for any touch in the right
        // half of the screen) would never see it move, and holding FIRE
        // while dragging to track a target wouldn't turn the camera at
        // all. ViewController does its own hit-testing against this
        // button's frame instead (see fireHitTest), so the SAME touch can
        // be both "fire is held" AND "this is the look-drag touch".
        fireButton.isUserInteractionEnabled = false
        parent.addSubview(fireButton)
        self.fireAction = fireAction
        self.fireUpAction = fireUpAction
        self.buttonDownAction = buttonDownAction
        self.buttonUpAction = buttonUpAction

        // Extra action buttons — small semi-transparent circles clustered
        // above FIRE, thumb-reachable from the same hand.
        configureExtraButton(jumpButton, title: "JUMP", tag: .jump, color: .systemBlue)
        configureExtraButton(aimButton, title: "AIM", tag: .aim, color: .systemPurple)
        configureExtraButton(crouchButton, title: "CROUCH", tag: .crouch, color: .systemTeal)
        configureExtraButton(sprintButton, title: "SPRINT", tag: .sprint, color: .systemOrange)
        configureExtraButton(reloadButton, title: "RELOAD", tag: .reload, color: .systemGray)
        configureExtraButton(menuButton, title: "MENU", tag: .menu, color: .darkGray)
        configureExtraButton(leanLeftButton, title: "◄ LEAN", tag: .leanLeft, color: .systemGreen)
        configureExtraButton(leanRightButton, title: "LEAN ►", tag: .leanRight, color: .systemGreen)
        configureExtraButton(proneButton, title: "PRONE", tag: .prone, color: .systemTeal)
        for btn in [jumpButton, aimButton, crouchButton, sprintButton, reloadButton, menuButton, leanLeftButton, leanRightButton, proneButton] {
            parent.addSubview(btn)
        }

        // Health/ammo HUD — top corners.
        healthLabel.text = "HP \(health)"
        healthLabel.textColor = .white
        healthLabel.font = .boldSystemFont(ofSize: 18)
        healthLabel.sizeToFit()
        parent.addSubview(healthLabel)

        ammoLabel.text = "\(ammo) / 90"
        ammoLabel.textColor = .white
        ammoLabel.font = .boldSystemFont(ofSize: 18)
        ammoLabel.textAlignment = .right
        ammoLabel.sizeToFit()
        parent.addSubview(ammoLabel)

        setPing(-1)
        pingLabel.textColor = UIColor(red: 0.7, green: 0.85, blue: 1.0, alpha: 1) // matches PingUi's own TextColor
        pingLabel.font = .systemFont(ofSize: 14)
        pingLabel.textAlignment = .right
        pingLabel.sizeToFit()
        parent.addSubview(pingLabel)

        // Crosshair — dead center, two thin white bars.
        crosshairH.backgroundColor = UIColor.white.withAlphaComponent(0.8)
        crosshairV.backgroundColor = UIColor.white.withAlphaComponent(0.8)
        parent.addSubview(crosshairH)
        parent.addSubview(crosshairV)

        layout(in: parent)
    }

    private var fireAction: (() -> Void)?
    private var fireUpAction: (() -> Void)?
    private var buttonDownAction: ((TouchHUDButton) -> Void)?
    private var buttonUpAction: ((TouchHUDButton) -> Void)?

    private func configureExtraButton(_ button: UIButton, title: String, tag: TouchHUDButton, color: UIColor) {
        let radius: CGFloat = 26
        button.frame = CGRect(x: 0, y: 0, width: radius * 2, height: radius * 2)
        button.layer.cornerRadius = radius
        button.backgroundColor = color.withAlphaComponent(0.45)
        button.setTitle(title, for: .normal)
        button.setTitleColor(.white, for: .normal)
        button.titleLabel?.font = .boldSystemFont(ofSize: 9)
        button.titleLabel?.adjustsFontSizeToFitWidth = true
        button.tag = tag.rawValue
        // AIM is the one other button (besides FIRE) you'd realistically
        // want to hold while also dragging to adjust aim — same
        // "don't let UIButton exclusively claim the touch" reasoning as
        // fireButton above, handled instead by ViewController's own
        // hit-testing (aimHitTest) so the touch can drive both ADS and
        // look-around at once. Every other extra button is a genuine
        // tap-once action with no reason to drag while held, so they keep
        // normal UIButton tracking.
        if tag == .aim {
            button.isUserInteractionEnabled = false
        } else {
            button.addTarget(self, action: #selector(handleExtraTouchDown(_:)), for: .touchDown)
            button.addTarget(self, action: #selector(handleExtraTouchUp(_:)), for: [.touchUpInside, .touchUpOutside, .touchCancel])
        }
    }

    @objc private func handleExtraTouchDown(_ sender: UIButton) {
        guard let button = TouchHUDButton(rawValue: sender.tag) else { return }
        UIView.animate(withDuration: 0.05) {
            sender.backgroundColor = sender.backgroundColor?.withAlphaComponent(0.85)
        }
        buttonDownAction?(button)
    }

    @objc private func handleExtraTouchUp(_ sender: UIButton) {
        guard let button = TouchHUDButton(rawValue: sender.tag) else { return }
        UIView.animate(withDuration: 0.15) {
            sender.backgroundColor = sender.backgroundColor?.withAlphaComponent(0.45)
        }
        buttonUpAction?(button)
    }

    // Same reasoning as triggerFireDown/Up below — aimButton also has
    // isUserInteractionEnabled=false, so it never reaches
    // handleExtraTouchDown/Up's flash animation; replicate just the visual
    // here for ViewController to call directly.
    func flashAim(pressed: Bool) {
        if pressed {
            UIView.animate(withDuration: 0.05) {
                self.aimButton.backgroundColor = self.aimButton.backgroundColor?.withAlphaComponent(0.85)
            }
        } else {
            UIView.animate(withDuration: 0.15) {
                self.aimButton.backgroundColor = self.aimButton.backgroundColor?.withAlphaComponent(0.45)
            }
        }
    }

    // Not @objc/UIControl-target-action anymore — fireButton has
    // isUserInteractionEnabled=false (see its own comment), so
    // ViewController's touchesBegan/Ended call these directly once its own
    // hit-test against fireButton.frame decides a touch pressed/released it.
    // Deliberately does NOT call `fireAction`/`fireUpAction` — those are
    // ViewController's own callback INTO here; calling them back out again
    // would recurse into ViewController.handleFire, which is what called
    // this in the first place.
    func triggerFireUp() {}

    func triggerFireDown() {
        // This `ammo` counter is purely this demo HUD's own decoration (see
        // TouchHUD's doc comment) — real-game mode's actual ammo lives in
        // the headless App's own `Gun` component and is what really gates
        // firing (see `shoot()` in crates/client/src/lib.rs), so don't let
        // this fake counter block the real fire trigger below once it
        // hits 0; only skip the cosmetic decrement/flash.
        if ammo > 0 {
            ammo -= 1
            UIView.animate(withDuration: 0.05, animations: {
                self.fireButton.backgroundColor = UIColor.systemRed.withAlphaComponent(0.9)
            }, completion: { _ in
                UIView.animate(withDuration: 0.15) {
                    self.fireButton.backgroundColor = UIColor.systemRed.withAlphaComponent(0.55)
                }
            })
        }
    }

    func layout(in parent: UIView) {
        let bounds = parent.bounds
        let margin: CGFloat = 24
        // iOS safe area (notch/Dynamic Island/home-indicator/rounded
        // corners) — in this landscape-only app the meaningful insets are
        // .left/.right (whichever side the sensor housing rotates to) and
        // .bottom (home indicator strip); pull every edge-hugging control
        // in from the raw view bounds by margin + the matching inset
        // instead of a flat margin, so nothing sits under those system
        // areas or gets clipped by the display's rounded corners.
        let safeArea = parent.safeAreaInsets
        let leftMargin = margin + safeArea.left
        let rightMargin = margin + safeArea.right
        let topMargin = margin + safeArea.top
        let bottomMargin = margin + safeArea.bottom
        joystickBase.center = CGPoint(x: leftMargin + joystickRadius, y: bounds.height - bottomMargin - joystickRadius)
        joystickKnob.center = joystickBase.center
        fireButton.center = CGPoint(x: bounds.width - rightMargin - fireButton.bounds.width / 2, y: bounds.height - bottomMargin - fireButton.bounds.height / 2)
        // JUMP/AIM: right thumb's other two most-used actions, flanking
        // FIRE directly (same reach as FIRE itself) instead of floating
        // over the gun model higher up the screen.
        jumpButton.center = CGPoint(x: fireButton.center.x, y: fireButton.frame.minY - margin - jumpButton.bounds.height / 2)
        aimButton.center = CGPoint(x: fireButton.frame.minX - margin - aimButton.bounds.width / 2, y: fireButton.center.y)
        healthLabel.frame.origin = CGPoint(x: leftMargin, y: topMargin + 20)
        ammoLabel.frame.origin = CGPoint(x: bounds.width - rightMargin - ammoLabel.bounds.width, y: topMargin + 20)
        pingLabel.sizeToFit()
        pingLabel.frame.origin = CGPoint(x: bounds.width - rightMargin - pingLabel.bounds.width, y: ammoLabel.frame.maxY + 4)
        // CROUCH/SPRINT/RELOAD: less time-critical, tucked under the ammo
        // counter in the empty upper-right corner instead — clear of both
        // the gun model and the FIRE/JUMP/AIM cluster. Must come after
        // ammoLabel's own origin is set above, since it anchors off
        // ammoLabel.frame.maxY.
        let topRow = [crouchButton, proneButton, sprintButton, reloadButton]
        let topSpacing: CGFloat = 8
        let topY = ammoLabel.frame.maxY + margin + crouchButton.bounds.height / 2
        var nextLeft = bounds.width - rightMargin
        for btn in topRow {
            nextLeft -= btn.bounds.width
            btn.center = CGPoint(x: nextLeft + btn.bounds.width / 2, y: topY)
            nextLeft -= topSpacing
        }
        // MENU: top-left, below HP — well clear of the combat cluster,
        // mirrors a pause/settings button's conventional corner placement.
        menuButton.center = CGPoint(x: healthLabel.frame.minX + menuButton.bounds.width / 2, y: healthLabel.frame.maxY + margin + menuButton.bounds.height / 2)
        // LEAN L/R: flank the move joystick above it, left thumb's other
        // most-used action while strafing — matches WASD+Q/E's proximity
        // on desktop.
        let leanY = joystickBase.frame.minY - margin - leanLeftButton.bounds.height / 2
        leanLeftButton.center = CGPoint(x: joystickBase.center.x - joystickRadius, y: leanY)
        leanRightButton.center = CGPoint(x: joystickBase.center.x + joystickRadius, y: leanY)
        crosshairH.frame = CGRect(x: bounds.midX - 10, y: bounds.midY - 1, width: 20, height: 2)
        crosshairV.frame = CGRect(x: bounds.midX - 1, y: bounds.midY - 10, width: 2, height: 20)
    }

    // Called once per frame — moves the knob to visualize the SAME
    // moveTouch drag ViewController already reads for camera movement, so
    // there's exactly one source of truth for "how far the joystick is
    // pushed", not a second copy of the touch-tracking logic.
    func updateKnob(dragX: CGFloat, dragY: CGFloat, maxDrag: CGFloat) {
        let clampedX = (dragX / maxDrag).clamped(to: -1...1) * (joystickRadius - knobRadius)
        let clampedY = (dragY / maxDrag).clamped(to: -1...1) * (joystickRadius - knobRadius)
        joystickKnob.center = CGPoint(x: joystickBase.center.x + clampedX, y: joystickBase.center.y + clampedY)
    }

    func resetKnob() {
        joystickKnob.center = joystickBase.center
    }
}

class ViewController: UIViewController {
    @IBOutlet var metalV: MetalView!

    // Native Metal demo state — only used when USE_NATIVE_METAL_DEMO.
    private var metalDevice: MTLDevice?
    private var metalQueue: MTLCommandQueue?
    private var metalLayer: CAMetalLayer?
    private var nativePipeline: MTLRenderPipelineState?
    private var nativeDepthState: MTLDepthStencilState?
    private var nativeVertexBuffer: MTLBuffer?
    private var nativeVertexCount: Int = 0
    private var nativeDepthTexture: MTLTexture?
    private var sponzaPipeline: MTLRenderPipelineState?
    private var sponzaMeshes: [NativeMeshEntry] = []
    // Not `private`: GameControllerInput.swift (Bluetooth keyboard/mouse,
    // Phase 6 Step 3) reads this the same way it already reads `bevyApp`.
    var rustMetalProbe: OpaquePointer?
    // Phase 6 Step 1 spike (temporary) — see RUN_HEADLESS_GAME_APP_SPIKE.
    private var headlessGameApp: OpaquePointer?
    private var headlessGameAppTimer: Timer?
    // Free-fly camera state, driven by touch (no Bevy/Rust involved).
    private var cameraPosition = SIMD3<Float>(0, 3, 4)
    private var cameraYaw: Float = 0
    private var cameraPitch: Float = 0
    // Left half of screen = move (drag vector -> forward/strafe), right
    // half = look (drag delta -> yaw/pitch), tracked by touch identity.
    private var moveTouch: UITouch?
    private var moveTouchStart: CGPoint = .zero
    private var moveTouchCurrent: CGPoint = .zero
    private var lookTouch: UITouch?
    private var lookTouchLast: CGPoint = .zero
    // FIRE/AIM: track which UITouch (if any) started inside each button's
    // hit area, so it can be recognized again on move/end even though the
    // SAME touch is very likely also `lookTouch` (see fireButton/aimButton's
    // own isUserInteractionEnabled=false comments) — holding either button
    // and dragging rotates the camera exactly like an ordinary look-drag,
    // while still keeping fire/aim engaged until that finger lifts.
    private var fireTouch: UITouch?
    private var aimTouch: UITouch?
    private var lastFrameTime: CFTimeInterval = CACurrentMediaTime()
    private let touchHUD = TouchHUD()
    // Native settings-menu overlay — see SettingsMenuView.swift's own doc
    // comment for why this exists instead of relying on bevy_ui.
    private let settingsMenu = SettingsMenuView()
    private var settingsMenuWasVisible = false
    // Native death/restart overlay — see GameOverView.swift's own doc
    // comment for why this exists instead of relying on bevy_ui.
    private let gameOverView = GameOverView()
    private var gameOverWasVisible = false
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
        touchHUD.install(in: self.view, fireAction: { [weak self] in
            self?.handleFire(pressed: true)
        }, fireUpAction: { [weak self] in
            self?.handleFire(pressed: false)
        }, buttonDownAction: { [weak self] button in
            self?.handleExtraButton(button, pressed: true)
        }, buttonUpAction: { [weak self] button in
            self?.handleExtraButton(button, pressed: false)
        })

        self.view.addSubview(settingsMenu)
        settingsMenu.onClose = { [weak self] in
            guard let self, let probe = self.rustMetalProbe else { return }
            native_metal_probe_set_menu_visible(probe, false)
        }
        settingsMenu.onSensitivityDec = { [weak self] in
            guard let self, let probe = self.rustMetalProbe else { return }
            let current = native_metal_probe_get_sensitivity(probe)
            native_metal_probe_set_sensitivity(probe, current - 0.25)
        }
        settingsMenu.onSensitivityInc = { [weak self] in
            guard let self, let probe = self.rustMetalProbe else { return }
            let current = native_metal_probe_get_sensitivity(probe)
            native_metal_probe_set_sensitivity(probe, current + 0.25)
        }
        settingsMenu.onToggleAutopilot = { [weak self] in
            guard let self, let probe = self.rustMetalProbe else { return }
            native_metal_probe_set_autopilot(probe, !native_metal_probe_get_autopilot(probe))
        }
        #if DEBUG
        settingsMenu.onToggleGodMode = { [weak self] in
            guard let self, let probe = self.rustMetalProbe else { return }
            native_metal_probe_set_god_mode(probe, !native_metal_probe_get_god_mode(probe))
        }
        settingsMenu.onToggleNoclip = { [weak self] in
            guard let self, let probe = self.rustMetalProbe else { return }
            native_metal_probe_set_noclip(probe, !native_metal_probe_get_noclip(probe))
        }
        settingsMenu.onToggleFreeze = { [weak self] in
            guard let self, let probe = self.rustMetalProbe else { return }
            native_metal_probe_set_freeze_zombies(probe, !native_metal_probe_get_freeze_zombies(probe))
        }
        settingsMenu.onKillAll = { [weak self] in
            guard let self, let probe = self.rustMetalProbe else { return }
            native_metal_probe_trigger_kill_all_zombies(probe)
        }
        settingsMenu.onSpawnMore = { [weak self] in
            guard let self, let probe = self.rustMetalProbe else { return }
            native_metal_probe_trigger_spawn_more_zombies(probe)
        }
        settingsMenu.onZombieCountDec = { [weak self] in
            guard let self, let probe = self.rustMetalProbe else { return }
            native_metal_probe_zombie_count_dec(probe)
        }
        settingsMenu.onZombieCountInc = { [weak self] in
            guard let self, let probe = self.rustMetalProbe else { return }
            native_metal_probe_zombie_count_inc(probe)
        }
        settingsMenu.onHealNearest = { [weak self] in
            guard let self, let probe = self.rustMetalProbe else { return }
            native_metal_probe_heal_nearest_zombie(probe)
        }
        settingsMenu.onNearDeathNearest = { [weak self] in
            guard let self, let probe = self.rustMetalProbe else { return }
            native_metal_probe_near_death_nearest_zombie(probe)
        }
        settingsMenu.onToggleGun = { [weak self] in
            guard let self, let probe = self.rustMetalProbe else { return }
            native_metal_probe_set_gun_visible(probe, !native_metal_probe_get_gun_visible(probe))
        }
        settingsMenu.onToggleMsaa = { [weak self] in
            guard let self, let probe = self.rustMetalProbe else { return }
            native_metal_probe_set_map_msaa(probe, !native_metal_probe_get_map_msaa(probe))
        }
        settingsMenu.onToggleShadow = { [weak self] in
            guard let self, let probe = self.rustMetalProbe else { return }
            native_metal_probe_set_map_shadows(probe, !native_metal_probe_get_map_shadows(probe))
        }
        settingsMenu.onToggleHitboxes = { [weak self] in
            guard let self, let probe = self.rustMetalProbe else { return }
            native_metal_probe_set_show_hitboxes(probe, !native_metal_probe_get_show_hitboxes(probe))
        }
        settingsMenu.onToggleAimBlur = { [weak self] in
            guard let self, let probe = self.rustMetalProbe else { return }
            native_metal_probe_set_aim_blur_enabled(probe, !native_metal_probe_get_aim_blur_enabled(probe))
        }
        settingsMenu.onTogglePlayerModel = { [weak self] in
            guard let self, let probe = self.rustMetalProbe else { return }
            native_metal_probe_set_player_model_visible(probe, !native_metal_probe_get_player_model_visible(probe))
        }
        settingsMenu.onToggleNoZombieAnim = { [weak self] in
            guard let self, let probe = self.rustMetalProbe else { return }
            native_metal_probe_set_zombie_anim_disabled(probe, !native_metal_probe_get_zombie_anim_disabled(probe))
        }
        settingsMenu.onToggleNoHpBar = { [weak self] in
            guard let self, let probe = self.rustMetalProbe else { return }
            native_metal_probe_set_zombie_health_bars_hidden(probe, !native_metal_probe_get_zombie_health_bars_hidden(probe))
        }
        settingsMenu.onToggleFullLod = { [weak self] in
            guard let self, let probe = self.rustMetalProbe else { return }
            native_metal_probe_set_zombie_lod_disabled(probe, !native_metal_probe_get_zombie_lod_disabled(probe))
        }
        #endif

        self.view.addSubview(gameOverView)
        gameOverView.onRestart = { [weak self] in
            guard let self, let probe = self.rustMetalProbe else { return }
            native_metal_probe_trigger_restart(probe)
        }
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        touchHUD.layout(in: self.view)
        settingsMenu.layout(in: self.view)
        gameOverView.layout(in: self.view)
    }

    // Polled once per frame (see enterFrame) — mirrors syncSettingsMenu's
    // own poll-not-push pattern, since GameOver can also become true from
    // the server's own GameStateEvent broadcast (on_game_state_event in
    // crate::lib), not just this client's own check_game_over.
    private func syncGameOver() {
        guard USE_REAL_GAME_NATIVE_RENDERER, let probe = self.rustMetalProbe else { return }
        let visible = native_metal_probe_game_over(probe)
        gameOverView.isHidden = !visible
        if visible && !gameOverWasVisible {
            gameOverView.layout(in: self.view)
        }
        gameOverWasVisible = visible
    }

    // Polled once per frame (see enterFrame) — the HUD's own `health`
    // property (TouchHUD.swift) previously never got written to at all
    // (only used to build the initial "HP 100" label text), so it silently
    // stayed at its hardcoded default forever regardless of real damage.
    // native_metal_probe_get_player_hp reads the same real Health
    // component the settings menu's own HP row already reads (see
    // syncSettingsMenu below) — -1 before the local player has spawned
    // yet, in which case the HUD just keeps showing its last known value
    // rather than flashing to a wrong number.
    private func syncHud() {
        guard USE_REAL_GAME_NATIVE_RENDERER, let probe = self.rustMetalProbe else { return }
        let hp = native_metal_probe_get_player_hp(probe)
        if hp >= 0 {
            touchHUD.health = Int(hp.rounded())
        }
        touchHUD.setPing(native_metal_probe_get_ping_ms(probe))

        // Apple Watch companion app sync — see WatchSessionManager's own
        // doc comment for why this runs every frame alongside the rest of
        // the HUD instead of on its own timer.
        let killCount = native_metal_probe_get_kill_count(probe)
        WatchSessionManager.shared.update(hp: hp, killCount: killCount)
    }

    // Polled once per frame (see enterFrame) rather than only on the MENU
    // button's own press — SettingsMenuVisible can also flip via a
    // hardware F1 key (GameControllerInput.swift), so this stays correct
    // regardless of which input source toggled it.
    private func syncSettingsMenu() {
        guard USE_REAL_GAME_NATIVE_RENDERER, let probe = self.rustMetalProbe else { return }
        let visible = native_metal_probe_menu_visible(probe)
        settingsMenu.isHidden = !visible
        if visible {
            let sensitivity = native_metal_probe_get_sensitivity(probe)
            let hp = native_metal_probe_get_player_hp(probe)
            let zombieLiveCount = native_metal_probe_get_live_zombie_count(probe)
            let autopilot = native_metal_probe_get_autopilot(probe)
            #if DEBUG
            let godMode = native_metal_probe_get_god_mode(probe)
            let noclip = native_metal_probe_get_noclip(probe)
            let freeze = native_metal_probe_get_freeze_zombies(probe)
            let zombieTargetCount = native_metal_probe_get_zombie_target_count(probe)
            let gunVisible = native_metal_probe_get_gun_visible(probe)
            let mapMsaa = native_metal_probe_get_map_msaa(probe)
            let mapShadows = native_metal_probe_get_map_shadows(probe)
            let showHitboxes = native_metal_probe_get_show_hitboxes(probe)
            let aimBlur = native_metal_probe_get_aim_blur_enabled(probe)
            let playerModelVisible = native_metal_probe_get_player_model_visible(probe)
            let zombieAnimDisabled = native_metal_probe_get_zombie_anim_disabled(probe)
            let zombieHealthBarsHidden = native_metal_probe_get_zombie_health_bars_hidden(probe)
            let zombieLodDisabled = native_metal_probe_get_zombie_lod_disabled(probe)
            #else
            let godMode = false
            let noclip = false
            let freeze = false
            let zombieTargetCount: UInt32 = 0
            let gunVisible = false
            let mapMsaa = false
            let mapShadows = false
            let showHitboxes = false
            let aimBlur = false
            let playerModelVisible = false
            let zombieAnimDisabled = false
            let zombieHealthBarsHidden = false
            let zombieLodDisabled = false
            #endif
            if !settingsMenuWasVisible {
                settingsMenu.layout(in: self.view)
            }
            settingsMenu.refresh(
                sensitivity: sensitivity, playerHp: hp, autopilot: autopilot,
                godMode: godMode, noclip: noclip, freezeZombies: freeze,
                zombieLiveCount: zombieLiveCount, zombieTargetCount: zombieTargetCount,
                gunVisible: gunVisible, mapMsaa: mapMsaa, mapShadows: mapShadows,
                showHitboxes: showHitboxes, aimBlur: aimBlur, playerModelVisible: playerModelVisible,
                zombieAnimDisabled: zombieAnimDisabled, zombieHealthBarsHidden: zombieHealthBarsHidden, zombieLodDisabled: zombieLodDisabled
            )
        }
        settingsMenuWasVisible = visible
    }

    // Phase 6 Step 5 (real shooting): in real-game-renderer mode, drive the
    // headless App's own `ButtonInput<MouseButton>::Left` exactly like a
    // desktop left-click — the real `shoot()` system (crates/client/src/
    // lib.rs) already reads that unmodified, no new Rust code needed. The
    // headless app's synthetic PrimaryWindow starts with CursorOptions
    // grab_mode already Locked (see native_game_app.rs), so `shoot()`'s
    // cursor-grab gate is satisfied from the start — press=true starts
    // (held = auto-fire at the game's own FIRE_RATE), press=false on
    // release stops it, same semantics as holding down a real mouse button.
    private func handleFire(pressed: Bool) {
        if pressed {
            touchHUD.triggerFireDown()
        } else {
            touchHUD.triggerFireUp()
        }
        if USE_REAL_GAME_NATIVE_RENDERER, let probe = self.rustMetalProbe {
            native_metal_probe_mouse_button_changed(probe, 0, pressed)
        }
        if pressed {
            print("[touch-hud] fire, ammo=\(touchHUD.ammo)")
        }
    }

    // Phase 6 Step 5 cont'd: jump/aim/crouch/sprint/reload, same
    // "synthesize the exact desktop key/mouse event the real game already
    // reads" approach as handleFire — see TouchHUDButton's own doc comment
    // for which KeyCode/MouseButton each one maps to and why (key_id
    // numbers match map_key_id in crates/client/src/cadisplaylink/ffi.rs:
    // letters A-Z are 0-25, so C=2, R=17; Space=36; ShiftLeft=38).
    // `jump`/`crouch`/`reload` are just_pressed-edge reads in the real
    // game, so a plain press+release (not held) is correct even though the
    // button might be held longer by the finger — sending press-down once
    // is what produces that edge; only aim/sprint care about hold duration.
    private func handleExtraButton(_ button: TouchHUDButton, pressed: Bool) {
        if button == .aim {
            touchHUD.flashAim(pressed: pressed)
        }
        guard USE_REAL_GAME_NATIVE_RENDERER, let probe = self.rustMetalProbe else { return }
        switch button {
        case .jump:
            native_metal_probe_key_changed(probe, 36, pressed) // Space
        case .aim:
            native_metal_probe_mouse_button_changed(probe, 1, pressed) // MouseButton::Right
        case .crouch:
            if pressed { native_metal_probe_key_changed(probe, 2, true) } // KeyC, just_pressed toggle
            else { native_metal_probe_key_changed(probe, 2, false) }
        case .sprint:
            native_metal_probe_key_changed(probe, 38, pressed) // ShiftLeft
        case .reload:
            if pressed { native_metal_probe_key_changed(probe, 17, true) } // KeyR, just_pressed edge
            else { native_metal_probe_key_changed(probe, 17, false) }
        case .menu:
            if pressed { native_metal_probe_key_changed(probe, 46, true) } // F1, just_pressed toggle
            else { native_metal_probe_key_changed(probe, 46, false) }
        case .leanLeft:
            native_metal_probe_key_changed(probe, 16, pressed) // KeyQ, held
        case .leanRight:
            native_metal_probe_key_changed(probe, 4, pressed) // KeyE, held
        case .prone:
            if pressed { native_metal_probe_key_changed(probe, 23, true) } // KeyX, just_pressed toggle
            else { native_metal_probe_key_changed(probe, 23, false) }
        }
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        self.view.backgroundColor = .black
        if USE_NATIVE_METAL_DEMO {
            if metalDevice == nil {
                self.startNativeMetalDemo()
            }
        } else if rustMetalProbe == nil {
            let viewPointer = Unmanaged.passUnretained(self.metalV).toOpaque()
            if USE_REAL_GAME_NATIVE_RENDERER {
                rustMetalProbe = native_metal_probe_create_real_game(viewPointer, Float(UIScreen.main.nativeScale))
            } else {
                rustMetalProbe = native_metal_probe_create(viewPointer, Float(UIScreen.main.nativeScale))
            }
            if #available(iOS 15.0, *) {
                displayLink.preferredFrameRateRange = CAFrameRateRange(minimum: 80, maximum: 120, preferred: 120)
            }
            if #available(iOS 14.0, *), gameControllerInput == nil {
                gameControllerInput = GameControllerInput(viewController: self)
            }
        }
        if RUN_HEADLESS_GAME_APP_SPIKE && headlessGameApp == nil {
            headlessGameApp = native_game_app_create()
            headlessGameAppTimer = Timer.scheduledTimer(withTimeInterval: 0.5, repeats: true) { [weak self] _ in
                guard let app = self?.headlessGameApp else { return }
                native_game_app_tick(app)
            }
        }
        self.displayLink.isPaused = false
    }

    func startNativeMetalDemo() {
        guard let device = MTLCreateSystemDefaultDevice(),
              let layer = self.metalV.layer as? CAMetalLayer else {
            return
        }
        layer.device = device
        layer.pixelFormat = .bgra8Unorm
        self.metalDevice = device
        self.metalQueue = device.makeCommandQueue()
        self.metalLayer = layer

        let verts = buildArenaVertices()
        self.nativeVertexCount = verts.count
        self.nativeVertexBuffer = device.makeBuffer(
            bytes: verts,
            length: verts.count * MemoryLayout<NativeVertex>.stride,
            options: .storageModeShared
        )

        do {
            let library = try device.makeLibrary(source: nativeShaderSource, options: nil)
            let pipelineDescriptor = MTLRenderPipelineDescriptor()
            pipelineDescriptor.vertexFunction = library.makeFunction(name: "vertex_main")
            pipelineDescriptor.fragmentFunction = library.makeFunction(name: "fragment_main")
            pipelineDescriptor.colorAttachments[0].pixelFormat = .bgra8Unorm
            pipelineDescriptor.depthAttachmentPixelFormat = .depth32Float
            self.nativePipeline = try device.makeRenderPipelineState(descriptor: pipelineDescriptor)
        } catch {
            print("[native-metal-demo] pipeline creation failed: \(error)")
        }

        let depthDescriptor = MTLDepthStencilDescriptor()
        depthDescriptor.depthCompareFunction = .less
        depthDescriptor.isDepthWriteEnabled = true
        self.nativeDepthState = device.makeDepthStencilState(descriptor: depthDescriptor)

        self.loadSponza(device: device)

        if #available(iOS 15.0, *) {
            displayLink.preferredFrameRateRange = CAFrameRateRange(minimum: 80, maximum: 120, preferred: 120)
        }
    }

    // Loads Sponza via Model I/O — NOT the real game's
    // models/Sponza/Sponza.gltf directly: Model I/O on iOS reports
    // `canImportFileExtension("gltf") == false` (confirmed on-device via
    // MDLAsset's error-reporting initializer: "Could not load asset with
    // unknown extension"), so glTF is a dead end here. Converted once
    // offline (Python + trimesh, see project history) to
    // assets/models/SponzaObj/Sponza.obj, which Model I/O's long-standing
    // OBJ importer handles fine — 103 meshes, position+normal only (the
    // conversion didn't preserve materials/textures, hence the flat
    // tintColor uniform in sponzaShaderSource instead of per-vertex color
    // or texturing).
    private func loadSponza(device: MTLDevice) {
        guard let resourcePath = Bundle.main.resourcePath else { return }
        let url = URL(fileURLWithPath: resourcePath)
            .appendingPathComponent("assets/models/SponzaObj/Sponza.obj")
        guard FileManager.default.fileExists(atPath: url.path) else {
            print("[native-metal-demo] Sponza.obj not found at \(url.path)")
            return
        }

        do {
            let library = try device.makeLibrary(source: sponzaShaderSource, options: nil)
            let pipelineDescriptor = MTLRenderPipelineDescriptor()
            pipelineDescriptor.vertexFunction = library.makeFunction(name: "sponza_vertex")
            pipelineDescriptor.fragmentFunction = library.makeFunction(name: "sponza_fragment")
            pipelineDescriptor.colorAttachments[0].pixelFormat = .bgra8Unorm
            pipelineDescriptor.depthAttachmentPixelFormat = .depth32Float

            let vertexDescriptor = MDLVertexDescriptor()
            vertexDescriptor.attributes[0] = MDLVertexAttribute(
                name: MDLVertexAttributePosition, format: .float3, offset: 0, bufferIndex: 0)
            vertexDescriptor.attributes[1] = MDLVertexAttribute(
                name: MDLVertexAttributeNormal, format: .float3, offset: 12, bufferIndex: 0)
            vertexDescriptor.layouts[0] = MDLVertexBufferLayout(stride: 24)
            pipelineDescriptor.vertexDescriptor = MTKMetalVertexDescriptorFromModelIO(vertexDescriptor)

            self.sponzaPipeline = try device.makeRenderPipelineState(descriptor: pipelineDescriptor)

            let allocator = MTKMeshBufferAllocator(device: device)
            let asset = MDLAsset(url: url, vertexDescriptor: vertexDescriptor, bufferAllocator: allocator)

            var entries: [NativeMeshEntry] = []
            for object in asset.childObjects(of: MDLMesh.self) {
                guard let mdlMesh = object as? MDLMesh else { continue }
                mdlMesh.addNormals(withAttributeNamed: MDLVertexAttributeNormal, creaseThreshold: 0.5)
                guard let mtkMesh = try? MTKMesh(mesh: mdlMesh, device: device) else { continue }
                let submeshes = mtkMesh.submeshes.map { NativeSubmesh(mtkSubmesh: $0, texture: nil) }
                entries.append(NativeMeshEntry(mesh: mtkMesh, submeshes: submeshes))
            }
            self.sponzaMeshes = entries
            print("[native-metal-demo] loaded \(entries.count) Sponza meshes")
        } catch {
            print("[native-metal-demo] Sponza pipeline/load failed: \(error)")
        }
    }

    private func ensureDepthTexture(device: MTLDevice, width: Int, height: Int) -> MTLTexture? {
        if let tex = nativeDepthTexture, tex.width == width, tex.height == height {
            return tex
        }
        let descriptor = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: .depth32Float, width: width, height: height, mipmapped: false
        )
        descriptor.usage = .renderTarget
        descriptor.storageMode = .private
        let tex = device.makeTexture(descriptor: descriptor)
        nativeDepthTexture = tex
        return tex
    }

    // Touch-driven free-fly camera update, called once per frame before
    // rendering — left half of the screen drags to move (forward/strafe,
    // scaled by drag distance from the touch's start point, like a virtual
    // joystick), right half drags to look (delta yaw/pitch). No Bevy/Rust
    // involved; pure UIKit touch tracking + simd math.
    private func updateCamera(deltaTime: Float) {
        if let lookTouch = self.lookTouch {
            let current = lookTouch.location(in: self.metalV)
            let dx = Float(current.x - lookTouchLast.x)
            let dy = Float(current.y - lookTouchLast.y)
            cameraYaw -= dx * 0.005
            cameraPitch = (cameraPitch - dy * 0.005).clamped(to: -1.54...1.54)
            lookTouchLast = current
        }
        if self.moveTouch != nil {
            let dx = Float(moveTouchCurrent.x - moveTouchStart.x)
            let dy = Float(moveTouchCurrent.y - moveTouchStart.y)
            let maxDrag: Float = 60.0
            let strafe = (dx / maxDrag).clamped(to: -1...1)
            let forwardAmount = (-dy / maxDrag).clamped(to: -1...1)
            let yawRot = simd_quatf(angle: cameraYaw, axis: SIMD3<Float>(0, 1, 0))
            let forward = yawRot.act(SIMD3<Float>(0, 0, -1))
            let right = yawRot.act(SIMD3<Float>(1, 0, 0))
            let move = (forward * forwardAmount + right * strafe)
            cameraPosition += move * 6.0 * deltaTime
        }
    }

    // Draws the loaded Sponza meshes with a touch-driven free-fly camera —
    // one render pass, one draw call per submesh, no Bevy/wgpu anywhere in
    // the chain.
    func renderNativeMetalFrame() {
        let now = CACurrentMediaTime()
        let deltaTime = Float(now - lastFrameTime)
        lastFrameTime = now
        updateCamera(deltaTime: deltaTime)

        guard let layer = self.metalLayer,
              let queue = self.metalQueue,
              let device = self.metalDevice,
              let depthState = self.nativeDepthState,
              let drawable = layer.nextDrawable() else {
            return
        }
        let width = drawable.texture.width
        let height = drawable.texture.height
        guard let depthTexture = ensureDepthTexture(device: device, width: width, height: height) else {
            return
        }

        let aspect = Float(width) / Float(max(height, 1))
        let proj = perspective(fovYRadians: 60.0 * .pi / 180.0, aspect: aspect, near: 0.05, far: 500.0)
        let pitchRot = simd_quatf(angle: cameraPitch, axis: SIMD3<Float>(1, 0, 0))
        let yawRot = simd_quatf(angle: cameraYaw, axis: SIMD3<Float>(0, 1, 0))
        let lookDir = (yawRot * pitchRot).act(SIMD3<Float>(0, 0, -1))
        let view = lookAt(eye: cameraPosition, center: cameraPosition + lookDir, up: SIMD3<Float>(0, 1, 0))
        var uniforms = NativeUniforms(
            viewProjection: proj * view,
            lightDir: normalize(SIMD3<Float>(-0.8, -0.6, 0.2))
        )

        let pass = MTLRenderPassDescriptor()
        pass.colorAttachments[0].texture = drawable.texture
        pass.colorAttachments[0].loadAction = .clear
        pass.colorAttachments[0].storeAction = .store
        pass.colorAttachments[0].clearColor = MTLClearColorMake(1.0, 0.0, 1.0, 1.0)
        pass.depthAttachment.texture = depthTexture
        pass.depthAttachment.loadAction = .clear
        pass.depthAttachment.storeAction = .dontCare
        pass.depthAttachment.clearDepth = 1.0

        guard let commandBuffer = queue.makeCommandBuffer(),
              let encoder = commandBuffer.makeRenderCommandEncoder(descriptor: pass) else {
            return
        }

        if let sponzaPipeline = self.sponzaPipeline, !sponzaMeshes.isEmpty {
            encoder.setRenderPipelineState(sponzaPipeline)
            encoder.setDepthStencilState(depthState)
            encoder.setVertexBytes(&uniforms, length: MemoryLayout<NativeUniforms>.stride, index: 1)
            encoder.setFragmentBytes(&uniforms, length: MemoryLayout<NativeUniforms>.stride, index: 1)
            for entry in sponzaMeshes {
                for vb in entry.mesh.vertexBuffers {
                    encoder.setVertexBuffer(vb.buffer, offset: vb.offset, index: 0)
                }
                for sub in entry.submeshes {
                    let mtkSub = sub.mtkSubmesh
                    encoder.drawIndexedPrimitives(
                        type: mtkSub.primitiveType,
                        indexCount: mtkSub.indexCount,
                        indexType: mtkSub.indexType,
                        indexBuffer: mtkSub.indexBuffer.buffer,
                        indexBufferOffset: mtkSub.indexBuffer.offset
                    )
                }
            }
        } else if let pipeline = self.nativePipeline, let vertexBuffer = self.nativeVertexBuffer {
            // Fallback while Sponza is still loading (or failed to load):
            // draw the simple box arena instead of an empty magenta screen.
            encoder.setRenderPipelineState(pipeline)
            encoder.setDepthStencilState(depthState)
            encoder.setVertexBuffer(vertexBuffer, offset: 0, index: 0)
            encoder.setVertexBytes(&uniforms, length: MemoryLayout<NativeUniforms>.stride, index: 1)
            encoder.setFragmentBytes(&uniforms, length: MemoryLayout<NativeUniforms>.stride, index: 1)
            encoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: nativeVertexCount)
        }

        encoder.endEncoding()
        commandBuffer.present(drawable)
        commandBuffer.commit()
    }

    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        displayLink.isPaused = true
    }

    @objc func enterFrame() {
        if !USE_NATIVE_METAL_DEMO {
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
                touchHUD.updateKnob(dragX: CGFloat(dx), dragY: CGFloat(dy), maxDrag: CGFloat(maxDrag))
            }
            native_metal_probe_update_camera(probe, forward, strafe, lookDx, lookDy, deltaTime)
            native_metal_probe_render_frame(probe)
            syncHud()
            syncSettingsMenu()
            syncGameOver()
            return
        }
        self.renderNativeMetalFrame()
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
        if USE_REAL_GAME_NATIVE_RENDERER, let probe = self.rustMetalProbe {
            for touch in touches {
                let location = touch.location(in: self.metalV)
                native_metal_probe_touch_started(probe, touchId(for: touch), Float(location.x), Float(location.y))
            }
        }
        for touch in touches {
            let location = touch.location(in: self.metalV)
            // FIRE/AIM hit-test (see fireButton/aimButton's own
            // isUserInteractionEnabled=false comments) — checked
            // against self.view's coordinate space since that's where
            // TouchHUD laid the buttons out. Deliberately doesn't
            // `continue`/skip the isLeftHalf check below: a touch that
            // starts on FIRE/AIM is *also* eligible to become
            // lookTouch (it will, since both buttons sit in the right
            // half), so holding down and dragging fires/aims AND
            // rotates the camera with the same finger.
            let viewLocation = touch.location(in: self.view)
            if fireTouch == nil, touchHUD.fireButton.frame.contains(viewLocation) {
                fireTouch = touch
                handleFire(pressed: true)
            } else if aimTouch == nil, touchHUD.aimButton.frame.contains(viewLocation) {
                aimTouch = touch
                handleExtraButton(.aim, pressed: true)
            }
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
        if USE_REAL_GAME_NATIVE_RENDERER, let probe = self.rustMetalProbe {
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
                touchHUD.resetKnob()
            }
            if touch == lookTouch {
                lookTouch = nil
            }
            if touch == fireTouch {
                fireTouch = nil
                handleFire(pressed: false)
            }
            if touch == aimTouch {
                aimTouch = nil
                handleExtraButton(.aim, pressed: false)
            }
        }
    }

    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        if USE_REAL_GAME_NATIVE_RENDERER, let probe = self.rustMetalProbe {
            for touch in touches {
                let location = touch.location(in: self.metalV)
                native_metal_probe_touch_ended(probe, touchId(for: touch), Float(location.x), Float(location.y))
            }
        }
        endNativeTouch(touches)
    }

    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        if USE_REAL_GAME_NATIVE_RENDERER, let probe = self.rustMetalProbe {
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
