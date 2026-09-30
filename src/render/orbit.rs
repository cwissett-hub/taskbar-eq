//! The orbit family: spheres circling in 3D space, pulsing to the music.
//!
//! Asked for as "a ball rotating around and pulsing to the music, in 3d space". Built on the same real
//! perspective projection as `pipes` - a camera, a divide, a near plane - and it suits this panel far
//! better than pipes does, for three reasons worth stating because they are the whole design:
//!
//! 1. **A sphere has no thin features to lose.** Every 3D idea before this one failed on legibility at
//!    48 usable rows: a pipe needs a tube wide enough to read as solid, a wireframe needs distinguishable
//!    edges, a receding grid needs distinct depth rows. A filled disc reads at three pixels.
//! 2. **Depth arrives three ways at once** - size, shading and OCCLUSION. A ball passing behind another
//!    is the one depth cue that cannot be faked in 2D, and it is free here: sort by depth, paint in
//!    order.
//! 3. **A pulse is a SIZE change.** That matters more than it sounds. `tube.rs:54-60` measured a driven
//!    element 1.46 dL* brighter than its idle neighbour as INVISIBLE against a ~2.3 dL* threshold, so
//!    "the ball glows on the beat" would have been dead on arrival at this size. "The ball swells on the
//!    beat" is position, which is the channel that works.
//!
//! # The orbit is a WIDE ellipse, and that is the letterbox talking
//!
//! A circular orbit is wrong here. With `Z_C = 8` and a circular radius of 3.5 the ring projects about
//! 50px wide - on a 380px panel, the same lost-in-a-sea-of-black problem that killed the isometric
//! pipes. So the orbit is elliptical: `RX` is derived from the panel width and `RZ` stays small.
//!
//! That is not a fudge, it is the correct reading of the constraint. `x` costs no vertical rows at all,
//! while `z` costs about 2.9 rows per step of depth separation. Spending the width on `x` and keeping
//! `z` for as much depth as the rows will pay for is exactly the right trade, and it makes the projected
//! ring a wide ellipse - which is what an orbit seen from slightly above actually looks like.
//!
//! # Why the camera is above the ring
//!
//! Same reason as `pipes`, and it is load-bearing: `row = CY - F*Y/z`, so anything at `Y = 0` sits on
//! the horizon where every depth projects to the same row. A ring in the camera's own plane would
//! collapse to a horizontal line and the orbit would read as a slider. `Y_C` puts it well below.
//!
//! # The tilt
//!
//! The orbit plane tilts slowly, which does two things: it stops the ring being a fixed shape you stop
//! looking at, and the changing ellipse is itself a depth cue - a ring seen edge-on versus face-on is
//! unambiguous 3D information. Bounded well under the aliasing limit `reel.rs` measured: motion past
//! half a feature pitch per frame appears to run BACKWARDS.
//!
//! # A real system (fidelity pass 1)
//!
//! The ring of balls was reported, with `mesh`, `pipes` and `brutal`, as "very basic, not cohesive with
//! blossom or vaporwave" - scattered fuzzy dots with no visible orbit. It is now a SYSTEM, still on the
//! same camera, the same tilted plane and the same projection:
//!
//! - **A sun** at the centre of the plane, radius `4 + 6 * bass` px: a `hot` core under a three-step
//!   `lit` corona (three concentric discs at falling alpha), pulsing with the bass.
//! - **Orbits you can see.** Each ball is a planet on its OWN ring, nested outward from the sun, and
//!   every ring's path is drawn as a 1 px `edge` line at `edge_alpha * 0.5`. The rings are circles in
//!   the plane, so the camera's own perspective turns them into ellipses.
//! - **Planets lit by the sun.** Each is shaded by the direction to the sun on screen, with a 1 px
//!   crescent on the sunward limb, so a planet reads as a sphere and says where the light is.
//! - **Comet tails.** Up to 6 fading dots behind each planet, at its own past positions; the count is
//!   the level, and since a planet's speed is `0.2 + 1.5 * level` rad/s the spacing is too.
//! - **Depth by alpha as well as size.** The far half of the plane (behind the sun) draws at 60 %, and
//!   the sun is painted between the two halves, so a planet going behind it is hidden by it.
//! - **The flourish is an alignment.** Every planet eases to one angle over 400 ms - a syzygy - a white
//!   flash line runs through them from the sun, and they scatter back to where they would have been.

use crate::render::canvas::{Canvas, Rgba};
use crate::render::{Family, FrameData};
use crate::themes::Theme;

/// Focal length in pixels. Shared with `pipes` by value rather than by import, because the two families
/// are free to diverge and a shared constant would make that look like a mistake.
const F: f32 = 32.0;

/// Near plane, and the post-divide coordinate clamp. See `pipes` for why the clip - not the clamp - is
/// what bounds the coordinate, and why the test must be `>=` rather than a negated `<`.
const Z_NEAR: f32 = 3.0;
const COORD_LIMIT: i32 = 4096;

/// The plane's centre in eye space - where the sun sits: distance in front of the camera, and how far
/// below it. `Y_C` is negative because the camera looks along +z with +y up, so the plane hangs below
/// the horizon, and that is load-bearing: a ring at `Y = 0` would project to a flat line.
const Z_C: f32 = 13.0;
const Y_C: f32 = -3.0;

/// In-plane depth half-extent of the OUTERMOST ring, in world units; the x half-extent comes from the
/// panel width. See the module docs on the letterbox: x costs no rows, depth costs ~2.9 rows a unit.
///
/// Also note the SIGN of the tilt term (`y = Y_C + ring*st`, not minus): with the minus the near half
/// of the ring rose to the TOP of the panel, which is a plane tilted away from the viewer and reads as
/// upside down. Plus puts the near half low, which is looking DOWN on an orbit.
const RZ: f32 = 6.0;
/// The most the outer ring's depth is stretched past `RZ` on a tall panel.
const DEPTH_GAIN: f32 = 1.2;

/// Most planets a colourway may ask for. The actual count is `Theme::orbit.balls`.
const MAX_BALLS: usize = 16;

/// Milliseconds a planet takes to fade in or out when the reactive count changes.
///
/// Fade, not appear. A planet popping into existence is the same class of discontinuity as the
/// flourish snap that was reported as jarring: ramp a presence value and multiply the radius by it.
const PRESENCE_MS: f32 = 260.0;

/// How much of the system the quietest passages keep, as a fraction, when `reactive` is set. One
/// planet always orbits regardless, so the display never goes empty and never has to "start".
const REACTIVE_FLOOR: f32 = 0.0;

