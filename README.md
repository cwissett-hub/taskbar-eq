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

### The meter itself

| Action | What it does |
|---|---|
| **Left-click** | Opens the Widgets panel (synthesises `Win+W`). The overlay sits *on top of* the Widgets button, so without this the weather would be unreachable while music plays |
| **Right-click** | Opens the tray menu — the same menu, one implementation with two entry points |

Nothing else is handled: no wheel, no drag, no hover. The overlay takes `WS_EX_NOACTIVATE` so it
never steals focus, but deliberately **not** `WS_EX_TRANSPARENT`, which is what lets it receive those
two clicks at all. Because it is opaque and clickable it is clamped to leave 8 px of clearance, so it
can never end up covering a pinned taskbar button.

### The tray menu

Right-click, left-click or the context-menu key — all three open it.

| Item | What it does |
|---|---|
| **One submenu per family** | Every colourway in that family. The active one is ticked, and so is its family. Families appear in the order the theme registry first mentions them, not alphabetically |
| **Spotify controls** | Bind the three transport keys, and choose how they are sent. The parent label tells you the state: `Spotify controls`, `…: not set up` when no key is bound, or `…: not working` when a bound key failed to register |
| **Random** | `Any theme now`, `Another colourway of this theme now`, and the two keys for those |
| **Flourishes** | `Flourish now`, `Turn flourishes off`/`on`, and the two keys |
| **Open config file…** | Saves first so the file exists, then opens it — falling back to Notepad, because `.toml` often has no association |
| **Start with Windows** | Ticked state is read from the registry (`HKCU\…\Run`, value `TaskbarEQ`), not from the config, so it tells the truth even if something else changed it |
| **Exit** | The only quit path, by design: when nothing is playing the overlay does not exist, so the tray icon is all that is left to click |

Every key label shows the *live* registration result, not what the config says — so you see
`Ctrl+Period`, or `not set`, or `Ctrl+Period  (in use elsewhere)`.

### Keys

Seven bindable actions. **All of them ship unbound**, and that is a deliberate choice rather than an
omission: `RegisterHotKey` is first-come and exclusive, and this app can start at logon, so a default
binding would quietly seize a chord machine-wide for every other program on the machine.

| Action | What it does |
|---|---|
| `play_pause`, `next_track`, `prev_track` | Spotify transport |
| `random_theme` | Any colourway in any family |
| `random_colourway` | Another colourway of the family already showing |
| `flourish` | Fire the current family's flourish now |
| `flourish_toggle` | Flourishes on/off, persisted |

**Binding one:** tray menu → the submenu → click the `…key:` line. A small **Set key** dialog opens,
echoes the modifiers as you hold them, and commits on the first non-modifier key. `Esc` cancels,
`Backspace` or `Delete` unbinds, and it times out after 30 seconds so the app can never be left with
its keys released. It releases every one of its own hotkeys while open — otherwise the chords most
worth rebinding would fire instead of being captured — and it keeps pumping messages, so the meter
carries on running behind it.

It **refuses** a chord and tells you why rather than silently taking it: modifiers alone, a duplicate
of another action, or a bare printable key (which would eat that key everywhere). Bare `F1`–`F24` and
the four media keys are allowed. It also warns without blocking about `Ctrl+Alt+…` colliding with
AltGr, about seizing a media key from every other player, and about `F12` being the debugger key.

In the config file the notation is `Ctrl+Alt+Shift+Win+Key`, in that order, case- and
order-insensitive on the way in. Key names are layout-independent (`Comma`, `Semicolon`,
`LeftBracket`, `MediaPlayPause`, `Numpad4`, `F9`) but the *menu* asks your keyboard layout how to
draw them, so a stored `RightBracket` shows as `]` on a UK layout. `MOD_NOREPEAT` is always set, so
holding Next Track cannot skip a whole playlist.

### Spotify transport, and why it is Spotify-only

Two backends, switchable in the menu:

