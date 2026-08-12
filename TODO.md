# Working log

Kept current and pushed with every change, so progress is visible without reading the whole commit
history. Newest first within each section. Commit hashes link the claim to the evidence.

**Last updated:** the Spectrogram's vacuous fold test is fixed. All nine flourishes done, review
sheet written, README current.

---

## In progress

- [ ] **One documented defect left that I can act on alone:** the **Patchbay** folds 12.8 of the 64
      bands into each cable at 190px (only 5 cables fit), so its sag cue flattens on real music. Next
      up. Everything else on the list needs your eyes or a recurrence of a bug.

## Waiting on you

- [ ] **THE REVIEW SHEET IS WRITTEN: open `docs/review/index.html`.** Nine items, one per family, each
      with the render, what it is meant to be, and what specifically I want judged. It is one pass
      rather than thirteen. **Item 7, the Pantone plate slip, is the one I would change first** - it is
      the strongest of the nine by a distance and comes down on one word.
- [ ] **The `flourish` and `flourish_toggle` hotkeys are deliberately unbound** - the keys are yours to
      choose. Tray menu -> Flourishes -> the two "key" entries. `Ctrl+Semicolon` and `Ctrl+Quote` both
      registered cleanly when tested; `Ctrl+Win+F` and `Ctrl+Alt+G` are already taken on this machine.
- [ ] **`random_theme` was restored as `Ctrl+]`** after I clobbered it with a careless regex. Say if
      that is not what you had.
- [ ] **"Any theme" is now uniform per FAMILY, not per colourway** - my judgement call, not a bug fix.
      A colourway in a small family is now individually likelier than one in a large family. Three-line
      revert if you would rather have the old behaviour.
- [ ] **Windows 10 verification** - blocked on a `--diagnose` log from the other machine.

## Open, unresolved

- [ ] **A rare test flake, understood but not fully fixed.** Five family flourish tests (VFD, VU,
      waterfall, valve, nixie) fire via the audio path, which consults the process-global `ENABLED`
      switch that `dsp::flourish`'s own tests toggle. No lock can protect that: a switch that is false
      is false for every test running at that moment. Seen ONCE in 25 full-suite runs, and 36 runs
      since have been clean. The fix is the pattern the pantone, reel and scope tests now use -
      `Trigger::force_next()`, which is instance-local - but converting the other five needs each
      fixture's firing frame re-derived, because forcing at the START of the firing sequence measures a
      third of a second of decayed envelope and fails. Attempted, reverted, recorded rather than left
      half-done.

- [ ] **THE RESOURCE LEAK. Cause still unknown.** A days-old instance measured 18,962 threads /
      131,454 handles / 1.47 GB / 1.46 cores against a healthy 14 / 320 / 26 MB / 4% of one core. That
      is what took a fullscreen app from 160fps to 30 with input loss. A fresh instance is flat over ten
      minutes in every state I can create; the bad one had lived days and survived a machine sleep,
      which is the leading suspect.
      `win::health` bounds the damage (warns at 3,000 handles, exits at 30,000) and logs an hourly
      baseline, so a recurrence arrives with a growth curve. **If it happens again, please do not kill
      the process before telling me** - I did exactly that and destroyed the only evidence.

---

## Done

### Correctness and performance

- `a8a9f75` **Random buttons were biased, two unrelated causes.** Within-family shuffle chi2/df
  **24.93 -> 1.19**; the clock's low bits are dead (`nanos & 3` was zero on all 2,000 samples) and one
  xorshift round cannot reach bit 0, and `% 8` reads only those bits - invisible in the whole-list mode,
  stark within a family. Also: "any theme" was uniform per colourway, so scope took 14.2% of presses
  against nixie's 5.1%; now ~7.7% each.
- `7d9758f` **Halved the taskbar rediscovery** (CPU 4.0% -> 2.7% of one core), **cached the media
  session manager** (was 216,000 WinRT activations a day, and hammering it wedges the broker), **capture
  failures now reach the log** instead of `eprintln!`, which goes nowhere in a windows-subsystem binary.
  Caching the UIA client was tried and **reverted - it is 68ms/call against 52ms per-call**, a
  pessimisation I had asserted was an optimisation.
- `c36fcd1` **Sleeps while a fullscreen app is on top** - no UIA, no drawing, window off screen, 15x
  fewer wakeups. Plus `win::health`. Caught a deadlock of my own before it shipped: suspending on
  `should_show` also catches "I do not know where to draw", so the meter would never have come back.