/// The base orbit rate, in turns per second: 0.2 rad/s, the spec's floor, shared by every planet.
/// Each planet adds `LEVEL_RAD_S * level` of its own on top.
const ORBIT_HZ: f32 = 0.2 / std::f32::consts::TAU;
const LEVEL_RAD_S: f32 = 1.5;

/// The plane's slow tilt: a changing ellipse is itself a depth cue.
const TILT_HZ: f32 = 0.031;
const TILT_BASE: f32 = 0.75;
const TILT_AMP: f32 = 0.10;

/// The level window, `vapor`'s MEASURED p10-p90 of real music - not a 0..1 mapping, which renders dead,
/// and not normalised against the frame's loudest band, which is provably inert at p50 0.819.
const LEVEL_FLOOR: f32 = 0.119;
const LEVEL_SPAN: f32 = 0.456;
const LEVEL_GAMMA: f32 = 0.6;

/// How fast a planet swells and how slowly it settles, per millisecond. Asymmetric on purpose: a kick
/// must arrive as a kick. Symmetric ballistics make a pulse read as a wobble.
const SWELL_PER_MS: f32 = 0.055;
const SETTLE_PER_MS: f32 = 0.011;

/// Planet radius in px at the sun's depth, at rest and added at full level (2..4 px); the perspective
/// makes the near ones bigger and the far ones smaller than that.
const PLANET_R_REST: f32 = 2.0;
const PLANET_R_PULSE: f32 = 2.0;

/// The innermost ring's half-width in px at the sun's depth, clear of the fattest sun and its corona,
/// and its in-plane depth. Rings step evenly out from there to the panel's width and `RZ`.
const INNER_PX: f32 = 26.0;
const B_INNER: f32 = 1.4;

/// Path alpha as a fraction of `edge_alpha` (the spec's 0.5), and the far half's alpha.
const RING_ALPHA: f32 = 0.5;
const BEHIND: f32 = 0.6;

/// The sun: `4 + 6 * bass` px, and the corona as (extra radius px, alpha), outermost first.
const SUN_R_REST: f32 = 4.0;
const SUN_R_BASS: f32 = 6.0;
const CORONA: [(f32, f32); 3] = [(8.0, 0.07), (5.0, 0.12), (2.5, 0.22)];

/// Comet tails: up to 6 dots, one every `TAIL_LAG` frames of history.
const TAIL_DOTS: usize = 6;
const TAIL_LAG: usize = 3;
const HIST: usize = TAIL_DOTS * TAIL_LAG + 1;
const TAIL_ALPHA: f32 = 0.75;

/// The alignment: ease in, hold, scatter back, in ms; and how far round from the side the planets line
/// up, so the line runs slightly toward the viewer, in front of the sun, rather than through it.
const ALIGN_IN_MS: f32 = 400.0;
const ALIGN_HOLD_MS: f32 = 350.0;
const ALIGN_OUT_MS: f32 = 700.0;
const ALIGN_TILT: f32 = 0.3;

/// The golden angle, in radians: the start offset between neighbouring planets.
const GOLDEN: f32 = 2.399_963;

/// A faint, twinkling field behind the system, so the sky is a sky and not a hole.
const STARS: u32 = 28;

#[derive(Default)]
pub struct Orbit {
    /// The planet count the per-planet state is sized for.
    n: usize,
    /// Smoothed pulse per planet, 0..1.
    pulse: [f32; MAX_BALLS],
    /// How present each planet is, 0..1 - see `PRESENCE_MS`.
    presence: [f32; MAX_BALLS],
    /// Each planet's own level-driven lead, in radians, on top of the shared `phase`.
    extra: [f32; MAX_BALLS],
    /// The angle each planet was DRAWN at this frame, after the alignment. Radians.
    shown: [f32; MAX_BALLS],
    /// The comet-tail history: past `shown` angles, a ring per planet. Fixed size, so no allocation.
    hist: [[f32; HIST]; MAX_BALLS],
    hist_head: usize,
    hist_n: usize,
    /// Draw order scratch, far to near.
    order: [usize; MAX_BALLS],
    depth: [f32; MAX_BALLS],
    /// The sun's smoothed bass, 0..1.
    bass: f32,
    /// Shared orbit phase and tilt phase, in turns.
    phase: f32,
    tilt_t: f32,
    /// Milliseconds into the alignment flourish, and the angle it lines up on.
    align: Option<f32>,
    align_to: f32,
    flourish: crate::dsp::flourish::Trigger,
}

fn resp(level: f32, sensitivity: f32) -> f32 {
    if !level.is_finite() {
        return 0.0;
    }
    let x = ((level - LEVEL_FLOOR) / LEVEL_SPAN).clamp(0.0, 1.0);
    (x.powf(LEVEL_GAMMA) * sensitivity.max(0.0)).clamp(0.0, 1.0)
}

/// Whether an eye-space depth is safely in front of the near plane.
///
/// `>=` and not `!(z < Z_NEAR)`: for a NaN `z` this is FALSE and the point is rejected, where the
/// negated form is TRUE and leaks a NaN into the divide. Same value, opposite safety - see `pipes`.
fn in_front(z: f32) -> bool {
    z >= Z_NEAR
}

/// Projects an eye-space point to (col, row, pixels-per-world-unit). `None` when it must be skipped.
fn project(cx: f32, cy: f32, x: f32, y: f32, z: f32) -> Option<(i32, i32, f32)> {
    if !in_front(z) || !x.is_finite() || !y.is_finite() {
        return None;
    }
    let inv = F / z;
    let col = cx + x * inv;
    let row = cy - y * inv;
    if !col.is_finite() || !row.is_finite() {
        return None;
    }
    Some((
        (col.round() as i32).clamp(-COORD_LIMIT, COORD_LIMIT),
        (row.round() as i32).clamp(-COORD_LIMIT, COORD_LIMIT),
        inv,
    ))
}

/// A filled disc, as horizontal spans, measured to the pixel CENTRES against `r + 0.5`.
///
/// Not `sqrt(r^2 - dy^2) + 0.5`, the first version: that leaves a one-pixel nub at the top, bottom and
/// both sides, and three stacked corona discs turned the nubs into a visible cross through the sun.
fn disc(c: &mut Canvas, col: i32, row: i32, r: i32, colour: Rgba) {
    if r <= 0 {
        c.fill_rect(col, row, 1, 1, colour);
        return;
    }
    let rr = (r as f32 + 0.5) * (r as f32 + 0.5);
    for dy in -r..=r {
        let dx = (rr - (dy * dy) as f32).max(0.0).sqrt().floor() as i32;
        c.fill_rect(col - dx, row + dy, dx * 2 + 1, 1, colour);
    }
}

