# Taskbar EQ

A real-time audio visualiser that overlays the Windows 11 Widgets (weather) button while audio
is playing, and hands the weather back when it stops.

Single portable `taskbar-eq.exe` — no installer, no admin, no runtime. Copy it to any Windows
machine and run it.

**Status:** design approved, implementation not started. See
[the design spec](docs/superpowers/specs/2026-07-30-taskbar-eq-design.md).

---

## Themes

15 colourways ship built into the binary, across three families:

| Family | Look | Colourways |
|---|---|---|
| `segmented` | Smoked-glass panel, discrete segments, ghost grid, peak-hold caps | VFD Ice · Matrix Green · Neon Pink · Vac Tube Orange · Classic Three-Colour |
| `scope` | Phosphor CRT trace on a graticule, real persistence trails | P1 green · P7 dual-layer · P11 blue-violet · Amber · White-hot |
| `vu` | Twin backlit needle dials, printed arc, red overload zone | Warm cream · Amber · Ice blue · Green · Red |

Switch themes by right-clicking the overlay, or from the tray icon.

### Adding your own

Drop a `.toml` file in `%APPDATA%\taskbar-eq\themes\`. It appears in the menu immediately —
the directory is watched, so saving the file updates the live overlay without a restart.
Giving your theme the same `id` as a built-in overrides that built-in.

A malformed file is skipped with a warning. It will not crash the app or affect other themes.

---

## Prompt: generate more themes

Paste the block below to any coding agent to have it author new colourways. It is
self-contained — the agent needs no other context about this project.

````text
I need you to author new colourway files for Taskbar EQ, a Windows 11 taskbar audio
visualiser. A colourway is a TOML file; you are writing data, not code.

WHERE THEY GO
  %APPDATA%\taskbar-eq\themes\<id>.toml
  The directory is hot-reloaded, so saving a file updates the live overlay.
  Using the same `id` as a built-in overrides that built-in.

THE CANVAS — READ THIS BEFORE CHOOSING COLOURS
  The visualiser is 190 x 60 physical pixels, sitting on the Windows 11 taskbar,
  over the weather widget. Three constraints follow from that, and they matter
  more than taste:

  1. DARK MODE ONLY. Light mode is not supported. Do not design for it.
  2. THE BACKGROUND IS NOT BLACK AND NOT CONSTANT. Windows 11 acrylic tints the
     taskbar from the user's wallpaper. On the reference machine it measured
     #3D1712 — a warm reddish-brown. It will differ on other wallpapers. This is
     why every theme draws its own near-black panel: the panel provides a
     predictable black level so the theme does not depend on the wallpaper.
     Keep `panel_alpha` at 0.55 or above, or the acrylic washes the theme out.
  3. COLOURS ARE EMISSIVE. These are glowing phosphors, LEDs and needles, not
     chart fills. They should sit near maximum lightness. A colourway that
     "reads gray" is correct here if it is meant to look white-hot.

  Hard requirement: every lit colour must reach at least 3:1 contrast against
  the theme's own panel colour. Compute it (WCAG relative luminance) — do not
  eyeball it. This is the one check that is not a matter of preference.

  Avoid a rainbow ramp for bar height. Green -> amber -> red IS allowed, and is
  a built-in, because it encodes headroom (safe / loud / peaking) rather than
  magnitude. A hue ramp that merely tracks height destroys perceptual ordering.

PICK A FAMILY
  Each family is a renderer with fixed geometry and behaviour. You choose which
  one your colourway targets; you cannot invent a new family in a TOML file
  (that needs Rust code).

  family = "segmented"   bars of discrete stacked segments on a glass panel,
                         with a faint dormant grid and peak-hold caps
  family = "scope"       an oscilloscope trace with a graticule and phosphor
                         persistence trails
  family = "vu"          two analogue needle dials with a printed arc and a
                         red overload zone

SCHEMA
  schema = 1                  REQUIRED. Do not invent other values.
  id     = "kebab-case"       REQUIRED, stable, unique. Also the filename.
  name   = "Display Name"     REQUIRED. Shown in the menu; keep it short, it
                              sits in a context menu.
  family = "segmented"        REQUIRED. One of the three above.

  [colour]
  lit         = "#RRGGBB"   the main emissive colour
  hot         = "#RRGGBB"   the brighter core / highlight. Usually `lit` pushed
                            toward white, not a different hue.
  panel       = "#RRGGBB"   the display panel. Near-black, tinted toward `lit`'s
                            hue. This is what makes each theme feel like its own
                            device rather than a recolour.
  panel_alpha = 0.55        0.55-0.75. Lower lets the wallpaper through; see
                            constraint 2 above.
  edge        = "#RRGGBB"   1px bezel line around the panel
  edge_alpha  = 0.13        0.10-0.25

  [look]                    family-specific; see per-family keys below
  [ballistics]              how the display moves
  attack    = 0.55          0.0-1.0. How fast it rises. High = snappy.
  decay     = 0.11          0.0-1.0. How fast it falls. LOWER THAN ATTACK — fast
                            attack with slow decay is what makes a meter feel
                            right. Do not set these equal.
  peak_fall = 0.0055        how fast peak-hold marks sink. Small values = slow.

PER-FAMILY [look] KEYS

  segmented:
    ghost   = 0.11          alpha of the unlit dormant segment grid. 0 hides it
                            entirely; 0.17 makes a clearly visible grid at rest.
    bloom   = 9.0           glow radius in px. 5 = tight, 14 = heavy neon haze.
    texture = "glass"       glass | scanlines | haze | filament | grille | none
                            glass     = lit highlight along the top edge
                            scanlines = fine horizontal lines (CRT / terminal)
                            haze      = soft radial glow (neon tubing)
                            filament  = warm pool along the bottom (valve heat)
                            grille    = fine vertical lines (hi-fi speaker cloth)

  scope:
    fade = 0.30             persistence. LOW = long trails, HIGH = tight trace.
                            0.11 is a long amber tail; 0.30 is crisp.
    bloom = 6.0

  vu:
    bloom = 5.0
    Note: `hot` is used for the needle, `lit` for the printed ink and the glow
    pool behind the dial.

TWO OPTIONAL TABLES

  Zoned colours — for meters that change colour by height. Repeatable; `upto` is
  a fraction of full height and they must be in ascending order, with the last
  one >= 1.0. Overrides [colour].lit/hot when present.

    [[zone]]
    upto = 0.58
    lit  = "#3ddc5a"
    hot  = "#b6ffc6"

  Dual-layer phosphor — `scope` only. Models a two-layer CRT phosphor where the
  fresh trace is one colour and the decaying tail another (this is how the real
  P7 radar phosphor behaves). Give the trail a much lower `fade` than the main
  trace so it lingers.

    [dual]
    trail = "#cfe86a"
    fade  = 0.055

RULES
  - Unknown keys are ignored with a warning. Missing keys take defaults. So a
    minimal file is valid — but set `panel` and `panel_alpha` explicitly, because
    the defaults are tuned for the ice-blue built-in and will look wrong under
    another hue.
  - One colourway per file. Filename = `id`.
  - Do not emit comments claiming a colour was "validated" unless you actually
    computed the contrast ratio and can state the number.

WHAT I WANT
  <describe here: how many, what mood, which families, any reference hardware or
   palette you want them to evoke>

FOR EACH THEME YOU PRODUCE, TELL ME
  1. The complete TOML file.
  2. The computed contrast ratio of `lit` against `panel`, as a number.
  3. One sentence on what real device or material it is imitating — if you cannot
     name one, the theme is probably an arbitrary hue shift, and I would rather
     have fewer, more deliberate themes than more generic ones.
````

---

## Building

Requires the Rust stable MSVC toolchain.

```
cargo build --release
```

Output: `target/release/taskbar-eq.exe`. Copy it anywhere.
