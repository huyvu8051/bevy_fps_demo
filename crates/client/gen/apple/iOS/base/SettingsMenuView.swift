//
//  SettingsMenuView.swift
//  client_iOS_CADisplayLink
//
//  Native UIKit settings-menu overlay. The real game's own settings menu
//  (`spawn_settings_menu` in crates/client/src/lib.rs) is a `bevy_ui` Node
//  hierarchy — invisible under our custom Metal renderer, which only ever
//  issues Mesh3d/MeshMaterial3d draw calls and never touches bevy_ui at
//  all. Per the plan's own Phase 4 guidance ("drive native UIKit views
//  instead of reimplementing bevy_ui"), this view mirrors the same real
//  ECS state via the native_metal_probe_* FFI functions declared in
//  libbevy_in_app.h (Rust side: crates/client/src/cadisplaylink/
//  native_menu.rs) rather than owning any state of its own — ViewController
//  polls `native_metal_probe_menu_visible` once per frame (see
//  `enterFrame`/`syncSettingsMenu`) so this view stays correct regardless
//  of which input toggled it (touch MENU button or a hardware F1 key via
//  GameControllerInput.swift).
//
//  Paged like the real bevy_ui menu it replaces (spawn_settings_menu's own
//  "<" / "N/M" / ">" nav row + SettingsMenuPage(0)/(1) containers), split
//  further than that 2-page original into 4 shorter pages so no single
//  page is a wall of buttons:
//    1. Settings   — HP, Sensitivity (always present)
//    2. Cheats     — God Mode/Noclip/Freeze + Kill All/Spawn +100
//    3. Zombies    — count -10/n/+10, Heal Nearest/1HP Nearest
//    4. Visual     — Gun/MSAA/Shadow, Hitboxes/AimBlur/PlayerMdl,
//                    NoZAnim/NoHPBar/FullLOD
//  Pages 2-4 are debug-only (`#[cfg(debug_assertions)]`-gated on the Rust
//  side too), matching spawn_settings_menu's own cheat-section gate.

import UIKit

final class SettingsMenuView: UIView {
    private let card = UIView()
    private let titleLabel = UILabel()
    private let pageLabel = UILabel()
    private let prevPageButton = UIButton(type: .system)
    private let nextPageButton = UIButton(type: .system)
    private let closeButton = UIButton(type: .system)

    // Page 1: player-facing settings — always present, even in release.
    private let settingsPage = UIView()
    private let hpLabel = UILabel()
    private let sensitivityLabel = UILabel()
    private let sensitivityDecButton = UIButton(type: .system)
    private let sensitivityIncButton = UIButton(type: .system)

    #if DEBUG
    // Page 2: God Mode/Noclip/Freeze + Kill All/Spawn +100.
    private let cheatsPage = UIView()
    private let godModeButton = ToggleRow(title: "God Mode")
    private let noclipButton = ToggleRow(title: "Noclip")
    private let freezeButton = ToggleRow(title: "Freeze Zombies")
    private let killAllButton = UIButton(type: .system)
    private let spawnMoreButton = UIButton(type: .system)

    // Page 3: zombie count + Heal Nearest/1HP Nearest.
    private let zombiesPage = UIView()
    private let zombieCountLabel = UILabel()
    private let zombieCountDecButton = UIButton(type: .system)
    private let zombieCountIncButton = UIButton(type: .system)
    private let healNearestButton = UIButton(type: .system)
    private let nearDeathNearestButton = UIButton(type: .system)

    // Page 4: visual/perf debug toggles — compact 3-per-row tiles, same
    // grouping as spawn_settings_menu's own three toggle rows.
    private let visualPage = UIView()
    private let gunButton = ToggleRow(title: "Gun")
    private let msaaButton = ToggleRow(title: "MSAA")
    private let shadowButton = ToggleRow(title: "Shadow")
    private let hitboxesButton = ToggleRow(title: "Hitbox")
    private let aimBlurButton = ToggleRow(title: "AimBlur")
    private let playerModelButton = ToggleRow(title: "PlyrMdl")
    private let noZombieAnimButton = ToggleRow(title: "NoZAnim")
    private let noHpBarButton = ToggleRow(title: "NoHPBar")
    private let fullLodButton = ToggleRow(title: "FullLOD")

    private let pageCount = 4
    #else
    private let pageCount = 1
    #endif

    private var currentPage = 0