| Backend | How | Trade-off |
|---|---|---|
| **Spotify session** (default) | WinRT `Windows.Media.Control`, addressed by Spotify's app id | 2–10 ms, works minimised and unfocused, and goes to Spotify even when Chrome also has media keys. Success is **observed**, not assumed: it watches for the playback status to flip or the title to change, for 750 ms |
| **Media keys** | `SendInput` of the real `VK_MEDIA_*` keys | ~76 ms, and whoever owns the key wins — commonly Chrome |

The track name behind the banner comes from the same session, and the expensive part of reading it is
gated. `TryGetMediaPropertiesAsync` marshals the whole properties record — including the thumbnail
reference, which for Spotify is album art — and it was being called every 400 ms for as long as anything
played. It now runs when the session says its properties changed, with a 2 s safety net in case that
notification never arrives: worst case the banner is 2 s late and the call rate still falls by four fifths.

A failed command is **never retried**, in either backend. A double-skip is worse than a missed press.
The backends never silently fall back to each other either, because a control that sometimes goes to
the wrong application is harder to live with than one that reliably does nothing.

### The track banner

When the track changes, its `Title - Artist` slides over whatever the family is drawing: 140 ms rise,
2.2 s hold, 520 ms fall. A name too long to fit marquees, but only during the hold, and eases with a
pause at each end. It takes the theme's own lit colour, so it inherits rainbow and ink colourways
rather than sitting outside them.

The meter behind it dims to 66% **toward the panel colour** and never by alpha — alpha is what the
Windows weather widget shows through, so dimming that way would make the forecast appear inside the
meter. Turn the whole thing off with `show_track_name = false`.

### Flourishes

Once in a while — about every 30 seconds on real music — the display does something that has nothing
to do with the audio, and everything to do with what it is pretending to be. **Thirteen of the fourteen
families** have one, and each is that instrument's characteristic fault or ritual:

| Family | Flourish |
|---|---|
| Segmented VFD | A self-test: every segment lights, then drains |
| VU dials | The needles slam to the end stop and the OVER lamps light |
| Spectrogram | A broadband tear, written into the history so it scrolls away as data |
| Valve row | Gas ionisation — a cold blue haze, the wrong colour for the display on purpose |
| Nixie tubes | Every cathode fires at once, as a badly-driven tube's do |
| Oscilloscope | Loss of trigger lock: the sweep free-runs and the phosphor smears every phase |
| Reel-to-reel | Wow and flutter — 1.1 Hz and 8.5 Hz speed error, reaching the tape slack |
| Pantone | A printing plate slips out of register |
| Patchbay | The panel re-patches itself, every cable to the other jack of its pair, and back |
| Vaporwave grid | A lightning storm — five staggered strikes, each flashing the sky behind it |
| Radar | Barrage jamming — the receiver saturates, so returns appear at every range at once |
| Chroma field | An ink plate starves, and the dry patch travels across the stripes |
| Flame organ | A flashback: every burner guts to its pilot, then an ignition front relights them |
| Dolphin LCD | The dolphin leaps clear of the display and lands, throwing a splash along the waterline |
| 3D spectrum | The whole stack surges one depth step forward and settles back |
| 3D Pipes | Every run is abandoned at once and fresh pipes start, the way the screensaver resets |
| Fluid | Cavitation — the surface breaks into a patchy froth and the tank runs slack |

**Every family has one.** The fluid tank was the last and the hardest, because it has the tightest
invariants here: the liquid must stay inside the tank, the drawn surface must follow the simulated field,
and each colourway's own damping must show through. Three models were tried that INJECTED a disturbance
into the wave field and none can work — the wave equation propagates whatever is injected and interference
then raises crests elsewhere, taking a family that clips on 0.00% of column-frames to 3.8%. Cavitation is a
*loss* of coupling, so it is now heavier damping (the tank runs down, which removes energy and therefore
cannot clip) plus a froth on the *drawn* surface line (which cannot propagate at all). Both halves are
asserted separately, and deleting either fails a different test.

Two colourways — **Vaporwave Toxic** and **Vaporwave Noir** — set `bolt_bright = 0`, which turns their
lightning off deliberately for anyone who finds the strikes distracting. The storm respects that: a
colourway that switches lightning off is not handed a lightning storm.