/// A DOTTED line that does not plot its first pixel, so a polyline of them blends every pixel at most
/// once - at `edge_alpha * 0.5` a doubled vertex is a visible bead. `step` carries the dot phase across
/// segments. Dotted because twelve nested rings bunch to 1-2 px apart at the front and back of the
/// plane, and solid lines there fuse into stripes; dots stay a path.
fn seg(c: &mut Canvas, x0: i32, y0: i32, x1: i32, y1: i32, col: Rgba, step: &mut u32) {
    let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
    let (sx, sy) = (if x1 >= x0 { 1 } else { -1 }, if y1 >= y0 { 1 } else { -1 });
    let (mut x, mut y, mut err) = (x0, y0, dx + dy);
    // Bounded: a projected coordinate is clamped to COORD_LIMIT, so this is at most ~16k steps.
    while x != x1 || y != y1 {
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
        *step += 1;
        if (*step).is_multiple_of(2) {
            c.fill_rect(x, y, 1, 1, col);
        }
    }
}

fn with_alpha(c: Rgba, a: f32) -> Rgba {
    Rgba::new(c.r, c.g, c.b, (a.clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// The shortest signed angle from `a` to `b`, in (-PI, PI].
fn toward(a: f32, b: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    let d = (b - a).rem_euclid(tau);
    if d > std::f32::consts::PI {
        d - tau
    } else {
        d
    }
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// The system's geometry for one frame: the principal point and every ring's two half-extents.
#[derive(Clone, Copy)]
struct Geo {
    cx: f32,
    cy: f32,
    st: f32,
    ct: f32,
    a: [f32; MAX_BALLS],
    b: [f32; MAX_BALLS],
}

fn geo(w: i32, h: i32, n: usize, tilt: f32) -> Geo {
    let (st, ct) = (tilt.sin(), tilt.cos());
    // The plane's depth is spent on the ROWS there are: `RZ` fills a 44 px panel (28 of its 40 rows),
    // and the 60 px panel takes 1.2x it - more taper, more ellipse - rather than a band of empty sky.
    // Short panels get shallower rings rather than rings that leave the panel.
    let vs = ((h - 4) as f32 / 40.0).clamp(0.4, DEPTH_GAIN);
    // The inner ring clears the sun, which shrinks with the panel (see `draw`).
    let vi = ((h - 4) as f32 / 56.0).clamp(0.4, 1.0).min((w as f32 / 380.0).sqrt().clamp(0.6, 1.0));
    // x from the WIDTH: x costs no vertical rows, so this is where the panel's aspect is spent.
    let a_max = ((w as f32 * 0.5 - 12.0) * Z_C / F).max(1.0);
    let a_in = (INNER_PX * vi * Z_C / F).min(a_max * 0.6);
    let mut g = Geo { cx: w as f32 * 0.5, cy: 0.0, st, ct, a: [0.0; MAX_BALLS], b: [0.0; MAX_BALLS] };
    let n = n.clamp(1, MAX_BALLS);
    for k in 0..n {
        let f = if n == 1 { 1.0 } else { k as f32 / (n - 1) as f32 };
        g.a[k] = a_in + (a_max - a_in) * f;
        g.b[k] = (B_INNER + (RZ - B_INNER) * f) * vs;
    }
    // Centre the OUTER ring's projected rows in the panel. Not the sun: the near half of a tilted ring
    // projects much larger than the far half, so the sun sits above the ellipse's middle, as it should.
    let bo = g.b[n - 1];
    let far = -(Y_C + bo * st) * F / (Z_C + bo * ct);
    let near = -(Y_C - bo * st) * F / (Z_C - bo * ct);
    g.cy = h as f32 * 0.5 - (far + near) * 0.5;
    g
}

/// Where planet `k`'s ring puts angle `ang`, in eye space. Positive `sin` is the far half.
fn point(g: &Geo, k: usize, ang: f32) -> (f32, f32, f32) {
    let ring = g.b[k] * ang.sin();
    (g.a[k] * ang.cos(), Y_C + ring * g.st, Z_C + ring * g.ct)
}

fn ring_px(g: &Geo, k: usize, ang: f32) -> Option<(i32, i32, f32)> {
    let (x, y, z) = point(g, k, ang);
    project(g.cx, g.cy, x, y, z)
}

/// Half a ring's path: the far half (`back`) or the near one.
fn ring_half(c: &mut Canvas, g: &Geo, k: usize, back: bool, col: Rgba) {
    const SEGS: usize = 40;
    let pi = std::f32::consts::PI;
    let a0 = if back { 0.0 } else { pi };
    let mut prev = ring_px(g, k, a0);
    let mut step = k as u32;
    for j in 1..=SEGS {
        let cur = ring_px(g, k, a0 + pi * j as f32 / SEGS as f32);
        if let (Some(p), Some(q)) = (prev, cur) {
            seg(c, p.0, p.1, q.0, q.1, col, &mut step);
        }
        prev = cur;
    }
}

/// A planet: keyline, then a sphere shaded by the direction to the sun, with a crescent on that limb.
/// `tones` runs night side, two terminator steps, day side, crescent.
#[allow(clippy::too_many_arguments)]
fn planet(c: &mut Canvas, col: i32, row: i32, r: i32, sun: (i32, i32), tones: &[Rgba; 5], key: Rgba, alpha: f32) {
    disc(c, col, row, r + 1, with_alpha(key, alpha));
    let (vx, vy) = ((sun.0 - col) as f32, (sun.1 - row) as f32);
    let len = (vx * vx + vy * vy).sqrt();
    let (lx, ly) = if len > 0.5 { (vx / len, vy / len) } else { (0.0, -1.0) };
    let rf = r.max(1) as f32;
    let rr = (r as f32 + 0.5) * (r as f32 + 0.5);
    for dy in -r..=r {
        for dx in -r..=r {
            let d2 = (dx * dx + dy * dy) as f32;
            if d2 > rr {
                continue;
            }
            let s = (dx as f32 * lx + dy as f32 * ly) / rf;
            let rim = d2.sqrt() > rf - 1.0;
            let tone = if rim && s > 0.3 {
                4
            } else if s > 0.45 {
                3
            } else if s > 0.05 {
                2
            } else if s > -0.35 {
                1
            } else {
                0
            };
            c.fill_rect(col + dx, row + dy, 1, 1, with_alpha(tones[tone], alpha));
        }
    }
}

impl Orbit {
    /// Draws planet `i` and its tail at `alpha`.
    #[allow(clippy::too_many_arguments)]
    fn draw_planet(&self, c: &mut Canvas, t: &Theme, d: &FrameData, g: &Geo, i: usize, sun: (i32, i32), key: Rgba, scale: f32, rcap: f32) {
        let n = self.n;
        let present = self.presence[i].clamp(0.0, 1.0);
        if present < 0.02 {
            return;
        }
        let Some((col, row, inv)) = ring_px(g, i, self.shown[i]) else { return };
        let behind = self.shown[i].sin() > 0.0;
        let alpha = if behind { BEHIND } else { 1.0 };
        // COLOUR PER PLANET through the shared rainbow resolver: `t.lit` unchanged on a fixed colourway,
        // the hue of its own frequency slice on a rainbow one - a frequency legend, since ring order IS
        // the slice order.
        let x01 = i as f32 / n.max(1) as f32;
        let lit = crate::render::tint(t, x01, d.time_s, false, &t.lit, 1.0);
        let hot = crate::render::tint(t, x01, d.time_s, true, &t.hot, 1.0);
        let pulse = self.pulse[i];
        let px = (PLANET_R_REST + PLANET_R_PULSE * pulse) * scale * present * inv / (F / Z_C);
        let r = px.clamp(1.0, rcap).round() as i32;

        // The tail, oldest dot first so the newer ones sit on top, all under the planet.
        let dots = ((TAIL_DOTS as f32 * pulse).ceil() as usize).min(TAIL_DOTS);
        let tail = Rgba::lerp_linear(lit, hot, 0.25);
        for j in (1..=dots).rev() {
            let lag = j * TAIL_LAG;
            if lag >= self.hist_n {
                continue;
            }
            let ang = self.hist[i][(self.hist_head + HIST - lag) % HIST];
            let Some((tx, ty, _)) = ring_px(g, i, ang) else { continue };
            let fade = 1.0 - (j - 1) as f32 / dots as f32;
            let da = if ang.sin() > 0.0 { BEHIND } else { 1.0 };
            let rd = (r as f32 * 0.8 * (1.0 - j as f32 / (TAIL_DOTS + 1) as f32)).round() as i32;
            disc(c, tx, ty, rd, with_alpha(tail, TAIL_ALPHA * fade * present * da));
        }

        let night = Rgba::lerp_linear(lit, key, 0.72);
        let tones = [
            night,
            Rgba::lerp_linear(night, lit, 0.4),
            Rgba::lerp_linear(night, lit, 0.75),
            lit,
            Rgba::lerp_linear(lit, hot, 0.8),
        ];
        planet(c, col, row, r, sun, &tones, key, alpha);
    }
}

impl Family for Orbit {
    fn id(&self) -> &'static str {
        "orbit"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());
        c.clear();

        let dt = if d.dt_ms.is_finite() { d.dt_ms.clamp(0.0, 200.0) } else { 16.7 };
        let fired = self.flourish.update(&d.levels, dt, t.flourish);

        // Opaque panel first.
        let panel = Rgba::from_hex(&t.panel, t.panel_alpha);
        c.rounded_rect(1, 2, w - 2, h - 4, 3, panel);
        if w < 60 || h < 32 {
            return; // shed rather than smudge
        }

        let n = t.orbit.balls.clamp(1, MAX_BALLS as i32) as usize;
        if self.n != n {
            self.n = n;
            self.pulse = [0.0; MAX_BALLS];
            self.presence = [0.0; MAX_BALLS];
            self.extra = [0.0; MAX_BALLS];
            self.hist_n = 0;
        }

        // How many are ACTIVE. Fixed colourways use all of them; a reactive one lets the music decide,
        // with one planet always kept so the system never empties.
        let bands = d.levels.len().max(1);
        let overall = d.levels.iter().map(|v| resp(*v, t.sensitivity)).sum::<f32>() / bands as f32;
        let active = if t.orbit.reactive {
            let f = REACTIVE_FLOOR + (1.0 - REACTIVE_FLOOR) * overall.clamp(0.0, 1.0);
            (1.0 + (n as f32 - 1.0) * f).round().clamp(1.0, n as f32) as usize
        } else {
            n
        };

        // ---- advance ----
        let tau = std::f32::consts::TAU;
        self.phase = (self.phase + dt / 1000.0 * ORBIT_HZ).fract();
        self.tilt_t = (self.tilt_t + dt / 1000.0 * TILT_HZ).fract();
        if !self.phase.is_finite() {
            self.phase = 0.0;
        }
        if !self.tilt_t.is_finite() {
            self.tilt_t = 0.0;
        }
        let tilt = TILT_BASE + TILT_AMP * (tau * self.tilt_t).sin();
        let g = geo(w, h, n, tilt);

        let ease = |cur: f32, target: f32| {
            let k = if target > cur { SWELL_PER_MS } else { SETTLE_PER_MS };
            let next = cur + (target - cur) * (k * dt).min(1.0);
            if next.is_finite() { next.clamp(0.0, 1.0) } else { 0.0 }
        };
        let low = (bands / 8).max(1).min(d.levels.len());
        let bass = d.levels[..low].iter().copied().fold(0.0f32, f32::max);
        self.bass = ease(self.bass, resp(bass, t.sensitivity));

        // Each planet reads its OWN slice of the spectrum, low to high outward from the sun.
        let nb = d.levels.len();
        for i in 0..n {
            let lo = (i * nb) / n;
            let hi = (((i + 1) * nb) / n).clamp(lo + 1, nb.max(1));
            let band = if nb == 0 { 0.0 } else { d.levels[lo..hi].iter().copied().fold(0.0f32, f32::max) };
            self.pulse[i] = ease(self.pulse[i], resp(band, t.sensitivity));
            let want = if i < active { 1.0 } else { 0.0 };
            self.presence[i] += (want - self.presence[i]) * (dt / PRESENCE_MS).clamp(0.0, 1.0);
            if !self.presence[i].is_finite() {
                self.presence[i] = 0.0;
            }
            self.extra[i] = (self.extra[i] + LEVEL_RAD_S * self.pulse[i] * dt / 1000.0).rem_euclid(tau);
            if !self.extra[i].is_finite() {
                self.extra[i] = 0.0;
            }
        }
        // Each planet starts a golden angle round from the one inside it. Not `i / n` of a turn: with
        // the inner (bass) planets usually the faster, evenly stepped starts wind into a spiral arm that
        // periodically straightens into a line on its own - an alignment nobody asked for.
        let natural = |s: &Self, i: usize| tau * s.phase + GOLDEN * i as f32 + s.extra[i];

        // ---- the alignment ----
        if fired {
            // Line up on whichever side the planets already lean toward, so the ease is short.
            let side: f32 = (0..n).map(|i| natural(self, i).cos()).sum();
            self.align_to = if side >= 0.0 { -ALIGN_TILT } else { std::f32::consts::PI + ALIGN_TILT };
            self.align = Some(0.0);
        }
        let (pull, flash) = match self.align {
            Some(ms) => {
                let ms = ms + dt;
                let total = ALIGN_IN_MS + ALIGN_HOLD_MS + ALIGN_OUT_MS;
                self.align = if ms < total { Some(ms) } else { None };
                let pull = if ms < ALIGN_IN_MS {
                    smooth(ms / ALIGN_IN_MS)
                } else if ms < ALIGN_IN_MS + ALIGN_HOLD_MS {
                    1.0
                } else {
                    1.0 - smooth((ms - ALIGN_IN_MS - ALIGN_HOLD_MS) / ALIGN_OUT_MS)
                };
                let flash = if ms < ALIGN_IN_MS {
                    smooth(ms / ALIGN_IN_MS).powi(2)
                } else {
                    (1.0 - (ms - ALIGN_IN_MS) / ALIGN_HOLD_MS).clamp(0.0, 1.0)
                };
                (pull, flash)
            }
            None => (0.0, 0.0),
        };
        for i in 0..n {
            let a = natural(self, i);
            self.shown[i] = (a + pull * toward(a, self.align_to)).rem_euclid(tau);
        }
        self.hist_head = (self.hist_head + 1) % HIST;
        for i in 0..n {
            self.hist[i][self.hist_head] = self.shown[i];
        }
        self.hist_n = (self.hist_n + 1).min(HIST);

        let key = Rgba::from_hex(&t.panel, 1.0);
        let edge = Rgba::from_hex(&t.edge, 1.0);
        let hot = Rgba::from_hex(&t.hot, 1.0);
        // Bodies shrink with the panel: a 128 px system with full-size planets is a traffic jam.
        let vs = ((h - 4) as f32 / 56.0).clamp(0.5, 1.0).min((w as f32 / 380.0).sqrt().clamp(0.6, 1.0));

        // ---- stars ----
        let star = Rgba::lerp_linear(edge, hot, 0.5);
        for s in 0..STARS {
            let hsh = s.wrapping_mul(2_654_435_761).rotate_left(13) ^ 0x9e37_79b9;
            let x = 3 + (hsh % (w as u32 - 6)) as i32;
            let y = 4 + ((hsh >> 12) % (h as u32 - 9)) as i32;
            let tw = 0.5 + 0.5 * (d.time_s * 1.3 + s as f32 * 1.7).sin();
            c.fill_rect(x, y, 1, 1, with_alpha(star, t.edge_alpha * (0.5 + 1.3 * tw)));
        }

        // ---- far half: paths, then the planets behind the sun ----
        let ring = with_alpha(edge, t.edge_alpha * RING_ALPHA);
        let ring_far = with_alpha(edge, t.edge_alpha * RING_ALPHA * BEHIND);
        for k in 0..n {
            ring_half(c, &g, k, true, ring_far);
        }

        // FAR TO NEAR inside each half: occlusion is the depth cue that cannot be faked.
        for i in 0..n {
            self.order[i] = i;
            self.depth[i] = point(&g, i, self.shown[i]).2;
        }
        let depth = self.depth;
        self.order[..n].sort_unstable_by(|p, q| depth[*q].partial_cmp(&depth[*p]).unwrap_or(std::cmp::Ordering::Equal));

        let sun = project(g.cx, g.cy, 0.0, Y_C, Z_C).map(|(x, y, _)| (x, y)).unwrap_or((w / 2, h / 2));
        let scale = t.orbit.scale.clamp(0.3, 3.0) * vs;
        let rcap = (h - 4) as f32 / 8.0;
        let order = self.order;
        for &i in order[..n].iter().filter(|&&i| self.shown[i].sin() > 0.0) {
            self.draw_planet(c, t, d, &g, i, sun, key, scale, rcap);
        }

        // The near half of every path is nearer than the far planets, but the sun sits on top of the
        // plane: its disc is drawn over all of them, so no path cuts across it.
        for k in 0..n {
            ring_half(c, &g, k, false, ring);
        }

        // ---- the sun, between the halves ----
        let sun_lit = crate::render::tint(t, 0.0, d.time_s, false, &t.lit, 1.0);
        let sun_hot = crate::render::tint(t, 0.0, d.time_s, true, &t.hot, 1.0);
        let r_sun = (SUN_R_REST + SUN_R_BASS * self.bass) * vs;
        let glow = 0.8 + 0.5 * self.bass + 1.2 * flash;
        for (extra, a) in CORONA {
            disc(c, sun.0, sun.1, (r_sun + extra * vs).round() as i32, with_alpha(sun_lit, a * glow));
        }
        let rs = r_sun.round() as i32;
        disc(c, sun.0, sun.1, rs, Rgba::lerp_linear(sun_hot, sun_lit, 0.45));
        disc(c, sun.0, sun.1, rs - 1, sun_hot);

        // ---- near half: the flash, then the planets in front ----
        if flash > 0.01 {
            if let Some((ox, oy, _)) = ring_px(&g, n - 1, self.align_to) {
                let (ex, ey) = (sun.0 + ((ox - sun.0) as f32 * 1.1) as i32, sun.1 + ((oy - sun.1) as f32 * 1.1) as i32);
                let white = Rgba::new(255, 255, 255, 255);
                c.line(sun.0, sun.1, ex, ey, with_alpha(white, 0.9 * flash));
                c.line(sun.0, sun.1 - 1, ex, ey - 1, with_alpha(white, 0.3 * flash));
                c.line(sun.0, sun.1 + 1, ex, ey + 1, with_alpha(white, 0.3 * flash));
            }
        }
        for &i in order[..n].iter().filter(|&&i| self.shown[i].sin() <= 0.0) {
            self.draw_planet(c, t, d, &g, i, sun, key, scale, rcap);
        }

        // The bloom composites each pixel OVER its own halo, so on an opaque panel it changes no
        // interior pixel and the clip below clears the rest - measured in `pipes`. Translucent only.
        if t.panel_alpha < 1.0 {
            c.bloom(t.bloom as i32, t.glow_strength);
        }
        // Clip last: the near planets at the bottom of the ellipse sit a pixel from the corners.
        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::themes::builtin;

    fn frame(gain: f32, t_s: f32) -> FrameData {
        let mut d = FrameData { dt_ms: 16.7, time_s: t_s, ..FrameData::default() };
        let hit = ((t_s / 0.5).fract() < 0.08) as i32 as f32;
        for (i, v) in d.levels.iter_mut().enumerate() {
            let f = i as f32 / crate::dsp::bands::NUM_BANDS as f32;
            let shape = (1.0 - f).powf(1.4) * 0.58 + 0.15;
            let wob = 1.0 + 0.32 * ((t_s * 2.4 + f * 8.0).sin());
            *v = ((shape * wob + hit * 0.45) * gain).clamp(0.0, 1.0);
        }
        d.peaks = d.levels;
        d.rms_l = 0.30 * gain;
        d.rms_r = 0.27 * gain;
        d
    }

    /// Luminance above which a pixel is a planet, a tail or the sun rather than a path or a star.
    const BODY_LUM: f32 = 90.0;

    fn lum(px: Rgba) -> f32 {
        let a = px.a as f32 / 255.0;
        (0.2126 * px.r as f32 + 0.7152 * px.g as f32 + 0.0722 * px.b as f32) * a
    }

    /// The pulse must be a SIZE change, not a brightness change.
    ///
    /// This is the house rule the family is built on: `tube.rs` measured brightness-as-level as
    /// invisible at this size. Mutation: drive the colour from `pulse` and leave the radius at rest -
    /// that renders plausibly and fails here.
    #[test]
    fn a_louder_ball_is_drawn_bigger_not_just_brighter() {
        let t = builtin::orbit_chrome();
        // Measured on the PLANETS themselves: the median height of the body pixels in the column
        // through each planet's centre. The first version counted lit pixels over the whole panel,
        // which the fidelity pass made vacuous - the sun's bass swell and the comet tails grow with
        // the level too, and zeroing the planets' own swell still passed it.
        let count = |gain: f32| -> usize {
            let mut fam = Orbit::default();
            let mut c = Canvas::new(380, 60);
            for k in 0..120 {
                fam.draw(&mut c, &t, &frame(gain, k as f32 * 0.0167));
            }
            let g = geo_of(&fam, 380, 60);
            let mut runs: Vec<usize> = (0..fam.n)
                .filter_map(|i| ring_px(&g, i, fam.shown[i]))
                .map(|(x, y, _)| {
                    // Above the orbit paths and the stars (both ~60 on chrome): body pixels only.
                    let up = (1..20).take_while(|d| lum(c.get(x, y - d)) > BODY_LUM).count();
                    let down = (1..20).take_while(|d| lum(c.get(x, y + d)) > BODY_LUM).count();
                    up + down + 1
                })
                .collect();
            runs.sort_unstable();
            runs[runs.len() / 2]
        };
        let quiet = count(0.18);
        let loud = count(0.95);
        assert!(
            loud > quiet * 3 / 2,
            "a loud planet is {loud} px tall against {quiet} px quiet - under 1.5x the planets are \
             not actually swelling, they are only getting brighter"
        );
    }

    /// Occlusion: a ball behind another must be hidden by it, which is the depth cue that cannot be
    /// faked in 2D. Measured as the drawn area being LESS than the sum of the discs, and then directly,
    /// on two overlapping planets of different hues.
    ///
    /// Mutation: sort near-to-far instead of far-to-near. The area bound alone SURVIVED that (a union
    /// is order-blind) - the direct check is what fails it.
    #[test]
    fn the_ring_is_painted_far_to_near_so_near_balls_occlude() {
        let t = builtin::orbit_chrome();
        let mut fam = Orbit::default();
        let mut c = Canvas::new(380, 60);
        for k in 0..200 {
            fam.draw(&mut c, &t, &frame(0.85, k as f32 * 0.0167));
        }
        // With twelve balls at up to ~6px radius the discs sum to well over 1000px; overlap must bring
        // the drawn total below that. A pure sum would mean nothing is hidden.
        let mut drawn = 0;
        for y in 0..60 {
            for x in 0..380 {
                if lum(c.get(x, y)) > BODY_LUM {
                    drawn += 1;
                }
            }
        }
        assert!(drawn > 120, "only {drawn} pixels drawn - the ring is not rendering");
        assert!(
            drawn < 3000,
            "{drawn} pixels drawn, which is more than twelve overlapping balls can cover - the depth \
             sort is not producing occlusion"
        );

        // The area bound above cannot see the ORDER - a union covers the same pixels whichever disc is
        // painted last, and reversing the sort passed it. So, directly: park two planets on
        // neighbouring rings at the front of the plane, 1-2 px apart and overlapping almost entirely,
        // on a rainbow colourway where each has its own hue. The farther one's centre must show the
        // NEARER one's colour.
        let mut t = builtin::orbit_rainbow();
        t.flourish = 0.0;
        let mut fam = Orbit::default();
        let mut c = Canvas::new(380, 60);
        for k in 0..60 {
            fam.draw(&mut c, &t, &frame(0.6, k as f32 * 0.0167));
        }
        let (far, near) = (8, 9);
        let front = -std::f32::consts::FRAC_PI_2;
        for i in [far, near] {
            fam.extra[i] = front - std::f32::consts::TAU * fam.phase - GOLDEN * i as f32;
        }
        let mut still = frame(0.6, 1.0);
        still.dt_ms = 0.0;
        fam.draw(&mut c, &t, &still);
        let g = geo_of(&fam, 380, 60);
        assert!(point(&g, near, fam.shown[near]).2 < point(&g, far, fam.shown[far]).2, "the test's 'near' is not nearer");
        let (x, y, _) = ring_px(&g, far, fam.shown[far]).unwrap();
        let px = c.get(x, y);
        let hue = |i: usize| crate::render::tint(&t, i as f32 / fam.n as f32, still.time_s, false, &t.lit, 1.0);
        let cos = |a: Rgba, b: Rgba| {
            let (a, b) = ([a.r as f32, a.g as f32, a.b as f32], [b.r as f32, b.g as f32, b.b as f32]);
            let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
            dot / ((a.iter().map(|v| v * v).sum::<f32>() * b.iter().map(|v| v * v).sum::<f32>()).sqrt() + 1e-3)
        };
        assert!(
            cos(px, hue(near)) > cos(px, hue(far)),
            "the farther planet's centre shows {px:?}, nearer its own hue {:?} than the covering planet's {:?} - \
             a far planet was painted over a near one",
            hue(far),
            hue(near)
        );
    }

    /// A vertex at or behind the eye must be rejected, not projected - the 294.6ms hang guard.
    #[test]
    fn a_point_at_or_behind_the_eye_is_rejected() {
        for z in [0.0f32, -1.0, 1.0, f32::NAN, f32::NEG_INFINITY] {
            assert!(
                project(190.0, 30.0, 1.0, -7.5, z).is_none(),
                "z={z} was projected instead of clipped"
            );
        }
        assert!(project(190.0, 30.0, f32::NAN, -7.5, 8.0).is_none(), "a NaN x was projected");
        assert!(project(190.0, 30.0, 0.0, -7.5, Z_C).is_some(), "the ring centre must project");
    }

    /// The orbit must actually go round: a ball has to change depth over a lap, or it is a 2D ring.
    ///
    /// Depth is read from the renderer's OWN geometry (`geo_of` + `point`), not re-derived by hand:
    /// a hand-rolled copy of the z formula (the previous version of this test) can drift from what
    /// `geo`/`point` actually compute - it used `RZ` and `ang.sin()` directly, which is the outer
    /// ring's NOMINAL half-extent before `geo`'s per-panel-height `vs` scaling and skips `shown[i]`'s
    /// own alignment pull entirely - so a mutation in either would go uncaught by a copy that never
    /// calls the mutated code.
    ///
    /// Mutation: set `RZ` to 0 - every ring collapses to a flat ellipse and the measured spread
    /// drops to exactly 0. Verified failing. (Freezing `phase` and `tilt_t` alone does NOT zero the
    /// spread here - `shown[i]` also carries the alignment flourish's own pull toward `align_to`,
    /// which keeps moving independently of the orbit clock, so that pair is not a usable mutation
    /// for this particular assertion.)
    /// Slow (one lap at the 0.2 rad/s floor is ~1900 frames); gated out of the default suite. A ball changes depth over one lap. Run: `cargo test --release slow_ -- --ignored`.
    #[test]
    #[ignore]
    fn slow_a_ball_changes_depth_over_one_lap() {
        let mut fam = Orbit::default();
        let t = builtin::orbit_chrome();
        let (w, h) = (380, 60);
        let mut c = Canvas::new(w, h);
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        // One lap is 1/ORBIT_HZ seconds; sample across a little more than that.
        let frames = (1.0 / ORBIT_HZ / 0.0167) as usize + 20;
        let mut outer_b = 0.0f32;
        for k in 0..frames {
            fam.draw(&mut c, &t, &frame(0.5, k as f32 * 0.0167));
            let g = geo_of(&fam, w, h);
            let ball = fam.n - 1; // the outermost ring - the widest depth swing
            outer_b = g.b[ball];
            let z = point(&g, ball, fam.shown[ball]).2;
            lo = lo.min(z);
            hi = hi.max(z);
        }
        assert!(
            hi - lo > outer_b,
            "depth only varied by {:.2} world units over a lap (ring half-extent {outer_b:.2}) - \
             the ring is flat, not an orbit",
            hi - lo
        );
    }

    fn flat(level: f32) -> FrameData {
        let mut d = FrameData { dt_ms: 16.7, ..FrameData::default() };
        for v in d.levels.iter_mut() {
            *v = level;
        }
        d.peaks = d.levels;
        d
    }

    fn geo_of(fam: &Orbit, w: i32, h: i32) -> Geo {
        let tilt = TILT_BASE + TILT_AMP * (std::f32::consts::TAU * fam.tilt_t).sin();
        geo(w, h, fam.n, tilt)
    }

    fn dist(a: Rgba, b: Rgba) -> i32 {
        (a.r as i32 - b.r as i32).abs() + (a.g as i32 - b.g as i32).abs() + (a.b as i32 - b.b as i32).abs()
    }

    #[test]
    fn planets_ride_visible_ellipses_around_a_sun() {
        // A rest frame: silence, so every planet is at its smallest and the sun at its 4 px floor.
        let t = builtin::orbit_chrome();
        let (w, h) = (380, 60);
        let mut fam = Orbit::default();
        let mut c = Canvas::new(w, h);
        for _ in 0..30 {
            fam.draw(&mut c, &t, &flat(0.0));
        }
        let g = geo_of(&fam, w, h);
        let (sx, sy, _) = project(g.cx, g.cy, 0.0, Y_C, Z_C).unwrap();
        let hot = Rgba::from_hex(&t.hot, 1.0);
        for (dx, dy) in [(0, 0), (-2, 0), (2, 0), (0, -2), (0, 2)] {
            let px = c.get(sx + dx, sy + dy);
            assert!(dist(px, hot) < 16, "the sun at ({},{}) is {px:?}, not a hot disc {hot:?}", sx + dx, sy + dy);
        }

        // The outermost ring's path, sampled round its whole length, away from any planet.
        let panel = Rgba::from_hex(&t.panel, 1.0);
        let k = fam.n - 1;
        let planets: Vec<(i32, i32)> =
            (0..fam.n).map(|i| ring_px(&g, i, fam.shown[i]).map(|p| (p.0, p.1)).unwrap()).collect();
        let (mut on, mut total) = (0, 0);
        for j in 0..96 {
            let a = std::f32::consts::TAU * j as f32 / 96.0;
            let (x, y, _) = ring_px(&g, k, a).unwrap();
            if planets.iter().any(|p| (p.0 - x).abs() < 7 && (p.1 - y).abs() < 7) {
                continue;
            }
            total += 1;
            // The paths are DOTTED, so a sample lands on a dot or beside one: look at its 3x3.
            let near = (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (dx, dy)));
            if near.clone().any(|(dx, dy)| lum(c.get(x + dx, y + dy)) > lum(panel) + 6.0) {
                on += 1;
            }
        }
        assert!(on >= 3 && on * 3 >= total * 2, "only {on} of {total} points on the outer orbit are lit - no visible ellipse");
        // And a control: between the two outer rings, at the side, the panel shows - paths, not a haze.
        let (x1, y1, _) = ring_px(&g, k, 0.0).unwrap();
        let (x0, _, _) = ring_px(&g, k - 1, 0.0).unwrap();
        let gap = (-1..=1).map(|dx| lum(c.get((x0 + x1) / 2 + dx, y1))).fold(0.0f32, f32::max);
        assert!(gap < lum(panel) + 6.0, "between two rings is lit ({gap:.1}) - the orbits are a wash, not lines");
    }

    #[test]
    fn comet_tail_length_follows_level() {
        // One planet, so nothing else crosses its path. Walk BACK along its ring from where it is and
        // count the path pixels its tail has lit, outside the planet's own disc.
        let t = builtin::orbit_solo();
        let (w, h) = (380, 60);
        let tail = |level: f32| -> usize {
            let mut fam = Orbit::default();
            let mut c = Canvas::new(w, h);
            for _ in 0..120 {
                fam.draw(&mut c, &t, &flat(level));
            }
            let g = geo_of(&fam, w, h);
            let a = fam.shown[0];
            let (px, py, _) = ring_px(&g, 0, a).unwrap();
            let clear = 9; // the fattest solo planet plus its keyline
            let mut seen = std::collections::HashSet::new();
            for step in 1..=200 {
                let (x, y, _) = ring_px(&g, 0, a - step as f32 * 0.01).unwrap();
                if (x - px).pow(2) + (y - py).pow(2) <= clear * clear {
                    continue;
                }
                if lum(c.get(x, y)) > 90.0 {
                    seen.insert((x, y));
                }
            }
            seen.len()
        };
        let (quiet, loud) = (tail(0.2), tail(0.9));
        assert!(loud >= 20, "a loud planet's tail lit only {loud} px of its path");
        assert!(loud > quiet * 3 / 2, "the tail is {loud} px loud against {quiet} px quiet - its length is not the level");
    }

    #[test]
    fn alignment_flourish_brings_planets_to_one_angle() {
        let t = builtin::orbit_chrome();
        let mut fam = Orbit::default();
        let mut c = Canvas::new(380, 60);
        let spread = |fam: &Orbit| {
            let mut worst = 0.0f32;
            for i in 0..fam.n {
                for j in 0..fam.n {
                    worst = worst.max(toward(fam.shown[i], fam.shown[j]).abs());
                }
            }
            worst
        };
        for k in 0..60 {
            fam.draw(&mut c, &t, &frame(0.5, k as f32 * 0.0167));
        }
        assert!(spread(&fam) > 1.0, "the planets are already bunched ({:.2} rad) - the test proves nothing", spread(&fam));
        fam.flourish.force_next();
        // 24 frames is 400.8 ms: the ease is complete.
        for k in 60..84 {
            fam.draw(&mut c, &t, &frame(0.5, k as f32 * 0.0167));
        }
        let s = spread(&fam);
        assert!(s < 0.1, "400 ms into the alignment the planets span {s:.2} rad - they have not lined up");
        // And they scatter back GRADUALLY - half way through the release they are neither lined up
        // nor home. A snap back at the end of the envelope is the jarring step the old scatter had.
        for k in 84..126 {
            fam.draw(&mut c, &t, &frame(0.5, k as f32 * 0.0167));
        }
        let mid = spread(&fam);
        assert!(mid > 0.3, "1.1 s in, mid-release, the planets still span only {mid:.2} rad - no gradual scatter");
        for k in 126..180 {
            fam.draw(&mut c, &t, &frame(0.5, k as f32 * 0.0167));
        }
        assert!(spread(&fam) > 1.0, "the planets stayed aligned ({:.2} rad) - they never scattered back", spread(&fam));
    }

    /// Run: cargo test --release probe_orbit_cost -- --ignored --nocapture
    #[test]
    #[ignore]
    fn probe_orbit_cost() {
        for id in ["orbit-chrome", "orbit-swarm"] {
            let t = builtin::all().into_iter().find(|t| t.id == id).unwrap();
            let mut fam = Orbit::default();
            let mut c = Canvas::new(380, 60);
            for k in 0..60 {
                fam.draw(&mut c, &t, &frame(0.7, k as f32 * 0.0167));
            }
            let n = 400;
            let t0 = std::time::Instant::now();
            for k in 0..n {
                fam.draw(&mut c, &t, &frame(0.7, k as f32 * 0.0167));
            }
            let steady = t0.elapsed().as_secs_f64() * 1000.0 / n as f64;
            println!("{id} steady: {steady:.3} ms/frame at 380x60");

            // The whole alignment: ease, flash, hold and scatter.
            fam.flourish.force_next();
            let m = 88;
            let t1 = std::time::Instant::now();
            for k in n..(n + m) {
                fam.draw(&mut c, &t, &frame(0.7, k as f32 * 0.0167));
            }
            let flourish = t1.elapsed().as_secs_f64() * 1000.0 / m as f64;
            println!("{id} flourish: {flourish:.3} ms/frame at 380x60");
        }
    }

    /// The fidelity-pass eye test: calm / loud / flourish (the aligned moment, flash on) for the
    /// colourways, at the wide size and at 128x44.
    ///
    /// Run: cargo test --release dump_orbit_fidelity -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_orbit_fidelity() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();
        let write = |name: String, c: &Canvas| {
            let mut out = Vec::new();
            for y in 0..c.height() {
                for x in 0..c.width() {
                    let px = c.get(x, y);
                    out.extend_from_slice(&[px.r, px.g, px.b, px.a]);
                }
            }
            std::fs::write(dir.join(format!("{name}.rgba")), &out).unwrap();
        };
        for tid in ["orbit-chrome", "orbit-plasma", "orbit-rainbow", "orbit-solo", "orbit-swarm", "orbit-sodium"] {
            let theme = builtin::all().into_iter().find(|t| t.id == tid).unwrap();
            for (w, hh) in [(380, 60), (128, 44)] {
                // The steady state: the synthetic hits would otherwise fire the alignment by themselves.
                let mut steady = theme.clone();
                steady.flourish = 0.0;
                for (tag, gain) in [("calm", 0.30), ("loud", 0.85)] {
                    let theme = steady.clone();
                    let mut fam = Orbit::default();
                    let mut c = Canvas::new(w, hh);
                    for k in 0..300 {
                        fam.draw(&mut c, &theme, &frame(gain, k as f32 * 0.0167));
                    }
                    write(format!("{tid}-{tag}-{w}x{hh}"), &c);
                }
                let mut fam = Orbit::default();
                let mut c = Canvas::new(w, hh);
                for k in 0..300 {
                    fam.draw(&mut c, &theme, &frame(0.62, k as f32 * 0.0167));
                }
                fam.flourish.force_next();
                for k in 300..324 {
                    fam.draw(&mut c, &theme, &frame(0.62, k as f32 * 0.0167));
                }
                write(format!("{tid}-flourish-{w}x{hh}"), &c);
            }
        }
        println!("wrote orbit fidelity dumps to {}", dir.display());
    }

    /// Run: cargo test --release dump_orbit -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_orbit() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();
        let write = |name: String, c: &Canvas| {
            let mut out = Vec::new();
            for y in 0..c.height() {
                for x in 0..c.width() {
                    let px = c.get(x, y);
                    out.extend_from_slice(&[px.r, px.g, px.b, px.a]);
                }
            }
            std::fs::write(dir.join(format!("{name}.rgba")), &out).unwrap();
        };
        for t in builtin::all().into_iter().filter(|t| t.family == "orbit") {
            let mut fam = Orbit::default();
            let mut c = Canvas::new(380, 60);
            for k in 0..300 {
                fam.draw(&mut c, &t, &frame(0.7, k as f32 * 0.0167));
            }
            write(format!("orbit-{}", t.id), &c);
        }
        // A lap, so the tilt and the occlusion are visible as a sequence.
        let t = builtin::orbit_chrome();
        let mut fam = Orbit::default();
        let mut c = Canvas::new(380, 60);
        let mut shot = 0;
        for k in 0..900 {
            fam.draw(&mut c, &t, &frame(0.7, k as f32 * 0.0167));
            if k >= 60 && (k - 60) % 130 == 0 && shot < 6 {
                write(format!("orbit-lap-{shot}"), &c);
                shot += 1;
            }
        }
        println!("wrote orbit dumps");
    }
}
