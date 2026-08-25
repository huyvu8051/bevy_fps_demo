//
//  MapSelectView.swift
//  client_iOS_CADisplayLink
//
//  Shown on a plain icon-tap launch (no MAP_ID env var already present in
//  the process environment) before the real game/renderer is created, so a
//  manual test session can pick a fixed scenario preset instead of always
//  landing in the normal game. Tapping a button calls setenv("MAP_ID", ...)
//  then lets ViewController proceed exactly as if that env var had been
//  there from the start — see ViewController.createGameProbeIfNeeded and
//  shared::Scenario (crates/shared/src/lib.rs) for what each map actually
//  spawns. A `devicectl process launch -e '{"MAP_ID":"1"}'` command-line
//  launch already has MAP_ID set before this view would even be created,
//  so ViewController skips ever showing it in that case.
//

import UIKit

final class MapSelectView: UIView {
    var onSelect: ((Int) -> Void)?

    private let titleLabel = UILabel()
    private var buttons: [UIButton] = []
    private var subtitleLabels: [UILabel] = []

    // (MAP_ID, button title, one-line description) — keep in sync with
    // Scenario::from_map_id (crates/shared/src/lib.rs).
    private let entries: [(id: Int, title: String, subtitle: String)] = [
        (0, "Map 0", "Bình thường"),
        (1, "Map 1", "Test hitbox — 5 zombie đứng yên, respawn 2s"),
        (2, "Map 2", "Test FPS — 50 zombie, noclip trên cao"),
    ]

    override init(frame: CGRect) {
        super.init(frame: frame)
        setup()
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        setup()
    }

    private func setup() {
        backgroundColor = .black
        isHidden = true

        titleLabel.text = "Chọn Map"
        titleLabel.textColor = .white
        titleLabel.font = .boldSystemFont(ofSize: 32)
        titleLabel.textAlignment = .center
        addSubview(titleLabel)

        for entry in entries {
            let button = UIButton(type: .system)
            button.tag = entry.id
            button.setTitle(entry.title, for: .normal)
            button.titleLabel?.font = .boldSystemFont(ofSize: 22)
            button.setTitleColor(.white, for: .normal)
            button.backgroundColor = UIColor(red: 0.2, green: 0.3, blue: 0.45, alpha: 1)
            button.layer.cornerRadius = 10
            button.addTarget(self, action: #selector(tapButton(_:)), for: .touchUpInside)
            addSubview(button)
            buttons.append(button)

            let subtitle = UILabel()
            subtitle.text = entry.subtitle
            subtitle.textColor = UIColor(white: 0.8, alpha: 1)
            subtitle.font = .systemFont(ofSize: 13)
            subtitle.textAlignment = .center
            subtitle.numberOfLines = 2
            addSubview(subtitle)
            subtitleLabels.append(subtitle)
        }
    }

    func layout(in parent: UIView) {
        frame = parent.bounds
        titleLabel.frame = CGRect(x: 0, y: bounds.height / 2 - 180, width: bounds.width, height: 50)

        let buttonWidth: CGFloat = 280
        let buttonHeight: CGFloat = 50
        let rowSpacing: CGFloat = 80
        let startY = bounds.height / 2 - 90
        for (index, button) in buttons.enumerated() {
            let y = startY + CGFloat(index) * rowSpacing
            button.frame = CGRect(x: (bounds.width - buttonWidth) / 2, y: y, width: buttonWidth, height: buttonHeight)
            subtitleLabels[index].frame = CGRect(x: 0, y: y + buttonHeight + 4, width: bounds.width, height: 32)
        }
    }

    @objc private func tapButton(_ sender: UIButton) {
        onSelect?(sender.tag)
    }
}
