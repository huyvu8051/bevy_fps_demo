#!/usr/bin/env ruby
# One-shot script (not part of the build) that adds the "bevy_watch Watch
# App" watchOS target to bevy_in_iOS.xcodeproj and embeds it into the iOS
# app target. There's no xcodegen project.yml driving the active
# gen/apple project (see gen/apple_winit_legacy for the old xcodegen-based
# approach, now unused) — this project's pbxproj is hand-maintained, so
# this script exists to make that one-time edit reproducible/reviewable
# via git diff instead of an opaque "clicked around in Xcode" commit.
# Safe to re-run: it's a no-op if the watch target already exists.
require 'xcodeproj'

project_path = File.join(__dir__, 'iOS', 'bevy_in_iOS.xcodeproj')
project = Xcodeproj::Project.open(project_path)

ios_target = project.targets.find { |t| t.name == 'bevy_in_iOS' }
raise "iOS target not found" unless ios_target

if project.targets.find { |t| t.name == 'bevy_watch Watch App' }
  puts "watch target already exists, nothing to do"
  exit 0
end

watch_dir = File.join(__dir__, 'watchOS', 'bevy_watch_Watch_App')

# --- watch app target -------------------------------------------------
watch_target = project.new_target(:application, 'bevy_watch Watch App', :watchos, '10.0')

watch_group = project.main_group.new_group('watchOS', '../watchOS/bevy_watch_Watch_App')
%w[bevy_watchApp.swift ContentView.swift WatchConnectivityManager.swift].each do |name|
  ref = watch_group.new_reference(File.join(watch_dir, name))
  watch_target.add_file_references([ref])
end
assets_ref = watch_group.new_reference(File.join(watch_dir, 'Assets.xcassets'))
watch_target.resources_build_phase.add_file_reference(assets_ref)

watch_target.build_configurations.each do |config|
  # Relative to SRCROOT, which xcodebuild resolves as the iOS/ project
  # dir (same as bevy_in_iOS's own INFOPLIST_FILE = bevy_in_iOS/Info.plist)
  # — the watch app's sources live one level up, as a sibling of iOS/.
  config.build_settings['INFOPLIST_FILE'] = '../watchOS/bevy_watch_Watch_App/Info.plist'
  config.build_settings['PRODUCT_BUNDLE_IDENTIFIER'] = 'com.huyvu8051.bevyfpsdemo.watchkitapp'
  config.build_settings['PRODUCT_NAME'] = '$(TARGET_NAME)'
  config.build_settings['DEVELOPMENT_TEAM'] = 'GNNA6PH896'
  config.build_settings['CODE_SIGN_STYLE'] = 'Automatic'
  config.build_settings['SWIFT_VERSION'] = '5.0'
  config.build_settings['WATCHOS_DEPLOYMENT_TARGET'] = '10.0'
  config.build_settings['SDKROOT'] = 'watchos'
  config.build_settings['TARGETED_DEVICE_FAMILY'] = '4'
  config.build_settings['SKIP_INSTALL'] = 'YES'
  config.build_settings['CURRENT_PROJECT_VERSION'] = '1.0'
  config.build_settings['MARKETING_VERSION'] = '1.0'
  config.build_settings['ASSETCATALOG_COMPILER_APPICON_NAME'] = 'AppIcon'
  config.build_settings['ASSETCATALOG_COMPILER_GLOBAL_ACCENT_COLOR_NAME'] = 'AccentColor'
  config.build_settings['ENABLE_PREVIEWS'] = 'YES'
end

# --- embed watch app into the iOS target -------------------------------
embed_phase = ios_target.new_copy_files_build_phase('Embed Watch Content')
embed_phase.dst_subfolder_spec = '16' # Products Directory
embed_phase.dst_path = '$(CONTENTS_FOLDER_PATH)/Watch'
build_file = embed_phase.add_file_reference(watch_target.product_reference)
build_file.settings = { 'ATTRIBUTES' => ['RemoveHeaderOnCopy'] }

ios_target.add_dependency(watch_target)

project.save
puts "Added watch target and embedded it into bevy_in_iOS"
