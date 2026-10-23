#!/bin/bash
# Nen texture cua AK sang ASTC 8x8 (.ktx2) va tro scene.gltf vao chung.
#
# Dung TextureConverter cua Xcode -- khong co formula brew cho toktx/astcenc.
#
# Chi ap dung cho Material_37. Material_38 dung alphaMode BLEND voi kenh
# alpha that, nen khong dang doi 8 MB lay rui ro hong trong suot.
#
# wgpu doi be rong texture chia het cho block: 4096 chia het cho 8 va 4,
# khong chia het cho 6 -- nen 6x6 khong dung duoc o do phan giai nay.
#
# Khong theo chuan glTF: spec chi cho phep PNG/JPEG o images[].uri. Bevy bo
# qua rang buoc do (bevy_gltf tra ve ImageOrPath::Path cho uri ngoai file,
# AssetServer phan loai theo duoi file), nhung Blender/three.js se khong mo
# duoc file nua.
set -euo pipefail

TC=/Applications/Xcode.app/Contents/Developer/usr/bin/TextureConverter
DIR="$(cd "$(dirname "$0")/.." && pwd)/crates/client/assets/models/ak47_scope/raw"
TEX="$DIR/textures"

[ -x "$TC" ] || { echo "khong thay TextureConverter -- can Xcode"; exit 1; }

cd "$TEX"

# baseColor la du lieu mau -> perceptual weighting (mac dinh) + srgb_format
"$TC" --compression_format=ASTC8x8 --compression_quality=Production --srgb_format \
      --output=Material_37_baseColor.ktx2 Material_37_baseColor.jpeg

# normal/metallicRoughness la du lieu phi-mau -> linear weighting
for f in Material_37_normal Material_37_metallicRoughness; do
  "$TC" --compression_format=ASTC8x8 --compression_quality=Production \
        --channel_weighting=Linear --output="$f.ktx2" "$f.png"
done

sed -i '' \
  -e 's|textures/Material_37_baseColor\.jpeg|textures/Material_37_baseColor.ktx2|' \
  -e 's|textures/Material_37_metallicRoughness\.png|textures/Material_37_metallicRoughness.ktx2|' \
  -e 's|textures/Material_37_normal\.png|textures/Material_37_normal.ktx2|' \
  "$DIR/scene.gltf"

echo "xong. ban goc jpeg/png van con trong $TEX -- xoa tay neu muon"
echo "giam bundle them 38.6 MB (git van giu lich su)."
