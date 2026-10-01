//! The Virtual Self family: angel wings on Y2K chrome, where the spectrum IS the plumage.
//!
//! Asked for as a "Virtual Self" look - the early-2000s chrome-and-cherub aesthetic - and the meter that
//! carries it is a pair of wings. Thirty-two bands are fanned into two mirrored wings that meet at a
//! central root, low bands at the root and highs at the tips, so THE WINGS BEAT ON THE BASS: a kick
//! lengthens the root feathers and the whole span pulses out from the middle.
//!
//! # Why a wing is a meter and not just an ornament
//!
//! A feather is a bar rotated by its index. Reading a wing is reading a fan of bars whose LENGTHS encode
//! the spectrum - exactly the segmented meter's reading, bent into an arc. The fan runs from -70deg to
//! +70deg through the horizontal, and the same fan is mirrored across the centre to make the second wing,
//! so the display is symmetric by construction and a glance still resolves "where is the energy" the way
//! a straight bar row does. Bass at the root means the loudest, most rhythmic part of most music drives
//! the biggest, most central feathers - the ones the eye lands on first.
//!
//! # The wings span the panel (fidelity pass 2)
//!
//! The user's note on the first cut: "lots of space on the sides that goes unused" - the fan was a
//! circle of radius ~0.24w around the centre, so on a 380x60 strip the wings filled the middle 45% and
//! the outer thirds were floor. The fan is now ELLIPTICAL: a feather's length is measured in a space
//! scaled to the panel - 40% of the width out to each side (an 80% span) and the room above or below
//! the root vertically - so a full-scale feather reaches the side, top or bottom of its own sector
//! instead of all being cut off at the panel height. Lengths saturate at `LEN_GAIN`, so loud real music
//! (whose mids sit at ~0.4) actually reaches the span rather than only a test tone at 1.0.
//!
//! Behind the main pair is an ECHO pair, the Virtual Self "angel" doubling: the same fan at 70% length
//! and 0.35 alpha, rooted a little up and out from the main root so it shows as a second set of wings
//! rather than hiding under the first, and LAGGING 100ms behind it. The lag is a delayed COPY of the
//! main lengths, read out of a ring buffer of past frames - deliberately not a second smoothing of the
//! level (the v0.3.2 high-BPM retune removed exactly that double smoothing, see
//! `slow_vs_high_bpm_response`). On a kick the main wings snap out and the echo follows a beat-fraction
//! later, so every beat reads as a flap with a ghost behind it.
//!
//! On a strong onset a burst of sparks flies from the wing tips out toward the side edges and fades
//! over 500ms, from a fixed pool - the drops light the outer thirds too.
//!
//! # The panel is opaque, and the glow is a 1px halo on the accents only
//!
//! Like every other family, this one paints an opaque panel (dark or ice) that covers the Windows weather
//! widget while music plays - a see-through family would show the forecast through the wings. The wings,
//! grid and chrome are drawn over that panel, and a 2px bevel frames its edge.
//!
//! The glow is confined to the bright accents - the hot tips and the peak dots - because a glow over
//! the whole fan washes the panel flat and destroys the one reading the meter has, its LENGTH. The
//! first cut did that with `Canvas::bloom` on a separate accent layer; that cost ~0.47ms of a 0.65ms
//! frame (three full-frame buffers and two blur passes, every frame) and measured 1.97ms against the
//! 2ms CI gate on the slower runner. It is replaced by a 1px translucent halo drawn under each accent -
//! the same look at a radius this small, at a fraction of the cost and with no layer at all.
//!
//! # The chrome: a bevel, a floor, a lens flare
//!
//! The Y2K surface is three quiet parts around the loud wings:
//!
//! - a 2px bevel frame - light on the top and left, dark on the bottom and right - which is the whole of
//!   the "extruded chrome panel" look at this size;
//! - a single-point perspective floor receding to a horizon at 35% height, drawn in `edge` at
//!   `edge_alpha` so it reads as a floor the wings stand on rather than as a second meter. It runs to
//!   BOTH edges: the converging lines continue past the panel corners, so the outer ones meet the sides,
//!   and the receding horizontals span the full width. They drift slowly toward the viewer always and
//!   faster with the level, and an onset jumps them a whole cell, so the sides move with the beat;
//! - a lens flare - bright core, four streaks, one ring - that bursts from the wing root on a flourish and
//!   fades over 600ms. A flare is the single most Y2K thing a bright light can do.
//!
//! # Colour goes through linear light, and no rainbow
//!
//! Feathers are `lit` at the root shading to `hot` at the tip - a chrome sheen along each feather - mixed
//! through the shared linear-light blend, never by averaging sRGB bytes. There is no rainbow colourway
//! here: the identity is chrome and ice, a restrained Y2K palette, and a hue wheel would fight it.

use crate::dsp::bands::NUM_BANDS;
use crate::render::canvas::{Canvas, Rgba};
use crate::render::{Family, FrameData};
use crate::themes::Theme;

/// Feathers per wing. Every second band (`i*2`), so a wing spans the whole spectrum at half the band
/// resolution - which is what keeps a single feather wide enough to read as a feather rather than a hair.
const FEATHERS: usize = 32;

/// Half the fan angle, in degrees, in the panel-scaled space. The fan runs from `-FAN_DEG` (up) through
/// `0` (out) to `+FAN_DEG` (down).
const FAN_DEG: f32 = 70.0;

/// Where the wing root sits, as a fraction of the panel interior height. Below centre, because a wing
/// hangs from a shoulder above its own midline and the downstroke feathers need the room beneath.
const ROOT_Y_FRAC: f32 = 0.62;

/// The grid's vanishing horizon, as a fraction of the interior height from the top.
const HORIZON_FRAC: f32 = 0.35;

/// Grid line spacing: the converging lines are `GRID_VERTS` cells across the panel's bottom edge, and
/// carry on for `GRID_VERTS_PAST` more cells past each corner so the outer lines meet the side edges.
/// `GRID_HORIZ` receding horizontals.
const GRID_VERTS: i32 = 9;
const GRID_VERTS_PAST: i32 = 4;
const GRID_HORIZ: i32 = 5;

/// How fast the horizontal grid lines scroll, in cells per second: a slow constant drift so the floor
/// is never frozen, plus this much per unit of level. Slow: the floor drifts, it does not race.
const GRID_DRIFT: f32 = 0.3;
const GRID_SCROLL: f32 = 3.5;

/// The onset that jumps the grid forward a whole cell - the same permissive flux net the flourish uses to
/// find candidates, so the floor lurches on the beat without needing its own calibration.
const GRID_ONSET_RATIO: f32 = 2.8;
const GRID_ONSET_REFRACTORY_MS: f32 = 120.0;

/// Each wing reaches this fraction of the panel WIDTH from the centre at full scale: 0.40 is the 80%
/// span. Vertically a full-scale feather reaches the interior edge above or below the root.
const SPAN_FRAC: f32 = 0.40;

/// How far the steep feathers still reach sideways: a feather's horizontal reach is `cos(theta)` raised
/// to this power, so below 1 the top and bottom of the fan sweep OUT as well as up and down - the
/// angel-wing silhouette, whose highest primaries are also its outermost - instead of a round fan that
/// leaves the upper corners of each side empty.
const SWEEP_POW: f32 = 0.4;

/// Feather length in the panel-scaled space (1.0 = full span): a stub plus the level, saturating at
/// `1 / LEN_GAIN` of full scale. Real loud music sits at ~0.4-0.7 per band, so without the gain the wings
/// would only reach the span on a test tone.
const LEN_STUB: f32 = 0.07;
const LEN_GAIN: f32 = 1.8;

/// Each feather is a WEDGE of the fan (a sector, not a line), so neighbouring feathers nearly touch and
/// the wing reads as a solid, notched plumage at any reach. The outer fraction is drawn in `hot`.
const TIP_FRAC: f32 = 0.30;

/// The chrome sheen: the band between the `lit` body and the `hot` tip, starting this far from the tip,
/// drawn in the linear-light midpoint of the two - the bright gradient along a feather.
const SHEEN_FRAC: f32 = 0.55;

/// The fan step between neighbouring feathers, in degrees, and the fraction of it each feather's wedge
/// covers either side of its own angle: 0.4 leaves a fifth of the step open, the notch between feathers.
const FAN_STEP: f32 = 2.0 * FAN_DEG / (FEATHERS - 1) as f32;
const WEDGE_HALF: f32 = 0.55;

