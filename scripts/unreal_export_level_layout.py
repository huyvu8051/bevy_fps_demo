# Run INSIDE Unreal Editor's Python console (Window > Developer Tools >
# Python Console), not a regular shell — uses the `unreal` module, which
# only exists inside a running Unreal Editor process.
#
# Generic "export whatever level is currently open" step of the
# Unreal-to-Bevy pipeline: unlike unreal_generate_map12_containers.py
# (which PROCEDURALLY builds a level), this one just reads back whatever
# is ALREADY in the open level — e.g. a demo/showcase map that shipped
# with a purchased/free asset pack (Factory Environment Collection etc.)
# — and produces the same two outputs the rest of the pipeline expects:
#
# 1. A JSON layout file (every Static Mesh Actor's mesh reference +
#    transform) — the next pipeline step converts this into a RON file
#    loadable the same way item_gallery_cars.rs already loads
#    item_gallery_car_physics.ron, spawned with a generic Rust fn mirroring
#    spawn_item_gallery_test_props.
# 2. Every UNIQUE mesh referenced in that layout, auto-exported to FBX —
#    import each into Blender once, clean up if needed (poly count,
#    pivot), export glTF for Bevy's asset_server.load() to actually use.
#
# Open the level you want to export FIRST (double-click it in Content
# Browser, or File > Open Level), THEN run this script.

import json
import os

import unreal

# --- Config ---
OUTPUT_DIR = "/tmp/unreal_level_export"
LAYOUT_JSON_PATH = os.path.join(OUTPUT_DIR, "level_layout.json")
FBX_OUTPUT_DIR = os.path.join(OUTPUT_DIR, "meshes")

os.makedirs(FBX_OUTPUT_DIR, exist_ok=True)

editor_actor_subsystem = unreal.get_editor_subsystem(unreal.EditorActorSubsystem)
all_actors = editor_actor_subsystem.get_all_level_actors()

layout = []
unique_meshes = {}  # mesh package path -> the StaticMesh asset object (for the export pass below)

for actor in all_actors:
    mesh_component = actor.get_component_by_class(unreal.StaticMeshComponent)
    if mesh_component is None:
        continue  # skip lights, triggers, non-mesh actors — nothing to export/spawn for these
    static_mesh = mesh_component.static_mesh
    if static_mesh is None:
        continue

    mesh_path = static_mesh.get_path_name()  # e.g. "/Game/FactoryEnvironmentCollection/Meshes/SM_Wall01.SM_Wall01"
    unique_meshes[mesh_path] = static_mesh

    location = actor.get_actor_location()
    rotation = actor.get_actor_rotation()
    scale = actor.get_actor_scale3d()

    layout.append({
        "mesh": mesh_path,
        "location": [location.x, location.y, location.z],
        "rotation": [rotation.roll, rotation.pitch, rotation.yaw],
        "scale": [scale.x, scale.y, scale.z],
    })

with open(LAYOUT_JSON_PATH, "w") as f:
    json.dump(layout, f, indent=2)

unreal.log(f"[level_export] Wrote {len(layout)} actor transforms to {LAYOUT_JSON_PATH}")
unreal.log(f"[level_export] {len(unique_meshes)} unique meshes referenced — exporting to FBX...")

# --- Export every unique mesh to FBX ---
exported = 0
failed = []
for mesh_path, static_mesh in unique_meshes.items():
    # Flatten "/Game/Foo/Bar/SM_Wall01.SM_Wall01" -> "SM_Wall01.fbx" — good
    # enough as long as mesh NAMES are unique across the pack (usually
    # true for a single asset pack); rename by hand afterward if two
    # different meshes collide on the same short name.
    mesh_name = mesh_path.split("/")[-1].split(".")[0]
    export_path = os.path.join(FBX_OUTPUT_DIR, f"{mesh_name}.fbx")

    task = unreal.AssetExportTask()
    task.object = static_mesh
    task.filename = export_path
    task.automated = True
    task.replace_identical = True
    task.prompt = False
    task.exporter = unreal.StaticMeshExporterFBX()

    success = unreal.Exporter.run_asset_export_task(task)
    if success:
        exported += 1
    else:
        failed.append(mesh_path)

unreal.log(f"[level_export] Exported {exported}/{len(unique_meshes)} meshes to {FBX_OUTPUT_DIR}")
if failed:
    unreal.log_warning(f"[level_export] Failed to export: {failed}")
    unreal.log_warning("[level_export] Fall back to manual export for these: right-click the "
                        "asset in Content Browser -> Asset Actions -> Export...")
