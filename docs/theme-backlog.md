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

---

## 4. Blossom needs distinct colours (DONE 2026-09-01)

Reported after the family shipped: "all the colourways just degrade to pink or white blossoms. I like the
vibe but we need more distinct colours."

Correct, and the cause is a constraint I imposed without noticing. The five shipped `lit` values are
`#ffb7d2`, `#dfe4ff`, `#ff9ec0`, `#f6f2ff`, `#ffc2d6` - three pale pinks and two near-whites. I picked
them by asking "what colour is cherry blossom", which is a very narrow band, so the colourways differ in
their SKY and BRANCH while the petals - the element that dominates the frame and the one the eye tracks -
stay nearly identical. Every other family gets wide hue variety; this one was quietly denied it.

Worth noting the reel family had the same fault for the same reason (five authentic hardware neutrals,
reported as boring) and the fix there was to stop being literal. The precedent is already in the tree:
flame ships Plasma and Rainbow alongside its real flame-test colours.

### Options, roughly in order of how much they would help

1. **Non-literal petal hues.** Deep magenta, amber/gold, ice cyan, jade, violet. The family's identity is
   the BRANCH plus the falling-and-tumbling motion, not the petal colour - so a violet blossom is still
   unmistakably this family, the same way Neon Miami is still unmistakably a tape deck.
2. **Per-petal hue variation within one colourway**, via the existing rainbow machinery (`rainbow` +
   `rainbow_spread` through `render::tint`, exactly as `orbit-rainbow` does). This attacks the complaint
   most directly, because the sameness is WITHIN a frame as well as between colourways. Note `RAINBOW_SAT`
   is a measured ceiling of 0.68 - at full saturation pure blue only reaches 2.31:1 against a near-black
   panel and fails the 3:1 rule at every brightness.
3. **Hue by petal age or depth**, so a petal shifts as it falls - freshly released warm, settling cool.
   Cheap, and it makes the field read as depth as well as motion.
4. **A much bolder sky.** Least effective on its own: the sky is already the thing that differs most, and
   it is not what the eye is following.

### Constraint that shaped the original and still applies

Petals must clear 3:1 against their own panel, and every colourway here is a dusk for that reason -
pale-pink-on-white is the one cherry blossom picture this panel cannot draw. That rules out a WHITE sky,
not a coloured petal, so option 1 is unaffected by it.

Also worth knowing before tuning: the shipped contrast test compares `lit` against the FLAT panel colour,
not against the sky gradient drawn over it. The current five have 8.6:1 in the worst case, so they are
fine - but the test is not what establishes that, and a bolder sky plus a darker petal could pass the
suite while being hard to read.

---

## 5. Lightning striking the castle on a bass hit (blossom)

Asked for 2026-09-01, straight after the castle. Queued behind it because the bolt needs a TARGET - it
terminates on the castle, so it cannot be built until the castle's anchor point exists.

### The trap, and it is measured rather than theoretical

The obvious trigger - a threshold on the single-frame rise in the bass mean - PROVABLY CANNOT FIRE on
real music in this project. The vaporwave family shipped exactly that and its lightning fired ZERO times:
the largest single-frame bass-mean rise anywhere in the 8-second fixture is 0.140 against a threshold of
0.157. It went unnoticed because the synthetic tests passed. That measurement is recorded in
`render/fluid.rs`'s droplet-rate test, which asserts it as a second claim precisely so the number does
not get lost.

So the trigger must be a BASS-WEIGHTED SPECTRAL FLUX detector judged against a RUNNING MEDIAN, the
mechanism `dsp::flourish` documents: "judge a hit against the median of recent hits, not against a
constant". Relative, so it means the same thing on a compressed pop master and on drum-and-bass.

Concretely: a second `dsp::onset::Flux` over the low quarter of the bands, ratio around 3.2 and a
refractory around 1500ms, which gives a strike every few seconds on bassy material rather than on every
kick. NOT the family's existing `onset` (ratio 2.8, 200ms) - that fires ~3 times a second and shakes the
branch, which is the right rate for a branch and far too often for lightning.

