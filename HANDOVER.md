# Taskbar EQ — handover

**As of 2026-07-31.** Branch `feat/taskbar-eq-implementation`. Written because a session
limit was approaching mid-run; this is the resume map.

## Where it stands

**10 of 16 plan tasks complete. 65 tests pass. Release build is warning-free.**

The app builds and does the core job with one theme: it finds the Widgets button, captures
system audio, and draws a VFD-ice segmented meter over the weather while audio plays.

| Task | What | State |
|---|---|---|
| 1 | Scaffold, DPI awareness | done, review clean |
| 2 | `geom::Rect`, visibility rules | done, review clean |
| 3 | Widget rect discovery (UI Automation) | done, review clean, 3 minors deferred |
| 4 | Canvas rasteriser (premultiplied BGRA, bloom) | done, review clean |
| 5 | Layered overlay window | done, review clean — **pixel-verified** |
| 6 | FFT + log band mapping | done, review clean |
| 7 | Ballistics + peak hold | done, review clean |
| 8 | Reveal/hide gate | done, review clean, 0 fix rounds |
| 9 | WASAPI loopback capture | done, review clean — **live-audio verified** |
| 10 | Segmented family + VFD Ice | committed `e1d918b`, **review not yet run** |
| 11 | Tray icon, config, autostart, quit | not started |
| 12 | Remaining 4 segmented colourways | not started |
| 13 | Scope family + 5 phosphors | not started |
| 14 | VU family + 5 backlights | not started |
| 15 | External TOML colourways + schema | not started |
| 16 | Hot reload, theme menu, Win+W | not started |

## What is verified, and how

Distinguishing measured evidence from assumption, because they are not the same thing.

**Objectively verified against real output:**

- **The overlay genuinely composites over your Widgets button.** Confirmed twice by
  independent code. My throwaway spike self-reported 3120 ice-blue pixels (exactly
  26 bars x 5px x 24px — zero blend loss). Task 5's own self-check reported 960 pixels,
  again an exact match for its bar area, with 818 distinct colours ruling out the
  locked-session false negative.
- **WASAPI loopback reads real audio.** `rms=0.0000` while silent, then 0.0242 → 0.0823 →
  0.1211 with distinct L/R channels and `maxband` tracking. Not a loop that compiles and
  reads silence.
- **The widget rect really does move.** Observed at X=1385, 1416 and 1425 in one
  afternoon, always W=190 H=60. The per-second re-discovery is load-bearing.
- **The VFD-ice golden depicts what it should.** 190x60, 25 evenly spaced bars, dormant
  ghost grid above the lit region, blank rows where segment gaps were punched, and the
  `#%%%#` profile showing the dimmer edge with the brighter inset hot core.

**NOT verified — needs your eyes:**

- Whether any of it actually **looks good**. No agent can judge that.
- Whether the bar ballistics *feel* right against real music.
- Whether the reveal/hide timings feel right in practice (400ms in, 2s out).
- Everything in Tasks 11–16, which do not exist yet.

Screen capture returns solid black on a locked session, so anything needing pixels had to
happen while you were logged in. That is why Task 5's verification was front-loaded.

## Resume instructions

```bash
cd C:\Users\cwisset\Documents\projects\taskbar-eq
git log --oneline | head -5          # confirm HEAD
git status --short                   # MUST be clean; see "if dirty" below
cargo test                           # expect 65 passing, 1 ignored
```

The ledger at `.superpowers/sdd/2026-07-30-taskbar-eq/progress.md` is the authoritative
record — trust it and `git log` over anyone's recollection, including mine.

**Next actions in order:**

1. **Review Task 10.** It is committed but its review never ran. Do not treat it as done.
   `bash <superpowers>/skills/subagent-driven-development/scripts/review-package \
      docs/superpowers/plans/2026-07-30-taskbar-eq.md bf4c672 e1d918b`
