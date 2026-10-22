#!/bin/bash
# Applies a jq program to a .glb's JSON chunk and repairs the two length fields
# the format carries: the file total at offset 8 and the chunk's own at offset
# 12. A glTF JSON chunk is space padded to a four byte boundary, so any edit
# that changes its length means rewriting both rather than patching in place.
#
#   glb_patch.sh <file.glb> '<jq program>'
#
# Adding a socket:
#   glb_patch.sh w.glb '(.nodes|length) as $i
#     | .nodes += [{name:"Socket_Eject", translation:[0,0,0]}]
#     | .scenes[0].nodes += [$i]'
#
# Renaming one:
#   glb_patch.sh w.glb '(.nodes[] | select(.name=="Socket_ScopeView") | .name) = "Socket_Eyepiece"'
set -e
f="$1"; filter="$2"
work="${TMPDIR:-/tmp}/glb_patch.$$"
mkdir -p "$work"
trap 'rm -rf "$work"' EXIT

jsonlen=$(od -An -tu4 -j 12 -N 4 "$f" | tr -d ' ')
head -c 20 "$f" | tail -c 4 | grep -q JSON || { echo "not a JSON chunk"; exit 1; }

tail -c +21 "$f" | head -c "$jsonlen" > "$work/in.json"
tail -c +$((21 + jsonlen)) "$f" > "$work/rest.bin"

jq -c "$filter" "$work/in.json" > "$work/out.json"

newlen=$(wc -c < "$work/out.json" | tr -d ' ')
pad=$(( (4 - newlen % 4) % 4 ))
for ((i = 0; i < pad; i++)); do printf ' ' >> "$work/out.json"; done
newlen=$((newlen + pad))
total=$((12 + 8 + newlen + $(wc -c < "$work/rest.bin" | tr -d ' ')))

le32() {
  printf "$(printf '\\x%02x\\x%02x\\x%02x\\x%02x' \
    $(($1 & 255)) $((($1 >> 8) & 255)) $((($1 >> 16) & 255)) $((($1 >> 24) & 255)))"
}

{
  head -c 8 "$f"
  le32 "$total"
  le32 "$newlen"
  printf 'JSON'
  cat "$work/out.json" "$work/rest.bin"
} > "$work/out.glb"
mv "$work/out.glb" "$f"

echo "$(basename "$f"): json $jsonlen -> $newlen, header says $total, file is $(wc -c < "$f" | tr -d ' ')"
