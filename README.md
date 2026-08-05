# Taskbar EQ

A real-time audio visualiser that overlays the Windows 11 Widgets (weather) button while audio
is playing, and hands the weather back when it stops.

Single portable `taskbar-eq.exe` — no installer, no admin, no runtime.

---

## Install

There is no installer. It is one file. But downloading an unsigned exe from the internet does
involve two Windows prompts, so here is the whole path.

### 1. Download

**[dist/taskbar-eq.exe](https://raw.githubusercontent.com/cwissett-hub/taskbar-eq/main/dist/taskbar-eq.exe)**

Put it wherever you like — Desktop, Documents, anywhere. It does not need to be in
`Program Files` and does not need admin.

### 2. Expect Windows to complain, because the exe is not code-signed

This is normal for a hobby binary and not a sign anything is wrong. Two things may happen:

- **Edge/Chrome:** the download may be flagged. Choose *Keep* / *Keep anyway*.
- **On first run:** "Windows protected your PC" (SmartScreen). Click **More info**, then
  **Run anyway**.

If you would rather not click through that, build it from source instead — see the bottom of
this page. Signing it properly needs a paid code-signing certificate, which is why the
prebuilt binary is unsigned.

### 3. Run it

Double-click. Nothing visibly happens until audio plays — that is expected.

- **Play something** and watch the weather widget on your taskbar.
- It appears about **400 ms after audio starts** and hides about **2 s after it stops**. Both
  delays are deliberate: the first stops notification dings blanking your weather, the second
  stops it strobing between tracks.
- A **tray icon** appears immediately.

### 4. Optional: start it with Windows

Right-click the tray icon → **Start with Windows**. That writes a single value under
`HKCU\Software\Microsoft\Windows\CurrentVersion\Run` — user-level only, no admin, and
un-ticking it removes the value.

### Uninstall

Delete the exe. If you enabled autostart, untick it first (or delete the `TaskbarEQ` value
from that Run key). Settings live in `%APPDATA%\taskbar-eq\` — delete that folder too if you
want it gone completely. Nothing is written anywhere else and nothing is registered.

### Requirements

- **Windows 11** — the overlay targets the Widgets (weather) button.
- **Windows 10** should work via a fallback that anchors beside the tray's overflow chevron,
  because Win10 has no Widgets button. **This path is untested** — see Known gaps.
- Any audio output, including virtual devices. It captures whatever your default output is
  playing, so Spotify, YouTube, Teams, anything.

---

## Using it

| Action | What it does |
|---|---|
| **Right-click the equaliser** | Theme menu — colourways grouped into a submenu per family, following your Windows light/dark setting |
| **Left-click the equaliser** | Opens the Widgets panel (sends `Win+W`), so the weather stays reachable while covered |
| **Right-click the tray icon** | Same theme menu, plus Start-with-Windows and Quit |

**Quit from the tray icon.** That is the only quit path by design: when nothing is playing the
overlay does not exist, so the tray icon is all that is left to click.

---

## Status

**Last updated: 2026-08-05.** Full test suite green (199 at the time of writing), release
build warning-free. The colourway and family counts below are asserted by a test; the test
count itself is a snapshot and can drift.
**32 colourways across 5 families.**

| | Feature | State |
|---|---|---|
| ✅ | Overlay tracks the Widgets button (its rect moves as the weather text changes) | working |
| ✅ | Windows 10 / no-Widgets fallback, anchored beside the overflow chevron | **untested** |
| ✅ | WASAPI loopback capture, follows default-device changes | working |
| ✅ | dB-scaled spectrum — 2048-pt FFT, 64 log bands, bass-compensating tilt | working |
| ✅ | Reveal/hide gate, 4.5 s hide delay, 450 ms cross-fade | working |
| ✅ | **Segmented VFD — 5 colourways** | working |
| ✅ | **Oscilloscope — 9 colourways**, triggered sweep, auto-ranged gain, persistence incl. dual-layer P7 | working |
| ✅ | **VU dials — 8 colourways**, twin needles, dB-mapped, ~300 ms ballistics | working |
| ✅ | **Vaporwave grid — 5 colourways**, terrain from the spectrum, bass-triggered lightning | **unseen** |
| ✅ | **Valve row — 5 colourways**, per-band cathode glow inside the glass | **unseen** |
| ✅ | Theme menu: per-family submenus, follows the Windows light/dark setting | **unseen** |
| ✅ | Tray icon, start-with-Windows, clean quit | working |
| ✅ | Right-click equaliser → theme menu; left-click → `Win+W` | working |
| ✅ | External TOML colourways, versioned schema, override-by-id, `[vaporwave]` + `[tube]` tables | working |
| ✅ | **Hot reload** — save a theme file and the taskbar updates, no restart | working |
| ✅ | Frame-rate-independent animation (`dt_ms`), so scroll and the gate's timings do not drift with load | working |
| ✅ | **Wide display** — claims the dead taskbar left of the widget, clamped to real clearance | **unseen** |
| ✅ | Layouts scale with width: 4 VU dials and 20 valves at 380 px, 2 and 10 at 190 px | **unseen** |

### Width

The display defaults to **380 px** — roughly double the ~190 px the Widgets button occupies —
extending *leftward* into the empty taskbar between your last pinned app and the widget. On the
development machine that gap is 352 px; on the other side there are only 15 px before "Show
Hidden Icons", which is why it grows left and not right.

Set `width` in `%APPDATA%\taskbar-eq\config.toml` (physical pixels) to change it. It is a
**request, not a guarantee**: the overlay receives its own clicks — it deliberately does not set
`WS_EX_TRANSPARENT`, or right-click and left-click would pass through — so every pixel it covers
is a pixel of taskbar that can no longer be clicked. It therefore measures the clearance to the
nearest element on the same taskbar row every second and clamps itself to fit, keeping 8 px
clear. Open enough windows and it shrinks; fill the taskbar completely and it falls back to
exactly the widget's own rect rather than covering a pinned button.

Layouts scale rather than stretch, because at 60 px tall some of them cannot simply be scaled up:

- **VU dials** — 2 at 190 px (left/right channel), 4 at 380 px. A dial's arc apex sits near the
  top of the panel, so a radius derived from width alone leaves the canvas: at 380 px it computed
  to 112 on a 60 px panel and the arc, ticks and scale all vanished, leaving two bare needle
  lines. Height caps the radius, so extra width buys extra dials. Dials 0 and 1 are always the
  stereo pair; the rest are frequency bands, the way a console carries a stereo pair plus band
  meters. Each dial carries a silkscreen label — `L`, `R`, then `LO`/`HI` — because unlabelled
  dials give no clue that two of them are channels and two are bands.
- **Valve row** — 10 at 190 px, 20 at 380 px, each valve the size it was tuned at. A fixed count
  stretched to a 37 px pitch with 20 px glass, which read as arched windows rather than valves.
- **Segmented, oscilloscope and vaporwave** scale directly and gain from the room.

### Known gaps

- **Nothing has been tested on Windows 10 or on a second machine.** The chevron fallback
  exists precisely because Win10 has no Widgets button, but that path is reasoning from code,
  not evidence. If nothing appears, run `tools/probe/Probe-Taskbar.ps1` there and send me the
  output — the element names will say exactly what to match.
- **The dark menu uses two undocumented uxtheme calls** (ordinals 135/136). There is no
  documented way to dark-mode a Win32 menu. If either ordinal is missing on your build the
  menu silently stays light — deliberately, since a light menu is a cosmetic flaw and a crash
  is not an acceptable price for avoiding it.
- **Hidden-line removal in the vaporwave family is inert at the shipped settings.** It only
  bites when the perspective term packs grid lines tighter than the audio lift can separate
  them; the `persp` needed for legibility at 60 px removes that condition. Measured: `persp`
  1.4 changes 0 pixels, 2.07 changes 83. Kept for the wider variant and for theme files that
  raise `persp`.
- Until 2026-08-05 the TOML parser **rejected `family = "tube"` and `family = "vapor"`** even
  though this README documented both, so neither of the newest families could be authored or
  tuned from a file. Fixed, and the parser now reads the renderer's own family list so it cannot
  fall behind again. If you wrote a theme file for either family before then, it was being
  skipped with a warning.
- The width is **clamped by what UI Automation reports**, so an element it cannot see is an
  element the overlay may cover. Every named taskbar element on the test machine was accounted
  for, but this has not been tried on a taskbar with third-party shell extensions.
- Theme *aesthetics* at 190×60 and 380×60 are not verified by anything automated. Every family has an
  `#[ignore]`d dump harness (`cargo test --release dump_ -- --ignored`) that writes raw RGBA
  for eyeballing, because "does this look like a smear" is not a question a golden can answer.
  See [HANDOVER.md](HANDOVER.md) for the full measured-vs-assumed split.

---

## Themes

**32 colourways across 5 families.** A *family* is a renderer with fixed geometry — code. A
*colourway* is data. That split is the extensibility seam: new colourways need no rebuild.

**Segmented VFD** — a smoked-glass panel with discrete stacked segments, a faint dormant grid,
peak-hold caps and a per-segment halo.

| Colourway | Character |
|---|---|
| VFD Ice | Hi-fi vacuum-fluorescent ice blue, near-white-hot as real VFD phosphor is |
| Matrix Green | Terminal phosphor on near-black, visible dormant grid, fine scanlines |
| Neon Pink | Hot magenta neon on a purple-black panel, heaviest bloom |
| Vac Tube Orange | Warm valve-filament amber, slowest peak fall, filament glow along the bottom |
| Classic Three-Colour | Green while there is headroom, amber when loud, red at the top |

**Oscilloscope** — a triggered sweep on a graticule, with genuine phosphor persistence. The
gain auto-ranges, so the trace uses the full screen at any volume; a scope shows you the
*shape* of the wave, and the VU family is what shows level.

| Colourway | Character |
|---|---|
| P1 green | The reference. Tightest bloom of the set — the others were brought down to match it |
| P7 dual-layer | Blue-white flash over a slower yellow-green tail, genuinely two buffers |
| P11 blue-violet | Pale periwinkle, the photographic phosphor |
| Amber | Warm, slow |
| White-hot | Neutral, brightest |
| MW2 trace | The green readout from the 2009 Modern Warfare 2 reveal trailer — acid chartreuse, crisp, the only scope colourway with scanlines |
| Signal red · Electric azure · Hot magenta | Saturated and punchy, against the five faithful-but-low-key phosphors |

**VU dials** — twin backlit needle dials with a printed arc, a red overload zone and ~300 ms
ballistics. The needle is dB-mapped across [−45, 0] dBFS, because a VU is a dB instrument.

| Colourway | Character |
|---|---|
| Warm cream · Amber · Ice · Green · Red | Vintage panel backlights |
| Neon cyan · Hot pink · Lime | Near-black panels so the needle has something to contrast against |

**Vaporwave grid** — not an instrument but a scene: a slotted sun over a scrolling perspective
grid, the terrain displaced by the spectrum, lightning fired by bass transients. The terrain
auto-ranges against the frame's loudest band, so the hills show the *shape* of the spectrum at any
volume; the lightning deliberately reads the raw signal instead, because it fires on a bass rise
and normalising would partly cancel the very rise it triggers on.

| Colourway | Character |
|---|---|
| Sunset | The tuned reference — magenta sun over a violet grid |
| Miami | Warm orange horizon, cyan grid |
| Outrun | Deep purple sky, hot pink grid |
| Toxic | Acid green, and the calm one: lightning disabled |
| Monochrome | Greyscale, for when the colour is too much |

**Valve row** — a rank of vacuum tubes bolted through a milled chassis, each glowing with its
band. The heaters never go fully out, because a tube that goes black at silence looks broken
rather than quiet. Unlike the oscilloscope and the grid, this family deliberately does **not**
auto-range: it is a level meter, so a quiet passage should look quiet.

| Colourway | Character |
|---|---|
| Soviet lab | Military olive chassis, orange valves — the reference |
| Grey steel | Cold-war steel with white-hot heaters |
| Mercury vapour | The blue rectifier look |
| Bakelite | Domestic radio set — brown, brass, deep amber |
| Nixie green | Matches the Matrix Green VFD |

### Adding your own

Drop a `.toml` file (any filename — the `id` inside is what matters) into
`%APPDATA%\taskbar-eq\themes\` and it appears in the menu **immediately**. The directory is
watched, so saving the file updates the live overlay without a restart — edit a colour, hit
save, and watch the taskbar change.

A file whose `id` matches a built-in **replaces** it; any other `id` is added alongside the 32
built-ins, which are always embedded in the exe regardless of whether that folder exists.

Failure modes are all deliberately soft, because these files are hand-authored:

- **Malformed TOML** — skipped with a warning naming the file; the others still load.
- **An unknown key** — warns and is ignored, so a file written for a later build still works.
- **An unknown `schema` version** — rejected with a message naming both versions.
- **Deleting the theme you had selected** — falls back to the first available one and remembers
  that, rather than pointing at nothing.

See the schema in the prompt below for the exact format: `schema = 1` plus `[colour]`,
`[look]`, `[ballistics]` and optional `[[zone]]` / `[dual]` tables.

One thing worth knowing if you go tuning: **`bloom` is the halo radius, `glow_strength` is its
brightness.** Raising `bloom` expecting more glow makes it *fainter*, because a wider blur
kernel spreads the same energy thinner. That caught me out repeatedly.

---

## Prompt: generate more themes

Paste the block below into any coding agent to have it author new colourways. It is
self-contained — the agent needs no other context about this project.

````text
I need you to author new colourways for Taskbar EQ, a Windows 11 taskbar audio
visualiser. A colourway is data, not code.

THE CANVAS - READ THIS BEFORE CHOOSING COLOURS
  The display is 190 x 60 physical pixels, on the Windows 11 taskbar over the weather
  widget. Three constraints follow, and they matter more than taste:

  1. DARK MODE ONLY. Light mode is not supported. Do not design for it.
  2. THE PANEL IS FULLY OPAQUE - panel_alpha 1.0. Anything less transmits the weather
     text behind it: at 0.96, 4% of white text is ~10 luminance, invisible against a
     lit bar but clearly visible against the dark segment gaps. Do not lower it.
  3. COLOURS ARE EMISSIVE - glowing phosphors and LEDs, not chart fills. They belong
     near maximum lightness. A colourway that "reads gray" is correct here if it is
     meant to look white-hot.

  Hard requirement: every lit colour must reach at least 3:1 contrast against its own
  theme's panel. Compute it (WCAG relative luminance); do not eyeball it. A test will
  fail you otherwise.

  Avoid a hue ramp that merely tracks bar height. Green -> amber -> red IS allowed and
  ships as a built-in, because it encodes headroom (safe / loud / peaking), not
  magnitude.

PICK A FAMILY - a renderer with fixed geometry. You cannot invent one in data.
  segmented  discrete stacked segments on a glass panel, dormant grid, peak-hold caps
  scope      a triggered oscilloscope trace with a graticule and phosphor persistence
  vu         two analogue needle dials with a printed arc and a red overload zone
  vapor      a sunset over a scrolling perspective grid; the grid carries the audio
  tube       a row of vacuum tubes, each glowing with its band inside the glass

  An unknown family name is NOT an error - it falls back to `segmented` and logs a
  warning. So a typo does not fail loudly; check the family name spelling.

FILE FORMAT - one `.toml` file per theme, saved under `%APPDATA%\taskbar-eq\themes\`
(filename does not matter; `id` inside is the identity, and the override key - a file
whose `id` matches one of the 32 built-ins REPLACES it, any other `id` is added):

  schema = 1
  id     = "my-theme"
  name   = "My Theme"
  family = "segmented"

  [colour]
  lit         = "#..."
  hot         = "#..."
  panel       = "#..."
  panel_alpha = 1.0
  edge        = "#..."
  edge_alpha  = 0.15

  [look]
  ghost         = 0.11
  bloom         = 5.0
  glow_strength = 0.35
  edge_glow     = 4.0
  fade          = 0.30
  sensitivity   = 1.0
  texture       = "glass"

  [ballistics]
  attack    = 0.55
  decay     = 0.11
  peak_fall = 0.005

  # optional, repeatable - see "zones" below
  [[zone]]
  upto = 0.5
  lit  = "#..."
  hot  = "#..."

  # optional, scope family only - see "dual" below
  [dual]
  trail = "#..."
  fade  = 0.20

  # optional, vapor family only. Every key optional; these are the shipped defaults,
  # which are NOT the browser-tuner values - see "the 60px problem" below.
  [vaporwave]
  horizon      = 0.48   # fraction of panel height
  amp          = 0.55   # terrain displacement scale
  lines        = 12     # receding horizontal grid lines
  verts        = 18     # converging verticals
  scroll       = 1.24
  persp        = 1.40   # depth-spacing exponent
  spread       = 1.50   # width spread of the near edge
  glow         = 0.98   # peak-glow brightness
  smoothing    = 0.65   # spectral smoothing; higher = rolling hills, not spikes
  sun          = 0.83
  slots        = 6      # horizontal gaps cut in the sun
  slot_bias    = 0.0    # slot widening toward the horizon
  slot_top     = 0.18
  halo         = 0.84
  warmth       = 0.63
  bolt_sens    = 0.55   # rise in bass needed to fire lightning
  bolt_bright  = 0.90   # set 0.0 to disable lightning entirely
  sky_flash    = 0.35
  grid_flash   = 0.60
  bolt_decay   = 0.55
  occlusion    = true
  crisp        = true
  sun_rim      = true
  sky_top      = "#1a0b2e"
  sky_horizon  = "#ff5f93"
  ground       = "#12061f"
  sun_crown    = "#fff6d0"
  sun_upper    = "#ffd76e"
  sun_lower    = "#ff9c4a"
  sun_base     = "#ff5f93"

  # optional, tube family only. A valve is several materials, none of which is a
  # variation of the accent colour, so they are set independently.
  [tube]
  chassis_top    = "#3c4436"
  chassis_bottom = "#161a12"
  internals      = "#0b0d08"   # plate metal, silhouetted against the glow - keep it DARK
  socket         = "#241a10"   # bakelite
  collar         = "#8a6a2a"   # brass
  glass          = "#cfe0d8"   # specular highlight

Every field below is optional except `schema`/`id`/`name`/`family` - anything you omit
takes the documented default, so a minimal file is valid. Unknown keys and unknown
`texture` values are ignored rather than rejected, so a file written for a later
version of this schema still loads.

FIELDS
  schema        always `1` - the version of this file format. A newer number than the
                app understands is rejected outright (with a message naming both
                numbers), not silently reinterpreted.
  id            kebab-case, stable, unique. Also the override key - see FILE FORMAT.
  name          shown in a context menu, so keep it short.
  family        segmented | scope | vu | vapor | tube

  [colour]
  lit           the main emissive colour
  hot           the brighter core. Usually `lit` pushed toward white, not a new hue.
  panel         the display panel. Near-black, tinted toward `lit`'s hue - this is
                what makes each theme feel like its own device rather than a recolour.
  panel_alpha   1.0. See constraint 2.
  edge          1px bezel line;  edge_alpha  0.10-0.25

  [look]
  ghost         alpha of the unlit dormant grid. 0 hides it; 0.17 is clearly visible.
  bloom         halo RADIUS in px, NOT brightness. Must stay small relative to the 7px
                bar pitch - at 16 the halos of adjacent bars merged into one wash
                sitting behind the segments. 3-8 is the usable range.
  glow_strength halo brightness. THIS is the knob for "more glow", not bloom. ~0.35
                gives a tight visible halo; above ~0.7 the bars merge together.
  edge_glow     a dim halo masked to the display's edge ring, as a multiple of
                glow_strength. ~4.0 reads as the bezel catching light. 0.3 measured
                DARKER than the panel it sat on, so do not go low.
  fade          cross-fade duration in seconds when switching to this theme at
                runtime. 0.30 is the shipped default; unrelated to `[dual].fade` below.
  texture       glass | scanlines | haze | filament | grille | none
                glass=lit top edge, scanlines=CRT lines, haze=neon radial glow,
                filament=warm pool along the bottom, grille=fine vertical lines

  [ballistics]
  attack/decay  0-1 per frame. decay MUST be lower than attack - fast attack with slow
                decay is what makes a meter feel right. Never set them equal.
  peak_fall     how fast peak-hold marks sink. Small = slow.

  [[zone]]      optional, repeatable table array, for meters that change colour by
                height (see the classic three-colour built-in). Each has upto/lit/hot;
                upto values must ascend and the last one must be >= 1.0.

  [dual]        optional, scope family only - a second, slower-fading phosphor layer
                behind the trace (real P7 tubes work this way: a blue-white flash over
                a lingering yellow-green afterglow).
  trail         hex colour of the afterglow layer.
  fade          0-1 per frame, how fast the afterglow decays. Small = slow. Distinct
                from `[look].fade` (the theme cross-fade) above.

MEASURED REFERENCE - the five shipped colourways, so you can anchor your numbers
rather than guess. "ratio" is the luminance of a lit segment divided by the gap
BETWEEN adjacent bars, measured at 75% level. Below about 2.2 the bars stop reading
as separate; above about 9 there is no visible halo at all. Aim for 4-8.

  theme                  bloom  glow_str  edge_glow  ghost  texture     ratio
  vfd-ice                  4.0      0.35        4.0   0.11  glass        5.91
  matrix-green             5.0      0.35        4.0   0.17  scanlines    5.97
  neon-pink                6.0      0.35        4.0   0.09  haze         7.88
  vac-tube-orange         12.0      0.35        4.0   0.13  filament     4.21
  classic-three-colour     7.0      0.35        4.0   0.13  grille       5.66

  Two things that table teaches, both of which are counter-intuitive and were both
  learned the hard way:

  * A BIGGER bloom radius makes the halo FAINTER, not stronger. The blur normalises
    by kernel size, so a wider radius spreads the same energy across far more pixels.
    neon-pink at radius 14 measured as the faintest of the five; dropping it to 6 made
    it brighter. If you want more glow, raise glow_strength, never bloom.
  * The texture affects the measurement. vac-tube-orange reads as the strongest halo
    partly because `filament` brightens the lower panel, which lifts the gap reading.
    Do not chase the ratio number alone.

WHAT I WANT
  <describe: how many, what mood, which families, any reference hardware>

FOR EACH THEME, TELL ME
  1. The finished `.toml` file in the FILE FORMAT above, ready to drop into
     `%APPDATA%\taskbar-eq\themes\`.
  2. The computed contrast ratio of `lit` against `panel`, as a number.
  3. Which shipped colourway in the table above yours sits closest to, and why you
     departed from it.
  4. One sentence on what real device it imitates. If you cannot name one, the theme is
     probably an arbitrary hue shift - I would rather have fewer, more deliberate themes
     than more generic ones.
````

---

## Build from source

Requires the [Rust stable MSVC toolchain](https://rustup.rs). No other dependencies.

```
git clone https://github.com/cwissett-hub/taskbar-eq
cd taskbar-eq
cargo build --release        # -> target/release/taskbar-eq.exe
cargo test                   # the full suite
```

Building yourself also sidesteps the SmartScreen prompt entirely.

Goldens under `tests/golden/` are ASCII luminance maps rather than PNGs, so a rendering change
shows up as a readable diff and needs no image dependency. The catch: a golden regenerated
from a broken renderer locks the bug in, so read any golden before committing it.

`tools/probe/` holds read-only PowerShell scripts for re-measuring taskbar geometry on another
machine — useful if the overlay lands in the wrong place.