2. **Run chunk 3 for the rest.** The workflow scripts are reusable:
   `Workflow({scriptPath: ".superpowers/sdd/2026-07-30-taskbar-eq/chunk3.js"})` covers
   Tasks 10–13. Build a `chunk4.js` the same way for 14–16 (copy chunk3.js, swap the
   `TASKS` array and `meta`). **Strip CR characters and non-ASCII control chars from any
   assembled script** or the Workflow tool rejects it.
3. **Final whole-branch review on Opus**, pointed at the deferred-minor list in the ledger.
4. **Then the things only you can do:** run it with music, cycle the themes, and judge.

**If the working tree is dirty on resume:** an agent died mid-task. Read the diff before
anything else — that happened once already at Task 4 and the uncommitted work contained two
genuine panic fixes worth keeping. Do not discard it blind.

## Defects found so far

Recorded because they are the useful output of the review loop, and several were in my own
plan text rather than in anyone's implementation.

**Would have shipped as working-but-wrong:**

- `QUNS_FULLSCREEN`/`QUNS_PRESENTATION` were transposed (6 and 3; real values are 3 and 4,
  and 6 is `QUIET_TIME`). The overlay would have hidden during quiet hours and drawn over
  fullscreen games — exactly backwards, and it compiled and tested fine.
- A **vacuous test**: `output_is_normalised_within_range` asserted `(0.0..=1.0).contains(&v)`
  against a function that clamps to exactly that range. It would have passed against a
  function returning all zeros. Proven vacuous by swapping in a no-op `process()`.
- `debug_assert_eq!` for the FFT-size precondition — compiled out in release, so unchecked
  in the shipping binary.

**Crash vectors:**

- `from_hex` panicked instead of degrading: `len()` is a byte count, so a 6-byte non-ASCII
  string passed the length check then sliced across a UTF-8 char boundary. Reachable from a
  hand-edited theme file — the one input you are meant to author yourself.
- `rounded_rect` panicked in **release** builds: `w.min(h)/2` goes negative for a negative
  dimension and `i32::clamp`'s `min <= max` assertion is unconditional, not debug-only.
- **NaN poisoning** in the ballistics: NaN propagates through a one-pole filter and `clamp`
  does not sanitise it, so one bad sample freezes the meter permanently.

**Resource and API:**

- A **GDI handle leak** on `Overlay::show`'s error path — per frame at 60fps.
- An **unbounded capture→render channel** a stalled consumer could grow without limit.
- A heap allocation per FFT call in the audio hot path (`process()` → `process_with_scratch()`).

**Still outstanding (known, not yet hit):** the plan's Task 11 `Tray::poll()` synthesises
`TrayEvent::Quit` on right-click, which would quit the app when you right-click the tray
icon. My ruling is in the ledger and in chunk3.js's Task 11 briefing — poll() must record a
right-click flag only, and Quit must come solely from the menu returning `ID_QUIT`.

## Verified API facts

Fifteen hard-won facts are in the `GOTCHAS` block of
`.superpowers/sdd/2026-07-30-taskbar-eq/chunk3.js` and are passed to every agent. The ones
that cost the most time:

1. `AC_SRC_ALPHA`/`AC_SRC_OVER`/`BLENDFUNCTION` are in `Graphics::Gdi`, not
   `UI::WindowsAndMessaging`.
2. `CoInitializeEx` returns `HRESULT`, not `Result` — `.ok()` is required.
3. `IMMDevice::Activate` needs the `Win32_System_Com_StructuredStorage` and
   `Win32_System_Variant` features or it silently does not exist.
4. `DPI_AWARENESS_CONTEXT` is an opaque `*mut c_void`. A real v2 context read back as
   `0x22` against a `-4` sentinel, so never compare raw values. `AreDpiAwarenessContextsEqual`
   *does* discriminate v1 from v2 (measured — contradicting a review that claimed otherwise).
5. Verify screen output from **inside** the process that drew it. Sampling from a separate
   PowerShell process produced 12 byte-identical readings because `Add-Type` startup
   outlasted the overlay's hold window.
