# `bling`, `nos`, `drift` and `prism` families — design

Four new render families requested 30 Sep 2026:
- a "sparkly gangsta Y2K" family in the style of old Blingee GIFs;
- two Fast & Furious families — the early films / 2 Fast 2 Furious with the "DANGER TO MANIFOLD"
  NOS-screen motif, and Tokyo Drift;
- a family from an image the user supplied: a glowing prismatic rainbow arc on a dark plum sky over a
  rim-lit dark horizon curve.

Four colourways each. Ships as **v0.4.0**. The user delegated the bling meter choice ("whatever you
think best displays the music") and asked for "two families for the fast and furious". This spec is
the authority; the plan argues from it.

## Shared rules

- Opaque panel first, `clip_to_rounded_rect(1, 2, w-2, h-4, 3)` last on every return path.
- Allocation-free `draw` after frame 1: pools, scratch canvases and baked backgrounds preallocated in
  the struct (rebuilt only on (w,h) change); numbers in `[u8; N]`; no `format!`/`String`/`Vec`.
- < 1 ms steady / < 1.5 ms flourish at 380x60 release (2 ms gate in `slow_vs_timing`).
- 3:1 contrast, liveness at 128x44 / 128x60; tests measure paint over the panel (never alpha).
- **No double smoothing.** `FrameData.levels` are already smoothed by `Smoother::new(theme.ballistics)`
  in main.rs. Families must NOT re-apply `t.ballistics` attack/decay (peak holds are fine).
- All text via `render::font3x5` (2x scaling = each glyph pixel drawn 2x2); every string char has a
  glyph (asserted); truncate to whole glyphs, never wrap.
- Flourishes via `Trigger` + `Envelope`, bass-gated: trigger AND `mean(levels[0..8]) >= 0.6` (the
  `sesh` pattern, `#[cfg(test)]` bypass via `Trigger::was_forced()`).
- Constant-seed splitmix RNG; NaN guards; no new dependencies; no `bloom` on opaque panels (it changes
  no pixel and costs ~1 ms); **no logos, car badges or film wordmarks** — our own fonts and shapes.
  Menu labels may name the aesthetic.
- Registration: `KNOWN_FAMILIES` 28 -> 32; README count/label test **197 colourways / 32 families**;
  `docs/themes.md`; review sheet sections 23-26; `dump_<family>` and `probe_<family>_cost` `#[ignore]`.

## Family 1: `bling` — a Blingee GIF, circa 2007

Label **"Blingee: bling bling"**. Ids `bling-pink`, `bling-gold`, `bling-ice`, `bling-myspace`.

**Meter choice (delegated): rhinestone bars.** A word cannot show 64 levels at once; gem columns can.
Glitter text and stamps carry the Blingee identity as decoration.

- **Glitter panel.** Sparse per-pixel glitter: ~4 % of interior pixels (positions hashed once per
  (w,h), stored in a preallocated list) twinkle in `lit` / `hot` / white, brightness
  `0.3 + 0.7 * |sin(t * rate + phase)|`, rates 2-6 Hz. Density rises with rms to 8 %.
- **Rhinestone bars (the meter).** 24 columns (bands folded 64 -> 24). Each column is a stack of 5x5
  faceted gems on a 6 px pitch (bright centre pixel, lighter top-left facet, darker bottom-right,
  1 px dark outline), lit bottom-up to the level in `lit`, the top gem in `hot`. A white 3x3 `+`
  **glint** travels up each lit column once per ~0.8 s (per-column phase). Peak hold: one `hot` gem
  above the column, falling at `peak_fall`.
- **Glitter text (decoration).** One phrase centred in the upper third, 2x `font3x5` with a 1 px dark
  outline, filled with a moving glitter gradient (`lit` -> white -> `hot`, shifting at 30 px/s).
  Phrases `BLING BLING`, `ICED OUT`, `4 REAL`, `XOXO`, `~*UR MINE*~`, `$$$`, `HOTTIE`, `LUV U 4EVA`,
  swapping every 4th strong onset. Dropped when `h < 52` or it will not fit.
- **Stamps.** Up to 3 preallocated stamps pop in on strong onsets at random upper-half positions and
  fade over 600 ms, scaling in (one frame at 60 %, then 100 %): a 7x7 sparkle burst, a `$`, a 7x5
  crown, a 7x6 heart with a star — our own bitmaps.
- **Flourish — flash.** 500 ms: frame 1 a white flash at alpha 0.85; then every gem turns diamond
  white with a chrome sweep (a diagonal bright band crossing left to right over 400 ms) and a 15x15
  spinning 4-point sparkle star (drawn with `line`) at the loudest column.

| id | panel | lit | hot | edge | ghost | note |
|---|---|---|---|---|---|---|
| `bling-pink` | `#2a0a22` | `#ff5fc8` | `#ffd6f2` | `#c0c0d0` | 0.10 | hot-pink glitter, silver outlines |
| `bling-gold` | `#1a1204` | `#ffc83a` | `#fff2c0` | `#a88a3a` | 0.10 | gold glitter, diamond gems |
| `bling-ice` | `#06121e` | `#8fe8ff` | `#ffffff` | `#6aa0c0` | 0.10 | "iced out", blue-white |
| `bling-myspace` | `#000000` | `#b6ff3a` | `#ff4fd8` | `#3a3a3a` | 0.12 | lime and pink, very 2006 |

## Family 2: `nos` — a street-racing dash (the early films, 2 Fast 2 Furious)

Label **"Fast & Furious: NOS"**. Ids `nos-2fast`, `nos-original`, `nos-quarter`, `nos-miami`.

- **Panel.** Dark in-car display with a carbon-fibre weave (4x4 diagonal checker one shade off the
  panel, baked once).
- **Tacho (the meter).** 48 LED segments on a 150° arc across the lower two-thirds (bands folded
  64 -> 48), each a 2x4 px block along the arc, lit when its band's level passes its threshold — the
  arc fills left to right as a spectrum. Colour by position: `lit` to 70 %, amber `#ffb000` to 90 %,
  red `#ff2a2a` above. A 1 px `hot` needle from the arc centre sweeps with rms.
- **Shift lights.** 10 LEDs across the top — green x3, amber x3, red x3, blue — lighting with rms; at
  rms_norm >= 0.9 all flash blue at 8 Hz.
- **Gear + speed.** Right side, 2x `font3x5`: gear `N`,`1`-`6` from rms, and a 3-digit speed
  `000`-`199` = `round(rms_norm * 199)` with `MPH`. At `w < 200` only the gear shows.
- **Under-glow.** On bass onsets a neon strip in `zones[0].lit` glows up from the bottom edge
  (8 px vertical gradient, alpha 0.6 -> 0), decaying over 250 ms.
- **Flourish — the NOS hit.** 800 ms. Frames up to 300 ms: the interior becomes the green-on-black
  warning — panel `#000000`, 1 px `#39ff5a` frame, `DANGER TO MANIFOLD` centred in 2x `font3x5`
  green (on two lines `DANGER TO` / `MANIFOLD` if it will not fit on one), blinking at 4 Hz, `NOS`
  and a draining bar gauge beneath. Then a **purge**: back to the dash with ~12 preallocated white
  speed lines (1 px streaks moving left at 400 px/s) and the tacho pinned at redline, decaying with
  the envelope.

| id | panel | lit | hot | edge | ghost | under-glow `zones[0].lit` | note |
|---|---|---|---|---|---|---|---|
| `nos-2fast` | `#07060c` | `#39c0ff` | `#ffffff` | `#2a2a3a` | 0.10 | `#b43aff` | 2 Fast: blue LEDs, purple glow |
| `nos-original` | `#0a0806` | `#ff9a1a` | `#ffffff` | `#3a2a1a` | 0.10 | `#3aff5a` | 2001 LA: orange and green |
| `nos-quarter` | `#050505` | `#e6e6e6` | `#ff2a2a` | `#303030` | 0.08 | `#39c0ff` | a quarter mile at a time |
| `nos-miami` | `#0c0414` | `#ff5fc8` | `#b6ff3a` | `#2a1a3a` | 0.12 | `#39ffe0` | Miami neon: pink and lime |

## Family 3: `drift` — Tokyo Drift

Label **"Fast & Furious: Tokyo Drift"**. Ids `drift-shibuya`, `drift-touge`, `drift-orange`, `drift-night`.

- **Scene (baked once per (w,h)).** A skyline silhouette along the bottom third (hashed block heights,
  a few lit windows in `edge`) and neon sign blocks in `zones[0].lit` at the top (short 2-4 px bars,
  some flickering at 1-2 Hz).
- **The meter — a drift line.** Our own 14x6 side-profile car silhouette in `lit` with a `hot`
  tail-light slides along a ground line near the bottom. Its **tyre-smoke trail is the meter**: 64
  puffs along the trail (bands in order, from the car backwards), each a filled circle of radius
  `1 + level * 7` px in white at `ghost` alpha — the smoke billows with the spectrum. The car's x
  swings with rms (a slow pendulum between 25 % and 75 % of the width); a 3-bitmap set (left, level,
  right) shows the drift angle. The trail follows a fixed ring of the car's last 64 positions.
- **Steering gauge.** Top right, a radius-8 semicircle with a needle at the drift angle and
  `ANGLE NN` in `font3x5` (`[u8; 2]`). Dropped when `w < 200`.
- **Flourish — the drift.** 900 ms: the car whips across the full width in one sweep, the smoke goes
  to max radius tinted `hot`, the neon signs flash, and `DRIFT!` (2x `font3x5`, `hot`, 1 px outline)
  slams in centred for 400 ms. No Japanese glyphs.

| id | panel | lit | hot | edge | ghost | neon `zones[0].lit` | note |
|---|---|---|---|---|---|---|---|
| `drift-shibuya` | `#06040e` | `#e6e6ee` | `#ff2a5a` | `#2a2440` | 0.35 | `#ff4fd8` | night Shibuya: pink and cyan |
| `drift-touge` | `#040806` | `#d8e8d8` | `#ffb000` | `#1a2a20` | 0.30 | `#39ff8a` | mountain pass: green neon |
| `drift-orange` | `#0a0602` | `#ff7a1a` | `#ffffff` | `#2a1a0a` | 0.35 | `#ff7a1a` | orange-and-black livery |
| `drift-night` | `#020408` | `#8fb8ff` | `#ffffff` | `#141c2a` | 0.30 | `#39c0ff` | cold blue night |

## Family 4: `prism` — the user's image: a spectral arc over a dark horizon

Label **"Prism: light bloom"**. Ids `prism-sunset`, `prism-aurora`, `prism-mono`, `prism-dawn`.

Reference (user-supplied screenshot): a deep plum-to-black sky; a wide glowing **prismatic band**
arcing across it, dispersing orange -> hot pink -> yellow -> green -> cyan -> blue, soft-edged and
luminous; beneath it a dark horizon curve with a thin bright rim-light where the glow meets it; faint
translucent magenta sheets in the upper corners.

- **Backdrop (baked once per (w,h)).** A vertical gradient panel (`panel` at the top to near-black at
  the bottom) with two faint translucent sheets: large soft ellipses in `edge` at alpha 0.12 in the
  top-left and top-right corners.
- **Horizon.** A dark curve (a wide ellipse arc, apex at ~70 % height, dipping to ~90 % at the
  edges) filled below with near-black; its top edge gets a 1 px **rim-light** in `hot` whose
  brightness at each x follows the arc's glow directly above it.
- **The spectral arc (the meter).** A band following a wider concentric ellipse above the horizon.
  Along the arc, position = frequency: 64 samples left -> right, each sample's hue from the colourway's
  spectral stops (sunset: orange -> pink -> yellow -> green -> cyan -> blue), mixed in linear light.
  The band's **thickness** at each sample is `2 + level * 10` px (half above, half below the centre
  line) and its **brightness** `0.35 + 0.65 * level`, with a soft falloff: each sample drawn as a
  column of 3 layers (core at full, a 2 px halo at 0.4, a 4 px halo at 0.15 alpha), so the arc glows
  without `bloom`. Adjacent samples blend (draw with overlap), so it reads as one continuous band.
