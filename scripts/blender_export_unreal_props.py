# Run INSIDE Blender's own Python console or Scripting tab (uses the
# `bpy` module, which only exists inside a running Blender process) — not
# a regular shell.
#
# Automates the last leg of the Unreal-to-Bevy pipeline for whatever's
# currently imported in the Blender scene (e.g. the 7 FBX files from
# unreal_export_level_layout.py, already imported via File > Import > FBX):
# deletes Unreal's "UCX_" collision-proxy objects (this project generates
# its own collision via avian's ColliderConstructor::ConvexHullFromMesh,
# same as every other prop — no need to carry Unreal's custom collision
# mesh over at all), applies all transforms so each glTF exports clean,
# then exports each real mesh object to its own .glb file.
#
# Set OUTPUT_DIR below to your actual repo path if this isn't running on
# the same machine/location.

import os

import bpy

OUTPUT_DIR = "/Users/huyvu8051/projects/bevy_fps_demo/crates/client/assets/props/unreal_import"
os.makedirs(OUTPUT_DIR, exist_ok=True)

# --- Delete Unreal's collision-proxy objects (UCX_* prefix convention) ---
collision_objects = [obj for obj in bpy.context.scene.objects if obj.name.startswith("UCX_")]
for obj in collision_objects:
    bpy.data.objects.remove(obj, do_unlink=True)
print(f"[blender_export] Deleted {len(collision_objects)} UCX_ collision-proxy objects")

# --- Apply transforms on every remaining (real) mesh object ---
real_objects = [obj for obj in bpy.context.scene.objects if obj.type == "MESH"]
bpy.ops.object.select_all(action="DESELECT")
for obj in real_objects:
    obj.select_set(True)
if real_objects:
    bpy.context.view_layer.objects.active = real_objects[0]
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
print(f"[blender_export] Applied transforms on {len(real_objects)} mesh objects")

# --- Export each object to its own .glb, named after the object ---
exported = []
for obj in real_objects:
    bpy.ops.object.select_all(action="DESELECT")
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj

    export_path = os.path.join(OUTPUT_DIR, f"{obj.name}.glb")
    bpy.ops.export_scene.gltf(
        filepath=export_path,
        export_format="GLB",
        use_selection=True,
    )
    exported.append(export_path)

print(f"[blender_export] Exported {len(exported)} .glb files to {OUTPUT_DIR}:")
for path in exported:
    print(f"  {path}")
