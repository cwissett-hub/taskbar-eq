# Fidelity Pass 2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give `rave` a room, `dolphin` a hero leap, `vswings` full-width presence, and rethink `vsghost` as a big glitched kaomoji face; ship as v0.4.1.

**Architecture:** In-place changes to four existing family files; no new families, ids or schema. Preallocated pools for particles (crowd arms, droplets, sparks). Patterns: `src/render/brutal.rs` / `mesh.rs` (pools, fidelity work from batch 1), `src/render/bling.rs` (2x font3x5 text), `src/render/nos.rs` (baked layers keyed on size).

**Tech Stack:** Rust 2021, rustc 1.96, clippy `-D warnings`.

**Spec:** `docs/superpowers/specs/2026-10-01-fidelity-pass-2-design.md` — each family section is that task's requirements.

## Global Constraints

The spec's Shared rules verbatim (opaque panel, allocation-free draw, no double smoothing, no bloom on opaque panels, < 1 ms steady / < 1.5 ms flourish, 3:1 contrast, liveness at 128x44, paint-over-panel tests, break-the-feature proofs, dumps READ, review sections 27-30 with BEFORE/AFTER). One commit per task. No colourway count change (still 197 / 32).

## Review Focus

1. 128x44 per family (spec 1) — each family's narrow test.
2. Pools never allocate — inspection + probe.
3. vsghost test migration (spec 3) — the report lists every changed/replaced test with its old and new intent.
4. Performance — `probe_rave_cost`, `probe_dolphin_cost` (add if missing), `probe_vswings_cost`, `probe_vsghost_cost`; `slow_vs_timing` intact.
5. The Virtual Self families still beat at 160 BPM — `slow_vs_high_bpm_response` still passes after T3/T4.

---

### Task 1: `rave` — haze, cones, truss, crowd, floor splash, moving-head sweep
**Files:** `src/render/rave.rs`, `docs/review/index.html` (section 27), review PNGs.
- [ ] Tests: `haze_drifts` (two frames 1 s apart differ in the haze band at silence), `beams_are_cones_in_the_haze` (pixels flanking a beam line are lit at reduced alpha), `the_crowd_raises_arms_on_strong_onsets` (forced strong onset: arm-stroke pixels above the crowd line; gone after the fade), `the_truss_hangs_the_heads` (truss pixels in the top rows at rest). Break-the-feature proofs.
- [ ] Implement per spec; dumps (calm/loud/flourish x 2 colourways + 128x44) READ; section 27 BEFORE (rave-emitters.png) / AFTER. Commit `rave: a room for the lasers - haze, cones, a truss and a crowd - fidelity pass 2`.

### Task 2: `dolphin` — 2x hero, leap arc, splash, sun/moon, crests
**Files:** `src/render/dolphin.rs`, section 28, PNGs.
- [ ] Tests: `the_leap_height_follows_the_bass`, `re_entry_throws_a_splash` (droplet pixels above the sea line within 300 ms of re-entry), `the_sky_has_a_sun_and_its_reflection`, `crests_travel` (crest dots shift between frames). Proofs.
- [ ] Implement; dumps READ; section 28 BEFORE (dolphin-arc.png) / AFTER. Commit `dolphin: a hero leap with a splash, a sun on the sea and travelling crests - fidelity pass 2`.

### Task 3: `vswings` — 80 % span, echo wings, edge-to-edge floor, sparks
**Files:** `src/render/vswings.rs`, section 29, PNGs.
- [ ] Tests: `the_wings_span_most_of_the_panel` (painted wing pixels exist beyond 15 % and 85 % of the width at loud), `an_echo_pair_lags_behind` (after a step down in level, the echo layer's extent > the main layer's for ~100 ms), `the_floor_reaches_both_edges`, `sparks_fly_toward_the_sides_on_strong_onsets`. Existing tests keep shapes. `slow_vs_high_bpm_response` still passes. Proofs.
- [ ] Implement; dumps READ; section 29 BEFORE (vswings-loud.png) / AFTER. Commit `vswings: wings across the whole panel, an echo pair and sparks - fidelity pass 2`.

### Task 4: `vsghost` — the glitched face
**Files:** `src/render/vsghost.rs`, section 30, PNGs.
- [ ] Tests: `the_face_is_large_and_centred` (2x glyph pixels within the centre 60 % at 380x60), `the_eyes_open_with_the_bass` (eye glyph changes between bass 0.1 and 0.9), `the_mouth_opens_with_the_mids`, `slices_glitch_with_the_bass` (row offsets non-zero at bass 0.9, zero at 0.0), `the_ticker_bars_are_the_meter` (bass-only: left bars tall, right short). Migrate the existing tests per Review Focus 3. Kaomoji chars must have glyphs (`(`, `)`, `^`, `_`, `>`, `<`, `o`, `O`, `-`). `slow_vs_high_bpm_response` still passes (if the depth metric falls below 0.35 for vsghost because the meter is smaller, report the numbers and adjust the metric's region to the ticker, never the threshold).
- [ ] Implement; dumps READ; section 30 BEFORE (vsghost-cobalt-loud.png) / AFTER. Commit `vsghost: rethought as a glitched kaomoji face over a ticker - fidelity pass 2`.

### Task 5: Release prep (v0.4.1)
- [ ] CHANGELOG `## [0.4.1] — <date>` (the four families); Cargo 0.4.1; TODO ("Last updated" v0.4.1; batch 2 leaves the queue; queue: recapture the VS fixture, --levels; Waiting on you: judge sections 23-30); `cargo test`, clippy, `cargo test --release slow_ -- --ignored`, release build with the remap RUSTFLAGS (stop/relaunch the running app), SHA, cwisset 0. Commit `release: v0.4.1 - fidelity pass 2`.
