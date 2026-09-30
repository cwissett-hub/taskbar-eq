# Status

**Last updated: 2026-08-10.** Full test suite green (520 at the time of writing), release build
warning-free. The colourway and family counts below are asserted by a test; the test count itself is
a snapshot and can drift.
**163 colourways across 25 families.**

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
| ✅ | Right-click equaliser → theme menu; left-click → show now playing | working |
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

**Orbit** — spheres circling in 3D space, pulsing to the music. The same real perspective as 3D Pipes,
and a better fit for this panel for three reasons: a sphere has **no thin features to lose** at 48 rows,
depth arrives three ways at once (size, shading and **occlusion** — a ball passing behind another is
the one cue that cannot be faked in 2D), and a pulse is a **size** change rather than a brightness one,
which is the channel that actually works here.

The orbit is a **wide ellipse**, not a circle, and that is the letterbox talking rather than a fudge:
`x` costs no vertical rows at all, while `z` costs about 2.9 rows per step of depth separation. Spending
the panel's width on `x` and keeping `z` for as much depth as the rows will pay for is the correct trade
— and it happens to be what an orbit seen from slightly above actually looks like. The plane tilts
slowly, which stops the ring being a fixed shape and makes the changing ellipse its own depth cue.

**Cherry blossom** — petals coming off a branch in the wind. The first family whose subject is a
*field of many small things* rather than one instrument, which changes what it has to worry about: not
looking like noise.

Three mappings stop it being decorative, and none is brightness. **Wind is the level** — petals stream
faster and further as the music gets louder, so the whole field's slope tells you the level at a glance.
**A beat shakes the branch and releases a burst**, so the release pattern is the rhythm. **The bass bends
the branch**, giving a slow motion under the fast one.

The branch is the *anchor*, not decoration: a solid, static, recognisable shape that tells the eye what
it is looking at before it has resolved a single petal — and somewhere for the petals to come from.

Petals **tumble** through three masks (face, angled, edge), because a petal that slides without turning
reads as a speck, and each carries its own flutter sine at its own rate so the field never organises
itself into rain.

Every colourway is a dusk. A pale petal needs a dark sky to clear the project's 3:1 contrast rule, so
pink-on-white is the one cherry blossom picture this panel cannot draw — and a near-black sky with
lantern-pale petals turns out to be the more evocative of the two anyway.

The flourish is a **storm**: lightning strikes the castle and the gust lets go of the branch at the same
moment, because they are the same event. Getting it to fire at all was the whole problem. This project has
now three times shipped or nearly shipped a bass trigger that *provably cannot fire* — so the band window
was measured against the repo's real-music fixtures rather than chosen, and there is a test that drives
each fixture **separately**, because an aggregate passes while two of three give zero. At six bands two of
the three fixtures go silent; at eight, only the drum-and-bass one survives. Three bands — the kick's
fundamental, roughly 47–117 Hz — fires on all three, about once every 14 to 27 seconds.

The bolt lives in a corridor exactly ten columns wide: the only columns that both clear the moon disc and
land on castle stone. Its fork is allowed further left, because a fork does not have to land on anything
and clamping it to the same corridor drew it invisibly on top of the trunk.

**Kaleidoscope** — mirrored, repeating psychedelic symmetry. A kaleidoscope normally means *rosette*
symmetry, rotation about one centre, and that does not fit this panel: at 380x60 a centred disc is 60px
across and uses 16% of the width. It is the same failure the isometric experiment hit, and it is not
fixable by tuning.

So the symmetry **group** changes rather than the design shrinking. The symmetry groups of an infinite
strip are the seven *frieze* groups, and those are the groups a 6:1 letterbox actually has — here,
vertical mirror lines every cell plus one horizontal mirror down the middle. The result is a row of
complete four-fold rosettes marching across the panel at full height. A real kaleidoscope's tube view,
unrolled along its length, *is* a frieze; this is not a compromise shape.

**Radius is frequency**, which is what makes it a meter rather than an ornament: the band a pixel reads
is chosen by its distance from the nearest rosette centre, so bass swells the centres and treble lights
the rims, and every rosette is a full radial spectrum. Level stays *position* — it moves where the facet
edges fall, and it drives how fast the pattern turns.