**When one fires is judged by rarity, never by a threshold.** Each hit is compared against the median
of recent hits, so it fires on a moment that is exceptional *for this track* — which is what makes the
same setting work on sparse acoustic music and on a wall of compressed loudness. The default was
measured against a 119-second capture of nine varied tracks.

Each colourway sets its own rate, so a restrained one can be rarer than a loud one. `flourishes =
false` (or the menu, or the key) turns the lot off without discarding that per-colourway tuning.

There is a picture of all nine, with what to look at, in [docs/review/index.html](docs/review/index.html).

### It gets out of the way

The tool **suspends itself** whenever the **display is off**, or something is covering the taskbar: a
fullscreen app, presentation mode, or a hidden taskbar.

**The display-off case matters most on a laptop, and it is measured rather than assumed.**
`powercfg /srumutil` over eight days put this app *second* on the machine for energy — above VS Code and
5.3× Chrome across its thirty processes — alongside a battery draining in about 45 minutes with the lid
**closed**. With the lid shut nothing can be seen, so every frame composited was waste. The cost is also
worse than the CPU share suggests: a process waking every 16 ms keeps the machine out of the deep idle
states that make a closed lid cheap at all. A **dimmed** display is deliberately *not* treated as off —
Windows dims before it sleeps, and blanking the meter early on every idle timeout would be a visible
regression for nothing.

"A fullscreen app" is checked two ways, and the second matters more than it looks.
`SHQueryUserNotificationState` reports fullscreen only for **exclusive** Direct3D - so a game in
*borderless windowed* fullscreen, which is the default for most modern titles, left every signal saying
"nothing is covering you". The overlay carried on drawing a topmost layered window over the game and
carried on making UI Automation calls into the shell. Both are expensive in the way that gets reported as
stuttering: a topmost layered window denies a fullscreen app independent flip, so it composites through
the desktop compositor instead of presenting directly. So there is now a geometric check too - if the
foreground window covers its monitor's full bounds and is not one of the shell's own windows, the overlay
sleeps. Sized to the *work area* instead, that check would call every maximised window fullscreen;
`--diagnose` prints what it sees. Suspended, the overlay window is genuinely hidden rather than merely left
undrawn — a topmost layered window can keep a game out of exclusive fullscreen just by existing — and
the tick drops from 16 ms to 250 ms, roughly fifteen times fewer wakeups. No drawing, and no UI
Automation calls, which are the expensive part: each one blocks inside `explorer.exe` for about 52 ms.

Silence deliberately does **not** suspend it, or the first beat after a quiet passage would take a
quarter-second to appear.

A watchdog checks the process handle count every 30 seconds, warns in the log at 3,000, and exits at
30,000. That is not theoretical: an instance was measured at 18,962 threads and 131,454 handles, and took
a fullscreen game from 160 fps to 30 with dropped input.

That was originally thought to need days of uptime. It does not — it was later reported after **30 to 45
minutes**, on a machine that plays games in borderless fullscreen, and never on one that does no
fullscreen rendering at all.

`--stress` now measures each suspect path directly, and found two real per-call leaks: the UI Automation
tree walk (**+107 handles per 1,000 calls**) and the media session poll (**+22**). Creating the UIA client
on its own leaks nothing, so it is walking the tree that does. Both are now backed off while nothing is
moving — 4.4x less, which moves the watchdog's fatal threshold from 3.2 days of uptime to 13.9.

**Neither of them is big enough to be the reported fault**, and the arithmetic is what says so: at those
rates, 45 minutes predicts 293 handles, against the 131,454 measured. The borderless-fullscreen gap above
remains the leading explanation, because a UIA call into an `explorer.exe` that a game is monopolising
blocks for far longer than one into an idle shell. **[TODO.md](TODO.md) records what is measured and what
is still inference.** The watchdog stays either way.

---

## Configuration

