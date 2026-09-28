# Theme backlog

Ideas captured during live testing, not yet specced. Recorded here so they survive the
conversation they came from.

---

## Shipped

- **Dolphin car stereo** (item 1) — shipped as the `dolphin` family: coarse dot-matrix display,
  single-backlight-hue colourways, an animated dolphin sprite arcing over a waterline.
- **Vaporwave sunset** (item 2) — shipped as the `vapor` family: banded slotted sun, gradient sky,
  a receding perspective grid displaced by the spectrum, bass-triggered lightning.
- **Blossom needs distinct colours** (item 4, DONE 2026-09-01) — three colourways rethemed to
  hues cherry blossom does not literally have (amber, violet, gold), two added (Jade, Riot with
  per-petal hue variation), petal glow moved to its own bloomed layer.
- **Lightning striking the castle on a bass hit** (item 5, DONE 2026-09-01) — shipped: a
  bass-weighted spectral-flux detector against a running median (the obvious single-frame-rise
  trigger provably cannot fire on real music here — see `render/fluid.rs`'s droplet-rate test),
  measured at 2.40/4.40/2.20 strikes per minute over the three real-music fixtures. Found and
  removed a pre-existing dead `bloom` call on the moon that changed zero visible pixels.
- **A frenchcore family** (item 6, DONE 2026-09-01) — shipped as the `rave` family: a sweeping
  laser fan whose outline traces the spectrum, strobing on the kick with no rate limit (waived by
  the user — see the item's own notes if that ruling needs revisiting).
- **A kaleidoscope family** (item 7, DONE 2026-09-01) — shipped as the `kaleido` family: frieze-
  group (not rosette) symmetry, because a centred rosette uses only 16% of a 6:1 letterbox.
- **A brutalist-bars family** (item 8, DONE 2026-09-01) — shipped as the `brutal` family:
  concrete blocks slamming between floor/ceiling orientations on the beat, a strobe made of
  position rather than brightness.

With `rave`, `kaleido` and `brutal` landing together: 148 colourways across 22 families at the
time, now 150.

---

## Open

### Windows screensavers — 3D Pipes, 3D Maze, Mystify

Asked for 2026-08-28: "replicating the old windows screensavers pipes and maze etc". Deferred, not
rejected — 3D was set aside in favour of the car stereo/vaporwave work above. Recorded with the
measurements from the 3D feasibility investigation so this does not have to be worked out twice.
(3D Pipes and Orbit have since shipped as separate, already-specced families with real perspective
projection — see `docs/themes.md`. This entry is about 3D **Maze** and **Mystify**, which have not.)

**The problem all of them share, and it is not rendering.** A screensaver is autonomous; a meter
must be driven. None of Pipes, Maze or Mystify displays anything on its own — the hard part is the
audio hook, and this project's house rule (level is POSITION, not brightness) already ruled out
"glow with the bass": `tube.rs:54-60` measured a driven element only 1.46 dL* brighter than its
idle neighbour, against a ~2.3 dL* visible threshold.

**What the investigation established, now proven twice over by 3D Pipes and Orbit shipping:**
compute is not the constraint (4 of 22 families have been timed individually; the cheapest is
~0.7ms/frame, the priciest ~2ms, against a 16.7ms frame budget); the real constraint is vertical
rows (48 usable at 60px tall), so depth steps and amplitude travel compete for the same account;
a near-plane clip must land before any perspective divide, or a vertex near the eye projects to
infinity and one Bresenham edge iterates ~2.1 billion times (measured at 294.6ms, eighteen dropped
frames — `canvas.rs:624`).

**Per idea, still unbuilt:**

- **3D Maze** — the strongest sense of depth of anything considered, and the worst fit: a
  corridor with a vanishing point lands squarely on the depth-collapse wall (Vapor's tuned
  `persp` already collapsed 7 of 16 depth lines onto 2 rows) and the near-clip hazard at once.
  Texture-mapped walls in the original; a 48-row corridor has perhaps 3-4 usable depth steps.
- **Mystify** — the most feasible of the two and the least obviously "3D": bouncing polylines
  trailing their own history, driven by band levels so the shape IS the spectrum, reusing the
  scope family's phosphor persistence. No perspective, no clip, no depth buffer needed. Worth
  considering first if the appetite is for a screensaver family rather than specifically for depth.

Refused outright and worth remembering why: a starfield (150 one-pixel stars change ~1.3% of the
panel — too small an area to be noticed, per this project's own measured lesson) and, from the
`kaleido`/`rave` work, a zooming tunnel or checkerboard floor (wants a vanishing point, fails on a
6:1 letterbox the same way a rosette kaleidoscope did).
