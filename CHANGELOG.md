# Changelog

Format loosely follows [Keep a Changelog](https://keepachangelog.com/); dates are release dates,
not commit dates.

## [Unreleased]

- `drift` — reworked into an oblique "iso" street at the user's request (the old car was a flat
  14x6 side-on bitmap that only slid left and right). The road stays horizontal but depth now runs
  up and to the right, so the skyline's blocks show roofs and side faces, the sidewalk has slanted
  seams and zebra crossings scroll past. The car is a low-poly 3D model (body, cabin, rear wing,
  four wheels with counter-steering fronts) projected and flat-shaded every frame at its real
  heading. It weaves across the road on a damped spring, holds a slip angle up to 40 degrees into
  the turn and flicks to the opposite lock on bass kicks (at most every 650 ms, on its own after
  2.2 s without one). Skid marks trail from the rear wheels while they slide, the headlights throw
  a beam that sweeps the asphalt, and the neon streaks the wet road. The smoke meter now follows
  the car's curving path. The flourish adds a full 360 spin to the sweep. 0.19 ms/frame steady,
  0.61 ms in the flourish, at 380x60 — the same as before.

## [0.4.1] — 2026-10-01

- `rave` — a room for the lasers: haze (two scrolling value-noise fog bands plus a floor glow),
  beam cones that widen with the fog density, a truss lattice with the heads hanging off it and a
  two-row crowd silhouette that raises an arm on strong onsets, floor splash under the beams, and
  a moving-head sweep flourish. Dropping the per-frame bloom allocation and sharpening the strobe
  tail took it from 1.45 ms to 0.71 ms/frame.
- `dolphin` — a 2x hero leap on a bass-following arc, splash droplets and a ring on re-entry, a
  setting sun or crescent moon with a glittering sea reflection and twinkling stars, and travelling
  crest dots. Removing the per-frame bloom and the double in-draw smoothing took it from 1.40 ms to
  0.06 ms/frame.
- `vswings` — the fan now spans 80% of the panel width, a lagging 100 ms echo pair reads off a
  ring buffer, the floor reaches both side edges, and sparks fly from the wing tips on strong
  onsets. The accent layer's per-frame bloom (CI measured 1.97 ms against the 2 ms gate) is
  replaced by a 1-px halo: 0.73 ms to 0.09 ms/frame. High-BPM depth 0.49 → 0.75.
- `vsghost` — rethought: a big glitching kaomoji face whose eyes and mouth react to bass/mids, a
  crawling phrase ticker and ticker-bar meter, and faint margin "voices"; 0.04 ms/frame. High-BPM
  depth 0.43 → 0.67.
- `Canvas::fill_poly` no longer allocates for polygons of 16 points or fewer (identical output),
  keeping scan-line crossings on the stack.
- The stricter < 1 ms/frame timing target for the newer families is now a dev-machine-only check
  (CI sets `CI=true` to skip it); CI still enforces the 2 ms gate for every gated family (the list
  in `slow_vs_timing`, now also covering `rave` and `dolphin`). The windows-latest runner measures
  1.5-2.2x slower than the dev laptop, which turned prism's 0.57 ms into 1.19 ms and failed the
  v0.4.0 tag build on CI — that release's exe was attached to the GitHub release manually.

## [0.4.0] — 2026-10-01

- `bling` — a Blingee GIF panel: rhinestone gem columns, glitter text and sparkle / `$` / crown /
  heart stamps, topped off with a bass-hit flash flourish (a full-panel white wash before the gems
  and glitter redraw over it); four colourways.
- `nos` — a 2 Fast 2 Furious dash: an LED tacho arc, shift lights, a gear/MPH readout and
  under-glow, capped by the `DANGER TO MANIFOLD` flourish — a blinking green warning screen plus a
  speed-line purge on a bass hit; four colourways.
- `drift` — a Tokyo Drift night skyline, with the tyre-smoke trail standing in as the meter and a
  steering gauge alongside, plus a `DRIFT!` flourish that slams in centred over flashing signs on a
  bass hit; four colourways.
- `prism` — from the user's reference image: a glowing spectral arc over a rim-lit horizon, with a
  flare flourish that brightens the core on a bass hit; four colourways.