`%APPDATA%\taskbar-eq\config.toml`, written whenever a setting changes and openable from the tray
menu. Every field is optional and a missing **or corrupt** file falls back to defaults rather than
refusing to start, so a partial file keeps working and a bad one cannot lock you out.

| Field | Default | What it does |
|---|---|---|
| `theme` | `"vfd-ice"` | The colourway id. An unknown id falls back to VFD Ice rather than failing |
| `width` | `380` | Requested width in **physical** pixels, measured leftward from the Widgets button. A request only — it is clamped to the real clearance, with 12 px of hysteresis so unrelated taskbar churn does not wipe the phosphor trails |
| `threshold_dbfs` | `-55.0` | The reveal gate, in dBFS of capture RMS |
| `reveal_ms` | `400` | How long audio must stay above the threshold before the meter appears |
| `hide_ms` | `4500` | How long silence must last before it goes away. 2000 was tried and album track gaps popped it out |
| `fade_ms` | `450` | Reveal/hide crossfade |
| `media_backend` | `"session"` | `"session"` or `"media-keys"` — see Spotify transport above |
| `show_track_name` | `true` | The track-change banner |
| `flourishes` | `true` | Global on/off for flourishes, separate from each colourway's own rate |
| `[hotkeys]` | all empty | `play_pause`, `next_track`, `prev_track`, `random_theme`, `random_colourway`, `flourish`, `flourish_toggle` |
| `autostart` | `false` | **A record, not the truth.** The live state is the registry `Run` value, which is what the menu reads |

