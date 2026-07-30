# Taskbar EQ — design

**Date:** 2026-07-30
**Status:** approved design, ready for implementation planning

A real-time audio visualiser that overlays the Windows 11 Widgets (weather) button while
audio is playing, and hands the weather back when it stops.

---

## 1. Goal and constraints

Show a graphic equaliser in the taskbar, next to the weather, that moves while music plays.

Windows 11 provides **no supported way to draw inside the taskbar**. Deskbands — the API that
once allowed docked panels — are deprecated and non-functional in Windows 11, and the Widgets
button is not extensible. The only viable approach is a **topmost layered window positioned over
the widget's rect**, tracked at runtime.

This is a limitation to design around, not a defect to work around later.

### Hardware findings (measured, not assumed)

Measured on the target work machine, 2026-07-30:

| Fact | Value | Why it matters |
|---|---|---|
| Machine | Dell Precision 5490 | Determines the keyboard-backlight answer (§9) |
| Screen | 1920 × 1200 physical, 125% scale | Overlay must be DPI-aware |
| Taskbar | 60 px tall physical, left-aligned | Widgets sits at the **right**, not the left |
| Widget rect | X=1385, Y=1140, **W=190, H=60** physical | The available canvas |
| Widget label | `Widgets 20°C Mostly cloudy` | Width is content-driven → must track live |
| Taskbar surface | `#3D1712` (sampled) | **Warm reddish-brown**, not grey |
| `EnableTransparency` | 1 | Acrylic tints the taskbar from the wallpaper |
| Theme | dark (`AppsUseLightTheme=0`, `SystemUsesLightTheme=0`) | Light mode is out of scope |
| System accent | `#D0000C` | Informs the VU "red" colourway |
| Default audio endpoint | `Speakers (2- SoundWire Speakers)` — **virtual** | Capture must follow default-device changes |

**Critical DPI finding.** A DPI-unaware process reads the taskbar via `GetWindowRect` as
1536 × 48 — virtualised, and wrong. UI Automation returns true physical pixels (1920 × 60).
The application MUST set per-monitor-v2 DPI awareness **before creating any window**, or the
overlay lands in the wrong place. The dev machine is at 125% and the personal machine may
differ, so this cannot be verified by eye on one box alone.

### Environment

- **Toolchain:** Rust stable `x86_64-pc-windows-msvc`, already installed. Linker is MSVC
  14.44 from VS 2022 BuildTools. A release build was verified end to end (compiled and ran
  in 4.6 s).
- Chosen over C#/.NET because no .NET SDK is present (runtime only), and because a
  single dependency-free binary suits an always-running overlay.
- **The user is in the local Administrators group but not elevated.** The design deliberately
  requires no elevation at any point.
- **Both target machines are Windows.** macOS and Linux are explicit non-goals (§9).

---

## 2. Placement and visibility

The overlay is a borderless, topmost, per-pixel-alpha layered window (`WS_EX_LAYERED |
WS_EX_TOPMOST | WS_EX_NOACTIVATE`, updated via `UpdateLayeredWindow`).

**Rect discovery.** Locate the Widgets button by walking the `Shell_TrayWnd` UI Automation
subtree for a Button whose automation name begins with `Widgets`. Re-check every 1 s: the
width changes as the weather text cycles (`20°C Mostly cloudy` → `Rain coming In about 1 hour`).

**Hide entirely when any of these hold:**

- The Widgets button is not found (user has the widget turned off, or the tray layout changed).
- A fullscreen application is foreground — detected via `SHQueryUserNotificationState`
  returning `QUNS_RUNNING_D3D_FULL_SCREEN` or `QUNS_PRESENTATION_MODE`. This is the documented
  API and covers games and fullscreen video without heuristics.
- The taskbar is auto-hidden or not visible.

**Primary taskbar only.** The Widgets button does not exist on secondary taskbars, so
multi-monitor support means "follow the primary display's taskbar" — including when that
changes on dock/undock.

When the widget cannot be found, the overlay stays hidden and the tray icon tooltip explains
why. It does not fall back to an arbitrary position — a glowing rectangle in a random part of
the taskbar is worse than nothing.

---

## 3. Auto-reveal behaviour

The weather is visible by default; the EQ appears only while audio plays.

| Transition | Trigger | Rationale |
|---|---|---|
| Reveal | 400 ms of continuous audio above the RMS threshold | An instant trigger means every Teams notification and UI click blanks the weather. 400 ms filters blips without feeling laggy. |
| Hide | 2 s continuously below threshold | Rides through the gap between tracks so it does not strobe through an album. |
| Fade | 250 ms, both directions | |

