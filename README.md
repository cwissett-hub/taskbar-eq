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
| **Right-click the equaliser** | Theme menu |
| **Left-click the equaliser** | Opens the Widgets panel (sends `Win+W`), so the weather stays reachable while covered |
| **Right-click the tray icon** | Same theme menu, plus Start-with-Windows and Quit |

**Quit from the tray icon.** That is the only quit path by design: when nothing is playing the
overlay does not exist, so the tray icon is all that is left to click.

---

## Status

**Last updated: 2026-07-31.** 102 tests passing, release build warning-free.

| | Feature | State |
|---|---|---|
| ✅ | Overlay tracks the Widgets button (its rect moves as the weather text changes) | working |
| ✅ | Windows 10 / no-Widgets fallback, anchored beside the overflow chevron | **untested** |
| ✅ | WASAPI loopback capture, follows default-device changes | working |
| ✅ | dB-scaled spectrum — 2048-pt FFT, 64 log bands, bass-compensating tilt | working |
| ✅ | Reveal/hide gate with blip rejection | working |
| ✅ | **Segmented family — 5 colourways** | working |
| ✅ | Tray icon, theme menu, start-with-Windows, clean quit | working |
| ✅ | Right-click equaliser → theme menu; left-click → `Win+W` | working |
| 🔜 | Oscilloscope family — 5 phosphors, persistence trace | in progress |
| 🔜 | Analogue VU family — 5 dial backlights, twin needles | in progress |
| 🔜 | External TOML colourways with a versioned schema | queued |
| 🔜 | Hot reload of theme files | queued |
| 📋 | Vaporwave grid family ([specced](docs/superpowers/specs/2026-07-31-vaporwave-grid-family-design.md)) | needs 6 new Canvas primitives |

### Known gaps

- **Nothing has been tested on Windows 10 or on a second machine.** The chevron fallback
  exists precisely because Win10 has no Widgets button, but that path is reasoning from code,
  not evidence. If nothing appears, run `tools/probe/Probe-Taskbar.ps1` there and send me the
  output — the element names will say exactly what to match.
- Theme *aesthetics* at 190×60 are not verified by anything automated. See
  [HANDOVER.md](HANDOVER.md) for the full measured-vs-assumed split, which is kept honest
  deliberately.

---

## Themes

Five colourways ship built in, all in the **segmented** family — a smoked-glass panel with
discrete stacked segments, a faint dormant grid, peak-hold caps and a per-segment halo.

| Colourway | Character |
|---|---|
| VFD Ice | Hi-fi vacuum-fluorescent ice blue, near-white-hot as real VFD phosphor is |
| Matrix Green | Terminal phosphor on near-black, visible dormant grid, fine scanlines |
| Neon Pink | Hot magenta neon on a purple-black panel, heaviest bloom |
| Vac Tube Orange | Warm valve-filament amber, slowest peak fall, filament glow along the bottom |
| Classic Three-Colour | Green while there is headroom, amber when loud, red at the top |

Two more families are designed and next: an **oscilloscope** (phosphor trace with genuine
persistence, including the dual-layer P7 whose fading tail is a different colour from its
trace) and an **analogue VU** (twin backlit needle dials with ~300 ms ballistics).

### Adding your own

Once the TOML work lands, dropping a `.toml` file in `%APPDATA%\taskbar-eq\themes\` will make
it appear in the menu, hot-reloaded. Until then colourways live in `src/themes/builtin.rs`.

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
  scope      an oscilloscope trace with a graticule and phosphor persistence
  vu         two analogue needle dials with a printed arc and a red overload zone

FIELDS
  id            kebab-case, stable, unique. Also the filename.
  name          shown in a context menu, so keep it short.
  family        segmented | scope | vu
  lit           the main emissive colour
  hot           the brighter core. Usually `lit` pushed toward white, not a new hue.
  panel         the display panel. Near-black, tinted toward `lit`'s hue - this is
                what makes each theme feel like its own device rather than a recolour.
  panel_alpha   1.0. See constraint 2.
  edge          1px bezel line;  edge_alpha  0.10-0.25
  ghost         alpha of the unlit dormant grid. 0 hides it; 0.17 is clearly visible.
  bloom         halo RADIUS in px, NOT brightness. Must stay small relative to the 7px
                bar pitch - at 16 the halos of adjacent bars merged into one wash
                sitting behind the segments. 3-8 is the usable range.
  glow_strength halo brightness. THIS is the knob for "more glow", not bloom. ~0.35
                gives a tight visible halo; above ~0.7 the bars merge together.
  edge_glow     a dim halo masked to the display's edge ring, as a multiple of
                glow_strength. ~4.0 reads as the bezel catching light. 0.3 measured
                DARKER than the panel it sat on, so do not go low.
  attack/decay  0-1 per frame. decay MUST be lower than attack - fast attack with slow
                decay is what makes a meter feel right. Never set them equal.
  peak_fall     how fast peak-hold marks sink. Small = slow.
  texture       glass | scanlines | haze | filament | grille | none
                glass=lit top edge, scanlines=CRT lines, haze=neon radial glow,
                filament=warm pool along the bottom, grille=fine vertical lines
  zones         optional, for meters that change colour by height (see the classic
                three-colour built-in). Ascending, last one >= 1.0.

WHAT I WANT
  <describe: how many, what mood, which families, any reference hardware>

FOR EACH THEME, TELL ME
  1. Every field value.
  2. The computed contrast ratio of `lit` against `panel`, as a number.
  3. One sentence on what real device it imitates. If you cannot name one, the theme is
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
cargo test                   # 102 tests
```

Building yourself also sidesteps the SmartScreen prompt entirely.

Goldens under `tests/golden/` are ASCII luminance maps rather than PNGs, so a rendering change
shows up as a readable diff and needs no image dependency. The catch: a golden regenerated
from a broken renderer locks the bug in, so read any golden before committing it.

`tools/probe/` holds read-only PowerShell scripts for re-measuring taskbar geometry on another
machine — useful if the overlay lands in the wrong place.
