# Themes

**150 colourways across 22 families.** A *family* is a renderer with fixed geometry — code. A
*colourway* is data. That split is the extensibility seam: new colourways need no rebuild.

## All 22 families

Six are written up in full below (Segmented VFD, Oscilloscope, VU dials, Vaporwave grid, Valve
row, Fluid). The other sixteen shipped later and their full write-ups still live in
[docs/status.md](status.md) rather than here — that is pre-existing organisation, not a new
split, and worth knowing before you go looking for one and only find it in the other file.

| Family | One-line character |
|---|---|
| Segmented VFD | Smoked-glass panel, discrete stacked segments, peak-hold caps |
| Oscilloscope | Triggered sweep on a graticule with genuine phosphor persistence |
| VU dials | Twin backlit needle dials, dB-mapped, ~300 ms ballistics |
| Vaporwave grid | Slotted sun over a receding perspective grid, bass-triggered lightning |
| Valve row | A rank of vacuum tubes glowing per band inside the glass |
| Fluid | Two submerged subwoofers driving a 1-D wave simulation with real interference |
| Nixie tubes | A rank of nixies; the struck digit climbs, the other nine stay dim unlit wire |
| Spectrogram | The only family with history — frequency up, time scrolling sideways |
| Reel-to-reel | Two spinning reels over a VU strip; motion (rotation, tape sag) is the cue |
| Patchbay | A modular synth panel — jack sockets, sagging patch cables, blinking LEDs |
| Radar | A PPI sweep painting blips as it rotates, 180° fan from the bottom centre |
| Pantone | Felipe Pantone's surface language — chromatic gradient, misregistration, halftone |
| Flame organ | A Rubens' tube: a gas manifold whose flame height traces the wave |
| Dolphin LCD | 1990s car head-unit dot-matrix display, a dolphin arcing over a spectrum |
| 3D spectrum | Winamp/WMP bars in depth — five staggered rows, oblique (not perspective) |
| 3D Pipes | The Windows screensaver, driven — real perspective projection |
| Orbit | Spheres circling in real 3D, pulsing to the music, occlusion as a depth cue |
| Cherry blossom | Petals off a branch in the wind; a lightning storm strikes a castle on a bass hit |
| Kaleidoscope | Frieze-group (not rosette) mirrored symmetry; radius is frequency |
| Rave lasers | A sweeping laser fan that strobes on the kick, for frenchcore |
| Brutalist | Concrete blocks slamming between floor/ceiling on the beat — position, not glow |
| Chroma field | Zero-sum vertical stripes in spectrum order; a swelling stripe pinches its neighbours |

**Five families reuse the `[tube]` table for their own hardware materials**, even though only
one of them is the Valve row (`tube`) family itself: **Cherry blossom**, **Flame organ**,
**Nixie tubes**, **Patchbay** and **Reel-to-reel** all read `t.tube.chassis_top` /
`chassis_bottom` / `internals` / `socket` / `collar` / `glass` for chassis, socket, collar and
glass colours — a blossom theme's moon is `t.tube.glass`, its castle sky is `t.tube.socket` and
`t.tube.collar`. So a theme file overriding one of those five family's colourway needs to set
`[tube]` fields too, not just `[colour]`.

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

A file whose `id` matches a built-in **replaces** it; any other `id` is added alongside the 150
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

