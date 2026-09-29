# Fidelity Pass 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Upgrade the rendering of `brutal`, `pipes`, `mesh`, `orbit` per the spec (material, volume, stage, system) and retune the three Virtual Self families for high-BPM music; ship as v0.3.2.

**Architecture:** In-place changes to four existing family files plus three `*_base()` ballistics and three onset constants. No new families, ids or schema. Each family's existing tests keep their shapes; goldens regenerate once per family in that family's commit after the eye test. Preallocated pools for any new particles.

**Tech Stack:** Rust 2021, rustc 1.96, clippy `-D warnings` gate.

**Spec:** `docs/superpowers/specs/2026-09-29-fidelity-pass-1-design.md` — the per-family sections ARE the requirements; this plan sequences them.

## Global Constraints

- Opaque panel first / clip last; allocation-free `draw` (pools preallocated in the struct); < 1 ms steady at 380x60 release (2 ms gate); 3:1 contrast + liveness (128x44); colour mixing in linear light (`Rgba::lerp_linear`); no new dependencies; no `cargo update`.
- Existing tests keep their names and assertion shapes. Golden PNG tests: regenerate ONCE per family, in the same commit, only after the implementer has READ the new dumps.
- One commit per family (or per task), clippy clean, `cargo test` green (the `rave` timing test is a known load flake — re-run alone).
- README count test: colourway count rises by one (`brutal-sodium`): **181 colourways / 28 families**.

## Review Focus