- **Breathing.** The whole arc drifts vertically ±2 px with a slow 8 s cycle and its hue stops rotate
  by ±4 % with rms, so a loud passage shifts the spectrum slightly — like the light moving.
- **Flourish — flare.** 700 ms, bass-gated: the arc's core brightens to white along its length from
  the loudest point outward (a travelling highlight), the rim-light flares full-width, and a soft
  lens flare (three concentric filled circles at alpha 0.2/0.1/0.05 in `hot`) blooms at the arc's
  apex.

Spectral stops per colourway (left -> right along the arc):

| id | panel (top) | hot (rim) | edge (sheets) | ghost | stops |
|---|---|---|---|---|---|
| `prism-sunset` | `#1a0616` | `#ffd0b0` | `#ff4fa0` | 0.10 | `#ff5a1a #ff3a8a #ffd21a #5aff7a #3ad2ff #5a6aff` (the image) |
| `prism-aurora` | `#04121a` | `#c0fff0` | `#3affc0` | 0.10 | `#3aff8a #3affe0 #3aa0ff #a05aff` |
| `prism-mono` | `#0a0a0e` | `#ffffff` | `#8a8a9a` | 0.08 | `#d0d0d8 #ffffff #d0d0d8` (a silver bloom) |
| `prism-dawn` | `#140a04` | `#fff0c0` | `#ffb05a` | 0.10 | `#ff7a1a #ffb03a #ffe07a #fff2d0` |