- `ddc627b` **Track-change freeze fixed.** `SHQueryUserNotificationState` was a cross-process shell call
  running once per FRAME, blocking 280-380ms whenever the shell was busy - and a track change is what
  makes it busy. Now a 200ms poller thread. Six track changes, zero stalls.
- `aa826be` Menu/dialog no longer stalls the meter (posted `WM_TICK`; 3 gaps -> 0).

### Features

- `c716837` **Bindable "flourish now" and "flourishes on/off"**, in the tray menu and as hotkeys, left
  unbound. Found three latent bugs while wiring: a slot-index catch-all that would have overwritten the
  shuffle key, a menu array that **panics at the sixth slot**, and test interference from the
  process-global switch.
- **The Spectrogram's vacuous fold test is FIXED.** It passed with `max` replaced by `mean` - the bug
  it existed to catch. Three attempts, and the cause was none of the things I first assumed:
  **the pitch-track marker was the problem.** `draw` marks each column's dominant band in `t.hot` at
  high alpha, at the row that band folds into - so for a spectrum with one loud band, that is exactly
  the row the test sampled. It was measuring the marker, which sits at full brightness whatever the
  fold does. Two reasonable-looking dead ends first: lowering the levels to stay inside `response`'s
  clamp (the marker does not care about level), and comparing against separate renders predicting each
  fold (which read 252.8 where a max fold predicts 138.8 - HIGHER than the value it was meant to
  match, because one pixel compared across two renders carries its neighbours' bloom).
  Now a louder decoy band parks the marker seven rows away, and the comparison is within ONE render:
  make one of the row's bands loud, or all of them - a max reads the same either way. Verified against
  a mean fold (51 against 135) and a min fold (13 against 135).

- **README rewritten for everything added since 2026-08-05.** A full "Using it" chapter (mouse, tray
  menu, the seven keys and the capture dialog, Spotify transport and both backends, the banner, all
  nine flourishes, suspend and the watchdog), a configuration reference for every config field, and the
  two missing CLI flags. Also repaired real damage found while reading it: two `%APPDATA%` paths had
  their `	` eaten into literal TAB characters, two bullets in Known gaps were truncated mid-sentence,
  the colourway count was stale at 87, and two source comments still said 88.
  Honest note in the config table: **`brightness` and `saturation` are inert.** They are parsed and
  saved and nothing reads them. Left in and documented as such rather than quietly listed as settings -
  say the word and they come out.

- **Patchbay flourish: the panel re-patches itself and comes back.** Every cable slides to the other
  jack of its own pair, so the chevron of leans mirrors and returns - using geometry that was already
  there, since each pair's second jack sits unpatched next to it. It ANIMATES: a hard swap reads as a
  dropped frame, because a single-frame change of shape at 60fps is not perceived as motion at all.
  The envelope is used as a PHASE (`sin(pi * (1 - level))`), not an amplitude, which is what brings the
  cable back into its own socket instead of leaving it fading out stranded between two.
  One measurement bug, found by the assertion failing on working code: the sample row sat within the
  jack radius, so the brightest pixel in it was a static socket collar and the measurement snapped to a
  jack column every frame - reporting that the cable teleported while it was sliding smoothly.
  Verified against two mutants: a dead envelope, and a hard swap instead of a slide.

- **Pantone flourish: a plate slips out of register.** The family ALREADY fringes horizontally and
  widens that fringe with energy, so pushing the horizontal shift further would have read as louder
  music. The slip is therefore mostly vertical - an axis nothing else here uses - which meant
  generalising `Canvas::chromatic_aberration` into a 2-D `misregister(dx, dy)`; the old entry point
  delegates and is asserted byte-identical, because five colourways' goldens encode its output.
  Two vacuous tests found by mutation. The plate-lag assertion compared against `MISREG_Y as i32`, so
  mutating that constant to zero moved the expectation with it - it is a literal 3 now. And the peak
  test passed with the envelope mutated to 1ms, because `Envelope` sets its level to 1.0 on the firing
  frame whatever its decay is, so a separate test now measures a third of a second in.
  Also found a REAL race, not a test artefact: `flourish::request()` is one process-global atomic and
  every family's `draw` consumes it, so in a parallel suite an unrelated drawing test eats it. The
  symptom was pathological - the effect provably fired at the right offset when run alone and compared
  byte-identical when the suite ran. `Trigger::force_next()` fires one instance and touches no globals.

- **Tape flourish: wow and flutter.** Real rates (1.1Hz wow, 8.5Hz flutter, deliberately not
  harmonically related) at theatrical depths, applied to the phase step rather than to the smoothed
  `omega` - injected into `omega` the flywheel's own ballistics would filter the flutter away. The wow
  reaches the tape slack at 0.35, because a rate wobble with a rigid tape span reads as the reels being
  wrong rather than the transport being wrong; flutter deliberately does not, being faster than tape
  under tension can follow. The existing spoke-aliasing guard now includes the flourish's peak speed
  multiplier, since that is what a later "make it deeper" tweak would silently spend.
  Two fixture problems found and recorded in the test: firing it with the audio firing sequence gave
  the NO-flourish arm 0.29 of rate spread because the sequence is itself a loud transient, so it fires
  by manual request against constant audio instead; and the first recovery window ended exactly with
  the envelope, so a "recovered" tail still carried 9% of it. Also promoted `flourish::test_guard()`
  out of its own test module - any test touching the process-global request switch needs that lock.
  The eyeball artefact is a rate plot, not a frame: a filmstrip is weak evidence here because three
  spokes are symmetric every 120 degrees. `target/eyeball/review-reel-warble-rate.png`.

- **Scope flourish: the sweep loses trigger lock.** Not a new drawing routine - the trigger is simply
  switched off for 1400ms, so the trace slides about one screen-width of phase and the phosphor smears
  every phase it passes through. It is the family's own documented worst bug, re-entered on purpose.
  Three metrics were tried before one measured the effect: whole-frame luminance difference gave 1.24x
  (mostly phosphor decay, which the flourish does not touch), and the shared `music()` fixture turned
  out not to lock AT ALL because its pseudo-noise manufactures extra zero crossings - 60px of slide
  with the flourish off. On a clean phase-walking tone it is 0px locked against 4px unlocked. A second
  test guards the drift bound and initially passed with the bound REMOVED, because persistence left
  older traces on screen for it to measure; with `fade = 1.0` it now fails at 3px of spread.

- **Nixie flourish: every cathode fires.** The unused digits all glow at once, which is what a
  badly-driven tube does. 520ms - the shortest of any family's, because this display's whole cue is
  WHICH digit is lit, so an effect that lights all ten has to get out of the way fast. The test took
  three attempts to become non-vacuous: "the brightest cell is still the live one" passed a mutation
  to full opacity (the live strike composites last and carries a glow cloud, so it wins regardless),
  and a flat live/ghost ratio was nearly vacuous because the cloud's spill already dominates the
  neighbouring cells. It now measures the share of the live digit's legibility HEADROOM that the
  ghosting eats: 32% shipped, 88% at full opacity, gate at 55%. Also caught myself testing a stale
  file - an earlier timed-out sweep had left the constant at 0.42, so three "mutations" matched
  nothing and silently tested unchanged code. Mutants are now echoed before each run.

- **Valve flourish: gas ionisation.** A cold blue haze through every envelope - the wrong colour for the
  display, which is the entire point, so it is a fixed blue rather than a tint of the theme. Drawn
  BENEATH the cathode glow, which is both easier and more truthful: the gas fills the tube while the
  cathode is a bright source at the plate. Tested as a hue shift (blue/red ratio), not a brightness
  change - a brightness test would pass on any stray glow and miss the only property that makes it read
  as a fault. 1100ms, the longest decay of any family's.
- `38ea1c5` **Waterfall flourish: a broadband tear** written into the history, so it scrolls away as data
  rather than fading as a filter. Three columns wide - one read as merely a brighter column.
- `4839f53` **VFD self-test** (every segment lights, then drains) and **VU needle slam** (needles to the
  end stop over 900ms, OVER lamps lit).
- `22ac2dc` **The flourish trigger.** Rarity judged against the median of recent hits, never an absolute
  threshold. Default measured on a 119-second capture of NINE varied tracks: one flourish per ~30s.
- `4f1f7f8` **One shared onset detector** (`dsp::onset`) with the fixture harness that measures it.
  Vapor and fluid had independently written the same algorithm; tuning unchanged, one behaviour fix (a
  refractory counted in frames rather than milliseconds).
- `eda1f08` **Chroma rework:** perceptual OKLCh ramp (chosen from a six-way sheet), lightbox glow,
  Risograph and Duotone colourways, balanced ink scrambling. **Retired the family's 2.30:1 contrast
  opt-in** - the perceptual ramp clears the project's 3:1 rule at every hue, so no colourway lowers the
  floor any more.
- `1d053f7`, `ff6c11c` **RWR scope** beside the sweep field, with NATO / RU / CN threat libraries.
  Bearing and designator are an emitter's identity, not a reading of the audio - measured, the low-band
  centroid spans 8% of a circle, which is why everything used to land in one quadrant.

---

## Notes to self

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
