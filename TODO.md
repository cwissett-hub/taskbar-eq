# Working log

Kept current and pushed with every change, so progress is visible without reading the whole commit
history. Newest first within each section. Commit hashes link the claim to the evidence.

**Last updated:** v0.2.2 — THREE VIRTUAL SELF FAMILIES SHIP. `vswings`, `vsghost` and `vsorb` —
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

1. **Two more themes requested 2026-09-28**, to brainstorm and spec before building:
   something **Bones / TeamSESH** themed, and something **Cyberpunk 2077** themed.
2. **The live `--levels` check** of the two-FFT bass path against real music (Task 4 deferred
   it for want of audio) — needs Spotify playing; note it overwrites
   `tests/fixtures/real-music-bands.csv`.

## Waiting on you

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