The same folder holds `taskbar-eq.log` (truncated per run) and a `themes\` directory for your own
colourway files, which hot-reload on save and can replace a built-in by reusing its `id`.

---

## Status

**Last updated: 2026-08-10.** Full test suite green (520 at the time of writing), release build
warning-free. The colourway and family counts below are asserted by a test; the test count itself is
a snapshot and can drift.
**118 colourways across 17 families.**

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
| ✅ | **Fluid — 6 colourways**, two submerged subwoofers driving a 1-D wave simulation | working |
| ✅ | Theme menu: per-family submenus, follows the Windows light/dark setting | **unseen** |
| ✅ | Tray icon, start-with-Windows, clean quit | working |
| ✅ | **Spotify transport** — play/pause, next, previous, via the media session or real media keys | working |
| ✅ | **Seven bindable hotkeys**, all unbound by default, with a capture dialog that refuses bad chords and says why | working |
| ✅ | **Track-name banner** on every change, marqueeing when too long | working |
| ✅ | **A flourish per family** — nine of them, fired by rarity rather than by a threshold | **needs your eyes** |
| ✅ | **Random colourway / random theme**, from the menu or a key | working |
| ✅ | **Suspends under a fullscreen app**, hiding the window and dropping to a 250 ms tick | working |
| ✅ | **Handle watchdog** — warns at 3,000, exits at 30,000 | working |
| ✅ | Right-click equaliser → theme menu; left-click → `Win+W` | working |
| ✅ | External TOML colourways, versioned schema, override-by-id, `[vaporwave]` + `[tube]` + `[fluid]` tables | working |
| ✅ | **Hot reload** — save a theme file and the taskbar updates, no restart | working |
| ✅ | Frame-rate-independent animation (`dt_ms`), so scroll and the gate's timings do not drift with load | working |
| ✅ | **Wide display** — claims the dead taskbar left of the widget, clamped to real clearance | **unseen** |
| ✅ | Layouts scale with width: 4 VU dials and 20 valves at 380 px, 2 and 10 at 190 px | **unseen** |

**Nixie tubes** — a rank of nixies, each with ten stacked cathode digits. The *struck digit climbs*
with the band, and the other nine stay visible as dim unlit wire, which is both what makes it
recognisable and the scale the eye reads the lit digit's height against. Brightness carries no
information at all. 7 tubes at 190 px, 14 at 380 px — a legible digit needs more glass than a valve
does.

| Colourway | Character |
|---|---|
| Nixie orange · Nixie ice · Nixie green · Nixie magenta · Nixie aged | IN-12 neon, argon blue-white, matched to Matrix Green, violet, and one with sputtered cloudy glass |

**Spectrogram** — the only family that shows *history*: frequency up the vertical axis, time scrolling
sideways, intensity as colour. A ring buffer of 512 past spectra holds raw levels rather than colours,
so changing theme or sensitivity recolours the whole visible history instead of leaving a seam.

| Colourway | Character |
|---|---|
| Heat · Ice · Viridis · Monochrome · Inferno | Classic sonagraph ramps. Viridis' and inferno's true dark ends measure 1.4:1 and 2.5:1 against the panel, below the 3:1 rule, so the transparent floor carries the darkest end instead |

**Reel-to-reel** — two spinning reels over a VU strip. The reels *rotate*, with spokes so the rotation
is actually visible, and the tape sag responds to level. Motion is the cue.

| Colourway | Character |
|---|---|
| Studio grey · Warm wood · Black and chrome · Olive military · Cream domestic | |

**Patchbay** — a modular synth panel: jack sockets top and bottom, curved patch cables whose *sag* and
brightness track a band group, and LEDs that blink on bass transients.

| Colourway | Character |
|---|---|
| Classic · Cream · All-black · Rainbow cables · Neon UV | |

| Colourway | Character |
|---|---|
| Classic green-amber-red · Vintage red · Modern blue-white · Amber · Plasma orange | |

**Radar** — a PPI sweep. The sweep line rotates leaving a decaying phosphor wake, painting blips where
energy is as it passes each bearing, so the display builds a whole picture over one revolution. A
180° fan from the bottom centre, because a circle does not fit a 190×60 panel.

| Colourway | Character |
|---|---|
| P1 green · Amber · Ice blue · Red alert · Monochrome | |

**Pantone** — Felipe Pantone's surface language on a bar meter: a full-spectrum chromatic gradient,
RGB channel misregistration, halftone screens and barcode bands. Its answer to the contrast problem
is the interesting part — see RGB wave below.

| Colourway | Character |
|---|---|
| Spectrum · Process (CMYK) · Barcode · Misregister · Halftone | Each leans on a different element of his vocabulary |

**Flame organ** — a **Rubens' tube**: a perforated gas pipe driven by sound, where the flame height traces
the wave inside. A manifold of nozzles along the bottom, each burning to a height set by its band, and the
only family whose reading is carried by something that looks alive. Not a fluid simulation — a
bottom-seeded heat-diffusion field, one pass over ~10,000 cells, the same cost class as the spectrogram's
history buffer.

The cooling **subtracts** rather than multiplies, and that is what makes it legible: a multiplicative
decay would make plume height logarithmic in the band, so a loud burner would stand only slightly taller
than a quiet one and the display would read as brightness. Subtracting a constant per row makes height
linear, so the plumes are a profile you can compare across — the same position-over-intensity rule the
nixie and valve families are built on. Faintness comes from **alpha**, not from a dark colour: the body is
translucent throughout, which is what makes the flames read as ghostly rather than as solid shapes.

**Dolphin LCD** — the 1990s aftermarket car head unit: Sony Xplod, Pioneer, JVC. The whole panel is one
coarse dot-matrix display, 3px dots on a 4px pitch, and the **unlit dots are drawn too** — that faint
lattice of dark wells is what makes it read as a display rather than as floating squares. A spectrum runs
along the bottom with peak-hold caps that fall, a dotted waterline sits above it, and a dolphin arcs
across the display and dips back through the line, its speed tracking loudness. One backlight hue per
colourway at three levels: lit dot, peak cap, unlit well.

The sprite carries a **hard dark keyline**, because without one it is lit dots on a lit lattice and reads
as an amorphous cluster — which is how the first render came out.

**3D spectrum** — the Winamp / Windows Media Player *bars in depth*. Five rows of extruded bars
staggered up and to the right, the nearest bright and crisp, each row behind it dimmer and drawn first
so the near bars occlude it. **Depth is time**: every row is a spectrum snapshot from further back, so a
transient visibly walks backwards into the display.

The geometry is deliberately **oblique, not perspective** — a constant integer offset per row, no
divide and no vanishing point. Perspective was refused on measurement: depth steps and amplitude compete
for the same ~48 usable rows, and at the vaporwave family's tuned `persp` seven of sixteen depth lines
collapsed onto two pixel rows, which also silently disabled its occlusion. An earlier oblique design was
refused too, for a subtler reason — extruding a *curve* by a constant offset gives a visible depth
face of `dy` minus the curve's own rise over the run, which collapses to zero wherever the slope matches
the offset. Discrete boxes have no such term.

**3D Pipes** — the Windows screensaver, driven. Pipes grow segment by segment through a lattice,
turning at right angles on the beat, in a **real perspective projection**: a camera, a divide and a near
plane, not an oblique fake.

That choice was made twice. An isometric version was built first and rejected — *"if we cant do true
3d then I think it's not really worth it"* — and it deserved to be: in isometric the x axis feeds both
width and height, so a 56-row panel runs out of height after about 64px of width, and one lattice
occupied 48px of a 380px panel. The workaround, several small lattices side by side, is fakery.

Perspective brings two hazards this project has already measured, and both are handled explicitly rather
than hoped about. Depth planes must land on **distinct integer pixel rows** — at the vaporwave grid's
tuned perspective, seven of sixteen collapsed onto two rows, which also silently disabled its occlusion,
because lines sharing a row cannot occlude each other. And a vertex near the eye projects to infinity,
saturates `as i32` to 2147483647, and sends one Bresenham edge on ~2.1 billion iterations — measured
at 294.6ms, eighteen dropped frames. A near-plane clip in front of the divide is what makes that
impossible.

| Colourway | Character |
|---|---|
| Sodium · Propane · Copper · Strontium · Potassium · Plasma · Rainbow flame | Flame tests, mostly: the ordinary warm gas flame, then what one looks like burning *correctly* (blue with a white core), copper-salt green, flare crimson, pale lilac, and a violet-to-cyan plasma. Rainbow sweeps hue across the manifold, which suits a rank of physically separate burners better than it suits a continuous display |

**Chroma field** — Pantone's *geometry*, not just his surface. Hard-edged vertical stripes filling the
whole panel in spectrum order, where each stripe's **width** is its band and the widths are
**zero-sum**: they always add up to exactly the panel interior, so a swelling stripe necessarily
pinches its neighbours. That constraint is the design — something is always moving, because the widths
must always sum. Integerised by largest-remainder (Hare quota), so exactness is by construction rather
than by rounding luck. 10 stripes at 190 px, 20 at 380 px. Black keylines, misregistration, a halftone
screen, and a glitch slice on bass transients.

| Colourway | Character |
|---|---|
| Spectrum · CMYK · Barcode · Misregistration · Halftone | Barcode withholds chroma almost entirely, as his stripe works do |

### RGB wave

Three colourways — one each on the **Segmented VFD**, **Oscilloscope** and **VU dials** — are the
gaming-keyboard rainbow: hue sweeps across the display and drifts over time. On a spectrum display
that spatial sweep doubles as a frequency legend, which is why the hue varies by *position* and not
just with the clock.

It is the one visual property that cannot be a hex string, since it changes every frame, so it is
two numbers in `[look]` instead:

| key | meaning |
|---|---|
| `rainbow` | hue cycles per second. `0` disables it and the fixed `lit`/`hot` are used |
| `rainbow_spread` | hue turns spanned across the width. `0` = whole display shifts together ("spectrum cycle"); `~0.85` = a wave |

**Full chroma cannot clear 3:1 at every hue against any flat panel** — that is arithmetic, not
tuning. A dark panel fails on blue (2.32:1); a light panel fails on yellow (1.00:1); mid grey fails
both ways. Two honest resolutions ship: the **Chroma field** family delineates every stripe with a
black keyline, so legibility comes from the keyline rather than hue-vs-panel contrast, and declares a
measured `contrast_floor` of 2.30 which a test requires to be *tight*; the **Pantone** family instead
quantises the palette to a few `inks`, because the ceiling only binds on a continuous wheel — with
four process inks the dark hues that force it are simply not in the palette.

**The rainbow cannot be fully saturated.** Every lit colour here must clear 3:1 contrast against its
own panel, and swept across all 360 hues against a near-black panel, fully saturated blue reaches
only **2.31:1** — it fails. 0.9 gives 2.48 and 0.8 gives 2.88, still failing; **0.70 is the first
value that passes**, at 3.59:1. Blue is simply too dark against black at any brightness, and only
pulling it toward white fixes it. A test walks all 360 hues of every rainbow colourway so nobody
raises the saturation back up.

Two things deliberately keep their own colour under a rainbow: the VU's **overload arc and needle**
stay red, because that colour means something; and the P7 phosphor's **slow trail** stays
yellow-green, because being a different colour from the trace is the entire point of it.

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
- **Fluid** — the height field is one float per pixel column, so a wider panel is a wider tank
  with the same wave speed in px/s: the two cones stay 44% of the width apart and the
  interference pattern in the middle simply gets more room. Nothing is stretched.
- **Segmented, oscilloscope and vaporwave** scale directly and gain from the room.

### No console window

It runs as a GUI application, so nothing appears on screen but the meter itself. Earlier builds were
console-subsystem binaries and popped a black terminal that then sat there for the life of the
process.

The subsystem is fixed at link time, so there is no runtime switch — but you can ask for output:

| | |
|---|---|
| `taskbar-eq.exe` | silent, no window |
| `taskbar-eq.exe --console` | allocates a console so you can watch it run |
| `taskbar-eq.exe --diagnose` | prints the whole "would the overlay draw?" decision chain and exits. See below |
| `taskbar-eq.exe --levels` | captures 8 seconds of real audio and reports what the DSP actually produces - per-band percentiles, frame peaks, onset rates - then exits. It also writes the band frames to `tests/fixtures/`, so it is a source-checkout tool: it is how the tuning constants were calibrated against real music rather than against assumptions |
| run from an existing terminal | inherits that terminal, so `--diagnose` prints where you ran it |

Diagnostics never depend on a console either way: the log at
`%APPDATA%\taskbar-eq\taskbar-eq.log` is written and flushed per line regardless.

### If it does not appear

Run it once with `--diagnose`:

```
taskbar-eq.exe --diagnose
```

It checks every gate in the same order the render loop does, prints the result, and writes the same
report to `%APPDATA%\taskbar-eq\taskbar-eq.log`. **The first `NO` in the output is the reason.** It
covers the Windows build, which DPI awareness actually took effect, the taskbar rect, how many UI
Automation elements were found, whether the Widgets button or the overflow chevron was located, the
rect it chose, whether that rect passes the plausibility check, fullscreen/presentation state, and
whether any audio is arriving at all.

That last one matters most: if no audio frames arrive, the reveal gate never opens, and that is
indistinguishable from "nothing renders".

The app also writes that log on every normal launch, so a failure can be reported after the fact.

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
- **Six families are new and only lightly reviewed.** An adversarial pass on each found real defects
  and **both are now fixed**, though neither turned out to be what the note above claimed. The
  **Patchbay** was said to flatten on real music; measured over three real-music captures the spread
  across cables is healthy (0.70–0.80 of the range, and 0% of frames flat) — the real fault was that
  its response window was placed on *band* levels while the family feeds it a *peak-biased group*
  level, so the bass cable sat pinned at full deflection on 64%, 96% and 100% of frames. And the
  **Spectrogram**'s vacuous fold test was not a ramp problem: the pitch-track marker lands on the same
  row a lone loud band folds into, so the test was measuring the marker. Fixed already: the Reel's peak lamp needed an RMS of 0.508 to light, against a real
  ceiling of about 0.12, so it was dead code that could never fire on music.
- **Every family leaks a few bezel pixels outside the rounded panel.** The bezel is drawn after
  `clip_to_rounded_rect`, and it is square while the panel's corners are not, so ~4 pixels per corner
  land on the bare taskbar. Measured and pre-existing, and invisible in practice against a dark
  taskbar - but it is a real leak, and it is the panel's own rounded corners that it escapes through.
- The width is **clamped by what UI Automation reports**, so an element it cannot see is an
  element the overlay may cover. Every named taskbar element on the test machine was accounted
  for, but this has not been tried on a taskbar with third-party shell extensions.
- Theme *aesthetics* at 190×60 and 380×60 are not verified by anything automated. Every family has an
  `#[ignore]`d dump harness (`cargo test --release dump_ -- --ignored`) that writes raw RGBA
  for eyeballing, because "does this look like a smear" is not a question a golden can answer.
  See [HANDOVER.md](HANDOVER.md) for the full measured-vs-assumed split.

