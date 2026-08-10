# Working log

Kept current and pushed with every change, so progress is visible without reading the whole commit
history. Newest first within each section. Commit hashes link the claim to the evidence.

**Last updated:** after the tape wow-and-flutter flourish.

---

## In progress

- [ ] **Per-family flourishes, two remaining.** Seven are done (VFD self-test, VU needle slam,
      waterfall tear, valve ionisation, nixie ghosting, scope trigger loss, tape wow/flutter). Still to
      do, each needing its own effect, a pixel-level test and a render: **Pantone plate misregister,
      patchbay**. One commit per family, pushed once verified.

## Waiting on you

- [ ] **Look at the review sheet** (not written yet - it will collect every render that needs an eye in
      one place, so Monday is a single pass rather than thirteen).
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
