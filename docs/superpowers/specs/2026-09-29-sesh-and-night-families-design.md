# `sesh` and `night` families — design

Two new render families for taskbar-eq, requested 28 Sep 2026: one in the Bones / TeamSESH
lo-fi VHS aesthetic (`sesh`), one in the Cyberpunk 2077 HUD aesthetic (`night`). Five colourways
each. Ships as **v0.3.0** together with the vsorb speed-up already on `main`.

This spec is the authority; the implementation plan argues from it.

## Goals

- Two families that read as **meters** at taskbar scale (380x60 down to 128x44) and unmistakably as
  their aesthetic, judged by the user from the review sheet: keep, tune or drop each colourway.
- Same engineering envelope as the Virtual Self families: opaque panel, `Trigger` + `Envelope`
  flourish, allocation-free `draw` after the first frame, < 2 ms/frame (aim < 1 ms; the CI runner is
  ~1.5x slower than the dev machine), 3:1 contrast, liveness at 128x44 / 128x60.
- No trademarked marks. No TeamSESH skull-and-crossbones, no CDPR samurai or "2077" wordmark, no
  copied logo geometry. Words are drawn in our own pixel fonts. Colours and layout idioms are fair
  game; a reader should recognise the world without any asset being lifted from it.

## Non-goals

- No new theme schema tables; colourways use the existing `Theme` fields.
- No audio-reactive text beyond the words listed here; no Japanese glyphs (the 3x5 font cannot).
- No light-taskbar logic.
- `sesh` gets ONE bespoke glyph set (pixel blackletter). Nothing else bespoke.

## Shared requirements

- Each family is one file under `src/render/`, a `Family` impl registered in `render/mod.rs`
  (`KNOWN_FAMILIES` 25 -> 27), labelled in `themes/mod.rs::family_label`, with colourways as
  constructors in `themes/builtin.rs` registered in `all()`.
- Panel is opaque (`panel_alpha 1.0`), filled first, clipped last with `clip_to_rounded_rect`.
- Colour mixing in linear light (`Rgba::lerp_linear` / canvas blend). Inversions and RGB channel
  splits are effects, not mixes, and may operate on sRGB bytes.
- Tests measure "painted" as pixels differing from the panel (or from a background-only render
  where the background is not flat) by `|dR|+|dG|+|dB| > 24`, never by alpha.
- Every colourway: contrast rule, liveness guard, `fits_the_narrow_panel` (190x48),
  `rest_frame_is_not_empty` (levels 0: resting structure visible), `flourish_at_narrow_size_does_not_panic`.
- Each family: an `#[ignore]` `dump_<family>` writing calm / loud / flourish PNGs at 380x60 over
  `#202020` plus one 190x48, an `#[ignore]` `probe_<family>_cost`, and a section on
  `docs/review/index.html` with a "what I want your eyes on" line. `slow_vs_timing` gains one id per
  family. README counts become **173 colourways / 27 families** (a test enforces README and the
  family labels).
- Words are chosen from fixed arrays; every character must have a glyph (asserted in a test).
  Phrases wider than the panel are truncated to whole glyphs, never wrapped.
- RNG is splitmix64 seeded from a constant.

## Family 1: `sesh` — Bones / TeamSESH VHS

Label: **"Bones: VHS tape"**. Ids: `sesh-tape`, `sesh-word`, `sesh-vhs`, `sesh-red`, `sesh-bleached`.

### What the meter is

A VHS tape whose picture tears with the music. Two layers, always both present; colourways set the
mix.

**Tape layer.** The interior carries faint horizontal scanlines (every other row at `t.ghost` alpha)
with a slow vertical roll (`roll_px += 6 px/s`, wraps). The 64 bands fold into **12 tracking bands**
stacked bottom (bass) to top (treble), each `ih/12` rows tall. A band's level sets its **tear**: the
band's rows are drawn shifted horizontally by `shift = level * 0.18 * w` px (sign alternates per band,
wrapping at the interior edges) with a bright noise streak `t.lit` at alpha `level` along its top
row and a dark dropout streak along its bottom. Peak-hold per band: a 1 px `t.hot` line at the peak
shift decaying at `peak_fall`. Silent tape = clean scanlines gently rolling.

