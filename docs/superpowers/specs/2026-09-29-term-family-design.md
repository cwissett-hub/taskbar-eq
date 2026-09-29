# `term` family — design

A render family that looks like the user's own terminal: VSCode with the **"2077" theme**
(endormi.2077-theme), as a live shell session whose output lines are the meter. Requested 29 Sep
2026 ("a theme based on my current VSCode theme, I want it to look like a terminal"). Five
colourways. Ships as **v0.3.1** with the `sesh` fixes already on `main`.

This spec is the authority; the implementation plan argues from it.

## Goals

- Reads as a terminal at a glance — prompt, monospace output, block cursor, status bar — AND as a
  meter: each output line's length is a band level.
- Uses the theme's real colours (below), so it sits on the user's screen like a second VSCode pane.
- Same envelope as the recent families: opaque panel, `Trigger` + `Envelope` flourish, allocation-free
  `draw`, < 1 ms/frame steady, 3:1 contrast, liveness at 128x44, `font3x5` for all text, no
  trademark marks (no VSCode logo; "2077" appears only as the colourway name in menus).

## Non-goals

No new theme schema; no new dependency; no light-taskbar logic; no Japanese glyphs; no shell
execution of any kind (the commands are decorative strings).

## The palette (from the installed theme JSON, `theme/2077 theme-color-theme.json`)

| role | hex |
|---|---|
| terminal background | `#0d0936` |
| editor background (status bar) | `#030d22` |
| foreground | `#e4eeff` |
| cursor / line numbers (hot pink) | `#ee0077` |
| ANSI cyan / bright cyan | `#0ab2fa` / `#4bc5fa` |
| string cyan | `#0ef3ff` |
| magenta | `#EA00D9` |
| red / bright red | `#ee1682` / `#ff2e97` |
| yellow | `#ffd400` |
| green / bright green | `#06ad00` / `#3dd69c` |
| blue / status text | `#3787d6` / `#4d8bee` |
| comment blue | `#0098df` |
| selection | `#310072` |
| line highlight | `#1c1347` |
| panel border | `#181657` |

## What the meter is

**Panel.** Opaque `t.panel` (terminal background), a 1 px `t.edge` border (`#181657`), and a
**status bar** along the bottom, `7 px` tall, in `#030d22` with `font3x5` text in `#4d8bee`:
left `~/music`, centre a `♪`-less `NOW` + the rms as a two-digit percent (`▮ 63%`), right `UTF-8  LF`.
When `h < 48` the status bar is dropped.

**Output lines — the meter.** The interior above the status bar is divided into **L rows** of 4 px
pitch (3 px glyph + 1 px gap), top-aligned. `L = min(12, (interior_h - 5) / 4)`. Each row `r`
shows: a 5 px **line number** column in `#ee0077` (`font3x5` digits, right-aligned, counting up
from a scrolling base), a gap, then a **bar** of `▮` block glyphs (3x5 solid) whose count is the
row's band level times the available cells, in the row's colour, capped by a `▯` outline glyph as
the peak-hold marker in `t.hot`. Row `r` folds bands `r*64/L .. (r+1)*64/L`; row 0 (top) is the
highest band so the bass sits at the bottom next to the prompt, like a log's newest line. Row
colours cycle bass→treble through magenta `#EA00D9`, red `#ff2e97`, yellow `#ffd400`, green
`#3dd69c`, cyan `#0ab2fa`, string-cyan `#0ef3ff`, blue `#3787d6` (colourway tables override).
The loudest row gets a **line highlight** (`#1c1347` full-width behind it); a row above 0.9 gets
**selection** (`#310072`) instead.

**Prompt line.** The bottom-most row is the prompt: `❯` (drawn as `>` in `#ee0077`) then a
**typed command** in `t.lit` that types out at 1 char per onset from a fixed list
`CMDS = ["cargo run --release", "git push", "ls -la", "npm run dev", "python analyse.py", "ssh fab-01", "cat /dev/audio", "vim main.rs", "clear", "htop"]`;
on a strong onset (mean bass > 0.55) with the command complete, the line "executes": the
output rows shift up one (a scroll: line-number base += 1) and the next command begins. A **block
cursor** (3x5 solid in `#ee0077`) follows the typed text and blinks at 1 Hz (visible 60 %).
Everything is `font3x5`, so `CMDS` chars must all have glyphs (a test asserts it).

**Comment line.** When `L >= 8`, the row above the prompt shows a dim comment in `#0098df`:
`# bpm ~ NNN` where NNN is derived from the onset interval (median of the last 8 onset gaps,
clamped 60..200), updated once a second. Purely decorative, no allocation (fixed `[u8; 3]`).

**Scanlines.** Every 3rd interior row at `t.ghost` alpha in `t.lit` — subtle CRT.

## Colourways

