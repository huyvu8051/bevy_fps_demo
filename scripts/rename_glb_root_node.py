#!/usr/bin/env python3
"""Renames a glb's scene root node — e.g. a Meshy export whose root ships
as "Armature" instead of the "target_character" convention this project's
other Meshy-sourced rigs (soldier_meshy, zombie_meshy) use.

Why this matters at all: bevy_animation::AnimationTargetId hashes the FULL
ancestor NAME PATH down to each bone, root's own name included (see
bevy_gltf's collect_path, loader/gltf_ext/scene.rs) — so an animation-only
file's clips only retarget onto a mesh whose scene root is named exactly
the same. Renaming the root here (rather than teaching the game's own
wiring code yet another root-name constant) is what lets a new import
reuse an existing animation library file whose clips were baked against
"target_character", as long as every bone name below the root already
matches too (this only renames the ONE root node — it does nothing for a
rig whose actual bone names differ).

Only touches nodes[N].name for the given scene's root node(s); everything
else (transforms, meshes, skins, animations, buffers) is untouched.

Usage:
    python3 rename_glb_root_node.py input.glb output.glb --to target_character
    python3 rename_glb_root_node.py input.glb output.glb --from Armature --to target_character
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
    ap.add_argument("--to", required=True, help="new name for the scene root node")
    ap.add_argument("--from", dest="from_name", default=None, help="only rename if the root's current name matches this (safety check)")
    ap.add_argument("--scene", type=int, default=None, help="scene index (default: the glTF's own default scene)")
    args = ap.parse_args()

    gltf, bin_bytes = read_glb(args.input)
    scene_idx = args.scene if args.scene is not None else gltf.get("scene", 0)
    root_indices = gltf["scenes"][scene_idx]["nodes"]

    renamed = []
    for idx in root_indices:
        node = gltf["nodes"][idx]
        current = node.get("name")
        if args.from_name is not None and current != args.from_name:
            sys.exit(f"root node[{idx}] is named {current!r}, not {args.from_name!r} — refusing (pass the actual name or drop --from)")
        node["name"] = args.to
        renamed.append((idx, current))

    if not renamed:
        sys.exit(f"{args.input}: scene {scene_idx} has no root nodes to rename")

    write_glb(args.output, gltf, bin_bytes)
    for idx, old in renamed:
        print(f"node[{idx}]: {old!r} -> {args.to!r}")
    print(f"wrote {args.output}")


if __name__ == "__main__":
    main()