/// The echo pair: alpha, length relative to the main pair, how far behind it lags, and where its root
/// sits relative to the main root (out along each side as a fraction of the width, up as a fraction of
/// the interior height). `ECHO_HIST` frames of history cover the lag at up to ~150 fps.
const ECHO_ALPHA: f32 = 0.35;
const ECHO_LEN: f32 = 0.70;
const ECHO_LAG_MS: f32 = 100.0;
const ECHO_HIST: usize = 16;
const ECHO_DX_FRAC: f32 = 0.085;
const ECHO_DY_FRAC: f32 = 0.16;

/// Sparks: a fixed pool, the pairs spawned per strong onset (each pair is mirrored, one per wing), the
/// fade, and the onset that counts as "strong" - a stricter flux ratio than the grid's lurch, gated on
/// the music actually being loud.
const SPARK_POOL: usize = 20;
const SPARK_PAIRS: usize = 8;
const SPARK_MS: f32 = 500.0;
const SPARK_TRAIL: i32 = 7;
const SPARK_ONSET_RATIO: f32 = 4.0;
const SPARK_REFRACTORY_MS: f32 = 250.0;
const SPARK_MIN_LEVEL: f32 = 0.22;

/// How long the lens flare takes to fade, in milliseconds.
const FLARE_MS: f32 = 600.0;

/// The panel is at least this large before the family draws its wings - smaller, and it sheds rather than
/// smudging, exactly as the other families do.
const MIN_W: i32 = 60;
const MIN_H: i32 = 18;

/// The title only appears on a panel at least this tall; below it there is no room under the wings for
/// legible glyphs, so it is gated off and the wings scale to fill the space instead.
const TITLE_MIN_H: i32 = 58;

/// The bevel frame colours - the one fixed chrome in the family, shared by every colourway because a
/// bevel is a lighting effect on the panel edge, not a palette choice.
const BEVEL_LIGHT: &str = "#f4f6fa";
const BEVEL_DARK: &str = "#8a94a6";

/// The chrome the title glyphs run through, top row to bottom: white -> silver -> cobalt -> silver ->
/// white, the classic extruded-metal gradient.
const CHROME_ROWS: [&str; 5] = ["#ffffff", "#f4f6fa", "#1f5bff", "#f4f6fa", "#ffffff"];

/// One spark: position and velocity in pixels and pixels per second, and its age. `age_ms >=
/// SPARK_MS` is a free slot.
#[derive(Clone, Copy)]
struct Spark {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    age_ms: f32,
}

impl Default for Spark {
    fn default() -> Self {
        Spark { x: 0.0, y: 0.0, vx: 0.0, vy: 0.0, age_ms: SPARK_MS }
    }
}

pub struct Vswings {
    /// Fires the lens flare on a rare, exceptional hit - see `dsp::flourish`.
    flourish: crate::dsp::flourish::Trigger,
    /// The flare's one-shot decay envelope.
    flare: crate::dsp::flourish::Envelope,
    /// Scroll position of the receding grid, advanced by level and jumped by an onset.
    grid_phase: f32,
    /// This frame's feather lengths in the panel-scaled space, one per feather. Tracks the
    /// already-smoothed `d.levels` directly - see `draw`.
    feather_len: [f32; FEATHERS],
    /// The echo's delay line: the last `ECHO_HIST` frames of `feather_len`, each stamped with the
    /// family clock, newest at `hist_head`. A COPY of the past, not a filter.
    hist: [[f32; FEATHERS]; ECHO_HIST],
    hist_at: [f32; ECHO_HIST],
    hist_head: usize,
    hist_len: usize,
    /// Milliseconds since the family started drawing, the delay line's clock.
    clock_ms: f32,
    /// The onset that lurches the grid. A local detector rather than the flourish's private one, because
    /// the grid lurches far more often than the flare fires.
    onset: crate::dsp::onset::Flux,
    /// The stricter onset that throws sparks.
    spark_onset: crate::dsp::onset::Flux,
    /// The spark pool - fixed, so a burst allocates nothing.
    sparks: [Spark; SPARK_POOL],
    /// xorshift state for the sparks' scatter; deterministic so the dumps and tests repeat.
    rng: u32,
}

impl Default for Vswings {
    fn default() -> Self {
        Vswings {
            flourish: Default::default(),
            flare: Default::default(),
            grid_phase: 0.0,
            feather_len: [0.0; FEATHERS],
            hist: [[0.0; FEATHERS]; ECHO_HIST],
            hist_at: [0.0; ECHO_HIST],
            hist_head: 0,
            hist_len: 0,
            clock_ms: 0.0,
            onset: Default::default(),
            spark_onset: Default::default(),
            sparks: [Spark::default(); SPARK_POOL],
            rng: 0x9e37_79b9,
        }
    }
}

/// The interior box the wings and grid clip to: `(x0, y0, x1, y1)`.
type Bbox = (f32, f32, f32, f32);

/// The largest `t >= 0` for which `root + t*dir` stays inside the box `[x0,x1] x [y0,y1]`.
///
/// A ray-box clip, so a feather can be grown to its musical length and then cut back to whatever the
/// panel holds along its own direction - the elliptical fan already stays inside by construction, and
/// this makes that a guarantee rather than an assumption.
fn ray_max(root: (f32, f32), dir: (f32, f32), bbox: Bbox) -> f32 {
    let (rx, ry) = root;
    let (ux, uy) = dir;
    let (x0, y0, x1, y1) = bbox;
    let eps = 1.0e-4;
    let mut t = f32::INFINITY;
    if ux > eps {
        t = t.min((x1 - rx) / ux);
    } else if ux < -eps {
        t = t.min((x0 - rx) / ux);
    }
    if uy > eps {
        t = t.min((y1 - ry) / uy);
    } else if uy < -eps {
        t = t.min((y0 - ry) / uy);
    }
    if t.is_finite() {
        t.max(0.0)
    } else {
        0.0
    }
}

/// The elliptical fan's geometry for one wing root: where it is and how far a full-scale feather
/// reaches out, up and down from it.
#[derive(Clone, Copy)]
struct Fan {
    root: (f32, f32),
    reach_x: f32,
    reach_up: f32,
    reach_dn: f32,
}

impl Fan {
    /// The tip of a feather of normalised length `l` at fan angle `theta` (radians) on `side` (+1 right,
    /// -1 left), cut back along its own ray to stay inside `bbox`.
    fn tip(&self, theta: f32, side: f32, l: f32, bbox: Bbox) -> (f32, f32) {
        let (st, ct) = theta.sin_cos();
        let ry = if st < 0.0 { self.reach_up } else { self.reach_dn };
        let (ex, ey) = (side * ct.max(0.0).powf(SWEEP_POW) * self.reach_x * l, st * ry * l);
        let len = (ex * ex + ey * ey).sqrt();
        if !len.is_finite() || len <= 1.0e-3 {
            return self.root;
        }
        let dir = (ex / len, ey / len);
        let len = len.min(ray_max(self.root, dir, bbox));
        (self.root.0 + dir.0 * len, self.root.1 + dir.1 * len)
    }

    /// The two outer corners of feather `i`'s wedge at normalised length `l`.
    fn wedge(&self, i: usize, side: f32, l: f32, bbox: Bbox) -> ((f32, f32), (f32, f32)) {
        let th = Vswings::theta(i);
        let half = (WEDGE_HALF * FAN_STEP).to_radians();
        (self.tip(th - half, side, l, bbox), self.tip(th + half, side, l, bbox))
    }
}

/// The normalised feather length for a level: a stub plus the level, saturating at full span.
fn feather_scale(level: f32) -> f32 {
    let l = if level.is_finite() { level.clamp(0.0, 1.0) } else { 0.0 };
    LEN_STUB + (1.0 - LEN_STUB) * (l * LEN_GAIN).min(1.0)
}

impl Vswings {
    /// A band of one feather's wedge: the part between fractions `s.0` and `s.1` of the way from `root`
    /// out to the wedge's outer corners `tip`. `s.0 = 0` is the whole body from the root. The edge from
    /// the last point back to the first is implicit; `fill_poly` clips to the canvas.
    fn band(c: &mut Canvas, root: (f32, f32), tip: ((f32, f32), (f32, f32)), s: (f32, f32), col: Rgba) {
        let (p1, p2) = tip;
        // Padded half a pixel either side across the wedge, so where a wedge narrows below a pixel near
        // the root its neighbours still overlap it and the body has no pinholes.
        let (dx, dy) = (p1.0 - p2.0, p1.1 - p2.1);
        let n = (dx * dx + dy * dy).sqrt().max(1.0e-3);
        let pad = (dx / n * 0.55, dy / n * 0.55);
        let q = |p: (f32, f32), f: f32, k: f32| {
            (
                (root.0 + (p.0 - root.0) * f + pad.0 * k).round() as i32,
                (root.1 + (p.1 - root.1) * f + pad.1 * k).round() as i32,
            )
        };
        c.fill_poly(&[q(p1, s.0, 1.0), q(p1, s.1, 1.0), q(p2, s.1, -1.0), q(p2, s.0, -1.0)], col);
    }