1. Pools (dust, falling ghosts, comet tails) never allocate in `draw` — T1/T2 tests assert `draw` twice on the same struct with no `Canvas::new` inside (inspection + the probe).
2. 128x44 for all four (spec's floors/orbits/fat pipes/slab faces) — each family's existing narrow test plus an eye look.
3. Goldens regenerated exactly once and the diff shows only that family's PNGs.
4. VS retune must not jitter slow music: existing per-family real-music `slow_` tests still pass (T3 runs them).
5. `brutal-sodium` contrast (slabs `#8a8378` on `#0a0804`) and the glow gradient not lowering any face below 3:1.

---

### Task 1: `brutal` + `pipes`

**Files:** Modify `src/render/brutal.rs`, `src/render/pipes.rs`, `src/themes/builtin.rs` (`brutal_sodium()` registered in `all()`), `README.md` (181), `docs/themes.md` (181 + the new id), `docs/review/index.html` (section 21: before/after per family), goldens for both families.

- [ ] Step 1: read the spec's `brutal` and `pipes` sections and both files' existing tests. Write the failing tests: `brutal`: `slabs_have_a_lit_top_and_a_shadow_side` (at level 0.8 the block's top 2 rows are lighter than the front and the right 3 columns darker, in luminance), `a_bass_onset_slams_and_puffs_dust` (frame after a forced onset: slab drawn ≥ 2 px above its settled y; a dust pixel exists above the slab base within 6 px; 300 ms later no dust); `pipes`: `three_pipes_grow_at_once` (after 60 frames at level 0.6, three distinct hues present), `pipes_are_fat_with_a_highlight` (a front-depth segment is ≥ 5 px wide with a row lighter than its body), `teapot_appears_on_the_flourish` (forced flourish → the 9x7 bitmap's pixel pattern found at the tip within 3 frames).
- [ ] Step 2: run to fail. Step 3: implement per spec. Step 4: `cargo test brutal pipes`, contrast, liveness, `the_readme`, clippy.
- [ ] Step 5: dumps — run both families' existing `dump_*`; READ calm/loud/flourish for each (brutal in `brutal-concrete` and `brutal-sodium`; pipes in two colourways) and at 128x44. Judge against the user's words: do they POP; are they cohesive with blossom/vaporwave (glow, depth, material)? Iterate. Then regenerate goldens (`cargo test brutal pipes -- --ignored` per the goldens' documented command) and confirm the golden diff is only these two families.
- [ ] Step 6: review sheet section 21 with BEFORE (the existing `brutal-bright.png`, `pipes-3d.png`) and AFTER figures; judge line. Commit `brutal, pipes: material, volume and a teapot - fidelity pass 1`.

### Task 2: `mesh` + `orbit`

**Files:** Modify `src/render/mesh.rs`, `src/render/orbit.rs`, `docs/review/index.html` (section 22), goldens for both.

- [ ] Step 1: failing tests: `mesh`: `bars_stand_on_a_floor_with_shadows` (grid lines present in the lower half at rest; a darker ellipse row exists directly under a loud bar), `a_dropping_peak_sheds_a_falling_ghost` (drive a bar to 0.9 then 0.2; a `ghost`-alpha block exists below the bar top on the next frames and is gone after 500 ms), `back_rows_are_fogged` (a back-row bar's colour is closer to panel than the front row's, same level); `orbit`: `planets_ride_visible_ellipses_around_a_sun` (rest frame: a `hot` disc at centre; ≥ 3 ring pixels in `edge` on the ellipse path), `comet_tail_length_follows_level` (tail dot count at 0.9 > at 0.2), `alignment_flourish_brings_planets_to_one_angle` (forced flourish; after 400 ms the planets' angles are within 0.1 rad).
- [ ] Steps 2-6 as Task 1 (dumps read, goldens once, section 22 with BEFORE `mesh-3d.png`/`orbit-3d.png`). Commit `mesh, orbit: a floor with shadows and fog; a sun, ellipses and comet tails - fidelity pass 1`.

### Task 3: VS retune, fixture, timing, release prep

**Files:** Modify `src/themes/builtin.rs` (three bases' ballistics), `src/render/vswings.rs`, `vsghost.rs`, `vsorb.rs` (refractory 120 ms), `src/render/mod.rs` (`slow_vs_high_bpm_response`), `tests/fixtures/high-bpm-bands.csv` (new), CHANGELOG (`## [0.3.2]`), Cargo.toml 0.3.2, TODO.md.

- [ ] Step 1: try the live capture: `cargo test live_identify -- --ignored` shows how Spotify is driven; write an `#[ignore] fn capture_high_bpm_fixture` that plays a Virtual Self track via the app's media session for 20 s while recording the 64-band envelope per frame to the CSV (or run `--levels` per its docs). If Spotify/audio is unavailable in the session, synthesise the fixture at 160 BPM (kick every 375 ms, 40 ms decay, bands 0..8 at 0.9, noise floor 0.05) and label the CSV's header line `# synthetic 160bpm`.
- [ ] Step 2: `slow_vs_high_bpm_response` (ignored, `slow_`): for each of the three families, feed the fixture, record per-frame painted-pixel count (over-panel), autocorrelate over lags 200-600 ms, assert the peak lag is within ±10 % of the beat period (375 ms for the synthetic; read from the header for a live capture). Watch it FAIL with the old ballistics (RED), then apply the retune (GREEN).
- [ ] Step 3: run the existing `slow_` per-family real-music tests for the three families (`cargo test --release slow_vs -- --ignored`) to prove no jitter regression; `slow_vs_timing` still passes.
- [ ] Step 4: docs — CHANGELOG 0.3.2 (four families, sodium colourway, VS retune with the fixture provenance), Cargo 0.3.2, TODO ("Last updated" v0.3.2; queue: batch 2 = dolphin, rave, vswings sides, vsghost rethink; then --levels; Waiting-on-you: judge sections 21-22 + whether the VS families now feel right on a real VS track).
- [ ] Step 5: `cargo test`, clippy, release build with the remap RUSTFLAGS (stop a running instance if it locks the exe; do not relaunch), SHA, cwisset 0. Commit `release: v0.3.2 - fidelity pass 1 and the Virtual Self retune`.

---

## Self-review notes

Spec coverage: brutal/pipes → T1, mesh/orbit → T2, VS retune + fixture → T3; review focus 1-5 → T1/T2 tests + probes, narrow tests, golden discipline, T3 slow tests, contrast test. Types: no cross-task interfaces beyond the count (181). Risk: goldens — regenerating is allowed once per family and the diff must be inspected; the live fixture may be impossible in-session (fallback specified).
