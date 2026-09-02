#!/usr/bin/env python3
"""Bakes a uniform scale + Y offset + XYZ rotation correction into a glb's
scene root node(s) — the general-purpose successor to
normalize_glb_root_rotation.py (rotation-only) for this project's move away
from per-character runtime RON tuning (scale/mesh_y/rot_*_deg fields, F9-
written in the editor) toward baking the same numbers permanently into the
asset instead. Once baked, a character/zombie's mesh-child Transform in
code can just be the identity — no per-kind scale/offset/rotation lookup
needed at all.

Composes properly with whatever TRS the root node(s) already have (most
Meshy exports already carry a baked-in uniform scale, e.g. 0.01 to convert
cm to meters) rather than assuming they start at identity — see the
transform composition math in compose_trs's own comment.

Only edits the JSON chunk; the BIN chunk (vertex/animation data) is copied
byte-for-byte unchanged — every animation clip keeps working exactly as
authored, only the root's own placement changes, carrying the whole
hierarchy (mesh + skeleton) with it exactly the way parenting would.

Usage:
    python3 bake_mesh_transform.py input.glb output.glb \\
        --scale 1.0 --offset-y -1.0 --rot-x 90 --rot-y 0 --rot-z 0

Does NOT touch node `matrix` (rare in practice) — errors out rather than
silently doing the wrong thing if a root node uses one instead of TRS.
"""

import argparse
import json
import math
import struct
import sys


def quat_mul(a, b):
    ax, ay, az, aw = a
    bx, by, bz, bw = b
    return (
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    )


def quat_from_axis_angle_deg(axis, deg):
    rad = math.radians(deg)
    s = math.sin(rad / 2)
    return (axis[0] * s, axis[1] * s, axis[2] * s, math.cos(rad / 2))


def quat_apply(q, v):
    qx, qy, qz, qw = q
    vx, vy, vz = v
    # v + 2*cross(q.xyz, cross(q.xyz, v) + q.w*v)
    cx1, cy1, cz1 = (qy * vz - qz * vy, qz * vx - qx * vz, qx * vy - qy * vx)
    tx, ty, tz = (cx1 + qw * vx, cy1 + qw * vy, cz1 + qw * vz)
    cx2, cy2, cz2 = (qy * tz - qz * ty, qz * tx - qx * tz, qx * ty - qy * tx)
    return (vx + 2 * cx2, vy + 2 * cy2, vz + 2 * cz2)


def compose_trs(parent_t, parent_r, parent_s, child_t, child_r, child_s):
    """New TRS for a node that had (child_t, child_r, child_s) if a parent
    with (parent_t, parent_r, parent_s) were baked directly into it —
    standard TRS composition: world = parent * child, expressed back as a
    single node's own TRS. Only exact for a UNIFORM parent scale (true for
    every use in this project — a single scalar scale field)."""
    new_scale = tuple(p * c for p, c in zip(parent_s, child_s))
    new_rotation = quat_mul(parent_r, child_r)
    scaled_child_t = tuple(p * c for p, c in zip(parent_s, child_t))
    rotated = quat_apply(parent_r, scaled_child_t)
    new_translation = tuple(p + r for p, r in zip(parent_t, rotated))
    return new_translation, new_rotation, new_scale


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

    has_bin = len(padded_bin) > 0
    total_len = 12 + 8 + len(new_json_bytes) + (8 + len(padded_bin) if has_bin else 0)
    with open(path, "wb") as f:
        f.write(struct.pack("<III", 0x46546C67, 2, total_len))
        f.write(struct.pack("<II", len(new_json_bytes), 0x4E4F534A))  # 'JSON'
        f.write(new_json_bytes)
        if has_bin:
            f.write(struct.pack("<II", len(padded_bin), 0x004E4942))  # 'BIN\0'
            f.write(padded_bin)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("input")
    ap.add_argument("output")
    ap.add_argument("--scale", type=float, default=1.0, help="uniform scale to bake in (default: 1.0, no-op)")
    ap.add_argument("--offset-y", type=float, default=0.0, help="Y translation to bake in (default: 0.0, no-op)")
    ap.add_argument("--rot-x", type=float, default=0.0, help="degrees around X")
    ap.add_argument("--rot-y", type=float, default=0.0, help="degrees around Y")
    ap.add_argument("--rot-z", type=float, default=0.0, help="degrees around Z")
    ap.add_argument("--scene", type=int, default=None, help="scene index (default: the glTF's own default scene)")
    args = ap.parse_args()

    gltf, bin_bytes = read_glb(args.input)
    scene_idx = args.scene if args.scene is not None else gltf.get("scene", 0)
    root_indices = gltf["scenes"][scene_idx]["nodes"]

    # Same EulerRot::YXZ compose order this project's runtime code has
    # always used for these 3 fields (Quat::from_euler(YXZ, y, x, z)).
    parent_r = quat_mul(
        quat_mul(quat_from_axis_angle_deg((0, 1, 0), args.rot_y), quat_from_axis_angle_deg((1, 0, 0), args.rot_x)),
        quat_from_axis_angle_deg((0, 0, 1), args.rot_z),
    )
    parent_t = (0.0, args.offset_y, 0.0)
    parent_s = (args.scale, args.scale, args.scale)

    for i in root_indices:
        node = gltf["nodes"][i]
        if "matrix" in node:
            sys.exit(f"root node[{i}] ({node.get('name')!r}) uses a matrix, not TRS — not supported, fix by hand")
        child_t = tuple(node.get("translation", (0.0, 0.0, 0.0)))
        child_r = tuple(node.get("rotation", (0.0, 0.0, 0.0, 1.0)))
        child_s = tuple(node.get("scale", (1.0, 1.0, 1.0)))

        new_t, new_r, new_s = compose_trs(parent_t, parent_r, parent_s, child_t, child_r, child_s)
        node["translation"] = list(new_t)
        node["rotation"] = list(new_r)
        node["scale"] = list(new_s)
        print(f"root node[{i}] ({node.get('name')!r}):")
        print(f"  translation {child_t} -> {new_t}")
        print(f"  rotation    {child_r} -> {new_r}")
        print(f"  scale       {child_s} -> {new_s}")

    write_glb(args.output, gltf, bin_bytes)
    print(f"wrote {args.output}")


if __name__ == "__main__":
    main()