    /// The 1px halo around a feather's hot tip band: the same band pushed out by a pixel on every side.
    fn halo_band(c: &mut Canvas, root: (f32, f32), tip: ((f32, f32), (f32, f32)), s0: f32, col: Rgba) {
        let (p1, p2) = tip;
        let unit = |x: f32, y: f32| {
            let n = (x * x + y * y).sqrt();
            if n > 1.0e-3 {
                (x / n, y / n)
            } else {
                (0.0, 0.0)
            }
        };
        let side = unit(p1.0 - p2.0, p1.1 - p2.1);
        let o1 = unit(p1.0 - root.0, p1.1 - root.1);
        let o2 = unit(p2.0 - root.0, p2.1 - root.1);
        let at = |p: (f32, f32), f: f32| (root.0 + (p.0 - root.0) * f, root.1 + (p.1 - root.1) * f);
        let (i1, i2) = (at(p1, s0), at(p2, s0));
        let r = |x: f32, y: f32| (x.round() as i32, y.round() as i32);
        c.fill_poly(
            &[
                r(i1.0 + side.0 - o1.0, i1.1 + side.1 - o1.1),
                r(p1.0 + side.0 + o1.0, p1.1 + side.1 + o1.1),
                r(p2.0 - side.0 + o2.0, p2.1 - side.1 + o2.1),
                r(i2.0 - side.0 - o2.0, i2.1 - side.1 - o2.1),
            ],
            col,
        );
    }

    /// The fan angle of feather `i`, in radians, in the panel-scaled space.
    fn theta(i: usize) -> f32 {
        (-FAN_DEG + FAN_STEP * i as f32).to_radians()
    }

    /// Records this frame's lengths in the echo's delay line and returns the newest past frame that is
    /// at least `ECHO_LAG_MS` old (or the oldest held, while the line is still filling).
    fn push_and_read_echo(&mut self) -> [f32; FEATHERS] {
        self.hist_head = (self.hist_head + 1) % ECHO_HIST;
        self.hist[self.hist_head] = self.feather_len;
        self.hist_at[self.hist_head] = self.clock_ms;
        self.hist_len = (self.hist_len + 1).min(ECHO_HIST);
        let mut pick = self.hist_head;
        for back in 0..self.hist_len {
            let idx = (self.hist_head + ECHO_HIST - back) % ECHO_HIST;
            pick = idx;
            if self.clock_ms - self.hist_at[idx] >= ECHO_LAG_MS - 0.5 {
                break;
            }
        }
        self.hist[pick]
    }

