#!/usr/bin/env python3
"""Appends specific named animation clips from a donor glb into a base
animation-library glb (extract_glb_animations.py's own output shape:
skeleton nodes + animations, no mesh/texture) — for pulling a few new
clips (e.g. death/hit-react) out of a fresh Meshy export into an existing
shared library like mixamo_base.glb, without re-generating the whole file.

Requires the donor's animated nodes to be a subset of the base file's own
node NAMES (not indices — matched by name, since the two files come from
separate glTF exports with their own node ordering). This is exactly the
same requirement wire_*_scene_animation_targets (crates/client) already
leans on for retargeting: bone names must match. Refuses (rather than
silently dropping data) if the donor targets a node name absent from the
base file.

Usage:
    python3 merge_glb_animations.py base.glb donor.glb output.glb \
        --clip Shot_and_Fall_Backward --clip Shot_and_Fall_Forward
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
    ap.add_argument("base")
    ap.add_argument("donor")
    ap.add_argument("output")
    ap.add_argument("--clip", action="append", required=True, dest="clips", help="donor animation name to import (repeatable)")
    args = ap.parse_args()

    base, base_bin = read_glb(args.base)
    donor, donor_bin = read_glb(args.donor)

    base_name_to_idx = {n.get("name"): i for i, n in enumerate(base["nodes"]) if "name" in n}
    donor_idx_to_name = {i: n.get("name") for i, n in enumerate(donor["nodes"])}

    wanted = {c: None for c in args.clips}
    for a in donor["animations"]:
        if a.get("name") in wanted:
            wanted[a["name"]] = a
    missing = [c for c, a in wanted.items() if a is None]
    if missing:
        sys.exit(f"donor has no animation(s) named: {missing}")

    new_accessors = list(base["accessors"])
    new_buffer_views = list(base["bufferViews"])
    new_bin = bytearray(base_bin)
    new_animations = list(base["animations"])

    for clip_name in args.clips:
        anim = wanted[clip_name]
        donor_to_base_accessor = {}

        def copy_accessor(donor_acc_idx):
            if donor_acc_idx in donor_to_base_accessor:
                return donor_to_base_accessor[donor_acc_idx]
            acc = donor["accessors"][donor_acc_idx]
            bv = donor["bufferViews"][acc["bufferView"]]
            if "byteStride" in bv:
                sys.exit(f"{clip_name}: accessor[{donor_acc_idx}]'s bufferView is interleaved — not supported for animation sampler data")
            abs_offset = bv.get("byteOffset", 0) + acc.get("byteOffset", 0)
            length = accessor_byte_length(donor, donor_acc_idx)
            raw = donor_bin[abs_offset : abs_offset + length]

            pad = (-len(new_bin)) % 4
            new_bin.extend(b"\x00" * pad)
            new_bv_idx = len(new_buffer_views)
            new_buffer_views.append({"buffer": 0, "byteOffset": len(new_bin), "byteLength": length})
            new_bin.extend(raw)

            new_acc = {k: v for k, v in acc.items() if k not in ("bufferView", "byteOffset")}
            new_acc["bufferView"] = new_bv_idx
            new_acc_idx = len(new_accessors)
            new_accessors.append(new_acc)
            donor_to_base_accessor[donor_acc_idx] = new_acc_idx
            return new_acc_idx

        new_anim = {"name": clip_name, "samplers": [], "channels": []}
        for sampler in anim["samplers"]:
            new_anim["samplers"].append(
                {
                    **{k: v for k, v in sampler.items() if k not in ("input", "output")},
                    "input": copy_accessor(sampler["input"]),
                    "output": copy_accessor(sampler["output"]),
                }
            )
        for ch in anim["channels"]:
            target = dict(ch["target"])
            if "node" in target:
                donor_node_name = donor_idx_to_name.get(target["node"])
                if donor_node_name not in base_name_to_idx:
                    sys.exit(f"{clip_name}: targets node {donor_node_name!r}, which the base file has no node named — refusing")
                target["node"] = base_name_to_idx[donor_node_name]
            new_anim["channels"].append({"sampler": ch["sampler"], "target": target})
        new_animations.append(new_anim)
        print(f"imported {clip_name!r}: {len(anim['channels'])} channels")

    out = dict(base)
    out["accessors"] = new_accessors
    out["bufferViews"] = new_buffer_views
    out["buffers"] = [{"byteLength": len(new_bin)}]
    out["animations"] = new_animations

    write_glb(args.output, out, bytes(new_bin))
    print(f"{len(base['animations'])} -> {len(new_animations)} animations")
    print(f"wrote {args.output}")


if __name__ == "__main__":
    main()
