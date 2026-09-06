# Run INSIDE Unreal Editor's Python console (Window > Developer Tools >
# Python Console), not from a regular terminal — this uses the `unreal`
# module, which only exists inside a running Unreal Editor process.
#
# Generates the Khu B container maze for Map 12 (docs/maps/
# map12_dimensions.md) as real placed Actors in the currently open Level:
# a 4x3 grid of container-sized boxes, some cells randomly removed to
# carve winding "lối đi ngoằn ngoèo" corridors, some remaining cells
# doubled up as a second stacked layer ("container xếp chồng nhiều
# tầng"), plus a fixed (non-random) row for the lore-critical sealed
# Meridian container row (447-460, see map12_cat_lai_freight_yard.md).
#
# No asset import needed to try this out — CONTAINER_MESH_PATH defaults to
# Unreal's built-in Cube primitive, scaled to real TEU container
# dimensions (2.4 x 2.6 x 6.1m). Swap it for a real container static mesh
# once you have one (Meshy.ai export, Marketplace kit, etc.) — everything
# else (grid math, maze carving, JSON export) stays the same.
#
# After running, `export_path` below holds a JSON list of every spawned
# container's (mesh, location, rotation, scale) — the next pipeline step
# (see the chat conversation this script came from) converts that into a
# RON file this project's `shared::read_ron_data_file!` can load, spawned
# in Bevy with the same generic pattern `item_gallery_cars.rs`'s
# `spawn_item_gallery_test_props` already uses for car props.

import json
import random

import unreal

# --- Config: matches docs/maps/map12_dimensions.md, Khu B ---
CONTAINER_MESH_PATH = "/Engine/BasicShapes/Cube.Cube"  # placeholder; swap for a real container mesh later
CONTAINER_LENGTH = 610.0  # 6.1m, Unreal units are centimeters
CONTAINER_WIDTH = 240.0  # 2.4m
CONTAINER_HEIGHT = 260.0  # 2.6m
# Engine Cube's own unscaled size is 100x100x100uu, hence /100.0 below to
# turn "real container size in cm" into a Cube scale factor.
CUBE_UNIT = 100.0

GRID_COLS = 4
GRID_ROWS = 3
SPACING_X = CONTAINER_LENGTH + 100.0  # 1m gap between rows, matches "lối đi" width
SPACING_Y = CONTAINER_WIDTH + 100.0
ORIGIN_X = 0.0
ORIGIN_Y = 0.0
ORIGIN_Z = CONTAINER_HEIGHT / 2.0  # container sits on the ground, pivot is centered

MAZE_REMOVE_CHANCE = 0.35  # fraction of grid cells carved out as corridors
STACK_CHANCE = 0.3  # fraction of remaining cells that also get a 2nd container stacked on top

SEALED_ROW_COUNT = 6  # container 447-460 in the doc — count kept small/symbolic, not literally 14
SEALED_ROW_GAP = 300.0  # cm, extra clearance separating it from the random maze above

random.seed(12)  # reproducible layout — change or remove to get a different maze each run

editor_actor_subsystem = unreal.get_editor_subsystem(unreal.EditorActorSubsystem)
container_mesh = unreal.EditorAssetLibrary.load_asset(CONTAINER_MESH_PATH)
if container_mesh is None:
    raise RuntimeError(f"Could not load {CONTAINER_MESH_PATH} — check the path or swap in your own mesh asset.")

placed = []  # accumulates {mesh, location, rotation_yaw, scale} for the JSON export


def spawn_container(x, y, z, yaw):
    location = unreal.Vector(x, y, z)
    rotation = unreal.Rotator(0.0, yaw, 0.0)
    actor = editor_actor_subsystem.spawn_actor_from_object(container_mesh, location, rotation)
    scale = unreal.Vector(CONTAINER_LENGTH / CUBE_UNIT, CONTAINER_WIDTH / CUBE_UNIT, CONTAINER_HEIGHT / CUBE_UNIT)
    actor.set_actor_scale3d(scale)
    actor.set_actor_label(f"Map12_Container_{len(placed)}")
    placed.append({
        "mesh": CONTAINER_MESH_PATH,
        "location": [x, y, z],
        "rotation_yaw": yaw,
        "scale": [scale.x, scale.y, scale.z],
    })
    return actor


# --- Khu B maze grid ---
for row in range(GRID_ROWS):
    for col in range(GRID_COLS):
        if random.random() < MAZE_REMOVE_CHANCE:
            continue  # carve a gap here -> becomes a walkable corridor
        x = ORIGIN_X + col * SPACING_X
        y = ORIGIN_Y + row * SPACING_Y
        yaw = random.choice([0.0, 90.0, 180.0, 270.0])
        spawn_container(x, y, ORIGIN_Z, yaw)
        if random.random() < STACK_CHANCE:
            spawn_container(x, y, ORIGIN_Z + CONTAINER_HEIGHT, yaw)

# --- Sealed Meridian container row (447-460) — fixed placement, not random,
# since this is a lore-critical prop (the one pried open from the inside,
# linking to Map 13) that needs a predictable, findable position. ---
sealed_row_y = ORIGIN_Y + GRID_ROWS * SPACING_Y + SEALED_ROW_GAP
for i in range(SEALED_ROW_COUNT):
    x = ORIGIN_X + i * SPACING_X
    spawn_container(x, sealed_row_y, ORIGIN_Z, 0.0)

# --- Export layout for the Bevy import pipeline ---
export_path = "/tmp/map12_container_layout.json"
with open(export_path, "w") as f:
    json.dump(placed, f, indent=2)

unreal.log(f"[map12_pcg] Spawned {len(placed)} containers, layout exported to {export_path}")
