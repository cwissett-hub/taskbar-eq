# `bling`, `nos`, `drift`, `prism` + Now-Playing Button Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Four new render families (Blingee, two Fast & Furious, the user's prism image) and a way to see what's playing on demand; ship as v0.4.0.

**Architecture:** One file per family under `src/render/`, registered in `render/mod.rs` (`KNOWN_FAMILIES` 28 -> 32), labelled in `themes/mod.rs::family_label`, colourways in `themes/builtin.rs`. Pattern references: `src/render/sesh.rs` (per-id `Style`, bass-gated flourish with `was_forced()`, `font3x5`, baked backgrounds, test helpers `theme/frames/drew_over_panel/lit`, `dump_sesh`, `probe_sesh_cost`), `src/render/term.rs` (text, `[u8;N]` formatting, `clip_label`), `src/render/brutal.rs` / `mesh.rs` (preallocated particle pools). The now-playing button touches `src/win/tray.rs`, `src/win/hotkeys.rs`, `src/win/overlay.rs` and `src/main.rs`.

**Tech Stack:** Rust 2021, rustc 1.96, clippy `-D warnings` gate.

**Spec:** `docs/superpowers/specs/2026-09-30-bling-nos-drift-prism-design.md` — each family section (with its colourway table and tests) IS that task's requirements. This plan sequences them and adds Task 5 (the button), which the spec does not cover.

## Global Constraints

- The spec's Shared rules verbatim — in particular: opaque panel first / clip last; allocation-free `draw`; **no double smoothing** (never re-apply `t.ballistics` attack/decay in `draw`; `FrameData.levels` is already smoothed in main.rs); no `bloom` on opaque panels; bass-gated flourishes; no logos/badges/wordmarks; `font3x5` text with a glyph for every char.
- Counts after each family task: T1 185/29, T2 189/30, T3 193/31, T4 197/32 (README `**N colourways across M families**` + the label in its family list; `docs/themes.md` counts in both places + one line + ids). `KNOWN_FAMILIES` length bumped each time.
- Every new test is shown non-vacuous by breaking its feature and watching it fail (this codebase repeatedly finds vacuous tests — expect some).
- One commit per task; `cargo test` green (the `rave` timing test is a known load flake — re-run alone); clippy clean; the dump PNGs READ with the Read tool before committing; review-sheet section per family (23 bling, 24 nos, 25 drift, 26 prism).
- Performance: `probe_<family>_cost` steady < 1 ms, flourish < 1.5 ms at 380x60 release.

## Review Focus