Default RMS threshold: **−55 dBFS**. Low enough to catch quiet passages, high enough to ignore
dither and idle noise on a virtual endpoint.

While hidden, capture continues but **rendering stops entirely** — the cost when not listening
is effectively zero. Thresholds and timings are configurable (§6).

### DSP parameters

Fixed so the implementation does not have to guess:

| Parameter | Value |
|---|---|
| FFT size | 2048 samples, Hann window |
| Hop | 512 samples (~93 fps at 48 kHz, decoupled from render rate) |
| Internal bands | 64, log-spaced 40 Hz – 16 kHz |
| Spectral tilt | bass-weighted rolloff so the display is not dominated by low-frequency energy |
| Output | 64 band levels, 0.0–1.0, downsampled by the renderer to its own bar count |

The internal band count is deliberately **independent of the bar count**. `segmented` derives
its geometry from the live rect (5 px bar + 2 px gap → ~25 bars at 190 px width), so the rect
changing width must not change the DSP. `scope` and `vu` ignore bands entirely — `scope` needs
the time-domain waveform and `vu` needs per-channel RMS, so `dsp` exposes all three outputs.

---

## 4. Theme system

**The central extensibility requirement: themes must be expandable without a rebuild.**

The seam is: **colourways are data, families are code.** A colourway is pure parameters.
A family is a renderer that owns per-frame *state* — persistence buffers, ballistics
integrators, peak arrays — which cannot be expressed as data without embedding a scripting
engine.

### Families (code)

Each implements a `Family` trait; adding one is a new file plus a single registration line,
touching no existing family.

| Family | Renders | Owns state |
|---|---|---|
| `segmented` | Smoked-glass panel, discrete stacked segments, dormant ghost grid, peak-hold caps | per-band level + peak arrays |
| `scope` | Phosphor CRT trace on a graticule, genuine persistence trails | persistence buffer(s) |
| `vu` | Twin backlit needle dials, printed arc, red overload zone, ghost peak needles | ~300 ms ballistics integrators |

### Colourways (data) — 15 shipped

| Family | Colourways |
|---|---|
| `segmented` | VFD Ice · Matrix Green · Neon Pink · Vac Tube Orange · Classic Three-Colour |
| `scope` | P1 green · P7 dual-layer · P11 blue-violet · Amber · White-hot |
| `vu` | Warm cream · Amber · Ice blue · Green · Red |

Two colourways carry **behavioural** exceptions, not just different hex values — the schema
must accommodate both:

- **P7 dual-layer** is a genuine two-layer phosphor: a blue-white flash on the fresh trace
  decaying through a long yellow-green tail. Implemented as two persistence buffers at
  different fade rates.
- **VU Red** flips its overload arc to white, because red-on-red is illegible.

Ice blue and Green are intentionally shared across families so themes read as one system.

### Loading and precedence

1. Built-in colourways are **embedded in the binary** — a bare copy of the exe has all 15 and
   stays portable.
2. Files in `%APPDATA%\taskbar-eq\themes\*.toml` are loaded after, and either add a new
   colourway or **override a built-in with the same `id`**.
3. A malformed file logs a warning, is skipped, and does not affect the others or crash the app.
4. **Hot reload:** the themes directory is watched; saving a file updates the live overlay
   without a restart.

### Schema (versioned from the first commit)

`schema = 1`. Unknown keys warn and are ignored; missing keys take documented defaults; an
unknown `schema` value is rejected with a clear message. This is what keeps a theme file
written later from breaking when fields are added.

```toml
schema = 1
id     = "vfd-ice"          # stable key; matching a built-in id overrides it
name   = "VFD Ice"          # menu label
family = "segmented"        # segmented | scope | vu

[colour]
lit         = "#8fe4ff"
hot         = "#e4f8ff"
panel       = "#040a0e"
panel_alpha = 0.55
edge        = "#96e1ff"
edge_alpha  = 0.13

[look]                      # family-specific; each family documents what it reads
ghost   = 0.11
bloom   = 9.0
texture = "glass"           # glass | scanlines | haze | filament | grille | none

[ballistics]
attack    = 0.55
decay     = 0.11
peak_fall = 0.0055
```

Zoned colourways (Classic Three-Colour) add repeated `[[zone]]` tables:

```toml
[[zone]]
upto = 0.58
lit  = "#3ddc5a"
hot  = "#b6ffc6"
```

Scope colourways add `fade`, and optionally a `[dual]` table for the P7 case:

```toml
[look]
fade = 0.30

[dual]
trail = "#cfe86a"
fade  = 0.055
```

---

## 5. Interaction

- **Left-click** → synthesise `Win+W` to open the Widgets panel. Covering the weather
  therefore costs no functionality.
