#!/usr/bin/env python3
"""Extracts each zombie kind's base-color texture out of its mesh .glb into
standalone JPEG files at every texture-quality tier (0 = flat single color,
..., 5 = original resolution) — same tiering as bake_car_textures.py, but
simpler: every zombie kind's mesh.glb ships exactly one material with only
a baseColorTexture (no normal/metallic-roughness texture slots at all, per
a real inspection of zombie.glb/zombie_meshy.glb/zombie_soldier.glb), so
there's only one slot to bake, not three.

Written per-kind into `<kind_dir>/mesh/textures/base_color_q{N}.jpg` —
unlike bake_car_textures.py's tier 5 (a byte-identical copy, safe there
since every car's embedded source image is already JPEG), every zombie
kind's embedded source is PNG (with an alpha channel bake_car_textures.py
never has to handle either), so tier 5 here is always a real re-encode
(quality=95, near-lossless) rather than a raw-bytes copy — writing PNG
bytes into a file the extension-based ImageLoader (FromExtension) expects
to be JPEG would silently fail to decode. Applied at runtime
by swapping the shared zombie StandardMaterial's base_color_texture handle
(desktop) / re-uploading the SkinnedModel's base_color_texture (mobile
native Metal renderer) whenever TextureQuality changes — see
apply_zombie_texture_quality (crates/client/src/zombie_client.rs) and
native_metal_probe.rs's own zombie-texture-quality handling.

Usage:
    python3 bake_zombie_textures.py crates/client/assets/characters/zombie/mesh/zombie.glb
    python3 bake_zombie_textures.py --all-zombies
"""

import argparse
import glob
import io
import json
import os
import struct

from PIL import Image

# tier -> (max dimension, JPEG quality); tier 5 handled separately (plain copy).
TIERS = {
    4: (1024, 85),
    3: (512, 80),
    2: (256, 75),
    1: (128, 70),
    0: (1, 50),
}

GLB_MAGIC = 0x46546C67
JSON_CHUNK_TYPE = 0x4E4F534A
BIN_CHUNK_TYPE = 0x004E4942


def read_glb(path):
    with open(path, "rb") as f:
        data = f.read()
    length = struct.unpack("<III", data[0:12])[2]
    offset = 12
    json_chunk = None
    bin_chunk = b""
    while offset < length:
        chunk_len, chunk_type = struct.unpack("<II", data[offset : offset + 8])
        chunk_data = data[offset + 8 : offset + 8 + chunk_len]
        if chunk_type == JSON_CHUNK_TYPE:
            json_chunk = json.loads(chunk_data)
        elif chunk_type == BIN_CHUNK_TYPE:
            bin_chunk = chunk_data
        offset += 8 + chunk_len
    return json_chunk, bin_chunk


def image_bytes_for_texture(gltf_json, bin_data, texture_index):
    texture = gltf_json["textures"][texture_index]
    image = gltf_json["images"][texture["source"]]
    bv = gltf_json["bufferViews"][image["bufferView"]]
    offset = bv.get("byteOffset", 0)
    length = bv["byteLength"]
    return bin_data[offset : offset + length]


def bake_zombie_textures(src_path):
    gltf_json, bin_data = read_glb(src_path)
    material = gltf_json["materials"][0]
    tex_ref = material.get("pbrMetallicRoughness", {}).get("baseColorTexture")
    out_dir = os.path.join(os.path.dirname(src_path), "textures")
    os.makedirs(out_dir, exist_ok=True)

    print(f"{src_path}:")
    if tex_ref is None:
        print("  base_color: no texture in material, skipped")
        return
    raw = image_bytes_for_texture(gltf_json, bin_data, tex_ref["index"])
    im = Image.open(io.BytesIO(raw))
    im = im.convert("RGB") if im.mode != "RGB" else im

    w, h = im.size
    # Tier 5: full resolution, high-quality re-encode (not a byte-identical
    # copy — see this fn's own doc comment for why that'd be unsafe here).
    im.save(os.path.join(out_dir, "base_color_q5.jpg"), format="JPEG", quality=95, optimize=True)
    for tier, (max_dim, jpeg_quality) in sorted(TIERS.items(), reverse=True):
        tier_im = im
        if max(w, h) > max_dim:
            scale = max_dim / max(w, h)
            tier_im = im.resize((max(1, round(w * scale)), max(1, round(h * scale))), Image.LANCZOS)
        out_path = os.path.join(out_dir, f"base_color_q{tier}.jpg")
        tier_im.save(out_path, format="JPEG", quality=jpeg_quality, optimize=True)
    print(f"  base_color: {w}x{h} -> textures/base_color_q0..q5.jpg")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("inputs", nargs="*")
    parser.add_argument("--all-zombies", action="store_true")
    args = parser.parse_args()

    inputs = list(args.inputs)
    if args.all_zombies:
        repo_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
        pattern = os.path.join(repo_root, "crates/client/assets/characters/*/mesh/*.glb")
        # Excludes anim-only glbs (e.g. soldier_meshy/anims/mixamo_base.glb
        # lives under anims/, not mesh/, so the glob above already skips it;
        # zombie_soldier/mesh/zombie_soldier.glb and the other two mesh
        # kinds are exactly what's left).
        inputs.extend(sorted(glob.glob(pattern)))

    if not inputs:
        parser.error("no input files — pass a path or --all-zombies")

    for path in inputs:
        bake_zombie_textures(path)


if __name__ == "__main__":
    main()