    fn next_rand(&mut self) -> f32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }

    /// Throws `SPARK_PAIRS` mirrored pairs from the main wings' tips toward the side edges. Each pair
    /// leaves the tip of one of the outward-pointing feathers, so a burst spreads up and down the wing's
    /// outer edge; speed is set so a spark crosses the remaining distance to its side edge in ~0.3-0.5s.
    fn spawn_sparks(&mut self, fan: Fan, bbox: Bbox, w: f32) {
        let mut placed = 0;
        for slot in 0..SPARK_POOL {
            if placed >= SPARK_PAIRS * 2 {
                break;
            }
            if self.sparks[slot].age_ms < SPARK_MS {
                continue;
            }
            // Find a free partner slot for the mirror twin.
            let Some(twin) = (slot + 1..SPARK_POOL).find(|&k| self.sparks[k].age_ms >= SPARK_MS) else {
                break;
            };
            let i = 6 + (self.next_rand() * (FEATHERS - 12) as f32) as usize;
            let i = i.min(FEATHERS - 1);
            let (tx, ty) = fan.tip(Self::theta(i), 1.0, self.feather_len[i], bbox);
            let dir_y = Self::theta(i).sin();
            let to_edge = (bbox.2 - tx).max(w * 0.25);
            let speed = to_edge / (0.3 + 0.2 * self.next_rand());
            let vy = dir_y * speed * 0.25 + (self.next_rand() - 0.5) * speed * 0.12;
            let age = self.next_rand() * 40.0; // a little stagger, so the burst is not one flat front
            self.sparks[slot] = Spark { x: tx, y: ty, vx: speed, vy, age_ms: age };
            self.sparks[twin] = Spark { x: 2.0 * fan.root.0 - tx, y: ty, vx: -speed, vy, age_ms: age };
            placed += 2;
        }
    }

    /// Advances and draws the sparks: a bright head and a short fading trail back along the flight path,
    /// all fading over `SPARK_MS`. A spark that leaves the interior is retired.
    fn draw_sparks(&mut self, c: &mut Canvas, hot: Rgba, bbox: Bbox, dt: f32) {
        let (x0, y0, x1, y1) = bbox;
        // A 2px head where the panel has the room, 1px on the smallest.
        let head_px = if y1 - y0 >= 46.0 { 2 } else { 1 };
        let head = Rgba::lerp_linear(hot, Rgba::new(255, 255, 255, 255), 0.45);
        for s in self.sparks.iter_mut() {
            if s.age_ms >= SPARK_MS {
                continue;
            }
            s.age_ms += dt;
            s.x += s.vx * dt / 1000.0;
            s.y += s.vy * dt / 1000.0;
            s.vy *= 0.97;
            if s.age_ms >= SPARK_MS || s.x < x0 || s.x > x1 || s.y < y0 || s.y > y1 {
                s.age_ms = SPARK_MS;
                continue;
            }
            // Bright for most of its flight, fading out over the last of it.
            let life = s.age_ms / SPARK_MS;
            let a = 1.0 - life * life;
            let alpha = |k: f32, col: Rgba| Rgba::new(col.r, col.g, col.b, (k.clamp(0.0, 1.0) * 255.0).round() as u8);
            // The trail: a streak back along the velocity, each pixel fainter.
            let sp = (s.vx * s.vx + s.vy * s.vy).sqrt().max(1.0);
            let (ux, uy) = (s.vx / sp, s.vy / sp);
            for k in 1..=SPARK_TRAIL {
                let (px, py) = (s.x - ux * k as f32, s.y - uy * k as f32);
                if px < x0 || px > x1 || py < y0 || py > y1 {
                    break;
                }
                let f = 1.0 - k as f32 / (SPARK_TRAIL + 1) as f32;
                c.fill_rect(px.round() as i32, py.round() as i32, 1, 1, alpha(a * 0.8 * f, hot));
            }
            let (hx, hy) = (s.x.round() as i32, s.y.round() as i32);
            c.fill_rect(hx - (head_px - 1) / 2, hy - (head_px - 1) / 2, head_px, head_px, alpha(a, head));
        }
    }

    /// The single-point perspective floor, edge to edge: lines converging on the vanishing point from
    /// well past both bottom corners (so the outer ones meet the side edges), a horizon line, and
    /// full-width horizontals whose spacing scrolls with `grid_phase`. Drawn in `edge` at `edge_alpha`
    /// so it stays quiet under the wings. Every line is clipped to the interior before it is plotted.
    /// Drawn straight onto the panel, before anything else - `panel` is what it is mixed over.
    fn draw_grid(&self, c: &mut Canvas, t: &Theme, panel: Rgba, cx: f32, bbox: Bbox) {
        let (x0, y0, x1, y1) = bbox;
        if t.edge_alpha <= 0.0 {
            return; // a colourway with no floor - the ghost
        }
        // The floor is the first thing on the opaque panel, so `edge` at `edge_alpha` over it IS one
        // opaque colour - precomputed in linear light, which makes every floor pixel a plain write
        // rather than a blend (the floor was a third of the frame as blended lines).
        let edge = Rgba::from_hex(&t.edge, 1.0);
        let col = Rgba::lerp_linear(panel, edge, t.edge_alpha.clamp(0.0, 1.0));
        let horizon = (y0 + HORIZON_FRAC * (y1 - y0)).round();
        let cell = (x1 - x0) / GRID_VERTS as f32;
        // Converging lines, from the bottom edge (extended past both corners) to the vanishing point,
        // stopping one row short of the horizon so they do not pile up into a knot on it. A line whose
        // bottom end is off the panel is cut where it crosses the side edge.
        for k in -GRID_VERTS_PAST..=GRID_VERTS + GRID_VERTS_PAST {
            let bx = x0 + k as f32 * cell;
            let (mut sx, mut sy) = (bx, y1);
            let edge_x = if bx < x0 { Some(x0) } else if bx > x1 { Some(x1) } else { None };
            if let Some(ex) = edge_x {
                let f = (ex - bx) / (cx - bx);
                sx = ex;
                sy = y1 + f * (horizon - y1);
            }
            let ey = horizon + 1.0;
            if sy <= ey {
                continue;
            }
            let f_end = (y1 - ey) / (y1 - horizon);
            let ex = bx + f_end * (cx - bx);
            c.line(sx.round() as i32, sy.round() as i32, ex.round() as i32, ey as i32, col);
        }
        // The horizon, full width.
        let span = (x1 - x0) as i32 + 1;
        c.fill_rect(x0 as i32, horizon as i32, span, 1, col);
        // Horizontals recede toward the horizon, bunched by a mild perspective exponent and scrolled by
        // the phase so the floor drifts under the beat - full width, as an infinite floor's would be.
        let phase = if self.grid_phase.is_finite() { self.grid_phase.rem_euclid(1.0) } else { 0.0 };
        for j in 0..GRID_HORIZ {
            let tl = (j as f32 + phase) / GRID_HORIZ as f32;
            let yy = (horizon + tl.powf(1.7) * (y1 - horizon)).round();
            if yy <= horizon {
                continue; // sitting on the horizon line already drawn
            }
            // Fainter with distance, so the lines bunching toward the horizon read as depth rather than
            // as a solid grey band across the sides.
            let k = 0.35 + 0.65 * tl;
            let hc = Rgba::lerp_linear(panel, edge, t.edge_alpha.clamp(0.0, 1.0) * k);
            c.fill_rect(x0 as i32, yy as i32, span, 1, hc);
        }
    }

    /// The lens flare at the wing root, drawn CRISP on the main canvas and NEVER bloomed - a bloomed core
    /// is precisely the soft disc that swallowed the wings in the first cut. The SHAPE carries it:
    ///
    /// - a small bright core (a radial gradient a few px across, scaled with height);
    /// - four 1px streaks that fade along their length - a fixed-length cross, so the fade lives in the
    ///   brightness, not in the reach, and the cross stays a cross as it decays;
    /// - a 1px ring that EXPANDS from the root as the envelope decays - small and bright at the peak,
    ///   wide and faint on the way out, the shockwave of the burst.
    ///
    /// Alpha is the envelope throughout.
    fn draw_flare(&self, c: &mut Canvas, t: &Theme, center: (f32, f32), size: (i32, i32), flare: f32) {
        let a = flare.clamp(0.0, 1.0);
        if a <= 0.004 {
            return;
        }
        let (cx, cy) = center;
        let (w, h) = size;
        let (fx, fy) = (cx.round() as i32, cy.round() as i32);
        let core = Rgba::from_hex(&t.hot, 1.0);
        let col = |k: f32| Rgba::new(core.r, core.g, core.b, (k.clamp(0.0, 1.0) * 255.0).round() as u8);

        // The core: a few px across, softly falling.
        let r_core = (5.0 + h as f32 / 30.0).round() as i32;
        c.radial_gradient(fx, fy, 1, r_core, &[(0.0, col(a)), (1.0, col(0.0))]);

        // Four 1px streaks, brightest at the core, fading to nothing at the tip. Length is fixed so the
        // cross is the dominant read at any envelope level.
        let reach = (0.25 * w as f32).round() as i32;
        for (sx, sy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            for s in 1..=reach {
                let f = s as f32 / reach as f32;
                c.fill_rect(fx + sx * s, fy + sy * s, 1, 1, col(a * (1.0 - f)));
            }
        }

        // The expanding ring: radius 4px at the peak out to ~0.3h as it fades.
        let r_ring = (4.0 + (0.3 * h as f32 - 4.0).max(0.0) * (1.0 - a)).round() as i32;
        if r_ring >= 3 {
            let ring = col(a * 0.9);
            let steps = (r_ring as f32 * 6.5).round() as i32 + 8;
            for i in 0..steps {
                let ang = i as f32 / steps as f32 * std::f32::consts::TAU;
                let px = fx + (r_ring as f32 * ang.cos()).round() as i32;
                let py = fy + (r_ring as f32 * ang.sin()).round() as i32;
                c.fill_rect(px, py, 1, 1, ring);
            }
        }
    }

    /// "VIRTUAL SELF" in a 3x5 chrome font with a 1px black outline.
    ///
    /// A private glyph table rather than the canvas's `text_3x5`, because that font deliberately omits
    /// V, T, S and E as ambiguous at 3x5 in its own context - here the string is fixed and known, so the
    /// letters can be drawn unambiguously and the whole title reads. Both passes are opaque, so the
    /// outline-then-chrome order needs no layer: the chrome simply overwrites its own outline's centre.
    fn draw_title(c: &mut Canvas, w: i32) {
        const TITLE: &str = "VIRTUAL SELF";
        let tw = TITLE.chars().count() as i32 * 4 - 1;
        if w < tw + 6 {
            return; // no room to centre it without clipping glyphs
        }
        let black = Rgba::new(0, 0, 0, 255);
        let x0 = (w - tw) / 2;
        let y0 = 2;
        let chrome: [Rgba; 5] = std::array::from_fn(|i| Rgba::from_hex(CHROME_ROWS[i], 1.0));
        // Pass 1: the black outline, a 3x3 block around every set pixel.
        for (ci, ch) in TITLE.chars().enumerate() {
            let rows = title_glyph(ch);
            for (dy, row) in rows.iter().enumerate() {
                for dx in 0..3 {
                    if row & (0b100 >> dx) != 0 {
                        let px = x0 + ci as i32 * 4 + dx;
                        let py = y0 + dy as i32;
                        c.fill_rect(px - 1, py - 1, 3, 3, black);
                    }
                }
            }
        }
        // Pass 2: the chrome gradient, on top of its own outline.
        for (ci, ch) in TITLE.chars().enumerate() {
            let rows = title_glyph(ch);
            for (dy, row) in rows.iter().enumerate() {
                for dx in 0..3 {
                    if row & (0b100 >> dx) != 0 {
                        let px = x0 + ci as i32 * 4 + dx;
                        let py = y0 + dy as i32;
                        c.fill_rect(px, py, 1, 1, chrome[dy]);
                    }
                }
            }
        }
    }
}

/// One glyph of the title font, five rows of three bits with bit 2 leftmost. Unknown characters are a
/// blank cell, so the string can only ever degrade to a gap.
fn title_glyph(ch: char) -> [u8; 5] {
    match ch {
        'V' => [0b101, 0b101, 0b101, 0b101, 0b010],
        'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
        'R' => [0b110, 0b101, 0b110, 0b101, 0b101],
        'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
        'U' => [0b101, 0b101, 0b101, 0b101, 0b111],
        'A' => [0b010, 0b101, 0b111, 0b101, 0b101],
        'L' => [0b100, 0b100, 0b100, 0b100, 0b111],
        'S' => [0b011, 0b100, 0b010, 0b001, 0b110],
        'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
        'F' => [0b111, 0b100, 0b110, 0b100, 0b100],
        _ => [0, 0, 0, 0, 0],
    }
}

