#!/bin/bash
# Cuts the three weapons' fire and reload one-shots out of the Sonniss GDC
# bundle (royalty free, commercial use allowed; see assets/audio/license.txt).
#
# Sources are fetched one file at a time from the Internet Archive mirror
# rather than pulling the whole multi-gigabyte bundle.
#
# The reload clip is composed, not cut: the two foley hits are placed so their
# onsets land on the beats the procedural animation already uses --
# RELOAD_HAND_AT_MAG and MAG_SWAP_AT in magazine_drop.rs. Change those and the
# OUT_AT / IN_AT fractions below have to follow.
#
# Every offset below was read off a peak envelope, not heard. If a take sounds
# wrong, move its two numbers; nothing else has to change.
set -e
cd "$(dirname "$0")/.."
out="crates/client/assets/audio/weapons"
work="${TMPDIR:-/tmp}/weapon_sounds"
mkdir -p "$out" "$work/src"

ITEM="https://archive.org/download/gameaudiogdc-part1through7"
B16="Sonniss.com - GDC 2016- Game Audio Bundle/Pole Position Production"
BPP="Sonniss.com - GDC - Game Audio Bundle/Coll Anderson - Guns"

AKM="$B16 - AKM 7.62x39mm/AKM_7.62x39mm_1m_left_on_ground_DPA4062_clean_Triple_shots_x_1.flac"
HK="$B16 -  Heckler & Koch HK416 5.56mm/HK_416_5.56mm_on_gun_DPA4062_clean_Seven_shots_x_1.flac"
REM="$B16 - Remington 700 7.62x51mm/Remington_700_7.62x51mm_1m_right_MKH8040_1_clean_Single_shots_x_14.flac"
G36="$B16 - Various Gun Foley & Handling/Heckler_&_Koch_G36C_5.56mm_foley_close_up_MKH60_mag_in_&_out.flac"
SAIGA="$B16 - Various Gun Foley & Handling/SAIGA-12_12g_solid_slug_foley_close_up_RSM191_R_cocking.flac"
AR15="$BPP/EFX INT AR-15 M4 Charging Handle With Mag 01_A.flac"

fetch() {
  local dst="$work/src/$(basename "$1")"
  [ -s "$dst" ] || curl -sfL -o "$dst" "$ITEM/$(jq -rn --arg s "$1" '$s|@uri' | sed 's/%2F/\//g')"
  echo "$dst"
}

# Peak normalises so the three weapons sit at the same level regardless of how
# far the microphone was from each muzzle.
norm() {
  local gain
  gain=$(ffmpeg -hide_banner -i "$1" -af volumedetect -f null - 2>&1 \
    | awk -F': ' '/max_volume/{printf "%.2f", -$2 - 1.0}')
  ffmpeg -y -v error -i "$1" -af "volume=${gain}dB" "$2"
}

# cut <src> <start> <end> <dst>  -- one shot, de-clicked at both ends.
cut() {
  local fade
  fade=$(awk -v a="$2" -v b="$3" 'BEGIN{printf "%.4f", b-a-0.05}')
  ffmpeg -y -v error -ss "$2" -to "$3" -i "$1" \
    -af "aresample=48000,aformat=channel_layouts=mono,afade=t=in:st=0:d=0.002,afade=t=out:st=$fade:d=0.05" \
    -c:a pcm_s16le "$work/raw.wav"
  norm "$work/raw.wav" "$4"
}

# event <src> <start> <end> <dst> -- one foley hit, no tail fade.
event() {
  local fade
  fade=$(awk -v a="$2" -v b="$3" 'BEGIN{printf "%.4f", b-a-0.03}')
  ffmpeg -y -v error -ss "$2" -to "$3" -i "$1" \
    -af "aresample=48000,aformat=channel_layouts=mono,afade=t=in:st=0:d=0.004,afade=t=out:st=$fade:d=0.03" \
    -c:a pcm_s16le "$4"
}

OUT_AT=0.18   # RELOAD_HAND_AT_MAG
IN_AT=0.74    # MAG_SWAP_AT

# reload <out.wav> <in.wav> <duration> <dst.wav>
reload() {
  local d=$3
  local a b
  a=$(awk -v d="$d" -v f="$OUT_AT" 'BEGIN{printf "%d", d*f*1000}')
  b=$(awk -v d="$d" -v f="$IN_AT" 'BEGIN{printf "%d", d*f*1000}')
  ffmpeg -y -v error -i "$1" -i "$2" -filter_complex \
    "[0:a]adelay=${a}[a];[1:a]adelay=${b}[b];[a][b]amix=inputs=2:normalize=0,apad,atrim=0:$d" \
    -c:a pcm_s16le "$work/raw_reload.wav"
  norm "$work/raw_reload.wav" "$4"
}

ak=$(fetch "$AKM"); hk=$(fetch "$HK"); rem=$(fetch "$REM")
g36=$(fetch "$G36"); saiga=$(fetch "$SAIGA"); ar15=$(fetch "$AR15")

# The last shot of each burst: it is the only one whose tail is not stepped on
# by the next round.
cut "$ak"  0.562 0.890 "$out/ak47_fire.wav"
cut "$hk"  1.765 2.300 "$out/m4a1_fire.wav"
cut "$rem" 4.070 4.650 "$out/m24_fire.wav"

event "$g36"   1.09 1.70 "$work/ak_out.wav"
event "$g36"   6.33 6.81 "$work/ak_in.wav"
event "$ar15"  0.46 1.24 "$work/m4_out.wav"
event "$ar15"  5.25 6.11 "$work/m4_in.wav"
event "$saiga" 2.30 3.09 "$work/m24_out.wav"
event "$saiga" 6.07 6.67 "$work/m24_in.wav"

reload "$work/ak_out.wav"  "$work/ak_in.wav"  2.3 "$out/ak47_reload.wav"
reload "$work/m4_out.wav"  "$work/m4_in.wav"  1.9 "$out/m4a1_reload.wav"
reload "$work/m24_out.wav" "$work/m24_in.wav" 3.1 "$out/m24_reload.wav"

ls -lh "$out"
