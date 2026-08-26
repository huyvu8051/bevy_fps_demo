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
    private var lastFrameTime: CFTimeInterval = CACurrentMediaTime()
    // Native settings-menu overlay — see SettingsMenuView.swift's own doc
    // comment for why this exists instead of relying on bevy_ui.
    private let settingsMenu = SettingsMenuView()
    private var settingsMenuWasVisible = false
    // Native death/restart overlay — see GameOverView.swift's own doc
    // comment for why this exists instead of relying on bevy_ui.
    private let gameOverView = GameOverView()
    private var gameOverWasVisible = false
    // Native map-select overlay — see MapSelectView.swift's own doc
    // comment. Only shown on a plain icon-tap launch (no MAP_ID env var
    // already present); a `devicectl process launch -e '{"MAP_ID":"N"}'`
    // skips it entirely, same shape as the existing REAL_GAME/
    // MINIMAL_MAP_PROBE env-var launch overrides.
    private let mapSelectView = MapSelectView()
    private var mapSelectionPending = ProcessInfo.processInfo.environment["MAP_ID"] == nil
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

        self.view.addSubview(mapSelectView)
        mapSelectView.onSelect = { [weak self] mapId in
            guard let self else { return }
            // setenv, not a new FFI call — Rust reads MAP_ID directly (see
            // current_scenario() in crates/client/src/lib.rs and
            // scenario_from_env() in crates/server/src/main.rs), the exact
            // same env var a devicectl launch -e would already have set
            // before this process even started.
            setenv("MAP_ID", "\(mapId)", 1)
            self.mapSelectionPending = false
            self.mapSelectView.isHidden = true
            self.createGameProbeIfNeeded()
            self.displayLink.isPaused = false
        }
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        settingsMenu.layout(in: self.view)
        gameOverView.layout(in: self.view)
        mapSelectView.layout(in: self.view)
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

    // Polled once per frame (see enterFrame) — HP/ammo/ping/crosshair are
    // now drawn straight off real ECS state by native_metal_probe.rs (see
    // ViewController's own class-level doc comment), so this only still
    // exists for the Apple Watch companion sync below.
    private func syncHud() {
        guard USE_REAL_GAME_NATIVE_RENDERER, let probe = self.rustMetalProbe else { return }
        let hp = native_metal_probe_get_player_hp(probe)

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

    // Extracted out of viewDidAppear so mapSelectView's tap handler can
    // call it too, once a map has actually been picked. Safe to call more
    // than once — the rustMetalProbe == nil guard makes every call after
    // the first a no-op.
    private func createGameProbeIfNeeded() {
        guard rustMetalProbe == nil else { return }
        let viewPointer = Unmanaged.passUnretained(self.metalV).toOpaque()
        // The probe's own standalone Sponza/box-arena/patrol demo World
        // (native_metal_probe_create, real_game=false) was removed from the
        // Rust side once confirmed unreachable here — USE_REAL_GAME_NATIVE_RENDERER
        // is always true in this build; see git history if a standalone
        // demo/probe entry point is ever worth resurrecting.
        if USE_REAL_GAME_NATIVE_RENDERER {
            rustMetalProbe = native_metal_probe_create_real_game(viewPointer, Float(UIScreen.main.nativeScale))
        }
        if #available(iOS 15.0, *) {
            displayLink.preferredFrameRateRange = CAFrameRateRange(minimum: 80, maximum: 120, preferred: 120)
        }
        if #available(iOS 14.0, *), gameControllerInput == nil {
            gameControllerInput = GameControllerInput(viewController: self)
        }
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        self.view.backgroundColor = .black
        if USE_NATIVE_METAL_DEMO {
            if metalDevice == nil {
                self.startNativeMetalDemo()
            }
        } else if mapSelectionPending {
            // Show the map-select menu instead of creating the probe —
            // createGameProbeIfNeeded() runs from mapSelectView.onSelect
            // (viewDidLoad) once the player actually picks one. Don't
            // unpause the display link yet either: nothing exists to
            // render/tick until a map's chosen.
            mapSelectView.isHidden = false
            mapSelectView.layout(in: self.view)
            return
        } else {
            createGameProbeIfNeeded()
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
            }
            if touch == lookTouch {
                lookTouch = nil
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