`lit` for each is the brightest stop (used by the contrast test) and must clear 3:1 on its panel.

## Tests specific to each family

- `bling`: `gem_columns_are_the_meter` (bass-only: leftmost columns >= 4 lit gems, rightmost <= 1);
  `glitter_twinkles` (two frames 100 ms apart differ in >= 30 glitter pixels at rest);
  `stamps_pop_on_strong_onsets_and_fade` (forced: stamp pixels exist; gone after 700 ms);
  `every_phrase_char_has_a_glyph`; `flash_flourish_whites_the_panel_first`.
- `nos`: `the_tacho_fills_left_to_right_with_the_spectrum` (bass-only: first 10 segments lit, last 10
  dark); `shift_lights_flash_blue_at_redline` (the 10th LED blue in some of 8 frames and off in
  another); `nos_flourish_shows_the_manifold_warning` (forced: `#39ff5a` text pixels on early frames,
  gone after 900 ms); `speed_and_gear_fit_or_drop` (128x44: no speed readout).
- `drift`: `the_smoke_trail_billows_with_the_spectrum` (bass-only vs treble-only: puffs near the car
  larger for bass); `the_car_swings_with_rms`; `drift_flourish_sweeps_the_car_across` (forced: car x
  spans >= 60 % of the width within 900 ms); `every_label_char_has_a_glyph`.
