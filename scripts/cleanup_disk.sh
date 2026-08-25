#!/usr/bin/env bash
# Frees disk space from two build-tooling caches that grow unbounded on this
# machine and are both 100% safe to delete (pure caches — next build/deploy
# just regenerates them, a bit slower once):
#
#   1. /tmp/dd_* — ad-hoc `xcodebuild -derivedDataPath /tmp/dd_*` scratch
#      dirs created during iOS build/deploy sessions (each ~3-5GB). Xcode
#      never cleans these up on its own.
#   2. ~/Library/Containers/com.apple.CoreDevice.CoreDeviceService/Data/
#      Library/Caches/AppInstallationBinaryDeltas — Apple's `devicectl`
#      binary-delta cache used to speed up installs to a real iOS device
#      over USB. Grows unbounded across many `devicectl install` calls.
#
# Usage: scripts/cleanup_disk.sh [--min-age-days N] [--dry-run]
#   --min-age-days N   only remove /tmp/dd_* dirs untouched for N+ days
#                      (default 1) — skips whatever a build in progress
#                      right now is actively using.
#   --dry-run          print what would be removed without removing it.
set -euo pipefail

MIN_AGE_DAYS=1
DRY_RUN=0
while [ $# -gt 0 ]; do
    case "$1" in
        --min-age-days) MIN_AGE_DAYS="$2"; shift 2 ;;
        --dry-run) DRY_RUN=1; shift ;;
        *) echo "unknown arg: $1" >&2; exit 1 ;;
    esac
done

before_kb="$(df -k / | tail -1 | awk '{print $4}')"

echo "==> /tmp/dd_* older than ${MIN_AGE_DAYS}d (Xcode DerivedData scratch dirs)"
# Deliberately not using `find` here -- in this environment's shell (Claude
# Code injects a `find` wrapper function, and even /usr/bin/find directly
# silently returns nothing under sandboxing) it does not reliably work.
# A plain glob + stat loop is simpler and actually works everywhere.
now="$(/bin/date +%s)"
for dir in /tmp/dd_*; do
    [ -d "$dir" ] || continue
    mtime="$(/usr/bin/stat -f '%m' "$dir")"
    age_days=$(( (now - mtime) / 86400 ))
    [ "$age_days" -ge "$MIN_AGE_DAYS" ] || continue
    size="$(du -sh "$dir" 2>/dev/null | cut -f1)"
    if [ "$DRY_RUN" = 1 ]; then
        echo "    [dry-run] would remove $dir ($size, age=${age_days}d)"
    else
        echo "    removing $dir ($size, age=${age_days}d)"
        rm -rf "$dir"
    fi
done

CACHE_DIR="$HOME/Library/Containers/com.apple.CoreDevice.CoreDeviceService/Data/Library/Caches/AppInstallationBinaryDeltas"
echo "==> devicectl install-binary-delta cache"
if [ -d "$CACHE_DIR" ]; then
    size="$(du -sh "$CACHE_DIR" 2>/dev/null | cut -f1)"
    if [ "$DRY_RUN" = 1 ]; then
        echo "    [dry-run] would clear $CACHE_DIR ($size)"
    else
        echo "    clearing $CACHE_DIR ($size)"
        rm -rf "${CACHE_DIR:?}"/*
    fi
else
    echo "    (not present, nothing to do)"
fi

after_kb="$(df -k / | tail -1 | awk '{print $4}')"
freed_gb=$(( (after_kb - before_kb) / 1024 / 1024 ))
echo "==> Done. Freed ~${freed_gb}GB on / (dry-run: ${DRY_RUN})."
