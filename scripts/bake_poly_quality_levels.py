#!/usr/bin/env python3
"""Generates triangle-reduced, TEXTURE-STRIPPED sibling glb files for the
5-tier polygon quality system (crates/client/src/lib.rs's
poly_quality_default, crates/shared/src/item_gallery.rs's
item_gallery_car_mesh_path).

Unlike the texture-quality tiers (bake_texture_quality_levels.py), a
poly-quality file carries NO material/texture data at all — the mesh gets
loaded via GltfAssetLabel::Primitive (geometry only) and the client builds
its own StandardMaterial from separately-loaded texture-quality images
(see item_gallery_cars.rs's spawn_item_gallery_test_props). Stripping
images here means the two axes never affect each other's file: picking a
low poly tier never forces a low texture tier or vice versa, and the
gltf loader never wastes time decoding a texture nobody's going to use.

Uses `gltfpack` (meshoptimizer, must be on PATH — `npm i -g gltfpack` or
via the meshoptimizer repo) for the actual triangle-count simplification,
run with `-noq` (bevy_gltf does NOT support the KHR_mesh_quantization
extension gltfpack applies by default — see bevy_gltf's own doc comment
for the full unsupported-extension list) — this script then strips every
image/texture/material-texture-reference from gltfpack's output by hand
(same bufferView-rebuild approach as bake_texture_quality_levels.py, just
dropping image bufferViews instead of resizing them).

Tier 5 is the existing `_light.glb` file's own geometry, used directly via
GltfAssetLabel::Primitive (no `_p5` file needed — same "highest tier reuses
the original file" convention bake_texture_quality_levels.py uses for its
own tier 5). This script generates tiers 1-4 as `_light_pN.glb` siblings.

Usage:
    python3 bake_poly_quality_levels.py input_light.glb
    python3 bake_poly_quality_levels.py --all-cars
"""

import argparse
import glob
import json
import os
import shutil
import struct
import subprocess
import tempfile

# tier -> (target ratio, minimum absolute triangle floor). The 16 cars'
# own triangle counts vary ~6x (car_taxi ~9.9k vs. car_sedan ~61.8k) — a
# flat ratio alone let the same 10% tier leave a low-poly car with too few
# triangles to read as a car (car_taxi's own 10% is only ~990, visibly
# faceted/broken) while a high-poly car at the same ratio still looked
# fine. Effective ratio per car is `max(ratio, floor / original_count)`,
# so a low-poly car's tier gets pulled UP toward its floor instead of
# following the global ratio down; a high-poly car is unaffected (its own
# ratio-based count already clears every floor).
TIERS = {
    4: (0.7, 6000),
    3: (0.45, 3500),
    2: (0.25, 2000),
    1: (0.1, 1200),
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


def strip_images(gltf_json, bin_data):
    """Removes every images/textures/samplers entry and every material's
    texture references, rebuilding the buffer without the now-unused
    image bufferViews. Geometry bufferViews (positions/normals/uvs/
    indices) are copied through byte-identical, just renumbered/
    reoffset since some bufferView indices before them disappear."""
    images = gltf_json.get("images", [])
    image_bufferview_indices = {img["bufferView"] for img in images if "bufferView" in img}

    old_buffer_views = gltf_json["bufferViews"]
    keep_old_indices = [i for i in range(len(old_buffer_views)) if i not in image_bufferview_indices]
    old_to_new = {old_i: new_i for new_i, old_i in enumerate(keep_old_indices)}

    new_bin = bytearray()
    new_buffer_views = []
    for old_i in keep_old_indices:
        bv = old_buffer_views[old_i]
        offset = bv.get("byteOffset", 0)
        length = bv["byteLength"]
        raw = bin_data[offset : offset + length]
        pad = (4 - len(new_bin) % 4) % 4
        new_bin.extend(b"\x00" * pad)
        new_bv = dict(bv)
        new_bv["byteOffset"] = len(new_bin)
        new_buffer_views.append(new_bv)
        new_bin.extend(raw)

    out = json.loads(json.dumps(gltf_json))
    out["bufferViews"] = new_buffer_views
    out["buffers"][0]["byteLength"] = len(new_bin)
    out.pop("images", None)
    out.pop("textures", None)
    out.pop("samplers", None)

    # Re-point every surviving accessor at its bufferView's new index.
    for acc in out["accessors"]:
        if "bufferView" in acc:
            acc["bufferView"] = old_to_new[acc["bufferView"]]

    for mat in out.get("materials", []):
        pbr = mat.get("pbrMetallicRoughness", {})
        pbr.pop("baseColorTexture", None)
        pbr.pop("metallicRoughnessTexture", None)
        mat.pop("normalTexture", None)
        mat.pop("occlusionTexture", None)
        mat.pop("emissiveTexture", None)

    return out, bytes(new_bin)


def triangle_count(gltf_json):
    total = 0
    for mesh in gltf_json["meshes"]:
        for prim in mesh["primitives"]:
            total += gltf_json["accessors"][prim["indices"]]["count"] // 3
    return total


def bake_tier(src_path, tier, ratio, floor, original_tris):
    effective_ratio = max(ratio, min(1.0, floor / original_tris))
    stem = src_path[:-4] if src_path.endswith(".glb") else src_path
    out_path = f"{stem}_p{tier}.glb"
    with tempfile.NamedTemporaryFile(suffix=".glb", delete=False) as tmp:
        tmp_path = tmp.name
    try:
        subprocess.run(
            ["gltfpack", "-i", src_path, "-o", tmp_path, "-si", str(effective_ratio), "-sa", "-noq"],
            check=True,
            capture_output=True,
            text=True,
        )
        gltf_json, bin_data = read_glb(tmp_path)
        result_tris = triangle_count(gltf_json)
        gltf_json, bin_data = strip_images(gltf_json, bin_data)
        write_glb(out_path, gltf_json, bin_data)
    finally:
        os.unlink(tmp_path)

    orig_size = os.path.getsize(src_path)
    new_size = os.path.getsize(out_path)
    floor_note = " (floor-raised)" if effective_ratio > ratio else ""
    print(f"  p{tier}: {out_path} ({orig_size/1e6:.1f}MB -> {new_size/1e6:.2f}MB, {result_tris} tris, ratio {effective_ratio:.3f}{floor_note})")


def bake_all_tiers(src_path):
    print(f"{src_path}:")
    gltf_json, _ = read_glb(src_path)
    original_tris = triangle_count(gltf_json)
    for tier, (ratio, floor) in sorted(TIERS.items(), reverse=True):
        bake_tier(src_path, tier, ratio, floor, original_tris)


def main():
    if shutil.which("gltfpack") is None:
        raise SystemExit("gltfpack not found on PATH (npm i -g gltfpack)")

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
