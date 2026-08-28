# Theme backlog

Ideas captured during live testing, not yet specced. Recorded here so they survive the
conversation they came from.

---

## 1. Dolphins car stereo

The 1990s/2000s aftermarket head unit — Sony Xplod, Pioneer, JVC — with a dolphin arcing
across a low-res display while a spectrum analyser runs underneath.

What makes it read as that object rather than a generic meter:

- **Coarse dot-matrix pixels.** The display is visibly made of discrete dots with dark
  wells between them. Deliberately chunkier than the VFD family's 5px bars.
- **A single backlight colour** — amber, ice blue or green — with everything drawn in that
  one hue at two or three brightness levels. No gradients.
- **The dolphin is animated and loops**, arcing up out of a waterline and back down,
  crossing the display over several seconds.
- **The meter sits low**, under the dolphin, often only 6–8 segments tall.
- Frequently a **waterline** — a horizontal dotted line the dolphin breaks through.

## 2. Vaporwave sunset

- **Banded sun**: a circle with horizontal slots cut out of it, larger gaps toward the
  bottom.
- **Gradient sky**: magenta → orange → deep purple.
- **Perspective grid** receding to the horizon in cyan or magenta, optionally scrolling
  toward the viewer so it reads as motion.
- Optional foreground silhouette — a low wedge car, or palm trees.
- Optional **lightning**, which has a natural hook: trigger a flash on a bass transient,
  so it reacts to the music rather than being decorative.
- Pixel-art treatment throughout, which suits 190×60 well.

---

## Why these are a new *family*, not new colourways

The existing seam is **colourways are data, families are code**. These two are neither a
recolour nor a variant of the segmented meter: they need a **pictorial backdrop plus
animation**, which the `Family` trait has no concept of. So they are a third thing — call
it a `scene` family — that composites:

1. a backdrop (procedural gradient, or sprite art),
2. an optional animated sprite with its own phase/position state,
3. the meter itself, drawn over or into the scene.

