# `term` Family Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** One new render family, `term` — the user's VSCode "2077" theme as a live terminal whose output lines are the meter — five colourways with their own session types, a `panic!` flourish, dumps and a review-sheet section, shipped as v0.3.1.

**Architecture:** One file `src/render/term.rs` with a per-colourway `Style` (row colours, command list, output lines) matched on `t.id`, registered in `render/mod.rs` (`KNOWN_FAMILIES` 27 -> 28), labelled in `themes/mod.rs`, colourways in `themes/builtin.rs`. All text through the shared `render::font3x5` (which gains `▮` and `▯`). Flourish via `dsp::flourish::Trigger` + `Envelope` gated on bass like `sesh`; onsets via `dsp::onset::Flux`. No new dependencies, no theme schema change.

**Tech Stack:** Rust 2021, rustc 1.96 (CI pinned), clippy `-D warnings` gate.

**Spec:** `docs/superpowers/specs/2026-09-29-term-family-design.md`. Reference implementations: `src/render/sesh.rs` (Style-per-id pattern, bass-gated flourish, font3x5 use, `[u8; N]` formatting, test helpers, dump writer, probe) and `src/render/night.rs` (readouts, `clip_label`).

## Global Constraints

- `draw` allocates nothing after the first frame: typed-command state is an index + length into `&'static str`s; numbers into `[u8; N]`; onset-gap history in a fixed `[f32; 8]`; no `format!`/`String`/`Vec` on the frame path; no `bloom`, no `fill_poly`.
- < 2.0 ms/frame gate; target < 0.5 ms steady, < 1.0 ms during the trace (CI runner ~1.5x slower).
- Opaque panel (`panel_alpha 1.0`) first, `clip_to_rounded_rect(1, 2, w-2, h-4, 3)` last on every return path.
- Tests count painted pixels against the panel colour (`drew_over_panel` from sesh's tests), never alpha.
- Every colourway passes the 3:1 contrast test and `every_colourway_is_visibly_alive_at_two_sizes` (128x44 / 128x60).
- All text via `font3x5::{draw, width}`; every char of every string constant has a glyph (asserted). Text wider than its box is truncated to whole glyphs, never wrapped.
- RNG splitmix64 with a constant seed; NaN guards on all `FrameData` reads.
- No trademark marks: no VSCode logo; "2077" is the colourway name only.
- README count/label test: **180 colourways / 28 families**, label "Terminal: 2077". `docs/themes.md` line. `every_family_ships_and_no_theme_is_orphaned` >= 4.
- One commit per task after tests pass, clippy is clean and dumps have been LOOKED at.

## Review Focus

1. **128x44**: status bar dropped, `L` ≈ 8 rows, line-number column fits, prompt + cursor visible, nothing outside the panel. (T1 `status_bar_dropped_below_48_rows` + the narrow test.)
2. **Silence**: prompt, blinking cursor, line numbers, status `▮ 00%`. (T1 `rest_frame_is_not_empty`.)
3. **Flourish at 190x48 / 128x44**: trace lines truncated to whole glyphs; scroll stays inside. (T1 `panic_trace_never_exceeds_the_interior`.)
4. **Onset starvation**: no onsets for 5 s — typing stalls, cursor still blinks, frame changes between frames. (T1 `cursor_blinks_without_onsets`.)
5. **Command lists per colourway are all glyph-complete and fit 128 px after truncation.** (T1 `every_command_and_label_char_has_a_glyph`.)

---

### Task 1: `term` — the family

**Files:**
- Modify: `src/render/font3x5.rs` (add `▮` solid block and `▯` outline glyphs + tests)
- Create: `src/render/term.rs`
- Modify: `src/render/mod.rs` (`mod term;`, `KNOWN_FAMILIES` `[&str; 27]` -> `[&str; 28]` with `"term"`, `family_for` arm), `src/themes/mod.rs::family_label` (`"term" => "Terminal: 2077"`), `src/themes/builtin.rs` (`term_base()` + 5 colourways in `all()`), `README.md` (`**180 colourways across 28 families**`, add `Terminal: 2077` to the family list), `docs/themes.md` (counts + one line + ids), `docs/review/index.html` (section 20)

**Interfaces:**
- Produces: `pub struct Term` with `Default`, `impl Family` (`id() == "term"`); ids `term-2077`, `term-2077-cyan`, `term-2077-hot`, `term-2077-matrix`, `term-2077-editor`; `pub(crate) flourish: Trigger`; `#[cfg(test)] pub fn typed_len_for_test(&self) -> usize`, `#[cfg(test)] pub fn line_base_for_test(&self) -> u32`, `#[cfg(test)] pub fn row_box_for_test(&self, row: usize) -> (i32, i32, i32, i32)` (x0, x1, y0, y1 of a meter row's bar area from the last frame), `#[cfg(test)] pub fn rows_for_test(&self) -> usize`.

- [ ] **Step 1: font glyphs** — in `font3x5.rs` add `'▮' => [0b111; 5]` and `'▯' => [0b111, 0b101, 0b101, 0b101, 0b111]` to `glyph`, extend `the_labels_the_families_need_all_have_glyphs` with `"▮▯>"` and a `term` label set (`"~/music", "UTF-8  LF", "EXIT 101", "# bpm ~ 142"`). Run `cargo test font3x5` (RED for the new chars first, then GREEN).

- [ ] **Step 2: Write the failing tests** in `term.rs` (copy `theme`, `frames`, `drew_over_panel`, `lit` from `sesh.rs` tests, `Sesh` -> `Term`):

```rust
    #[test]
    fn registered_and_labelled() {
        assert!(crate::render::KNOWN_FAMILIES.contains(&"term"));
        assert_eq!(crate::render::family_for("term").id(), "term");
        assert_ne!(crate::themes::family_label("term"), "term");
        assert_eq!(crate::themes::builtin::all().iter().filter(|t| t.family == "term").count(), 5);
    }

    #[test]
    fn rows_are_the_meter() {
        let t = theme("term-2077");
        let mut fam = Term::default();
        let mut c = Canvas::new(380, 60);
        let mut d = FrameData::default();
        for v in d.levels[0..8].iter_mut() { *v = 0.9; }
        d.peaks = d.levels; d.dt_ms = 16.7;
        for k in 0..40 { d.time_s = k as f32 * 0.0167; fam.draw(&mut c, &t, &d); }
        let panel = Rgba::from_hex(&t.panel, 1.0);
        let fill = |row: usize| -> f32 {
            let (x0, x1, y0, y1) = fam.row_box_for_test(row);
            let painted = (y0..y1).flat_map(|y| (x0..x1).map(move |x| (x, y))).filter(|&(x, y)| drew_over_panel(c.get(x, y), panel)).count();
            painted as f32 / ((x1 - x0) * (y1 - y0)) as f32
        };
        let rows = fam.rows_for_test();
        assert!(fill(rows - 1) >= 0.45, "bottom (bass) row {:.2}", fill(rows - 1));
        assert!(fill(0) <= 0.10, "top (treble) row {:.2}", fill(0));
    }

    #[test]
    fn prompt_types_on_onsets_and_scrolls_on_execute() {
        let t = theme("term-2077-hot");
        let mut fam = Term::default();
        let mut c = Canvas::new(380, 60);
        let mut d = FrameData::default();
        d.dt_ms = 16.7;
        let base0 = fam.line_base_for_test();
        // Alternate loud/quiet frames so every loud frame is an onset (the Flux detector needs a rise).
        for k in 0..80 {
            let loud = k % 6 == 0;
            for v in d.levels.iter_mut() { *v = if loud { 0.9 } else { 0.05 }; }
            d.peaks = d.levels; d.time_s = k as f32 * 0.0167;
            fam.draw(&mut c, &t, &d);
        }
        assert!(fam.typed_len_for_test() > 0 || fam.line_base_for_test() > base0, "nothing typed and nothing executed");
        // Keep going until at least one execute happened.
        for k in 80..400 {
            let loud = k % 6 == 0;
            for v in d.levels.iter_mut() { *v = if loud { 0.9 } else { 0.05 }; }
            d.peaks = d.levels; d.time_s = k as f32 * 0.0167;
            fam.draw(&mut c, &t, &d);
        }
        assert!(fam.line_base_for_test() > base0, "no command executed in 400 frames");
    }

    #[test]
    fn every_command_and_label_char_has_a_glyph() {
        for s in ALL_STRINGS() {
            for ch in s.chars() {
                assert!(crate::render::font3x5::glyph(ch).is_some(), "{s:?} {ch:?}");
            }
        }
    }

    #[test]
    fn status_bar_dropped_below_48_rows() {
        let t = theme("term-2077");
        let bar = Rgba::from_hex("#030d22", 1.0);
        let is_bar = |c: &Canvas, y: i32| (10..370).filter(|&x| { let p = c.get(x, y); (p.r as i32 - bar.r as i32).abs() < 6 && (p.g as i32 - bar.g as i32).abs() < 6 && (p.b as i32 - bar.b as i32).abs() < 6 }).count() > 300;
        let tall = frames(&mut Term::default(), &t, 380, 60, 0.3, 5);
        assert!((50..56).any(|y| is_bar(&tall, y)), "no status bar at 380x60");
        let short = frames(&mut Term::default(), &t, 380, 44, 0.3, 5);
        assert!(!(34..40).any(|y| is_bar(&short, y)), "status bar present at 380x44");
    }

    #[test]
    fn cursor_blinks_without_onsets() {
        let t = theme("term-2077");
        let mut fam = Term::default();
        let a = frames(&mut fam, &t, 380, 60, 0.0, 10);   // ~167 ms in
        let b = frames(&mut fam, &t, 380, 60, 0.0, 30);   // ~667 ms in: other half of the 1 Hz blink
        let panel = Rgba::from_hex(&t.panel, 1.0);
        let diff = (0..60).flat_map(|y| (0..380).map(move |x| (x, y))).filter(|&(x, y)| drew_over_panel(a.get(x, y), panel) != drew_over_panel(b.get(x, y), panel)).count();
        assert!(diff >= 9, "cursor did not blink: {diff} px differ");
    }

    #[test]
    fn rest_frame_is_not_empty() {
        let t = theme("term-2077-matrix");
        let c = frames(&mut Term::default(), &t, 380, 60, 0.0, 10);
        assert!(lit(&c, &t) as f32 >= 0.02 * (380 * 60) as f32, "prompt + numbers + status: {}", lit(&c, &t));
    }

    #[test]
    fn panic_trace_never_exceeds_the_interior() {
        for id in ["term-2077", "term-2077-cyan", "term-2077-hot", "term-2077-matrix", "term-2077-editor"] {
            let t = theme(id);
            for (w, h) in [(190, 48), (128, 44)] {
                let mut fam = Term::default();
                let _ = frames(&mut fam, &t, w, h, 0.5, 5);
                fam.flourish.force_next();
                let c = frames(&mut fam, &t, w, h, 0.5, 45);
                assert!(lit(&c, &t) > 0, "{id} {w}x{h}");
                let panel = Rgba::from_hex(&t.panel, 1.0);
                for y in 0..h { for x in [0, 1, w - 2, w - 1] { let p = c.get(x, y); assert!(p.a == 0 || !drew_over_panel(p, panel) || y < 2 || y >= h - 2, "{id}: paint at the edge {x},{y}"); } }
            }
        }
    }

    #[test]
    fn the_five_colourways_are_visibly_different() {
        let ids = ["term-2077", "term-2077-cyan", "term-2077-hot", "term-2077-matrix", "term-2077-editor"];
        let renders: Vec<Canvas> = ids.iter().map(|id| frames(&mut Term::default(), &theme(id), 380, 60, 0.7, 30)).collect();
        for i in 0..ids.len() { for j in (i + 1)..ids.len() {
            let (a, b) = (&renders[i], &renders[j]);
            let n = (4..56).flat_map(|y| (3..377).map(move |x| (x, y))).filter(|&(x, y)| { let (p, q) = (a.get(x, y), b.get(x, y)); (p.r as i32 - q.r as i32).abs() + (p.g as i32 - q.g as i32).abs() + (p.b as i32 - q.b as i32).abs() > 24 }).count();
            assert!(n as f32 >= 0.15 * (374.0 * 52.0), "{} vs {}: {:.1}%", ids[i], ids[j], 100.0 * n as f32 / (374.0 * 52.0));
        } }
    }
```

`ALL_STRINGS()` is a `#[cfg(test)] fn` returning every `&'static str` the family can draw: all five `CMDS` lists, all output lines, `STATUS_LEFT`, `STATUS_RIGHT`, `TRACE` lines, `EXIT 101`, `clear`, `# bpm ~`.

- [ ] **Step 3: Run to verify failure** - `cargo test term 2>&1 | tail -5` - compile errors.

- [ ] **Step 4: Implement `term.rs`**

Constants: `ROW_PITCH: i32 = 4`, `MAX_ROWS: usize = 12`, `STATUS_H: i32 = 7`, `STATUS_MIN_H: i32 = 48`, `LINE_NO_W: i32 = 8` (two digits + gap), `PROMPT_W: i32 = 4 + 1` (`>` + gap), `CURSOR_HZ: f32 = 1.0`, `CURSOR_DUTY: f32 = 0.6`, `EXEC_BASS: f32 = 0.55`, `FLOURISH_BASS_MIN: f32 = 0.6`, `PANIC_MS: f32 = 700.0`, `TRACE_SCROLL_MS: f32 = 60.0`, `OUTPUT_LINE_STEPS: u32 = 2`, `ONSET_RATIO`/`ONSET_REFRACTORY_MS` copied from `sesh.rs`, `BPM_MIN/MAX = 60/200`.

```rust
struct Style {
    rows: &'static [&'static str],      // row colour hexes, cycled bass->treble
    cmds: &'static [&'static str],
    outputs: &'static [&'static str],   // one fake output line per command, same length as cmds
    prompt_hex: &'static str,           // the ">" colour (pink everywhere except matrix = hot)
}
fn style(t: &Theme) -> Style { match t.id.as_str() { "term-2077-cyan" => .., "term-2077-hot" => .., "term-2077-matrix" => .., "term-2077-editor" => .., _ => TERM_2077 } }

pub struct Term {
    pub(crate) flourish: Trigger,
    panic: Envelope,
    onset: crate::dsp::onset::Flux,
    fill: [f32; MAX_ROWS], peak: [f32; MAX_ROWS],
    line_base: u32,          // scrolls on execute
    cmd_idx: usize, typed: usize,
    output_steps_left: u32,  // fake output line visible for this many scroll steps
    output_idx: usize,
    blink_ms: f32,
    onset_gaps: [f32; 8], gap_head: usize, since_onset_ms: f32, bpm: u16, bpm_refresh_ms: f32,
    trace_row_offset: usize, trace_scroll_ms: f32,
    layout: Layout,          // rows: [(x0,x1,y0,y1); MAX_ROWS], n_rows, status: bool, interior
    rng: u64,
}
```

`draw` order:
1. Panel opaque; interior `ix0=3, iy0=4, ix1=w-3, iy1=h-4`; 1 px border in `edge` inside it.
2. `status = h >= STATUS_MIN_H`; `bottom = if status { iy1 - STATUS_H } else { iy1 }`; scanlines every 3rd row in `lit @ ghost` from `iy0` to `bottom`.
3. Rows: `n = min(MAX_ROWS, (bottom - iy0 - 5) / ROW_PITCH)`; row `r` at `y = iy0 + 1 + r*ROW_PITCH`; the LAST row is the prompt; the row above it (if `n >= 8`) is the comment line; the rest (`m = n - 1 - comment`) are meter rows. Store boxes in `layout` (meter rows' bar area: x from `ix0 + 1 + LINE_NO_W + 1` to `ix1 - 2`).
4. Onsets: `onset = flux.update(...)`; `since_onset_ms += dt`; on onset push the gap into `onset_gaps`, reset; `strong = onset && mean(levels[0..8]) > EXEC_BASS`. Every 1000 ms recompute `bpm = clamp(60000 / median(onset_gaps), 60, 200)`.
5. Meter rows: band fold `bands = 64 / m`, row `i` (0 = top = treble) folds `levels[(m-1-i)*bands ..]`; smooth with `t.ballistics`; `cells = (bar_w) / 4`; `filled = round(fill * cells)`; draw line number (`line_base + i`, two digits, `hot`) then `filled` × `▮` in `style.rows[i % len]`, then `▯` at the peak cell in `hot`. Loudest row: `fill_rect` full-width `#1c1347` behind it first; any row ≥ 0.9: `#310072` instead. If `output_steps_left > 0`, the bottom meter row instead shows `style.outputs[output_idx]` in `lit` (truncated), still with its line number.
6. Comment row (when present): `# bpm ~ NNN` in `#0098df`.
7. Prompt row: `>` in `style.prompt_hex`, then `&cmds[cmd_idx][..typed]` in `lit`, then the cursor block (`▮` in `hot`) if `blink_ms % 1000 < 600`. On `onset`: `typed += 1` (cap at len). On `strong && typed == len`: execute — `line_base += 1`, `output_idx = cmd_idx`, `output_steps_left = 2`, `cmd_idx = (cmd_idx+1) % len`, `typed = 0`; `output_steps_left -= 1` on each later execute.
8. Status bar (when `status`): `fill_rect(ix0, bottom, iw, STATUS_H, #030d22)`; left `~/music`; centre `▮` + rms percent (`[u8; 3]`) + `%`; right `UTF-8  LF` — all `font3x5` in `#4d8bee`, truncated by `clip_label` to their thirds.
9. Flourish: `fired = trigger && bass >= FLOURISH_BASS_MIN` (test bypass via `was_forced()` as sesh); `env = panic.update(fired, dt, PANIC_MS)`. If `fired`: `trace_row_offset = 0`. While `env > 0.25`: every meter row `i` shows `TRACE[(i + trace_row_offset)]` in `#ff2e97` (rows past the end are empty, revealing the meter rows underneath — draw the meter first, then overpaint rows still holding trace lines with a panel-coloured `fill_rect` and the text); `trace_scroll_ms += dt; if > TRACE_SCROLL_MS { trace_row_offset += 1 }`; the cursor is solid; the status bar is `#ee1682` with `EXIT 101` centre. `0 < env <= 0.25`: prompt shows `clear`, typed = full, cursor solid.
10. `clip_to_rounded_rect`.

`TRACE: [&str; 6] = ["thread 'main' panicked at src/dsp/bands.rs:64:9:", "index out of bounds: the len is 64", "note: run with RUST_BACKTRACE=1", "   0: taskbar_eq::render::term::draw", "   1: taskbar_eq::render::frame", "   2: std::rt::lang_start"]` — check every char has a glyph (`'` and `:` and `_` — add `'` and `_` to font3x5 if missing).

Colourways in `builtin.rs` — `term_base()`: `family "term"`, `Texture::None_`, `panel_alpha 1.0`, `bloom 0.0`, `glow_strength 0.0`, `edge_alpha 1.0`, `ballistics { attack: 0.8, decay: 0.3, peak_fall: 0.02 }` (snappy: a terminal repaints instantly), `flourish` like `sesh_base`; the five per the spec table with `ghost` alphas 0.10/0.10/0.10/0.14/0.08 and names "Term 2077", "Term Cyan", "Term Hot", "Term Matrix", "Term Editor".

- [ ] **Step 5: Run** - `cargo test term`, `cargo test font3x5`, `cargo test the_readme` (after README/docs edits), `cargo test three_to_one`, `cargo test every_colourway_is_visibly_alive`, `cargo clippy --all-targets -- -D warnings`.

- [ ] **Step 6: Dumps and look** - `dump_term` (five colourways × calm / loud / panic-peak / panic-decay at 380x60; `term-2077` at 190x48 and 128x44) and `probe_term_cost` (steady + `force_next()` window). READ: 2077-loud, cyan-loud, hot-loud, matrix-calm, editor-loud, 2077-panic-peak, 2077-128x44. Judge: reads as a terminal (prompt, cursor, line numbers, status) AND a meter (bars grow with bass at the bottom); the five sessions are visibly different commands; the trace reads as a Rust panic then scrolls away; nothing outside the panel. Adjust and re-dump until right.

- [ ] **Step 7: Review sheet** - `<section>` 20 mirroring 18/19: figures for the seven PNGs, captions naming the session type per colourway, a `judge` line: does it read as YOUR terminal; are the bars a meter at a glance; is the panic funny or annoying; which sessions to keep.

- [ ] **Step 8: Commit** - `git add -A && git commit -m "term: a 28th family - the VSCode 2077 theme as a live terminal, five sessions"`

---

### Task 2: Timing, docs, release prep (v0.3.1)

**Files:**
- Modify: `src/render/mod.rs` (`slow_vs_timing` ids += `"term-2077"`, with the `< 1.0` assert extended to it), `docs/themes.md` (counts 180 / 28 both places), `CHANGELOG.md` (rename `## [Unreleased]` to `## [0.3.1] — <today>` and add the `term` line), `Cargo.toml` (0.3.1), `TODO.md` ("Last updated" = v0.3.1; the terminal item leaves the In-progress queue; "Waiting on you" first bullet adds section 20 with the eye questions from Task 1's judge line)

- [ ] **Step 1: Timing** - add `"term-2077"` to `slow_vs_timing`'s id array and to the `< 1.0` set; `cargo test --release slow_vs_timing -- --ignored --nocapture 2>&1 | tail -8` - all six under 2.0, the new one under 1.0. Do not loosen the gate.
- [ ] **Step 2: Docs** - as listed. Grep the tree for any stale "175 colourways" / "27 families" outside history lines.
- [ ] **Step 3: Full suite and release build (no tag/push/release — the controller does those)** - `cargo test 2>&1 | tail -3`; `cargo clippy --all-targets -- -D warnings 2>&1 | tail -2`; PowerShell release build with `$env:RUSTFLAGS = "--remap-path-prefix=$PWD=. --remap-path-prefix=$env:USERPROFILE\.cargo=~cargo"`; SHA-256; `cwisset` count (0). Commit `git commit -am "release: v0.3.1 - term family; sesh colourways and slam"`.

---

## Self-review notes

- Spec coverage: palette + Style → T1 step 4; rows/prompt/comment/status/scanlines → steps 2-8; per-colourway sessions + output lines → Style; flourish → step 9; tests → step 2 (all seven spec tests plus `cursor_blinks_without_onsets` and the five-way difference test); review focus 1-5 → named tests; performance → probe; docs/version → T2.
- Type consistency: `row_box_for_test`, `rows_for_test`, `typed_len_for_test`, `line_base_for_test` are declared in Interfaces and used in the tests as written; `ALL_STRINGS()` is `#[cfg(test)]`; `font3x5::glyph` is `pub`.
- Risk: the `Flux` onset detector needs a level rise — the typing test alternates loud/quiet frames for that reason; if `Flux`'s refractory (copied from sesh) is longer than 6 frames (100 ms) the test's cadence must slow to match — the implementer checks `ONSET_REFRACTORY_MS` and adjusts `k % 6` accordingly, saying so in the report.
