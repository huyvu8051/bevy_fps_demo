#!/bin/bash
# A/B frame time for one setting, at full render resolution, against a scene
# stripped of everything that moves.
#
# Full resolution because the game ships at 0.3 and measuring there hides
# everything that costs per pixel: a 16x change in the scope texture measured
# as nothing, and so did the render scale itself. Zombies frozen and
# unanimated, smoke, tracers, markers and the aim blur all off: what is left is
# the renderer, which is the only thing a scope measurement is about.
# Background noise on this machine is worth about 1ms, so the runs are
# interleaved and the result is a median, not a mean.
#
#   bench_ads.sh <setting_name> <value_A> <value_B> [rounds]
#   bench_ads.sh scope_camera_enabled true false
#   bench_ads.sh aim_blur_enabled true false 3
#
# Debug build: it reuses the cache everything else here warms. Absolute numbers
# are not comparable to a release build, but an A/B within one profile holds.
#
# Needs the binary built and the game NOT running: a second client on the same
# GPU moves the numbers more than most of what is being measured.
set -e
cd "$(dirname "$0")/.."
key="$1"; a="$2"; b="$3"; rounds="${4:-2}"
work="${TMPDIR:-/tmp}/bench_ads.$$"
mkdir -p "$work/home"
trap 'rm -rf "$work"' EXIT

[ -x ./target/debug/client ] || { echo "chua build: cargo build -p client"; exit 1; }
pgrep -f "target/debug/client" >/dev/null && { echo "client dang chay -- dong no truoc"; exit 1; }

# One field per line so the benchmarked key can be filtered out by name.
baseline=(
  "fps_cap:Some(1000)"
  "autopilot:Some(false)"
  "render_resolution_scale:Some(1.0)"
  "freeze_zombies:Some(true)"
  "zombie_anim_disabled:Some(true)"
  "zombie_health_bars_hidden:Some(true)"
  "aim_blur_enabled:Some(false)"
  "muzzle_smoke_enabled:Some(false)"
  "port_smoke_enabled:Some(false)"
  "barrel_smoke_enabled:Some(false)"
  "show_hit_markers:Some(false)"
  "show_floating_damage:Some(false)"
  "show_bullet_tracers:Some(false)"
  "show_hitboxes:Some(false)"
  "nav_debug_visible:Some(false)"
  "scope_camera_enabled:Some(true)"
)

printf '%s\n' "${baseline[@]}" | grep -q "^${key}:" || {
  echo "canh bao: '$key' khong co trong baseline, se dung mac dinh cua game cho moi thu khac" >&2
}

one() {
  # The benchmarked key must appear exactly once. RON rejects a duplicate
  # field outright, the game falls back to shipped defaults, and those cap at
  # 60fps -- so every sample reads 16.67ms and the A/B looks like a dead heat.
  # That is what `bench_ads.sh scope_camera_enabled true false`, this file's
  # own first example, did for as long as it existed. Any baseline field is
  # a candidate to be benchmarked, so the key is dropped from the baseline
  # rather than special-cased.
  {
    echo "("
    printf '%s\n' "${baseline[@]}" | grep -v "^${key}:" | sed 's/$/,/'
    echo "$key:Some($1))"
  } > "$work/home/.bevy_fps_demo_settings.ron"

  # VSYNC=0 or nothing below a refresh is visible: on a 120Hz panel every
  # configuration faster than 8.33ms reads as exactly 8.33ms.
  HOME="$work/home" BEVY_ASSET_ROOT="$PWD/crates/client" MAP_ID=0 FORCE_ADS=1 FPS_LOG=1 VSYNC=0 \
    ./target/debug/client > "$work/run.log" 2>&1 &
  local p=$!
  # macOS throttles an unfocused window to 10fps, which the filter below then
  # discards -- and a run that loses focus loses most of its samples with it.
  # Pull it to the front and give it a moment to settle before counting.
  perl -e 'sleep 4'
  osascript -e "tell application \"System Events\" to set frontmost of (first process whose unix id is $p) to true" >/dev/null 2>&1 || true
  perl -e 'sleep 34'
  kill -TERM $p 2>/dev/null || true
  wait $p 2>/dev/null || true

  # A settings file the game rejected means it ran on defaults, and the run
  # measured those rather than what was asked for.
  if grep -q "failed to parse saved settings" "$work/run.log"; then
    echo "LOI: game khong doc duoc file cai dat, da chay bang mac dinh" >&2
    sed 's/\x1b\[[0-9;]*m//g' "$work/run.log" | grep "failed to parse" | head -1 >&2
    return 1
  fi

  # Every sample after the first, which is the frame the level finished
  # loading on and reads several times the steady cost.
  #
  # Not `tail -N`: runs differ in length, so a fixed tail takes a different
  # stretch out of each one. That alone reported merge_map_meshes as twice as
  # fast as it is. Samples over 50ms are dropped as lost focus, and because
  # that filter can bite one side harder than the other, the report prints
  # each side's count so an uneven drop shows rather than skewing the median
  # in silence.
  grep -o "khung [0-9.]*ms" "$work/run.log" | awk 'NR > 1 && $2 + 0 < 50 { print $2 + 0 }' >> "$work/$2.txt"
  grep -c "khung [0-9.]*ms" "$work/run.log" >> "$work/$2.total" || true
}

for ((r = 0; r < rounds; r++)); do one "$a" A; one "$b" B; done

report() {
  local kept total
  kept=$(wc -l < "$work/$1.txt" | tr -d ' ')
  total=$(awk '{s += $1} END { print s + 0 }' "$work/$1.total")
  sort -n "$work/$1.txt" | awk -v k="$key=$2" -v kept="$kept" -v total="$total" '{ v[NR] = $1 }
    END { if (NR == 0) { printf "%-26s KHONG CO MAU NAO\n", k; exit }
          m = (NR % 2 ? v[(NR + 1) / 2] : (v[NR / 2] + v[NR / 2 + 1]) / 2)
          printf "%-26s trung vi %6.2fms  p10 %5.2f  p90 %5.2f  giu %d/%d mau\n",
                 k, m, v[int(NR * 0.1) + 1], v[int(NR * 0.9)], kept, total }'
}

report A "$a"
report B "$b"

# An uneven filter is the failure this harness has actually had, so say so
# rather than leaving it to be spotted in the counts.
a_kept=$(wc -l < "$work/A.txt" | tr -d ' ')
b_kept=$(wc -l < "$work/B.txt" | tr -d ' ')
if [ "$a_kept" -gt 0 ] && [ "$b_kept" -gt 0 ]; then
  awk -v x="$a_kept" -v y="$b_kept" 'BEGIN {
    r = (x > y ? x / y : y / x)
    if (r > 1.5) printf "\nCANH BAO: so mau hai ben lech %.1f lan, trung vi khong so sanh duoc. Chay lai.\n", r
  }'
fi