---

## Themes

**118 colourways across 17 families.** A *family* is a renderer with fixed geometry — code. A
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
grid, the terrain displaced by the spectrum, lightning fired by bass transients.

The grid **recedes** by default, which is a deliberate reversal of the classic flying-forward look.
Displacement scales with depth, so the nearest lines move most — and a line only ever shows the
spectrum from when it was born. Flowing toward the viewer, new audio is born at the horizon where it
is drawn at the *smallest* displacement, then needs a full scroll cycle (~1.3 s) to reach the front
where it would be biggest: both penalties at once, which reads as a calm grid lagging the music. Set
`recede = false` in `[vaporwave]` for the classic direction, accepting that the front of the grid
shows what the music did a second ago.

Each line peak-holds while it is the newest, because the terrain only samples the spectrum at about
9 Hz (one line born every ~112 ms) — instantaneous sampling at that rate lets a 50 ms kick fall
entirely between births and never be recorded. The terrain also auto-ranges against the frame's
loudest band so the hills show the *shape* of the spectrum at any volume, but with a deliberately
slow attack: a fast follower dropped the gain from 5.75 to 2.76 on the very frame a kick landed,
cancelling the hit it existed to reveal. The lightning reads the raw signal for the same reason.

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

**Fluid** — a shallow tank of liquid seen side-on, with two subwoofers submerged in it, one
toward each end. They pump vertically with `rms_l` and `rms_r` and displace the liquid directly
above them; the waves travel outward along the surface, reflect off the tank walls and
**interfere** in the middle. That interference is the family's signature — it is the one thing here
that cannot be produced by a per-column response curve, because a column's height depends on what
its neighbours did several frames earlier.

