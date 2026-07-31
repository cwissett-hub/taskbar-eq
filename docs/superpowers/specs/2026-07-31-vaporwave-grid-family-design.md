# Vaporwave grid family — design

**Date:** 2026-07-31
**Status:** visually approved by the user against an interactive mockup; ready to implement

A fourth theme family. Unlike `segmented` / `scope` / `vu`, which are instruments, this one is a
**scene**: a sunset over a perspective grid where the grid itself carries the audio.

Every number below was tuned by the user in a live browser tuner, not chosen by me. That matters —
several of my own guesses were wrong and got overridden (see §7).

---

## 1. Approved parameters

Copied verbatim from the tuner's final output.

```
mode        = terrain
occlusion   = true
crisp       = true
sunRim      = true
horizon     = 48      # % of height; horizon y = round(H * 0.48)
amp         = 101     # % displacement scale
lines       = 16      # receding horizontal grid lines
verts       = 18      # converging vertical lines
scroll      = 124     # % scroll speed
persp       = 207     # depth-spacing exponent, /100
spread      = 150     # % width spread of the near edge
glow        = 98      # % peak-glow brightness
smoothing   = 65      # % spectral smoothing radius
sun         = 83      # % of the base sun radius
slots       = 6       # horizontal slots cut in the sun
slotBias    = 0       # slot widening toward the horizon - user chose ZERO
slotTop     = 18      # % down the sun where the first slot sits
halo        = 84      # % halo strength
warmth      = 63      # % gradient warmth (higher = pinker at the horizon)
```

Lightning defaults (the user did not move these, so the tuner defaults stand):

```
boltSens    = 55      # % - rise in bass needed to fire
boltBright  = 90      # %
skyFlash    = 35      # %
gridFlash   = 60      # % - grid lines brighten on a strike
boltDecay   = 55      # %
```

## 2. Two techniques that are not optional

Both were discovered by fixing a specific complaint ("it started to look muddy"). Without either, the
theme looks like overlapping spaghetti and there is no point shipping it.

**Hidden-line removal.** Draw the horizontal lines far-to-near, and after building each line's
polyline, fill the area between it and the bottom of the canvas with the opaque ground colour
*before* stroking it. Each ridge then occludes everything behind it. Drawing all the lines and then
stroking them produces the muddiness.

**Half-pixel snapping.** A 1px stroke at a fractional y coordinate anti-aliases across two rows as
grey mush. Snap every plotted y to `round(y) + 0.5` and every x to `round(x)`. At 60px tall this is
the difference between a wireframe and a smear.

## 3. Geometry

With `horizon = round(H * 0.48)`, `groundH = H - horizon - 2`, and `vpx = W / 2`:

- **Depth** `f` runs 0 at the horizon to 1 nearest the viewer.
  `depthY(f) = horizon + f^(persp/100) * groundH`
- **Line half-width** `halfW(f) = (W/2) * (spread/100) * f` — so far lines are narrower, which is
  what makes it recede.
- **Displacement** `ampMax = (horizon - 4) * (amp/100) * 0.55`, and a line at depth `f` is displaced
  by `level * ampMax * f` — nearer lines lift more.
- **Scroll**: lines advance through `f = (k + scroll_phase) / lines`, with `scroll_phase` wrapping 0→1.
- **Frequency** maps left-to-right across each line's own (narrower) width, so `x01 = (x - x0) / (x1 - x0)`.

**Vertical fan — do not get this wrong.** The verticals must reach the canvas corners. Use
`fanSpan = max(W/2, halfW(1.0))` and draw from `(vpx, horizon)` to `(vpx + u*fanSpan, H)` for
`u` spanning −1..1. An earlier version fanned to `halfW(1.0)` alone and stopped short of the edges at
some spread values; the user spotted it.

## 4. The sun

Drawn after the sky and before the ground, so the horizon cuts it off naturally.
`R = round(H * 0.34 * (sun/100))`, centred at `(round(W/2), horizon)`, upper semicircle only.

1. **Halo** first, behind everything: radial gradient from `R*0.55` out to `R*(1.5 + 1.2*halo/100)`,
   warm, alpha `0.30 * halo/100` at the centre falling to zero.
2. **Body**: vertical gradient, clipped to the semicircle —
   `#fff6d0` at the crown → `#ffd76e` at 35% → `#ff9c4a` at 70% → `#ff5f93` at the horizon
   (the last two shift toward orange when `warmth < 50`).