- **Right-click** → theme menu, families as submenus.
- **Tray icon** → full menu: themes, brightness, saturation, start-with-Windows, quit.

The tray icon is **required, not a preference**: when no audio is playing the overlay does not
exist, so without it there would be no way to quit the application.

---

## 6. Settings

TOML at `%APPDATA%\taskbar-eq\config.toml` — plain text so it can be diffed between the two
machines. Holds: selected theme id, brightness, saturation, reveal/hide timings, RMS threshold,
autostart flag.

"Start with Windows" writes `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` — no
elevation, trivially reversible.

---

## 7. Module boundaries

Six modules. The three holding the interesting logic have **no Windows dependency**, so they
are testable without a taskbar:

| Module | Responsibility | Windows-dependent |
|---|---|---|
| `dsp` | samples → band levels. FFT, log-frequency mapping, attack/decay ballistics | No |
| `themes` | schema, parsing, built-in table, hot-reload merge and precedence | No |
| `render` | (levels, theme) → RGBA buffer. Glow, segments, needles, persistence | No |
| `capture` | WASAPI loopback; follows default-device changes | Yes |
| `placement` | widget rect discovery; visibility rules; DPI | Yes |
| `shell` | tray icon, menus, settings, autostart, hot-reload watcher | Yes |

Audio capture runs on its own thread and hands band levels to the render loop through a
lock-free single-producer/single-consumer buffer — the capture thread must never block on
rendering.

Render target is 60 fps while visible, 0 while hidden.

---

## 8. Testing strategy

- **`dsp`** — unit tests against synthetic signals: silence, full-scale sine at known
  frequencies (energy lands in the expected band), step response (attack/decay timing),
  and the reveal/hide hysteresis state machine driven by a scripted RMS sequence.
- **`themes`** — parse tests: valid file, unknown key (warns, still loads), missing key
  (defaults), malformed TOML (skipped, others unaffected), unknown schema (rejected),
  built-in override by id, zoned and `[dual]` variants.
- **`render`** — golden-image tests: render a fixed level array for each of the 15 colourways
  and compare against committed PNGs. This is what catches an accidental change to one theme
  while editing another.
- **`capture` / `placement`** — thin Win32 edges, verified manually, plus a smoke harness that
  prints the discovered widget rect and current visibility decision once per second.

---

## 9. Non-goals

**Keyboard backlight visualisation.** Investigated and ruled out on this hardware. The
Precision 5490 has a single-zone white backlight — one lamp, no addressable segments, so there
is nothing to make bars from; the best possible result is whole-keyboard brightness pulsing.
Even that is unreachable: the only exposed control is a Dell BIOS attribute via Dell Command
Monitor, which returns `Access denied` unelevated, and is NVRAM-backed — driving it at frame
rate would be slow and would wear the hardware. No Dell SMBIOS WMI interface is present.
(On a Razer/Legion/Alienware machine this would be straightforward via the Chroma SDK.)

**Light mode.** The user is always in dark mode. Not designed or tested for.

**macOS / Linux.** Both would need a different placement model and a different capture path.
If wanted later, each gets its own spec; only `dsp` and the theme table would be shared.

**Installer.** A portable exe is the deliverable. An MSI would need elevation on the work
laptop and buys nothing for a personal tool.

**Spectrogram family.** Mocked up and liked, but not selected. Kept here as the obvious
candidate if a fourth family is ever wanted — it needs a scrolling buffer, which the `Family`
trait must not preclude.

---

## 10. Suggested build order

Not the implementation plan, but the spine it should follow. The ordering is chosen so that
**something real appears on the actual taskbar as early as possible** — the look is judged by
eye, and a mockup in a browser is not the same as 190 × 60 pixels on a live acrylic taskbar.

1. **Skeleton + placement.** DPI awareness, layered topmost window, widget rect discovery,
   draw a flat filled rectangle. Verifies the hardest-to-fake part first: that a window can sit
   convincingly over the widget and track it.
2. **Capture + DSP.** WASAPI loopback → band levels, with the smoke harness printing levels.
3. **One colourway end to end** — VFD Ice on `segmented`. First point at which the thing can be
   looked at and judged.
4. **Reveal/hide state machine**, then the tray icon and quit path.
5. **Remaining `segmented` colourways**, then `scope`, then `vu`.
6. **TOML loading, precedence and hot reload** — built-ins move out to the schema last, once the
   shape of what a colourway needs is settled by three working families rather than guessed at.

Step 6 comes last on purpose. Defining the external schema before the renderers exist would mean
versioning a guess.

## 11. Deliverable

A single portable `taskbar-eq.exe`, copied to both machines. No installer, no admin, no runtime.
Local git repo at `Documents\projects\taskbar-eq`, no remote.
