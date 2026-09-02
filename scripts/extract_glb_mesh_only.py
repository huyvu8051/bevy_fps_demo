#!/usr/bin/env python3
"""Extracts a glb's mesh + textures + skeleton/rig into a standalone
"mesh library" glb — everything EXCEPT animations, so it can be paired at
runtime with a separate "animation library" glb (see
extract_glb_animations.py, its exact mirror image) that drives it, as long
as both share the same bone names.

Why this split is useful: bevy_animation::AnimationTargetId hashes the NAME
PATH from the animation root down to the bone, not the node index or which
file a clip came from — see extract_glb_animations.py's own docstring for
the full rationale. So one mesh-only file (this script's output) can be
driven by ANY animation-only file whose skeleton's bone names + hierarchy
match, and one animation-only file can drive MANY different mesh-only
files — the reuse the mesh + animation split is meant to unlock.

Keeps: every node (full hierarchy, unchanged), meshes, materials, images,
textures, samplers, skins, cameras — and only the accessor/bufferView/
buffer bytes actually used by those (mesh vertex/index data, skin inverse
bind matrices, image data via bufferViews) — repacked into one small
buffer. Node transforms (translation/rotation/scale) are kept as-is; they
still matter for skeleton bind pose even with no animation driving them.

Drops entirely: `animations`. Any accessor/bufferView that was ONLY
referenced by an animation sampler (never by a mesh/skin/anything else)
gets dropped too, since nothing else needs those bytes.

Usage:
    python3 extract_glb_mesh_only.py input.glb output.glb
"""

import argparse
import json
import struct
import sys


def read_glb(path):
    with open(path, "rb") as f:
        magic, version, _total_len = struct.unpack("<III", f.read(12))
        if magic != 0x46546C67:
            sys.exit(f"{path}: not a glb (bad magic)")
        j_len, _j_type = struct.unpack("<II", f.read(8))
        json_bytes = f.read(j_len)
        bin_bytes = b""
        chunk = f.read(8)
        if chunk:
            b_len, _b_type = struct.unpack("<II", chunk)
            bin_bytes = f.read(b_len)
    return json.loads(json_bytes), bin_bytes


