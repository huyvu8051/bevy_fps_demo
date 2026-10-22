#!/bin/bash
# A/B frame time for one setting, against a scene stripped of everything that
# moves. Zombies frozen and unanimated, smoke, tracers, markers and the aim
# blur all off: what is left is the renderer, which is the only thing a scope
# measurement is about. Background noise on this machine is worth about 1ms,
# so the runs are interleaved and the result is a median, not a mean.
#
#   bench_ads.sh <setting_name> <value_A> <value_B> [rounds]
#   bench_ads.sh scope_camera_enabled true false
#   bench_ads.sh aim_blur_enabled true false 3
#
# Needs the binary built and the game NOT running: a second client on the same
# GPU moves the numbers more than most of what is being measured.
set -e
cd "$(dirname "$0")/.."
key="$1"; a="$2"; b="$3"; rounds="${4:-2}"
work="${TMPDIR:-/tmp}/bench_ads.$$"
mkdir -p "$work/home"
trap 'rm -rf "$work"' EXIT

pgrep -f "target/debug/client" >/dev/null && { echo "client dang chay -- dong no truoc"; exit 1; }

one() {
  cat > "$work/home/.bevy_fps_demo_settings.ron" <<RON
(fps_cap:Some(1000),autopilot:Some(false),
 freeze_zombies:Some(true),zombie_anim_disabled:Some(true),zombie_health_bars_hidden:Some(true),
 aim_blur_enabled:Some(false),
 muzzle_smoke_enabled:Some(false),port_smoke_enabled:Some(false),barrel_smoke_enabled:Some(false),
 show_hit_markers:Some(false),show_floating_damage:Some(false),show_bullet_tracers:Some(false),
 show_hitboxes:Some(false),nav_debug_visible:Some(false),
 scope_camera_enabled:Some(true),
 $key:Some($1))
RON
  HOME="$work/home" BEVY_ASSET_ROOT="$PWD/crates/client" MAP_ID=0 FORCE_ADS=1 FPS_LOG=1 \
    ./target/debug/client > "$work/run.log" 2>&1 &
  local p=$!
  perl -e 'sleep 34'
  kill -TERM $p 2>/dev/null || true
  wait $p 2>/dev/null || true
  # Samples over 50ms are the window having lost focus: macOS throttles an
  # unfocused window to exactly 10fps, which reads as a 100ms regression.
  grep -o "khung [0-9.]*ms" "$work/run.log" | awk '{if ($2 < 50) print $2}' | tail -22 >> "$work/$2.txt"
}

for ((r = 0; r < rounds; r++)); do one "$a" A; one "$b" B; done

report() {
  sort -n "$work/$1.txt" | awk -v k="$key=$2" '{v[NR]=$1}
    END{m=(NR%2 ? v[(NR+1)/2] : (v[NR/2]+v[NR/2+1])/2)
        printf "%-26s trung vi %6.2fms   n=%-3d  %.2f..%.2f\n", k, m, NR, v[1], v[NR]}'
}

report A "$a"
report B "$b"
