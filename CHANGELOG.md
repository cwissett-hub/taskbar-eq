# Changelog

Format loosely follows [Keep a Changelog](https://keepachangelog.com/); dates are release dates,
not commit dates.

## [0.2.1] — 2026-09-28

Health fixes: a pass over correctness, test speed, and release hygiene, with no new families.

- Hotkeys: re-applying a binding unchanged is no longer treated as a duplicate of itself — "Use
  suggested keys" no longer silently killed the *other* keys on the same action.
- Config: atomic save (no more truncated file on a crash mid-write), lenient `media_backend`
  parsing with a log line on fallback instead of a hard failure, and tests moved off the real
  `%APPDATA%` so a test run cannot touch your live settings.
- Kaleidoscope: no more empty slab on quiet material.
- Bands: bass is now log-spaced via a second 8192-point FFT below the crossover band, and the
  long-FFT history is fed fresh samples from capture directly rather than inferring overlap —
  the one algorithmic change in this release, covered by the fixture-CSV regression tests.
- Ballistics: attack, decay and peak-fall are now frame-rate independent (`Smoother::update`
  takes `dt_ms`), so a slow frame no longer makes the meter under- or over-shoot.
- Capture: polls the default device once a second rather than continuously, and capture is now
  included in the `--stress` leak hunt.
- Rendering: blending, gradients and bloom now happen in linear light, and rainbow hues sweep
  through OKLCH instead of sRGB — "everything faint is brighter" fixed properly rather than by
  raising alpha. Goldens regenerated for the gamma change, checked by eye before committing. Rave
  lasers' strobe wash retuned for the new linear-light mixing.
- Tests: the default `cargo test` is now the fast suite only — a cheap structural liveness guard
  replaces the expensive whole-registry sweeps, which move behind `slow_` + `#[ignore]` along with
  the per-family real-music/flourish checks (run them with `cargo test --release slow_ --
  --ignored`). `shell_state`'s poller is paused for the defaults test so desktop state cannot
  race the assertion. Clippy clean with `-D warnings`.
- CLI: `--help` and `--version`, and an unknown flag now refuses to launch (with a usage message)
  instead of being silently ignored. `--levels` locates `tests/fixtures` by walking up from the
  current directory rather than a compiled-in path.
- Build: `[profile.release] panic = "abort"` was tried to strip build-machine paths from the exe
  and reverted — it voided the identify feature's Drop guard and did not actually strip paths.
  `--remap-path-prefix` does the job instead (see README "Build from source"); `grep -c <username>`
  on the release exe is 0.
- CI: added, building on `windows-latest`, running the fast suite, the slow suite, and
  `cargo clippy --all-targets -- -D warnings` on every push and PR.
- Docs: README split into itself plus `docs/status.md`, `docs/known-gaps.md`, `docs/themes.md`
  and `docs/theme-prompt.md`; `HANDOVER.md` retired in favour of `docs/lessons.md`; a privacy
  note ("What leaves your machine") added under Song identification; releases now ship via
  GitHub Releases instead of a committed `dist/taskbar-eq.exe`.

## [0.2.0] — 2026-09-25

Song identification.

- **Identify Song** — an eighth hotkey slot and a Songs tray submenu. Press it while anything
  plays and the banner names the track via a ported Shazam fingerprint, posting at 4/8/12 seconds
  the way SongRec does.
- Shazam signature generator ported from [SongRec](https://github.com/marin-m/SongRec) (GPL-3,
  hence this repository's licence), with a test-only decoder for round-tripping.
- `songs.jsonl` append-only history store, and an HTML history page
  (`%APPDATA%\taskbar-eq\songs.html`) with search, sortable columns, cover art and links to
  Spotify, Apple Music, YouTube and Shazam.
- A live test drives Spotify over the real loopback and posts to Shazam; its first real reply
  became the parser fixture.

## [0.1.0] — 2026-09-01 and earlier

Everything before song identification.

- Core visualiser: WASAPI loopback capture following default-device changes, a 2048-point FFT
  with 64 log-spaced bands, the reveal/hide gate, and the overlay tracking the Widgets button (or
  the overflow chevron on Windows 10).
- Grew from 5 colourways across 2 families to **150 colourways across 22 families** — segmented
  VFD, oscilloscope, VU dials, vaporwave grid, valve row, fluid, nixie tubes, spectrogram,
  reel-to-reel, patchbay, radar, pantone, flame organ, dolphin LCD, 3D spectrum, 3D pipes, orbit,
  cherry blossom, kaleidoscope, rave lasers, brutalist and chroma field — each with its own
  flourish, fired by rarity against a running median rather than a fixed threshold.
- Spotify transport (session and media-key backends), seven bindable hotkeys with a capture
  dialog that refuses bad chords, a marquee track-name banner, and a themed context menu that
  follows Windows light/dark mode.
- Suspends under a fullscreen app (including borderless-windowed, the harder case) or a
  covered/hidden taskbar, and while the display is off — the display-off case alone made this
  app the second-largest energy consumer on a laptop measured over eight days.
- A handle/thread watchdog, and a `--stress` harness that found and reduced two real per-call
  leaks (the UI Automation tree walk, the media-session poll) 4.4x.
- External TOML colourways with a versioned schema, hot reload, and override-by-id.
- Fixed a long tail of measured defects along the way: transposed fullscreen/quiet-hours API
  constants, a vacuous range test, a `debug_assert!` compiled out of release, bloom compositing
  under opaque content instead of over it, random-theme selection biased by dead low bits of the
  system clock, and a menu z-order re-insertion that let other windows climb back on top within
  120ms.
