#!/usr/bin/env python3
"""Bakes a rotation correction into a glTF/.glb file's scene root node(s).

Fixes meshes (typically Meshy AI exports) whose rest orientation doesn't
match Bevy's convention (Y-up, -Z-forward) — e.g. a rig shipped lying flat
on its back — PERMANENTLY, once, in the asset itself. This replaces
per-character runtime rotation fields (pitch_deg/roll_deg in
character.ron): normalize the file with this tool instead, and every
consuming code path (local player, remote NetPlayer spawn, character
switching) only ever needs the same plain Y-axis facing_deg every other
character already uses.

Only edits the JSON chunk (composes the correction into each scene root
node's `rotation`, defaulting to identity if absent) — the BIN chunk
(vertex/animation data) is copied byte-for-byte unchanged, so every
animation clip keeps working exactly as authored; only the root's own
orientation changes, carrying the whole hierarchy (mesh + skeleton) with
it exactly the way parenting would.

Usage:
    python3 normalize_glb_root_rotation.py input.glb output.glb \\
        --x 90 --y 0 --z 0

Degrees, applied as intrinsic X-then-Y-then-Z (compose order chosen to
match this project's existing EulerRot::YXZ convention closely enough for
a one-axis-at-a-time correction — for a single-axis fix, as is almost
always the case, the compose order doesn't matter at all).

Does NOT touch node `matrix` (rare in practice) — errors out rather than
silently doing the wrong thing if a root node uses one instead of TRS.
"""

import argparse
import json
import math
import struct
import sys


def mat4_to_trs(m):
    """Decomposes a column-major glTF node matrix into (translation,
    rotation quaternion, scale). Assumes no shear; errors out if the
    columns aren't orthogonal (a shear/skew this can't represent as TRS)."""
    translation = (m[12], m[13], m[14])
    cols = [(m[0], m[1], m[2]), (m[4], m[5], m[6]), (m[8], m[9], m[10])]
    scale = tuple(math.sqrt(sum(c * c for c in col)) for col in cols)
    rot_cols = [tuple(c / s for c in col) for col, s in zip(cols, scale)]

    def dot(a, b):
        return sum(x * y for x, y in zip(a, b))

    for i in range(3):
        for j in range(i + 1, 3):
            if abs(dot(rot_cols[i], rot_cols[j])) > 1e-4:
                sys.exit("matrix root has shear/skew — not supported, fix by hand")

    m00, m10, m20 = rot_cols[0]
    m01, m11, m21 = rot_cols[1]
    m02, m12, m22 = rot_cols[2]
    trace = m00 + m11 + m22
    if trace > 0:
        s = math.sqrt(trace + 1.0) * 2
        qw = 0.25 * s
        qx = (m21 - m12) / s
        qy = (m02 - m20) / s
        qz = (m10 - m01) / s
    elif m00 > m11 and m00 > m22:
        s = math.sqrt(1.0 + m00 - m11 - m22) * 2
        qw = (m21 - m12) / s
        qx = 0.25 * s
        qy = (m01 + m10) / s
        qz = (m02 + m20) / s
    elif m11 > m22:
        s = math.sqrt(1.0 + m11 - m00 - m22) * 2
        qw = (m02 - m20) / s
        qx = (m01 + m10) / s
        qy = 0.25 * s
        qz = (m12 + m21) / s
    else:
        s = math.sqrt(1.0 + m22 - m00 - m11) * 2
        qw = (m10 - m01) / s
        qx = (m02 + m20) / s
        qy = (m12 + m21) / s
        qz = 0.25 * s
    return translation, (qx, qy, qz, qw), scale


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


def read_glb(path):
    with open(path, "rb") as f:
        magic, version, _total_len = struct.unpack("<III", f.read(12))
        if magic != 0x46546C67:
            sys.exit(f"{path}: not a glb (bad magic)")
        j_len, j_type = struct.unpack("<II", f.read(8))
        json_bytes = f.read(j_len)
        bin_bytes = b""
        chunk = f.read(8)
        if chunk:
            b_len, b_type = struct.unpack("<II", chunk)
            bin_bytes = f.read(b_len)
    return json.loads(json_bytes), bin_bytes


def write_glb(path, gltf_json, bin_bytes):
    new_json_bytes = json.dumps(gltf_json, separators=(",", ":")).encode("utf-8")
    json_pad = (-len(new_json_bytes)) % 4
    new_json_bytes += b" " * json_pad  # glTF pads the JSON chunk with spaces

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
    ap.add_argument("--x", type=float, default=0.0, help="degrees around X")
    ap.add_argument("--y", type=float, default=0.0, help="degrees around Y")
    ap.add_argument("--z", type=float, default=0.0, help="degrees around Z")
    ap.add_argument("--scene", type=int, default=None, help="scene index (default: the glTF's own default scene)")
    args = ap.parse_args()

    gltf, bin_bytes = read_glb(args.input)

    scene_idx = args.scene if args.scene is not None else gltf.get("scene", 0)
    root_indices = gltf["scenes"][scene_idx]["nodes"]

    correction = quat_mul(
        quat_mul(quat_from_axis_angle_deg((1, 0, 0), args.x), quat_from_axis_angle_deg((0, 1, 0), args.y)),
        quat_from_axis_angle_deg((0, 0, 1), args.z),
    )

    for i in root_indices:
        node = gltf["nodes"][i]
        if "matrix" in node:
            translation, existing, scale = mat4_to_trs(node["matrix"])
            del node["matrix"]
            node["translation"] = list(translation)
            if scale != (1.0, 1.0, 1.0):
                node["scale"] = list(scale)
            print(f"root node[{i}] ({node.get('name')!r}): decomposed matrix -> translation={translation} rotation={existing} scale={scale}")
        else:
            existing = tuple(node.get("rotation", (0.0, 0.0, 0.0, 1.0)))
        node["rotation"] = list(quat_mul(correction, existing))
        print(f"root node[{i}] ({node.get('name')!r}): rotation {existing} -> {node['rotation']}")

    write_glb(args.output, gltf, bin_bytes)
    print(f"wrote {args.output}")


if __name__ == "__main__":
    main()
