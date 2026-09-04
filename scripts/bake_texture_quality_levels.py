#!/usr/bin/env python3
"""Generates downsampled-texture sibling glb files for the 5-tier texture
quality system (crates/client/src/lib.rs's texture_quality_default,
crates/shared/src/item_gallery.rs's item_gallery_car_visual_path).

Tier 5 is the existing `_light.glb` file itself (no new file needed —
original resolution). This script generates tiers 1-4 as `_light_qN.glb`
siblings next to it, each with every embedded image resized to a smaller
max dimension and re-encoded as JPEG at a lower quality, while leaving
every other glb chunk (meshes, accessors, materials, nodes, animations)
byte-identical — so collider construction (ConvexHullFromMesh, both
client and server) never differs across tiers, only what's drawn does.

Only the JSON chunk's bufferView byteOffset/byteLength values change (to
match each image's new, smaller byte size) plus the BIN chunk itself and
buffers[0].byteLength — every other JSON field is copied through as-is.

Usage:
    python3 bake_texture_quality_levels.py input_light.glb
    python3 bake_texture_quality_levels.py --all-cars   # every car in
        crates/client/assets/props/car_*/mesh/*_light.glb
"""

import argparse
import glob
import io
import json
import os
import struct

from PIL import Image

# tier -> (max texture dimension, JPEG quality)
TIERS = {
    4: (1024, 85),
    3: (512, 80),
    2: (256, 75),
    1: (128, 70),
    # "No texture" — not literally zero bytes (glTF still needs a valid
    # image to decode), just a 1x1 solid color, so the material/texture
    # reference structure stays identical to every other tier and the
    # Rust loader/renderer needs no special-casing. Downsampling to 1x1
    # naturally averages the whole image down to one flat color.
    0: (1, 50),
}

GLB_MAGIC = 0x46546C67
JSON_CHUNK_TYPE = 0x4E4F534A
BIN_CHUNK_TYPE = 0x004E4942


def read_glb(path):
    with open(path, "rb") as f:
        data = f.read()
    magic, version, length = struct.unpack("<III", data[0:12])
    if magic != GLB_MAGIC:
        raise ValueError(f"{path} is not a glb file")
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


def write_glb(path, gltf_json, bin_data):
    json_bytes = json.dumps(gltf_json, separators=(",", ":")).encode("utf-8")
    # glTF spec: JSON chunk padded with spaces, BIN chunk padded with zeros,
    # both to a 4-byte boundary.
    json_pad = (4 - len(json_bytes) % 4) % 4
    json_bytes += b" " * json_pad
    bin_pad = (4 - len(bin_data) % 4) % 4
    bin_data = bin_data + b"\x00" * bin_pad

    total_len = 12 + 8 + len(json_bytes) + 8 + len(bin_data)
    with open(path, "wb") as f:
        f.write(struct.pack("<III", GLB_MAGIC, 2, total_len))
        f.write(struct.pack("<II", len(json_bytes), JSON_CHUNK_TYPE))
        f.write(json_bytes)
        f.write(struct.pack("<II", len(bin_data), BIN_CHUNK_TYPE))
        f.write(bin_data)


def resize_image_bytes(raw, max_dim, jpeg_quality):
    im = Image.open(io.BytesIO(raw))
    im = im.convert("RGB") if im.mode in ("P", "CMYK") else im
    w, h = im.size
    if max(w, h) > max_dim:
        scale = max_dim / max(w, h)
        im = im.resize((max(1, round(w * scale)), max(1, round(h * scale))), Image.LANCZOS)
    out = io.BytesIO()
    im.save(out, format="JPEG", quality=jpeg_quality, optimize=True)
    return out.getvalue()


def bake_tier(src_path, tier, max_dim, jpeg_quality):
    gltf_json, bin_data = read_glb(src_path)
    buffer_views = gltf_json["bufferViews"]
    images = gltf_json.get("images", [])

    # Which bufferView indices are images vs. everything else (mesh/anim
    # data) — only image ones get replaced; the rest are copied unchanged.
    image_bufferview_indices = {img["bufferView"] for img in images if "bufferView" in img}

    new_bin = bytearray()
    new_offsets = {}  # bufferView index -> (new_offset, new_length)
    for i, bv in enumerate(buffer_views):
        offset = bv.get("byteOffset", 0)
        length = bv["byteLength"]
        raw = bin_data[offset : offset + length]
        if i in image_bufferview_indices:
            raw = resize_image_bytes(raw, max_dim, jpeg_quality)
        # glTF requires each bufferView to start 4-byte aligned.
        pad = (4 - len(new_bin) % 4) % 4
        new_bin.extend(b"\x00" * pad)
        new_offset = len(new_bin)
        new_bin.extend(raw)
        new_offsets[i] = (new_offset, len(raw))

    out_json = json.loads(json.dumps(gltf_json))  # deep copy
    for i, bv in enumerate(out_json["bufferViews"]):
        new_offset, new_length = new_offsets[i]
        bv["byteOffset"] = new_offset
        bv["byteLength"] = new_length
        bv.pop("byteStride", None) if i in image_bufferview_indices else None
    out_json["buffers"][0]["byteLength"] = len(new_bin)

    stem = src_path[:-4] if src_path.endswith(".glb") else src_path
    out_path = f"{stem}_q{tier}.glb"
    write_glb(out_path, out_json, bytes(new_bin))
    orig_size = os.path.getsize(src_path)
    new_size = os.path.getsize(out_path)
    print(f"  q{tier}: {out_path} ({orig_size/1e6:.1f}MB -> {new_size/1e6:.2f}MB)")


def bake_all_tiers(src_path):
    print(f"{src_path}:")
    for tier, (max_dim, jpeg_quality) in sorted(TIERS.items(), reverse=True):
        bake_tier(src_path, tier, max_dim, jpeg_quality)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("inputs", nargs="*", help="source _light.glb file(s)")
    parser.add_argument("--all-cars", action="store_true", help="process every props/car_*/mesh/*_light.glb")
    args = parser.parse_args()

    inputs = list(args.inputs)
    if args.all_cars:
        repo_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
        pattern = os.path.join(repo_root, "crates/client/assets/props/car_*/mesh/*_light.glb")
        inputs.extend(sorted(glob.glob(pattern)))

    if not inputs:
        parser.error("no input files — pass a path or --all-cars")

    for path in inputs:
        bake_all_tiers(path)


if __name__ == "__main__":
    main()
