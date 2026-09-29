# Fidelity pass 1 — design

User feedback 29 Sep 2026: `brutal`, `pipes`, `mesh`, `orbit` are "really lacking in visual fidelity
and flair, very basic, not cohesive with blossom or vaporwave — I want them to pop more", and all
three Virtual Self families are "not responsive enough to high-BPM music such as VS". Batch 1 of
two (batch 2: dolphin, rave, vswings sides, vsghost rethink). Ships as **v0.3.2**.

Design approved in chat (first takes accepted, "go for it"). The controller has latitude on exact
numbers; the eye test decides. Each family keeps its ids, colourways and meter semantics — this is
a rendering upgrade, not a redesign — so existing tests keep their shapes and goldens are
regenerated ONCE per family in its own commit after a visual check.

## Shared rules

Same envelope as every recent family: opaque panel, clip last, allocation-free `draw`, < 1 ms
steady at 380x60 release (< 2 ms gate), 3:1 contrast, liveness at 128x44, `Trigger`+`Envelope`
flourishes kept, no new dependencies, no `fill_poly`/`bloom` additions beyond what the family
already uses. Every changed family re-dumps calm/loud/flourish and the implementer READS them.

## `brutal` — concrete with material

- Slabs become **blocks**: a lit top face (2 px, `lit` lightened 20 % in linear light), the front
  face in `lit`, a **shadow side** 3 px wide on the right in `lit` darkened 35 %.
- **Aggregate speckle**: a deterministic hash pattern (~5 % of front-face pixels) one shade darker;
  **formwork lines**: a 1 px darker horizontal line every 9 px on the front face.
- **Slam**: on a bass onset each slab drops from 3 px above its level to its level over 60 ms
  (ballistic `y` offset), and a 6 px **dust puff** (three 1 px `ghost`-alpha particles) rises from
  its base and fades in 300 ms.
- **Rebar**: the peak marker is a 1 px vertical `hot` tick with a 2 px bent top — a rebar end.
- New colourway `brutal-sodium`: panel `#0a0804`, slabs `#8a8378`, lit-from-below sodium glow: a
  vertical gradient `#ff9a1a` at alpha 0.35 → 0 from the floor up to 40 % of the height, hot `#ffd27a`.

## `pipes` — fat pipes filling the volume

- Pipe radius 3 px (7 px wide) at the front depth, scaled by the existing depth factor, with a
  1 px **highlight stripe** offset toward the light and a 1 px **shaded underside**; joints are
  filled circles of radius+1 at every turn.
- **Three pipes** grow concurrently (was one), each in its own hue from the colourway (`lit`,
  `hot`, and `lerp_linear(lit, hot, 0.5)`), staggered so the lattice fills; a pipe that has grown
  more than 60 % of the cell budget dissolves segment-by-segment from its tail.
- Growth rate follows level (as now); the newest segment glows `hot` for one frame.
- Flourish: the **teapot** — at the growing tip, draw a 9x7 px teapot silhouette (fixed bitmap,
  our own) in `hot` for 600 ms, then continue.

## `mesh` — a stage for the bars

- **Floor**: a perspective grid (8 verticals, 4 horizontals converging to a horizon at 30 %
  height) in `edge` at `edge_alpha`, the bars standing on it with **shadows**: an ellipse of
  `panel` darkened 40 % under each bar, width = bar width, height 2 px.
- **Specular**: top faces get a 1 px `hot` highlight along their front edge; side faces are `lit`
  darkened 30 %.
- **Fog**: rows behind the front fade toward `panel` by 25 % per depth step (linear light).
- **Camera yaw**: the isometric angle swings ±6° with a 20 s period (precomputed per frame, no
  allocation).
- **Falling ghosts**: when a bar's peak drops by more than 15 % in a frame, a `ghost`-alpha block
  of the lost height detaches and falls at 40 px/s, fading over 400 ms (preallocated pool of 16).

## `orbit` — a real system

- **Sun** at centre: radius `4 + 6 * bass` px, `hot` core with a 3-step `lit` corona (three
  concentric filled circles at descending alpha), pulsing.
- **Orbits**: each band group is a planet on an **ellipse** (ratio 0.35) whose path is drawn as a
  1 px `edge` ring at `edge_alpha * 0.5`; the planet is a filled circle radius 2-4 px with a
  **lit side** (a 1 px `lit` crescent toward the sun) and a **comet tail** of 6 fading dots behind
  it whose length = level. Angular speed = 0.2 + 1.5 * level rad/s.
- Depth: planets behind the sun (upper half of the ellipse) draw at 60 % alpha, in front at 100 %.
- Flourish: **alignment** — all planets ease to the same angle over 400 ms, a white flash line
  through them, then they scatter back.

## Virtual Self retune (`vswings`, `vsghost`, `vsorb`)

- `ballistics`: attack 0.9, decay 0.35, peak_fall 0.03 on all three bases (were 0.55/0.12/0.01,
  0.8/0.25/0.02, 0.5/0.1/0.01).
- Onset refractory constants in the three files: 120 ms (were 200 ms); onset ratio unchanged.
- **Verification is real audio**: an `#[ignore]` test `slow_vs_high_bpm_response` drives the
  three families with `tests/fixtures/high-bpm-bands.csv`, a NEW fixture captured from a
  150-170 BPM Virtual Self track via `--levels` (the user plays it; the implementer records it
  with Spotify through the app's media control, as `live_identify` does), and asserts that the
  per-frame band envelope's autocorrelation peak lies within ±10 % of the track's beat period —
  i.e. the meter actually beats at the tempo. If no live capture can be made in the session, the
  fixture is synthesised at 160 BPM (kick every 375 ms with a 40 ms decay) and the test says so.

## Review focus

1. Each family still passes its existing tests and goldens are regenerated once, in the family's
   commit, after the eye test.
2. Nothing new allocates per frame (dust/ghost pools preallocated).
3. 128x44: floors, orbits, fat pipes and slab faces still fit; no paint outside the panel.
4. The VS retune must not make the meters *jitter* on slow music — `slow_vs_timing` and the
   existing per-family real-music tests still pass.
5. Contrast: the new `brutal-sodium` and every recoloured face clear 3:1.

## Out of scope

Batch 2 families; new families; schema changes.