1. 128x44 / 190x48 per family (spec Review focus 1) — `fits_the_narrow_panel_and_flourish_does_not_panic` in each task.
2. Silence (spec 2) — `rest_frame_is_not_empty` in each task.
3. `DANGER TO MANIFOLD` two-line fallback at narrow widths — T2 adds `manifold_warning_falls_back_to_two_lines` (190x48: both words' pixels present, no glyph past the interior).
4. No double smoothing — each task's report states `grep -n "ballistics\.\(attack\|decay\)" src/render/<family>.rs` returns nothing.
5. Now-playing with nothing playing: tray line reads `♪ (nothing playing)` and is disabled; hotkey/click shows nothing (no empty banner). — T5 tests.

---

### Task 1: `bling`

**Files:** Create `src/render/bling.rs`; Modify `src/render/mod.rs`, `src/themes/mod.rs`, `src/themes/builtin.rs`, `README.md`, `docs/themes.md`, `docs/review/index.html`.
**Interfaces:** Produces `pub struct Bling` (`Default`, `id() == "bling"`), ids `bling-pink bling-gold bling-ice bling-myspace`, label `"Blingee: bling bling"`, `pub(crate) flourish: Trigger`.

- [ ] Step 1: write the spec's `bling` tests plus the four shared ones (copy helpers from `sesh.rs`); run — compile failure (RED).
- [ ] Step 2: implement per the spec's `bling` section (glitter panel, 24 gem columns, glint, peak gem, 2x outlined glitter phrase, three stamps, flash flourish). Add any missing `font3x5` glyphs (`~`, `*`, `$` if absent) with its coverage test extended.
- [ ] Step 3: `cargo test bling font3x5 the_readme three_to_one every_colourway_is_visibly_alive`, clippy; break-the-feature proofs.
- [ ] Step 4: `dump_bling` (calm/loud/flourish x 4 colourways, pink at 190x48 and 128x44) — READ at least pink-loud, gold-loud, myspace-calm, ice-flourish, pink-128x44. Bar: reads as a Blingee GIF (glitter everywhere, sparkly text, stamps) AND as a meter (gem columns rise with the spectrum). Iterate. Section 23.
- [ ] Step 5: commit `bling: a 29th family - a Blingee GIF with rhinestone bars, four colourways`.

### Task 2: `nos`

**Files:** Create `src/render/nos.rs`; same Modify list.
**Interfaces:** `pub struct Nos`, ids `nos-2fast nos-original nos-quarter nos-miami`, label `"Fast & Furious: NOS"`; each colourway registers one `Zone { upto: 1.0, lit: <under-glow>, hot: <under-glow> }` and the family reads `zones.first()`.

- [ ] Steps 1-5 as Task 1 with the spec's `nos` section and tests, plus `manifold_warning_falls_back_to_two_lines`. Eye bar: an in-car dash (carbon weave, LED tacho arc, shift lights, gear/MPH, under-glow) and the NOS hit's green `DANGER TO MANIFOLD` screen then the purge. Section 24. Commit `nos: a 30th family - a 2 Fast 2 Furious dash with the DANGER TO MANIFOLD hit, four colourways`.

### Task 3: `drift`

**Files:** Create `src/render/drift.rs`; same Modify list.
**Interfaces:** `pub struct Drift`, ids `drift-shibuya drift-touge drift-orange drift-night`, label `"Fast & Furious: Tokyo Drift"`; `zones[0].lit` = neon.

- [ ] Steps 1-5 with the spec's `drift` section and tests. The car silhouette (three 14x6 bitmaps) is our own — a generic sports coupe profile, no identifiable model. Eye bar: a night skyline with neon, a car drifting whose smoke trail billows with the spectrum, the whip-across flourish with `DRIFT!`. Section 25. Commit `drift: a 31st family - Tokyo Drift, the tyre smoke is the meter, four colourways`.

### Task 4: `prism`

**Files:** Create `src/render/prism.rs`; same Modify list.
**Interfaces:** `pub struct Prism`, ids `prism-sunset prism-aurora prism-mono prism-dawn`, label `"Prism: light bloom"`; the spectral stops per colourway are constants in the family matched on `t.id` (the `sesh` `Style` pattern; default = sunset); `lit` = brightest stop.

- [ ] Steps 1-5 with the spec's `prism` section and tests. Eye bar: compare against the user's reference — a luminous soft-edged rainbow band arcing over a dark rim-lit horizon on a plum sky — AND a meter (the band swells where the music is). The glow comes from layered alpha columns, not `bloom`. Section 26. Commit `prism: a 32nd family - a spectral arc over a dark horizon, four colourways`.

### Task 5: See what's playing — tray line, hotkey, single click (runs FIRST)

**Files:** Modify `src/win/tray.rs`, `src/win/hotkeys.rs`, `src/win/overlay.rs`, `src/main.rs`, `src/config.rs` (if slot names live there), README "Using it".

**Behaviour:**
- **Tray:** the first line of the context menu is `♪ <title>` (from `win::media::now_playing().0`, truncated to 48 chars with `...`), enabled when a title exists, disabled and reading `♪ (nothing playing)` otherwise. Clicking it re-shows the track banner (`render::banner::Banner::new(&title, r.h - 4)` — the same path main.rs uses on a track change) for the banner's normal duration.
- **Hotkey:** a new slot `"Show now playing"` appended to `hotkeys::SLOTS` (unbound by default), wired like `ID_IDENTIFY_NOW`: pressing it shows the banner. Earlier notes warn that the slot/menu arrays have had hard-coded lengths (a menu array once panicked at the sixth slot; the tray once hard-coded slot index 7) — grep every `SLOTS`-sized array and every literal slot index and make them follow `SLOTS`. Config: an old config without the new key must load (serde default).
- **Single click (user, 30 Sep 2026: "remove the click-through behaviour, just have a single click show the song"):** `WM_LBUTTONUP` on the overlay (overlay.rs:~362 -> `OverlayEvent::LeftClick`) currently opens the Windows Widgets panel via `win::overlay::open_widgets_panel()` (main.rs:~941-945). REMOVE that: a left click now triggers the same "show now playing" action. Delete `open_widgets_panel` and its `SendInput` helper if nothing else calls them (clippy `-D warnings` will flag dead code); update README/docs wherever they describe click-to-open-Widgets. Right click still opens the menu. No double-click handling.
- Nothing playing: hotkey and click do nothing (no empty banner).

- [ ] Step 1: tests (pure logic only — no GUI automation, per the user's rule): `now_playing_menu_label` (a pure fn: title -> label; empty -> disabled `(nothing playing)`; 80-char title -> truncated with `...`); `slots_include_show_now_playing` and every SLOTS-sized array test the existing suite has; config back-compat (a v0.3.2 config string loads with the new slot unbound). RED, then implement, GREEN.
- [ ] Step 2: build and run the app yourself briefly ONLY to confirm it starts and the log shows hotkeys bound (do not drive the GUI; the user tests it). Stop it again.
- [ ] Step 3: README "Using it": document the three ways. Commit `now playing: a tray line, a hotkey and a double-click all show the current song`.

### Task 6: Timing, docs, release prep (v0.4.0)

**Files:** `src/render/mod.rs` (`slow_vs_timing` += `bling-pink`, `nos-2fast`, `drift-shibuya`, `prism-sunset`, all in the `< 1.0` set), `CHANGELOG.md` (`## [0.4.0] — <date>`: the four families, the now-playing button), `Cargo.toml` 0.4.0, `TODO.md` ("Last updated" v0.4.0; queue: batch 2 of the fidelity pass — rave, dolphin, vswings sides, vsghost rethink — then recapture the VS fixture, then --levels; Waiting on you: judge sections 23-26 + try the now-playing button).

- [ ] Steps: timing run (gate not loosened); docs; `cargo test`; clippy; PowerShell release build with the remap RUSTFLAGS (stop a running instance if it locks the exe; do not relaunch); SHA; cwisset 0. Commit `release: v0.4.0 - bling, nos, drift, prism and a now-playing button`.

---

## Self-review notes

Spec coverage: families 1-4 -> T1-T4 (each task's requirements are the spec section, which carries every number and test); shared rules -> Global Constraints; review focus -> named tests + T5 nothing-playing rule; the button (user request after the spec) -> T5; version -> T6. Types: counts/ids/labels listed per task; `zones[0].lit` used by nos and drift as in night/term. Risk: four eye-judged families — the dump-and-read step is mandatory; T5 touches shared window/hotkey code — the SLOTS-literal grep is called out because of two prior bugs there.
