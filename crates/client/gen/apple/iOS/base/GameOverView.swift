//
//  GameOverView.swift
//  client_iOS_CADisplayLink
//
//  Native "GAME OVER" / restart overlay. The real game's own death screen
//  (`check_game_over`'s GameOverUi spawn in crates/client/src/lib.rs) is a
//  bevy_ui Node hierarchy, invisible under our custom Metal renderer for
//  the same reason the settings menu was — see SettingsMenuView.swift's
//  own doc comment. ViewController polls `native_metal_probe_game_over`
//  once per frame (see `syncGameOver`) and shows/hides this view;
//  `Restart` fires `native_metal_probe_trigger_restart`, the same
//  server-authoritative `RequestRestart` client event the real desktop
//  "Press R" / touch RestartButton both already use.

import UIKit

final class GameOverView: UIView {
    private let titleLabel = UILabel()
    private let restartButton = UIButton(type: .system)

    var onRestart: (() -> Void)?

    override init(frame: CGRect) {
        super.init(frame: frame)
        setup()
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        setup()
    }

    private func setup() {
        backgroundColor = UIColor.black.withAlphaComponent(0.75)
        isHidden = true

        titleLabel.text = "GAME OVER"
        titleLabel.textColor = UIColor(red: 0.9, green: 0.2, blue: 0.2, alpha: 1)
        titleLabel.font = .boldSystemFont(ofSize: 40)
        titleLabel.textAlignment = .center
        addSubview(titleLabel)

        restartButton.setTitle("Restart", for: .normal)
        restartButton.setTitleColor(.white, for: .normal)
        restartButton.titleLabel?.font = .boldSystemFont(ofSize: 20)
        restartButton.backgroundColor = UIColor(red: 0.2, green: 0.45, blue: 0.2, alpha: 1)
        restartButton.layer.cornerRadius = 10
        restartButton.addTarget(self, action: #selector(tapRestart), for: .touchUpInside)
        addSubview(restartButton)
    }

    func layout(in parent: UIView) {
        frame = parent.bounds
        let width: CGFloat = 220
        titleLabel.frame = CGRect(x: (bounds.width - 320) / 2, y: bounds.height / 2 - 80, width: 320, height: 50)
        restartButton.frame = CGRect(x: (bounds.width - width) / 2, y: bounds.height / 2, width: width, height: 50)
    }

    @objc private func tapRestart() { onRestart?() }
}
