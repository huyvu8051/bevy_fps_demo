#!/usr/bin/env python3
"""Retargets a glb's skeleton so its joints' REST rotations match a
reference rig's, joint-for-joint by name — the actual fix for reusing an
animation library across two same-named/same-hierarchy rigs whose joints
were authored with different local rotation conventions (bone names and
parent/child structure matching alone is NOT enough: bevy_animation clips
set each joint's LOCAL rotation to an ABSOLUTE value every frame, and that
value only produces correct motion if the target joint's "zero rotation"
means the same thing the clip's source rig's did).

What this does, per matched joint: replaces its local `rotation` with the
reference's (translation/scale — bone length/position — stay this mesh's
own, unchanged), then recomputes every joint's WORLD transform from the
new local rotations, and rebuilds the skin's `inverseBindMatrices`
accordingly so the mesh deforms correctly around its NEW rest pose instead
of getting distorted. Vertex positions/weights/UVs/materials are never
touched — only node `rotation` fields and the one inverseBindMatrices
accessor's data change.

Trade-off this accepts: the mesh's STATIC bind pose (no animation playing)
will now look like the reference's T-pose/rest convention, not its
original export's — that's the whole point (so a shared animation clip's
absolute rotation values land correctly), not a bug.

Only joints present in BOTH files (matched by node name) are touched;
anything in target's skin.joints with no name match in reference is left
as-is (and printed as a warning — the retarget is almost certainly wrong
for a name that doesn't exist in the reference skeleton at all).

Usage:
    python3 retarget_rest_pose.py target.glb reference.glb output.glb
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


def quat_mul(a, b):
    ax, ay, az, aw = a
    bx, by, bz, bw = b
    return (
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    )


def quat_conj(q):
    x, y, z, w = q
    return (-x, -y, -z, w)


def quat_apply(q, v):
    qx, qy, qz, qw = q
    vx, vy, vz = v
    cx1, cy1, cz1 = (qy * vz - qz * vy, qz * vx - qx * vz, qx * vy - qy * vx)
    tx, ty, tz = (cx1 + qw * vx, cy1 + qw * vy, cz1 + qw * vz)
    cx2, cy2, cz2 = (qy * tz - qz * ty, qz * tx - qx * tz, qx * ty - qy * tx)
    return (vx + 2 * cx2, vy + 2 * cy2, vz + 2 * cz2)


def compose_trs(parent_t, parent_r, parent_s, child_t, child_r, child_s):
    """world = parent * child, for a uniform-scale TRS pair."""
    new_scale = tuple(p * c for p, c in zip(parent_s, child_s))
    new_rotation = quat_mul(parent_r, child_r)
    scaled_child_t = tuple(p * c for p, c in zip(parent_s, child_t))
    rotated = quat_apply(parent_r, scaled_child_t)
    new_translation = tuple(p + r for p, r in zip(parent_t, rotated))
    return new_translation, new_rotation, new_scale


def invert_trs(t, r, s):
    """Inverse of a uniform-scale TRS, itself expressed as a TRS."""
    s_inv = tuple(1.0 / c for c in s)
    r_inv = quat_conj(r)
    neg_t_rotated = quat_apply(r_inv, t)
    t_inv = tuple(-si * v for si, v in zip(s_inv, neg_t_rotated))
    return t_inv, r_inv, s_inv


def trs_to_mat4_column_major(t, r, s):
    x, y, z, w = r
    xx, yy, zz = x * x, y * y, z * z
    xy, xz, yz = x * y, x * z, y * z
    wx, wy, wz = w * x, w * y, w * z
    # Rotation matrix, row i = R applied to basis vector i (standard quat->matrix).
    r00, r01, r02 = 1 - 2 * (yy + zz), 2 * (xy - wz), 2 * (xz + wy)
    r10, r11, r12 = 2 * (xy + wz), 1 - 2 * (xx + zz), 2 * (yz - wx)
    r20, r21, r22 = 2 * (xz - wy), 2 * (yz + wx), 1 - 2 * (xx + yy)
    sx, sy, sz = s
    tx, ty, tz = t
    # glTF matrix storage: column-major, 16 floats = col0,col1,col2,col3.
    return [
        r00 * sx, r10 * sx, r20 * sx, 0.0,
        r01 * sy, r11 * sy, r21 * sy, 0.0,
        r02 * sz, r12 * sz, r22 * sz, 0.0,
        tx, ty, tz, 1.0,
    ]


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("target")
    ap.add_argument("reference")
    ap.add_argument("output")
    ap.add_argument("--skin", type=int, default=0, help="target's skin index (default: 0)")
    args = ap.parse_args()

    target, target_bin = read_glb(args.target)
    reference, _ref_bin = read_glb(args.reference)

    ref_rotation_by_name = {n.get("name"): tuple(n.get("rotation", (0.0, 0.0, 0.0, 1.0))) for n in reference["nodes"] if "name" in n}

    skin = target["skins"][args.skin]
    joint_indices = skin["joints"]
    joint_names = {target["nodes"][i].get("name") for i in joint_indices}

    matched, unmatched = 0, []
    for i in joint_indices:
        node = target["nodes"][i]
        name = node.get("name")
        if name in ref_rotation_by_name:
            node["rotation"] = list(ref_rotation_by_name[name])
            matched += 1
        else:
            unmatched.append(name)
    print(f"matched {matched}/{len(joint_indices)} joints against the reference rig")
    if unmatched:
        print(f"WARNING: no reference match for: {unmatched} — left unchanged, retarget is likely wrong for these")

    # Recompute every node's world (t, r, s) top-down from the scene
    # root(s), using target's own (possibly just-updated) local TRS.
    parent_of = {}
    for i, n in enumerate(target["nodes"]):
        for c in n.get("children", []):
            parent_of[c] = i

    world_cache = {}

    def world_trs(i):
        if i in world_cache:
            return world_cache[i]
        node = target["nodes"][i]
        local_t = tuple(node.get("translation", (0.0, 0.0, 0.0)))
        local_r = tuple(node.get("rotation", (0.0, 0.0, 0.0, 1.0)))
        local_s = tuple(node.get("scale", (1.0, 1.0, 1.0)))
        if i in parent_of:
            pt, pr, ps = world_trs(parent_of[i])
            result = compose_trs(pt, pr, ps, local_t, local_r, local_s)
        else:
            result = (local_t, local_r, local_s)
        world_cache[i] = result
        return result

    # --- Rebuild inverseBindMatrices from the new world transforms. ---
    ibm_acc_idx = skin["inverseBindMatrices"]
    old_acc = target["accessors"][ibm_acc_idx]
    if old_acc["componentType"] != 5126 or old_acc["type"] != "MAT4":
        sys.exit(f"inverseBindMatrices accessor[{ibm_acc_idx}] isn't a float MAT4 — unexpected, refusing")

    new_bin = bytearray()
    for i in joint_indices:
        wt, wr, ws = world_trs(i)
        it, ir, iss = invert_trs(wt, wr, ws)
        mat = trs_to_mat4_column_major(it, ir, iss)
        for v in mat:
            new_bin.extend(struct.pack("<f", v))

    # Repack: keep every OTHER bufferView/accessor exactly as-is, just
    # append the new inverseBindMatrices bytes as one fresh bufferView at
    # the end of the buffer (simplest correct repack — no need to touch
    # anything the old one's bufferView slot held, since nothing else
    # shares it: a skin's IBM accessor is exclusively its own bufferView
    # in every export this project has seen).
    old_bv = target["bufferViews"][old_acc["bufferView"]]
    if old_bv["byteLength"] != len(new_bin):
        print(f"note: new IBM data is {len(new_bin)} bytes vs old {old_bv['byteLength']} (should match exactly for MAT4 x {len(joint_indices)} joints)")

    full_bin = bytearray(target_bin)
    pad = (-len(full_bin)) % 4
    full_bin.extend(b"\x00" * pad)
    new_bv_idx = len(target["bufferViews"])
    target["bufferViews"].append({"buffer": 0, "byteOffset": len(full_bin), "byteLength": len(new_bin)})
    full_bin.extend(new_bin)

    new_acc = {k: v for k, v in old_acc.items() if k not in ("bufferView", "byteOffset")}
    new_acc["bufferView"] = new_bv_idx
    new_acc_idx = len(target["accessors"])
    target["accessors"].append(new_acc)
    skin["inverseBindMatrices"] = new_acc_idx

    target["buffers"] = [{"byteLength": len(full_bin)}]

    write_glb(args.output, target, bytes(full_bin))
    print(f"wrote {args.output}")


if __name__ == "__main__":
    main()
