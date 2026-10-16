# Shader plan

The project had no custom shaders until the damage overlay. This note records
what is worth writing next and, for each one, how much it couples to Bevy's
shader library — because that coupling is what decides the cost of the 0.20
upgrade.

## The upgrade constraint

Bevy 0.20 drops `naga_oil` and moves to [WESL](https://wesl-lang.dev).

| | bevy 0.19.1 | bevy 0.20.0 |
|---|---|---|
| crate `wesl` | 0.3.1 (opt-in via `shader_format_wesl`) | 0.6.0 (always on) |
| preprocessor | naga_oil | WESL |
| GLSL | supported | removed |

The migration guide states plainly: **"Plain WGSL files with no preprocessor
directives keep working."** Everything else changes — imports take a semicolon
and new module paths, `#ifdef` becomes `@if`, `#{BINDING}` becomes
`constants::BINDING`, `#define_import_path` is gone.

WESL itself is pre-1.0 and moved three minor versions between the two Bevy
releases, so WESL-specific syntax written today is a guess at what 0.20 accepts.
Prefer directive-free WGSL.

## Staying on 0.19 for now

Checked 2026-10-16. The upgrade is blocked upstream, not by our code:

| crate | upstream | 0.20 status |
|---|---|---|
| `bevy_landmass`, `landmass_rerecast` | `andriyDev/landmass` | `bevy-0.20` branch exists |
| `bevy_rerecast` | `janhohenheim/rerecast` | open PR bumping to 0.20.0-rc.2 |
| `avian3d` | `avianphysics/avian` | nothing — highest branch is `bevy-0.18`, `main` is 0.19 |

Avian is the blocker and it is 56k lines across 153 files, six times the size
of this client, touching ECS, schedules, reflection and debug rendering.
Forking it to bump Bevy ourselves would cost more than rewriting the game, and
upstream would land their own bump in the meantime.

Our own code is cheap to port by comparison: one shader (`scope_lens.wgsl`),
`ExtractComponent` needing an `AppLabel`, and a deprecated `Interaction` that
still compiles. Checked against all 75 of the 0.20 migration guides — `Val`,
`BorderRadius`, `bevy_shape` and tonemapping all turned out not to apply here.

Watch avian. When it ships 0.20 the rest follows.

## Coupling tiers

**Tier 0 — no imports.** Fragment-only effects over a UI node. Declare the
varyings struct locally instead of importing it. Survives the upgrade untouched.

**Tier 1 — directives, so the file must stay `.wgsl`.** A 3D `Material` that
overrides only the fragment shader needs two things it cannot get any other
way: `VertexOutput`, whose fields are `#ifdef`-gated and so cannot be
redeclared locally, and the material bind group index, which Bevy substitutes
as `#{MATERIAL_BIND_GROUP}` — hardcoding `2` fails validation because that
group holds a storage buffer. Both are naga_oil syntax, so the file must be
named `.wgsl`; naming it `.wesl` routes it to the WESL parser, which rejects
both and panics the render thread. Migration to 0.20 is the import line, the
`#{...}` substitutions becoming `constants::MATERIAL_BIND_GROUP`, and the
rename back to `.wesl`.

**Tier 2 — engine bindings.** Anything reading lights, view uniforms or the
skinning pipeline. Several imports, paths all moved, and the surrounding
`#ifdef` logic has to become `@if`. Worth deferring to 0.20.

## Done

- **Damage overlay** (tier 0) — `assets/shaders/damage_overlay.wesl`. Red
  vignette that tightens and pulses as health drops, plus a hit flash.

- **Scope lens** (tier 1) — `assets/shaders/scope_lens.wgsl`. Barrel
  distortion, chromatic fringing and a vignette on the render-target image.
  Pipeline validates; the look itself has not been checked by eye.
- **Film grain and low-health desaturation** (tier 0) —
  `assets/shaders/screen_effects.wesl`, one post-process pass via
  `FullscreenMaterial`. Grain reseeds per frame and biases toward shadows;
  colour drains to grey as health drops. Measured: high-frequency energy in a
  flat region went from 0.006-0.010 to 0.022.

## Next

### Light adaptation — not tier 0 after all
Listed here as easy, which was wrong. Eye adaptation needs the average
luminance of the frame, so it needs a downsample pass feeding back into
exposure — multi-pass, not a single fragment shader. Bevy 0.19 ships no
auto-exposure to lean on either. Worth doing, but it is not a cheap one.

### Wet blood, the rest of it — tier 2
Decals now take real lighting, so there is already a specular that moves with
the view — that part is done without a shader. What is left is control: a
glossy pool interior against a rough dried rim, which needs a roughness map or
a shader reading light bindings. Low priority now that the cheap half landed.

### Burn-away dissolve — tier 2
Death currently sinks and fades the corpse, which needs no shader. A burn edge
creeping across the body needs a material on a skinned mesh — the deepest
coupling of anything here.

### Projected decals — tier 2
Blood is a flat quad, so it clips through curved walls and stair edges. Proper
decal projection needs depth reconstruction.

## Rules of thumb

- Write plain WGSL. Reach for a directive only when there is no alternative.
- Prefer a UI material over a 3D material when the effect is screen-space.
- Test on both maps. The garage is dark enough to hide effects that look fine
  on the desert map, and vice versa — a lit-decal change was wrongly rejected
  once because it was only checked in the garage.