- Now-playing, three ways: the tray menu's new `♪ <title>` line at the top (disabled and reading
  `♪ (nothing playing)` when nothing is loaded), a `show_now_playing` hotkey slot, and a left click
  on the meter — all three funnel through one `hotkeys::request_now_playing` flag so they always
  agree. **Behaviour change:** a left click on the meter used to send `Win+W` and open the Windows
  Widgets panel, since the overlay sits on top of the Widgets button; that is gone in favour of
  showing the current track. Press `Win+W` directly for the Widgets panel while the meter is up.
- `font3x5` gained an arched `N`, so `DANGER TO MANIFOLD` no longer reads `DAMGER TO MAMIFOLD` (the
  old near-solid `N` differed from `M` by one pixel), and a `!` glyph for `DRIFT!`.
- Fixed a hard-coded hotkey slot index in the tray menu's Songs submenu: the "Identify key" row
  read `transport.keys[7]` / `ID_BIND_BASE + 7`, correct only because `Slot::IdentifySong` happened
  to sit at index 7 in `Slot::ALL`. Both that row and the new "Show now playing key" row now look
  up their slot's position in `Slot::ALL` instead, so a future reordering can no longer silently
  bind a menu row to the wrong action.
- 32 families / 197 colourways.

## [0.3.2] — 2026-09-30

