#!/usr/bin/env python3
"""Extracts each car's base-color/normal/metallic-roughness textures out
of its `_light.glb` into standalone JPEG files at every texture-quality
tier (0 = flat single color, ..., 5 = original resolution) — the other
half of the texture/polygon axis split (see bake_poly_quality_levels.py's
own doc comment for why the two are separate files at all): a poly-tier
glb carries no material/texture data, and the client instead builds its
own StandardMaterial from whichever tier's images these are, so the two
axes never force each other's quality.

Written per-car into `<car>/mesh/textures/{base_color,normal,
metallic_roughness}_q{N}.jpg` — tier 5 is a plain copy of the original
embedded image (not a re-encode), so it's pixel-identical to what the
glb always shipped.

Reads each material's OWN texture index references (baseColorTexture/
normalTexture/metallicRoughnessTexture) rather than assuming a fixed
image order — confirmed necessary: car_sedan's normal/metallic-roughness
images are swapped relative to every other car's `_light.glb`.

Usage:
    python3 bake_car_textures.py crates/client/assets/props/car_ambulance/mesh/car_ambulance_light.glb
    python3 bake_car_textures.py --all-cars
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

SLOTS = [
    ("base_color", "pbrMetallicRoughness", "baseColorTexture"),
    ("metallic_roughness", "pbrMetallicRoughness", "metallicRoughnessTexture"),
    ("normal", None, "normalTexture"),
]

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


def texture_index_for_slot(material, group, key):
    node = material.get(group, material) if group else material
    tex_ref = node.get(key)
    if tex_ref is None:
        return None
    return tex_ref["index"]


def image_bytes_for_texture(gltf_json, bin_data, texture_index):
    texture = gltf_json["textures"][texture_index]
    image = gltf_json["images"][texture["source"]]
    bv = gltf_json["bufferViews"][image["bufferView"]]
    offset = bv.get("byteOffset", 0)
    length = bv["byteLength"]
    return bin_data[offset : offset + length]


def bake_car_textures(src_path):
    gltf_json, bin_data = read_glb(src_path)
    material = gltf_json["materials"][0]
    out_dir = os.path.join(os.path.dirname(src_path), "textures")
    os.makedirs(out_dir, exist_ok=True)

    print(f"{src_path}:")
    for slot_name, group, key in SLOTS:
        tex_index = texture_index_for_slot(material, group, key)
        if tex_index is None:
            print(f"  {slot_name}: no texture in material, skipped")
            continue
        raw = image_bytes_for_texture(gltf_json, bin_data, tex_index)
        im = Image.open(io.BytesIO(raw))
        im = im.convert("RGB") if im.mode in ("P", "CMYK") else im

        # Tier 5: pixel-identical copy of the original embedded bytes.
        q5_path = os.path.join(out_dir, f"{slot_name}_q5.jpg")
        with open(q5_path, "wb") as f:
            f.write(raw)

        w, h = im.size
        for tier, (max_dim, jpeg_quality) in sorted(TIERS.items(), reverse=True):
            tier_im = im
            if max(w, h) > max_dim:
                scale = max_dim / max(w, h)
                tier_im = im.resize((max(1, round(w * scale)), max(1, round(h * scale))), Image.LANCZOS)
            out_path = os.path.join(out_dir, f"{slot_name}_q{tier}.jpg")
            tier_im.save(out_path, format="JPEG", quality=jpeg_quality, optimize=True)
        print(f"  {slot_name}: {w}x{h} -> textures/{slot_name}_q0..q5.jpg")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("inputs", nargs="*")
    parser.add_argument("--all-cars", action="store_true")
    args = parser.parse_args()

    inputs = list(args.inputs)
    if args.all_cars:
        repo_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
        pattern = os.path.join(repo_root, "crates/client/assets/props/car_*/mesh/*_light.glb")
        inputs.extend(sorted(glob.glob(pattern)))

    if not inputs:
        parser.error("no input files — pass a path or --all-cars")

    for path in inputs:
        bake_car_textures(path)


if __name__ == "__main__":
    main()