### What it should draw

- A bolt from the top of the panel to a point on the castle - jagged, 1-2px, with a brighter core.
- A SKY FLASH: brighten the gradient for a few frames. The vaporwave family has `sky_flash` and
  `bolt_bright` as tunable fields and its bolt drawing is worth reading before writing a new one.
- The castle rim-lit on the struck side for the duration, which is what sells the bolt as hitting rather
  than passing behind.
- Optionally the petals briefly catching the light.

### Two things to decide when building

- Whether lightning REPLACES the gust flourish or coexists with it. Two whole-display events with
  different triggers may read as noise; the gust is currently the family's flourish.
- Whether the strike should be visible at all in the colourways whose sky is already bright (Lantern).
  The vaporwave family sets `bolt_bright = 0` on two colourways deliberately, and the storm code checks
  it - the precedent for opting a colourway out already exists.

### Composition, decided 2026-09-01: the moon PEEKS OUT FROM BEHIND the castle

Asked for directly, and it is the legibility choice as well as the aesthetic one - which is worth
recording because it settles the one question the design pass was told to argue both ways.

The castle is a flat silhouette drawn behind the branch and petals, so its OUTLINE is all it has; there is
no interior detail to fall back on at 60px. The moon (radius 10) is the brightest thing on the panel. A
dark tiered roofline crossing a bright disc is therefore the highest-contrast edge available anywhere in
the frame, which is exactly where the castle most needs to be readable. Against bare dusk sky it is
dark-on-dark and relies on a few tones of separation; against the moon it is unambiguous.

So: draw order becomes sky -> moon -> castle -> branch -> petals, and the castle must be POSITIONED so
its most distinctive feature - the stacked roofline, whichever design wins - crosses the disc rather than
sitting clear of it. Roughly: castle centred near 0.78-0.85 across, base at the panel bottom, tall enough
that its upper tiers reach the moon at 0.30 down.

Corollary worth remembering when the lightning lands (item 5): the bolt should strike the part of the
castle that is silhouetted against the MOON, because that is where a rim-lit edge will actually show.

---

## Status update, 2026-09-01

Item 4 is DONE. Options 1 and 2 both shipped: three colourways rethemed to hues cherry blossom does not
have (amber, violet, gold), two added (Jade, and Riot where every petal carries its own stable hue), and
petal glow on its own bloomed layer with per-colourway strength. Option 3 (hue by petal age) was not
taken - Riot covers the same ground more directly and a petal that changes colour mid-fall reads as a
fault. Option 4 (a bolder sky) was not taken either: I sampled the rendered sky and it is already doing
what it was set to do.

Item 5 (lightning striking the castle on a bass hit) is now UNBLOCKED - the castle exists, so the bolt has
a target. The trap recorded in that item still stands and is the first thing to check: the obvious
bass-rise trigger provably cannot fire on real music here.

### Carried forward from the castle work

The eave lesson is worth keeping for any future silhouette in this family. Architecturally correct
upturned eaves - 2-column blocks protruding 5-7px past a narrow storey - read as a SPIDER once the shape
had a crisp keyline. The detail was right and the proportion was wrong, and it was invisible while the
castle was still a low-contrast smudge. Detail that survives being correct can still fail at 28 rows.

The keyline itself is now load-bearing here and worth reusing: the castle has to survive a dark sky, where
only a body LIGHTER than the sky shows, and a near-white moon, where only an edge DARKER than the moon
shows. Body-plus-rim gives it one contrast against each.

A big moon overlap and a legible moon are not both available at this size. The castle is 41px wide and the
disc 21px across, so at MOON_Y 0.55 the tiers cut the moon into fragments and it stopped reading as a moon
at all. 0.42 crosses only its bottom edge, which is what "peeking out from behind" actually wants.