Values are quantised into four flat facets. A smooth radial gradient reads as a lens flare; a
kaleidoscope is coloured glass, and glass has edges — and the quantising is also what makes the mirror
lines visible, which is the entire point.

The geometry is precomputed into tables keyed on the canvas size, because the naive version wants an
`atan2` and a `sqrt` per pixel per frame — 22,800 of each. The tables are per *cell*, a quarter of one
rosette at about 28x28, so the per-frame work is one `sin` per cell pixel and a lookup per panel pixel.

**Rave lasers** — a sweeping laser rig that strobes on the kick, for frenchcore. A fan of beams is the one
piece of the rave vocabulary a 6:1 letterbox *flatters*: a wide fan is cramped in a square frame and
natural in a strip, so for once the aspect ratio is an advantage rather than the thing to design around.
The zooming tunnel and the checkerboard floor both want a vanishing point and fail here for the same
reason a rosette kaleidoscope does.

**The fan's outline is the spectrum.** Each beam owns a slice of the band range and reaches as far as that
slice is loud, so the beam tips trace the spectrum in polar form and the fan's silhouette *is* the meter.
On top of that the whole aperture follows the overall level — beams spread wide when it is loud and
collapse toward a pencil when it is quiet. Both are position, so the house rule holds; what brightness
carries here is *events*, which is a different thing and the entire point of the family.

Flash rate is deliberately unlimited: 200bpm is 3.33 flashes per second against a general 3-per-second
guidance threshold that carries a size exemption a 380x60 taskbar strip comfortably meets, and the user
waived it explicitly. The remaining limit is a *loudness* one worth keeping for its own sake — if every
beat is maximum then none of them is, so the per-kick strobe has a ceiling and every fourth kick goes
above it.

This is the one family where the detector's problem is the opposite of everywhere else. Frenchcore hands
over a distorted kick that dominates the spectrum, on the grid, every beat — so the trigger is a plain
flux detector on the kick's fundamental with a 130ms refractory (about 460bpm), and what had to be
engineered is a visual that survives firing three times a second rather than one that fires at all. The
shared flourish machinery's 180ms would cap at 333bpm and swallow a kick roll, which is exactly the
material this is for.

**Brutalist** — heavy concrete blocks that slam between two orientations on the beat. In one state they
rise from the floor, in the other they hang from the ceiling, and an onset toggles between them, so the
whole panel slams between configurations one to three times a second.

That is a strobe made of **position** rather than brightness, which is what lets it look violent while
leaving the house rule intact. The consequence is intended: flipping destroys frame-to-frame comparability
of the block *tops* — you cannot track a tip across a flip, because it moves the height of the panel. What
stays comparable is block **length**, which is what encodes the level, so the meter is unharmed and the
slam is free. It does mean the peak-hold caps are anchored to each block's own base; anchored to a panel
row they would appear to leap the full height on every beat.

No glow, no gradient, no ornament: `bloom` is 0 on every colourway here and a test enforces it, because a
halo softens exactly the edges this family is about. Half the band count at double the width, because the
subject is mass and a thin bar has none, and the 5px gaps do the work a keyline does elsewhere — at 1–2px
a gap closes up under any halo and the blocks weld into one mass.

**The concrete behaves like concrete.** A slab slamming into the floor with nothing coming off it reads as
a rectangle changing size, so each flip throws **dust** from the surface it hits and every block carries a
few static **cracks**. The dust is ejected *away* from the impacted surface while gravity always pulls
down, which makes the two states differ with no special-casing: a floor slam arcs up and falls back, a
ceiling slam simply rains down. That asymmetry is the physics doing the work, and it also tells the eye
which state the panel is in while the blocks themselves are still moving.

The cracks are static per block and derived from the block's index, never from a per-frame random — a
crack that moved would be noise, and noise is the one thing a family this flat cannot absorb. They are
coloured toward the *background* rather than simply darker, so they read as the panel showing through a
fissure and stay legible at the peak of the monolith, where the body becomes the panel colour exactly.

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

The subsystem is fixed at link time, so there is no runtime switch — but you can ask for output.
The full, current flag list (`--console`, `--diagnose`, `--levels`, `--stress`, `--help`,
`--version`) is quoted verbatim from `usage()` in the README's
[Command line](../README.md#command-line) section rather than duplicated here, since a table
here would drift out of sync with the code exactly the way the three-flag version of this table
already had.

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