- `prism`: `the_arc_swells_where_the_music_is` (bass-only: the arc's painted thickness on the left
  third > the right third by >= 2x); `hue_runs_along_the_arc` (at uniform level, the leftmost arc
  pixel is closer to the first stop and the rightmost to the last); `the_rim_light_follows_the_glow`;
  `flare_flourish_brightens_the_core`.
- All four: `registered_and_labelled`, `rest_frame_is_not_empty`,
  `fits_the_narrow_panel_and_flourish_does_not_panic` (190x48 + 128x44, forced flourish),
  `the_four_colourways_are_visibly_different` (>= 15 % interior pixels pairwise).

## Review focus

1. 128x44: bling drops text, gem pitch 5 px; nos drops speed and scales the tacho; drift drops the
   gauge; prism's arc and horizon scale; nothing outside the panel.
2. Silence: glitter twinkles; empty tacho, gear `N`, speed `000`; the car idles centre with a thin
   trail; the arc glows thin at 0.35 brightness.
3. Flourishes stay inside at narrow sizes; `DANGER TO MANIFOLD` falls back to two lines.
4. No double smoothing: reviewer greps for `ballistics.attack` / `ballistics.decay` in the four files.
5. No logo / badge / wordmark geometry.

## Out of scope

Car models or badges from the films; Japanese text; GIF import; schema changes.
