# Reference material

Artefacts from the design phase, kept because they are load-bearing for implementation.

## `mockups/`

Browser mockups from the design conversation. **These are the reference implementation of the
renderers** — every family was worked out here first, animated at true size (190 × 60) against
the real sampled taskbar colour.

Open either file directly in a browser; they are self-contained.

| File | Contains |
|---|---|
| `all-themes.html` | All five `segmented` colourways, plus the three family concepts side by side |
| `colourways.html` | The five `scope` phosphors and five `vu` backlights |

Worth reading before writing the Rust renderers, because several details are non-obvious and
were arrived at by trial:

- **Segment gaps are punched, not drawn.** Each bar is filled as one shadowed rect (cheap bloom —
  one shadow operation per bar rather than per segment), then the segment gaps are removed with
  `destination-out`. Drawing individual shadowed segments is far slower for the same result.
- **The scope's persistence is a real decaying buffer**, not a fixed-length trail. The trace is
  drawn to an offscreen buffer that is faded with `destination-out` each frame, which keeps it
  transparent so the panel shows through. Clearing and redrawing N past frames does not look the
  same.
- **P7 dual-layer needs two buffers** at different fade rates — a fast blue-white trace over a
  slow yellow-green trail.
- **VU ballistics are asymmetric and slow** (~0.085 smoothing per frame). Making attack and decay
  equal, or fast, destroys the feel immediately.
- The `sat`/`bright` sliders in the mockups desaturate toward luma rather than scaling channels,
  which is what keeps a phosphor colour looking like a dimmer phosphor rather than a grey one.

Note the mockups run at 33 fps for browser performance; the application targets 60.

## `taskbar-widget.png`

A 190 × 60 capture of the actual Widgets button, taken with `Probe-Colours.ps1`. This is the
real background the overlay sits on — warm reddish-brown (`#3D1712`), not grey, because Windows
11 acrylic tints the taskbar from the wallpaper.

## `../../tools/probe/`

The two read-only measurement scripts used to derive the geometry in the spec. Keep them: they
are how you re-measure on a different machine, and `Probe-Taskbar.ps1` in particular is the
quickest way to check whether the Widgets button can still be found by UI Automation after a
Windows update.

`Probe-Colours.ps1` demonstrates the DPI trap the spec calls out — it deliberately calls
`SetProcessDPIAware()` before capturing, because without it the coordinates are virtualised and
the capture lands in the wrong place.