3. **Slots**: `slots` horizontal gaps cut out. Slot *i* sits at
   `pos = slotTop/100 + (1 - slotTop/100) * i/(slots+1)`, thickness
   `max(1, round(1 + (slotBias/100) * pos * max(2, R*0.22)))`.
   **With the approved `slotBias = 0` every slot is 1px and uniform** — the user explicitly preferred
   uniform slots over my widening-toward-the-horizon idea.
4. **Rim**: 1px `rgba(255,250,230,0.75)` arc across the crown, from 1.08π to 1.92π.

## 5. Audio mapping

- **Grid displacement** comes from the band levels, smoothed with a moving average of radius
  `round(smoothing/100 * 5)` bands. At the approved 65 that is radius 3, i.e. rolling hills rather
  than spikes.
- **Peak glow**: line segments whose mean level exceeds 0.55 are re-stroked in `#eafcff` at
  `((v - 0.55)/0.45) * (glow/100) * f`, so loud sections glow and near lines glow more.
- **Lightning** is triggered by a **bass transient**, not a timer: with
  `bass = mean(levels[0..4])`, fire when `bass - prev_bass > 0.04 + (1 - boltSens/100)*0.26` and
  `bass > 0.35`. Decay the strike by `(boltDecay/100) * 0.09` per frame.
  A strike draws a wide dim pass (`#9fe8ff`, 3px, 30% of brightness) then a tight bright core
  (`#eafcff`, 1px), plus one fork starting 45% down. It also flashes the sky and brightens the grid.

The bass-transient state (`prev_bass`, `bolt_hit`, `bolt_seed`) lives **on the family**, which is
consistent with the existing families owning their own per-frame state.

## 6. Canvas primitives this needs — none of which exist yet

`Canvas` currently offers only `fill_rect`, `rounded_rect`, `clip_to_rounded_rect`, `punch_row`,
`punch_rect`, `bloom`, `get`, `bits`. This family additionally requires:

| Primitive | Used for |
|---|---|
| `line(x0, y0, x1, y1, c)` | grid lines, verticals, lightning. 1px, no anti-aliasing — snapping is the point |
| `fill_poly(pts, c)` | hidden-line removal fills, and the sun body |
| `vertical_gradient(rect, stops)` | sky, sun body |
| `radial_gradient(cx, cy, r0, r1, stops)` | sun halo |
| `fill_semicircle(cx, cy, r, c)` or scanline circle | sun body clip |
| `clip` region beyond the existing rounded-rect | confining the sun and bolt to the panel |

These are general-purpose and belong in `canvas.rs` with their own unit tests, not hidden inside the
family. Note the existing `Rgba` is premultiplied-BGRA on store; gradients must premultiply per stop.

## 7. Where my own judgement was wrong

Recorded because it is the useful part of this document.

- I proposed the spectrum as **discrete bars**, then as a **skyline silhouette**, then a **ridgeline**,
  then a **mesh**, then **cloud bands**. The user rejected all of them as not feeling part of the
  scene, and was right — the answer was the grid that was already there.
- I proposed **lightning as the spectrum carrier** and warned it would disappoint. The user's own
  instinct was better: lightning as a **bass-triggered accent**, which is what shipped.
- I proposed sun slots **widening toward the horizon** so it "dissolves". The user set `slotBias = 0`,
  preferring uniform slots.
- I set the vertical fan to `halfW(1.0)`, which stops short of the canvas edges. The user noticed
  before I did.

The pattern: I was reliably right about *rendering technique* (hidden-line removal, half-pixel
snapping, coupling bloom radius to strength) and reliably wrong about *composition*. Build the tool,
let the human drive it.

## 8. Open questions for implementation

- **Time base.** `FrameData` carries no timestamp, so scroll and decay are currently per-frame. At a
  fixed 60fps target that is acceptable, but it means scroll speed drifts if the render loop slows.
  Either add `dt_ms` to `FrameData` or accept the coupling — decide when implementing, and say which.
- **Width.** All of this was tuned at both 190×60 and 456×60. The 456 variant needs the extended-rect
  work (claiming the dead taskbar left of the widget), which is a separate piece with its own caveat
  that the gap shrinks as apps open. Ship at 190 first; the geometry is all relative to `W`.
- **Naming.** These parameter names become a published schema the moment a theme file uses them, and
  renaming later breaks user-authored themes. Name them once, carefully, and keep `schema = 1`.