Animation state is fine — families already own per-frame state (the scope's persistence
buffers, the VU's needle ballistics), so a dolphin's position fits the existing shape.

## The interesting part: sprites could stay *data*

Worth exploring before defaulting to hardcoded Rust drawing. A pixel-art sprite is
expressible as palette-indexed rows directly in the theme TOML:

```toml
[sprite.dolphin]
palette = ["", "#0a2a3a", "#4fd8ff", "#c8f4ff"]   # index 0 = transparent
frames = 4
rows = [
  "....2222....",
  "..22333322..",
  ".2333333332.",
  "..22333322..",
]
```

If that works, these themes become **authorable without a rebuild**, like every other
colourway — and the theme-authoring prompt in the README could generate them. That would
be a genuine extension of the extensibility seam rather than an exception to it.

The counter-argument: the vaporwave gradient and the perspective grid are better generated
procedurally than stored as pixels, and the lightning needs to respond to audio, which is
behaviour rather than data. So the honest answer is probably **both** — procedural
backdrop parameters in TOML, plus an optional sprite table for the figurative elements.
Decide it when specced; do not guess now.

## Related: use the empty taskbar to the left

The user noted the display could extend further left. Measured on the reference machine:
app buttons end at **x≈1119** and the Widgets button starts at **x≈1425**, leaving roughly
**300px of dead taskbar** between them.

That matters most for these two themes: a dolphin arcing across 190px is cramped, and a
perspective grid wants width. A `scene` family could claim, say, 456×60 and have room to
breathe.

Caveats to check when specced:
- That gap only exists while the taskbar is **left-aligned** and not full of windows. It
  shrinks as apps open, so the width has to be computed at runtime from the actual gap,
  not assumed.
- The overlay would then span from the last app button to the widget, so it must not cover
  a taskbar button that appears mid-song.
- Rect tracking already re-discovers every second, so the machinery exists; the change is
  computing an extended rect rather than using the widget rect directly.

---

## 3. Windows screensavers — 3D Pipes, 3D Maze, Mystify

Asked for 2026-08-28: "replicating the old windows screensavers pipes and maze etc". Deferred, not
rejected — 3D was set aside for now in favour of the car stereo above. Recorded with the measurements
from the 3D feasibility investigation so this does not have to be worked out twice.

### The problem all of them share, and it is not rendering

**A screensaver is autonomous; a meter must be driven.** Pipes wanders, Maze walks, Mystify drifts —
none of them displays anything. The hard part is the audio hook, not the geometry, and this project
has a house rule that decides it: encode level as POSITION, not brightness. `tube.rs:54-60` measured a
driven element only 1.46 dL* brighter than its idle neighbour, against a ~2.3 dL* visible threshold,
which is why every family since uses position. "The pipes glow with the bass" is therefore not a
design — it is the thing that has already been measured as invisible here.

### What the 3D investigation established, and it applies to all three

- **Compute is not the constraint.** Measured on this machine: flame 2.02ms, segmented 1.75ms, tube
  1.09ms, waterfall 0.74ms, against a 16.7ms frame. Note segmented — the SIMPLEST family and the
  default theme — costs 87% of flame, so the floor is ~1.75ms rather than zero. Only 4 of 14 families
  have ever been timed. Every figure is at 190x60 while the app runs 380x60 by default, so double them.
- **The constraint is VERTICAL ROWS.** 48 usable rows (`segmented.rs:9-10`, PAD_Y=6 on a 60px panel).
  In 3D, depth steps and amplitude travel are drawn from the same 48-row account.
- **Vapor already measured the depth collapse.** At the tuner's `persp=2.07`, SEVEN of sixteen depth
  lines landed on rows 28-29 — and that silently disabled occlusion, because lines sharing an integer
  row cannot occlude each other. Twelve depth lines measured unreadable. Any corridor or receding
  grid must assert one distinct integer row per depth level; copy
  `vapor.rs:1313` `the_shipped_perspective_keeps_one_pixel_row_per_grid_line`.
- **A hang, not a slowdown, waits for naive perspective.** `canvas.rs:624-650` Bresenham breaks only
  on reaching the endpoint — there is no off-canvas early-out. A vertex near the eye projects to
  infinity, `as i32` saturates to 2147483647, and one such edge iterates ~2.1e9 times: **measured at
  294.6ms**, about 18 dropped frames. A near-plane clip must come BEFORE any perspective divide.
- **Already available:** `canvas.rs:679 fill_poly` (scanline, even-odd, concave-safe, 7 tests),
  `canvas.rs:624 line`, `bloom`, `clip_to_rounded_rect`. Polygon fill is not a gap.
- **Refused outright:** a starfield. 150 one-pixel stars change ~1.3% of the panel, and this family
  set has a measured lesson that small-area changes go unnoticed unless they change KIND.

### Per idea

**3D Pipes.** Extruded segments on a lattice with elbow joints — geometrically the friendliest, since
a pipe run is axis-aligned boxes and needs no general rotation. Two real problems. (1) It wanders off
the panel by design; on a 380x60 strip a pipe that leaves does not come back, so the growth has to be
confined, which is not what Pipes looks like. (2) What does the audio DO? Candidates worth testing:
segment growth rate, a new branch on a flux onset, per-band pipe diameter. Note the flame family
already occupies "pipes along the bottom" visually, so this must not converge on it.

**3D Maze.** The strongest sense of depth of anything considered, and the worst fit for the
constraints: it is a corridor with a vanishing point, so it lands squarely on both the depth-collapse
wall and the near-clip hazard. Also the walls are texture-mapped in the original, and a 48-row
corridor has perhaps 3-4 usable depth steps. Would need the near-clip and a tuning tool built first.

**Mystify.** The most feasible of the three and the least obviously "3D": bouncing polylines trailing
their own history. It needs no perspective, no clip, no depth buffer; the vertices can be driven by
band levels so the shape IS the spectrum; and the trailing history reuses the scope family's phosphor
persistence. Worth considering FIRST if the appetite is for a screensaver family rather than
specifically for depth.