- `term` — readability pass on user feedback ("outputs get covered up almost immediately", "typing
  is too slow"):
  - Fake output lines now dwell for a fixed 3s each (`OUTPUT_DWELL_MS`) rather than a count of
    onsets — the old counter also had an off-by-one, decrementing on the same event that had just
    set it. The last two outputs are kept at once, newest on the bottom meter row and the previous
    one on the row above while its own dwell still has time left, each fading independently.
  - The prompt now types continuously while music plays (`rms > 0.02`), at a rate that scales with
    loudness, plus a burst of characters on every onset — no longer strictly one char per onset. A
    command still executes on a strong bass onset, but if it has sat complete for 1.5s with no
    strong onset, it executes on the next ordinary one instead, so a quiet track can never stall
    the prompt.
  - The status bar's left slot (previously a static `~/music`) now shows the now-playing title
    dressed up as a dodgy downloaded `.mp3` (`daire_-_earth_move_edit_(not_a_virus).mp3` and
    nine other jokes in the same vein), truncated from the middle to protect the ending and
    marquee-scrolled when it doesn't fit the slot. `media::with_now_playing` added so this reads
    the track without allocating a fresh `String` every refresh.
  - `font3x5` gained `+`, `[`, `]` for the new suffixes.
- `term` — follow-up fix: the filename could run into the centre rms readout (`...NOT_A_▮51%`
  overprinting). The status bar's right-hand `▮NN%  UTF-8  LF` is now built and right-aligned as
  one group (`build_status_group`), and the filename's slot is sized against that group's actual
  left edge minus a 6px gap, hard-clipped to whole glyphs — the two can no longer overlap
  regardless of layout changes on either side. New guard
  `filename_never_overlaps_the_readouts`. The marquee no longer scrolls continuously: it now
  pauses 1.5s on the title, scrolls once to the end, and pauses 1.5s on the suffix joke before
  resetting, so the punchline is actually readable rather than a blur.
- Fidelity pass 1: `brutal` (lit tops, shadow sides, formwork, aggregate, cracks, rebar peaks,
  slam + dust; new `brutal-sodium`), `pipes` (fat shaded pipes, three at once, joints, teapot
  flourish), `mesh` (perspective floor, shadows, fog, specular tops, camera yaw, falling ghosts),
  `orbit` (sun with corona, visible orbits, comet tails, alignment flourish).
- Virtual Self families retuned for 150-170 BPM (attack 0.9 / decay 0.35 / peak_fall 0.03, onset
  refractory 120 ms), verified against a *synthetic* 160 BPM fixture through the real smoothing
  pipeline. Review also found the three families were smoothing every band TWICE — once in the
  shared `Smoother` the pipeline already runs before `draw`, then again inside `draw` with the
  same ballistics — so the in-draw re-smoothing was removed and the families now track the
  already-smoothed level directly (variant B of the two measured; see `slow_vs_high_bpm_response`'s
  doc comment for both sets of numbers).
- Performance: pipes, mesh and orbit skip bloom on opaque panels (1+ ms saved, byte-identical).

## [0.3.1] — 2026-09-29

- `term` — the user's VSCode 2077 theme as a live terminal: output lines are the meter, a prompt
  types session-specific commands on onsets (dev / network / git / sysadmin / build), `panic!`
  flourish; 5 colourways.
- `font3x5`: block, outline, tilde and punctuation glyphs; lowercase input accepted.
- `themes`: the tray menu labels now name the artists/game a user would look for — the `sesh` family
  reads "Bones / TeamSESH: VHS" and `night` reads "Cyberpunk 2077: HUD" (was "Bones: VHS tape" /
  "Night City: HUD", which made `night` look absent and `sesh` hard to find in the label-sorted menu).
- `sesh`: the five colourways now read as five different things rather than one look nudged by a
  single `mix` constant. A per-colourway `Style` drives each: `sesh-tape` is a heavy 8-band tear with
  a rolling head-switching bar and the word shrunk to a bottom-right caption; `sesh-word` turns the
  tears off, pulses a big centred word on the bass and drips twice as hard; `sesh-vhs` gets a
  cold-cast panel, doubled ±2px red/cyan chroma bleed on the word and stamp, and a slow colour-shift
  wobble; `sesh-red` reddens the loud bands' tears; `sesh-bleached` bakes in faint paper grain. New
  guard `the_five_colourways_are_visibly_different` requires every pair to differ in ≥15% of interior
  pixels.
- `sesh`: the flourish is now a bass-hit **slam** rather than the word sliding in. It fires only when
  the trigger fires AND the low bands' mean clears 0.6, and the dropout is redesigned so nothing
  travels laterally — a blank frame, then a ~250ms slam of the word held in place (jittering ±1px,
  thicker outline) on a density-decaying static field with two or three tear rows that snap sideways
  and back per frame, no wrap-around.
- `sesh`: two colourways added and one recoloured, from Bones' *Blunts From The Graveyard* tapes —
  `sesh-graveyard` (violet-black panel, dim-violet picture, moon-white word with a green outline,
  sickly-green night-vision tears) and `sesh-nightvision` (phosphor green on near-black, black word
  outline, heavy sensor-noise grain, red REC dot); `sesh-vhs` recast to muted violet `#120c1c` with a
  pink-red/cyan bleed pair. 175 colourways; the visible-difference guard now covers all seven.

## [0.3.0] — 2026-09-29

Two families: `sesh` — Bones/TeamSESH VHS tape (12 tracking bands that tear with the music, a
blackletter word with drips, camcorder stamp, dropout flourish; 5 colourways) and `night` —
Cyberpunk 2077 HUD strip (16 hatch-filled chamfered cells, cyan scanner, RAM/HP/NET readouts,
relic-malfunction RGB-split flourish; 5 colourways).

- `gothic`: a pixel-blackletter font module (13 letters + R, two sizes).
- `font3x5`: one shared full-alphabet 3x5 label font (sesh uses it; vsghost/vswings still carry
  private copies).
- `vsorb`: ~2x cheaper per frame (cached backdrop, orb-bounded bloom; was failing the 2 ms gate
  on the CI runner).
- CI: toolchain pinned to 1.96 with the clippy component.

## [0.2.2] — 2026-09-29

Three Virtual Self families: vswings, vsghost, vsorb (13 colourways).

- `vswings` (5 colourways: Particle Arts, Eon Break, Angel Voices, Utopia, Ghost) — 32 bands
  fanned into two mirrored angel wings on a Y2K chrome floor; bass beats the root feathers.
- `vsghost` (4 colourways: White, Cobalt, Inverse, Violet) — a piano-roll glitch terminal, bands
  as scrolling MIDI columns, light rays, a romanised phrase, a datamosh flourish.
- `vsorb` (4 colourways: Chrome, Eon, Angel, Mono) — a wireframe chrome icosphere the bass
  inflates, ringed by 24 spectrum spikes; it shatters into its triangles on a flourish.
- Rulings: every panel is opaque (no per-pixel holes for the weather widget to show through, per
  `slow_no_family_leaves_a_transparent_pixel_inside_its_panel`); on the two light-panel sets
  (`vswings-particle-arts`'s ice panel, `vswings-angel-voices`'s pink panel) the wing tip is a
  saturated palette colour rather than white, retuned so it still reads as a bright edge without
  washing out against the panel.
- Timing: all three measured comfortably under the 2 ms/frame budget (`slow_vs_timing`,
  `cargo test --release slow_vs_timing -- --ignored --nocapture`).

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
