# Theme backlog

Ideas captured during live testing, not yet specced. Recorded here so they survive the
conversation they came from.

---

## 1. Dolphins car stereo

The 1990s/2000s aftermarket head unit — Sony Xplod, Pioneer, JVC — with a dolphin arcing
across a low-res display while a spectrum analyser runs underneath.

What makes it read as that object rather than a generic meter:

- **Coarse dot-matrix pixels.** The display is visibly made of discrete dots with dark
  wells between them. Deliberately chunkier than the VFD family's 5px bars.
- **A single backlight colour** — amber, ice blue or green — with everything drawn in that
  one hue at two or three brightness levels. No gradients.
- **The dolphin is animated and loops**, arcing up out of a waterline and back down,
  crossing the display over several seconds.
- **The meter sits low**, under the dolphin, often only 6–8 segments tall.
- Frequently a **waterline** — a horizontal dotted line the dolphin breaks through.

## 2. Vaporwave sunset

- **Banded sun**: a circle with horizontal slots cut out of it, larger gaps toward the
  bottom.
- **Gradient sky**: magenta → orange → deep purple.
- **Perspective grid** receding to the horizon in cyan or magenta, optionally scrolling
  toward the viewer so it reads as motion.
- Optional foreground silhouette — a low wedge car, or palm trees.
- Optional **lightning**, which has a natural hook: trigger a flash on a bass transient,
  so it reacts to the music rather than being decorative.
- Pixel-art treatment throughout, which suits 190×60 well.

---

## Why these are a new *family*, not new colourways

The existing seam is **colourways are data, families are code**. These two are neither a
recolour nor a variant of the segmented meter: they need a **pictorial backdrop plus
animation**, which the `Family` trait has no concept of. So they are a third thing — call
it a `scene` family — that composites:

1. a backdrop (procedural gradient, or sprite art),
2. an optional animated sprite with its own phase/position state,
3. the meter itself, drawn over or into the scene.

Animation state is fine — families already own per-frame state (the scope's persistence
buffers, the VU's needle ballistics), so a dolphin's position fits the existing shape.

## The interesting part: sprites could stay *data*

Worth exploring before defaulting to hardcoded Rust drawing. A pixel-art sprite is
expressible as palette-indexed rows directly in the theme TOML:

```toml
[sprite.dolphin]
palette = ["", "#0a2a3a", "#4fd8ff", "#c8f4ff"]   # index 0 = transparent
frames = 4
rows = [
  "....2222....",
  "..22333322..",
  ".2333333332.",
  "..22333322..",
]
```

If that works, these themes become **authorable without a rebuild**, like every other
colourway — and the theme-authoring prompt in the README could generate them. That would
be a genuine extension of the extensibility seam rather than an exception to it.

The counter-argument: the vaporwave gradient and the perspective grid are better generated
procedurally than stored as pixels, and the lightning needs to respond to audio, which is
behaviour rather than data. So the honest answer is probably **both** — procedural
backdrop parameters in TOML, plus an optional sprite table for the figurative elements.
Decide it when specced; do not guess now.

## Related: use the empty taskbar to the left

The user noted the display could extend further left. Measured on the reference machine:
app buttons end at **x≈1119** and the Widgets button starts at **x≈1425**, leaving roughly
**300px of dead taskbar** between them.

That matters most for these two themes: a dolphin arcing across 190px is cramped, and a
perspective grid wants width. A `scene` family could claim, say, 456×60 and have room to
breathe.

Caveats to check when specced:
- That gap only exists while the taskbar is **left-aligned** and not full of windows. It
  shrinks as apps open, so the width has to be computed at runtime from the actual gap,
  not assumed.
- The overlay would then span from the last app button to the widget, so it must not cover
  a taskbar button that appears mid-song.
- Rect tracking already re-discovers every second, so the machinery exists; the change is
  computing an extended rect rather than using the widget rect directly.
