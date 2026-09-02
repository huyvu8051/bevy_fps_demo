#!/usr/bin/env python3
"""Extracts a glb's animations into a standalone "animation library" glb —
skeleton (named nodes) + animation channels only, with every mesh/material/
image/texture/skin stripped out.

Why this works at all: Bevy identifies which entity an animation curve
targets by hashing the NAME PATH from the animation root down to the bone
(bevy_animation::AnimationTargetId), not by node index or which file the
clip came from. So a stripped-down library file with just the right named
node hierarchy is enough to drive ANY mesh's AnimationPlayer, as long as
that mesh's own bone names + hierarchy match this file's — see
bevy_animation::AnimationTargetId's own doc comment ("Any animation is
playable on any armature as long as the bone names match, which allows
for easy animation retargeting").

Keeps: `nodes` that are animated OR are an ancestor of one (so the full
name-path hashes correctly), `animations`, and only the accessor/
bufferView/buffer bytes those animations' samplers actually read (repacked
into one small buffer — everything else, including all mesh vertex/index
data and every texture byte, never makes it into the output file).

Drops entirely: meshes, materials, images, textures, texture samplers,
skins, cameras, extensions.

Usage:
    python3 extract_glb_animations.py input.glb output.glb [--scene N]
"""

import argparse
import json
import struct
import sys


COMPONENT_SIZES = {5120: 1, 5121: 1, 5122: 2, 5123: 2, 5125: 4, 5126: 4}
TYPE_COUNTS = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT2": 4, "MAT3": 9, "MAT4": 16}


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


def accessor_byte_length(gltf, accessor_idx):
    acc = gltf["accessors"][accessor_idx]
    elem_size = COMPONENT_SIZES[acc["componentType"]] * TYPE_COUNTS[acc["type"]]
    return acc["count"] * elem_size


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("input")
    ap.add_argument("output")
    ap.add_argument("--scene", type=int, default=None, help="scene index (default: the glTF's own default scene)")
    args = ap.parse_args()

    gltf, bin_bytes = read_glb(args.input)

    animations = gltf.get("animations", [])
    if not animations:
        sys.exit(f"{args.input}: no animations to extract")

    # --- 1. Which nodes survive: every animated node + all its ancestors.
    parent_of = {}
    for i, node in enumerate(gltf["nodes"]):
        for c in node.get("children", []):
            parent_of[c] = i

    keep_nodes = set()
    for anim in animations:
        for ch in anim["channels"]:
            target = ch["target"].get("node")
            if target is None:
                continue  # e.g. a weights-only channel with no node target
            n = target
            while n is not None and n not in keep_nodes:
                keep_nodes.add(n)
                n = parent_of.get(n)

    old_to_new_node = {old: new for new, old in enumerate(sorted(keep_nodes))}

    new_nodes = []
    for old_idx in sorted(keep_nodes):
        node = gltf["nodes"][old_idx]
        new_node = {}
        if "name" in node:
            new_node["name"] = node["name"]
        for key in ("translation", "rotation", "scale"):
            if key in node:
                new_node[key] = node[key]
        children = [old_to_new_node[c] for c in node.get("children", []) if c in keep_nodes]
        if children:
            new_node["children"] = children
        # Deliberately dropped even if present on a kept node: mesh, skin,
        # camera, weights, extras, extensions — this file never renders
        # anything, only drives AnimationPlayer targets by name.
        new_nodes.append(new_node)

    # --- 2. Which accessors survive: only ones animation samplers read.
    used_accessors = set()
    for anim in animations:
        for sampler in anim["samplers"]:
            used_accessors.add(sampler["input"])
            used_accessors.add(sampler["output"])

    old_to_new_accessor = {}
    new_accessors = []
    new_buffer_views = []
    new_bin = bytearray()
    for old_idx in sorted(used_accessors):
        acc = gltf["accessors"][old_idx]
        bv = gltf["bufferViews"][acc["bufferView"]]
        acc_offset = acc.get("byteOffset", 0)
        abs_offset = bv.get("byteOffset", 0) + acc_offset
        length = accessor_byte_length(gltf, old_idx)
        if "byteStride" in bv:
            sys.exit(f"accessor[{old_idx}]'s bufferView uses byteStride (interleaved) — not supported, unexpected for animation sampler data")
        raw = bin_bytes[abs_offset : abs_offset + length]

        pad = (-len(new_bin)) % 4
        new_bin.extend(b"\x00" * pad)
        new_bv_index = len(new_buffer_views)
        new_buffer_views.append({"buffer": 0, "byteOffset": len(new_bin), "byteLength": length})
        new_bin.extend(raw)

        new_acc = {k: v for k, v in acc.items() if k not in ("bufferView", "byteOffset")}
        new_acc["bufferView"] = new_bv_index
        old_to_new_accessor[old_idx] = len(new_accessors)
        new_accessors.append(new_acc)

    # --- 3. Animations, remapped to the new node/accessor indices.
    new_animations = []
    for anim in animations:
        new_anim = {"samplers": [], "channels": []}
        if "name" in anim:
            new_anim["name"] = anim["name"]
        for sampler in anim["samplers"]:
            new_sampler = {k: v for k, v in sampler.items()}
            new_sampler["input"] = old_to_new_accessor[sampler["input"]]
            new_sampler["output"] = old_to_new_accessor[sampler["output"]]
            new_anim["samplers"].append(new_sampler)
        for ch in anim["channels"]:
            new_ch = {"sampler": ch["sampler"], "target": dict(ch["target"])}
            if "node" in new_ch["target"]:
                new_ch["target"]["node"] = old_to_new_node[ch["target"]["node"]]
            new_anim["channels"].append(new_ch)
        new_animations.append(new_anim)

    # --- 4. Scene: just the surviving roots (animated-node ancestors that
    # were themselves original scene roots).
    scene_idx = args.scene if args.scene is not None else gltf.get("scene", 0)
    orig_scene_nodes = gltf["scenes"][scene_idx]["nodes"]
    new_scene_nodes = [old_to_new_node[n] for n in orig_scene_nodes if n in keep_nodes]
    if not new_scene_nodes:
        sys.exit("no animated nodes trace back to a scene root — check --scene")

    out = {
        "asset": gltf["asset"],
        "scene": 0,
        "scenes": [{"nodes": new_scene_nodes}],
        "nodes": new_nodes,
        "animations": new_animations,
        "accessors": new_accessors,
        "bufferViews": new_buffer_views,
        "buffers": [{"byteLength": len(new_bin)}],
    }

    write_glb(args.output, out, bytes(new_bin))
    print(f"{len(new_nodes)} nodes, {len(new_animations)} animations, {len(new_accessors)} accessors")
    print(f"buffer size: {len(new_bin)/1024:.1f} KB")
    print(f"wrote {args.output}")


if __name__ == "__main__":
    main()