    var onClose: (() -> Void)?
    var onSensitivityDec: (() -> Void)?
    var onSensitivityInc: (() -> Void)?
    #if DEBUG
    var onToggleGodMode: (() -> Void)?
    var onToggleNoclip: (() -> Void)?
    var onToggleFreeze: (() -> Void)?
    var onKillAll: (() -> Void)?
    var onSpawnMore: (() -> Void)?
    var onZombieCountDec: (() -> Void)?
    var onZombieCountInc: (() -> Void)?
    var onHealNearest: (() -> Void)?
    var onNearDeathNearest: (() -> Void)?
    var onToggleGun: (() -> Void)?
    var onToggleMsaa: (() -> Void)?
    var onToggleShadow: (() -> Void)?
    var onToggleHitboxes: (() -> Void)?
    var onToggleAimBlur: (() -> Void)?
    var onTogglePlayerModel: (() -> Void)?
    var onToggleNoZombieAnim: (() -> Void)?
    var onToggleNoHpBar: (() -> Void)?
    var onToggleFullLod: (() -> Void)?
    #endif

    override init(frame: CGRect) {
        super.init(frame: frame)
        setup()
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        setup()
    }

    private func setup() {
        backgroundColor = UIColor.black.withAlphaComponent(0.55)
        isHidden = true

        card.backgroundColor = UIColor(white: 0.12, alpha: 0.96)
        card.layer.cornerRadius = 16
        card.layer.borderWidth = 1
        card.layer.borderColor = UIColor(white: 0.35, alpha: 1).cgColor
        addSubview(card)

        titleLabel.text = "SETTINGS"
        titleLabel.textColor = .white
        titleLabel.font = .boldSystemFont(ofSize: 20)
        card.addSubview(titleLabel)

        configureFlatButton(prevPageButton, title: "<")
        prevPageButton.addTarget(self, action: #selector(tapPrevPage), for: .touchUpInside)
        card.addSubview(prevPageButton)

        pageLabel.textColor = .white
        pageLabel.font = .systemFont(ofSize: 15)
        pageLabel.textAlignment = .center
        card.addSubview(pageLabel)

        configureFlatButton(nextPageButton, title: ">")
        nextPageButton.addTarget(self, action: #selector(tapNextPage), for: .touchUpInside)
        card.addSubview(nextPageButton)
        // Only one page in a release build — the nav row would just toggle
        // between "page 1" and itself, so hide it entirely instead.
        prevPageButton.isHidden = pageCount <= 1
        nextPageButton.isHidden = pageCount <= 1
        pageLabel.isHidden = pageCount <= 1

        card.addSubview(settingsPage)
        hpLabel.textColor = UIColor(white: 0.85, alpha: 1)
        hpLabel.font = .systemFont(ofSize: 15)
        settingsPage.addSubview(hpLabel)

        sensitivityLabel.textColor = .white
        sensitivityLabel.font = .systemFont(ofSize: 16)
        sensitivityLabel.textAlignment = .center
        settingsPage.addSubview(sensitivityLabel)

        configureFlatButton(sensitivityDecButton, title: "−")
        sensitivityDecButton.addTarget(self, action: #selector(tapSensitivityDec), for: .touchUpInside)
        settingsPage.addSubview(sensitivityDecButton)

        configureFlatButton(sensitivityIncButton, title: "+")
        sensitivityIncButton.addTarget(self, action: #selector(tapSensitivityInc), for: .touchUpInside)
        settingsPage.addSubview(sensitivityIncButton)

        #if DEBUG
        card.addSubview(cheatsPage)
        cheatsPage.isHidden = true
        godModeButton.addTarget(self, action: #selector(tapGodMode))
        noclipButton.addTarget(self, action: #selector(tapNoclip))
        freezeButton.addTarget(self, action: #selector(tapFreeze))
        cheatsPage.addSubview(godModeButton)
        cheatsPage.addSubview(noclipButton)
        cheatsPage.addSubview(freezeButton)
        configureFlatButton(killAllButton, title: "Kill All")
        killAllButton.backgroundColor = UIColor(red: 0.5, green: 0.15, blue: 0.15, alpha: 1)
        killAllButton.addTarget(self, action: #selector(tapKillAll), for: .touchUpInside)
        cheatsPage.addSubview(killAllButton)
        configureFlatButton(spawnMoreButton, title: "Spawn +100")
        spawnMoreButton.addTarget(self, action: #selector(tapSpawnMore), for: .touchUpInside)
        cheatsPage.addSubview(spawnMoreButton)

        card.addSubview(zombiesPage)
        zombiesPage.isHidden = true
        zombieCountLabel.textColor = .white
        zombieCountLabel.font = .systemFont(ofSize: 16)
        zombieCountLabel.textAlignment = .center
        zombiesPage.addSubview(zombieCountLabel)
        configureFlatButton(zombieCountDecButton, title: "−10")
        zombieCountDecButton.addTarget(self, action: #selector(tapZombieCountDec), for: .touchUpInside)
        zombiesPage.addSubview(zombieCountDecButton)
        configureFlatButton(zombieCountIncButton, title: "+10")
        zombieCountIncButton.addTarget(self, action: #selector(tapZombieCountInc), for: .touchUpInside)
        zombiesPage.addSubview(zombieCountIncButton)
        configureFlatButton(healNearestButton, title: "Heal Nearest")
        healNearestButton.addTarget(self, action: #selector(tapHealNearest), for: .touchUpInside)
        zombiesPage.addSubview(healNearestButton)
        configureFlatButton(nearDeathNearestButton, title: "1HP Nearest")
        nearDeathNearestButton.addTarget(self, action: #selector(tapNearDeathNearest), for: .touchUpInside)
        zombiesPage.addSubview(nearDeathNearestButton)

        card.addSubview(visualPage)
        visualPage.isHidden = true
        gunButton.addTarget(self, action: #selector(tapGun))
        msaaButton.addTarget(self, action: #selector(tapMsaa))
        shadowButton.addTarget(self, action: #selector(tapShadow))
        hitboxesButton.addTarget(self, action: #selector(tapHitboxes))
        aimBlurButton.addTarget(self, action: #selector(tapAimBlur))
        playerModelButton.addTarget(self, action: #selector(tapPlayerModel))
        noZombieAnimButton.addTarget(self, action: #selector(tapNoZombieAnim))
        noHpBarButton.addTarget(self, action: #selector(tapNoHpBar))
        fullLodButton.addTarget(self, action: #selector(tapFullLod))
        for tile in [gunButton, msaaButton, shadowButton, hitboxesButton, aimBlurButton, playerModelButton, noZombieAnimButton, noHpBarButton, fullLodButton] {
            visualPage.addSubview(tile)
        }
        #endif

        configureFlatButton(closeButton, title: "Close")
        closeButton.backgroundColor = UIColor(red: 0.2, green: 0.45, blue: 0.2, alpha: 1)
        closeButton.addTarget(self, action: #selector(tapClose), for: .touchUpInside)
        card.addSubview(closeButton)

        // Tapping the dimmed backdrop (not the card itself) also closes the
        // menu, matching the usual modal-sheet convention.
        let backdropTap = UITapGestureRecognizer(target: self, action: #selector(tapBackdrop))
        addGestureRecognizer(backdropTap)

        updatePageVisibility()
    }

    private func configureFlatButton(_ button: UIButton, title: String) {
        button.setTitle(title, for: .normal)
        button.setTitleColor(.white, for: .normal)
        button.titleLabel?.font = .boldSystemFont(ofSize: 16)
        button.backgroundColor = UIColor(white: 0.25, alpha: 1)
        button.layer.cornerRadius = 8
    }

    private func updatePageVisibility() {
        settingsPage.isHidden = currentPage != 0
        #if DEBUG
        cheatsPage.isHidden = currentPage != 1
        zombiesPage.isHidden = currentPage != 2
        visualPage.isHidden = currentPage != 3
        #endif
        pageLabel.text = "\(currentPage + 1)/\(pageCount)"
    }

    // MARK: layout — plain manual frame math (same style as TouchHUD.layout,
    // no Auto Layout elsewhere in this codebase), recomputed whenever the
    // menu becomes visible or the parent view resizes.
    func layout(in parent: UIView) {
        frame = parent.bounds
        let cardHeight: CGFloat = 260
        let cardWidth: CGFloat = min(380, parent.bounds.width - 60)
        card.frame = CGRect(
            x: (parent.bounds.width - cardWidth) / 2,
            y: (parent.bounds.height - cardHeight) / 2,
            width: cardWidth, height: cardHeight
        )

        let pad: CGFloat = 20
        var y: CGFloat = pad
        titleLabel.frame = CGRect(x: pad, y: y, width: cardWidth - pad * 2 - 100, height: 26)
        // Nav row shares the title's row so paging doesn't cost extra
        // vertical space.
        nextPageButton.frame = CGRect(x: cardWidth - pad - 36, y: y - 5, width: 36, height: 36)
        pageLabel.frame = CGRect(x: cardWidth - pad - 100, y: y, width: 56, height: 26)
        prevPageButton.frame = CGRect(x: cardWidth - pad - 136, y: y - 5, width: 36, height: 36)
        y += 44

        let contentTop = y
        let contentHeight: CGFloat = 128
        let contentWidth = cardWidth

        settingsPage.frame = CGRect(x: 0, y: contentTop, width: contentWidth, height: contentHeight)
        var sy: CGFloat = 0
        hpLabel.frame = CGRect(x: pad, y: sy, width: contentWidth - pad * 2, height: 20)
        sy += 40
        sensitivityDecButton.frame = CGRect(x: pad, y: sy, width: 44, height: 40)
        sensitivityIncButton.frame = CGRect(x: contentWidth - pad - 44, y: sy, width: 44, height: 40)
        sensitivityLabel.frame = CGRect(x: pad + 52, y: sy, width: contentWidth - pad * 2 - 104, height: 40)

        #if DEBUG
        cheatsPage.frame = CGRect(x: 0, y: contentTop, width: contentWidth, height: contentHeight)
        var cy: CGFloat = 0
        let rowHeight: CGFloat = 36
        godModeButton.frame = CGRect(x: pad, y: cy, width: contentWidth - pad * 2, height: rowHeight)
        cy += rowHeight + 6
        let halfWidth = (contentWidth - pad * 2 - 8) / 2
        noclipButton.frame = CGRect(x: pad, y: cy, width: halfWidth, height: rowHeight)
        freezeButton.frame = CGRect(x: pad + halfWidth + 8, y: cy, width: halfWidth, height: rowHeight)
        cy += rowHeight + 6
        killAllButton.frame = CGRect(x: pad, y: cy, width: halfWidth, height: rowHeight)
        spawnMoreButton.frame = CGRect(x: pad + halfWidth + 8, y: cy, width: halfWidth, height: rowHeight)

        zombiesPage.frame = CGRect(x: 0, y: contentTop, width: contentWidth, height: contentHeight)
        var zy: CGFloat = 0
        zombieCountDecButton.frame = CGRect(x: pad, y: zy, width: 50, height: 40)
        zombieCountIncButton.frame = CGRect(x: contentWidth - pad - 50, y: zy, width: 50, height: 40)
        zombieCountLabel.frame = CGRect(x: pad + 58, y: zy, width: contentWidth - pad * 2 - 116, height: 40)
        zy += 52
        healNearestButton.frame = CGRect(x: pad, y: zy, width: halfWidth, height: rowHeight)
        nearDeathNearestButton.frame = CGRect(x: pad + halfWidth + 8, y: zy, width: halfWidth, height: rowHeight)

        visualPage.frame = CGRect(x: 0, y: contentTop, width: contentWidth, height: contentHeight)
        let tileGap: CGFloat = 8
        let tileWidth = (contentWidth - pad * 2 - tileGap * 2) / 3
        let tileHeight: CGFloat = 34
        let visualRows: [[ToggleRow]] = [
            [gunButton, msaaButton, shadowButton],
            [hitboxesButton, aimBlurButton, playerModelButton],
            [noZombieAnimButton, noHpBarButton, fullLodButton],
        ]
        var vy: CGFloat = 0
        for row in visualRows {
            for (i, tile) in row.enumerated() {
                tile.frame = CGRect(x: pad + CGFloat(i) * (tileWidth + tileGap), y: vy, width: tileWidth, height: tileHeight)
            }
            vy += tileHeight + 6
        }
        #endif

        y = contentTop + contentHeight + 8
        closeButton.frame = CGRect(x: pad, y: y, width: cardWidth - pad * 2, height: 44)
    }

    // MARK: state refresh — called by ViewController once per frame while
    // visible (cheap: a handful of FFI getter calls, not a full re-layout).
    func refresh(
        sensitivity: Float, playerHp: Float,
        godMode: Bool, noclip: Bool, freezeZombies: Bool,
        zombieLiveCount: UInt32, zombieTargetCount: UInt32,
        gunVisible: Bool, mapMsaa: Bool, mapShadows: Bool,
        showHitboxes: Bool, aimBlur: Bool, playerModelVisible: Bool,
        zombieAnimDisabled: Bool, zombieHealthBarsHidden: Bool, zombieLodDisabled: Bool
    ) {
        sensitivityLabel.text = String(format: "Sensitivity: %.2fx", sensitivity)
        if playerHp >= 0 {
            hpLabel.text = String(format: "HP: %.0f", playerHp)
        } else {
            hpLabel.text = "HP: —"
        }
        #if DEBUG
        godModeButton.setOn(godMode)
        noclipButton.setOn(noclip)
        freezeButton.setOn(freezeZombies)
        // Two different numbers on purpose: "Alive" is the real, current
        // headcount (updates instantly for Spawn +100/Kill All/deaths);
        // "Target" is just what the -10/+10 buttons below set the
        // auto-maintain population goal to — was showing only the latter
        // labeled plain "Zombies", which never moved for Spawn +100 and
        // read as a broken/wrong counter.
        zombieCountLabel.text = "Alive: \(zombieLiveCount)  Target: \(zombieTargetCount)"
        gunButton.setOn(gunVisible)
        msaaButton.setOn(mapMsaa)
        shadowButton.setOn(mapShadows)
        hitboxesButton.setOn(showHitboxes)
        aimBlurButton.setOn(aimBlur)
        playerModelButton.setOn(playerModelVisible)
        noZombieAnimButton.setOn(zombieAnimDisabled)
        noHpBarButton.setOn(zombieHealthBarsHidden)
        fullLodButton.setOn(zombieLodDisabled)
        #endif
    }

    @objc private func tapClose() { onClose?() }
    @objc private func tapBackdrop(_ gesture: UITapGestureRecognizer) {
        let point = gesture.location(in: self)
        if !card.frame.contains(point) {
            onClose?()
        }
    }
    @objc private func tapPrevPage() {
        currentPage = (currentPage + pageCount - 1) % pageCount
        updatePageVisibility()
    }
    @objc private func tapNextPage() {
        currentPage = (currentPage + 1) % pageCount
        updatePageVisibility()
    }
    @objc private func tapSensitivityDec() { onSensitivityDec?() }
    @objc private func tapSensitivityInc() { onSensitivityInc?() }
    #if DEBUG
    @objc private func tapGodMode() { onToggleGodMode?() }
    @objc private func tapNoclip() { onToggleNoclip?() }
    @objc private func tapFreeze() { onToggleFreeze?() }
    @objc private func tapKillAll() { onKillAll?() }
    @objc private func tapSpawnMore() { onSpawnMore?() }
    @objc private func tapZombieCountDec() { onZombieCountDec?() }
    @objc private func tapZombieCountInc() { onZombieCountInc?() }
    @objc private func tapHealNearest() { onHealNearest?() }
    @objc private func tapNearDeathNearest() { onNearDeathNearest?() }
    @objc private func tapGun() { onToggleGun?() }
    @objc private func tapMsaa() { onToggleMsaa?() }
    @objc private func tapShadow() { onToggleShadow?() }
    @objc private func tapHitboxes() { onToggleHitboxes?() }
    @objc private func tapAimBlur() { onToggleAimBlur?() }
    @objc private func tapPlayerModel() { onTogglePlayerModel?() }
    @objc private func tapNoZombieAnim() { onToggleNoZombieAnim?() }
    @objc private func tapNoHpBar() { onToggleNoHpBar?() }
    @objc private func tapFullLod() { onToggleFullLod?() }
    #endif
}

#if DEBUG
// Simple label+switch-look toggle row (a UIButton whose background color
// signals on/off — avoids pulling in UISwitch's own default styling, which
// looks out of place next to the flat game-HUD buttons elsewhere).
private final class ToggleRow: UIButton {
    private let titleText: String
    private var isOn = false

    init(title: String) {
        self.titleText = title
        super.init(frame: .zero)
        titleLabel?.font = .systemFont(ofSize: 13)
        titleLabel?.adjustsFontSizeToFitWidth = true
        setTitleColor(.white, for: .normal)
        layer.cornerRadius = 8
        setOn(false)
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    func setOn(_ on: Bool) {
        isOn = on
        backgroundColor = on ? UIColor(red: 0.2, green: 0.5, blue: 0.25, alpha: 1) : UIColor(white: 0.25, alpha: 1)
        setTitle("\(titleText): \(on ? "ON" : "OFF")", for: .normal)
    }

    func addTarget(_ target: Any?, action: Selector) {
        addTarget(target, action: action, for: .touchUpInside)
    }
}
#endif