No `zones` are used. Row colours are per-style constants inside the family, matched on `t.id`
(the `sesh` `Style` pattern; unknown ids fall back to `term-2077`'s style). Theme fields used:
`panel`, `lit` (typed text / fallback bar colour), `hot` (cursor, peak marker, line numbers),
`edge` (border), `ghost` (scanline alpha).

Glyphs: `font3x5` gains `▮` (solid 3x5 block), `▯` (3x5 outline) and keeps `>` as the prompt
chevron; the status bar uses only characters the font has (no `♪`).

| id | panel | lit | hot | edge | ghost | row colours | note |
|---|---|---|---|---|---|---|---|
| `term-2077` | `#0d0936` | `#e4eeff` | `#ee0077` | `#181657` | 0.10 | the seven-colour cycle above | the theme verbatim |
| `term-2077-cyan` | `#0d0936` | `#e4eeff` | `#4bc5fa` | `#181657` | 0.10 | `#0ab2fa`, `#0ef3ff`, `#4bc5fa` cycling | monochrome cyan, pink only on the prompt `>` |
| `term-2077-hot` | `#0d0936` | `#ffd6ec` | `#ee0077` | `#181657` | 0.10 | `#EA00D9`, `#ff2e97`, `#ee1682` cycling | pink/magenta lead |
| `term-2077-matrix` | `#030d22` | `#3dd69c` | `#06ad00` | `#0e0952` | 0.14 | `#3dd69c`, `#06ad00` alternating | green on the editor navy |
| `term-2077-editor` | `#030d22` | `#fdfeff` | `#47a1fa` | `#181657` | 0.08 | `#ff2cf1`, `#0ef3ff`, `#ffd400`, `#39c0ff`, `#c832ff` (token colours: keyword, string, number, function, constant) | the code-editor palette instead of the ANSI one |

All `lit`/`panel` pairs clear 3:1 comfortably (navy panels, light or saturated foregrounds).

## Flourish — `panic!`

`Envelope` 700 ms. Fires only on a strong bass onset (trigger AND mean bass ≥ 0.6, the `sesh`
gate). Frame 1: every output row is replaced by a red (`#ff2e97`) stack trace — fixed lines
`thread 'main' panicked at src/dsp/bands.rs:64:9`, `index out of bounds: the len is 64`,
`note: run with RUST_BACKTRACE=1`, then `   0: taskbar_eq::render::term::draw`, `   1: ...`,
truncated to whole glyphs at the interior width — the cursor freezes solid; the status bar turns
`#ee1682` with `EXIT 101`. While the envelope is above 0.25 the trace **scrolls up** one row per
~60 ms (the base shifts; rows leaving the top vanish) revealing the meter rows underneath from
the bottom. Below 0.25 the prompt line shows `clear` executing and the frame is clean. No
lateral motion anywhere.

## Tests specific to `term`

- `rows_are_the_meter`: with bands 0..8 at 0.9 and the rest 0, the bottom meter row's bar has
  ≥ 60 % of the available cells filled and the top row ≤ 10 %.
- `prompt_types_on_onsets_and_scrolls_on_execute`: drive 20 onset frames; the typed length
  grows; after the command completes and a strong onset arrives, the line-number base increments.
- `every_command_and_label_char_has_a_glyph` (CMDS, status strings, trace lines, `>` `▮` `▯`).
- `status_bar_dropped_below_48_rows`: at 380x44 no `#030d22` bar row exists at the bottom; at
  380x60 it does.
- `panic_trace_never_exceeds_the_interior` at 190x48 and 128x44 (forced flourish, no panic, no
  paint outside the panel).
- `the_five_colourways_are_visibly_different` (≥ 15 % of interior pixels pairwise, loud frame).
- `rest_frame_is_not_empty` (prompt, cursor, line numbers, status bar at silence).

## Review focus

1. **128x44**: no status bar, `L` small (≈ 8), line-number column still fits, prompt still shows
   a cursor; nothing outside the panel.
2. **Silence**: prompt + blinking cursor + line numbers + status `▮ 00%` — a terminal waiting.
3. **Flourish at narrow sizes**: trace lines truncated to whole glyphs, scroll stays inside.
4. **Onset starvation** (slow music): typing stalls but the cursor still blinks — the picture
   never freezes.
5. **`term-2077-editor`'s five token colours** must all clear 3:1 on `#030d22` (they do by
   inspection: all are bright).

## Performance

Text-heavy but tiny: ≤ 12 rows × ≤ 90 glyphs × 15 px each ≈ 16k pixel writes per frame plus
scanlines. Target < 0.5 ms steady, < 1.0 ms during the trace. No `bloom`, no `fill_poly`.

## Out of scope

Real shell output, clipboard, any reading of the user's actual terminal; ligatures; a light
colourway (the theme has none).
