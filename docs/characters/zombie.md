# Zombie

Slug: `zombie` · Asset metadata: [`crates/client/assets/characters/zombie/character.ron`](../../crates/client/assets/characters/zombie/character.ron)

## Story / lore

_TODO — fill in backstory, faction, how it fits the game's setting._

## Visual notes

_TODO — silhouette, color palette, distinguishing features for concept art / meshy.ai prompts._

## Gameplay role

Standard enemy. Chases the player, attacks in melee. Health/damage/speed
constants live in `crates/shared/src/lib.rs` (`ZOMBIE_MAX_HEALTH`, etc.),
not here — this file is for narrative/visual context only, not tuning
numbers that belong in code.

## Archetypes & special infected

This file covers only the base zombie (the one AI, 2 mesh kinds). For the
full roster — per-map environmental archetypes (office worker, factory
worker, BRDF soldier...) and the named special infected (Boomer/Hunter/
Tank/...) — see `docs/ZOMBIE_BESTIARY.md`.
