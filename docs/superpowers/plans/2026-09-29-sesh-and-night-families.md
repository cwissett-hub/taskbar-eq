# `sesh` and `night` Families Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Two new render families - `sesh` (Bones / TeamSESH VHS tape) and `night` (Cyberpunk 2077 HUD strip) - five colourways each, a flourish each, dumps and review-sheet sections, shipped as v0.3.0.

**Architecture:** One bespoke pixel-blackletter font module (`src/render/gothic.rs`) consumed only by `sesh`. Each family is one file under `src/render/`, a `Family` impl registered in `render/mod.rs` (`KNOWN_FAMILIES` 25 -> 27) and labelled in `themes/mod.rs::family_label`, with colourways as `Theme` constructors in `themes/builtin.rs` following the `vsghost_base()` / `vsghost_white()` pattern. Families use existing `Theme` fields (`lit`, `hot`, `panel`, `panel_alpha`, `edge`, `edge_alpha`, `ghost` (an f32 alpha), `zones[0].lit`, `flourish`, `ballistics`) and the `Canvas` API (`fill_rect`, `line`, `text_3x5`, `text_3x5_width`, `rounded_rect`, `clip_to_rounded_rect`, `copy_region`, `get`, `draw_over`, `clear`). Flourish via `dsp::flourish::Trigger` + `Envelope`; onsets via `dsp::onset::Flux`. No new dependencies, no new theme schema.

**Tech Stack:** Rust 2021, rustc 1.96 (CI pinned). `cargo clippy --all-targets -- -D warnings` is a CI gate.

**Spec:** `docs/superpowers/specs/2026-09-29-sesh-and-night-families-design.md`. The spec is the authority; this plan is its argument. Sibling reference implementations: `src/render/vsghost.rs` (text, onset, RNG, slice effects, test helpers, dump writer) and `src/render/vswings.rs` (scratch layer, clipping).

## Global Constraints

