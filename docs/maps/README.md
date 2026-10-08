# docs/maps/

`gdd/` holds the 20 designed campaign maps (`map01_*.md`..`map20_*.md`,
each with a matching `mapNN_dimensions.md`) — pure game-design reference.
This numbering is **unrelated** to the code's `Scenario::map_id()` (1/2/3,
see `crates/shared/src/network.rs`).

Only 3 code scenarios exist today, and only one of them has anything to do
with a GDD map:

| `Scenario::map_id()` | Scenario | Relationship to `gdd/` |
|---|---|---|
| 1 | Normal (the real building) | Implements GDD `map01_highrise.md` |
| 2 | StressTest (Arena) | Pure dev/test rig — not a GDD map |
| 3 | ItemGallery (item-size reference) | Pure dev/test rig — not a GDD map |

`MapKind::Campaign(map_id)` (also in `network.rs`) is a **third, separate**
number space — which GDD campaign map (1-20) a Campaign scenario's building
geometry implements. It coincides with `Scenario::map_id() == 1` only
because Normal happens to implement GDD map01. Don't confuse the three.
