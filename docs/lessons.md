# Lessons — things that cost real time to discover

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

