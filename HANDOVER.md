# Taskbar EQ — status and handover

A real-time audio visualiser that overlays the Windows 11 Widgets (weather) button while
audio is playing, and hands the weather back when it stops.

Single portable `taskbar-eq.exe` — no installer, no admin, no runtime.

---

## What works

**Twelve of sixteen planned tasks, plus a substantial round of visual fixes.**
102 tests pass; release builds are warning-free.

- Finds the Widgets button by UI Automation and tracks it — the rect genuinely moves as the
  weather text changes width, so it is re-probed every second.
- Falls back to anchoring beside the tray's overflow chevron when there is no Widgets
  button, which is what makes it work on Windows 10.
- Captures system audio via WASAPI loopback, follows default-device changes.
- 2048-point FFT, 64 log-spaced bands, dB-scaled with a bass-compensating tilt.
- Appears 400 ms after audio starts and hides 2 s after it stops, so notification dings do
  not blank the weather and it does not strobe between tracks.
- **Five colourways**: VFD Ice, Matrix Green, Neon Pink, Vac Tube Orange, Classic
  Three-Colour. Right-click the equaliser or the tray icon to switch; the choice persists.
- Left-click sends `Win+W` so the weather stays reachable while covered.
- Tray icon with theme menu, start-with-Windows, and quit.

## Not built yet

| Task | What |
|---|---|
| 13 | Oscilloscope family (5 phosphor colourways) — persistence-trace renderer |
| 14 | Analogue VU family (5 dial backlights) — twin needles, ~300 ms ballistics |
| 15 | External TOML colourways with a versioned schema |
| 16 | Hot reload of theme files |
| — | Vaporwave grid family — [specced](docs/superpowers/specs/2026-07-31-vaporwave-grid-family-design.md) with tuned parameters, needs 6 new Canvas primitives |

## Verified vs unverified

Distinguishing measured evidence from assumption, because they are not the same thing.

**Measured against real output:**

- The overlay composites over the widget. Confirmed twice by independent code sampling its
  own pixels: 3120 and 960 ice-blue pixels, each an exact match for the bars drawn, so no
  blend loss. 800+ distinct colours in the capture rules out the locked-session false
  negative (a locked Windows session makes screen capture silently return solid black).
- WASAPI reads real audio: RMS 0.0000 while silent, then 0.024 → 0.082 → 0.121 with
  distinct L/R channels.
- The spectrum no longer pegs. Sampled bar heights went from `[@@@@…]` 26/26 bars at 60/60
  to `[***++-------:-------:::::.]` peaking at 42–45/60 with bass high and treble present.
- Every colourway's `lit` colour clears 3:1 contrast against its own panel — computed via
  WCAG luminance in a test, not eyeballed.

**Not verified — needs a human, or hardware I do not have:**

- Whether any of it actually *looks* good. No automated check substitutes for that.
- Whether the five colourways read correctly at 190×60.
- Whether the chevron fallback lands somewhere sensible. The maths is tested; the
  aesthetics of the resulting position are not.
- **Anything on Windows 10 or on a second machine.** Reasoning from code, not evidence.

## Build and run

```
cargo build --release          # needs the Rust stable MSVC toolchain
target/release/taskbar-eq.exe
```

Quit from the tray icon. There is deliberately no other quit path: when nothing is playing
the overlay does not exist, so the tray icon is the only thing left to click.

## Things that cost real time to discover

Kept because they are the useful output, and several were defects in the plan rather than in
anyone's implementation.

**Would have shipped as working-but-wrong:**

- `QUNS_FULLSCREEN`/`QUNS_PRESENTATION` were transposed (6 and 3; the real values are 3 and
  4, and 6 is `QUIET_TIME`). The overlay would have hidden during quiet hours and drawn over
  fullscreen games. It compiled and tested fine.
- A **vacuous test** asserting `(0.0..=1.0).contains(&v)` against a function that clamps to
  exactly that range — it would have passed against a function returning all zeros. Proven
  vacuous by swapping in a no-op implementation.
- `debug_assert_eq!` for an FFT-size precondition: compiled out in release, so unchecked in
  the shipping binary.
- A test config containing `this is not toml {{{` left behind in `%APPDATA%` by a test,
  which silently forced default settings on every launch — so theme choices never persisted.

**Crash vectors:**

- `from_hex` panicked rather than degrading: `len()` is a byte count, so a 6-byte non-ASCII
  string passed the length check then sliced across a UTF-8 char boundary. Reachable from a
  hand-edited theme file.
- `rounded_rect` panicked in **release**: `w.min(h)/2` goes negative for a negative
  dimension and `i32::clamp`'s assertion is unconditional, not debug-only.
- NaN propagates through a one-pole filter and `clamp` does not sanitise it, so one bad
  sample froze the meter permanently.

**Rendering, all of which looked plausible and were wrong:**

- `Canvas::bloom` composites its halo **under** existing content, so an opaque panel hid the
  glow entirely. Raising `panel_alpha` to fix weather bleed-through is what killed the glow.
- It also scaled the four premultiplied channels independently, each clamping at 255. Since
  RGB ≤ A, alpha saturates first and the result is opaque-but-dark pixels — a black wash
  wherever the halo was strongest.
- Bloom radius must stay small relative to the 7px bar pitch or every halo merges into one
  diffuse mass *behind* the segments. Radius is the wrong lever for "brighter"; strength is.
- `punch_row` zeroes alpha across the **full canvas width**, so using it for segment gaps
  erased the panel too and left transparent stripes with the taskbar showing through.
  Painting the gaps with panel colour is correct; punching them is not.
- Discarding a known-good widget rect on the first UIA miss hid the overlay for a second and
  the real weather showed through — which looked exactly like bleed-through but was absence.

**Verified API facts:**

1. `AC_SRC_ALPHA`/`AC_SRC_OVER`/`BLENDFUNCTION` are in `Graphics::Gdi`, not
   `UI::WindowsAndMessaging`.
2. `CoInitializeEx` returns `HRESULT`, not `Result` — `.ok()` is required.
3. `IMMDevice::Activate` needs the `Win32_System_Com_StructuredStorage` and
   `Win32_System_Variant` features or the method silently does not exist.
4. `DPI_AWARENESS_CONTEXT` is an opaque `*mut c_void`; a real per-monitor-v2 context read
   back as `0x22` against a `-4` sentinel, so never compare raw values.
5. `WS_EX_NOACTIVATE` does **not** make a window click-through — it still receives mouse
   messages. `WS_EX_TRANSPARENT` does.
6. Verify screen output from **inside** the process that drew it. Sampling from a separate
   process produced twelve byte-identical readings because its startup outlasted the
   overlay's hold window.

## Layout

| Path | What |
|---|---|
| `src/dsp/` | FFT, band mapping, ballistics, the reveal/hide gate — all pure, no Windows deps |
| `src/render/` | Canvas rasteriser and the family renderers |
| `src/themes/` | Theme model and the built-in colourways |
| `src/win/` | The thin Win32 edges: DPI, placement, overlay, capture, tray, autostart |
| `docs/superpowers/` | Design specs and the implementation plan |
| `docs/reference/` | Browser mockups that are the reference implementation of each renderer |
| `tools/probe/` | Read-only scripts for re-measuring the taskbar on another machine |
| `tests/golden/` | ASCII luminance maps — a visual regression shows up as a readable diff |

Goldens are ASCII rather than PNG on purpose: a 190×60 canvas becomes 60 readable lines, so
a rendering change is reviewable in a diff and needs no image dependency. The catch is that
a golden generated from a broken renderer locks the bug in, so any regenerated golden must
be opened and read before being committed.