- `draw` allocates nothing after the first frame: scratch canvases, drip/glitch-bar arrays and word buffers live in the family struct; the only per-frame heap use is inside shared canvas primitives (`fill_poly`'s scanline Vec - avoid `fill_poly` and `bloom` in both families).
- `draw` < 2.0 ms/frame at 380x60 release (gate in `slow_vs_timing`); target < 1.0 ms steady and < 1.5 ms during a flourish, because the CI runner is ~1.5x slower than the dev machine.
- Panel is OPAQUE: `panel_alpha: 1.0` in both bases; `rounded_rect(1, 2, w-2, h-4, 3, panel)` is drawn first and `clip_to_rounded_rect(1, 2, w-2, h-4, 3)` last.
- Every colourway passes `every_lit_colour_clears_three_to_one_against_its_own_panel` and `every_colourway_is_visibly_alive_at_two_sizes` (128x44 / 128x60: >= 2 % painted, >= 2 colours at levels 0.15 and 0.6).
- Tests count "painted" pixels as `a > 8 && |dR|+|dG|+|dB| > 24` against the panel colour (copy `drew_over_panel` / `lit` from `vsghost.rs` tests), never by alpha alone.
- Colour mixing goes through `Rgba::lerp_linear` or the canvas's blend; the RGB channel split and inversion effects operate on bytes and are allowed.
- Words come from fixed `const` arrays; a test asserts every character has a glyph; a word wider than its box is truncated to whole glyphs, never wrapped.
- RNG is splitmix64 seeded from a constant (copy `Vsghost::rng` and its `next_u64` helper).
- No trademarked marks: no skull-and-crossbones, no samurai, no "2077" glyph run, no logo geometry. Words only in our own fonts.
- README.md must say **173 colourways** / **27 families** and list both labels (`the_readme_states_the_real_colourway_and_family_counts` enforces it). `docs/themes.md` gains one line per family. `every_family_ships_and_no_theme_is_orphaned` requires >= 4 colourways per family.
- One commit per task after its tests pass, its clippy is clean and its dumps have been LOOKED at with the Read tool. A family that looks wrong in its dump is not done.
- Palette (hex): `sesh` monochrome `#0a0a0a #e8e6e0 #ffffff #3a3a3a #111111`, red `#c8102e`, chroma `#ff2a2a #2ad2ff`; `night` yellow `#FCEE0A`, cyan `#00F0FF` / `#37EBF3`, red `#FF003C`, magenta `#FF2BD6`, corpo `#E6E6E6` / `#1a1a1a`.

## Review Focus

1. **A 190x48 panel and a 128x44 panel**: `sesh` drops the stamp below 48 rows and keeps a 9 px word; `night` drops the readouts below 160 px wide. Nothing draws outside the panel. (T2 `fits_the_narrow_panel`, T3 `readouts_are_dropped_on_a_narrow_panel`.)
2. **Silence** (levels 0, rms 0): `sesh` shows rolling scanlines + word + stamp; `night` shows frame, empty cells, flat scanner, `HP 100`. (T2/T3 `rest_frame_is_not_empty`.)
3. **A flourish at 190x48**: `sesh`'s blank first frame and static, `night`'s RGB split and glitch bars, all inside the interior, no panic. (T2 `dropout_first_frame_is_blank` at 190x48, T3 `malfunction_leaves_the_panel_after_the_envelope`.)
4. **A word longer than the panel** (`SESHOLLOWATERBOYZ` at 128 px): truncated to whole glyphs, centred, never wrapped. (T1 `truncates_to_whole_glyphs`, T2 `the_word_is_centred_and_whole`.)
5. **`sesh-red`'s red** is used only as `hot` (outline, REC dot), never as the word fill on black - 3:1 fails. (T2 colourway table + the contrast test.)

---

### Task 1: `gothic` - the pixel-blackletter font

**Files:**
- Create: `src/render/gothic.rs`
- Modify: `src/render/mod.rs` (`mod gothic;` next to the other modules)

**Interfaces:**
- Produces:
  - `pub const GOTHIC_CHARS: &str = "SEHBONTAMLWYZ "` - the only characters with glyphs.
  - `pub enum GothicSize { Small, Large }` - 9 rows (glyphs 5-6 columns) and 13 rows (7-8 columns).
  - `pub fn glyph(ch: char, size: GothicSize) -> Option<(&'static [u16], i32)>` - rows as bitmasks (bit 0 = leftmost column) and the glyph's width in columns; `None` for an unsupported char.
  - `pub fn text_width(text: &str, size: GothicSize) -> i32` - sum of glyph widths + 1 px between glyphs (space is 3 px, no trailing gap).
  - `pub fn truncate_to_width(text: &str, size: GothicSize, max_w: i32) -> &str` - the longest prefix whose `text_width` <= `max_w`, cut on glyph boundaries.
  - `pub fn draw(c: &mut Canvas, x: i32, y: i32, text: &str, size: GothicSize, fill: Rgba, outline: Option<Rgba>) -> i32` - draws `text` with its top-left at (x, y); if `outline` is given, draws the text first at the eight 1 px offsets in the outline colour, then the fill; returns the width drawn.

- [ ] **Step 1: Write the failing tests** in `src/render/gothic.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::canvas::{Canvas, Rgba};

    #[test]
    fn every_supported_char_has_a_glyph_at_both_sizes_and_rows_fit_the_width() {
        for ch in GOTHIC_CHARS.chars() {
            for size in [GothicSize::Small, GothicSize::Large] {
                let (rows, w) = glyph(ch, size).unwrap_or_else(|| panic!("{ch:?} {size:?}"));
                let want_rows = match size { GothicSize::Small => 9, GothicSize::Large => 13 };
                assert_eq!(rows.len(), want_rows, "{ch:?} {size:?} row count");
                assert!(w >= 3 && w <= 8, "{ch:?} {size:?} width {w}");
                for (i, r) in rows.iter().enumerate() {
                    assert_eq!(r >> w, 0, "{ch:?} {size:?} row {i} has bits beyond width {w}");
                }
                if ch != ' ' {
                    assert!(rows.iter().any(|r| *r != 0), "{ch:?} {size:?} is blank");
                }
            }
        }
        assert!(glyph('Q', GothicSize::Small).is_none());
    }

    #[test]
    fn blackletter_strokes_are_two_pixels_wide_somewhere_in_every_letter() {
        // The tell of the style: at least one row of every letter has two adjacent set bits.
        for ch in GOTHIC_CHARS.chars().filter(|c| *c != ' ') {
            let (rows, _) = glyph(ch, GothicSize::Large).unwrap();
            assert!(rows.iter().any(|r| (r & (r >> 1)) != 0), "{ch:?} has no heavy stroke");
        }
    }

    #[test]
    fn width_is_glyphs_plus_gaps() {
        let s = text_width("SESH", GothicSize::Small);
        let sum: i32 = "SESH".chars().map(|c| glyph(c, GothicSize::Small).unwrap().1).sum();
        assert_eq!(s, sum + 3);
        assert_eq!(text_width("", GothicSize::Small), 0);
    }

    #[test]
    fn truncates_to_whole_glyphs() {
        let full = text_width("SESHOLLOWATERBOYZ", GothicSize::Small);
        let cut = truncate_to_width("SESHOLLOWATERBOYZ", GothicSize::Small, full / 2);
        assert!(!cut.is_empty() && cut.len() < 17);
        assert!(text_width(cut, GothicSize::Small) <= full / 2);
        let one_more = &"SESHOLLOWATERBOYZ"[..cut.len() + 1];
        assert!(text_width(one_more, GothicSize::Small) > full / 2);
        assert_eq!(truncate_to_width("SESH", GothicSize::Small, 1), "");
    }

    #[test]
    fn draw_paints_inside_its_box_and_outline_surrounds_fill() {
        let mut c = Canvas::new(80, 20);
        let fill = Rgba::from_hex("#ffffff", 1.0);
        let out = Rgba::from_hex("#ff0000", 1.0);
        let w = draw(&mut c, 4, 3, "SESH", GothicSize::Small, fill, Some(out));
        assert_eq!(w, text_width("SESH", GothicSize::Small));
        let mut white = 0;
        let mut red = 0;
        for y in 0..20 {
            for x in 0..80 {
                let p = c.get(x, y);
                if p.a > 8 {
                    assert!(x >= 3 && x <= 4 + w && y >= 2 && y <= 3 + 9, "paint at {x},{y} outside the box");
                    if p.r > 200 && p.g > 200 { white += 1 } else if p.r > 200 { red += 1 }
                }
            }
        }
        assert!(white > 40, "fill {white}");
        assert!(red > white / 2, "outline {red} vs fill {white}");
        // Any fill pixel's 4-neighbourhood is fill or outline, never empty.
        for y in 1..19 {
            for x in 1..79 {
                let p = c.get(x, y);
                if p.a > 8 && p.g > 200 {
                    for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                        assert!(c.get(x + dx, y + dy).a > 8, "fill at {x},{y} has a bare side");
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 2: Run to verify failure** - `cargo test gothic 2>&1 | tail -5` - Expected: compile errors (module missing).

- [ ] **Step 3: Implement `gothic.rs`**

Module doc: why a bespoke font (the shared 3x5 cannot do blackletter; the word IS the aesthetic). Then:

```rust
pub const GOTHIC_CHARS: &str = "SEHBONTAMLWYZ ";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GothicSize { Small, Large }
```

Glyph data as `const` arrays of `u16` rows, bit 0 = leftmost column, one `const` per letter per size, e.g. `const S_SMALL: ([u16; 9], i32) = ([...], 6);`. Hand-draw each with the blackletter tells: verticals 2 px wide, diamond terminals (a 1-px point above/below a 2-px stroke), broken bowls on `O`/`S`/`B` (a straight diagonal where a round letter would curve), hairline horizontals 1 px. Small = 9 rows, widths 5-6 (space 3); Large = 13 rows, widths 7-8 (space 4). Write the rows as binary literals with a leading comment showing the picture, e.g.

```rust
// .##..#
// #..##.
// #.....
// .##...
// ...##.
// ....#.
// #...#.
// .##.#.
// ..##..
```

`glyph` matches on `(ch, size)`; `text_width` sums widths plus 1 per gap (spaces have no extra gap logic - the space glyph is blank at its width); `truncate_to_width` walks chars accumulating width and returns `&text[..byte_idx]`; `draw` iterates rows/bits and `fill_rect(x + col, y + row, 1, 1, colour)` (it is a tiny glyph; per-pixel `fill_rect` is fine), outline pass first at the 8 offsets when `Some`.

- [ ] **Step 4: Run** - `cargo test gothic 2>&1 | tail -3` - Expected: 5 passed. `cargo clippy --all-targets -- -D warnings 2>&1 | tail -2`.

- [ ] **Step 5: Eyeball the font** - add an `#[ignore] fn dump_gothic()` test writing `target/eyeball/gothic-small.rgba` and `gothic-large.rgba` (all letters, white on `#202020`, using the writer from `vsghost.rs::dump_vsghost`), convert to PNG with the PowerShell snippet in `docs/review/` (or the one Task 10 of the health-fixes plan used: `System.Drawing` raw RGBA -> PNG), and READ both. Judge: reads as blackletter (heavy verticals, pointed terminals, broken bowls), every letter distinguishable, `S` and `E` not confusable. Fix rows and re-dump until it does. Copy the two PNGs to `docs/review/gothic-small.png` / `gothic-large.png`.

- [ ] **Step 6: Commit** - `git add -A && git commit -m "gothic: a pixel-blackletter font for the sesh family, 13 letters at 9 and 13 rows"`

---

### Task 2: `sesh` - Bones / TeamSESH VHS tape

**Files:**
- Create: `src/render/sesh.rs`
- Modify: `src/render/mod.rs` (`mod sesh;`, `KNOWN_FAMILIES` `[&str; 25]` -> `[&str; 26]` with `"sesh"`, `family_for` arm `"sesh" => Box::new(sesh::Sesh::default())`), `src/themes/mod.rs::family_label` (`"sesh" => "Bones: VHS tape"`), `src/themes/builtin.rs` (`sesh_base()` + 5 colourways in `all()`), `README.md` (count line -> `168 colourways` / `26 families`, add the label to the family list), `docs/themes.md` (one line + ids), `docs/review/index.html` (section 18)

**Interfaces:**
- Consumes: Task 1's `gothic::{GothicSize, glyph, text_width, truncate_to_width, draw}`.
- Produces: `pub struct Sesh` with `Default`, `impl Family` (`id() == "sesh"`); ids `sesh-tape`, `sesh-word`, `sesh-vhs`, `sesh-red`, `sesh-bleached`; `#[cfg(test)] pub fn set_word_for_test(&mut self, s: &str)`; `#[cfg(test)] pub fn suppress_tape_for_test(&mut self)` (draw the word layer only, for the centring test); `pub flourish: Trigger` field visible to tests via `fam.flourish.force_next()` (make the field `pub(crate)` as `vswings` does, or add `#[cfg(test)] pub fn force_flourish(&mut self)`).

- [ ] **Step 1: Write the failing tests** (copy `theme`, `frames`, `drew_over_panel`, `lit` from `vsghost.rs` tests, with `Vsghost` -> `Sesh`):

```rust
    #[test]
    fn registered_and_labelled() {
        assert!(crate::render::KNOWN_FAMILIES.contains(&"sesh"));
        assert_eq!(crate::render::family_for("sesh").id(), "sesh");
        assert_ne!(crate::themes::family_label("sesh"), "sesh");
        assert_eq!(crate::themes::builtin::all().iter().filter(|t| t.family == "sesh").count(), 5);
    }

    /// The lit streak of each tracking band sits at the band's horizontal shift, so a louder band's
    /// streak is further from where the quiet band's is.
    #[test]
    fn louder_bands_tear_further() {
        let t = theme("sesh-tape");
        let run = |level: f32| {
            let mut fam = Sesh::default();
            fam.set_word_for_test("");
            let mut c = Canvas::new(380, 48);
            let mut d = FrameData::default();
            for v in d.levels[0..16].iter_mut() { *v = level; }
            d.peaks = d.levels; d.dt_ms = 16.7;
            for k in 0..30 { d.time_s = k as f32 * 0.0167; fam.draw(&mut c, &t, &d); }
            // Mean |x - centre| of painted pixels in the bottom third: the tear displacement.
            let panel = Rgba::from_hex(&t.panel, 1.0);
            let (mut sum, mut n) = (0.0f32, 0usize);
            for y in 32..46 { for x in 0..380 { if drew_over_panel(c.get(x, y), panel) { sum += (x as f32 - 190.0).abs(); n += 1; } } }
            if n == 0 { 0.0 } else { sum / n as f32 }
        };
        let quiet = run(0.2);
        let loud = run(0.9);
        assert!(loud > quiet + 0.08 * 380.0 * 0.3, "quiet {quiet} loud {loud}");
    }

    #[test]
    fn the_word_is_centred_and_whole() {
        let t = theme("sesh-word");
        let mut fam = Sesh::default();
        fam.suppress_tape_for_test();
        fam.set_word_for_test("SESHOLLOWATERBOYZ");
        let c = frames(&mut fam, &t, 190, 48, 0.3, 5);
        let panel = Rgba::from_hex(&t.panel, 1.0);
        let (mut x0, mut x1) = (i32::MAX, i32::MIN);
        for y in 0..48 { for x in 0..190 { if drew_over_panel(c.get(x, y), panel) { x0 = x0.min(x); x1 = x1.max(x); } } }
        assert!(x0 < x1, "nothing drawn");
        let width = x1 - x0 + 1;
        assert!(width <= 190 - 4 - 8, "word too wide: {width}");
        let centre = (x0 + x1) as f32 / 2.0;
        assert!((centre - 95.0).abs() <= 2.5, "centre {centre}");
        // Whole glyphs: the drawn width equals the gothic width (+2 for the outline) of SOME prefix.
        let ok = (1..=17).any(|n| {
            let prefix = &"SESHOLLOWATERBOYZ"[..n];
            let gw = crate::render::gothic::text_width(prefix, crate::render::gothic::GothicSize::Small);
            (gw + 2 - width).abs() <= 1
        });
        assert!(ok, "drawn width {width} matches no whole-glyph prefix");
    }

    #[test]
    fn every_word_char_has_a_gothic_glyph() {
        for w in WORDS {
            for ch in w.chars() {
                for size in [crate::render::gothic::GothicSize::Small, crate::render::gothic::GothicSize::Large] {
                    assert!(crate::render::gothic::glyph(ch, size).is_some(), "{w:?} {ch:?}");
                }
            }
        }
    }

    #[test]
    fn rest_frame_is_not_empty() {
        let t = theme("sesh-vhs");
        let c = frames(&mut Sesh::default(), &t, 380, 48, 0.0, 10);
        assert!(lit(&c, &t) as f32 >= 0.02 * (380 * 48) as f32, "scanlines + word + stamp: {}", lit(&c, &t));
    }

    #[test]
    fn dropout_first_frame_is_blank() {
        for id in ["sesh-tape", "sesh-bleached"] {
            let t = theme(id);
            let mut fam = Sesh::default();
            let _ = frames(&mut fam, &t, 190, 48, 0.5, 5);
            fam.flourish.force_next();
            let c = frames(&mut fam, &t, 190, 48, 0.5, 1);
            // Interior only: the bevel/frame outside (1,2)-(w-2,h-4) is not part of the picture.
            let panel = Rgba::from_hex(&t.panel, 1.0);
            let mut painted = 0;
            for y in 4..44 { for x in 3..187 { if drew_over_panel(c.get(x, y), panel) { painted += 1; } } }
            assert_eq!(painted, 0, "{id}: {painted} painted pixels on the dropout frame");
        }
    }

    #[test]
    fn fits_the_narrow_panel_and_flourish_does_not_panic() {
        for id in ["sesh-tape", "sesh-word", "sesh-vhs", "sesh-red", "sesh-bleached"] {
            let t = theme(id);
            let mut fam = Sesh::default();
            let _ = frames(&mut fam, &t, 190, 48, 0.5, 5);
            fam.flourish.force_next();
            let c = frames(&mut fam, &t, 190, 48, 0.5, 40);
            assert!(lit(&c, &t) > 0, "{id}");
            let _ = frames(&mut Sesh::default(), &t, 128, 44, 0.6, 5);
        }
    }
```

- [ ] **Step 2: Run to verify failure** - `cargo test sesh 2>&1 | tail -5` - compile errors.

- [ ] **Step 3: Implement `sesh.rs`**

Constants: `TRACKING_BANDS: usize = 12`, `ROLL_PX_PER_S: f32 = 6.0`, `TEAR_FRAC: f32 = 0.18`, `DRIPS: usize = 8`, `DRIP_PX_PER_S: f32 = 18.0`, `DRIP_ACCEL: f32 = 40.0`, `WORD_SWAP_EVERY: u32 = 6`, `DROPOUT_MS: f32 = 550.0`, `STAMP_MIN_H: i32 = 48`, `BASS_ONSET: f32 = 0.55`, `ONSET_RATIO`/`ONSET_REFRACTORY_MS` copied from `vsghost.rs`.

```rust
pub const WORDS: [&str; 5] = ["SESH", "BONES", "TEAMSESH", "SESHOLLOWATERBOYZ", "TEAM SESH"];

pub struct Sesh {
    pub(crate) flourish: Trigger,
    dropout: Envelope,
    onset: crate::dsp::onset::Flux,
    onset_count: u32,
    strong_count: u32,
    roll_px: f32,
    elapsed_s: f32,
    rec_blink_ms: f32,
    word_idx: usize,
    word_override: Option<String>,
    tape_suppressed: bool,          // #[cfg(test)] only meaningful
    tear: [f32; TRACKING_BANDS],    // smoothed level per tracking band
    peak: [f32; TRACKING_BANDS],    // peak-hold shift, decays at peak_fall
    drips: [Drip; DRIPS],           // { x: i32, y: f32, vy: f32, alive: bool }
    scratch: Option<Canvas>,        // static field + torn word during the dropout
    rng: u64,
}
```

`fn mix(t: &Theme) -> f32 { match t.id.as_str() { "sesh-tape" => 0.25, "sesh-word" => 0.85, "sesh-vhs" => 0.5, "sesh-red" => 0.6, "sesh-bleached" => 0.5, _ => 0.5 } }`

`draw` order:
1. `rounded_rect(1, 2, w-2, h-4, 3, panel)` (opaque). Interior `ix0=3, iy0=4, ix1=w-3, iy1=h-4`.
2. Timekeeping: `dt = d.dt_ms` (finite, clamped 0..200), `elapsed_s += dt/1000`, `roll_px = (roll_px + ROLL_PX_PER_S*dt/1000) % 2.0`.
3. Onsets: `onset = self.onset.update(&d.levels, dt, ONSET_RATIO, ONSET_REFRACTORY_MS)`; `strong = onset && mean(levels[0..8]) > BASS_ONSET`; counters; word swap on `strong_count % WORD_SWAP_EVERY == 0` (skip when `word_override.is_some()`); `rec_blink_ms = 120` on strong.
4. Fold 64 bands into 12: band `b` covers `levels[b*64/12 .. (b+1)*64/12]`, `tear[b]` smoothed with `t.ballistics` attack/decay per ms (copy `vsghost`'s smoother), `peak[b]` = max(peak*(1-peak_fall*dt), tear).
5. Tape layer (unless `tape_suppressed`): for every interior row `y`, band `b = (iy1-1-y) * 12 / ih` (bottom = band 0); `shift = (tear[b] * TEAR_FRAC * w * (1.3 - mix)).round()` with sign `if b % 2 == 0 {1} else {-1}`; scanline rows (`(y + roll_px as i32) % 2 == 0`) get `fill_rect(ix0, y, iw, 1, lit @ ghost)` shifted: draw as two `fill_rect`s wrapping at the interior edges. Band top row: streak `fill_rect` over `[ix0+shift .. ]` in `lit` at alpha `(tear[b]*(1.2-mix)).min(1)`; band bottom row: dropout streak in `panel`-darkened (`lerp_linear(panel, black, 0.5)`; bleached: `lerp_linear(panel, lit, 0.35)`); peak: 1 px `hot` at `ix0 + peak_shift` for the band's middle row.
6. Stamp (when `h >= STAMP_MIN_H`): `text_3x5(ix0+2, iy0+1, "PLAY", hot)` + a 3-row solid triangle via three `fill_rect`s + `HH:MM:SS` from `elapsed_s`; top-right `REC` + 3x3 dot in `hot` when `rec_blink_ms > 0` (decrement by dt). Format the time into a fixed `[u8; 8]` buffer (no `format!`): write digits by hand and `std::str::from_utf8`.
7. Word layer: `size = if h >= 58 - (12.0*mix).round() as i32 { Large } else { Small }`; `word = truncate_to_width(word, size, iw - 8)`; `x = ix0 + (iw - text_width)/2`, `y` = vertically centred in the interior, `gothic::draw(c, x, y, word, size, lit, Some(edge @ edge_alpha))`; on `onset` offset `x` by `+-1` (sign from rng). `sesh-vhs` chroma bleed: draw twice more at `x-1`/`x+1` in `#ff2a2a`/`#2ad2ff` at alpha 0.5 BEFORE the main pass. `sesh-red`: outline colour is `hot` instead of `edge`.
8. Drips: on `strong`, spawn up to 2 dead drips at `x` = a random column inside the word's box, `y` = word bottom, `vy = DRIP_PX_PER_S`; each frame `vy += DRIP_ACCEL*dt/1000; y += vy*dt/1000`; draw `fill_rect(x, y as i32, 1, 2, lit)`; die at `iy1`.
9. Flourish: `fired = self.flourish.update(&d.levels, dt, t.flourish)`; `env = self.dropout.update(fired, dt, DROPOUT_MS)`. If `fired`: paint the interior with `panel` (`fill_rect(ix0, iy0, iw, ih, panel)`) and return after the clip (frame 1 = blank). Else if `env > 0.15`: static field - for each interior pixel row, per-pixel noise from `rng` into `scratch` (`lit`/`edge` at alpha `env*0.8`, roughly 50/50), `c.draw_over(scratch)`; then the word torn: draw the word into `scratch` (cleared), and `copy_region` three horizontal slices of it back at shifts `+-(0.3*iw*env)` with wrap (two `copy_region` calls per slice); stamp text `TRACKING` instead of the time.
10. `clip_to_rounded_rect(1, 2, w-2, h-4, 3)`.

NaN guards on every float read from `d` (copy `vsghost`'s `is_finite` pattern). `scratch` allocated once per (w,h); `Vec`-free otherwise.

Colourways in `builtin.rs` - `sesh_base()`: `family "sesh"`, `Texture::None_`, `panel_alpha 1.0`, `bloom 0.0`, `glow_strength 0.0`, `edge_alpha 0.9`, `ballistics { attack: 0.7, decay: 0.2, peak_fall: 0.015 }`, `flourish` like `vsghost_base`; then the five per the spec table (`ghost` values 0.22 / 0.12 / 0.18 / 0.18 / 0.25; names "Sesh Tape", "Sesh Word", "Sesh VHS", "Sesh Red", "Sesh Bleached"). Register in `all()`.

- [ ] **Step 4: Run** - `cargo test sesh 2>&1 | tail -3`; `cargo test every_colourway_is_visibly_alive 2>&1 | tail -3`; `cargo test three_to_one 2>&1 | tail -3`; `cargo test the_readme 2>&1 | tail -3` (after editing README); `cargo clippy --all-targets -- -D warnings 2>&1 | tail -2`.

- [ ] **Step 5: Dumps and look** - `#[ignore] fn dump_sesh()` (copy `dump_vsghost`'s writer and `frame` shaper) writing `sesh-<id>-{calm,loud,flourish,flourish-decay}.rgba` at 380x60 for all five plus `sesh-tape-190x48` and `sesh-word-128x44`. `#[ignore] fn probe_sesh_cost()` copied from `probe_vsghost_cost`, extended with a `force_next()` window as `probe_vsorb_cost` does; print steady and flourish ms/frame. Run `cargo test --release dump_sesh -- --ignored` and READ at least: tape-loud, word-calm, vhs-loud, red-loud, bleached-loud, tape-flourish. Judge: the tape tears read as a bottom-up meter; the word reads as blackletter and stays whole; the stamp is legible; the dropout is a blank frame then static then snap-back; the red is only on outline/dot; bleached reads as a bleached tape not a broken one. Adjust and re-dump until right.

- [ ] **Step 6: Review sheet** - append `<section>` 18 to `docs/review/index.html` mirroring section 16's structure: `<h2><span class="n">18</span>Bones: VHS tape</h2>`, a `what` paragraph, figures for tape-loud, word-calm, vhs-loud, red-loud, bleached-loud, tape-flourish (copy the PNGs into `docs/review/`), and a `judge` paragraph asking: does the tape read as a meter; does the word read as blackletter at 9 px; is the one red enough; is the bleached colourway a keep.

- [ ] **Step 7: Commit** - `git add -A && git commit -m "sesh: a 26th family - Bones/TeamSESH VHS tape with a blackletter word, five colourways"`

---

### Task 3: `night` - Cyberpunk 2077 HUD strip

**Files:**
- Create: `src/render/night.rs`
- Modify: `src/render/mod.rs` (`KNOWN_FAMILIES` 26 -> 27 with `"night"`, `family_for`), `src/themes/mod.rs::family_label` (`"night" => "Night City: HUD"`), `src/themes/builtin.rs` (`night_base()` + 5 colourways, each with `zones: vec![Zone { upto: 1.0, lit: <scanner>.into(), hot: <scanner>.into() }]`), `README.md` (-> `173 colourways` / `27 families`, label), `docs/themes.md`, `docs/review/index.html` (section 19)

**Interfaces:**
- Produces: `pub struct Night` with `Default`, `impl Family` (`id() == "night"`); ids `night-yellow`, `night-arasaka`, `night-netrunner`, `night-corpo`, `night-liberty`; `pub(crate) flourish: Trigger`; `#[cfg(test)] pub fn first_cell_x(&self) -> i32` (the x of cell 0's left edge as laid out on the last frame).

- [ ] **Step 1: Write the failing tests** (same helpers, `Night`):

```rust
    #[test]
    fn registered_and_labelled() { /* "night", label != id, count 5 */ }

    #[test]
    fn cells_fill_with_their_bands() {
        let t = theme("night-yellow");
        let mut fam = Night::default();
        let mut c = Canvas::new(380, 60);
        let mut d = FrameData::default();
        for v in d.levels[0..4].iter_mut() { *v = 0.9; }
        d.peaks = d.levels; d.dt_ms = 16.7;
        for k in 0..40 { d.time_s = k as f32 * 0.0167; fam.draw(&mut c, &t, &d); }
        let panel = Rgba::from_hex(&t.panel, 1.0);
        let lit_col = Rgba::from_hex(&t.lit, 1.0);
        let is_lit = |p: Rgba| drew_over_panel(p, panel) && (p.r as i32 - lit_col.r as i32).abs() < 40 && (p.g as i32 - lit_col.g as i32).abs() < 40;
        let filled_rows = |cell: i32| -> usize {
            let (x0, x1, y0, y1) = fam.cell_box_for_test(cell);
            (y0..y1).filter(|&y| (x0 + 1..x1 - 1).any(|x| is_lit(c.get(x, y)))).count()
        };
        let (_, _, y0, y1) = fam.cell_box_for_test(0);
        let h = (y1 - y0) as usize;
        assert!(filled_rows(0) * 100 >= h * 60, "cell 0 {} of {h}", filled_rows(0));
        assert!(filled_rows(8) * 100 <= h * 10, "cell 8 {} of {h}", filled_rows(8));
    }

    #[test]
    fn chamfer_is_cut() {
        let t = theme("night-yellow");
        let c = frames(&mut Night::default(), &t, 380, 48, 0.3, 5);
        let panel = Rgba::from_hex(&t.panel, 1.0);
        // Frame corner (top-right of the frame rect) is panel; 6px along the top edge is accent.
        let (fx1, fy0) = (380 - 3, 4); // frame's top-right corner pixel
        assert!(!drew_over_panel(c.get(fx1, fy0), panel), "corner should be cut");
        assert!(drew_over_panel(c.get(fx1 - 7, fy0), panel), "top edge should be drawn");
        assert!(drew_over_panel(c.get(fx1, fy0 + 7), panel), "right edge should be drawn");
    }

    #[test]
    fn readouts_are_dropped_on_a_narrow_panel() {
        let t = theme("night-yellow");
        let mut fam = Night::default();
        let _ = frames(&mut fam, &t, 128, 44, 0.3, 3);
        assert!(fam.first_cell_x() <= 3 + 6, "first cell at {}", fam.first_cell_x());
        let mut wide = Night::default();
        let _ = frames(&mut wide, &t, 380, 60, 0.3, 3);
        assert!(wide.first_cell_x() >= 3 + 34, "readouts should occupy 34px: first cell at {}", wide.first_cell_x());
    }

    #[test]
    fn rest_frame_is_not_empty() {
        let t = theme("night-corpo");
        let c = frames(&mut Night::default(), &t, 380, 48, 0.0, 10);
        assert!(lit(&c, &t) as f32 >= 0.02 * (380 * 48) as f32, "frame + cells + HP 100: {}", lit(&c, &t));
    }

    #[test]
    fn malfunction_leaves_the_panel_after_the_envelope() {
        let t = theme("night-netrunner");
        let mut a = Night::default();
        let mut b = Night::default();
        let _ = frames(&mut a, &t, 190, 48, 0.4, 5);
        let _ = frames(&mut b, &t, 190, 48, 0.4, 5);
        a.flourish.force_next();
        let ca = frames(&mut a, &t, 190, 48, 0.4, 50);
        let cb = frames(&mut b, &t, 190, 48, 0.4, 50);
        for y in 0..48 { for x in 0..190 {
            let (p, q) = (ca.get(x, y), cb.get(x, y));
            assert!((p.r as i32 - q.r as i32).abs() <= 1 && (p.g as i32 - q.g as i32).abs() <= 1 && (p.b as i32 - q.b as i32).abs() <= 1, "residue at {x},{y}");
        } }
    }

    #[test]
    fn flourish_at_narrow_size_does_not_panic() {
        for id in ["night-yellow", "night-arasaka", "night-netrunner", "night-corpo", "night-liberty"] {
            let t = theme(id);
            let mut fam = Night::default();
            let _ = frames(&mut fam, &t, 190, 48, 0.5, 5);
            fam.flourish.force_next();
            let c = frames(&mut fam, &t, 190, 48, 0.5, 12);
            assert!(lit(&c, &t) > 0, "{id}");
            let _ = frames(&mut Night::default(), &t, 128, 44, 0.6, 5);
        }
    }
```

`cell_box_for_test(cell) -> (x0, x1, y0, y1)` is a `#[cfg(test)]` accessor returning the cell's box from the last frame's layout (store the layout in the struct).

- [ ] **Step 2: Run to verify failure** - compile errors.

- [ ] **Step 3: Implement `night.rs`**

Constants: `CELLS: usize = 16`, `CELL_GAP: i32 = 2`, `CHAMFER_FRAME: i32 = 6`, `CHAMFER_CELL: i32 = 2`, `HATCH_PITCH: i32 = 3`, `READOUT_W: i32 = 34`, `READOUT_MIN_W: i32 = 160`, `SCANLINE_EVERY: i32 = 3`, `MALFUNCTION_MS: f32 = 600.0`, `SPLIT_PX: f32 = 3.0`, `GLITCH_BARS: usize = 2`.

```rust
pub struct Night {
    pub(crate) flourish: Trigger,
    malfunction: Envelope,
    onset: crate::dsp::onset::Flux,
    fill: [f32; CELLS],
    peak: [f32; CELLS],
    hp: f32,                       // heals toward 100 in silence, drops with peak mean
    net_hex: u8,                   // advanced on strong onsets
    scan_prev: [f32; NUM_BANDS],   // last frame's scanner heights (afterimage)
    scan_prev2: [f32; NUM_BANDS],
    layout: Layout,                // { cells: [(i32,i32,i32,i32); CELLS], first_cell_x: i32 } recomputed per frame, no alloc
    scratch: Option<Canvas>,       // RGB split source
    bars: [f32; GLITCH_BARS],      // glitch bar y positions (0..1 of ih)
    rng: u64,
}
```

`draw` order:
1. Opaque panel `rounded_rect(1, 2, w-2, h-4, 3, panel)`. Interior `ix0=3, iy0=4, ix1=w-3, iy1=h-4`.
2. Scanlines: every `SCANLINE_EVERY`th interior row `fill_rect(ix0, y, iw, 1, lit @ ghost)`.
3. Frame: 1 px `edge`-coloured rect outline via four `fill_rect`s, except the top-right corner: stop the top edge `CHAMFER_FRAME` px early, stop the right edge `CHAMFER_FRAME` px low, and join them with a `line` from `(ix1-1-CHAMFER_FRAME, iy0)` to `(ix1-1, iy0+CHAMFER_FRAME)`. Hatch triangle bottom-left: `line`s at 45 degrees, `HATCH_PITCH` apart, inside an 8x8 triangle, in `lit @ 0.5`.
4. Readouts (when `w >= READOUT_MIN_W`): column `[ix0+2, ix0+READOUT_W)`; rows at `iy0+2`, `+8`, `+14` when `h >= 52`, else only the `HP` row. `RAM` label in `edge` + 4 block glyphs (`fill_rect` 3x5 each) filled by `rms` quartiles in `lit`; `HP` + number: `hp = lerp(hp, 100*(1-mean(peaks)), 0.05)` per frame, formatted into a `[u8; 3]` buffer; `NET` + two hex digits from `net_hex` (advance `rng` on strong onsets). On `night-arasaka` the values are `hot`.
5. Layout: cells span `[cx0, ix1-2)` where `cx0 = ix0 + (READOUT_W if readouts else 0) + 2`; `cell_w = (span - (CELLS-1)*CELL_GAP) / CELLS`; cell `i` box `x0 = cx0 + i*(cell_w+CELL_GAP)`, `y0 = iy0 + (if h >= 48 { ih/3 } else { 2 })`, `y1 = iy1 - 2`. Store in `layout`.
6. Cells: fill per cell `= mean(levels[i*4..i*4+4])` smoothed with `t.ballistics`; outline 1 px in `edge` with a 2 px chamfer at its top-right (skip the corner pixel and the two adjacent); fill height `fh = fill*(y1-y0-2)`; hatch: for each row `y` in the fill zone, set pixels where `(x + y) % HATCH_PITCH == 0` in `lit` via 1 px `fill_rect`s (bounded by the cell box); solid 2 px cap `fill_rect(x0+1, y1-1-fh, cell_w-2, 2, lit)`; peak tick 1 px `hot` at the peak height; outline switches to `hot` while `|fill - peak| < 0.05`.
7. Scanner: for each band `i`, `x = cx0 + i*(span-1)/63`, `y = iy0 + 2 + (top_third_h)*(1-level)`; afterimages: draw `scan_prev2` polyline at `zones[0].lit @ 0.15`, `scan_prev` at `0.35`, current at `1.0` (`line` between consecutive points); then shift `scan_prev2 <- scan_prev <- current`. When `h < 48` the scanner overlays the cells' top third instead.
8. Flourish: `fired = flourish.update(...)`; `env = malfunction.update(fired, dt, MALFUNCTION_MS)`. If `fired`: `bars = [0.0, 0.3]`. If `env > 0.1`: copy the interior into `scratch` (`copy_region`), then re-composite as an RGB split: for each interior pixel, `r` from `scratch.get(x - dx, y)`, `g` from `(x, y)`, `b` from `(x + dx, y)` where `dx = (SPLIT_PX*env).round()`; write with `fill_rect(x, y, 1, 1, Rgba::new(r, g, b, 255))` (bounded to the interior; a per-pixel loop over <= 380x52 - measure it in the probe). Glitch bars: `bars[k] += (0.9 + 0.6*k)*dt/1000*env`, wrap; `fill_rect(ix0, iy0 + (bars[k]*ih) as i32, iw, 3, red @ env)`. Readouts show `SYSTEM` / `MALFUNCTION` / `RELIC 2.0 ERR` in `#FF003C` (white on arasaka) instead of RAM/HP/NET.
9. `clip_to_rounded_rect(1, 2, w-2, h-4, 3)`.

Colourways in `builtin.rs` - `night_base()`: `family "night"`, `Texture::None_`, `panel_alpha 1.0`, `bloom 0.0`, `glow_strength 0.0`, `edge_alpha 1.0`, `ballistics { attack: 0.75, decay: 0.22, peak_fall: 0.012 }`, `flourish` like `vsghost_base`; then the five per the spec table with `zones` set to the scanner colour and names "Night Yellow", "Night Arasaka", "Night Netrunner", "Night Corpo", "Night Liberty". Register in `all()`.

- [ ] **Step 4: Run** - `cargo test night 2>&1 | tail -3`; liveness; three_to_one; `the_readme`; clippy.

- [ ] **Step 5: Dumps and look** - `dump_night` (calm/loud/flourish/flourish-decay for all five at 380x60, plus `night-yellow-190x48`, `night-yellow-128x44`) and `probe_night_cost` with a `force_next()` window (the RGB split is the expensive path - report both numbers; if the flourish exceeds 1.5 ms, split only the cell band rows, not the whole interior). READ at least: yellow-loud, arasaka-loud, netrunner-calm, corpo-loud, liberty-flourish, yellow-128x44. Judge: cells read as a bottom-up meter with the hatch visible; the chamfer and hatch triangle read as 2077 UI; the scanner is a line not a smear; the readouts are legible; the malfunction reads as an RGB split with red bars, then clean; nothing resembles the samurai or the 2077 wordmark.

- [ ] **Step 6: Review sheet** - `<section>` 19 `Night City: HUD` mirroring section 16, figures for yellow-loud, arasaka-loud, netrunner-calm, corpo-loud, liberty-flourish, and a `judge` paragraph: does it read as the 2077 HUD without any lifted asset; are the readouts worth their 34 px; is the hatch visible at taskbar scale; is corpo a keep.

- [ ] **Step 7: Commit** - `git add -A && git commit -m "night: a 27th family - Cyberpunk 2077 HUD strip with hatch-filled chamfered cells, five colourways"`

---

### Task 4: Timing, docs, changelog, release prep

**Files:**
- Modify: `src/render/mod.rs` (`slow_vs_timing` ids), `docs/themes.md`, `CHANGELOG.md`, `Cargo.toml` (0.3.0), `TODO.md`

- [ ] **Step 1: Timing** - in `slow_vs_timing` (`src/render/mod.rs`) extend the id array to `["vswings-particle-arts", "vsghost-white", "vsorb-chrome", "sesh-vhs", "night-yellow"]`. Run `cargo test --release slow_vs_timing -- --ignored --nocapture 2>&1 | tail -8` - all five under 2.0 ms; the two new ones should be under 1.0 ms. If one is over, profile its stages (temporary `Instant` prints, removed before commit) and fix the family - do not loosen the gate.

- [ ] **Step 2: Docs** - `docs/themes.md`: counts 173 / 27 at the top; the two family lines complete (one sentence + ids); the "Adding your own" count. `CHANGELOG.md`: `## [0.3.0] — <today>` above 0.2.2: "Two families: `sesh` (Bones/TeamSESH VHS tape, blackletter word, dropout flourish; 5 colourways) and `night` (Cyberpunk 2077 HUD strip, hatch-filled chamfered cells, relic-malfunction flourish; 5 colourways)", plus "vsorb: ~2x cheaper per frame (cached backdrop, orb-bounded bloom)" and "gothic: a pixel-blackletter font module". `TODO.md`: "Last updated" describes v0.3.0; the two themes leave the In-progress queue (leaving the live `--levels` check); "Waiting on you" gains a FIRST bullet: judge the two new families on the review sheet (sections 18-19) - the specific eye questions from each family's `judge` paragraph. `Cargo.toml` version `0.3.0`.

- [ ] **Step 3: Full suite and release build (no tag/push/release - the controller does those)** - `cargo test 2>&1 | tail -3`; `cargo clippy --all-targets -- -D warnings 2>&1 | tail -2`; release build with path stripping in PowerShell: `$env:RUSTFLAGS = "--remap-path-prefix=$PWD=. --remap-path-prefix=$env:USERPROFILE\.cargo=~cargo"; cargo build --release; Remove-Item Env:RUSTFLAGS`; `Get-FileHash target/release/taskbar-eq.exe -Algorithm SHA256`; count `cwisset` in the exe (expect 0). Commit `git commit -am "release: v0.3.0 - sesh and night families"`.

---

## Self-review notes

- Spec coverage: shared requirements -> Global Constraints + T2/T3 tests; gothic font -> T1; `sesh` tape/stamp/word/drips/mix/flourish -> T2 steps 5-9; `night` frame/cells/scanner/readouts/flourish -> T3 steps 3-8; colourway tables -> T2/T3 builtin steps; performance -> probes in T2/T3 + T4 timing; review focus 1-5 -> named tests; no-trademark -> constraint + eye-test judge lines; v0.3.0 -> T4.
- Type consistency: `gothic::{GothicSize, glyph, text_width, truncate_to_width, draw}` used identically in T1 and T2; `Sesh::{set_word_for_test, suppress_tape_for_test, flourish}` and `Night::{first_cell_x, cell_box_for_test, flourish}` are declared in Interfaces and used by the tests as written; `KNOWN_FAMILIES` length bumps 25->26->27 in order.
- Review Focus: 1 -> T2/T3 narrow tests; 2 -> `rest_frame_is_not_empty` x2; 3 -> `dropout_first_frame_is_blank` (190x48) + `malfunction_leaves_the_panel_after_the_envelope` (190x48) + `flourish_at_narrow_size_does_not_panic` (128x44 frames); 4 -> T1 `truncates_to_whole_glyphs` + T2 `the_word_is_centred_and_whole` at 190 px (128 px is covered by the liveness guard's 128x44 frames in the narrow test); 5 -> colourway table (red only in `hot`) + the contrast test over `lit`/`panel`.
- Risk: eye-judged output; the dump-and-look step is mandatory per task and the user judges from the review sheet. The RGB split's per-pixel loop is the one cost risk; T3 step 5 pins the mitigation.
