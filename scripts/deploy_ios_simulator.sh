#!/usr/bin/env bash
# Build the client crate for the iOS Simulator and deploy+launch it — one
# command. Mirrors scripts/deploy_ios_device.sh's steps, adapted for
# simctl instead of devicectl: cargo build -> copy the static lib into the
# Xcode project's expected location (two filenames, see the .gitignore
# comment on iOS/libs/) -> xcodebuild for the simulator -> simctl
# install + launch (booting the simulator first if it isn't already).
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
# Usage: scripts/deploy_ios_simulator.sh [--clean] [device-name-substring]
#   With no name given, deploys to the first booted Simulator, or if none
#   is booted, the first available "iPhone" Simulator (booting it). Pass a
#   substring (e.g. "17 Pro") to pick a specific one when several exist.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IOS_PROJECT_DIR="$REPO_ROOT/crates/client/gen/apple/iOS"
BUNDLE_ID="com.huyvu8051.bevyfpsdemo"
DERIVED_DATA="/tmp/dd_deploy_ios_simulator"
CLEAN=0
if [ "${1:-}" = "--clean" ]; then
    CLEAN=1
    shift
fi
NAME_FILTER="${1:-iPhone}"

echo "==> cargo build -p client --target aarch64-apple-ios-sim --profile dev-fast"
cd "$REPO_ROOT"
cargo build -p client --target aarch64-apple-ios-sim --profile dev-fast

echo "==> Copying libclient.a into the Xcode project"
LIB_SRC="$REPO_ROOT/target/aarch64-apple-ios-sim/dev-fast/libclient.a"
LIB_DIR="$IOS_PROJECT_DIR/libs/debug"
mkdir -p "$LIB_DIR"
cp "$LIB_SRC" "$LIB_DIR/libclient.a"
cp "$LIB_SRC" "$LIB_DIR/libbevy_in_app.a"

echo "==> Finding a Simulator device"
DEVICES_JSON="$(xcrun simctl list devices -j)"
# Prefer an already-booted device matching the filter (avoids interrupting
# whatever's currently running on it); fall back to the first available
# (not unavailable) device matching the filter, which gets booted below.
DEVICE_LINE="$(xcrun simctl list devices | grep -i "$NAME_FILTER" | grep -v 'unavailable' | grep 'Booted' | head -1 || true)"
if [ -z "$DEVICE_LINE" ]; then
    DEVICE_LINE="$(xcrun simctl list devices | grep -i "$NAME_FILTER" | grep -v 'unavailable' | head -1 || true)"
fi
if [ -z "$DEVICE_LINE" ]; then
    echo "No Simulator device found matching '$NAME_FILTER'." >&2
    xcrun simctl list devices >&2
    exit 1
fi
DEVICE_ID="$(echo "$DEVICE_LINE" | sed -n 's/.*(\([0-9A-F-]\{36\}\)).*/\1/p')"
DEVICE_NAME="$(echo "$DEVICE_LINE" | sed -n 's/^ *\(.*\) (.*/\1/p')"
if [ -z "$DEVICE_ID" ]; then
    echo "Couldn't parse a device UDID out of: $DEVICE_LINE" >&2
    exit 1
fi
echo "    -> $DEVICE_NAME ($DEVICE_ID)"

echo "==> Booting Simulator (no-op if already booted)"
xcrun simctl boot "$DEVICE_ID" 2>/dev/null || true
open -a Simulator --args -CurrentDeviceUDID "$DEVICE_ID"
xcrun simctl bootstatus "$DEVICE_ID" -b

echo "==> xcodebuild (simulator)"
cd "$IOS_PROJECT_DIR"
[ "$CLEAN" = 1 ] && rm -rf "$DERIVED_DATA"
xcodebuild -project bevy_in_iOS.xcodeproj -scheme bevy_in_iOS -configuration Debug \
    -destination "id=$DEVICE_ID" -derivedDataPath "$DERIVED_DATA" build

APP_PATH="$DERIVED_DATA/Build/Products/Debug-iphonesimulator/bevy_in_iOS.app"

echo "==> Installing on Simulator"
xcrun simctl install "$DEVICE_ID" "$APP_PATH"

echo "==> Launching"
xcrun simctl terminate "$DEVICE_ID" "$BUNDLE_ID" >/dev/null 2>&1 || true
xcrun simctl launch "$DEVICE_ID" "$BUNDLE_ID"

echo "==> Done."
