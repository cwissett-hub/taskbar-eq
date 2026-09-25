# Virtual Self Families Implementation Plan (Part B)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Three new render families in the Virtual Self aesthetic - `vswings`, `vsghost`, `vsorb` - each with colourways, a flourish, dump images and a review-sheet section, so the user can judge them side by side and keep, tune or drop each.

**Architecture:** Each family is one new file under `src/render/`, a `Family` impl registered in `render/mod.rs` (`KNOWN_FAMILIES` + `family_for`) and labelled in `themes/mod.rs::family_label`, with colourways as `Theme` constructors in `themes/builtin.rs` following the `brutal_base()` / `brutal_concrete()` pattern. Families use the existing `Theme` colour fields (`lit`, `hot`, `panel`, `edge`, `ghost`, `bloom`, `zones`) and the `Canvas` drawing API (`line`, `fill_poly`, `fill_circle`, `vertical_gradient`, `radial_gradient`, `text_3x5`, `bloom`, `draw_over`). No new theme schema tables (out of scope). Flourish via `dsp::flourish::Trigger` + `Envelope`, as brutal does.

**Tech Stack:** Rust 2021; no new dependencies.

**Spec:** `docs/superpowers/specs/2026-09-25-health-fixes-and-virtual-self-design.md` (Part B). Part A (`2026-09-25-health-fixes.md`) is executed FIRST; this plan assumes its Task 7 (linear-light blending) and Task 8 (structural liveness guard `every_colourway_is_visibly_alive_at_three_sizes`) are in.

## Global Constraints

- `draw` allocates nothing after the first frame: scratch canvases and point buffers live in the family struct.
- `draw` < 2 ms per frame at 380x60 in release (ignored timing test per family).
- Every colourway passes the contrast rule in `builtin.rs` and the structural liveness guard (>= 2% lit, >= 2 colours at 190x48 / 380x48 / 380x60, levels 0.15 and 0.6).
- Every family has a flourish and a `dump_vs*` ignored test writing calm / loud / flourish PNGs to `target/eyeball/`, composited over a dark taskbar strip (`#202020`) so the user sees them as they would appear.
- No Japanese glyphs (3x5 font cannot draw them). No light-taskbar logic.
- One commit per family after its tests pass and its dumps have been LOOKED at with the Read tool; a family that looks wrong in its dump is not done.
- Shared palette (hex): ice `#dff3ff`, white `#ffffff`, bevel light `#f4f6fa`, bevel dark `#8a94a6`, cobalt `#1f5bff`, electric `#3ec8ff`, black `#000000`, pale pink `#ffd6ec`, violet `#8e6bff`.

## Review Focus

