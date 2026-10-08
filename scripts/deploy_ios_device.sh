#!/usr/bin/env bash
# Build the client crate for a real iOS device and deploy+launch it —
# one command. Mirrors the manual steps used throughout development:
# cargo build -> copy the static lib into the Xcode project's expected
# location (two filenames, see the .gitignore comment on iOS/libs/) ->
# xcodebuild for the connected device -> devicectl install + launch.
#
# Two things measured to actually dominate iteration time here (see
# README.md's "Build nhanh hơn khi lặp lại" section for the numbers):
# 1. `dev-fast` cargo profile (line-tables-only debug info + stripped) —
#    roughly halves the linked binary size, which roughly halves
#    xcodebuild's CodeSign step (proportional to binary size: ~19s -> ~9s
#    measured). Full `dev` profile (real LLDB debugging) is still one
#    `cargo build --profile dev` away when actually needed.
# 2. Reusing derived data across runs instead of `rm -rf`-ing it every
#    time — lets xcodebuild's own incremental build apply (~13s saved
#    measured). Pass --clean as the first arg to force a fresh one if
#    something looks stale/wrong.
#
# Usage: scripts/deploy_ios_device.sh [--clean] [device-name-substring]
#   With no name given, deploys to the first connected physical iOS device
#   Xcode can see. Pass a substring (e.g. "Huy") to pick a specific one
#   when more than one is connected.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IOS_PROJECT_DIR="$REPO_ROOT/crates/client/gen/apple/iOS"
BUNDLE_ID="com.huyvu8051.bevyfpsdemo"
DERIVED_DATA="/tmp/dd_deploy_ios_device"
CLEAN=0
if [ "${1:-}" = "--clean" ]; then
    CLEAN=1
    shift
fi
NAME_FILTER="${1:-}"

echo "==> cargo build -p client --target aarch64-apple-ios --profile dev-fast"
cd "$REPO_ROOT"
cargo build -p client --target aarch64-apple-ios --profile dev-fast

echo "==> Copying libclient.a into the Xcode project"
LIB_SRC="$REPO_ROOT/target/aarch64-apple-ios/dev-fast/libclient.a"
LIB_DIR="$IOS_PROJECT_DIR/libs/debug"
mkdir -p "$LIB_DIR"
cp "$LIB_SRC" "$LIB_DIR/libclient.a"
cp "$LIB_SRC" "$LIB_DIR/libbevy_in_app.a"

echo "==> Finding a connected iOS device"
cd "$IOS_PROJECT_DIR"
DESTINATIONS="$(xcodebuild -project bevy_in_iOS.xcodeproj -scheme bevy_in_iOS -showdestinations 2>/dev/null)"
DEVICE_LINE="$(echo "$DESTINATIONS" | grep -E 'platform:iOS,' | grep -v 'placeholder' | grep -i "$NAME_FILTER" | head -1 || true)"
if [ -z "$DEVICE_LINE" ]; then
    echo "No connected iOS device found (plug it in, unlock it, trust this computer)." >&2
    echo "$DESTINATIONS" | grep -E 'platform:iOS,' >&2 || true
    exit 1
fi
DEVICE_ID="$(echo "$DEVICE_LINE" | sed -n 's/.*id:\([^,]*\),.*/\1/p')"
DEVICE_NAME="$(echo "$DEVICE_LINE" | sed -n 's/.*name:\([^}]*\)}.*/\1/p')"
echo "    -> $DEVICE_NAME ($DEVICE_ID)"

echo "==> xcodebuild (device)"
[ "$CLEAN" = 1 ] && rm -rf "$DERIVED_DATA"
xcodebuild -project bevy_in_iOS.xcodeproj -scheme bevy_in_iOS -configuration Debug \
    -destination "id=$DEVICE_ID" -derivedDataPath "$DERIVED_DATA" -allowProvisioningUpdates build

APP_PATH="$DERIVED_DATA/Build/Products/Debug-iphoneos/bevy_in_iOS.app"

echo "==> Installing on device"
xcrun devicectl device install app --device "$DEVICE_ID" "$APP_PATH"

echo "==> Launching"
xcrun devicectl device process launch --device "$DEVICE_ID" "$BUNDLE_ID" || {
    echo "Install succeeded but launch failed — if the device is locked, unlock it and launch the app by hand." >&2
    exit 1
}

echo "==> Done."
