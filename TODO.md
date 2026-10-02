# Working log

Kept current and pushed with every change, so progress is visible without reading the whole commit
history. Newest first within each section. Commit hashes link the claim to the evidence.

**Last updated:** v0.4.2 — TOKYO DRIFT, THIRD TAKE: CHASE CAM ON A TOUGE, R34
TAIL. The user: "a view from behind the car going along a touge road" and "model the car off the
rear of an R34 Skyline". `drift` is now a pseudo-3D racer road (OutRun-style rows, a winding course,
haze to the horizon) on a mountain pass: pines on the slope, a guardrail on reflector posts, the
city's neon in the valley, ridges that shift as you corner. The car is seen from behind with four
round tail lamps and a tall wing. It drifts because of the bends (slip into the corner, swapping
lock through S-bends), with a clutch kick and exhaust pop on bass kicks. The smoke meter rolls back
toward the camera; the flourish is a 360. 0.23 ms/frame steady. Review sheet section 32.

Earlier (same day, superseded): TOKYO DRIFT REWORKED INTO AN ISO STREET, at the user's
request ("the car sprite is completely 2d and just goes left and right, doesn't look like drifting,
find a way to make it a more iso view"). `drift` now draws in an oblique iso projection: iso-box
skyline with roofs and side faces, slanted sidewalk seams, zebra crossings, neon streaking the wet
road. The car is a low-poly 3D coupe (body, cabin, wing, four wheels, counter-steering fronts),
projected and flat-shaded every frame, so it turns. It weaves across the road holding up to 40
degrees of slip and flicks to the opposite lock on bass kicks, laying skid marks; the smoke meter
follows its path; the flourish adds a 360 spin. 0.19 ms/frame steady, unchanged. The user judges it
on review sheet section 31 — see Waiting on you below.

Earlier: v0.4.1 — FIDELITY PASS 2: FOUR MORE FAMILIES REWORKED. `rave` (a room for the
lasers — haze, beam cones, a truss and a crowd that raises an arm on strong onsets, floor splash, a
moving-head sweep flourish; per-frame bloom removed, 1.45 → 0.71 ms), `dolphin` (a 2x hero on a
bass-following leap arc, splash, a sun/moon with a sea reflection, stars, travelling crests;
per-frame bloom AND double in-draw smoothing removed, 1.40 → 0.06 ms), `vswings` (80% span, a
lagging echo pair, floor reaching both edges, sparks on strong onsets; the accent layer's bloom —
1.97 ms on the CI runner, over the 2 ms gate — replaced by a 1-px halo, 0.73 → 0.09 ms;
`Canvas::fill_poly` no longer allocates for polygons of ≤ 16 points) and `vsghost` (rethought: a
big glitching kaomoji face reacting to bass/mids, a crawling phrase ticker and ticker-bar meter,
margin "voices"; 0.04 ms). High-BPM depth: vswings 0.49 → 0.75, vsghost 0.43 → 0.67. Also: the
< 1 ms/frame timing target for the newer families is now dev-machine-only (CI keeps the 2 ms gate
for every gated family — the list in `slow_vs_timing`, now also covering `rave` and `dolphin`) —
the windows-latest runner measures 1.5-2.2x slower than the dev laptop, which failed the v0.4.0
tag build on CI; that release's exe was attached to the GitHub release manually.
The user judges the four reworked families on the review sheet — see Waiting on you below.

Earlier: v0.4.0 — FOUR FAMILIES SHIP, PLUS A NOW-PLAYING CLICK. `bling` (a Blingee GIF
panel — rhinestone gem columns, glitter text and sparkle / `$` / crown / heart stamps, a bass-hit
flash flourish; four colourways), `nos` (a 2 Fast 2 Furious dash — LED tacho arc, shift lights, gear/MPH, under-glow,
the `DANGER TO MANIFOLD` flourish plus a speed-line purge; four colourways), `drift` (a Tokyo Drift
night skyline — the tyre-smoke trail is the meter, a steering gauge, a `DRIFT!` flourish; four
colourways) and `prism` (from the user's reference image — a glowing spectral arc over a rim-lit
horizon, a flare flourish; four colourways) join 32 families / 197 colourways. Now-playing is
reachable three ways — the tray menu's new `♪ <title>` line, a `show_now_playing` hotkey slot, and
a left click on the meter, all through one `hotkeys::request_now_playing` flag — **and a left click
on the meter no longer opens the Windows Widgets panel**; it used to send `Win+W` since the overlay
sits on top of the Widgets button, and that click-through is gone in favour of showing the track
(press `Win+W` directly for Widgets). `font3x5` gained an arched `N` (the old one differed from `M`
by one pixel, which made `DANGER TO MANIFOLD` read `DAMGER TO MAMIFOLD`) and a `!` glyph. Also fixed:
a hard-coded hotkey slot index (`transport.keys[7]` / `ID_BIND_BASE + 7`) in the tray's Songs
submenu, which only worked because `Slot::IdentifySong` happened to sit at index 7 — both that row
and the new "Show now playing key" row now look their slot up in `Slot::ALL` by position. All ten
timed families still measure comfortably under budget (`slow_vs_timing`): vswings 0.79 ms, vsghost
0.05 ms, vsorb 0.77 ms, sesh 0.26 ms, night 0.19 ms, term 0.03 ms, bling 0.09 ms, nos 0.03 ms, drift
0.29 ms, prism 0.57 ms. The user judges the four new families and the now-playing click on the
review sheet — see Waiting on you below.

Earlier: v0.3.2 — FIDELITY PASS 1: FOUR FAMILIES REWORKED, PLUS THE VIRTUAL SELF RETUNE.
`brutal` (lit tops, shadow sides, formwork, aggregate, cracks, rebar peaks; new `brutal-sodium`
colourway) fires its slam + dust on every bass onset — the flourish proper is THE MONOLITH, every
block slamming to full height at once with the panel inverted. `pipes` (fat shaded pipes, three at
once, joints, a teapot flourish) and `mesh` (perspective floor, shadows, fog, specular tops,
camera yaw) round out the visual pass; `mesh` sheds a falling ghost whenever a front bar drops 15%
below the peak it rose to, which is NOT its flourish either — `mesh`'s own flourish is the whole
stack surging forward one depth step and settling back. `orbit` (sun with corona, visible orbits,
comet tails, an alignment flourish) rounds out the four. All three of `pipes`/`mesh`/`orbit` also skip bloom entirely on opaque panels
(1+ ms saved, byte-identical output) since the bloom composite is a no-op there. Separately, all
three Virtual Self families (`vswings`, `vsghost`, `vsorb`) are retuned for 150-170 BPM material:
`ballistics` attack/decay/peak_fall 0.9/0.35/0.03 (all three bases, were 0.55/0.12/0.01,
0.8/0.25/0.02, 0.5/0.1/0.01) and onset refractory 120 ms (was 200 ms). A final review also found
the three families were smoothing every band TWICE — the shared pipeline `Smoother` already runs
before `draw`, and each family's `draw` was re-smoothing the result with the same ballistics — so
the in-draw re-smoothing was removed; the families now track the already-smoothed level directly.
Verified against a new *synthetic* 160 BPM fixture (`tests/fixtures/high-bpm-bands.csv`,
`slow_vs_high_bpm_response`), fed through the real `Smoother` pipeline rather than straight into
`draw`. Neither check alone discriminates old from new ballistics for all three families on this
fixture - each catches a different subset (the lag check fails old `vswings`/`vsghost` but passes
old `vsorb`; the depth check fails old `vswings`/`vsorb` but passes old `vsghost`) - so it takes
both together to show the retune actually changed something for every family; see that test's doc
comment for the full old-vs-new table and the two smoothing variants measured (A: as shipped with
double smoothing, B: single smoothing — B shipped, since all three cleared the depth floor on the
new ballistics and the existing slow tests still passed). All timed families still measure comfortably under budget (`slow_vs_timing`):
vswings 0.72 ms, vsghost 0.06 ms, vsorb 0.80 ms, sesh 0.27 ms, night 0.20 ms, term 0.04 ms. The
user judges the four reworked families and the VS retune on the review sheet — see Waiting on you
below.

Earlier: v0.3.1 — TERM FAMILY SHIPS, PLUS A SESH REWORK. `term` (the user's own VSCode
2077 theme as a live terminal — output lines are the meter, a prompt types session-specific
commands on onsets across dev / network / git / sysadmin / build, a red `panic!` stack-trace
flourish; 5 colourways) joins two reworks landed alongside it: `sesh` now ships seven
colourways (two added from Bones' *Blunts From The Graveyard* tapes, one recoloured) with a
bass-hit **slam** flourish replacing the old sliding word, and the tray menu labels for `sesh`
and `night` are rewritten to name the artist/game a user would actually search for, so neither
gets lost in the label-sorted menu. New shared font work: `font3x5` gained block, outline,
tilde and punctuation glyphs plus lowercase input. All six timed families measure comfortably
under budget (`slow_vs_timing`): vswings 0.72 ms, vsghost 0.06 ms, vsorb 0.75 ms, sesh 0.26 ms,
night 0.20 ms, term 0.03 ms. The user judges `term`, the `sesh` rework and `night` on the
review sheet — see Waiting on you below.

Earlier: v0.3.0 — TWO FAMILIES SHIP: `sesh` AND `night`. `sesh` (Bones/TeamSESH VHS
tape — 12 tracking bands that tear with the music, a blackletter word with drips, a camcorder
stamp, a dropout flourish) and `night` (Cyberpunk 2077 HUD strip — 16 hatch-filled chamfered
cells, a cyan scanner, RAM/HP/NET readouts, a relic-malfunction RGB-split flourish), 10 new
colourways (173 across 27 families). Two new shared font modules: `gothic` (pixel-blackletter,
13 letters + R, two sizes) and `font3x5` (one shared full-alphabet 3x5 label font — sesh uses
it; vsghost/vswings still carry private copies). `vsorb` came along for the ride at ~2x cheaper
per frame (cached backdrop, orb-bounded bloom — it was failing the 2 ms gate on the CI runner).
All five timed families measure comfortably under budget (`slow_vs_timing`): vswings 0.85 ms,
vsghost 0.06 ms, vsorb 0.78 ms, sesh 0.29 ms, night 0.20 ms. The user judges the two new
families on the review sheet — see Waiting on you below.

Earlier: v0.2.2 — THREE VIRTUAL SELF FAMILIES SHIP. `vswings`, `vsghost` and `vsorb` —
13 new colourways (163 across 25 families), each under the 2 ms/frame timing budget
(`slow_vs_timing`). Every panel is opaque and the two light-panel colourways (`vswings-particle-
arts`'s ice panel, `vswings-angel-voices`'s pink panel) got their wing-tip colour retuned off
white to a saturated palette colour so it still reads against the panel. The user judges the
three families on the review sheet — see Waiting on you below.

Earlier: v0.2.1 — HEALTH FIXES SHIP. No new families; a pass over correctness, test
speed and release hygiene instead. Bands now go log-spaced below the crossover on a real second
FFT (the one algorithmic change), ballistics are frame-rate independent, config saves atomically,
and a hotkey rebind no longer clobbers its own other keys. Blending, gradients and bloom now
happen in linear light and rainbows sweep OKLCH at a fixed lightness — see Waiting on you below,
both rulings want a look. The default `cargo test` is now the fast suite only; the slow
whole-registry and real-music sweeps moved behind `slow_` + `--ignored`, which is also what CI
now runs, alongside clippy, on every push. The exe no longer carries a build-machine path
(`--remap-path-prefix`, replacing an earlier `panic = "abort"` attempt that was tried and
reverted — it voided the identify Drop guard and did not strip paths). Releases ship via GitHub
Releases now; see CHANGELOG.md for the full list, one line per change.

Earlier: SONG IDENTIFICATION SHIPPED (v0.2.0). Press a key while anything plays and the banner
names the song via Shazam; every find goes to `songs.jsonl`; Songs -> Song history opens a dark
cyberpunk page with Spotify / Apple Music / YouTube / Shazam links. The repo is GPL-3 because the
fingerprinting is a port of SongRec.

Earlier still: 150 colourways across 22 families, every one with a flourish. See CHANGELOG.md.

---

## In progress

Nothing right now. Queued, in order:

1. **Recapture the high-BPM fixture from a real VS track.** `tests/fixtures/high-bpm-bands.csv`
   is synthetic (160 BPM, kick + off-beat hats) because a live loopback capture needs Spotify
   playing through this machine, which an agent session can't rely on — see the fixture's own
   header. Replace it with a real 150-170 BPM Virtual Self capture via `--levels` when one can be
   made, and rerun `slow_vs_high_bpm_response`.
2. **The live `--levels` check** of the two-FFT bass path against real music — needs Spotify
   playing; note it overwrites `tests/fixtures/real-music-bands.csv`.
3. **Check the other older families for in-draw double smoothing and per-frame bloom** —
   `dolphin` and `rave` both had them (fidelity pass 2 found and removed both; see CHANGELOG
   0.4.1), so the remaining families that predate that pass are worth a sweep for the same two
   patterns.

## Waiting on you

- [ ] **Judge section 33 on `docs/review/index.html`: drift's tucked-in wheels, the hills and the
      new music reactions** (kick: lamp flare and camera jolt; underglow on the bass; city lights
      on the mids; reflectors on the treble). Running on your taskbar now; unreleased on main.
- [ ] **Judge section 32 on `docs/review/index.html`: Tokyo Drift as a chase cam down a touge
      with an R34-style tail.** Running on your taskbar now (pick any `drift-*` colourway).
      Open questions: do the four round tail lamps read as an R34 at 1:1 (they are ~2 px each)?
      Orange and Night have a white `hot`, so their lamps are white; force red lamps everywhere?
      Smoke amount on loud tracks? Is the valley's city glow enough? Want a Bayside Blue
      colourway? Keep the 360 in the flourish or make it a bigger flick?
- [ ] **Judge sections 23-30 on `docs/review/index.html`: keep, tune or drop each colourway
      (v0.4.0 + v0.4.1), with the implementers' open questions, and try the now-playing
      click/tray/hotkey.** Section 23, `bling`: busy at 1:1? are the gold gems meant to read as
      diamond-cut or gold? Section 24, `nos`: is the backlit dial face too big? Section 25,
      `drift`: Shibuya's second neon colour reads red rather than cyan — there's only one neon
      zone right now, so a second zone would let the two colours coexist instead of one
      overriding the other; worth adding? Section 26, `prism`: does the sunset match your
      reference image; do you want a wider, fainter halo? Section 27, `rave`: are the arms too
      busy at 1:1; is the sharpened strobe tail OK? Section 28, `dolphin`: is the 2x hero hidden
      by a loud sea at 128x44 (`HERO_MIN_ROWS`)? Section 29, `vswings`: is the echo only visible
      after beats; do the feather shafts look like dashes? Section 30, `vsghost`: do the eyes
      read as `0` rather than `O`; the slices re-roll every frame — does that read as jittery in
      motion? Separately: left-click the meter, the tray menu's `♪ <title>` line, and a bound
      `show_now_playing` key should all show the same banner — and the left click should no
      longer open the Windows Widgets panel (press `Win+W` directly for that now). Also please
      delete the leftover folder `C:\Users\cwisset\Documents\projects\te-v040` — a throwaway
      checkout the assistant could not remove itself.
- [ ] **Judge sections 20-22 on `docs/review/index.html`: keep, tune or drop each colourway
      (v0.3.2).** Section 20, `term`'s filename/output readability fix: does the status bar's
      right-aligned group (`▮NN%  UTF-8  LF`) still leave the filename slot legible; does the
      3 s output dwell and the pause-scroll-pause marquee actually read better than before.
      Sections 21-22, `brutal`+`pipes` and `mesh`+`orbit`: do the fidelity-pass reworks (lit tops
      and rebar on `brutal`, the teapot flourish on `pipes`, the perspective floor and falling
      ghosts on `mesh`, the corona and comet tails on `orbit`) actually read as more cohesive with
      `blossom`/`vaporwave`, per the brief that started this pass; is `brutal-sodium` a keep. And
      separately: do the three Virtual Self families now keep up on a REAL 150-170 BPM Virtual
      Self track, not just the synthetic 160 BPM fixture `slow_vs_high_bpm_response` checks
      against — if the retune still reads slow or smeared by ear, the ballistics numbers are one
      line each to move further.
- [ ] **Judge the three Virtual Self families in `docs/review/index.html` (sections 15-17): keep,
      tune or drop each (v0.2.2).** Specific eye questions the implementers flagged: `vswings`'s
      tip-vs-body contrast on the ice (`particle-arts`) and pink (`angel-voices`) panels; `vsghost`'s
      ghost grid only showing in the lower two-thirds of the panel; `vsorb-chrome` being the
      weakest read of the four orb colourways, its lower rim blending into the gradient.
- [ ] **Rainbows are now fully saturated OKLCH at a fixed lightness (Task 7, v0.2.1) — my
      ruling, not obviously correct.** Rainbow hues used to be capped at `RAINBOW_SAT` 0.68-0.70
      because full saturation failed the 3:1 contrast rule at some hues in gamma-space sRGB. In
      OKLCH at a fixed `RAINBOW_L` of 0.72, the ceiling stopped binding (worst hue now measures
      7.3:1), so saturation went to 1.0 across the board. That is a real change to how every
      rainbow colourway looks — punchier, more saturated — and I have not seen it running. Say if
      0.72 lightness reads as too pale or too dark; the constant is one line to move either way.
- [ ] **"Everything faint is brighter" pass (Task 7, v0.2.1) — a global rendering change, not
      eyeballed yet.** Blending, gradients and bloom now average light linearly instead of
      averaging gamma-encoded sRGB codes, so every partially-transparent mark (halos, gradients,
      soft edges) is measurably brighter than before: half-alpha white over black is code 188, not
      128. Three goldens (p1-green, vfd-ice, vu-cream) were regenerated and read correctly by eye;
      the other nineteen families were not individually reviewed at this resolution. If a family's
      glow now looks blown out or a gradient looks washed, that is this change, and the fix is
      almost certainly a `glow_strength`/`bloom` retune per colourway rather than reverting the
      linear-light maths (which is the physically correct way to average light).
- [ ] **One judgement call in the fullscreen fix worth a second opinion (carried over,
      unresolved).** The shell-class exclusion list decides what does NOT count as a fullscreen
      app, and every name on it costs coverage. `Windows.UI.Core.CoreWindow` is on it, because
      measured here the LOCK SCREEN is a full-monitor CoreWindow, and Start and Search are the
      same class and also full-monitor on Win11 - so without it the meter would vanish whenever
      you opened Start. The cost: a genuinely fullscreen UWP app whose own CoreWindow is
      foreground would not suspend us. Packaged games hosted in `ApplicationFrameWindow` ARE
      caught - I took that name off the list for exactly that reason.

## Open, unresolved

- [ ] **User-authored TOML colourways for the Virtual Self families get `Ballistics::default()`,
      not the family base.** `schema::parse` (`src/themes/schema.rs`) falls back to
      `Ballistics::default()` (attack 0.55 / decay 0.11 / peak_fall 0.0055 — the generic VFD Ice
      figure) for any ballistics field a TOML theme omits; it has no way to fall back to
      `vswings_base`/`vsghost_base`/`vsorb_base`'s own ballistics instead, because the parser
      builds a theme from `[ballistics]` + the schema's own default, with no notion of "this
      family's base." A user TOML for `vswings`/`vsghost`/`vsorb` that does not set `[ballistics]`
      explicitly therefore gets that generic, unrelated default — not the 150-170 BPM numbers the
      built-in colourways now ship with, and not what the old built-in numbers were either — found
      while retuning the three for v0.3.2.
- [ ] **The empty taskbar to the left is still unclaimed.** Measured on the reference machine:
      app buttons end at x≈1119 and the Widgets button starts at x≈1425, roughly 300px of dead
      taskbar between them, most useful to a wide `scene`-style family (see
      `docs/theme-backlog.md` item 1/2). That gap only exists while the taskbar is left-aligned
      and not full of windows, so it would have to be computed at runtime from the actual gap,
      not assumed — the rect-tracking machinery to do that already exists (it re-discovers every
      second).
- [ ] **Song-history "hide" is per-browser.** The page stores hidden songs in `localStorage`, so
      the tray's Songs submenu still lists them, and a different browser shows them again. A
      native Win32 history window would own that state properly; parked until you want it.
- [ ] **Shazam's endpoint is undocumented.** When it changes, identification will say "no match"
      and the log will carry the HTTP status and the first 200 bytes; the full last reply is in
      `%APPDATA%\taskbar-eq\last_shazam.json`. Fallback would be ACRCloud or AudD behind the same
      `Find` type.
- [ ] **A rare test flake, understood but not fully fixed.** Five family flourish tests (VFD, VU,
      waterfall, valve, nixie) fire via the audio path, which consults the process-global
      `ENABLED` switch that `dsp::flourish`'s own tests toggle. No lock can protect that: a switch
      that is false is false for every test running at that moment. Seen ONCE in 25 full-suite
      runs, and 36 runs since have been clean. The fix is the pattern the pantone, reel and scope
      tests now use - `Trigger::force_next()`, which is instance-local - but converting the other
      five needs each fixture's firing frame re-derived, because forcing at the START of the
      firing sequence measures a third of a second of decayed envelope and fails. Attempted,
      reverted, recorded rather than left half-done.
- [ ] **THE RESOURCE LEAK. Cause still unknown.** A days-old instance measured 18,962 threads /
      131,454 handles / 1.47 GB / 1.46 cores against a healthy 14 / 320 / 26 MB / 4% of one core.
      That is what took a fullscreen app from 160fps to 30 with input loss. A fresh instance is
      flat over ten minutes in every state I can create; the bad one had lived days and survived
      a machine sleep, which is the leading suspect.
      `win::health` bounds the damage (warns at 3,000 handles, exits at 30,000) and logs an hourly
      baseline, so a recurrence arrives with a growth curve. **If it happens again, please do not
      kill the process before telling me** - I did exactly that once and destroyed the only
      evidence.
- [ ] **THE LEAK IS MEASURED, AND IT IS NOT THE FAULT YOU REPORTED.** `--stress` hammers each
      suspect path and counts this process's own handles and threads. Two of six leak:

      | suspect | handles/1k calls | threads/1k |
      |---|---|---|
      | UIA `taskbar_elements` (the taskbar tree walk) | **+107** | +2 |
      | media session poll (the real GSMTC round trip) | **+22** | +3 |
      | `CoCreateInstance(CUIAutomation)` on its own | 0.0 | 0.0 |
      | `taskbar_rect`, `notification_state`, `foreground_window` | 0.0 | 0.0 |

      So it is not COM object creation and not talking to the shell - it is walking the
      accessibility tree, and the WinRT session read. Both are real defects and both are reduced
      4.4x, not eliminated (391 -> 90 handles an hour, moving the watchdog's fatal threshold from
      3.2 days of uptime to 13.9). **But the arithmetic rules them out as the reported fault**: at
      those rates, 45 minutes of running predicts 293 handles and 23 threads, against 131,454 and
      18,962 measured on the bad instance - about 450x and 800x more. The borderless-fullscreen
      gap remains the leading candidate: a UIA call into an `explorer.exe` that a game is
      monopolising blocks for far longer, and RPC worker threads pile up while it does. **Still
      inference. A retest on the gaming machine is what would settle it.** The real fixes are a
      `SetWinEventHook` on `EVENT_OBJECT_LOCATIONCHANGE` instead of polling the tree, and a GSMTC
      event subscription instead of polling the session - both larger changes with their own
      risks.

---

## Notes to self

- **Standing rule (user, 29 Sep 2026): after each job — merge, push, release build, relaunch the
  running exe — BEFORE starting the next job.**
- **CI is wired up as of v0.2.1** (`.github/workflows/ci.yml`) — was a note-to-self here, now
  done: it runs the fast suite, `cargo test --release slow_ -- --ignored`, and
  `cargo clippy --all-targets -- -D warnings` on every push and PR to `windows-latest`.
- **Restore the file BEFORE the run, never only after.** Twice now a mutation sweep timed out
  mid-iteration and left a mutant constant in the tree, and the next thing I measured was silently
  testing changed code - once reporting three "caught" mutants that had matched nothing at all. Copy
  the good file in at the START of each iteration and echo the constant so the log proves what ran.
- **Measure before claiming.** Three times this session a confident claim was wrong: the UIA cache
  (slower, not faster), the trigger key "dropping presses" (the log deduplicates), the fixture that
  "showed nothing" (its own audio saturated the display). Every one was caught by measuring.
- **A probe that measures the wrong thing is worse than none.** Vacuous or misdirected probes found here:
  a whole-canvas ink ratio diluted by static print; lit-row counts that could not discriminate; total
  luminance on a light-panel colourway (inverted); a leak probe that timed the failure path because COM
  was not initialised; a UIA timing taken with a cold apartment.
- **Fixtures must contain the hazard.** The random-bias tests seed with multiples of 100 because the real
  clock does; sweeping arbitrary seeds passes against the buggy code.

---

## Done

See [CHANGELOG.md](CHANGELOG.md) for the full release history, one line per change, newest first.