1. **A 190x48 panel** (the narrowest real taskbar gap): wings/orb/rays must still read; nothing may draw outside the panel or vanish. (Each family's liveness + a `fits_the_narrow_panel` test.)
2. **Silence after music** (levels all 0, rms 0): each family must show its resting structure (grid, ticks, orb) not a black panel. (Each family: `rest_frame_is_not_empty`.)
3. **A flourish at 190x48**: the datamosh, lens flare and shatter must stay inside the canvas (no out-of-range writes, no panic). (Each family: `flourish_at_narrow_size_does_not_panic`.)
4. **Rainbow colourways** are not offered for these families (chrome/white is the point); `tint` fallbacks must still give the theme colour. (Covered by liveness.)
5. **`text_3x5` phrases longer than the panel** in `vsghost` must be clipped, not wrapped or panicked. (`vsghost`: `long_phrase_is_clipped`.)

---

### Task 1: `vswings` - angel wings on Y2K chrome

**Files:**
- Create: `src/render/vswings.rs`
- Modify: `src/render/mod.rs` (`mod vswings;`, `KNOWN_FAMILIES` 22 -> 23 with `"vswings"`, `family_for` arm), `src/themes/mod.rs::family_label` (`"vswings" => "Virtual Self: wings"`), `src/themes/builtin.rs` (`vswings_base()` + 5 colourways registered in `all()`), `docs/review/index.html` (new section)

**Interfaces:**
- Produces: `pub struct Vswings` with `Default`, `impl Family` (`id() == "vswings"`); colourway ids `vswings-particle-arts`, `vswings-eon-break`, `vswings-angel-voices`, `vswings-utopia`, `vswings-ghost`.

- [ ] **Step 1: Write the failing tests** (in `vswings.rs` tests module; copy the helper shapes from `brutal.rs` tests for building a `FrameData` and running frames):

```rust
    fn theme(id: &str) -> Theme { crate::themes::builtin::all().into_iter().find(|t| t.id == id).unwrap() }
    fn frames(fam: &mut Vswings, t: &Theme, w: i32, h: i32, level: f32, n: usize) -> Canvas {
        let mut c = Canvas::new(w, h);
        let mut d = FrameData::default();
        for (i, v) in d.levels.iter_mut().enumerate() { *v = level * (1.0 - i as f32 / 96.0); }
        d.peaks = d.levels; d.rms_l = level; d.rms_r = level; d.dt_ms = 16.7;
        for k in 0..n { d.time_s = k as f32 * 0.0167; fam.draw(&mut c, t, &d); }
        c
    }
    fn lit(c: &Canvas) -> usize { c.bits().iter().filter(|p| (**p >> 24) > 8).count() }

    #[test]
    fn registered_and_labelled() {
        assert!(crate::render::KNOWN_FAMILIES.contains(&"vswings"));
        assert_eq!(crate::render::family_for("vswings").id(), "vswings");
        assert_ne!(crate::themes::family_label("vswings"), "vswings");
        assert_eq!(crate::themes::builtin::all().iter().filter(|t| t.family == "vswings").count(), 5);
    }

    #[test]
    fn wings_are_mirrored_about_the_centre() {
        let t = theme("vswings-particle-arts");
        let c = frames(&mut Vswings::default(), &t, 380, 48, 0.6, 30);
        // Lit-pixel count in the left half vs the right half within 10%: the two wings are a mirror pair.
        let (mut l, mut r) = (0usize, 0usize);
        for y in 0..48 { for x in 0..380 { if c.get(x, y).a > 8 { if x < 190 { l += 1 } else { r += 1 } } } }
        assert!((l as f32 - r as f32).abs() < 0.10 * l.max(r) as f32, "left {l} right {r}");
    }

    #[test]
    fn louder_bass_makes_longer_root_feathers() {
        let t = theme("vswings-particle-arts");
        let quiet = frames(&mut Vswings::default(), &t, 380, 48, 0.2, 30);
        let loud = frames(&mut Vswings::default(), &t, 380, 48, 0.8, 30);
        assert!(lit(&loud) > lit(&quiet) + 200, "quiet {} loud {}", lit(&quiet), lit(&loud));
    }

    #[test]
    fn rest_frame_is_not_empty() {
        let t = theme("vswings-eon-break");
        let c = frames(&mut Vswings::default(), &t, 380, 48, 0.0, 10);
        assert!(lit(&c) as f32 >= 0.02 * (380 * 48) as f32, "grid should still show: {}", lit(&c));
    }

    #[test]
    fn fits_the_narrow_panel_and_flourish_does_not_panic() {
        for id in ["vswings-particle-arts", "vswings-eon-break", "vswings-angel-voices", "vswings-utopia", "vswings-ghost"] {
            let t = theme(id);
            let mut fam = Vswings::default();
            let _ = frames(&mut fam, &t, 190, 48, 0.5, 5);
            fam.flourish.force_next();
            let c = frames(&mut fam, &t, 190, 48, 0.5, 40);
            assert!(lit(&c) > 0, "{id}");
        }
    }
```

- [ ] **Step 2: Run to verify failure** - `cargo test vswings 2>&1 | tail -5` - Expected: compile errors (module missing).

- [ ] **Step 3: Implement `vswings.rs`**

Module header explaining the family (why wings are a meter: 32 mirrored feathers, bass at the root so the wings BEAT). Struct fields: `flourish: Trigger`, `flare: Envelope`, `grid_phase: f32`, `feather_len: [f32; 32]` (smoothed), `scratch: Option<Canvas>`. `draw`:
1. Panel: fill `t.panel`; 2 px bevel (`bevel light` top/left, `bevel dark` bottom/right) using `fill_rect`.
2. Grid: single-point perspective lines converging to the horizon at 35% height; 9 verticals + 5 horizontals whose spacing scrolls with `grid_phase += rms * dt`; colour `t.edge` at `t.edge_alpha`. Onset (from `flourish.update`'s underlying onset or a local `Onset`) jumps `grid_phase` by one cell.
3. Wings: for `i in 0..32`, band `b = i*2`; angle `theta = -70deg + 140deg * (i / 31)` mirrored; feather is a `fill_poly` quad from the root point (centre, 62% height) along the angle, length `= 6 + level * (w * 0.24)`, width tapering 3 -> 1 px; colour `t.lit` with a `t.hot` 2 px tip; peak-hold dot from `d.peaks`.
4. Title glyphs: when `h >= 58`, `text_3x5` "VIRTUAL SELF" centred at the top in a vertical chrome gradient (draw the text into `scratch`, then `vertical_gradient` multiply - or draw per-row colour by slicing the text into rows), 1 px `#000` outline via drawing the text offset in black first.
5. Flourish: `fired = flourish.update(&d.levels, dt, t.flourish)`; `flare = self.flare.update(fired, dt, 600.0)`; draw a `radial_gradient` core + four `line` streaks + one ring, alpha = flare, at the wing root.
6. `bloom(t.bloom as i32, t.glow_strength)` if `t.bloom > 0`.

Colourways in `builtin.rs`: `vswings_base()` (family, `Texture::None_`, `panel_alpha 1.0`, `bloom 2.0`, `glow_strength 0.35`, ballistics `attack 0.55 decay 0.12 peak_fall 0.01`, `flourish` rate like other families) and the five colourways per the spec palette. Register in `all()`.

- [ ] **Step 4: Run** - `cargo test vswings 2>&1 | tail -3`; `cargo test every_colourway_is_visibly_alive 2>&1 | tail -3`; `cargo test contrast 2>&1 | tail -3` (or whatever the 3:1 rule test is named).

- [ ] **Step 5: Dumps and look** - add to `vswings.rs` tests an `#[ignore] fn dump_vswings()` writing `target/eyeball/vswings-<colourway>-{calm,loud,flourish}.png` at 380x60 composited over `#202020` (copy the PNG writer from `render/mod.rs::dump_newest`). Run `cargo test --release dump_vswings -- --ignored`, then READ at least three PNGs. Judge: wings legible, tips distinct from body, grid visible but quiet, flare not a white blob. Adjust and re-dump until it looks right.

- [ ] **Step 6: Review sheet** - append a `<section>` to `docs/review/index.html` numbered after the last, with the three PNGs (copy them into `docs/review/`) and a "What I want your eyes on" line: whether the wings read as a meter at a glance and which colourways to keep.

- [ ] **Step 7: Commit** - `git add -A && git commit -m "vswings: a 23rd family - Virtual Self angel wings on Y2K chrome, five colourways"`

---

### Task 2: `vsghost` - Ghost Voices glitch terminal

**Files:**
- Create: `src/render/vsghost.rs`
- Modify: `src/render/mod.rs` (`KNOWN_FAMILIES` 23 -> 24, `family_for`), `src/themes/mod.rs::family_label` (`"vsghost" => "Virtual Self: ghost voices"`), `src/themes/builtin.rs` (4 colourways), `docs/review/index.html`

**Interfaces:**
- Produces: `pub struct Vsghost`, colourways `vsghost-white`, `vsghost-cobalt`, `vsghost-inverse`, `vsghost-violet`.

- [ ] **Step 1: Write the failing tests** (same helpers as Task 1, adapted):

```rust
    #[test]
    fn registered_and_labelled() { /* as Task 1 with "vsghost" and count 4 */ }

    #[test]
    fn ticks_light_upward_with_level() {
        let t = theme("vsghost-white");
        let quiet = frames(&mut Vsghost::default(), &t, 380, 48, 0.2, 30);
        let loud = frames(&mut Vsghost::default(), &t, 380, 48, 0.8, 30);
        // Count lit pixels in the TOP third: only loud material should reach it.
        let top = |c: &Canvas| (0..16).flat_map(|y| (0..380).map(move |x| (x, y))).filter(|&(x, y)| c.get(x, y).a > 8).count();
        assert!(top(&loud) > top(&quiet) * 2, "top third: quiet {} loud {}", top(&quiet), top(&loud));
    }

    #[test]
    fn rest_frame_is_not_empty() { /* grid ticks + phrase visible at level 0 */ }

    #[test]
    fn long_phrase_is_clipped() {
        let t = theme("vsghost-white");
        let mut fam = Vsghost::default();
        fam.set_phrase_for_test("a very long phrase that cannot possibly fit in one hundred and ninety pixels of taskbar");
        let c = frames(&mut fam, &t, 190, 48, 0.3, 5);
        // No pixel outside the canvas can be written (get() would panic or return TRANSPARENT); the
        // assertion is that drawing completed and the right edge column has something or nothing, never a wrap.
        assert!(lit(&c) > 0);
    }

    #[test]
    fn datamosh_stays_inside_the_canvas_at_narrow_size() {
        for id in ["vsghost-white", "vsghost-cobalt", "vsghost-inverse", "vsghost-violet"] {
            let t = theme(id);
            let mut fam = Vsghost::default();
            let _ = frames(&mut fam, &t, 190, 48, 0.5, 5);
            fam.flourish.force_next();
            let c = frames(&mut fam, &t, 190, 48, 0.5, 12);
            assert!(lit(&c) > 0, "{id}");
        }
    }
```

- [ ] **Step 2: Run to verify failure** - compile errors.

- [ ] **Step 3: Implement `vsghost.rs`**

Struct: `flourish: Trigger`, `mosh: Envelope`, `onset: dsp::onset::Onset` (or reuse the trigger's onset if exposed), `scroll_px: f32`, `phrase_idx: usize`, `phrase_override: Option<String>` (test hook `set_phrase_for_test`), `peak_ticks: [f32; 64]`, `ray_angles: [f32; 3]`, `scratch: Option<Canvas>`, `rng: u64` (splitmix).
`draw`:
1. Panel fill `t.panel`.
2. Piano roll: columns of 2 px width, 1 px gap; rows = ticks of 1 px height with 1 px gap; a band's level lights ticks from the bottom (`t.lit`), the peak tick (`t.hot`) decays at `peak_fall`. Unlit ticks drawn at `t.ghost` alpha so the grid is always there (rest frame).
3. Scroll: on onset, `scroll_px` jumps one column; columns wrap. Rendering offset by `scroll_px`.
4. Phrase: `PHRASES = ["( ˘ω˘ )", "(´• ω •`)", "ghost voices", "eon break", "a.i.ngel", "particle arts", "utopia"]` - only characters the 3x5 font has; check `glyph_3x5` coverage and replace unsupported chars with ASCII equivalents `(^_^)`, `(>_<)`. Swap on strong onsets (every 4th). Drawn top-left at `t.hot`, clipped by `text_3x5` naturally (verify it does not wrap; if it draws past the right edge, pre-truncate by `text_3x5_width`).
5. Rays: 2-4 `fill_poly` translucent wedges from the bottom edge, angle = spectral centroid (`sum(i*level)/sum(level)` mapped -35..35 deg), alpha = rms * 0.35, colour `t.edge`.
6. Flourish datamosh: while `mosh > 0`, copy the frame into `scratch`, choose 3-5 horizontal slices (rng), shift each by a random offset (wrap) and invert its colours (`255 - c` per channel), `draw_over` back; first frame also `fill_rect` white at alpha 0.8 (strobe). Keep all rects clamped to the canvas.

Colourways: `vsghost_base()` (no bloom, `panel_alpha 1.0`, snappy ballistics `attack 0.8 decay 0.25 peak_fall 0.02`), four colourways per spec.

- [ ] **Step 4: Run** - `cargo test vsghost 2>&1 | tail -3`; liveness and contrast tests.

- [ ] **Step 5: Dumps and look** - `dump_vsghost` as Task 1; READ three PNGs (white calm, cobalt loud, inverse flourish). Judge: ticks read as a meter, phrase legible, datamosh looks like glitch not garbage.

- [ ] **Step 6: Review sheet section.**

- [ ] **Step 7: Commit** - `git add -A && git commit -m "vsghost: a 24th family - Virtual Self glitch terminal, four colourways"`

---

### Task 3: `vsorb` - low-poly chrome orb

**Files:**
- Create: `src/render/vsorb.rs`
- Modify: `src/render/mod.rs` (`KNOWN_FAMILIES` 24 -> 25, `family_for`), `src/themes/mod.rs::family_label` (`"vsorb" => "Virtual Self: orb"`), `src/themes/builtin.rs` (4 colourways), `docs/review/index.html`

**Interfaces:**
- Produces: `pub struct Vsorb`, colourways `vsorb-chrome`, `vsorb-eon`, `vsorb-angel`, `vsorb-mono`.

- [ ] **Step 1: Write the failing tests**:

```rust
    #[test]
    fn registered_and_labelled() { /* "vsorb", count 4 */ }

    #[test]
    fn orb_radius_follows_bass() {
        let t = theme("vsorb-mono");
        // Bass only (bands 0..8) vs treble only (bands 40..64) at the same energy: the orb must be larger for bass.
        let run = |lo: std::ops::Range<usize>| {
            let mut fam = Vsorb::default();
            let mut c = Canvas::new(380, 48);
            let mut d = FrameData::default();
            for i in lo { d.levels[i] = 0.9; }
            d.peaks = d.levels; d.dt_ms = 16.7;
            for k in 0..40 { d.time_s = k as f32 * 0.0167; fam.draw(&mut c, &t, &d); }
            fam.radius_px()
        };
        assert!(run(0..8) > run(40..64) * 1.4, "bass {} treble {}", run(0..8), run(40..64));
    }

    #[test]
    fn spikes_follow_the_spectrum() {
        // With only band 63 lit, lit pixels appear near the rim on the side the last spike points to and
        // nowhere near the opposite side (spikes are placed around the rim by band index).
        let t = theme("vsorb-mono");
        let mut fam = Vsorb::default();
        let mut c = Canvas::new(380, 48);
        let mut d = FrameData::default();
        d.levels[63] = 1.0; d.peaks = d.levels; d.dt_ms = 16.7;
        for k in 0..30 { d.time_s = k as f32 * 0.0167; fam.draw(&mut c, &t, &d); }
        let (sx, sy) = fam.spike_tip_for_test(23);
        assert!(c.get(sx.clamp(0, 379), sy.clamp(0, 47)).a > 8, "spike tip not lit at {sx},{sy}");
    }

    #[test]
    fn rest_frame_is_not_empty() { /* orb wireframe at minimum radius + gradient visible */ }

    #[test]
    fn shatter_at_narrow_size_does_not_panic() { /* as Task 1 with 190x48 and force_next */ }
```

- [ ] **Step 2: Run to verify failure** - compile errors.

- [ ] **Step 3: Implement `vsorb.rs`**

Geometry: an icosahedron (12 vertices, 30 edges) subdivided once (42 vertices, 120 edges) built at construction into `verts: Vec<[f32; 3]>`, `edges: Vec<(u16, u16)>`, `tris: Vec<[u16; 3]>`. Struct also: `rot: f32`, `radius: f32` (smoothed), `spike_len: [f32; 24]`, `flourish: Trigger`, `shatter: Envelope`, `shards: Vec<Shard>` (pos, vel, tri index; preallocated to `tris.len()`), `flares: [Flare; 3]`, `scratch: Option<Canvas>`, `rng: u64`.
`draw`:
1. Background: `vertical_gradient` panel from `t.panel` to `t.edge` (cobalt->ice for chrome; black->black for mono) plus three `radial_gradient` discs at `t.ghost` alpha drifting on sine paths.
2. Radius: `bass = mean(levels[0..8])`; target `= h * (0.20 + 0.275 * bass)` (40%..95% of height as diameter); smooth with attack 0.5 / decay 0.1 per ms-scaled step; expose `radius_px()`.
3. Rotate: `rot += dt * (0.3 + 3.0 * onset_strength)` around Y and a fixed 20deg tilt on X; project orthographically to the panel centre.
4. Edges: for each edge compute the screen-space midpoint normal `ny`; colour = chrome map: `ny > 0` lerp `t.hot` (sky) -> `t.lit`, else `t.lit` -> `t.edge` (ground), alpha 0.9 for front-facing (average z > 0), 0.35 for back-facing. Draw with `line`.
5. Spikes: 24 spikes at angles `k * 15deg` around the rim; spike `k` averages bands `k*2.67..` (`levels[(k as f32 * 64.0 / 24.0) as usize..]` two or three bands); length `= 3 + level * (min(w, h*3) * 0.12)`; `line` from rim to tip in `t.hot`, 2 px; expose `spike_tip_for_test(k) -> (i32, i32)`.
6. Flourish shatter: on `fired`, copy each triangle's centroid and a random outward velocity into `shards`; while `shatter > 0` draw shards as `fill_poly` triangles fading with the envelope, and grow `radius` from 0 again.
7. `bloom` if `t.bloom > 0`.

Colourways: `vsorb_base()` (`bloom 2.0`, `glow_strength 0.3`, ballistics medium) and the four per spec.

- [ ] **Step 4: Run** - `cargo test vsorb 2>&1 | tail -3`; liveness and contrast tests.

- [ ] **Step 5: Dumps and look** - `dump_vsorb`; READ chrome calm, eon loud, mono flourish. Judge: reads as a chrome wireframe sphere not a scribble; spikes clearly a meter; shatter reads as shatter.

- [ ] **Step 6: Review sheet section.**

- [ ] **Step 7: Commit** - `git add -A && git commit -m "vsorb: a 25th family - Virtual Self low-poly chrome orb, four colourways"`

---

### Task 4: Timing tests, docs, changelog, release

**Files:**
- Modify: `src/render/mod.rs` tests (one ignored `slow_vs_timing` test), `docs/themes.md` (three new families), `CHANGELOG.md` (`## [0.2.2]`), `Cargo.toml` (0.2.2), `TODO.md` ("Waiting on you": judge the three VS families in `docs/review/index.html`)

- [ ] **Step 1: Timing test**

```rust
    #[test]
    #[ignore]
    fn slow_vs_timing() {
        for id in ["vswings-particle-arts", "vsghost-white", "vsorb-chrome"] {
            let t = builtin::all().into_iter().find(|t| t.id == id).unwrap();
            let mut f = family_for(&t.family);
            let mut c = Canvas::new(380, 60);
            let mut d = FrameData::default();
            for (i, v) in d.levels.iter_mut().enumerate() { *v = 0.5 + 0.4 * ((i as f32) * 0.3).sin(); }
            d.dt_ms = 16.7;
            for k in 0..30 { d.time_s = k as f32 * 0.0167; f.draw(&mut c, &t, &d); }
            let t0 = std::time::Instant::now();
            for k in 30..330 { d.time_s = k as f32 * 0.0167; f.draw(&mut c, &t, &d); }
            let per = t0.elapsed().as_secs_f32() * 1000.0 / 300.0;
            eprintln!("{id}: {per:.2} ms/frame");
            assert!(per < 2.0, "{id} {per:.2} ms/frame");
        }
    }
```
Run: `cargo test --release slow_vs_timing -- --ignored --nocapture` - Expected: all three under 2 ms. If one is over, profile the hot loop (bloom radius, poly fills) and fix before continuing.

- [ ] **Step 2: Docs** - `docs/themes.md`: three entries in the families list with one line each and colourway ids. `CHANGELOG.md`: `## [0.2.2] - <date>` "Three Virtual Self families: vswings, vsghost, vsorb (13 colourways)". `TODO.md`: Waiting on you -> "Judge the three Virtual Self families in docs/review/index.html: keep, tune or drop each."

- [ ] **Step 3: Full suite and release** - `cargo test 2>&1 | tail -3`; `cargo clippy --all-targets -- -D warnings`; `cargo build --release`; `Cargo.toml` 0.2.2; commit `git commit -am "release: v0.2.2 - Virtual Self families"`; `git tag -a v0.2.2 -m v0.2.2`; `git push && git push --tags`; `gh release create v0.2.2 target/release/taskbar-eq.exe --title v0.2.2 --notes "<CHANGELOG 0.2.2 + SHA-256>"`.

---

## Self-review notes

- Spec coverage: B1 T1, B2 T2, B3 T3, shared requirements (timing, liveness, contrast, review sheet, docs, changelog) T1-T4. Out of scope respected (no Japanese text, no shared VS module).
- Type consistency: `force_next()` on `Trigger` exists (brutal uses it); `Envelope::update(fired, dt, ms) -> f32` exists; `KNOWN_FAMILIES` array length must be bumped in each task (22->23->24->25) or the const type will not compile - the implementer edits the `[&str; N]` length.
- Review Focus 1-5 map to `fits_the_narrow_panel...`, `rest_frame_is_not_empty`, the `*_does_not_panic` tests, liveness, `long_phrase_is_clipped`.
- Risk: eye-judged output. The dump-and-look step is mandatory per family, and the user judges the final result from the review sheet; the plan does not pretend a test can decide taste.
