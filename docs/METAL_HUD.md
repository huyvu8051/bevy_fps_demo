# Metal HUD

Apple's Metal Performance HUD, overlaid on a running build. It is the only
thing that reports encoder and command-buffer counts, which is what separates
"the renderer is slow" from "the engine is busy".

Both commands use the same env block, so the two renderers can be read side by
side.

## Bevy client

```sh
MAP_ID=3 \
MTL_HUD_ENABLED=1 \
MTL_HUD_ENCODER_TIMING_ENABLED=1 \
MTL_HUD_SHOW_METRICS_RANGE=1 \
MTL_HUD_INSIGHTS_ENABLED=1 \
MTL_HUD_ELEMENTS=device,memory,fps,fpsgraph,gputime,frameinterval,frameintervalgraph,frameintervalhistogram,presentdelay,metalcpu,gputimeline,shaders,thermal,layersize \
MTL_HUD_ALIGNMENT=topright \
MTL_HUD_OPACITY=0.9 \
MTL_HUD_SCALE=0.3 \
cargo run -p client
```

`cargo run`, not `./target/release/client`: called directly, Bevy resolves
assets against the wrong root and every number it reports is of a different
scene than the one on screen.

Add `RENDER_SCALE=1.0` when measuring — the game ships at 0.3, and the HUD
prints the *window* size, not what Bevy renders into.

## desert_swift_metal

```sh
MTL_HUD_ENABLED=1 \
MTL_HUD_ENCODER_TIMING_ENABLED=1 \
MTL_HUD_SHOW_METRICS_RANGE=1 \
MTL_HUD_INSIGHTS_ENABLED=1 \
MTL_HUD_ELEMENTS=device,memory,fps,fpsgraph,gputime,frameinterval,frameintervalgraph,frameintervalhistogram,presentdelay,metalcpu,gputimeline,shaders,thermal,layersize \
MTL_HUD_ALIGNMENT=topright \
MTL_HUD_OPACITY=0.9 \
MTL_HUD_SCALE=0.3 \
open build/apps/DesertSwiftMetalMac.app
```

`open` goes through LaunchServices, which does not reliably hand the app any
`DSM_*` flags. To pass those, run the binary inside the bundle directly:
`build/apps/DesertSwiftMetalMac.app/Contents/MacOS/DesertSwiftMetalMac`.

## Reading it

- **FPS is useless** while vsync holds it at the cap. Read `GPU` against the
  refresh interval instead: 2.6ms of 8.33ms is 31% used, 8.2ms is none left.
- **`Command Buffer CPU  N | T`** — `T` is how long those buffers were *open*,
  not CPU spent creating them. Measured directly, a command buffer costs 4.5µs
  and a render pass 25.9µs, so a frame's 26 buffers and 34 passes are worth
  about 1ms together. An `8.56ms` on that row is not 8.56ms of work.
- **Drawable size, top left.** Two HUDs are only comparable when it matches.
  A tiling WM (AeroSpace) resizes the window, so neither app's own window
  settings decide it.
- Insight lines in yellow are Apple's own diagnosis and worth reading, but
  "90% of frame time encoding" is an elapsed-time ratio, not a CPU budget.