def write_glb(path, gltf_json, bin_bytes):
    new_json_bytes = json.dumps(gltf_json, separators=(",", ":")).encode("utf-8")
    json_pad = (-len(new_json_bytes)) % 4
    new_json_bytes += b" " * json_pad

    bin_pad = (-len(bin_bytes)) % 4
    padded_bin = bin_bytes + b"\x00" * bin_pad

    total_len = 12 + 8 + len(new_json_bytes) + 8 + len(padded_bin)
    with open(path, "wb") as f:
        f.write(struct.pack("<III", 0x46546C67, 2, total_len))
        f.write(struct.pack("<II", len(new_json_bytes), 0x4E4F534A))  # 'JSON'
        f.write(new_json_bytes)
        f.write(struct.pack("<II", len(padded_bin), 0x004E4942))  # 'BIN\0'
        f.write(padded_bin)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("input")
    ap.add_argument("output")
    args = ap.parse_args()

    gltf, bin_bytes = read_glb(args.input)

    if not gltf.get("animations"):
        sys.exit(f"{args.input}: no animations present — nothing to strip, input is already mesh-only")

    # --- Which bufferViews survive: every one NOT exclusively used by an
    # animation sampler. Images can point at a bufferView directly (glb
    # embedded textures) too, not just accessors, so collect both kinds of
    # reference.
    accessors = gltf.get("accessors", [])
    buffer_views = gltf.get("bufferViews", [])

    anim_only_accessors = set()
    for anim in gltf["animations"]:
        for sampler in anim["samplers"]:
            anim_only_accessors.add(sampler["input"])
            anim_only_accessors.add(sampler["output"])

    other_accessors = set()
    for mesh in gltf.get("meshes", []):
        for prim in mesh.get("primitives", []):
            other_accessors.update(prim.get("attributes", {}).values())
            if "indices" in prim:
                other_accessors.add(prim["indices"])
            for target in prim.get("targets", []):
                other_accessors.update(target.values())
    for skin in gltf.get("skins", []):
        if "inverseBindMatrices" in skin:
            other_accessors.add(skin["inverseBindMatrices"])

    # Only ever drop an accessor if it is EXCLUSIVELY animation-only —
    # anim_only_accessors isn't actually needed for that (an accessor used
    # by a mesh/skin is kept regardless of whether it's ALSO referenced by
    # an animation sampler), but is computed above for clarity/future use.
    del anim_only_accessors
    keep_accessors = sorted(other_accessors)

    other_buffer_views = {accessors[i]["bufferView"] for i in keep_accessors if "bufferView" in accessors[i]}
    for image in gltf.get("images", []):
        if "bufferView" in image:
            other_buffer_views.add(image["bufferView"])
    keep_buffer_views = sorted(other_buffer_views)

    # --- Repack: copy only the surviving bufferViews' bytes into a fresh
    # buffer, remapping accessor.bufferView and image.bufferView indices.
    old_to_new_bv = {}
    new_buffer_views = []
    new_bin = bytearray()
    for old_idx in keep_buffer_views:
        bv = buffer_views[old_idx]
        offset = bv.get("byteOffset", 0)
        length = bv["byteLength"]
        raw = bin_bytes[offset : offset + length]

        pad = (-len(new_bin)) % 4
        new_bin.extend(b"\x00" * pad)
        new_bv = {k: v for k, v in bv.items() if k != "byteOffset"}
        new_bv["byteOffset"] = len(new_bin)
        old_to_new_bv[old_idx] = len(new_buffer_views)
        new_buffer_views.append(new_bv)
        new_bin.extend(raw)

    new_accessors = []
    old_to_new_accessor = {}
    for old_idx in keep_accessors:
        acc = dict(accessors[old_idx])
        if "bufferView" in acc:
            acc["bufferView"] = old_to_new_bv[acc["bufferView"]]
        old_to_new_accessor[old_idx] = len(new_accessors)
        new_accessors.append(acc)

    # --- Rewrite every reference to the remapped accessor/bufferView
    # indices: meshes, skins, images.
    new_meshes = []
    for mesh in gltf.get("meshes", []):
        new_mesh = dict(mesh)
        new_prims = []
        for prim in mesh.get("primitives", []):
            new_prim = dict(prim)
            new_prim["attributes"] = {k: old_to_new_accessor[v] for k, v in prim.get("attributes", {}).items()}
            if "indices" in prim:
                new_prim["indices"] = old_to_new_accessor[prim["indices"]]
            if "targets" in prim:
                new_prim["targets"] = [
                    {k: old_to_new_accessor[v] for k, v in t.items()} for t in prim["targets"]
                ]
            new_prims.append(new_prim)
        new_mesh["primitives"] = new_prims
        new_meshes.append(new_mesh)

    new_skins = []
    for skin in gltf.get("skins", []):
        new_skin = dict(skin)
        if "inverseBindMatrices" in skin:
            new_skin["inverseBindMatrices"] = old_to_new_accessor[skin["inverseBindMatrices"]]
        new_skins.append(new_skin)

    new_images = []
    for image in gltf.get("images", []):
        new_image = dict(image)
        if "bufferView" in image:
            new_image["bufferView"] = old_to_new_bv[image["bufferView"]]
        new_images.append(new_image)

    out = dict(gltf)
    out.pop("animations", None)
    out["accessors"] = new_accessors
    out["bufferViews"] = new_buffer_views
    out["buffers"] = [{"byteLength": len(new_bin)}]
    out["meshes"] = new_meshes
    out["skins"] = new_skins
    if "images" in gltf:
        out["images"] = new_images
    # nodes/materials/textures/samplers/scenes/scene carried over verbatim —
    # only accessor/bufferView-shaped things changed index.

    write_glb(args.output, out, bytes(new_bin))
    print(f"dropped {len(gltf['animations'])} animation(s)")
    print(f"kept {len(new_accessors)}/{len(accessors)} accessors, {len(new_buffer_views)}/{len(buffer_views)} bufferViews")
    print(f"buffer size: {len(new_bin)/1024:.1f} KB (was {len(bin_bytes)/1024:.1f} KB)")
    print(f"wrote {args.output}")


if __name__ == "__main__":
    main()
