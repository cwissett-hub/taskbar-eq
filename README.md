# Taskbar EQ

![taskbar-eq](docs/screenshot.png)

A real-time audio visualiser that overlays the Windows 11 Widgets (weather) button while audio
is playing, and hands the weather back when it stops.

Single portable `taskbar-eq.exe` — no installer, no admin, no runtime.

---

## Install

There is no installer. It is one file. But downloading an unsigned exe from the internet does
involve two Windows prompts, so here is the whole path.

### 1. Download

**[taskbar-eq.exe](https://github.com/cwissett-hub/taskbar-eq/releases/latest/download/taskbar-eq.exe)**
— always the latest [release](https://github.com/cwissett-hub/taskbar-eq/releases/latest).

Put it wherever you like — Desktop, Documents, anywhere. It does not need to be in
`Program Files` and does not need admin.

**Verify it if you want to be sure of what you downloaded**: every release's notes carry the
exe's SHA-256. `Get-FileHash taskbar-eq.exe -Algorithm SHA256` (PowerShell) or `sha256sum
taskbar-eq.exe` and compare against the hash in that release's notes.

### 2. Expect Windows to complain, because the exe is not code-signed

This is normal for a hobby binary and not a sign anything is wrong. Two things may happen:

- **Edge/Chrome:** the download may be flagged. Choose *Keep* / *Keep anyway*.
- **On first run:** "Windows protected your PC" (SmartScreen). Click **More info**, then
  **Run anyway**.
- **Windows Defender may name it outright** — `Trojan:Win32/Wacatac.B!ml` is the common one for
  a brand-new unsigned exe. That `!ml` suffix means it is a machine-learning **reputation**
  verdict (new file, no prior install base, unsigned), not a signature match on anything the
  file actually does. The SHA-256 in the release notes is the thing to check if you want
  certainty beyond "trust the source" — a modified file will not match it.

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
  because Win10 has no Widgets button. **This path is untested** — see
  [docs/known-gaps.md](docs/known-gaps.md).
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
| `identify_song` | Name the song currently playing, via Shazam (see Song identification) |

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
| Orbit | The ring scatters outward and is drawn back in |
| Cherry blossom | A storm: lightning strikes the castle and a gust lets the branch go of a great deal at once |
| Brutalist | The monolith: every block slams to full height and figure and ground invert |
| Rave lasers | The rig blacks out, then every beam snaps to full spread at once |
| Kaleidoscope | The mirrors multiply: the fold count doubles and the pattern crowds the strip |
| Fluid | Cavitation — the surface breaks into a patchy froth and the tank runs slack |
| Virtual Self: wings | A lens flare bursts from the wing root — bright core, four streaks and a ring — and fades |
| Virtual Self: ghost voices | A datamosh: horizontal slices shift and colour-invert for a few frames, with a white strobe on the first |

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

### Command line

It runs as a GUI application with no visible window by default, but every flag `parse_args`
understands is quoted here verbatim from `usage()` (`src/main.rs`), so this cannot drift the way
a hand-copied table would:

```
taskbar-eq 0.2.1
Usage: taskbar-eq [FLAG]

  --console    allocate a console window and stay attached to it, for watching the app run
  --diagnose   print the whole "would the overlay draw?" decision chain, then exit
  --levels     capture 8 seconds of real audio and report what the DSP actually produces, then exit
  --stress     hunt the process handle/thread leak by hammering each suspect path, then exit
  --help       print this usage text and exit
  --version    print the version and exit
```

An unrecognised flag refuses to launch and prints this same text to stderr rather than being
silently ignored. Run from an existing terminal and the app inherits it, so `--diagnose` prints
where you ran it; run without one and `--console`/`--help`/`--version`/an unknown flag all
allocate a fresh console so the output is never silently lost. `--diagnose`'s own walkthrough —
what each line of its decision chain means — is in
[docs/status.md](docs/status.md#if-it-does-not-appear).

---

## Song identification

Press the Identify key (bind it under **Songs** in the tray menu, or click **Identify this song
now**) while anything plays - a browser video, a Discord call, a game - and the banner names the
song. It says "listening..." while it records, then "Title - Artist", or "no match" after about
twelve seconds. Every find is appended to `%APPDATA%\taskbar-eq\songs.jsonl`; **Songs -> Song
history...** opens a page listing them with Spotify, Apple Music, YouTube and Shazam links, and the
Songs submenu itself lists the last ten (click one to open it in Spotify).

The fingerprinting is a port of [SongRec](https://github.com/marin-m/SongRec)'s Shazam signature
code, which is why this repository is licensed under the GPL-3 (see `LICENSE`). It uses Shazam's
undocumented endpoint, so it can stop working without notice; when it does, the last raw reply is
in `last_shazam.json` beside the log.

### What leaves your machine

Only when you press the Identify key or menu item. A Shazam audio *fingerprint* (spectral peaks,
not audio) is posted to `amp.shazam.com`, along with a fixed fake location (45N 2E, Europe/Paris)
and an Android user-agent, exactly as the open-source SongRec client does. Nothing is sent at any
other time. Locally, `taskbar-eq.log` records each identified title and `songs.jsonl` keeps every
find with its links; both stay in `%APPDATA%\taskbar-eq`.

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
| `[hotkeys]` | all empty | `play_pause`, `next_track`, `prev_track`, `random_theme`, `random_colourway`, `flourish`, `flourish_toggle`, `identify_song` |
| `autostart` | `false` | **A record, not the truth.** The live state is the registry `Run` value, which is what the menu reads |

The same folder holds `taskbar-eq.log` (truncated per run) and a `themes\` directory for your own
colourway files, which hot-reload on save and can replace a built-in by reusing its `id`.

---

## More

**163 colourways across 25 families**: Segmented VFD, Oscilloscope, VU dials, Vaporwave grid,
Valve row, Fluid, Nixie tubes, Spectrogram, Reel-to-reel, Patchbay, Radar, Pantone, Flame organ,
Dolphin LCD, 3D spectrum, 3D Pipes, Orbit, Cherry blossom, Kaleidoscope, Rave lasers, Brutalist,
Chroma field, Virtual Self: wings, Virtual Self: ghost voices, Virtual Self: orb. The full catalogue,
with a screenshot and one-liner per family, moved out of this README to keep it a reasonable length:

- [docs/themes.md](docs/themes.md) — the full colourway catalogue, all 25 families, and the
  external-theme file format
- [docs/theme-prompt.md](docs/theme-prompt.md) — a self-contained prompt for having a coding
  agent author new colourways
- [docs/status.md](docs/status.md) — the feature-completion table and per-family notes
- [docs/known-gaps.md](docs/known-gaps.md) — what is untested or measured-but-not-fixed
- [docs/lessons.md](docs/lessons.md) — defects that cost real time to find, kept as the useful
  output of finding them
- [CHANGELOG.md](CHANGELOG.md) — release history
- [TODO.md](TODO.md) — open work and judgement calls waiting on a human

---

## Build from source

Requires the [Rust stable MSVC toolchain](https://rustup.rs). No other dependencies.

```
git clone https://github.com/cwissett-hub/taskbar-eq
cd taskbar-eq
cargo build --release        # -> target/release/taskbar-eq.exe
cargo test                   # the default suite - fast, so you actually run it before committing
cargo test --release slow_ -- --ignored   # the slow set (run in release; this is the CI job)
```

The default `cargo test` is kept short enough to run before every commit: every test that takes
more than ~5 s single-threaded in debug is gated behind `#[ignore]` with a `slow_` prefix. That
`slow_` set covers the whole-registry render sweeps AND the per-family real-music / flourish checks
(each drives a long audio fixture). Run it with the `slow_` line above before shipping a rendering or
DSP change; it is the set CI runs on every push. The fast default suite still keeps the cheap
structural guard (`every_colourway_is_visibly_alive_at_two_sizes`) and all the golden tests.

Building yourself also sidesteps the SmartScreen prompt entirely.

**To strip build-machine paths from the exe** (the released binary is built this way — a plain
`cargo build --release` embeds your username and checkout path in panic messages and debug info):

```powershell
$env:RUSTFLAGS = "--remap-path-prefix=$PWD=. --remap-path-prefix=$env:USERPROFILE\.cargo=~cargo"
cargo build --release
```

Run from the repo root, in PowerShell. It rewrites the checkout path to `.` and the cargo registry
path to `~cargo`, both stable substitutions rather than anything machine-specific — `[profile.
release] strip = true` already strips symbols, but that alone does not touch paths baked into the
binary as strings.

Goldens under `tests/golden/` are ASCII luminance maps rather than PNGs, so a rendering change
shows up as a readable diff and needs no image dependency. The catch: a golden regenerated
from a broken renderer locks the bug in, so read any golden before committing it.

`tools/probe/` holds read-only PowerShell scripts for re-measuring taskbar geometry on another
machine — useful if the overlay lands in the wrong place.

---

## Licence

GPL-3.0-or-later — see [LICENSE](LICENSE). Copyleft because the Shazam fingerprinting is a port
of [SongRec](https://github.com/marin-m/SongRec)'s signature-generation code; there is no way to
ship that functionality under a more permissive licence than the code it is derived from.