**Camcorder stamp.** Top-left, 3x5 font, `t.hot`: `PLAY` with a solid triangle glyph, then
`HH:MM:SS` of real elapsed time since the family was created. Top-right: `REC` with a 3x3 dot that
blinks on strong onsets (every onset above the trigger's median). When `h < 48` only the time shows.

**Word layer.** One word from `WORDS = ["SESH", "BONES", "TEAMSESH", "SESHOLLOWATERBOYZ", "TEAM SESH"]`
drawn centred in the **pixel-blackletter** glyph set (below), in `t.lit` with a 1 px `t.edge`
outline. Word height 9 px at `h < 58`, 13 px at `h >= 58`; if the word at that height is wider than
`iw - 8` it is truncated to whole glyphs. The word jitters +-1 px horizontally on every onset and
swaps to the next word every 6th strong onset. **Drips:** up to 8 drips (preallocated) spawn from
random letter bottoms on bass onsets (`mean(levels[0..8]) > 0.55`), fall at `18 px/s` accelerating,
1 px wide, in `t.lit`, and die at the interior bottom.

**Pixel blackletter.** A bespoke `glyph_gothic(ch) -> &'static [u8; N]` bitmap set covering exactly the
letters used: `S E H B O N T A M L W Y Z` plus space. Two sizes: 9 rows (5-6 columns per glyph) and
13 rows (7-8 columns), hand-drawn with the blackletter tells: heavy vertical strokes two pixels wide,
diamond/lozenge terminals, a broken (angled) bowl on O and S, hairline horizontals. Rendering is
`draw_gothic(c, x, y, text, size, col, outline)`. A test asserts every WORDS char has a glyph at both
sizes and that no glyph row exceeds its declared width.

**Colourway mix.** `Theme` has no spare field for a family knob and this spec adds none, so the mix
is a per-colourway constant inside the family: `fn mix(t: &Theme) -> f32` matching on `t.id`, default
0.5. `mix` in 0..1 where 0 = all tape, 1 = all word, and it scales exactly three things:
tear shift `= level * 0.18 * w * (1.3 - mix)`, streak alpha `= level * (1.2 - mix)` clamped to 1,
word size 13 px when `h >= 58 - round(12 * mix)` (so `sesh-word` goes large from `h >= 48`,
`sesh-tape` only from `h >= 55`), else 9 px. `ghost` is the scanline alpha (the schema's `ghost` is an
f32), `edge_alpha` the word-outline alpha.

| id | panel | lit | hot | edge | ghost | mix | note |
|---|---|---|---|---|---|---|---|
| `sesh-tape` | `#0a0a0a` | `#e8e6e0` | `#ffffff` | `#3a3a3a` | 0.22 | 0.25 | heavy tracking, small word |
| `sesh-word` | `#0a0a0a` | `#f2f0ea` | `#ffffff` | `#404040` | 0.12 | 0.85 | big word, gentle tape |
| `sesh-vhs` | `#0d0d0f` | `#dcdcdc` | `#ffffff` | `#444444` | 0.18 | 0.5 | chroma bleed: word drawn thrice offset -1/0/+1 px in `#ff2a2a` / lit / `#2ad2ff` at alpha 0.5 (constants in the family) |
| `sesh-red` | `#0a0a0a` | `#e8e6e0` | `#c8102e` | `#3a3a3a` | 0.18 | 0.6 | the ONE red: word outline and REC dot are `hot` |
| `sesh-bleached` | `#e8e6e0` | `#111111` | `#000000` | `#b8b4ac` | 0.25 | 0.5 | inverted tape; noise streaks dark on light |

All lit/panel pairs must clear 3:1 (bleached: `#111` on `#e8e6e0` is ~15:1).

### Flourish — dropout

`Envelope` 550 ms. Frame 1 of the envelope: the whole interior painted panel-black (bleached:
panel-white) — a true dropout. Then, while the envelope is above 0.15: a static field (per-pixel noise
from the RNG, `t.lit`/`t.edge` at alpha `env`), the word torn into three horizontal slices shifted
by up to `0.3 * iw` px with wrap, and the stamp reading `TRACKING` instead of the time. Below 0.15
the picture snaps back cleanly. The static field is written into a preallocated scratch canvas.

### Tests specific to `sesh`

- `louder_bands_tear_further`: with bands 0..16 at 0.9 vs 0.2, the bottom four tracking bands'
  measured horizontal displacement (position of the lit streak) differs by at least `0.08 * w`.
- `the_word_is_centred_and_whole`: the painted bbox of the word layer (rendered with tape suppressed
  via a test hook) is horizontally centred within 2 px and never exceeds `iw - 8`.
- `every_word_char_has_a_gothic_glyph` at both sizes.
- `dropout_first_frame_is_blank`: after `force_next()`, the first frame's interior is entirely panel.

## Family 2: `night` — Cyberpunk 2077 HUD

Label: **"Night City: HUD"**. Ids: `night-yellow`, `night-arasaka`, `night-netrunner`, `night-corpo`,
`night-liberty`.

### What the meter is

A Night City HUD strip. Panel black; a 1 px frame in the accent colour with the **top-right corner
chamfered** (cut at 45 degrees, 6 px) and a **hatch triangle** in the bottom-left corner (8x8, 45
degree hatch lines 2 px apart in accent at alpha 0.5). Faint scanlines every 3rd row at `t.ghost`.

**Cells.** 16 chamfered cells across the interior (each averaging 4 bands), 2 px apart, each with a
1 px outline in `t.edge` and its own top-right chamfer (2 px). A cell fills bottom-up to its level
with a **45 degree hatch** in `t.lit` (lines 3 px apart, 1 px wide) capped by a solid 2 px bar in
`t.lit`; the cell outline switches to `t.hot` while the band is within 0.05 of its peak; the peak
itself is a 1 px `t.hot` tick decaying at `peak_fall`. Cells never draw outside their box.

**Scanner.** A 1 px polyline in `t.zones[0].lit` (each colourway registers exactly one `Zone { upto: 1.0,
lit: <scanner>, hot: <scanner> }`; the family reads only `.lit`), cyan by default, across the top third tracing the
raw 64-band spectrum left to right, with a 2-frame afterimage at alpha 0.35 (kept in the struct, no
allocation). It sits above the cells; when `h < 48` it overlays the cells' top third instead.

**Readouts.** Left column, 3x5 font, `t.edge` labels with `t.lit` values, 3 rows when `h >= 52`, 1 row
otherwise: `RAM` followed by 4 block glyphs filled by rms quartiles; `HP` and a 2-3 digit number =
`round(100 * (1 - peak_mean))` (drops when loud, heals in silence); `NET` and a live 2-digit hex from
the RNG advanced once per strong onset. Readouts occupy a fixed 34 px column; cells start after it.
At `w < 160` the readouts are dropped and cells use the full width.

**Colourways:**

`ghost` is the scanline alpha (f32). Readout VALUES are `lit`, labels `edge`; on `night-arasaka` the
values are white (`hot`) so red is reserved for the cells.

| id | panel | lit | hot | edge | ghost | zones[0].lit (scanner) | note |
|---|---|---|---|---|---|---|---|
| `night-yellow` | `#000000` | `#FCEE0A` | `#FFFFFF` | `#8a8410` | 0.10 | `#00F0FF` | the classic |
| `night-arasaka` | `#050505` | `#FF003C` | `#FFFFFF` | `#7a0a20` | 0.10 | `#FF003C` | red on black, white readouts |
| `night-netrunner` | `#000408` | `#00F0FF` | `#FF2BD6` | `#0a6a70` | 0.12 | `#37EBF3` | cyan primary, magenta peaks |
| `night-corpo` | `#1a1a1a` | `#E6E6E6` | `#FCEE0A` | `#5a5a5a` | 0.08 | `#9a9a9a` | white/grey; yellow only on peaks |
| `night-liberty` | `#000000` | `#FCEE0A` | `#FF003C` | `#6a6410` | 0.10 | `#FF003C` | yellow with red peaks and scanner |

All `lit`/`panel` pairs clear 3:1 by inspection (yellow on black ~17:1, red on black ~5.3:1, cyan
~14:1, `#E6E6E6` on `#1a1a1a` ~13:1).

### Flourish — relic malfunction

`Envelope` 600 ms. While above 0: the whole finished frame is re-composited as an **RGB split** —
the frame copied into scratch and drawn back three times with only R / G / B channels offset by
`-3 / 0 / +3` px times `env`; two horizontal **glitch bars** (full width, 3 px tall, `#FF003C` at
alpha `env`) sweep top to bottom at different speeds; the readouts column shows `SYSTEM` /
`MALFUNCTION` / `RELIC 2.0 ERR` in red (`#FF003C`) instead of RAM/HP/NET (on `night-arasaka` the
text is white). Below 0.1 the frame is clean.

### Tests specific to `night`

- `cells_fill_with_their_bands`: band 0..3 at 0.9 and all else 0 -> cell 0's painted rows >= 60 % of
  its height and cell 8's painted rows <= 10 %.
- `chamfer_is_cut`: the pixel at the frame's top-right corner is panel; the pixel 6 px along the top
  edge from the corner is accent.
- `readouts_are_dropped_on_a_narrow_panel`: at 128x44, the first cell begins within 6 px of the
  interior left edge.
- `malfunction_leaves_the_panel_after_the_envelope`: 50 frames after `force_next()` the frame equals
  a never-fired frame at the same state (tolerance 1/255 per channel).

## Review focus

1. **190x48 and 128x44**: both families must still read; `sesh` drops the stamp and shrinks the word,
   `night` drops readouts. Nothing draws outside the panel.
2. **Silence**: `sesh` shows rolling scanlines + the word + the stamp; `night` shows the frame, empty
   cells, a flat scanner and `HP 100`.
3. **Flourish inside the canvas** at narrow sizes; `sesh`'s first-frame blank and `night`'s RGB split
   must not write outside the interior.
4. **Contrast**: `sesh-red`'s `#c8102e` is used only as `hot` on outlines and the REC dot, never as
   the word fill on black (3:1 fails).
5. **No trademark**: reviewers check no skull, no samurai, no "2077" glyph run, no logo geometry.

## Performance

Per-pixel work is the static field (`sesh` flourish only), the hatch fills (`night`, cell-bounded)
and the RGB split (`night` flourish, one scratch copy + three channel passes). Both families avoid
`bloom`. Target < 1.0 ms/frame steady and < 1.5 ms during a flourish at 380x60 release, measured by
the family's `probe_*_cost` with and without `force_next()`.

## Out of scope

Drips interacting with the tape tear; a fourth-wall "Windows 95 dialog" gag for `sesh`; braindance
scrubber or skyline for `night`; light-taskbar variants; any new dependency.