The surface is a 1-D height field, one float per pixel column, integrated with the discrete wave
equation. It runs at a fixed Courant number of 0.5, and the measured frame interval decides only
*how many* fixed sub-steps to take — never how big they are — which is what makes a slow frame run
the water in slow motion instead of blowing the simulation up.

The five colourways differ by **physics**, not hue: `damping` decides whether a wave survives the
trip to the far wall at all, `wave_speed` how coarse the pattern is, and each one adds or removes
whole elements (caustics, droplets, the specular horizon, emission, thin-film colour).

| Colourway | Character |
|---|---|
| Deep water | Deep tank, cyan meniscus, caustics under the crests — the reference |
| Mercury | Heavy, almost lossless: rings into a standing lattice, hard specular horizon, no caustics |
| Oil slick | A shallow film — fast-travelling swells, heavy spray, and a meniscus whose colour shifts with the surface slope |
| Glowing coolant | The liquid itself emits, so the body is bloomed rather than merely bright |
| Dark ink | So viscous the waves die at the cone; the two drivers carry the whole reading |
| Pantone | The liquid itself becomes a duotone of process inks, cycling slowly, plates misregistered |

### Adding your own

Drop a `.toml` file (any filename — the `id` inside is what matters) into
`%APPDATA%\taskbar-eq\themes\` and it appears in the menu **immediately**. The directory is
watched, so saving the file updates the live overlay without a restart — edit a colour, hit
save, and watch the taskbar change.

A file whose `id` matches a built-in **replaces** it; any other `id` is added alongside the 93
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
whose `id` matches one of the 82 built-ins REPLACES it, any other `id` is added):

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