impl Family for Vswings {
    fn id(&self) -> &'static str {
        "vswings"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());
        if w < MIN_W || h < MIN_H {
            return; // shed rather than smudge
        }
        let dt = if d.dt_ms.is_finite() { d.dt_ms.clamp(0.0, 250.0) } else { 16.7 };
        self.clock_ms += dt;
        if !self.clock_ms.is_finite() || self.clock_ms > 1.0e9 {
            // Restart the clock (and with it the delay line) rather than lose f32 resolution.
            self.clock_ms = 0.0;
            self.hist_len = 0;
        }

        // The interior the wings and grid live in - inside the 2px bevel, so nothing overdraws the frame.
        let (ix0, iy0) = (2.0f32, 2.0f32);
        let (ix1, iy1) = ((w - 3) as f32, (h - 3) as f32);
        let bbox = (ix0, iy0, ix1, iy1);
        let cx = w as f32 / 2.0;
        let root_y = iy0 + ROOT_Y_FRAC * (iy1 - iy0);

        let panel = Rgba::from_hex(&t.panel, t.panel_alpha);
        let lit = Rgba::from_hex(&t.lit, 1.0);
        let hot = Rgba::from_hex(&t.hot, 1.0);
        let sheen = Rgba::lerp_linear(lit, hot, 0.5);
        // The feather shafts: a line down every second feather, a step toward the panel from `lit`, so
        // the solid wing reads as separate feathers.
        let shaft = Rgba::lerp_linear(lit, panel, 0.45);

        // ---- the opaque panel ----
        //
        // This family covers the widget like the others - see the module note. Filled first; a
        // `clip_to_rounded_rect` at the very end keeps anything (bevel included) off the rounded corners.
        c.rounded_rect(1, 2, w - 2, h - 4, 3, panel);

        // ---- the floor's scroll, the onsets ----
        let rms = if d.rms_l.is_finite() { d.rms_l.clamp(0.0, 1.0) } else { 0.0 };
        let onset = self.onset.update(&d.levels, dt, GRID_ONSET_RATIO, GRID_ONSET_REFRACTORY_MS);
        self.grid_phase += (GRID_DRIFT + rms * GRID_SCROLL) * (dt / 1000.0);
        if onset {
            self.grid_phase += 1.0 / GRID_HORIZ as f32;
        }
        if !self.grid_phase.is_finite() {
            self.grid_phase = 0.0;
        }
        self.grid_phase = self.grid_phase.rem_euclid(1.0);
        let strong = self.spark_onset.update(&d.levels, dt, SPARK_ONSET_RATIO, SPARK_REFRACTORY_MS);

        // ---- this frame's feather lengths, and the echo's delayed copy of them ----
        //
        // `d.levels` has already been through `Smoother::new(theme.ballistics)` upstream in
        // `Ticker::tick` - re-smoothing it here with the SAME ballistics is a second low-pass pass, not a
        // second effect. Length tracks the already-smoothed level directly. The echo is a DELAYED COPY
        // of these lengths (a ring buffer), never a filtered version of them.
        let mut loud = 0.0f32;
        for i in 0..FEATHERS {
            let band = (i * 2).min(NUM_BANDS - 1);
            let level_i = if d.levels[band].is_finite() { d.levels[band].clamp(0.0, 1.0) } else { 0.0 };
            loud += level_i;
            self.feather_len[i] = feather_scale(level_i);
        }
        loud /= FEATHERS as f32;
        let echo = self.push_and_read_echo();

        let fan = Fan {
            root: (cx, root_y),
            reach_x: SPAN_FRAC * w as f32,
            reach_up: root_y - iy0 - 0.5,
            reach_dn: iy1 - root_y - 0.5,
        };

        // ---- the receding floor, edge to edge, first on the panel: the wings stand on it ----
        self.draw_grid(c, t, panel, cx, bbox);

        // ---- the echo pair, behind the main wings ----
        //
        // Two fans rooted a little up and out from the main root (one per side), 70% length, at 0.35
        // alpha. Mixed over the opaque panel, so 0.35 alpha IS one opaque colour - precomputed in linear
        // light, which makes the echo an opaque fill (no per-pixel blend), exactly what a translucent
        // fill over the panel would have produced. It hides the quiet floor where it crosses it.
        let echo_body = Rgba::lerp_linear(panel, lit, ECHO_ALPHA);
        let echo_tip = Rgba::lerp_linear(panel, hot, ECHO_ALPHA);
        let echo_shaft = Rgba::lerp_linear(panel, lit, ECHO_ALPHA * 0.5);
        let ey = root_y - ECHO_DY_FRAC * (iy1 - iy0);
        for side in [1.0f32, -1.0] {
            let ex = cx + side * ECHO_DX_FRAC * w as f32;
            let efan = Fan {
                root: (ex, ey),
                reach_x: fan.reach_x,
                reach_up: ey - iy0 - 0.5,
                reach_dn: iy1 - ey - 0.5,
            };
            for (i, &el) in echo.iter().enumerate() {
                let tip = efan.wedge(i, side, el * ECHO_LEN, bbox);
                Self::band(c, efan.root, tip, (0.0, 1.0 - TIP_FRAC), echo_body);
                Self::band(c, efan.root, tip, (1.0 - TIP_FRAC, 1.0), echo_tip);
            }
            // The echo's own shafts, so the ghost pair reads as feathers too and not as a flat blob.
            for i in (1..FEATHERS).step_by(2) {
                let (tx, ty) = efan.tip(Self::theta(i), side, echo[i] * ECHO_LEN, bbox);
                let (sx, sy) = (ex + (tx - ex) * 0.25, ey + (ty - ey) * 0.25);
                let (fx, fy) = (ex + (tx - ex) * 0.9, ey + (ty - ey) * 0.9);
                if (fx - sx).abs() + (fy - sy).abs() >= 4.0 {
                    c.line(sx.round() as i32, sy.round() as i32, fx.round() as i32, fy.round() as i32, echo_shaft);
                }
            }
        }

        // ---- the glow: a 1px halo around every hot tip, drawn BEFORE the bodies ----
        //
        // The halo replaces the old per-frame `bloom` of an accent layer (see the module note). Drawn
        // before any wing body, so the bodies and tips cover all of it but the 1px fringe outside the
        // wing's own silhouette - which is the only part of a bloom that showed on an opaque panel
        // anyway. That fringe lies on the panel (or the quiet floor and echo), so `hot` at the glow
        // alpha over the panel is again one precomputed opaque colour: no per-pixel blend.
        let glow = if t.bloom > 0.0 { (t.glow_strength * 1.25).clamp(0.0, 1.0) } else { 0.0 };
        if glow > 0.0 {
            let halo = Rgba::lerp_linear(panel, hot, glow);
            for i in 0..FEATHERS {
                for side in [1.0f32, -1.0] {
                    let tip = fan.wedge(i, side, self.feather_len[i], bbox);
                    Self::halo_band(c, fan.root, tip, 1.0 - TIP_FRAC, halo);
                }
            }
        }

        // ---- the main wings: crisp bodies ----
        //
        // The same fan, mirrored across the centre: `side = +1` is the right wing, `-1` the left, so the
        // display is symmetric by construction and `wings_are_mirrored_about_the_centre` holds.
        for i in 0..FEATHERS {
            for side in [1.0f32, -1.0] {
                let tip = fan.wedge(i, side, self.feather_len[i], bbox);
                Self::band(c, fan.root, tip, (0.0, 1.0 - SHEEN_FRAC), lit);
                Self::band(c, fan.root, tip, (1.0 - SHEEN_FRAC, 1.0 - TIP_FRAC), sheen);
            }
        }

        // ---- then the accents: hot tips and peak dots ----
        //
        // After every body, so a later feather's body never covers an earlier feather's tip. A peak dot
        // sits out past its feather, so its own small halo is a true blend over whatever is there.
        let dot_halo = Rgba::new(hot.r, hot.g, hot.b, (glow * 255.0).round() as u8);
        for i in 0..FEATHERS {
            let band = (i * 2).min(NUM_BANDS - 1);
            let peak_i = if d.peaks[band].is_finite() { d.peaks[band].clamp(0.0, 1.0) } else { 0.0 };
            let peak_l = feather_scale(peak_i);
            for side in [1.0f32, -1.0] {
                let tip = fan.wedge(i, side, self.feather_len[i], bbox);
                Self::band(c, fan.root, tip, (1.0 - TIP_FRAC, 1.0), hot);
                // Peak-hold dot, where the peak sits ahead of the current length.
                if peak_l > self.feather_len[i] + 0.02 {
                    let (px, py) = fan.tip(Self::theta(i), side, peak_l, bbox);
                    let (dx, dy) = (px.round() as i32, py.round() as i32);
                    c.fill_rect(dx - 2, dy - 2, 4, 4, dot_halo);
                    c.fill_rect(dx - 1, dy - 1, 2, 2, hot);
                }
            }
        }

        // ---- the shafts, over the tips, on every second feather ----
        for i in (1..FEATHERS).step_by(2) {
            for side in [1.0f32, -1.0] {
                let (tx, ty) = fan.tip(Self::theta(i), side, self.feather_len[i], bbox);
                let (sx, sy) = (cx + (tx - cx) * 0.2, root_y + (ty - root_y) * 0.2);
                let (ex, ey) = (cx + (tx - cx) * 0.92, root_y + (ty - root_y) * 0.92);
                if (ex - sx).abs() + (ey - sy).abs() >= 4.0 {
                    c.line(sx.round() as i32, sy.round() as i32, ex.round() as i32, ey.round() as i32, shaft);
                }
            }
        }

        // ---- advance the flourish envelope (the flare itself is drawn crisp, below the bevel) ----
        let fired = self.flourish.update(&d.levels, dt, t.flourish);
        let flare = self.flare.update(fired, dt, FLARE_MS);

        // ---- sparks from the tips toward the sides, on a strong onset in loud music ----
        if (strong && loud >= SPARK_MIN_LEVEL) || fired {
            self.spawn_sparks(fan, bbox, w as f32);
        }
        self.draw_sparks(c, hot, bbox, dt);

        // ---- the flourish: a lens flare from the wing root, crisp and never bloomed ----
        self.draw_flare(c, t, (cx, root_y), (w, h), flare);

        // ---- the chrome bevel frame ----
        //
        // The extruded-panel edge: light on the top and left, dark on the bottom and right. Drawn INSIDE
        // the panel rect (at x/y 1..) so the final clip keeps it - the panel fill starts at x=1,y=2.
        let bevel_l = Rgba::from_hex(BEVEL_LIGHT, 1.0);
        let bevel_d = Rgba::from_hex(BEVEL_DARK, 1.0);
        c.fill_rect(1, 2, w - 2, 2, bevel_l);
        c.fill_rect(1, 2, 2, h - 4, bevel_l);
        c.fill_rect(1, h - 4, w - 2, 2, bevel_d);
        c.fill_rect(w - 3, 2, 2, h - 4, bevel_d);

        // ---- the title, only where it fits, on top of everything ----
        if h >= TITLE_MIN_H {
            Self::draw_title(c, w);
        }

        // Keep nothing on the rounded corners - the panel, the bevel and the wings are all authored
        // inside this rect, and this makes that a guarantee rather than an assumption.
        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::canvas::Canvas;

    fn theme(id: &str) -> Theme {
        crate::themes::builtin::all().into_iter().find(|t| t.id == id).unwrap()
    }
    fn frames(fam: &mut Vswings, t: &Theme, w: i32, h: i32, level: f32, n: usize) -> Canvas {
        let mut c = Canvas::new(w, h);
        let mut d = FrameData::default();
        for (i, v) in d.levels.iter_mut().enumerate() {
            *v = level * (1.0 - i as f32 / 96.0);
        }
        d.peaks = d.levels;
        d.rms_l = level;
        d.rms_r = level;
        d.dt_ms = 16.7;
        for k in 0..n {
            d.time_s = k as f32 * 0.0167;
            fam.draw(&mut c, t, &d);
        }
        c
    }
    /// Pixels the family DREW OVER the panel - opaque and differing from the panel colour by more than a
    /// rounding margin. The panel is opaque, so "lit" is paint on top of it, not raw alpha.
    fn drew_over_panel(px: Rgba, panel: Rgba) -> bool {
        let d = (px.r as i32 - panel.r as i32).abs()
            + (px.g as i32 - panel.g as i32).abs()
            + (px.b as i32 - panel.b as i32).abs();
        px.a > 8 && d > 24
    }
    fn lit(c: &Canvas, t: &Theme) -> usize {
        let panel = Rgba::from_hex(&t.panel, 1.0);
        let mut n = 0;
        for y in 0..c.height() {
            for x in 0..c.width() {
                if drew_over_panel(c.get(x, y), panel) {
                    n += 1;
                }
            }
        }
        n
    }

    #[test]
    fn registered_and_labelled() {
        assert!(crate::render::KNOWN_FAMILIES.contains(&"vswings"));
        assert_eq!(crate::render::family_for("vswings").id(), "vswings");
        assert_ne!(crate::themes::family_label("vswings"), "vswings");
        assert_eq!(crate::themes::builtin::all().iter().filter(|t| t.family == "vswings").count(), 5);
    }

    #[test]
    fn wings_are_mirrored_about_the_centre() {
        let t = theme("vswings-particle-arts");
        let c = frames(&mut Vswings::default(), &t, 380, 48, 0.6, 30);
        // Painted-over-panel count in the left half vs the right half within 10%: the two wings are a
        // mirror pair.
        let panel = Rgba::from_hex(&t.panel, 1.0);
        let (mut l, mut r) = (0usize, 0usize);
        for y in 0..48 {
            for x in 0..380 {
                if drew_over_panel(c.get(x, y), panel) {
                    if x < 190 {
                        l += 1
                    } else {
                        r += 1
                    }
                }
            }
        }
        assert!((l as f32 - r as f32).abs() < 0.10 * l.max(r) as f32, "left {l} right {r}");
    }

    #[test]
    fn louder_bass_makes_longer_root_feathers() {
        let t = theme("vswings-particle-arts");
        let quiet = frames(&mut Vswings::default(), &t, 380, 48, 0.2, 30);
        let loud = frames(&mut Vswings::default(), &t, 380, 48, 0.8, 30);
        assert!(lit(&loud, &t) > lit(&quiet, &t) + 200, "quiet {} loud {}", lit(&quiet, &t), lit(&loud, &t));
    }

    #[test]
    fn rest_frame_is_not_empty() {
        let t = theme("vswings-eon-break");
        let c = frames(&mut Vswings::default(), &t, 380, 48, 0.0, 10);
        assert!(lit(&c, &t) as f32 >= 0.02 * (380 * 48) as f32, "grid should still show: {}", lit(&c, &t));
    }

    #[test]
    fn fits_the_narrow_panel_and_flourish_does_not_panic() {
        for id in ["vswings-particle-arts", "vswings-eon-break", "vswings-angel-voices", "vswings-utopia", "vswings-ghost"] {
            let t = theme(id);
            let mut fam = Vswings::default();
            let _ = frames(&mut fam, &t, 190, 48, 0.5, 5);
            fam.flourish.force_next();
            let c = frames(&mut fam, &t, 190, 48, 0.5, 40);
            assert!(lit(&c, &t) > 0, "{id}");
        }
    }

    /// One frame of a flat-ish spectrum at `level`, the same shape `frames` feeds.
    fn flat(level: f32) -> FrameData {
        let mut d = FrameData::default();
        for (i, v) in d.levels.iter_mut().enumerate() {
            *v = level * (1.0 - i as f32 / 96.0);
        }
        d.peaks = d.levels;
        d.rms_l = level;
        d.rms_r = level;
        d.dt_ms = 16.7;
        d
    }

    /// The exact colours the main wings are painted in (body, sheen, tip) - opaque fills, so a pixel of
    /// the main pair matches one of these exactly and nothing else on the panel does.
    fn main_colours(t: &Theme) -> [Rgba; 3] {
        let (lit, hot) = (Rgba::from_hex(&t.lit, 1.0), Rgba::from_hex(&t.hot, 1.0));
        [lit, Rgba::lerp_linear(lit, hot, 0.5), hot]
    }

    /// The echo pair's exact colours (body, tip): `lit`/`hot` at the echo alpha over the panel.
    fn echo_colours(t: &Theme) -> [Rgba; 2] {
        let panel = Rgba::from_hex(&t.panel, t.panel_alpha);
        let (lit, hot) = (Rgba::from_hex(&t.lit, 1.0), Rgba::from_hex(&t.hot, 1.0));
        [Rgba::lerp_linear(panel, lit, ECHO_ALPHA), Rgba::lerp_linear(panel, hot, ECHO_ALPHA)]
    }

    fn same(a: Rgba, b: Rgba) -> bool {
        (a.r, a.g, a.b, a.a) == (b.r, b.g, b.b, b.a)
    }

    /// How far from the centre column the given colours reach, in pixels (0 if absent).
    fn extent(c: &Canvas, cols: &[Rgba]) -> i32 {
        let cx = c.width() / 2;
        let mut best = 0;
        for y in 0..c.height() {
            for x in 0..c.width() {
                let px = c.get(x, y);
                if cols.iter().any(|&k| same(px, k)) {
                    best = best.max((x - cx).abs());
                }
            }
        }
        best
    }

    #[test]
    fn the_wings_span_most_of_the_panel() {
        // At loud, the main pair's own paint reaches past 15% and 85% of the width on both sides - the
        // first cut's fan stopped at ~27% / 73%. Measured by the wings' exact colours, so the floor, the
        // echo and the sparks cannot stand in for them.
        for (w, h) in [(380, 60), (190, 48)] {
            let t = theme("vswings-eon-break");
            let c = frames(&mut Vswings::default(), &t, w, h, 0.8, 30);
            let cols = main_colours(&t);
            let (mut left, mut right) = (false, false);
            for y in 0..h {
                for x in 0..w {
                    if cols.iter().any(|&k| same(c.get(x, y), k)) {
                        left |= (x as f32) < 0.15 * w as f32;
                        right |= (x as f32) > 0.85 * w as f32;
                    }
                }
            }
            assert!(left && right, "{w}x{h}: wing paint beyond 15% {left}, beyond 85% {right}");
            // And quiet music does NOT: the span is the meter, not a fixed ornament.
            let q = frames(&mut Vswings::default(), &t, w, h, 0.1, 30);
            let reach = extent(&q, &cols);
            assert!((reach as f32) < 0.25 * w as f32, "{w}x{h}: quiet wings reach {reach}px");
        }
    }

    #[test]
    fn an_echo_pair_lags_behind() {
        // Loud, then a step down. The main pair follows at once (no in-draw smoothing); the echo holds
        // its loud extent for ~100ms - wider than the main pair throughout - and only then follows.
        let t = theme("vswings-eon-break");
        let mut fam = Vswings::default();
        let mut c = Canvas::new(380, 60);
        for _ in 0..60 {
            fam.draw(&mut c, &t, &flat(0.8));
        }
        // (At a steady loud level the 70%-length echo sits wholly behind the main pair - nothing to
        // measure until the main pair moves.)
        let main_before = extent(&c, &main_colours(&t));
        let mut log = Vec::new();
        for k in 1..=14 {
            c.clear();
            fam.draw(&mut c, &t, &flat(0.05));
            log.push((k, extent(&c, &main_colours(&t)), extent(&c, &echo_colours(&t))));
        }
        let (_, main_1, _) = log[0];
        assert!(main_1 * 2 < main_before, "the main pair must drop at once: {main_before} -> {main_1}");
        // Frames 1-5 (up to ~84ms): the echo still shows the loud wings, past the main pair, unmoved.
        let echo_before = log[0].2;
        for &(k, m, e) in &log[..5] {
            assert!(e > m + 20, "frame {k}: echo {e}px should still reach past main {m}px; {log:?}");
            assert!(e * 10 >= echo_before * 9, "frame {k}: echo {e}px moved before its lag ({echo_before}px)");
        }
        // By ~150ms it has followed.
        let (_, _, e9) = log[8];
        assert!(e9 * 10 < echo_before * 7, "the echo should have followed by ~150ms: {echo_before} -> {e9}; {log:?}");
    }

    #[test]
    fn the_floor_reaches_both_edges() {
        // The floor's paint (grey on Eon Break: white at a low alpha over black) is in the outermost
        // columns on BOTH sides, in the far half of the floor too - the first cut's floor only touched
        // the sides at the bottom corners. And it moves there at a steady level: the horizontals scroll.
        let t = theme("vswings-eon-break");
        let (w, h) = (380, 60);
        let floor = |px: Rgba| px.r == px.g && px.g == px.b && px.r > 8 && px.r < 200;
        let mut fam = Vswings::default();
        let mut c = Canvas::new(w, h);
        let snap = |c: &Canvas| -> Vec<(i32, i32)> {
            let mut v = Vec::new();
            for y in 4..h - 4 {
                for x in (3..9).chain(w - 9..w - 3) {
                    if floor(c.get(x, y)) {
                        v.push((x, y));
                    }
                }
            }
            v
        };
        for _ in 0..30 {
            fam.draw(&mut c, &t, &flat(0.4));
        }
        let a = snap(&c);
        let horizon = 2.0 + HORIZON_FRAC * (h - 5) as f32;
        let mid = horizon + 0.5 * ((h - 3) as f32 - horizon);
        for (lo, hi) in [(3, 9), (w - 9, w - 3)] {
            let far = a.iter().filter(|&&(x, y)| x >= lo && x < hi && (y as f32) <= mid).count();
            let all = a.iter().filter(|&&(x, _)| x >= lo && x < hi).count();
            assert!(far >= 6 && all >= 20, "columns {lo}..{hi}: {far} far-floor px, {all} in all");
        }
        for _ in 0..40 {
            fam.draw(&mut c, &t, &flat(0.4));
        }
        let b = snap(&c);
        assert_ne!(a, b, "the floor at the sides should scroll at a steady level");
    }

    #[test]
    fn sparks_fly_toward_the_sides_on_strong_onsets() {
        let t = theme("vswings-eon-break");
        let (w, h) = (380, 60);
        let live = |f: &Vswings| f.sparks.iter().filter(|s| s.age_ms < SPARK_MS).count();
        // Bright spark paint in the outermost 8% of the width, where neither pair of wings reaches.
        let outer_sparks = |c: &Canvas| {
            let mut n = 0;
            for y in 0..h {
                for x in (3..(0.08 * w as f32) as i32).chain(w - (0.08 * w as f32) as i32..w - 3) {
                    let px = c.get(x, y);
                    if px.b > 200 && px.g > 150 {
                        n += 1;
                    }
                }
            }
            n
        };

        // A steady loud level throws nothing; nor does a small rise in quiet music.
        let mut fam = Vswings::default();
        let _ = frames(&mut fam, &t, w, h, 0.8, 60);
        assert_eq!(live(&fam), 0, "a steady level has no onsets");
        let mut fam = Vswings::default();
        let _ = frames(&mut fam, &t, w, h, 0.03, 60);
        let _ = frames(&mut fam, &t, w, h, 0.12, 10);
        assert_eq!(live(&fam), 0, "a weak onset in quiet music throws no sparks");

        // A drop: quiet, then loud.
        let mut fam = Vswings::default();
        let mut c = Canvas::new(w, h);
        for _ in 0..60 {
            fam.draw(&mut c, &t, &flat(0.08));
        }
        fam.draw(&mut c, &t, &flat(0.9));
        let n = live(&fam);
        assert!((12..=20).contains(&n), "a strong onset throws 12-20 sparks, got {n}");
        // Each one heads for its own side: left of centre flying left, right of centre flying right,
        // further out three frames later than it was thrown.
        let cx = w as f32 / 2.0;
        let start: Vec<(usize, f32)> =
            fam.sparks.iter().enumerate().filter(|(_, s)| s.age_ms < SPARK_MS).map(|(i, s)| (i, (s.x - cx).abs())).collect();
        let (left, right) = (
            fam.sparks.iter().filter(|s| s.age_ms < SPARK_MS && s.x < cx).count(),
            fam.sparks.iter().filter(|s| s.age_ms < SPARK_MS && s.x > cx).count(),
        );
        assert_eq!(left, right, "sparks are thrown in mirrored pairs");
        let mut seen_outer = 0;
        for k in 0..12 {
            c.clear();
            fam.draw(&mut c, &t, &flat(0.9));
            seen_outer += outer_sparks(&c);
            if k == 2 {
                for &(i, d0) in &start {
                    let s = fam.sparks[i];
                    if s.age_ms < SPARK_MS {
                        assert!((s.x - cx).abs() > d0 + 4.0, "spark {i} should fly outward: {d0:.0} -> {:.0}", (s.x - cx).abs());
                    }
                }
            }
        }
        assert!(seen_outer > 0, "no spark paint reached the outer 8% of the panel");
        // And they are gone by 500ms (+ a frame of slack).
        for _ in 0..20 {
            fam.draw(&mut c, &t, &flat(0.9));
        }
        assert_eq!(live(&fam), 0, "sparks outlive their 500ms fade");
    }

    #[test]
    fn the_span_clamps_on_the_smallest_panel() {
        // 128x44, a drop to full scale with the sparks in flight: every feather tip of both pairs and
        // every live spark stays inside the interior (inside the bevel), and the wings still reach the
        // outer sixths - clamped to the panel, not shrunk away from it.
        let t = theme("vswings-eon-break");
        let (w, h) = (128, 44);
        let bbox = (2.0, 2.0, (w - 3) as f32, (h - 3) as f32);
        let inside = |p: (f32, f32)| p.0 >= bbox.0 - 0.5 && p.0 <= bbox.2 + 0.5 && p.1 >= bbox.1 - 0.5 && p.1 <= bbox.3 + 0.5;
        let mut fam = Vswings::default();
        let mut c = Canvas::new(w, h);
        for _ in 0..40 {
            fam.draw(&mut c, &t, &flat(0.05));
        }
        let cols = main_colours(&t);
        for k in 0..20 {
            c.clear();
            fam.draw(&mut c, &t, &flat(1.0));
            let root_y = 2.0 + ROOT_Y_FRAC * (bbox.3 - bbox.1);
            let fan = Fan { root: (w as f32 / 2.0, root_y), reach_x: SPAN_FRAC * w as f32, reach_up: root_y - 2.5, reach_dn: bbox.3 - root_y - 0.5 };
            for i in 0..FEATHERS {
                for side in [1.0f32, -1.0] {
                    let (a, b) = fan.wedge(i, side, 1.0, bbox);
                    assert!(inside(a) && inside(b), "frame {k}: feather {i} tip {a:?} {b:?} leaves the interior");
                }
            }
            for s in fam.sparks.iter().filter(|s| s.age_ms < SPARK_MS) {
                assert!(inside((s.x, s.y)), "frame {k}: a spark at ({}, {}) left the interior", s.x, s.y);
            }
            if k >= 2 {
                let (mut l, mut r) = (false, false);
                for y in 0..h {
                    for x in 0..w {
                        if cols.iter().any(|&q| same(c.get(x, y), q)) {
                            l |= (x as f32) < 0.17 * w as f32;
                            r |= (x as f32) > 0.83 * w as f32;
                        }
                    }
                }
                assert!(l && r, "frame {k}: the clamped wings should still reach the outer sixths");
            }
        }
        assert!(fam.sparks.iter().any(|s| s.age_ms < SPARK_MS), "the drop should have sparks in flight");
    }

    #[test]
    #[ignore]
    fn probe_vswings_cost() {
        let t = theme("vswings-eon-break");
        let mut fam = Vswings::default();
        let mut c = Canvas::new(380, 60);
        // A beating loud input - a kick every 22 frames - so the sparks and the echo are in play, not a
        // constant tone that never fires either.
        let frame = |k: usize| {
            let mut d = FrameData { dt_ms: 16.7, time_s: k as f32 * 0.0167, ..FrameData::default() };
            let kick = (1.0 - (k % 22) as f32 / 6.0).max(0.0);
            for (i, v) in d.levels.iter_mut().enumerate() {
                *v = (0.6 * (1.0 - i as f32 / 96.0) * (0.6 + 0.4 * kick)).clamp(0.0, 1.0);
            }
            d.peaks = d.levels;
            d.rms_l = 0.3;
            d
        };
        for k in 0..60 {
            fam.draw(&mut c, &t, &frame(k));
        }
        let n = 300;
        let t0 = std::time::Instant::now();
        for k in 0..n {
            fam.draw(&mut c, &t, &frame(60 + k));
        }
        let steady = t0.elapsed().as_secs_f64() * 1000.0 / n as f64;
        println!("vswings: {steady:.3} ms/frame at 380x60 (steady, beating)");
        fam.flourish.force_next();
        let m = 36;
        let t1 = std::time::Instant::now();
        for k in 0..m {
            fam.draw(&mut c, &t, &frame(360 + k));
        }
        let flourish = t1.elapsed().as_secs_f64() * 1000.0 / m as f64;
        println!("vswings: {flourish:.3} ms/frame at 380x60 (flourish)");
    }

    /// Dumps for the eye test - composited over `#202020` like every other family's dump.
    ///
    /// Run: cargo test --release dump_vswings -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_vswings() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();
        let bg = Rgba::from_hex("#202020", 1.0);
        let write = |name: String, c: &Canvas| {
            let mut out = Vec::with_capacity((c.width() * c.height() * 4) as usize);
            for y in 0..c.height() {
                for x in 0..c.width() {
                    let px = c.get(x, y);
                    let a = px.a as f32 / 255.0;
                    for (ch, base) in [(px.r, bg.r), (px.g, bg.g), (px.b, bg.b)] {
                        out.push((ch as f32 * a + base as f32 * (1.0 - a)).round() as u8);
                    }
                    out.push(255);
                }
            }
            std::fs::write(dir.join(format!("{name}.rgba")), &out).unwrap();
        };
        // A shaped spectrum that beats: bass-heavy with a wobble, so the root feathers dominate.
        let frame = |level: f32, t_s: f32| {
            let mut d = FrameData { dt_ms: 16.7, time_s: t_s, ..FrameData::default() };
            for (i, v) in d.levels.iter_mut().enumerate() {
                let f = i as f32 / NUM_BANDS as f32;
                let shape = (1.0 - f).powf(1.4) * 0.7 + 0.12;
                let wob = 1.0 + 0.3 * (t_s * 2.4 + f * 6.0).sin();
                *v = ((shape * wob) * level).clamp(0.0, 1.0);
            }
            d.peaks = d.levels;
            d.rms_l = level * 0.5;
            d.rms_r = level * 0.5;
            d
        };
        for id in ["vswings-particle-arts", "vswings-eon-break", "vswings-angel-voices", "vswings-utopia", "vswings-ghost"] {
            let t = theme(id);
            // Calm and loud, at the real wide size.
            // Production builds a FRESH canvas every frame (main.rs), so the dump must clear before each
            // draw - otherwise a transparent family's glow accumulates across frames and reads as a blob
            // that never happens on screen. The family's own state (smoothing, phase) still settles.
            for (tag, level) in [("calm", 0.28f32), ("loud", 0.85)] {
                let mut fam = Vswings::default();
                let mut c = Canvas::new(380, 60);
                for k in 0..90 {
                    c.clear();
                    fam.draw(&mut c, &t, &frame(level, k as f32 * 0.0167));
                }
                write(format!("vswings-{}-{tag}", &t.id["vswings-".len()..]), &c);
            }
            // Flourish: settle, fire, then capture TWO frames - one at the envelope peak and one at
            // ~40% decay (the ring has expanded by then). Each is a fresh frame (clear each), post-hit
            // music level dropped to 0.3.
            let mut fam = Vswings::default();
            let mut c = Canvas::new(380, 60);
            for k in 0..90 {
                c.clear();
                fam.draw(&mut c, &t, &frame(0.6, k as f32 * 0.0167));
            }
            fam.flourish.force_next();
            // Peak: two frames after the fire, flare ~0.94.
            for k in 90..93 {
                c.clear();
                fam.draw(&mut c, &t, &frame(0.45, k as f32 * 0.0167));
            }
            write(format!("vswings-{}-flourish", &t.id["vswings-".len()..]), &c);
            // ~40% decay: 0.6 of the 600ms decay has elapsed by ~22 frames after the fire.
            for k in 93..113 {
                c.clear();
                fam.draw(&mut c, &t, &frame(0.3, k as f32 * 0.0167));
            }
            write(format!("vswings-{}-flourish-decay", &t.id["vswings-".len()..]), &c);
        }
        // The drop: a beating loud track (a kick every 375ms on top of the loud shape), captured
        // `after` frames past a kick - the echo still trailing, the sparks in flight. At the wide size
        // and the smallest, plus calm/loud/flourish at the smallest.
        let beat = |level: f32, k: usize| {
            let mut d = frame(level, k as f32 * 0.0167);
            let since = (k % 22) as f32; // 22 frames = ~367ms
            let kick = (1.0 - since / 6.0).max(0.0);
            for (i, v) in d.levels.iter_mut().enumerate() {
                let f = i as f32 / NUM_BANDS as f32;
                *v = (*v * (0.55 + 0.45 * kick) + kick * 0.35 * (1.0 - f)).clamp(0.0, 1.0);
            }
            d.peaks = d.levels;
            d
        };
        for id in ["vswings-particle-arts", "vswings-eon-break", "vswings-angel-voices", "vswings-utopia", "vswings-ghost"] {
            let t = theme(id);
            for (w, h, sz) in [(380, 60, ""), (128, 44, "-128")] {
                let mut fam = Vswings::default();
                let mut c = Canvas::new(w, h);
                for k in 0..(88 + 7) {
                    c.clear();
                    fam.draw(&mut c, &t, &beat(0.85, k));
                }
                write(format!("vswings-{}-drop{sz}", &t.id["vswings-".len()..]), &c);
            }
            for (tag, level) in [("calm", 0.28f32), ("loud", 0.85)] {
                let mut fam = Vswings::default();
                let mut c = Canvas::new(128, 44);
                for k in 0..90 {
                    c.clear();
                    fam.draw(&mut c, &t, &frame(level, k as f32 * 0.0167));
                }
                write(format!("vswings-{}-{tag}-128", &t.id["vswings-".len()..]), &c);
            }
            let mut fam = Vswings::default();
            let mut c = Canvas::new(128, 44);
            for k in 0..90 {
                c.clear();
                fam.draw(&mut c, &t, &frame(0.6, k as f32 * 0.0167));
            }
            fam.flourish.force_next();
            for k in 90..100 {
                c.clear();
                fam.draw(&mut c, &t, &frame(0.45, k as f32 * 0.0167));
            }
            write(format!("vswings-{}-flourish-128", &t.id["vswings-".len()..]), &c);
        }
        // One at the awkward mid size, to prove it still reads.
        let t = theme("vswings-eon-break");
        let mut fam = Vswings::default();
        let mut c = Canvas::new(190, 48);
        for k in 0..90 {
            c.clear();
            fam.draw(&mut c, &t, &frame(0.7, k as f32 * 0.0167));
        }
        write("vswings-eon-break-190x48".into(), &c);
    }
}
