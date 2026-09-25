# Health fixes and the Virtual Self families - design

**Date:** 2026-09-25
**Status:** approved in conversation ("I agree with all ... fix in the order you think best then
develop a theme based around the aesthetic of Virtual Self"; then "build all three, I'd need to see
all in action to judge").

## Intent

Two deliverables, in this order.

**A. Health fixes** from the 25 Sep 2026 whole-codebase review (three reviewers: core/Windows,
DSP/render/themes, product/repo). Everything the user agreed to, minus anything that exists only to
serve a light Windows taskbar ("I don't like Windows light themes so we won't worry about that").

**B. Three new render families** in the aesthetic of Virtual Self (Porter Robinson's Y2K / late-90s
trance alias: Particle Arts website, Win98 chrome bevels, wireframe VRML grids, angel wings, lens
flares, chrome gradient text, kaomoji, harsh white glitch). Built as three separate families so the
user can judge them side by side in the review sheet and keep, tune or drop each.

Audience: the user and a couple of friends; the repo is public and GPL-3.

## Part A - health fixes

Ordered by the user's benefit. Each item names the acceptance test that proves it.

### A1. "Use suggested keys" no longer kills the other hotkeys
`win::hotkeys::Registry::apply_all` builds the duplicate-check list from `self.live` INCLUDING the
slot being re-applied, so an unchanged chord is refused as a duplicate of itself. Fix: exclude the
current slot's own live chord from `others`. Test: `apply_all` twice with the same texts via a fake
`Registrar` leaves every slot `Registered`; a real duplicate between two different slots is still
`Refused`.

### A2. Config integrity
- `Config::save` writes `config.toml.tmp` then renames over `config.toml` (atomic on NTFS).
- The load fallback logs through `log::write`, not `eprintln!`.
- `media_backend` deserialises leniently: an unknown string logs and falls back to `Session`
  without failing the document (custom `Deserialize` or `#[serde(deserialize_with)]`).
- Tests never touch the real `%APPDATA%`: `Config::dir()` honours `TASKBAR_EQ_CONFIG_DIR` when set,
  and every config test sets it to a per-test temp dir.
Tests: round-trip through save/load in a temp dir; a config with `media_backend = "nonsense"` loads
with theme and width intact; a `.tmp` left behind by a simulated crash does not shadow the real file.

### A3. Kaleido quiet floor
`render/kaleido.rs` `LEVEL_FLOOR = 0.119` with no minimum geometry draws a black slab on quiet
material. Fix: a minimum rosette radius / minimum lit fraction so the panel always shows structure
above the gate threshold. Test: at every colourway and plausible panel height, a frame at level
0.12 has at least N lit pixels (N chosen from the fix, e.g. 5% of the panel) and 2 distinct colours.

### A4. Log-spaced bass
`dsp::bands` uses one 2048-point FFT at 48 kHz (23 Hz bins); log band edges from `F_LOW` collapse
and the `last + 1` guard makes bands 0..~32 consecutive single bins - linear, not log. Fix: a
second analysis path for the low bands: an 8192-sample window (the ring already holds audio; keep
a longer low-rate history) analysed with its own FFT for bands whose edge spacing would otherwise
be under one bin, blended at the crossover. `NUM_BANDS` stays 64 and `Frame` is unchanged so no
family changes. Test: a 60 Hz tone and a 120 Hz tone peak at least 6 bands apart; a 1 kHz tone and
2 kHz tone still peak where they did (regression against the existing fixture CSVs in
`tests/fixtures/real-music-*.csv`, tolerance documented in the test).

### A5. Ballistics per millisecond
`dsp::ballistics::Smoother::update` applies attack/decay/peak-fall once per tick. Make it take
`dt_ms` and convert each rate `r` to `1 - (1 - r).powf(dt_ms / 16.667)`, so a 31 ms frame decays
the same distance as two 16 ms frames. Test: two updates at 16.667 ms equal one at 33.333 ms
within 1e-4.

### A6. Capture device poll and stress rows
The capture loop calls `GetDefaultAudioEndpoint` + `GetId` (two cross-process COM calls) every
packet. Poll at most once per second (compare against `Instant`). Add two rows to `--stress`:
capture steady state, and a forced reopen loop (start/stop `capture_loop` N times), reporting
handle and thread deltas like the existing rows. Test: unit test for the throttle helper (a pure
`should_poll(last: Instant, now: Instant) -> bool`); the stress rows are manual.

### A7. Linear-light blending and OKLCH rainbow
In `render/canvas.rs`: `blend_over`, `sample_stops`, the `lerp`/`mix` helpers, and `bloom` blend in
linear light via a 256-entry sRGB->linear LUT and a linear->sRGB encode (LUT or `powf`). `tint` /
`themes::rainbow_hsv` produce OKLCH (`Rgba::from_oklch` exists) with chroma clipped by
`oklch_max_chroma`, so rainbow yellows and cyans stop blowing out and blues stop going dark. The
family-facing API is unchanged. Tests: blending 50% white over black gives ~188 (not 128) in sRGB;
a two-stop gradient's midpoint luminance is the mean of the endpoints in linear light; the ASCII
goldens in `tests/golden/` are regenerated ONCE with a commit message that says why, after a visual
check via the dump tests; the rainbow test asserts every hue step has the same OKLab L within 0.02.

### A8. Tests and lint
- `cargo clippy --all-targets` compiles and is warning-free (`#[allow]` with a reason where a lint
  is wrong for this code, otherwise fix).
- `shell_state` tests: the vacuous default test asserts the module's statics after a reset helper;
  the racy publish test and its siblings share a `static SERIAL: Mutex<()>`.
- `tray` tests that create real tray icons are `#[ignore]`d with a doc comment saying why.
- The five >60 s render sweeps (`every_colourway_renders_at_every_plausible_overlay_size`, three
  `fluid` sweeps, `no_family_leaves_a_transparent_pixel_inside_its_panel`) become `#[ignore]` with
  a `slow_` prefix; a `cargo test slow_ -- --ignored` line goes in README "Build from source" and
  `TODO.md`. A cheap replacement stays in the default suite: every colourway at THREE sizes
  (190x48, 380x48, 380x60) asserting >= 2% lit pixels and >= 2 distinct colours at level 0.15 and
  0.6 - the structural guard that would have caught pipes shipping black.
- `hotkeys`: `apply_all` test (A1); `capture`: reset-frame-on-reopen is out of scope (needs a
  device).

### A9. CLI and binary hygiene
- `--help` prints usage for `--console`, `--diagnose`, `--levels`, `--stress`, `--version`, and
  exits; `--version` prints `taskbar-eq <CARGO_PKG_VERSION>`. Unknown `--flags` print help and exit
  1 rather than silently launching the overlay.
- `.cargo/config.toml` with `[build] rustflags = ["--remap-path-prefix=<abs repo>=."]` is NOT
  portable; instead `build.rs` is left alone and `Cargo.toml` gains `[profile.release] ... ` plus a
  documented `cargo rustc` line? No - simplest portable fix: `[profile.release] debug = false,
  strip = true` is already set; add `panic = "abort"` (drops the unwinding tables that carry paths)
  and verify with `grep -c cwisset target/release/taskbar-eq.exe` == 0. If paths survive, add
  `RUSTFLAGS=--remap-path-prefix` to the README build line and to the CI job instead.
- `rust-version = "1.85"` (or whatever `cargo msrv`-free inspection of used features justifies;
  default to the toolchain that built it minus nothing: record the current stable).
- `main.rs`: the three CLI probes (`diagnose`, `measure_levels`, `stress` and their helpers) move
  to `src/probes.rs`. Behaviour unchanged.

### A10. Docs, privacy, release
- README gains "What leaves your machine" under Song identification: the Shazam fingerprint (not
  audio) is posted to amp.shazam.com only when you press the key or menu item; a fixed fake
  location and an Android user-agent are sent (as SongRec does); the log records identified titles;
  `songs.jsonl` is local only.
- README shrinks to Install / Using it / Song identification / Configuration / Build from source /
  Licence, with a screenshot at the top (a dump PNG of one family composited on a taskbar strip;
  `docs/screenshot.png`). `## Status`, `## Known gaps`, the theme catalogue and the
  "generate more themes" prompt move to `docs/status.md`, `docs/known-gaps.md`, `docs/themes.md`,
  `docs/theme-prompt.md`, with links from README. Stale numbers corrected where they move.
- `HANDOVER.md`: keep only "Things that cost real time" as `docs/lessons.md`; delete the rest.
- `TODO.md`: one `## In progress`, `## Waiting on you` trimmed to open judgement calls, ticked items
  moved to a new `CHANGELOG.md` (Keep a Changelog format; v0.2.0 = song identification, v0.2.1 =
  this work).
- `docs/theme-backlog.md`: collapse done items to a ledger line; move the one open idea to TODO.
- Release: bump to 0.2.1, `git tag v0.2.1`, `gh release create v0.2.1 target/release/taskbar-eq.exe
  --notes-file <from CHANGELOG>` with the SHA-256 in the notes; README download link points at
  `https://github.com/cwissett-hub/taskbar-eq/releases/latest/download/taskbar-eq.exe`; `dist/` is
  removed from the tree and added to `.gitignore` (history is NOT rewritten).
- Minimal CI: `.github/workflows/ci.yml` on `windows-latest`: `cargo build --release`,
  `cargo test`, `cargo clippy --all-targets -- -D warnings`; on a `v*` tag, upload the exe to the
  release. Best effort: if the org's GitHub blocks Actions, commit the file anyway and note it.

### Out of scope for Part A
Light-taskbar panels; Windows accent-follow colourways; cover-art palettes; time-of-day drift;
generic `[palette]` table (the tube-table hijack is documented in `docs/themes.md` instead);
rewriting git history to drop old exes; the full main.rs `App` refactor beyond moving the probes;
secondary-monitor overlays.

## Part B - the Virtual Self families

Three families, each a normal `render::Family` registered in `render/mod.rs`, with colourways in
`themes/builtin.rs`, a flourish (every family has one - see `dsp::flourish::request`), the
standard `dump_` ignored test writing PNGs to `target/eyeball/`, and a section each in
`docs/review/index.html` telling the user what to judge. Panel: 380x48 primary, must survive
190x48 and 380x60 (the A8 structural guard applies).

Shared palette vocabulary (colourways pick from it): ice `#dff3ff`, white `#ffffff`, silver bevel
light `#f4f6fa` / dark `#8a94a6`, cobalt `#1f5bff`, electric `#3ec8ff`, black `#000000`, pale pink
`#ffd6ec`, violet `#8e6bff`. Chrome text = vertical gradient white -> silver -> cobalt -> white with
a 1 px dark outline. Panels are dark or ice; no light-taskbar logic.

### B1. `vswings` - angel wings on Y2K chrome
- **Meter:** 32 bands mirrored into two fanned wings meeting at the centre; each feather is a bar
  rotated by its index (fan from -70deg to +70deg), length = level, with a white tip and a peak-hold
  dot. Low bands at the wing root (centre), highs at the tips, so bass makes the wings beat.
- **Ground:** a receding wireframe grid (single-point perspective, cobalt lines on the panel) that
  scrolls with time and jumps on onsets.
- **Chrome:** the panel has a 2 px Win98 bevel (light top-left, dark bottom-right) and a chrome
  gradient title glyph row ("VIRTUAL SELF" in the existing `text_3x5` style, chrome-tinted) when
  the panel is >= 60 tall; hidden at 48.
- **Flourish:** a lens flare (bright core, four streaks, ring) bursts from the wing root and fades
  over 600 ms.
- **Colourways (5):** `vswings-particle-arts` (ice panel, white wings, cobalt grid),
  `vswings-eon-break` (black panel, cobalt/electric wings, white grid), `vswings-angel-voices`
  (pale pink panel, white/violet wings), `vswings-utopia` (black panel, chrome-gradient wings),
  `vswings-ghost` (black panel, pure white wings, no grid).

### B2. `vsghost` - Ghost Voices glitch terminal
- **Meter:** a piano-roll: 48 thin white ticks per band column arranged as a MIDI grid; a band's
  level lights ticks upward; peak ticks stay lit and decay. Bands scroll left one column per onset
  so the panel becomes a rolling score.
- **Text:** a kaomoji `( ˘ω˘ )` / `(´• ω •`)` glyph set and short romanised strings
  ("ghost voices", "eon break", "a.i.ngel") rendered in `text_3x5`, one phrase visible at a time,
  swapping on strong onsets. No Japanese glyphs (the 3x5 font cannot draw them); the review sheet
  says so.
- **Rays:** two to four translucent blue-white light rays from the bottom edge whose angle drifts
  with the spectral centroid and whose intensity follows RMS.
- **Flourish:** a datamosh - 3 to 5 horizontal slices of the frame shift by random offsets and
  invert for 4 frames, with a hard white strobe on the first frame.
- **Colourways (4):** `vsghost-white` (black/white), `vsghost-cobalt` (black/cobalt/electric),
  `vsghost-inverse` (white panel, black ticks), `vsghost-violet` (black/violet/pink).

### B3. `vsorb` - low-poly chrome orb
- **Meter:** a wireframe icosphere (subdivided icosahedron, ~80 edges) at the panel centre, radius
  = bass energy (bands 0-8 mean) between 40% and 95% of the panel height; 24 spectrum spikes
  radiate from the rim, one per pair of bands, length = level. The spikes are the readable meter,
  the orb is the bass.
- **Shading:** edges shaded chrome by their screen-space normal (a two-band gradient: sky above,
  ground below - the classic chrome map), rotating slowly with time and faster on onsets.
- **Background:** a vertical Y2K gradient (cobalt -> ice) with three soft lens-flare discs drifting.
- **Flourish:** the orb shatters into its triangles which fly outward and fade over 700 ms while a
  fresh orb grows from zero.
- **Colourways (4):** `vsorb-chrome` (ice gradient, chrome orb, white spikes), `vsorb-eon` (black,
  cobalt/electric), `vsorb-angel` (pink/white), `vsorb-mono` (black, white wireframe, no gradient).

### Shared requirements
- Each family >= 60 fps at 380x60 in release on the dev machine: `draw` < 2 ms measured by an
  ignored timing test, no allocation inside `draw` after warm-up (scratch buffers live in the
  struct).
- Every colourway passes the existing 3:1 contrast rule in `builtin.rs` and the A8 structural
  guard.
- `docs/review/index.html` gets one section per family with the dump PNGs (calm, loud, flourish
  frame) and a "what to judge" line; the user decides which families and colourways stay.
- README `docs/themes.md` lists the three families; `CHANGELOG.md` v0.2.2 = Virtual Self families.

### Out of scope for Part B
Japanese text; audio-reactive typography beyond `text_3x5`; sharing code across the three
families beyond `canvas` helpers (they are meant to be judged and possibly deleted independently).
