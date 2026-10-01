# Fidelity pass 2 — design

Batch 2 of the user's fidelity request (29 Sep 2026). Batch 1 (brutal, pipes, mesh, orbit + the VS
high-BPM retune) shipped in v0.3.2. The first takes below were approved with batch 1 ("go for it");
the user is away and asked for this to be done independently. Ships as **v0.4.1**.

User's words, per family:
- `rave` — "very bland, literally just lasers, needs something else".
- `dolphin` — "also needs some polish" (one of the families that "don't pop", not cohesive with
  blossom/vaporwave).
- `vswings` — "lots of space on the sides that goes unused".
- `vsghost` — "just looks like bars over a mostly invisible graphic, needs to be rethought. I like the
  vibe but the execution needs work." (Vibe: Virtual Self "Ghost Voices" — glitch terminal, kaomoji,
  datamosh.)

Each family keeps its ids and colourways; vsghost's meter changes (approved: a rethink), the other
three keep their meter semantics. Existing tests keep their names and shapes unless the rethink makes
one meaningless — then it is replaced by an equivalent and the report says so.

## Shared rules

As every recent family: opaque panel first / clip last; allocation-free `draw` (pools preallocated,
baked layers rebuilt only on (w,h) change); **no double smoothing** (never `t.ballistics`
attack/decay in `draw`); no `bloom` on opaque panels; < 1 ms steady / < 1.5 ms flourish at 380x60
release; 3:1 contrast; liveness at 128x44; tests count paint over the panel; every new test shown
non-vacuous by breaking its feature; dumps READ before committing; review-sheet section per family
(27 rave, 28 dolphin, 29 vswings, 30 vsghost) with BEFORE and AFTER figures.

## `rave` — a room for the lasers

The lasers stay the meter (beam fan outline traces the spectrum). Add the room they cut through:
- **Haze.** A low-contrast drifting fog layer (two slow-scrolling noise bands baked once into a
  scratch canvas twice the panel width, scrolled by an offset each frame) at `ghost` alpha.
- **Beams become cones in the haze.** Each beam is drawn as its existing line PLUS a 3-5 px soft cone
  (two flanking lines at alpha 0.25 and 0.1) so it reads as light through smoke.
- **Truss.** A 2 px dark lattice bar across the top (a zig-zag of `line`s) with the three laser heads
  as small boxes hanging off it.
- **Crowd.** A silhouette strip along the bottom (heads and shoulders, hashed per (w,h)) in near-
  black; arms (2-3 px vertical strokes) rise on strong onsets in a wave travelling left to right, and
  the crowd bobs 1 px on each beat.
- **Floor splash.** Where a beam meets the floor/crowd line, a small elliptical splash of its colour
  at alpha 0.3.
- **Strobe** kept; the flourish additionally sweeps a moving-head white beam across the room once.

## `dolphin` — the hero leap

Keep the LED-matrix look and the sea meter. Upgrade:
- **The dolphin at 2x** (the current sprite scaled with 2x2 dots, or a new 2x sprite) on a proper
  parabolic **leap arc** whose height follows the bass; at the apex, a 1-frame highlight.
- **Splash.** On re-entry, 6-10 LED droplets (pool) fly up and fall, plus a ring of lit dots on the
  sea surface spreading out.
- **Sky.** A sun/moon disc (LED dots in `hot`) low on the horizon with a reflected column on the
  sea; a few stars twinkling in the upper rows.
- **Wave crests.** The sea surface gets travelling crest dots one row above the level line, scrolling
  with time; per-dot glow = a dim neighbour ring (no bloom).

## `vswings` — fill the sides

The wings stay centred and remain the meter. Use the empty thirds:
- **Wider span.** The fan reaches 80 % of the panel width (was ~45 %), feathers scaled with it.
- **Echo wings.** A second, fainter pair (alpha 0.35, 70 % length, lagging 100 ms behind) behind the
  main pair — the Virtual Self "angel" doubling.
- **Floor to the edges.** The perspective grid already exists — extend it to both edges and add
  slow-scrolling horizontal lines so the sides move.
- **Particle sparks.** On strong onsets, 12-20 sparks (pool) fly outward from the wing tips toward the
  side edges and fade over 500 ms.

## `vsghost` — the rethink: a glitched face

Keep the Ghost Voices vibe (glitch terminal, kaomoji, datamosh) but make the main object large and
legible:
- **The face.** A big kaomoji drawn in 2x `font3x5` (e.g. `( ^_^ )`, `( >_< )`, `( o_o )`,
  `( -_- )`, `( ^o^ )`), centred, filling ~40 % of the height. The face cycles on every 4th strong
  onset (like the current phrase). Its **eyes** brighten and widen (swap `-` → `o` → `O` glyphs)
  with bass level; its **mouth** opens with mids (`_` → `o` → `O`).
- **Scanline datamosh as texture.** Every frame, 2-4 horizontal slices of the face are shifted by
  `level * 6` px (bass-driven), so the face glitches with the music continuously — not only on the
  flourish.
- **The meter.** A row of 32 thin ticker bars along the bottom 25 % of the panel (the old piano
  roll, compressed: 2 px columns, lit bottom-up), plus the phrase (`GHOST VOICES`, `EON BREAK`, …)
  as a scrolling ticker above it in `font3x5`, letters brightening with treble.
- **Flourish** — keep the datamosh slam (slices shifted and colour-inverted, one-frame strobe) but
  apply it to the face.
- The 120 ms refractory (from the VS retune) may make face swaps/glitches busier at 160 BPM; the
  face swap uses its own every-4th-strong-onset counter, so it stays calm.

## Review focus

1. Each family's 128x44 still reads (rave crowd/truss scale; dolphin 2x sprite fits; vswings span
   clamps; vsghost face drops to 1x font).
2. No allocation in the new pools; no bloom; no double smoothing.
3. vsghost's existing tests: the meter tests move to the ticker bars; the phrase-clipping test keeps
   its intent on the ticker.
4. Performance: rave's haze scroll and cones and vswings' echo pair are the cost risks.

## Out of scope

New families; schema changes; the high-BPM fixture recapture (needs live audio).
