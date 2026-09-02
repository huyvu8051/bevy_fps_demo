#!/usr/bin/env python3
"""Downscales + recompresses a glb's embedded PNG texture(s) to JPEG via
macOS's `sips` (same "resize + convert with an OS tool, splice the result
back into the buffer" approach used ad hoc for soldier_meshy's own 52MB ->
1.6MB texture compression) — for a Meshy AI export whose single 8192x8192
PNG base-color texture accounts for essentially the entire file size.

Only the image bytes change: mesh/skin/animation/node data, and every
other bufferView, are carried over untouched (just relocated to make room
in one contiguous buffer, same repacking idiom as
extract_glb_mesh_only.py/extract_glb_animations.py).

Requires macOS (`sips`); does nothing fancy Windows/Linux-side because
there's no reason to duplicate what a builtin OS tool already does well.

Usage:
    python3 compress_glb_texture.py input.glb output.glb [--size 2048] [--quality 0.85]
"""

import argparse
import json
import shutil
import struct
import subprocess
import sys
import tempfile
from pathlib import Path


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
    ap.add_argument("--size", type=int, default=2048, help="max width/height in pixels (default: 2048)")
    ap.add_argument("--quality", type=float, default=0.85, help="JPEG quality 0..1 (default: 0.85)")
    args = ap.parse_args()

    if shutil.which("sips") is None:
        sys.exit("sips not found — this script is macOS-only")

    gltf, bin_bytes = read_glb(args.input)
    images = gltf.get("images", [])
    image_bvs = [i for i, im in enumerate(images) if "bufferView" in im]
    if not image_bvs:
        sys.exit(f"{args.input}: no embedded (bufferView-backed) images to compress")

    buffer_views = gltf["bufferViews"]
    # bufferView index -> replacement raw bytes (only entries actually recompressed).
    replacement = {}

    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        for img_idx in image_bvs:
            im = images[img_idx]
            bv = buffer_views[im["bufferView"]]
            offset = bv.get("byteOffset", 0)
            length = bv["byteLength"]
            raw = bin_bytes[offset : offset + length]

            src = tmp / f"src_{img_idx}.png"
            dst = tmp / f"dst_{img_idx}.jpg"
            src.write_bytes(raw)
            subprocess.run(
                [
                    "sips",
                    "-Z", str(args.size),
                    "-s", "format", "jpeg",
                    "-s", "formatOptions", str(int(args.quality * 100)),
                    str(src),
                    "--out", str(dst),
                ],
                check=True,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
            new_bytes = dst.read_bytes()
            replacement[im["bufferView"]] = new_bytes
            im["mimeType"] = "image/jpeg"
            print(f"image[{img_idx}]: {length/1024:.1f} KB -> {len(new_bytes)/1024:.1f} KB")

    # Repack every bufferView (unchanged ones verbatim, recompressed ones
    # swapped in) into one contiguous buffer, same idiom as the other
    # extract_glb_*.py scripts — needed since the recompressed image is a
    # different size than the slot it came from.
    new_buffer_views = []
    new_bin = bytearray()
    for i, bv in enumerate(buffer_views):
        if i in replacement:
            raw = replacement[i]
        else:
            offset = bv.get("byteOffset", 0)
            length = bv["byteLength"]
            raw = bin_bytes[offset : offset + length]

        pad = (-len(new_bin)) % 4
        new_bin.extend(b"\x00" * pad)
        new_bv = {k: v for k, v in bv.items() if k != "byteOffset"}
        new_bv["byteLength"] = len(raw)
        new_bv["byteOffset"] = len(new_bin)
        new_buffer_views.append(new_bv)
        new_bin.extend(raw)

    gltf["bufferViews"] = new_buffer_views
    gltf["buffers"] = [{"byteLength": len(new_bin)}]

    write_glb(args.output, gltf, bytes(new_bin))
    print(f"total: {len(bin_bytes)/1024/1024:.1f} MB -> {len(new_bin)/1024/1024:.1f} MB")
    print(f"wrote {args.output}")


if __name__ == "__main__":
    main()
