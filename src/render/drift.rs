//! The drift family: Tokyo Drift on a touge. A chase camera behind a car drifting down a winding
//! mountain road at night - the city's neon in the valley below - whose tyre smoke IS the meter.
//!
//! # The view
//!
//! A pseudo-3D racer road (the OutRun technique): every ground row below the horizon is a depth
//! `z = CAM_H * f / (row - horizon)`, and the road's centre at that depth comes from integrating the
//! course's curvature forward from the camera, so bends sweep across the wide panel and the road
//! narrows to the horizon. The camera is turned to the road's heading at the car (`ROT`) and follows
//! the car sideways (`CAM_FOLLOW`), so the car stays near the bottom centre while the road swings under
//! it. Everything with a position on the road - the car, its smoke, the posts and the trees - is
//! projected through the same perspective camera.
//!
//! # The scene
//!
//! - **Sky (baked).** A night gradient toward `edge` with a wash of the neon at the horizon (the city's
//!   light pollution) and a scatter of stars.
//! - **Ridges.** Two mountain silhouettes on the horizon (a hazy far one, a black near one with a lit
//!   rim), shifting sideways as the car corners - the far one at half the rate.
//! - **The road.** Asphalt in alternating bands, red-and-white kerbs and a dashed centre line, all of
//!   which stream toward the camera; fogged toward the horizon.
//! - **The touge.** On the left a cut-rock cliff stands beside the road behind a gutter, `CLIFF_H` to
//!   `CLIFF_H + CLIFF_VAR` high with a jagged top, each stretch its own shade of rock, two ragged
//!   strata lines, cracks and a rim catching the sky, pines
//!   along its top; near the camera it towers out of the top of the panel. On the right a guardrail on
//!   reflector posts, a black drop, and far below the valley, where the city's lights glitter - the
//!   colourway's neon with a few warm and `hot` ones, some twinkling.
//! - **The car.** A low-poly coupe modelled on the rear of an R34 Skyline (the user's ask) - no badge:
//!   a squared-off tail with FOUR ROUND TAIL LAMPS (a `hot` ring with a darker centre, two each side),
//!   a tall rear wing on struts, a cabin with dark glass that catches the neon, four wheels, the
//!   fronts counter-steering.
//!   Projected and flat-shaded each frame (faces culled by projected winding, parts in painter's
//!   order, a 1 px dark outline under it all). Seen from behind: tail lamps and their glow, neon
//!   underglow on the road under it, the headlights' beam on the road ahead.
//! - **The hills.** The course rises and falls (`HILLS`, gradients to ~11 %); road segments are drawn
//!   near to far, each filling only the rows above the nearer road, so a crest hides what is beyond
//!   it, and posts and pines behind a crest are skipped.
//! - **The music.** Beyond the smoke meter: speed and drift angle follow rms; a bass kick is a clutch
//!   kick (more angle, an exhaust pop, the tail lamps flaring and the camera jolting down); the neon
//!   underglow pulses with the bass, the valley's city lights swell with the mids and the reflectors
//!   glint with the treble.
//! - **The drift.** It comes from the road. In a bend the car slides to the outside and holds a slip
//!   angle into the corner (up to `SLIP_MAX` at full rms); through an S-bend it swaps lock, quickly
//!   (`FLICK_MS`). A bass kick is a clutch kick: `KICK_SLIP` more angle that decays, and a pop of
//!   flame from the exhaust. Silence: the car drives the bends on grip, nearly straight.
//! - **The meter - the tyre smoke.** A fixed ring of the last 64 puffs, laid alternately at the two
//!   rear tyres and then left in the air: they drag behind the car, rise and spread toward the outside
//!   of the corner, so the smoke rolls back up toward the camera. Puff `k` (band `k`, bass at the
//!   tyres) has a radius of `PUFF_R0 + level * PUFF_GAIN` world units, swelling with age, in white at
//!   `ghost` alpha. `FrameData.levels` arrive already smoothed by main's `Smoother`; no second
//!   attack/decay here.
//! - **Steering gauge.** Top right, a radius-8 semicircle with a needle at the slip angle and
//!   `ANGLE NN` (`[u8; 2]`). Dropped below 200 px wide.
//!
//! # The flourish - the drift
//!
//! 900 ms, fired only on a bass hit: the car spins a full 360 on the road, the smoke goes to full
//! radius tinted `hot`, the reflectors and the valley flash, and `DRIFT!` (2x `font3x5`, `hot`, 1 px
//! dark outline) slams in centred in the sky for 400 ms (one frame at 3x, then 2x with a 1 px shake).
//!
//! The panel is opaque (painted first) and the clip to the rounded rect runs last on every path.
//! `draw` allocates nothing after the first frame at a size: the sky canvas and the layout are rebuilt
//! only on a size (or colour) change; the road table, the row tables, the smoke ring and the car's
//! polygons are fixed arrays.

use std::f32::consts::TAU;

use crate::dsp::bands::NUM_BANDS;
use crate::dsp::flourish::{Envelope, Trigger};
use crate::dsp::onset::Flux;
use crate::render::canvas::{Canvas, Rgba};
use crate::render::font3x5;
use crate::render::{Family, FrameData};
use crate::themes::Theme;

// ---- the camera ----

/// The horizon's share of the interior height from the top.
const HZ_FRAC: f32 = 0.40;
/// Focal length in px on a full-height panel; camera height and the car's distance, world units.
const FOCAL: f32 = 100.0;
const CAM_H: f32 = 11.0;
const CAR_Z: f32 = 46.0;
/// Nothing nearer than this is projected.
const NEAR: f32 = 2.0;
/// How much of the road's heading at the car the camera turns to, and how much of the car's sideways
/// position it follows.
const ROT: f32 = 0.85;
const CAM_FOLLOW: f32 = 0.75;
/// The light, in world (x right, y up, z ahead): from above, a little left and behind.
const LIGHT: (f32, f32, f32) = (-0.35, 0.85, -0.40);

// ---- the road ----

/// The road's half width, its kerbs' width and the centre line's, world units.
const HALF_W: f32 = 19.0;
const KERB_W: f32 = 1.6;
const LINE_W: f32 = 0.5;
/// Lengths of the alternating asphalt bands and the centre dashes.
const SEG: f32 = 6.0;
const DASH: f32 = 10.0;
/// The road table: depth `Z_FAR` in `DZ` steps.
const Z_FAR: f32 = 320.0;
const DZ: f32 = 2.0;
const ROAD_N: usize = 161;
/// The course repeats every `COURSE` units (a multiple of every roadside period, so nothing jumps
/// at the wrap); its curvature reaches `C_MAX` per unit in the tightest bends.
const COURSE: f32 = 7200.0;
const C_MAX: f32 = 0.012;
/// The hills: (cycles per course, amplitude in world units, phase). Gradients reach ~11 %, so the
/// road climbs to crests that hide what is beyond and drops into dips.
const HILLS: [(f32, f32, f32); 2] = [(5.0, 12.0, 0.0), (11.0, 6.0, 1.3)];
/// Roadside: reflector posts, the guardrail's height, pines.
const POST_GAP: f32 = 12.0;
const POST_H: f32 = 2.2;
const RAIL_H: f32 = 1.5;
const TREE_GAP: f32 = 18.0;
/// The cliff on the inside of the pass: its face stands this far beyond the kerb (a gutter), its
/// height varies between `CLIFF_H` and `CLIFF_H + CLIFF_VAR` over knots `CLIFF_KNOT` apart, and
/// ragged strata lines run along it.
const CLIFF_GAP: f32 = 1.5;
const CLIFF_H: f32 = 17.0;
const CLIFF_VAR: f32 = 10.0;
const CLIFF_KNOT: f32 = 30.0;
const STRATA: [f32; 2] = [0.3, 0.62];
/// Fog: full haze at this depth.
const FOG_Z: f32 = 520.0;
/// Per-row tables are this long (taller panels clamp).
const ROWS: usize = 256;
/// Ridges: a periodic height table this wide; city lights in a virtual strip twice that.
const RIDGE_W: usize = 1024;
const LIGHTS: usize = 220;

// ---- the motion ----

/// Speed, units per second, plus more with rms.
const V_BASE: f32 = 60.0;
const V_RMS: f32 = 70.0;
/// How fast the motion's amplitude follows rms (a motion ease, not a level smoother).
const AMP_EASE_MS: f32 = 600.0;
/// The slip angle at full rms in the tightest bend, degrees, and how fast the body swaps lock.
const SLIP_MAX: f32 = 38.0;
const FLICK_MS: f32 = 160.0;
/// The slide toward the outside of a bend, world units at full rms, and its spring (rad/s).
const LAT_SLIDE: f32 = 6.0;
const LAT_OMEGA: f32 = 4.0;
/// A clutch kick: extra slip and how fast it decays; the exhaust pop's length.
const KICK_SLIP: f32 = 14.0;
const KICK_DECAY_MS: f32 = 300.0;
const FLAME_MS: f32 = 90.0;
/// A kick's camera jolt (world units of camera height) and how fast it settles.
const BUMP: f32 = 1.1;
const BUMP_MS: f32 = 140.0;
/// Below this much motion there is no clutch kick.
const KICK_AMP: f32 = 0.08;
/// A kick: an onset (the `bling` net) with the low bands over this.
const KICK_BASS: f32 = 0.45;
const ONSET_RATIO: f32 = 2.8;
const ONSET_REFRACTORY_MS: f32 = 200.0;
/// The front wheels counter-steer up to this, degrees.
const COUNTER_MAX: f32 = 30.0;

// ---- the smoke ----

/// The trail: one puff per band.
const TRAIL: usize = NUM_BANDS;
/// Puff radius `PUFF_R0 + level * PUFF_GAIN` world units, swelling by `PUFF_GROW` over its life.
const PUFF_R0: f32 = 0.35;
const PUFF_GAIN: f32 = 2.0;
const PUFF_GROW: f32 = 0.8;
/// A puff moves at this fraction of the car's speed (so it falls back toward the camera), rises and
/// spreads to the outside of the corner, units per second; no puff is drawn bigger than this, px.
const DRAG: f32 = 0.6;
const RISE: f32 = 8.0;
const SPREAD: f32 = 5.0;
const PUFF_MAX_PX: f32 = 8.0;
/// How much of the road's rise and fall the smoke rides.
const SMOKE_SLOPE: f32 = 0.35;

// ---- the car's model: `u` forward, `v` to its left, `y` up, world units ----

/// The body: an octagonal prism, nose and tail chamfered, counter-clockwise seen from above. Edge 0
/// is the front face, edge 4 the rear.
const BODY: [(f32, f32); 8] =
    [(11.0, -3.6), (11.0, 3.6), (9.2, 5.0), (-10.4, 5.0), (-11.0, 4.5), (-11.0, -4.5), (-10.4, -5.0), (9.2, -5.0)];
const BODY_Y: (f32, f32) = (1.0, 4.0);
/// The cabin: a frustum from the body's top to the roof; its faces are front, left, rear, right.
const CABIN_LO: [(f32, f32); 4] = [(4.0, -4.3), (4.0, 4.3), (-6.6, 4.3), (-6.6, -4.3)];
const CABIN_HI: [(f32, f32); 4] = [(0.4, -3.5), (0.4, 3.5), (-4.6, 3.5), (-4.6, -3.5)];
const CABIN_Y: (f32, f32) = (4.0, 6.9);
/// The rear wing: a thin plate on two struts.
const WING: [(f32, f32); 4] = [(-9.6, -5.2), (-9.6, 5.2), (-11.8, 5.2), (-11.8, -5.2)];
const WING_Y: (f32, f32) = (6.4, 7.0);
const STRUT: (f32, f32) = (-10.5, 2.8);
/// Wheels: the axles, the track's half width, and a wheel's half length, half width and height.
const AXLE_F: f32 = 7.2;
const AXLE_R: f32 = -7.0;
const TRACK: f32 = 4.15;
const WHEEL: (f32, f32, f32) = (1.9, 0.8, 2.5);
/// Headlamps: each one's `v` span out from the centre line and its `y` span, on the front face.
const LAMP_V: (f32, f32) = (1.7, 3.5);
const LAMP_Y: (f32, f32) = (2.3, 3.7);
/// The R34-style tail: four round lamps on the rear face, two each side - centres out from the
/// centre line, their height, the ring's radius and the darker centre's.
const TAIL_V: [f32; 2] = [1.75, 3.45];
const TAIL_Y: f32 = 3.0;
const TAIL_R: f32 = 0.82;
const TAIL_CORE: f32 = 0.42;
/// The exhaust, on the rear face.
const EXHAUST: (f32, f32, f32) = (-11.2, -2.6, 1.6);

// ---- the gauge, the drift ----

/// Below this width the steering gauge is dropped.
const GAUGE_MIN_W: i32 = 200;
const GAUGE_R: i32 = 8;
/// The drift: total length, the spin's share, `DRIFT!`'s window, the flash.
const DRIFT_MS: f32 = 900.0;
const SPIN_MS: f32 = 720.0;
const TEXT_FROM_MS: f32 = 120.0;
const TEXT_MS: f32 = 400.0;
const FLASH_MS: f32 = 600.0;
const FLASH_HZ: f32 = 12.0;
/// The flourish only fires on a BASS hit (the `sesh` pattern).
const FLOURISH_BASS_MIN: f32 = 0.6;

const WHITE: Rgba = Rgba { r: 255, g: 255, b: 255, a: 255 };
const BLACK: Rgba = Rgba { r: 0, g: 0, b: 0, a: 255 };
const TYRE: Rgba = Rgba { r: 0x0a, g: 0x0a, b: 0x0c, a: 255 };
const TYRE_HI: Rgba = Rgba { r: 0x34, g: 0x34, b: 0x3a, a: 255 };
const WARM: Rgba = Rgba { r: 0xff, g: 0xc8, b: 0x7a, a: 255 };
const ASPHALT: Rgba = Rgba { r: 0x4a, g: 0x4a, b: 0x52, a: 255 };
const KERB_RED: Rgba = Rgba { r: 0xc8, g: 0x1e, b: 0x2a, a: 255 };
const FLAME: Rgba = Rgba { r: 0xff, g: 0x8a, b: 0x1e, a: 255 };
/// `DRIFT!`'s outline and the car's.
const OUTLINE: Rgba = Rgba { r: 0x05, g: 0x03, b: 0x08, a: 255 };

/// A city light in the valley: a column in the virtual strip, a row, its colour, and its twinkle.
#[derive(Clone, Copy, Default)]
struct Light {
    vx: f32,
    row: i32,
    kind: u8,
    twinkle: bool,
    rate: f32,
    phase: f32,
}

/// Everything positional, for a panel size. Shared by `draw`, the bake and the tests.
#[derive(Clone, Copy)]
struct Layout {
    /// World scale: 1 on a full-height panel; the focal length; the horizon line and the first
    /// ground row.
    s: f32,
    f: f32,
    hz: f32,
    hz_row: i32,
    ridge_far: [u8; RIDGE_W],
    ridge_near: [u8; RIDGE_W],
    lights: [Light; LIGHTS],
    lights_n: usize,
    show_gauge: bool,
    gauge: (i32, i32),
    angle_text: (i32, i32),
    /// `DRIFT!`'s top-left at 2x.
    drift_text: (i32, i32),
}

impl Default for Layout {
    fn default() -> Self {
        Layout {
            s: 1.0,
            f: FOCAL,
            hz: 0.0,
            hz_row: 0,
            ridge_far: [0; RIDGE_W],
            ridge_near: [0; RIDGE_W],
            lights: [Light::default(); LIGHTS],
            lights_n: 0,
            show_gauge: false,
            gauge: (0, 0),
            angle_text: (0, 0),
            drift_text: (0, 0),
        }
    }
}

/// The width of `text` in `font3x5` at `scale`.
fn text_w(text: &str, scale: i32) -> i32 {
    (text.chars().count() as i32 * 4 * scale - scale).max(0)
}

/// One draw of splitmix64 on a caller's state.
fn splitmix(s: &mut u64) -> u64 {
    *s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *s;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// A uniform 0..1 from splitmix.
fn unit(s: &mut u64) -> f32 {
    (splitmix(s) >> 40) as f32 / (1u64 << 24) as f32
}

/// A uniform 0..1 hashed from an integer, for roadside objects keyed by their slot on the course.
fn hash01(k: i64) -> f32 {
    let mut s = (k as u64).wrapping_mul(0x2545_F491_4F6C_DD1D) ^ 0x1234_5678;
    unit(&mut s)
}

/// A periodic ridge: a few sines with hashed phases, scaled into `lo..hi` of `span` rows.
fn ridge(out: &mut [u8; RIDGE_W], seed: u64, span: f32, lo: f32, hi: f32) {
    let mut s = seed;
    let waves: [(f32, f32, f32); 5] = [
        (3.0, 1.0, unit(&mut s)),
        (7.0, 0.55, unit(&mut s)),
        (13.0, 0.3, unit(&mut s)),
        (29.0, 0.14, unit(&mut s)),
        (61.0, 0.06, unit(&mut s)),
    ];
    let total: f32 = waves.iter().map(|w| w.1).sum();
    for (i, h) in out.iter_mut().enumerate() {
        let x = i as f32 / RIDGE_W as f32;
        let mut v = 0.0;
        for &(k, a, ph) in &waves {
            v += a * (TAU * (k * x + ph)).sin();
        }
        let n = (v / total) * 0.5 + 0.5;
        *h = (span * (lo + (hi - lo) * n)).round().clamp(0.0, 255.0) as u8;
    }
}

fn layout(w: i32, h: i32) -> Layout {
    let (ix0, iy0, ix1, iy1) = (3, 4, w - 3, h - 4);
    let (iw, ih) = (ix1 - ix0, iy1 - iy0);
    let mut l = Layout::default();
    l.s = (ih as f32 / 52.0).clamp(0.6, 1.0);
    l.f = FOCAL * l.s;
    l.hz_row = iy0 + (ih as f32 * HZ_FRAC).round() as i32;
    l.hz = l.hz_row as f32;
    let sky_h = (l.hz_row - iy0).max(1) as f32;
    ridge(&mut l.ridge_far, 0x7261_6467_6566_6172, sky_h, 0.18, 0.62);
    ridge(&mut l.ridge_near, 0x6e65_6172_7269_6467, sky_h, 0.04, 0.34);

    // The valley's lights: denser toward the horizon (the far city).
    let ground = (iy1 - l.hz_row).max(1) as f32;
    let mut s: u64 = 0x5eed_d71f_7000_0002;
    for i in 0..LIGHTS {
        let r = unit(&mut s);
        let row = l.hz_row + 1 + (r * r * ground * 0.32) as i32;
        let k = unit(&mut s);
        l.lights[i] = Light {
            vx: unit(&mut s) * 2.0 * RIDGE_W as f32,
            row,
            kind: if k < 0.68 {
                0
            } else if k < 0.9 {
                1
            } else {
                2
            },
            twinkle: splitmix(&mut s).is_multiple_of(3),
            rate: 0.7 + unit(&mut s) * 1.3,
            phase: unit(&mut s),
        };
        l.lights_n += 1;
    }

    // ---- the gauge (top right) and its readout ----
    l.show_gauge = w >= GAUGE_MIN_W;
    l.gauge = (ix1 - 3 - GAUGE_R, iy0 + 2 + GAUGE_R);
    l.angle_text = (l.gauge.0 - GAUGE_R - 5 - text_w("ANGLE 00", 1), l.gauge.1 - 5);

    // ---- DRIFT!, centred in the sky ----
    let tw = text_w("DRIFT!", 2);
    l.drift_text = (ix0 + (iw - tw) / 2, iy0 + (sky_h as i32 - 10) / 2);
    l
}

/// The course's curvature at distance `s` (wrapped): bends that alternate left and right with short
/// straights between, some tighter than others. Positive bends to the right.
fn curve(s: f32) -> f32 {
    let p = TAU * s.rem_euclid(COURSE) / COURSE;
    let shape = 1.5 * (8.0 * p).sin() * (0.75 + 0.25 * (3.0 * p).sin()) + 0.3 * (5.0 * p).sin();
    C_MAX * shape.clamp(-1.0, 1.0)
}

/// The course's height at distance `s` (wrapped).
fn hill(s: f32) -> f32 {
    let p = TAU * s.rem_euclid(COURSE) / COURSE;
    HILLS.iter().map(|&(k, a, ph)| a * (k * p + ph).sin()).sum()
}

/// The cliff's height at distance `s`: hashed knots, smoothly joined, wrapping with the course.
fn cliff_h(s: f32) -> f32 {
    let knots = (COURSE / CLIFF_KNOT) as i64;
    let u = s.rem_euclid(COURSE) / CLIFF_KNOT;
    let k = u.floor() as i64;
    let t = ease(u - k as f32);
    let (a, b) = (hash01(k.rem_euclid(knots) * 31 + 7), hash01((k + 1).rem_euclid(knots) * 31 + 7));
    CLIFF_H + CLIFF_VAR * (a + (b - a) * t)
}

/// A twinkling light is off for a stutter each cycle.
fn light_on(l: &Light, e: f32) -> bool {
    if !l.twinkle {
        return true;
    }
    let f = (e * l.rate + l.phase).fract();
    !(0.0..0.18).contains(&f)
}

/// The perspective camera: screen centre column, horizon line, focal length, and its sideways
/// position in the road's frame.
#[derive(Clone, Copy)]
struct Cam {
    cx: f32,
    hz: f32,
    f: f32,
    x: f32,
    /// Its height above the ground under the car.
    y: f32,
}

impl Cam {
    fn project(&self, x: f32, y: f32, z: f32) -> (f32, f32) {
        let z = z.max(NEAR);
        (self.cx + (x - self.x) * self.f / z, self.hz + (self.y - y) * self.f / z)
    }
}

/// Fills a convex polygon given in float pixel coordinates, offset by `off`: a pixel is in when its
/// centre is, so a slowly turning shape steps a pixel at a time rather than jumping with rounded
/// vertices. Rows above `min_row` are left alone.
fn fill_convex(c: &mut Canvas, pts: &[(f32, f32)], off: (f32, f32), min_row: i32, col: Rgba) {
    fill_convex_rows(c, pts, off, min_row, i32::MAX, col);
}

/// `fill_convex` limited to rows `min_row..max_row` - the cliff uses the upper limit to stay behind
/// the nearer road a crest has already drawn.
fn fill_convex_rows(c: &mut Canvas, pts: &[(f32, f32)], off: (f32, f32), min_row: i32, max_row: i32, col: Rgba) {
    let n = pts.len();
    if n < 3 || col.a == 0 {
        return;
    }
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for p in pts {
        lo = lo.min(p.1 + off.1);
        hi = hi.max(p.1 + off.1);
    }
    if !lo.is_finite() || !hi.is_finite() {
        return;
    }
    // Rows whose centre is in [lo, hi).
    let first = ((lo - 0.5).ceil() as i32).max(min_row).max(0);
    let last = ((hi - 0.5).ceil() as i32 - 1).min(c.height() - 1).min(max_row - 1);
    for y in first..=last {
        let yc = y as f32 + 0.5;
        let (mut xl, mut xr) = (f32::MAX, f32::MIN);
        for i in 0..n {
            let (ax, ay) = (pts[i].0 + off.0, pts[i].1 + off.1);
            let (bx, by) = (pts[(i + 1) % n].0 + off.0, pts[(i + 1) % n].1 + off.1);
            if (ay <= yc) != (by <= yc) {
                let x = ax + (yc - ay) / (by - ay) * (bx - ax);
                xl = xl.min(x);
                xr = xr.max(x);
            }
        }
        if xl >= xr {
            continue;
        }
        let (x0, x1) = ((xl - 0.5).ceil() as i32, (xr - 0.5).ceil() as i32);
        if x1 > x0 {
            c.fill_rect(x0, y, x1 - x0, 1, col);
        }
    }
}

/// A horizontal span `x0..x1` (float, rounded) on row `y`.
fn span(c: &mut Canvas, x0: f32, x1: f32, y: i32, col: Rgba) {
    let (a, b) = (x0.round() as i32, x1.round() as i32);
    if b > a {
        c.fill_rect(a, y, b - a, 1, col);
    }
}

/// Twice the signed area of a screen polygon (y down). Negative is what an outward-wound face turned
/// toward the viewer projects to.
fn area2(pts: &[(f32, f32)]) -> f32 {
    let n = pts.len();
    let mut a = 0.0;
    for i in 0..n {
        let (p, q) = (pts[i], pts[(i + 1) % n]);
        a += p.0 * q.1 - q.0 * p.1;
    }
    a
}

/// Where a part of the car is and which way it faces: a centre on the road (world x, z), a heading
/// (`cs`, `sn` of the yaw from straight ahead, positive toward +x), and the camera.
#[derive(Clone, Copy)]
struct Pose {
    x: f32,
    z: f32,
    cs: f32,
    sn: f32,
    /// The ground's height under it.
    e: f32,
    cam: Cam,
}

impl Pose {
    fn new(x: f32, z: f32, yaw: f32, cam: Cam) -> Self {
        Pose { x, z, cs: yaw.cos(), sn: yaw.sin(), e: 0.0, cam }
    }
    /// Local (u, v) on the ground to world (x, z): forward is (sn, cs), left is (-cs, sn).
    fn at(&self, u: f32, v: f32) -> (f32, f32) {
        (self.x + u * self.sn - v * self.cs, self.z + u * self.cs + v * self.sn)
    }
    fn screen(&self, p: (f32, f32, f32)) -> (f32, f32) {
        let (x, z) = self.at(p.0, p.1);
        self.cam.project(x, p.2 + self.e, z)
    }
    /// A local direction (u, v, y) to world (x, y, z).
    fn turn(&self, n: (f32, f32, f32)) -> (f32, f32, f32) {
        (n.0 * self.sn - n.1 * self.cs, n.2, n.0 * self.cs + n.1 * self.sn)
    }
    /// The pose of a part at local (u, v), turned `dyaw` further.
    fn child(&self, u: f32, v: f32, dyaw: f32) -> Pose {
        let (x, z) = self.at(u, v);
        let (c, s) = (dyaw.cos(), dyaw.sin());
        Pose { x, z, cs: self.cs * c - self.sn * s, sn: self.sn * c + self.cs * s, e: self.e, cam: self.cam }
    }
}

/// What a face is made of.
#[derive(Clone, Copy)]
enum Mat {
    Body,
    Glass,
    Tyre,
    Rim,
    Head,
    Tail,
    TailCore,
    Carbon,
}

/// The car's colours for a colourway, each material from its shadow to its lit tone.
struct Paint {
    lit: Rgba,
    body_lo: Rgba,
    glass_lo: Rgba,
    glass_hi: Rgba,
    rim: Rgba,
    head: Rgba,
    tail: Rgba,
    tail_core: Rgba,
    carbon_lo: Rgba,
    carbon_hi: Rgba,
}

impl Paint {
    fn new(t: &Theme) -> Self {
        let lit = Rgba::from_hex(&t.lit, 1.0);
        let panel = Rgba::from_hex(&t.panel, 1.0);
        let neon = neon_of(t);
        Paint {
            lit,
            body_lo: Rgba::lerp_linear(lit, BLACK, 0.78),
            glass_lo: Rgba::lerp_linear(panel, BLACK, 0.4),
            glass_hi: Rgba::lerp_linear(panel, neon, 0.4),
            rim: Rgba::lerp_linear(lit, BLACK, 0.35),
            head: Rgba::lerp_linear(lit, WHITE, 0.75),
            tail: Rgba::from_hex(&t.hot, 1.0),
            tail_core: Rgba::lerp_linear(Rgba::from_hex(&t.hot, 1.0), BLACK, 0.55),
            carbon_lo: OUTLINE,
            carbon_hi: Rgba::lerp_linear(OUTLINE, lit, 0.35),
        }
    }
    fn col(&self, m: Mat, shade: f32) -> Rgba {
        match m {
            Mat::Body => Rgba::lerp_linear(self.body_lo, self.lit, shade),
            Mat::Glass => Rgba::lerp_linear(self.glass_lo, self.glass_hi, shade * shade),
            Mat::Tyre => Rgba::lerp_linear(TYRE, TYRE_HI, shade * 0.6),
            Mat::Rim => self.rim,
            Mat::Head => self.head,
            Mat::Tail => self.tail,
            Mat::TailCore => self.tail_core,
            Mat::Carbon => Rgba::lerp_linear(self.carbon_lo, self.carbon_hi, shade),
        }
    }
}

/// One projected polygon of the car (or, with `n == 2`, a 1 px strut line), in draw order.
#[derive(Clone, Copy)]
struct Shape {
    pts: [(f32, f32); 8],
    n: usize,
    col: Rgba,
}

const NO_SHAPE: Shape = Shape { pts: [(0.0, 0.0); 8], n: 0, col: Rgba { r: 0, g: 0, b: 0, a: 0 } };
const MAX_SHAPES: usize = 56;

/// The car's visible polygons for a frame: a fixed array, so drawing the car allocates nothing.
struct Shapes {
    s: [Shape; MAX_SHAPES],
    n: usize,
}

impl Shapes {
    fn new() -> Self {
        Shapes { s: [NO_SHAPE; MAX_SHAPES], n: 0 }
    }
    fn push(&mut self, sh: Shape) {
        if self.n < MAX_SHAPES {
            self.s[self.n] = sh;
            self.n += 1;
        }
    }
}

/// Projects a face (local vertices, wound counter-clockwise seen from outside) and whether it is
/// turned toward the viewer.
fn projected(pose: &Pose, verts: &[(f32, f32, f32)]) -> ([(f32, f32); 8], usize, bool) {
    let mut pts = [(0.0f32, 0.0f32); 8];
    let n = verts.len().min(8);
    for i in 0..n {
        pts[i] = pose.screen(verts[i]);
    }
    let front = area2(&pts[..n]) < -0.05;
    (pts, n, front)
}

fn faces_viewer(pose: &Pose, verts: &[(f32, f32, f32)]) -> bool {
    projected(pose, verts).2
}

/// Adds a face if it is turned toward the viewer, flat-shaded by its normal against `LIGHT`.
fn face(out: &mut Shapes, pose: &Pose, verts: &[(f32, f32, f32)], m: Mat, paint: &Paint) {
    let (pts, n, front) = projected(pose, verts);
    if !front || n < 3 {
        return;
    }
    let (a, b, c) = (verts[0], verts[1], verts[2]);
    let (e1, e2) = ((b.0 - a.0, b.1 - a.1, b.2 - a.2), (c.0 - a.0, c.1 - a.1, c.2 - a.2));
    let nl = (e1.1 * e2.2 - e1.2 * e2.1, e1.2 * e2.0 - e1.0 * e2.2, e1.0 * e2.1 - e1.1 * e2.0);
    let nw = pose.turn(nl);
    let len = (nw.0 * nw.0 + nw.1 * nw.1 + nw.2 * nw.2).sqrt().max(1e-6);
    let ll = (LIGHT.0 * LIGHT.0 + LIGHT.1 * LIGHT.1 + LIGHT.2 * LIGHT.2).sqrt();
    let dot = (nw.0 * LIGHT.0 + nw.1 * LIGHT.1 + nw.2 * LIGHT.2) / (len * ll);
    // Normalised so a flat top is fully lit.
    let shade = (0.3 + 0.7 * dot.max(0.0) / (LIGHT.1 / ll)).clamp(0.0, 1.0);
    out.push(Shape { pts, n, col: paint.col(m, shade) });
}

/// A solid between two plans of the same vertex count (a prism when they match, a frustum when the
/// top is smaller): the top, then the sides, each side `i` on the plan edge `i -> i + 1`.
#[allow(clippy::too_many_arguments)]
fn solid(
    out: &mut Shapes,
    pose: &Pose,
    lo: &[(f32, f32)],
    hi: &[(f32, f32)],
    (y0, y1): (f32, f32),
    top: Mat,
    sides: &[Mat],
    paint: &Paint,
) {
    let n = lo.len().min(hi.len()).min(8);
    let mut v = [(0.0f32, 0.0f32, 0.0f32); 8];
    for i in 0..n {
        v[i] = (hi[i].0, hi[i].1, y1);
    }
    face(out, pose, &v[..n], top, paint);
    for i in 0..n {
        let j = (i + 1) % n;
        let quad = [(lo[i].0, lo[i].1, y0), (lo[j].0, lo[j].1, y0), (hi[j].0, hi[j].1, y1), (hi[i].0, hi[i].1, y1)];
        face(out, pose, &quad, sides[i.min(sides.len() - 1)], paint);
    }
}

/// A screen point.
type Pt = (f32, f32);

/// A ground plan of four corners, and a face of four local vertices.
type Plan4 = [(f32, f32); 4];
type Quad = [(f32, f32, f32); 4];

/// A wheel's pose, its plan, and its outer face (the one on the car's outside).
fn wheel_parts(pose: &Pose, u: f32, v: f32, steer: f32) -> (Pose, Plan4, Quad) {
    let w = pose.child(u, v, steer);
    let (hl, hw, hh) = WHEEL;
    let plan = [(hl, -hw), (hl, hw), (-hl, hw), (-hl, -hw)];
    let outer = if v > 0.0 {
        [(hl, hw, 0.0), (-hl, hw, 0.0), (-hl, hw, hh), (hl, hw, hh)]
    } else {
        [(-hl, -hw, 0.0), (hl, -hw, 0.0), (hl, -hw, hh), (-hl, -hw, hh)]
    };
    (w, plan, outer)
}

/// A whole wheel. Drawn before the body, which hides all of it but the tyre under the sills.
fn wheel(out: &mut Shapes, pose: &Pose, u: f32, v: f32, steer: f32, paint: &Paint) {
    let (w, plan, _) = wheel_parts(pose, u, v, steer);
    solid(out, &w, &plan, &plan, (0.0, WHEEL.2), Mat::Tyre, &[Mat::Tyre], paint);
}

/// A near wheel's outer face and rim, drawn AFTER the body so it shows in the lower flank like a
/// wheel in its arch. Only the outer face: the rest of the wheel is behind the body, and drawing it
/// over the body was what made the old wheels poke through the tail like a tractor's.
fn wheel_outer(out: &mut Shapes, pose: &Pose, u: f32, v: f32, steer: f32, paint: &Paint) {
    let (w, _, outer) = wheel_parts(pose, u, v, steer);
    face(out, &w, &outer, Mat::Tyre, paint);
    let hw = WHEEL.1 + 0.02;
    let rim = if v > 0.0 {
        [(0.7, hw, 0.7), (-0.7, hw, 0.7), (-0.7, hw, 1.8), (0.7, hw, 1.8)]
    } else {
        [(-0.7, -hw, 0.7), (0.7, -hw, 0.7), (0.7, -hw, 1.8), (-0.7, -hw, 1.8)]
    };
    face(out, &w, &rim, Mat::Rim, paint);
}

fn wing(out: &mut Shapes, pose: &Pose, paint: &Paint) {
    let col = paint.col(Mat::Carbon, 0.6);
    for v in [STRUT.1, -STRUT.1] {
        let a = pose.screen((STRUT.0, v, BODY_Y.1));
        let b = pose.screen((STRUT.0, v, WING_Y.0));
        let mut pts = [(0.0, 0.0); 8];
        pts[0] = a;
        pts[1] = b;
        out.push(Shape { pts, n: 2, col });
    }
    solid(out, pose, &WING, &WING, WING_Y, Mat::Carbon, &[Mat::Carbon], paint);
}

/// The four tail lamps' centres across the rear face.
fn tail_lamps() -> [f32; 4] {
    [TAIL_V[1], TAIL_V[0], -TAIL_V[0], -TAIL_V[1]]
}

/// A round lamp on the rear face (an octagon), wound the same way as the face so it culls with it.
fn lamp_disc(u: f32, vc: f32, r: f32) -> [(f32, f32, f32); 8] {
    let mut p = [(0.0f32, 0.0f32, 0.0f32); 8];
    for (k, q) in p.iter_mut().enumerate() {
        let a = -(k as f32) * TAU / 8.0;
        *q = (u, vc + r * a.cos(), TAIL_Y + r * a.sin());
    }
    p
}

/// The rear face's four corners.
fn tail_face() -> Quad {
    let (r0, r1) = (BODY[4], BODY[5]);
    [(r0.0, r0.1, BODY_Y.0), (r1.0, r1.1, BODY_Y.0), (r1.0, r1.1, BODY_Y.1), (r0.0, r0.1, BODY_Y.1)]
}

/// The car's visible polygons in painter's order: the wheels, the body, its lamps, the wing and
/// the cabin (the wing after the cabin when the tail faces the viewer), the near wheels' outer faces.
fn car_shapes(pose: &Pose, steer: f32, paint: &Paint) -> Shapes {
    car_shapes_opt(pose, steer, paint, true)
}

/// `car_shapes`, optionally without the wheels - a test hook to measure what the wheels add.
fn car_shapes_opt(pose: &Pose, steer: f32, paint: &Paint, with_wheels: bool) -> Shapes {
    let mut out = Shapes::new();
    let wheels = [(AXLE_F, TRACK, steer), (AXLE_F, -TRACK, steer), (AXLE_R, TRACK, 0.0), (AXLE_R, -TRACK, 0.0)];
    let near = wheels.map(|(u, v, st)| {
        let (w, _, outer) = wheel_parts(pose, u, v, st);
        faces_viewer(&w, &outer)
    });
    if with_wheels {
        for &(u, v, st) in &wheels {
            wheel(&mut out, pose, u, v, st, paint);
        }
    }
    solid(&mut out, pose, &BODY, &BODY, BODY_Y, Mat::Body, &[Mat::Body], paint);
    let (va, vb) = LAMP_V;
    let (ya, yb) = LAMP_Y;
    let (uf, ur) = (BODY[0].0 + 0.02, BODY[4].0 - 0.02);
    for (lo, hi) in [(va, vb), (-vb, -va)] {
        face(&mut out, pose, &[(uf, lo, ya), (uf, hi, ya), (uf, hi, yb), (uf, lo, yb)], Mat::Head, paint);
    }
    for vc in tail_lamps() {
        face(&mut out, pose, &lamp_disc(ur, vc, TAIL_R), Mat::Tail, paint);
        face(&mut out, pose, &lamp_disc(ur - 0.01, vc, TAIL_CORE), Mat::TailCore, paint);
    }
    let tail_seen = faces_viewer(pose, &tail_face());
    if !tail_seen {
        wing(&mut out, pose, paint);
    }
    solid(
        &mut out,
        pose,
        &CABIN_LO,
        &CABIN_HI,
        CABIN_Y,
        Mat::Body,
        &[Mat::Glass, Mat::Glass, Mat::Glass, Mat::Glass],
        paint,
    );
    if tail_seen {
        wing(&mut out, pose, paint);
    }
    for (i, &(u, v, st)) in wheels.iter().enumerate() {
        if with_wheels && near[i] {
            wheel_outer(&mut out, pose, u, v, st, paint);
        }
    }
    out
}

/// Draws the car's shapes: a 1 px dark outline under all of them, then the faces and struts.
fn draw_shapes(c: &mut Canvas, sh: &Shapes) {
    let list = &sh.s[..sh.n];
    for s in list.iter().filter(|s| s.n >= 3) {
        for off in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
            fill_convex(c, &s.pts[..s.n], off, 0, OUTLINE);
        }
    }
    for s in list {
        if s.n == 2 {
            let (a, b) = (s.pts[0], s.pts[1]);
            c.line(a.0.floor() as i32, a.1.floor() as i32, b.0.floor() as i32, b.1.floor() as i32, s.col);
        } else {
            fill_convex(c, &s.pts[..s.n], (0.0, 0.0), 0, s.col);
        }
    }
}

/// One puff of smoke: depth ahead of the camera, sideways offset from the road's centre, height, its
/// sideways drift, and a small fixed radius jitter.
#[derive(Clone, Copy, Default)]
struct Puff {
    z: f32,
    d: f32,
    y: f32,
    vd: f32,
    jr: f32,
}

/// Which layers draw - a test hook, so the car and the smoke can be measured on their own.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Only {
    All,
    Car,
    Smoke,
}

pub struct Drift {
    /// Fires the drift on a rare bass hit. `pub(crate)` so the tests can force it.
    pub(crate) flourish: Trigger,
    hit: Envelope,
    /// Finds the kicks.
    onset: Flux,
    elapsed_s: f32,
    /// The motion's eased amplitude (0..1).
    amp: f32,
    /// The camera's distance along the course (wrapped) and its sideways position.
    s_cam: f32,
    cam_x: f32,
    /// The car's slide from the road's centre and its speed, units and units per second.
    lat: f32,
    vlat: f32,
    /// The body's slip angle against the road (degrees, signed), the clutch kick's share of it, and
    /// how hard the road at the car is bending (-1..1).
    slip: f32,
    kick: f32,
    bend: f32,
    /// The exhaust pop's envelope (0..1).
    flame: f32,
    /// The drift's spin (radians) and its direction.
    spin: f32,
    spin_dir: f32,
    /// The ridges' sideways shift, px.
    bg_off: f32,
    /// The road's centre and its height (relative to the ground under the car) at depth `i * DZ`, and
    /// the lowest screen row already covered by nearer road when segment `i` was drawn.
    road: [f32; ROAD_N],
    elev: [f32; ROAD_N],
    seg_clip: [f32; ROAD_N],
    /// The camera's jolt on a kick (0..1).
    bump: f32,
    /// Whether a ground row got road this frame (a row behind a crest does not).
    row_ok: [bool; ROWS],
    /// Per ground row: depth, the road's centre column and half width, px.
    row_z: [f32; ROWS],
    row_cx: [f32; ROWS],
    row_hw: [f32; ROWS],
    ring: [Puff; TRAIL],
    head: usize,
    ring_live: bool,
    layout: Layout,
    layout_dim: (i32, i32),
    /// The baked sky. Keyed on size and colours.
    bg: Canvas,
    bg_key: u64,
    rng: u64,
    only: Only,
}

impl Default for Drift {
    fn default() -> Self {
        Drift {
            flourish: Default::default(),
            hit: Default::default(),
            onset: Default::default(),
            elapsed_s: 0.0,
            amp: 0.0,
            s_cam: 0.0,
            cam_x: 0.0,
            lat: 0.0,
            vlat: 0.0,
            slip: 0.0,
            kick: 0.0,
            bend: 0.0,
            flame: 0.0,
            spin: 0.0,
            spin_dir: 1.0,
            bg_off: 0.0,
            road: [0.0; ROAD_N],
            elev: [0.0; ROAD_N],
            seg_clip: [0.0; ROAD_N],
            bump: 0.0,
            row_ok: [false; ROWS],
            row_z: [0.0; ROWS],
            row_cx: [0.0; ROWS],
            row_hw: [0.0; ROWS],
            ring: [Puff::default(); TRAIL],
            head: 0,
            ring_live: false,
            layout: Layout::default(),
            layout_dim: (0, 0),
            bg: Canvas::new(1, 1),
            bg_key: 0,
            rng: 0xbb67_ae85_84ca_a73b,
            only: Only::All,
        }
    }
}

/// A finite level, clamped 0..1.
fn lvl(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn with_alpha(c: Rgba, a: f32) -> Rgba {
    Rgba { a: (a.clamp(0.0, 1.0) * 255.0).round() as u8, ..c }
}

/// The colourway's neon: its one zone's `lit`, falling back to `lit`.
fn neon_of(t: &Theme) -> Rgba {
    let hex = t.zones.first().map(|z| z.lit.as_str()).unwrap_or(t.lit.as_str());
    Rgba::from_hex(hex, 1.0)
}

/// FNV-1a over the strings/values the baked sky depends on. No allocation.
fn colour_key(t: &Theme, w: i32, h: i32) -> u64 {
    let mut k: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |b: u8| {
        k ^= b as u64;
        k = k.wrapping_mul(0x0100_0000_01b3);
    };
    let neon = t.zones.first().map(|z| z.lit.as_str()).unwrap_or("");
    for s in [t.panel.as_str(), t.lit.as_str(), t.hot.as_str(), t.edge.as_str(), neon] {
        for b in s.bytes() {
            eat(b);
        }
        eat(0);
    }
    for b in w.to_le_bytes().into_iter().chain(h.to_le_bytes()) {
        eat(b);
    }
    k | 1
}

/// Draws `text` in `font3x5` at `scale` with its top-left at (x, y).
fn text(c: &mut Canvas, x: i32, y: i32, s: &str, scale: i32, col: Rgba) {
    let mut cx = x;
    for ch in s.chars() {
        if let Some(rows) = font3x5::glyph(ch) {
            for (dy, row) in rows.iter().enumerate() {
                for dx in 0..3 {
                    if row & (0b100 >> dx) != 0 {
                        c.fill_rect(cx + dx * scale, y + dy as i32 * scale, scale, scale, col);
                    }
                }
            }
        }
        cx += 4 * scale;
    }
}

/// `text` with a 1 px outline all round in `outline`.
fn text_outlined(c: &mut Canvas, x: i32, y: i32, s: &str, scale: i32, col: Rgba, outline: Rgba) {
    for (ox, oy) in [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)] {
        text(c, x + ox, y + oy, s, scale, outline);
    }
    text(c, x, y, s, scale, col);
}

/// Smoothstep.
fn ease(p: f32) -> f32 {
    let p = p.clamp(0.0, 1.0);
    p * p * (3.0 - 2.0 * p)
}

/// An angle in degrees, wrapped to -180..180.
fn wrap_deg(a: f32) -> f32 {
    (a + 180.0).rem_euclid(360.0) - 180.0
}

/// The scene's colours for a colourway.
struct Palette {
    panel: Rgba,
    lit: Rgba,
    hot: Rgba,
    neon: Rgba,
    haze: Rgba,
    road_a: Rgba,
    road_b: Rgba,
    kerb_w: Rgba,
    line: Rgba,
    slope_a: Rgba,
    slope_b: Rgba,
    shoulder: Rgba,
    valley_hi: Rgba,
    valley_lo: Rgba,
    ridge_far: Rgba,
    ridge_near: Rgba,
    ridge_rim: Rgba,
    pine: Rgba,
    rock_a: Rgba,
    rock_b: Rgba,
    rock_line: Rgba,
    rock_rim: Rgba,
    post: Rgba,
    rail: Rgba,
}

impl Palette {
    fn new(t: &Theme) -> Self {
        let panel = Rgba::from_hex(&t.panel, 1.0);
        let lit = Rgba::from_hex(&t.lit, 1.0);
        let hot = Rgba::from_hex(&t.hot, 1.0);
        let edge = Rgba::from_hex(&t.edge, 1.0);
        let neon = neon_of(t);
        // Asphalt is a near-neutral grey with the colourway's tint, so the road reads against the dark
        // slope and the darker valley.
        let road_a = Rgba::lerp_linear(edge, ASPHALT, 0.5);
        Palette {
            panel,
            lit,
            hot,
            neon,
            haze: Rgba::lerp_linear(Rgba::lerp_linear(panel, edge, 0.55), neon, 0.03),
            road_a,
            road_b: Rgba::lerp_linear(road_a, BLACK, 0.2),
            kerb_w: Rgba::lerp_linear(edge, WHITE, 0.6),
            line: Rgba::lerp_linear(edge, lit, 0.7),
            slope_a: Rgba::lerp_linear(panel, BLACK, 0.2),
            slope_b: Rgba::lerp_linear(panel, edge, 0.08),
            shoulder: Rgba::lerp_linear(panel, edge, 0.45),
            valley_hi: Rgba::lerp_linear(Rgba::lerp_linear(panel, edge, 0.12), neon, 0.012),
            valley_lo: Rgba::lerp_linear(panel, BLACK, 0.45),
            ridge_far: Rgba::lerp_linear(panel, edge, 0.45),
            ridge_near: Rgba::lerp_linear(panel, BLACK, 0.45),
            ridge_rim: Rgba::lerp_linear(panel, edge, 0.8),
            pine: Rgba::lerp_linear(panel, BLACK, 0.6),
            // Cut rock: a cold grey taking the colourway's tint, two bands, darker strata, a rim
            // catching the sky along the top.
            rock_a: Rgba::lerp_linear(Rgba::lerp_linear(panel, edge, 0.6), ASPHALT, 0.3),
            rock_b: Rgba::lerp_linear(Rgba::lerp_linear(panel, edge, 0.3), ASPHALT, 0.15),
            rock_line: Rgba::lerp_linear(panel, BLACK, 0.4),
            rock_rim: Rgba::lerp_linear(edge, lit, 0.35),
            post: Rgba::lerp_linear(edge, WHITE, 0.45),
            rail: Rgba::lerp_linear(edge, WHITE, 0.3),
        }
    }
}

impl Drift {
    #[cfg(test)]
    fn only_for_test(&mut self, only: Only) {
        self.only = only;
    }

    fn next_rng(&mut self) -> u64 {
        splitmix(&mut self.rng)
    }

    /// The road's centre at depth `z`, in the camera's frame (linear in the table).
    fn road_at(&self, z: f32) -> f32 {
        let f = (z / DZ).clamp(0.0, (ROAD_N - 1) as f32);
        let i = (f as usize).min(ROAD_N - 2);
        let t = f - i as f32;
        self.road[i] + (self.road[i + 1] - self.road[i]) * t
    }

    /// The road's height at depth `z` (linear in the table).
    fn elev_at(&self, z: f32) -> f32 {
        let f = (z / DZ).clamp(0.0, (ROAD_N - 1) as f32);
        let i = (f as usize).min(ROAD_N - 2);
        let t = f - i as f32;
        self.elev[i] + (self.elev[i + 1] - self.elev[i]) * t
    }

    /// Whether a point on the ground at depth `z`, at screen row `y`, is in view (not behind a crest).
    fn ground_seen(&self, z: f32, y: f32) -> bool {
        let i = ((z / DZ) as usize).min(ROAD_N - 1);
        y <= self.seg_clip[i] + 1.0
    }

    fn cam(&self, w: i32) -> Cam {
        // A kick jolts the camera down a touch, as if the car squatted on the throttle.
        Cam { cx: w as f32 * 0.5, hz: self.layout.hz, f: self.layout.f, x: self.cam_x, y: CAM_H - BUMP * self.bump }
    }

    /// The car's pose this frame.
    fn car_pose(&self, cam: Cam) -> Pose {
        let x = self.road_at(CAR_Z) + self.lat;
        let heading = (self.road_at(CAR_Z + DZ) - self.road_at(CAR_Z - DZ)) / (2.0 * DZ);
        let yaw = heading.atan() + self.slip.to_radians() + self.spin;
        Pose::new(x, CAR_Z, yaw, cam)
    }

    /// Rebuilds the layout on a size change and the baked sky on a size or colour change.
    fn resize(&mut self, w: i32, h: i32, t: &Theme) {
        if self.layout_dim != (w, h) {
            self.layout = layout(w, h);
            self.layout_dim = (w, h);
            self.ring_live = false;
        }
        let key = colour_key(t, w, h);
        if key == self.bg_key {
            return;
        }
        self.bg_key = key;
        if self.bg.width() != w || self.bg.height() != h {
            self.bg = Canvas::new(w, h);
        }
        let l = &self.layout;
        let bg = &mut self.bg;
        bg.clear();
        let p = Palette::new(t);
        let edge = Rgba::from_hex(&t.edge, 1.0);
        let (ix0, iy0, ix1) = (3, 4, w - 3);
        let iw = ix1 - ix0;
        let sky_h = (l.hz_row - iy0).max(1) as f32;
        // The sky: panel at the top, toward edge and a wash of neon at the horizon.
        for y in iy0..l.hz_row {
            let f = (y - iy0) as f32 / sky_h;
            let col = Rgba::lerp_linear(p.panel, edge, 0.85 * f.powf(1.6));
            bg.fill_rect(ix0, y, iw, 1, Rgba::lerp_linear(col, p.neon, 0.22 * f.powi(3)));
        }
        // Stars, thinning toward the glow.
        let mut s: u64 = 0x0f0f_1a2b_3c4d_5e6f;
        let n = (iw * (l.hz_row - iy0) / 90).max(4);
        for _ in 0..n {
            let x = ix0 + (splitmix(&mut s) % iw as u64) as i32;
            let fy = unit(&mut s);
            let y = iy0 + (fy * fy * sky_h * 0.8) as i32;
            let a = 0.25 + 0.5 * unit(&mut s);
            bg.fill_rect(x, y, 1, 1, with_alpha(Rgba::lerp_linear(p.lit, WHITE, 0.6), a));
        }
        // The gauge's ring.
        if l.show_gauge {
            let (gx, gcy) = l.gauge;
            let ring = Rgba::lerp_linear(p.panel, p.lit, 0.45);
            let r = GAUGE_R as f32;
            for y in gcy - GAUGE_R - 1..=gcy {
                for x in gx - GAUGE_R - 1..=gx + GAUGE_R + 1 {
                    let d = ((x - gx) as f32).hypot((y - gcy) as f32);
                    if (d - r).abs() <= 0.5 {
                        bg.fill_rect(x, y, 1, 1, ring);
                    }
                }
            }
        }
    }

    /// Advances the course, the camera and the car; returns the speed, units per second.
    fn advance(&mut self, dt: f32, rms_norm: f32, kick: bool, drifting: bool, since_ms: f32, boost: f32) -> f32 {
        let k = (dt / AMP_EASE_MS).min(1.0);
        self.amp += (rms_norm - self.amp) * k;
        if !self.amp.is_finite() {
            self.amp = 0.0;
        }
        let amp = self.amp.clamp(0.0, 1.0);
        let v = V_BASE + V_RMS * rms_norm.max(boost);
        let dts = dt / 1000.0;
        let c_cam = curve(self.s_cam);
        self.s_cam = (self.s_cam + v * dts).rem_euclid(COURSE);
        self.bg_off = (self.bg_off + c_cam * v * dts * self.layout.f).rem_euclid(RIDGE_W as f32 * 2.0);
        if !self.s_cam.is_finite() || !self.bg_off.is_finite() {
            self.s_cam = 0.0;
            self.bg_off = 0.0;
        }

        // The road ahead: curvature integrated twice from the camera, then turned so it runs straight
        // away at the car (most of the way - `ROT`).
        let (mut x, mut dx) = (0.0f32, 0.0f32);
        let mut heading = [0.0f32; ROAD_N];
        for i in 0..ROAD_N {
            self.road[i] = x;
            heading[i] = dx;
            let z = i as f32 * DZ;
            dx += curve(self.s_cam + z) * DZ;
            x += dx * DZ;
        }
        let at_car = heading[(CAR_Z / DZ) as usize];
        let ground = hill(self.s_cam + CAR_Z);
        for i in 0..ROAD_N {
            self.road[i] -= ROT * at_car * i as f32 * DZ;
            self.elev[i] = hill(self.s_cam + i as f32 * DZ) - ground;
        }

        // The drift: into the bend, sliding out of it; a clutch kick on the beat; full lock and a
        // spin in the flourish.
        self.bend = (2.5 * curve(self.s_cam + CAR_Z) / C_MAX).tanh();
        if kick && amp > KICK_AMP && !drifting {
            self.kick = KICK_SLIP;
            self.flame = 1.0;
            self.bump = 1.0;
        }
        self.bump *= (-dt / BUMP_MS).exp();
        self.kick *= (-dt / KICK_DECAY_MS).exp();
        self.flame = (self.flame - dt / FLAME_MS).max(0.0);
        let dir = if self.bend >= 0.0 { 1.0 } else { -1.0 };
        let slip_to = if drifting {
            self.spin_dir * SLIP_MAX
        } else {
            SLIP_MAX * amp * self.bend + dir * self.kick * amp
        };
        self.slip += (slip_to - self.slip) * (1.0 - (-dt / FLICK_MS).exp());
        let lat_to = -self.bend * LAT_SLIDE * amp;
        let steps = ((dt / 8.0).ceil() as i32).clamp(1, 25);
        let h = dts / steps as f32;
        for _ in 0..steps {
            let a = LAT_OMEGA * LAT_OMEGA * (lat_to - self.lat) - 2.0 * LAT_OMEGA * self.vlat;
            self.vlat += a * h;
            self.lat += self.vlat * h;
        }
        self.spin = if drifting && since_ms < SPIN_MS { self.spin_dir * TAU * ease(since_ms / SPIN_MS) } else { 0.0 };
        let car_x = self.road_at(CAR_Z) + self.lat;
        self.cam_x += (CAM_FOLLOW * car_x - self.cam_x) * (dt / 200.0).min(1.0);
        for v in [&mut self.slip, &mut self.lat, &mut self.vlat, &mut self.cam_x, &mut self.kick, &mut self.bump] {
            if !v.is_finite() {
                *v = 0.0;
            }
        }
        v
    }
}

impl Family for Drift {
    fn id(&self) -> &'static str {
        "drift"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());

        // ---- the opaque panel ----
        let panel = Rgba::from_hex(&t.panel, 1.0);
        c.rounded_rect(1, 2, w - 2, h - 4, 3, panel);
        let (ix0, iy0, ix1, iy1) = (3i32, 4i32, w - 3, h - 4);
        let (iw, ih) = (ix1 - ix0, iy1 - iy0);
        if iw < 40 || ih < 20 {
            c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);
            return;
        }
        self.resize(w, h, t);

        let dt = if d.dt_ms.is_finite() { d.dt_ms.clamp(0.0, 200.0) } else { 16.7 };
        self.elapsed_s = (self.elapsed_s + dt / 1000.0).rem_euclid(3600.0);
        if !self.elapsed_s.is_finite() {
            self.elapsed_s = 0.0;
        }
        let e = self.elapsed_s;
        let rms = lvl((d.rms_l + d.rms_r) * 0.5);
        // rms rarely passes ~0.25 in practice (the `term` convention).
        let rms_norm = (rms * 4.0).clamp(0.0, 1.0);
        let bass = d.levels[0..8].iter().map(|&v| lvl(v)).sum::<f32>() / 8.0;
        let mids = d.levels[16..40].iter().map(|&v| lvl(v)).sum::<f32>() / 24.0;
        let treble = d.levels[40..64].iter().map(|&v| lvl(v)).sum::<f32>() / 24.0;

        // ---- the drift (the flourish) ----
        let triggered = self.flourish.update(&d.levels, dt, t.flourish);
        let bass_gate = triggered && bass >= FLOURISH_BASS_MIN;
        #[cfg(test)]
        let fired = bass_gate || (triggered && self.flourish.was_forced());
        #[cfg(not(test))]
        let fired = bass_gate;
        let env = self.hit.update(fired, dt, DRIFT_MS);
        let since_ms = (1.0 - env) * DRIFT_MS;
        if fired {
            self.spin_dir = if self.slip >= 0.0 { 1.0 } else { -1.0 };
        }
        let drifting = env > 0.0;
        let boost = if drifting { (env / 0.35).min(1.0) } else { 0.0 };

        // ---- the course, the camera, the car ----
        let kick = self.onset.update(&d.levels, dt, ONSET_RATIO, ONSET_REFRACTORY_MS) && bass >= KICK_BASS;
        let v = self.advance(dt, rms_norm, kick, drifting, since_ms, boost);
        let cam = self.cam(w);
        let pose = self.car_pose(cam);
        let (s, f, hz_row) = (self.layout.s, self.layout.f, self.layout.hz_row);

        // ---- the smoke ring: puffs drag, rise and spread; a new one at a rear tyre ----
        let dts = dt / 1000.0;
        if !self.ring_live {
            self.ring = [Puff { z: -1.0, ..Puff::default() }; TRAIL];
            self.head = 0;
            self.ring_live = true;
        }
        let out = if self.slip >= 0.0 { -1.0 } else { 1.0 };
        for p in self.ring.iter_mut() {
            p.z -= v * (1.0 - DRAG) * dts;
            p.y += RISE * dts;
            p.d += p.vd * dts;
        }
        self.head = (self.head + 1) % TRAIL;
        let side = if self.head.is_multiple_of(2) { TRACK } else { -TRACK };
        let (px, pz) = pose.at(AXLE_R - WHEEL.0, side);
        let rnd = self.next_rng();
        let road_px = self.road_at(pz);
        self.ring[self.head] = Puff {
            z: pz,
            d: px - road_px,
            y: 0.6,
            vd: out * SPREAD * (0.4 + 0.6 * self.slip.abs() / SLIP_MAX) + ((rnd % 7) as f32 - 3.0) * 0.4,
            jr: ((rnd >> 8) % 5) as f32 * 0.08 - 0.16,
        };

        // ---- per ground row: depth, the road's centre and half width ----
        // Road segments from near to far: each fills the rows between its screen row and the nearer
        // road above which nothing has been drawn yet, so a crest hides the road beyond it until the
        // road climbs back into view (the classic pseudo-3D hill clip).
        let rows_end = (iy1.max(0) as usize).min(ROWS);
        for ok in self.row_ok[..rows_end].iter_mut() {
            *ok = false;
        }
        let mut clip = iy1 as f32;
        let (mut prev_y, mut prev_z) = (f32::MAX, 0.0f32);
        for i in 1..ROAD_N {
            let z = i as f32 * DZ;
            self.seg_clip[i] = clip;
            if z < 8.0 {
                continue;
            }
            let y = cam.project(0.0, self.elev[i], z).1;
            if y < clip {
                let top = ((y - 0.5).ceil() as i32).max(iy0);
                let bot = ((clip.min(prev_y) - 0.5).ceil() as i32 - 1).min(iy1 - 1);
                for r in top..=bot {
                    let ri = r as usize;
                    if ri >= rows_end {
                        continue;
                    }
                    let tt = if prev_y < f32::MAX { ((r as f32 + 0.5 - y) / (prev_y - y)).clamp(0.0, 1.0) } else { 0.0 };
                    let zr = z + (prev_z - z) * tt;
                    self.row_z[ri] = zr;
                    self.row_cx[ri] = cam.cx + (self.road_at(zr) - cam.x) * f / zr;
                    self.row_hw[ri] = HALF_W * f / zr;
                    self.row_ok[ri] = true;
                }
                clip = y;
            }
            prev_y = y;
            prev_z = z;
        }

        let pal = Palette::new(t);
        let only = self.only;
        let flash = drifting && since_ms < FLASH_MS;
        let flash_on = flash && (since_ms / 1000.0 * FLASH_HZ).fract() < 0.5;

        if only == Only::All {
            // ---- the sky ----
            c.copy_region(&self.bg, (ix0, iy0), (ix0, iy0), iw, hz_row - iy0);

            // ---- the ridges, shifting as the car corners ----
            let l = &self.layout;
            for x in ix0..ix1 {
                let i_far = ((x as f32 + self.bg_off * 0.5) as i64).rem_euclid(RIDGE_W as i64) as usize;
                let i_near = ((x as f32 + self.bg_off) as i64).rem_euclid(RIDGE_W as i64) as usize;
                let hf = l.ridge_far[i_far] as i32;
                c.fill_rect(x, hz_row - hf, 1, hf, pal.ridge_far);
                let hn = l.ridge_near[i_near] as i32;
                if hn > 0 {
                    c.fill_rect(x, hz_row - hn, 1, hn, pal.ridge_near);
                    c.fill_rect(x, hz_row - hn, 1, 1, pal.ridge_rim);
                }
            }

            // ---- the ground, row by row ----
            for y in iy0..iy1.min(ROWS as i32) {
                let yi = y as usize;
                if !self.row_ok[yi] {
                    // Beyond a crest, below the horizon: the far side of the pass in the haze.
                    if y >= hz_row {
                        c.fill_rect(ix0, y, iw, 1, Rgba::lerp_linear(pal.haze, pal.valley_lo, 0.35));
                    }
                    continue;
                }
                let (z, cx, hw) = (self.row_z[yi], self.row_cx[yi], self.row_hw[yi]);
                let fog = (z / FOG_Z).clamp(0.0, 1.0).powf(1.3);
                let fogged = |col: Rgba| Rgba::lerp_linear(col, pal.haze, fog);
                let dist = self.s_cam + z;
                let band = ((dist / SEG).floor() as i64) & 1 == 1;
                let kerb = KERB_W * f / z;
                let shoulder = 2.0 * f / z;
                let (l0, r0) = (cx - hw, cx + hw);
                let valley_f = ((y - hz_row) as f32 / (iy1 - hz_row).max(1) as f32).clamp(0.0, 1.0);
                // The mountainside, the kerbs, the asphalt, the shoulder and the valley.
                span(c, ix0 as f32, l0 - kerb, y, fogged(if band { pal.slope_a } else { pal.slope_b }));
                let kerb_col = fogged(if band { pal.kerb_w } else { KERB_RED });
                span(c, l0 - kerb, l0, y, kerb_col);
                span(c, l0, r0, y, fogged(if band { pal.road_a } else { pal.road_b }));
                span(c, r0, r0 + kerb, y, kerb_col);
                span(c, r0 + kerb, r0 + kerb + shoulder, y, fogged(pal.shoulder));
                // The drop: black right at the edge, the far valley's glow further out.
                let valley = Rgba::lerp_linear(pal.valley_hi, pal.valley_lo, valley_f.sqrt());
                let lip = r0 + kerb + shoulder;
                span(c, lip, lip + 3.0 * f / z, y, pal.valley_lo);
                span(c, lip + 3.0 * f / z, ix1 as f32, y, valley);
                if ((dist / DASH).floor() as i64) & 1 == 0 {
                    let lw = (LINE_W * f / z).max(0.6);
                    span(c, cx - lw * 0.5, cx + lw * 0.5, y, fogged(pal.line));
                }
            }

            // ---- the city in the valley ----
            for li in &l.lights[..l.lights_n] {
                let yi = li.row;
                if yi < hz_row || yi >= iy1 || yi as usize >= ROWS {
                    continue;
                }
                let x = ix0 as f32 + (li.vx - self.bg_off * 0.6).rem_euclid(2.0 * RIDGE_W as f32);
                let edge_x = if self.row_ok[yi as usize] {
                    self.row_cx[yi as usize] + self.row_hw[yi as usize] * 1.25 + 2.0
                } else {
                    w as f32 * 0.5
                };
                if x >= ix1 as f32 || x < edge_x || !(flash || light_on(li, e)) {
                    continue;
                }
                let base = match li.kind {
                    0 => pal.neon,
                    1 => WARM,
                    _ => pal.hot,
                };
                let depth = 1.0 - (yi - hz_row) as f32 / (iy1 - hz_row).max(1) as f32;
                let col = if flash_on { WHITE } else { base };
                // The city swells with the mids: brighter, and the bright ones bloom to 2 px.
                let a = (0.62 + 0.3 * (1.0 - depth * 0.6) + 0.4 * mids).min(1.0);
                let wd = if mids > 0.45 && li.kind != 1 { 2 } else { 1 };
                c.fill_rect(x as i32, yi, wd, 1, with_alpha(col, a));
            }

            // ---- pines along the cliff top, far to near (the cliff, drawn next, covers the far ones) ----
            let wall_d = HALF_W + KERB_W + CLIFF_GAP;
            let far_slot = ((self.s_cam + Z_FAR) / TREE_GAP).floor() as i64;
            for k in (0..60).map(|j| far_slot - j) {
                let z = k as f32 * TREE_GAP - self.s_cam;
                if z < 10.0 {
                    break;
                }
                let x = self.road_at(z) - (wall_d + 2.0 + hash01(k) * 14.0);
                let th = 7.0 + hash01(k * 7 + 3) * 7.0;
                let g = self.elev_at(z) + cliff_h(self.s_cam + z) - 0.5;
                let fog = (z / FOG_Z).clamp(0.0, 1.0).powf(1.3);
                let col = Rgba::lerp_linear(pal.pine, pal.haze, fog);
                let apex = cam.project(x, g + th, z);
                let bl = cam.project(x - th * 0.32, g + 1.5, z);
                let br = cam.project(x + th * 0.32, g + 1.5, z);
                fill_convex(c, &[apex, br, bl], (0.0, 0.0), 0, col);
                let (t0, t1) = (cam.project(x, g + 1.6, z), cam.project(x, g, z));
                let tw = (0.6 * f / z).max(1.0);
                c.fill_rect(
                    (t0.0 - tw * 0.5).round() as i32,
                    t0.1.round() as i32,
                    tw.round() as i32,
                    (t1.1 - t0.1).round().max(1.0) as i32,
                    col,
                );
            }

            // ---- the cliff: a rock face beside the road, far to near ----
            // Each stretch between two road samples is a quad from the gutter up to the cliff's top,
            // kept to the rows above the nearer road (`seg_clip`), so behind a crest only the top shows.
            // Each road sample's slot on the course, so a stretch keeps its shade, cracks and ragged
            // top as it scrolls past.
            let slots = (COURSE / DZ) as i64;
            let slot = |z: f32| (((self.s_cam + z) / DZ).floor() as i64).rem_euclid(slots);
            let mut prev: Option<(Pt, Pt, i64)> = None;
            for i in (1..ROAD_N).rev() {
                let z = i as f32 * DZ;
                if z < 4.0 {
                    break;
                }
                let k = slot(z);
                let x = self.road[i] - wall_d;
                let ht = cliff_h(self.s_cam + z) + (hash01(k * 13 + 5) - 0.5) * 1.8;
                let (b, tp) = (cam.project(x, self.elev[i], z), cam.project(x, self.elev[i] + ht, z));
                if let Some((pb, pt, pk)) = prev {
                    let fog = (z / FOG_Z).clamp(0.0, 1.0).powf(1.3);
                    let shade = hash01(k * 7 + 1);
                    let rock = Rgba::lerp_linear(Rgba::lerp_linear(pal.rock_b, pal.rock_a, shade), pal.haze, fog);
                    let max_row = self.seg_clip[i].ceil() as i32;
                    fill_convex_rows(c, &[pb, pt, tp, b], (0.0, 0.0), iy0, max_row, rock);
                    let line = Rgba::lerp_linear(pal.rock_line, pal.haze, fog);
                    let near = (pb.0 - b.0).abs() < 64.0;
                    // Ragged strata: each sample nudges every line up or down a little.
                    for (j, h) in STRATA.into_iter().enumerate() {
                        let jit = |kk: i64| h + (hash01(kk * 17 + j as i64) - 0.5) * 0.035;
                        let q0 = (pb.0, pb.1 + (pt.1 - pb.1) * jit(pk));
                        let q1 = (b.0, b.1 + (tp.1 - b.1) * jit(k));
                        if near && q1.1 < max_row as f32 {
                            c.line(q0.0.round() as i32, q0.1.round() as i32, q1.0.round() as i32, q1.1.round() as i32, line);
                        }
                    }
                    // A crack down from the top every few stretches.
                    if near && hash01(k * 29 + 11) < 0.22 {
                        let depth = 0.35 + 0.4 * hash01(k * 3 + 2);
                        let q = (b.0, tp.1 + (b.1 - tp.1) * depth);
                        if tp.1 < max_row as f32 {
                            let qy = q.1.min(max_row as f32 - 1.0);
                            c.line(tp.0.round() as i32, tp.1.round() as i32 + 1, q.0.round() as i32, qy.round() as i32, line);
                        }
                    }
                    if near && tp.1 < max_row as f32 {
                        let rim = Rgba::lerp_linear(pal.rock_rim, pal.haze, fog);
                        c.line(pt.0.round() as i32, pt.1.round() as i32, tp.0.round() as i32, tp.1.round() as i32, rim);
                    }
                }
                prev = Some((b, tp, k));
            }

            // ---- reflector posts and the guardrail, far to near ----
            let far_slot = ((self.s_cam + Z_FAR * 0.8) / POST_GAP).floor() as i64;
            let mut prev_rail: Option<(f32, f32)> = None;
            for k in (0..60).map(|j| far_slot - j) {
                let z = k as f32 * POST_GAP - self.s_cam;
                if z < 8.0 {
                    break;
                }
                let fog = (z / FOG_Z).clamp(0.0, 1.0).powf(1.3);
                let pw = (0.35 * f / z).max(1.0);
                let g = self.elev_at(z);
                if !self.ground_seen(z, cam.project(self.road_at(z), g, z).1) {
                    prev_rail = None;
                    continue;
                }
                for (side, refl) in [(-1.0f32, pal.neon), (1.0, pal.hot)] {
                    let x = self.road_at(z) + side * (HALF_W + KERB_W + 1.0);
                    let (b, tp) = (cam.project(x, g, z), cam.project(x, g + POST_H, z));
                    let col = Rgba::lerp_linear(pal.post, pal.haze, fog);
                    let (px0, pwi) = ((b.0 - pw * 0.5).round() as i32, pw.round() as i32);
                    c.fill_rect(px0, tp.1.round() as i32, pwi, (b.1 - tp.1).round().max(1.0) as i32, col);
                    // The reflectors glint with the treble.
                    let rc = if flash_on {
                        WHITE
                    } else {
                        Rgba::lerp_linear(Rgba::lerp_linear(refl, WHITE, 0.7 * treble), pal.haze, fog * 0.6)
                    };
                    c.fill_rect(px0, tp.1.round() as i32, pwi, (0.5 * f / z).round().max(1.0) as i32, rc);
                    if side > 0.0 {
                        let rail = cam.project(x, g + RAIL_H, z);
                        if let Some(q) = prev_rail {
                            let rcol = Rgba::lerp_linear(pal.rail, pal.haze, fog);
                            let (qx, qy, rx, ry) = (q.0.round() as i32, q.1.round() as i32, rail.0.round() as i32, rail.1.round() as i32);
                            c.line(qx, qy, rx, ry, rcol);
                            if z < 70.0 {
                                c.line(qx, qy + 1, rx, ry + 1, rcol);
                            }
                        }
                        prev_rail = Some(rail);
                    }
                }
            }

            // ---- the headlights' beam ahead, and the neon underglow ----
            let beam = Rgba::lerp_linear(pal.lit, WHITE, 0.6);
            let nose = BODY[0].0;
            for (len, spread, a) in [(70.0, 16.0, 0.035), (45.0, 10.0, 0.04), (22.0, 6.0, 0.05)] {
                let pts = [
                    pose.screen((nose, -3.0, 0.0)),
                    pose.screen((nose, 3.0, 0.0)),
                    pose.screen((nose + len, spread, 0.0)),
                    pose.screen((nose + len, -spread, 0.0)),
                ];
                fill_convex(c, &pts, (0.0, 0.0), hz_row, with_alpha(beam, a));
            }
            for (ku, kv, a) in [(1.3, 2.0, 0.12), (1.0, 1.45, 0.25)] {
                let mut glow = [(0.0f32, 0.0f32); 8];
                for (i, q) in BODY.iter().enumerate() {
                    glow[i] = pose.screen((q.0 * ku, q.1 * kv, 0.0));
                }
                fill_convex(c, &glow, (0.0, 0.0), hz_row, with_alpha(pal.neon, a * (0.3 + 1.9 * bass)));
            }
        }

        // ---- the car ----
        if only != Only::Smoke {
            let paint = Paint::new(t);
            let steer = (-self.slip).clamp(-COUNTER_MAX, COUNTER_MAX).to_radians();
            let shapes = car_shapes(&pose, steer, &paint);
            draw_shapes(c, &shapes);
            if only == Only::All && faces_viewer(&pose, &tail_face()) {
                // The four round tail lamps' glow and, on a kick, the exhaust's pop.
                // A kick flares them like a dab of the brakes.
                let flare = (self.kick / KICK_SLIP).clamp(0.0, 1.0);
                let r = ((1.3 + 1.2 * flare) * f / CAR_Z).max(1.0).round() as i32;
                for vc in tail_lamps() {
                    let (gx, gy) = pose.screen((BODY[4].0 - 0.5, vc, TAIL_Y));
                    c.fill_circle(gx.round() as i32, gy.round() as i32, r, with_alpha(pal.hot, 0.2 + 0.35 * flare));
                }
                if self.flame > 0.0 {
                    let (fx, fy) = pose.screen(EXHAUST);
                    let fr = (self.flame * 1.6 * f / CAR_Z).max(1.0);
                    c.fill_circle(fx.round() as i32, fy.round() as i32, fr.round() as i32, with_alpha(FLAME, 0.85));
                    c.fill_circle(fx.round() as i32, fy.round() as i32, (fr * 0.5).round() as i32, with_alpha(WARM, 0.95));
                }
            }
        }

        // ---- the tyre smoke: the meter, rolling back toward the camera ----
        if only != Only::Car {
            let tint = Rgba::lerp_linear(WHITE, pal.hot, 0.7 * boost);
            let ghost = if t.ghost.is_finite() { t.ghost.clamp(0.05, 0.6) } else { 0.3 };
            let r_max_px = PUFF_MAX_PX * s;
            // Newest (at the tyres, farthest) first, so the older puffs nearer the camera sit on top.
            for i in 0..TRAIL {
                let p = self.ring[(self.head + TRAIL - i) % TRAIL];
                if p.z < 6.0 {
                    continue;
                }
                let age = i as f32 / (TRAIL - 1) as f32;
                let lv = lvl(d.levels[i]) + (1.0 - lvl(d.levels[i])) * boost;
                let rw = (PUFF_R0 + lv * PUFF_GAIN) * (1.0 + PUFF_GROW * age) + p.jr;
                let x = self.road_at(p.z) + p.d;
                // The smoke follows the slope only partly: the road behind the car is off the panel, so a puff
                // riding a full downhill rise would look like a plume shooting into the sky.
                let (sx, sy) = cam.project(x, SMOKE_SLOPE * self.elev_at(p.z) + p.y + rw * 0.6, p.z);
                let r = (rw * f / p.z).min(r_max_px);
                if r < 0.5 || sx + r < ix0 as f32 || sx - r >= ix1 as f32 {
                    continue;
                }
                // Thinner with age and as it nears the lens.
                let near = ((p.z - 6.0) / 14.0).clamp(0.0, 1.0);
                let a = ghost * (1.0 - 0.7 * age) * near;
                // One soft disc and a denser core at 60 % of its radius. Two near-full discs per puff
                // were three quarters of the frame (alpha blends are the expensive pixels) and pushed
                // the CI runner past the 2 ms gate.
                let (xi, yi, ri) = (sx.round() as i32, sy.round() as i32, r.round().max(1.0) as i32);
                c.fill_circle(xi, yi, ri, with_alpha(tint, a * 0.26));
                let core = (r * 0.6).round() as i32;
                if core >= 1 {
                    c.fill_circle(xi, yi, core, with_alpha(tint, a * 0.30));
                }
            }
        }

        if only == Only::All {
            let l = &self.layout;
            // ---- the steering gauge ----
            if l.show_gauge {
                let angle = wrap_deg(self.slip + self.spin.to_degrees());
                let (gx, gcy) = l.gauge;
                let th = (90.0 - angle.clamp(-45.0, 45.0) * 2.0).to_radians();
                let reach = (GAUGE_R - 1) as f32;
                let (nx, ny) = (gx as f32 + reach * th.cos(), gcy as f32 - reach * th.sin());
                c.line(gx, gcy, nx.round() as i32, ny.round() as i32, pal.hot);
                c.fill_rect(gx, gcy, 1, 1, pal.lit);
                let a = angle.abs().round().clamp(0.0, 99.0) as u32;
                let digits = [b'0' + (a / 10) as u8, b'0' + (a % 10) as u8];
                let ds = std::str::from_utf8(&digits).unwrap_or("00");
                let (tx, ty) = l.angle_text;
                text(c, tx, ty, "ANGLE", 1, Rgba::lerp_linear(panel, pal.lit, 0.75));
                text(c, tx + text_w("ANGLE ", 1) + 1, ty, ds, 1, pal.lit);
            }

            // ---- DRIFT! ----
            let tin = since_ms - TEXT_FROM_MS;
            if drifting && (0.0..TEXT_MS).contains(&tin) {
                let (tx, ty) = l.drift_text;
                let big_w = text_w("DRIFT!", 3);
                if tin < 20.0 && big_w + 4 <= iw && 15 + 2 <= hz_row - iy0 {
                    // The slam: one frame at 3x.
                    let bx = ix0 + (iw - big_w) / 2;
                    let by = (ty - 2).max(iy0 + 1);
                    text_outlined(c, bx, by, "DRIFT!", 3, pal.hot, OUTLINE);
                } else {
                    let shake = if tin < 100.0 { if (tin / 33.0) as i32 % 2 == 0 { 1 } else { -1 } } else { 0 };
                    text_outlined(c, tx + shake, ty, "DRIFT!", 2, pal.hot, OUTLINE);
                }
            }
        }

        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::canvas::Canvas;

    const IDS: [&str; 4] = ["drift-shibuya", "drift-touge", "drift-orange", "drift-night"];

    fn theme(id: &str) -> Theme {
        crate::themes::builtin::all().into_iter().find(|t| t.id == id).unwrap()
    }
    /// The colourway with its audio flourish off, so a motion test is not interrupted by a drift.
    fn calm_theme(id: &str) -> Theme {
        Theme { flourish: 0.0, ..theme(id) }
    }
    fn frames(fam: &mut Drift, t: &Theme, w: i32, h: i32, level: f32, n: usize) -> Canvas {
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
            c.clear();
            fam.draw(&mut c, t, &d);
        }
        c
    }
    /// `n` frames with only `bands` at `level`, the rest silent, and the given rms.
    #[allow(clippy::too_many_arguments)]
    fn frames_bands(
        fam: &mut Drift,
        t: &Theme,
        w: i32,
        h: i32,
        bands: std::ops::Range<usize>,
        level: f32,
        rms: f32,
        n: usize,
    ) -> Canvas {
        let mut c = Canvas::new(w, h);
        let mut d = FrameData::default();
        for v in d.levels[bands].iter_mut() {
            *v = level;
        }
        d.peaks = d.levels;
        d.rms_l = rms;
        d.rms_r = rms;
        d.dt_ms = 16.7;
        for k in 0..n {
            d.time_s = k as f32 * 0.0167;
            c.clear();
            fam.draw(&mut c, t, &d);
        }
        c
    }
    /// A music-like frame `k`: bass-heavy and wobbling, with a kick every `beat` frames held for three
    /// (0 = no kicks).
    fn music(level: f32, rms: f32, k: usize, beat: usize) -> FrameData {
        let t_s = k as f32 * 0.0167;
        let mut d = FrameData { dt_ms: 16.7, time_s: t_s, ..FrameData::default() };
        let kick = beat > 0 && k % beat < 3;
        for (i, v) in d.levels.iter_mut().enumerate() {
            let f = i as f32 / NUM_BANDS as f32;
            let shape = (1.0 - f).powf(1.2) * 0.7 + 0.12;
            let wob = 1.0 + 0.3 * (t_s * 2.4 + f * 6.0).sin();
            let kk = if kick { 1.0 + 0.9 * (1.0 - f) } else { 1.0 };
            *v = (shape * wob * kk * level).clamp(0.0, 1.0);
        }
        d.peaks = d.levels;
        d.rms_l = rms;
        d.rms_r = rms;
        d
    }
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
    /// The painted box (x0, y0, x1, y1, inclusive) of whatever the hook left drawn, inside the panel.
    fn bbox(c: &Canvas, t: &Theme) -> Option<(i32, i32, i32, i32)> {
        let panel = Rgba::from_hex(&t.panel, 1.0);
        let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for y in 4..c.height() - 4 {
            for x in 3..c.width() - 3 {
                if drew_over_panel(c.get(x, y), panel) {
                    x0 = x0.min(x);
                    x1 = x1.max(x);
                    y0 = y0.min(y);
                    y1 = y1.max(y);
                }
            }
        }
        (x0 <= x1).then_some((x0, y0, x1, y1))
    }
    /// The mean row of the painted pixels, and how many there are.
    fn centroid_y(c: &Canvas, t: &Theme) -> (f32, usize) {
        let panel = Rgba::from_hex(&t.panel, 1.0);
        let (mut sum, mut n) = (0.0f32, 0usize);
        for y in 4..c.height() - 4 {
            for x in 3..c.width() - 3 {
                if drew_over_panel(c.get(x, y), panel) {
                    sum += y as f32;
                    n += 1;
                }
            }
        }
        (sum / n.max(1) as f32, n)
    }
    /// A behind-the-car camera for the model tests: the car at `CAR_Z`, straight ahead.
    fn test_cam() -> Cam {
        Cam { cx: 60.0, hz: 20.0, f: FOCAL, x: 0.0, y: CAM_H }
    }

    #[test]
    fn registered_and_labelled() {
        assert!(crate::render::KNOWN_FAMILIES.contains(&"drift"));
        assert_eq!(crate::render::family_for("drift").id(), "drift");
        assert_eq!(crate::themes::family_label("drift"), "Fast & Furious: Tokyo Drift");
        let themes: Vec<Theme> = crate::themes::builtin::all().into_iter().filter(|t| t.family == "drift").collect();
        let ids: Vec<String> = themes.iter().map(|t| t.id.clone()).collect();
        assert_eq!(ids, IDS.map(String::from).to_vec());
        // Each colourway registers exactly one zone: its neon.
        let neon = ["#ff4fd8", "#39ff8a", "#ff7a1a", "#39c0ff"];
        for (t, n) in themes.iter().zip(neon) {
            assert_eq!(t.zones.len(), 1, "{}", t.id);
            assert_eq!(t.zones[0].lit, n, "{}", t.id);
            assert_eq!(t.zones[0].lit, t.zones[0].hot, "{}", t.id);
        }
    }

    /// Silence still shows the touge: sky, ridges, road, roadside, the car.
    #[test]
    fn rest_frame_is_not_empty() {
        for id in IDS {
            let t = theme(id);
            for (w, h) in [(380, 60), (190, 48), (128, 44)] {
                let c = frames(&mut Drift::default(), &t, w, h, 0.0, 10);
                let n = lit(&c, &t);
                assert!(n as f32 >= 0.10 * (w * h) as f32, "{id} {w}x{h}: only {n} px at rest");
            }
        }
    }

    /// The winding test, from the chase camera: the roof always faces the viewer and the floor never
    /// does; straight ahead the viewer sees the tail, not the nose.
    #[test]
    fn from_behind_the_roof_and_tail_show() {
        let roof: Vec<(f32, f32, f32)> = CABIN_HI.iter().map(|p| (p.0, p.1, CABIN_Y.1)).collect();
        let floor: Vec<(f32, f32, f32)> = BODY.iter().rev().map(|p| (p.0, p.1, BODY_Y.0)).collect();
        for k in 0..36 {
            let pose = Pose::new(0.0, CAR_Z, k as f32 * TAU / 36.0, test_cam());
            assert!(faces_viewer(&pose, &roof), "heading {}: roof hidden", k * 10);
            assert!(!faces_viewer(&pose, &floor), "heading {}: floor shown", k * 10);
        }
        let pose = Pose::new(0.0, CAR_Z, 0.0, test_cam());
        let side = |i: usize| {
            let (a, b) = (BODY[i], BODY[(i + 1) % 8]);
            faces_viewer(&pose, &[(a.0, a.1, 1.2), (b.0, b.1, 1.2), (b.0, b.1, 4.0), (a.0, a.1, 4.0)])
        };
        assert!(side(4), "tail hidden");
        assert!(!side(0), "nose shown");
    }

    /// The R34 tail: straight on from behind, four separate round `hot` lamps across the rear face.
    #[test]
    fn the_tail_has_four_round_lamps() {
        let t = theme("drift-night");
        let paint = Paint::new(&t);
        let mut c = Canvas::new(120, 70);
        let pose = Pose::new(0.0, CAR_Z, 0.0, test_cam());
        draw_shapes(&mut c, &car_shapes(&pose, 0.0, &paint));
        let (_, gy) = pose.screen((BODY[4].0, 0.0, TAIL_Y));
        let row = gy.round() as i32;
        let mut runs = 0;
        let mut inside = false;
        for x in 0..120 {
            let p = c.get(x, row);
            let lamp = p == paint.tail || p == paint.tail_core;
            if lamp && !inside {
                runs += 1;
            }
            inside = lamp;
        }
        assert_eq!(runs, 4, "{runs} lamp runs across the tail's row {row}");
    }

    /// The wheels sit inside the body: from behind and turned up to 30 degrees, drawing them adds no
    /// width to the car's painted silhouette (the old ones stuck out like a tractor's), only a little
    /// tyre under the sills.
    #[test]
    fn the_wheels_tuck_in_under_the_body() {
        let t = theme("drift-shibuya");
        let paint = Paint::new(&t);
        for deg in [-30.0f32, 0.0, 30.0] {
            let paint_box = |with: bool| {
                let mut c = Canvas::new(120, 70);
                let pose = Pose::new(0.0, CAR_Z, deg.to_radians(), test_cam());
                draw_shapes(&mut c, &car_shapes_opt(&pose, 0.0, &paint, with));
                let (mut x0, mut x1, mut y1) = (i32::MAX, i32::MIN, i32::MIN);
                for y in 0..70 {
                    for x in 0..120 {
                        if c.get(x, y).a > 0 {
                            x0 = x0.min(x);
                            x1 = x1.max(x);
                            y1 = y1.max(y);
                        }
                    }
                }
                (x0, x1, y1)
            };
            let (a0, a1, ay) = paint_box(false);
            let (b0, b1, by) = paint_box(true);
            assert!(b0 >= a0 - 1 && b1 <= a1 + 1, "{deg}: wheels widen the car {a0}..{a1} -> {b0}..{b1}");
            assert!(by >= ay && by <= ay + 3, "{deg}: tyres hang {} px below the body", by - ay);
        }
    }

    /// The cliff: just left of the road, from its foot to above the horizon, the frame shows rock - not
    /// the sky, not the flat slope - and near the camera it rises out of the top of the panel.
    #[test]
    fn a_cliff_stands_beside_the_road() {
        let t = calm_theme("drift-night");
        let mut fam = Drift::default();
        let c = frames(&mut fam, &t, 380, 60, 0.0, 30);
        let pal = Palette::new(&t);
        let cam = fam.cam(380);
        let z = 60.0;
        let x = fam.road_at(z) - (HALF_W + KERB_W + CLIFF_GAP) - 0.5;
        let (bx, by) = cam.project(x, fam.elev_at(z), z);
        let (_, ty) = cam.project(x, fam.elev_at(z) + CLIFF_H, z);
        assert!(ty < fam.layout.hz, "the cliff top at row {ty:.1} does not rise above the horizon");
        let mut rock = 0;
        for y in (ty.ceil() as i32 + 1)..(by.floor() as i32 - 1) {
            let p = c.get(bx.round() as i32 - 1, y);
            let near = |q: Rgba| (p.r as i32 - q.r as i32).abs() + (p.g as i32 - q.g as i32).abs() + (p.b as i32 - q.b as i32).abs() < 40;
            if near(pal.rock_a) || near(pal.rock_b) || near(pal.rock_line) {
                rock += 1;
            }
        }
        assert!(rock >= 3, "only {rock} rock px up the cliff face at depth {z}");
        let top_near = cam.project(fam.road_at(10.0) - (HALF_W + KERB_W + CLIFF_GAP), fam.elev_at(10.0) + CLIFF_H, 10.0).1;
        assert!(top_near < 4.0, "near the camera the cliff top is at row {top_near:.1}, inside the panel");
    }

    /// The road has hills: over a run, the road at a fixed far depth moves up and down the screen,
    /// and now and then a crest hides the road beyond it (rows under the horizon get no road).
    #[test]
    fn the_road_climbs_and_dips() {
        let t = calm_theme("drift-touge");
        let mut fam = Drift::default();
        let (mut lo, mut hi, mut hidden) = (f32::MAX, f32::MIN, 0);
        for _ in 0..120 {
            let _ = frames(&mut fam, &t, 380, 60, 0.15, 10);
            let cam = fam.cam(380);
            let y = cam.project(0.0, fam.elev_at(200.0), 200.0).1;
            lo = lo.min(y);
            hi = hi.max(y);
            let hz = fam.layout.hz_row as usize;
            if (hz..hz + 3).any(|r| !fam.row_ok[r]) {
                hidden += 1;
            }
        }
        assert!(hi - lo >= 6.0, "the far road only moved rows {lo:.1}..{hi:.1}");
        assert!(hidden >= 5, "a crest hid the far road in only {hidden} of 120 samples");
    }

    /// The model really turns: from behind it is narrow; turned 45 degrees either way it is wider on
    /// screen (its side now shows), and it shows several shades rather than one flat colour.
    #[test]
    fn the_car_model_turns_and_is_shaded() {
        let t = theme("drift-shibuya");
        let paint = Paint::new(&t);
        let draw_at = |deg: f32| {
            let mut c = Canvas::new(120, 70);
            let pose = Pose::new(0.0, CAR_Z, deg.to_radians(), test_cam());
            let sh = car_shapes(&pose, 0.0, &paint);
            for s in &sh.s[..sh.n] {
                if s.n >= 3 {
                    fill_convex(&mut c, &s.pts[..s.n], (0.0, 0.0), 0, s.col);
                }
            }
            let (mut x0, mut x1) = (i32::MAX, i32::MIN);
            let mut cols: Vec<u32> = Vec::new();
            for y in 0..70 {
                for x in 0..120 {
                    let p = c.get(x, y);
                    if p.a > 0 {
                        x0 = x0.min(x);
                        x1 = x1.max(x);
                        let k = (p.r as u32) << 16 | (p.g as u32) << 8 | p.b as u32;
                        if !cols.contains(&k) {
                            cols.push(k);
                        }
                    }
                }
            }
            (x1 - x0 + 1, cols.len())
        };
        let (w0, n0) = draw_at(0.0);
        let (w45, n45) = draw_at(45.0);
        let (wm45, _) = draw_at(-45.0);
        assert!(w45 >= w0 + 6 && wm45 >= w0 + 6, "turned 45, {w45} / {wm45} px wide vs {w0} from behind");
        assert!(n0 >= 4 && n45 >= 4, "only {n0} / {n45} distinct colours: not shaded");
    }

    /// Silent: the car drives the bends on grip - next to no slip - and stays near the bottom centre.
    #[test]
    fn silent_the_car_drives_on_grip_near_the_centre() {
        for id in IDS {
            let t = calm_theme(id);
            for (w, h) in [(380, 60), (128, 44)] {
                let mut fam = Drift::default();
                fam.only_for_test(Only::Car);
                let _ = frames(&mut fam, &t, w, h, 0.0, 60);
                for _ in 0..30 {
                    let c = frames(&mut fam, &t, w, h, 0.0, 10);
                    let (x0, y0, x1, y1) = bbox(&c, &t).expect("no car drawn");
                    assert!(((x0 + x1) / 2 - w / 2).abs() <= w / 10, "{id} {w}x{h}: car off centre at {x0}..{x1}");
                    assert!(y1 >= h / 2 && y0 >= 4 && y1 < h - 4, "{id} {w}x{h}: car not at the bottom, rows {y0}..{y1}");
                    assert!(fam.slip.abs() < 1.0, "{id} {w}x{h}: slip {} with no music", fam.slip);
                }
            }
        }
    }

    /// Loud: the drift comes from the road - in a bend the slip points into it (most of the time), it
    /// reaches a big angle, and over a run of S-bends it swaps lock.
    #[test]
    fn loud_the_drift_follows_the_bends() {
        let t = calm_theme("drift-shibuya");
        let mut fam = Drift::default();
        fam.only_for_test(Only::Car);
        let mut c = Canvas::new(380, 60);
        let (mut agree, mut bent, mut swaps, mut last, mut max_slip) = (0, 0, 0, 0.0f32, 0.0f32);
        for k in 0..1200 {
            c.clear();
            fam.draw(&mut c, &t, &music(0.85, 0.22, k, 0));
            if k < 60 {
                continue;
            }
            if fam.bend.abs() > 0.6 {
                bent += 1;
                if fam.slip * fam.bend > 0.0 {
                    agree += 1;
                }
            }
            if fam.slip.abs() > 8.0 {
                let sg = fam.slip.signum();
                if last != 0.0 && sg != last {
                    swaps += 1;
                }
                last = sg;
            }
            max_slip = max_slip.max(fam.slip.abs());
        }
        assert!(bent > 100, "only {bent} frames in a bend: the course is too straight");
        assert!(agree * 10 >= bent * 9, "slip into the bend on only {agree} of {bent} bent frames");
        assert!(max_slip >= 25.0, "slip only reached {max_slip:.0} degrees");
        assert!(swaps >= 2, "only {swaps} swaps of lock in 19 s of S-bends");
    }

    /// A clutch kick on the bass: more slip and the exhaust pops.
    #[test]
    fn a_kick_is_a_clutch_kick() {
        let t = calm_theme("drift-shibuya");
        let mut fam = Drift::default();
        let mut c = Canvas::new(380, 60);
        for k in 0..200 {
            c.clear();
            fam.draw(&mut c, &t, &music(0.6, 0.15, k, 0));
        }
        assert!(fam.kick < 0.5 && fam.flame == 0.0, "kick {} flame {} with no beat", fam.kick, fam.flame);
        let mut fired = false;
        for k in 200..260 {
            c.clear();
            fam.draw(&mut c, &t, &music(0.6, 0.15, k, if k < 203 { 1 } else { 0 }));
            fired |= fam.flame > 0.0 && fam.kick > KICK_SLIP * 0.8 && fam.bump > 0.5;
        }
        assert!(fired, "a kick did not kick");
    }

    /// The road bends: over a run its far end swings across the panel, and it streams (consecutive
    /// frames differ on the ground even in silence).
    #[test]
    fn the_road_winds_and_streams() {
        let t = calm_theme("drift-touge");
        let mut fam = Drift::default();
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for _ in 0..60 {
            let _ = frames(&mut fam, &t, 380, 60, 0.15, 10);
            let row = (fam.layout.hz_row + 2) as usize;
            lo = lo.min(fam.row_cx[row]);
            hi = hi.max(fam.row_cx[row]);
        }
        assert!(hi - lo >= 80.0, "the far road only swung {lo:.0}..{hi:.0}");
        let a = frames(&mut fam, &t, 380, 60, 0.0, 1);
        let b = frames(&mut fam, &t, 380, 60, 0.0, 1);
        let hz = fam.layout.hz_row;
        let mut diff = 0;
        for y in hz..56 {
            for x in 3..377 {
                if a.get(x, y) != b.get(x, y) {
                    diff += 1;
                }
            }
        }
        assert!(diff >= 50, "only {diff} ground px changed between frames");
    }

    /// The valley's lights show beyond the guardrail and some of them twinkle.
    #[test]
    fn the_city_glitters_in_the_valley() {
        let t = theme("drift-shibuya");
        let l = layout(380, 60);
        let tw: Vec<Light> = l.lights[..l.lights_n].iter().copied().filter(|li| li.twinkle).collect();
        assert!(!tw.is_empty(), "no twinkling light");
        assert!(tw.iter().any(|li| (0..60).any(|k| !light_on(li, k as f32 / 60.0)) && (0..60).any(|k| light_on(li, k as f32 / 60.0))));
        let mut fam = Drift::default();
        let c = frames(&mut fam, &t, 380, 60, 0.0, 20);
        let neon = neon_of(&t);
        let mut n = 0;
        for y in fam.layout.hz_row..56 {
            let edge = (fam.row_cx[y as usize] + fam.row_hw[y as usize] * 1.25) as i32 + 2;
            for x in edge.max(3)..377 {
                let p = c.get(x, y);
                let d = (p.r as i32 - neon.r as i32).abs() + (p.g as i32 - neon.g as i32).abs() + (p.b as i32 - neon.b as i32).abs();
                if d < 120 {
                    n += 1;
                }
            }
        }
        assert!(n >= 4, "only {n} neon city lights in the valley");
    }

    /// Bass-only vs treble-only, straight (rms 0): bass makes the fresh puffs at the tyres big, so the
    /// smoke sits low by the wheels; treble makes the old risen puffs big, so it sits higher.
    #[test]
    fn the_smoke_billows_with_the_spectrum() {
        for id in IDS {
            let t = calm_theme(id);
            for (w, h) in [(380, 60), (128, 44)] {
                let run = |bands: std::ops::Range<usize>| {
                    let mut fam = Drift::default();
                    fam.only_for_test(Only::Smoke);
                    let c = frames_bands(&mut fam, &t, w, h, bands, 0.9, 0.0, 90);
                    centroid_y(&c, &t)
                };
                let ((yb, nb), (yt, nt)) = (run(0..16), run(48..64));
                assert!(nb > 20 && nt > 20, "{id} {w}x{h}: smoke {nb} / {nt} px");
                assert!(yb >= yt + 2.0, "{id} {w}x{h}: bass smoke at row {yb:.1}, treble at {yt:.1}");
            }
        }
    }

    /// A forced drift spins the car a full turn and lets go after.
    #[test]
    fn drift_flourish_spins_the_car() {
        for id in IDS {
            let t = theme(id);
            for (w, h) in [(380, 60), (190, 48), (128, 44)] {
                let mut fam = Drift::default();
                fam.only_for_test(Only::Car);
                let _ = frames(&mut fam, &t, w, h, 0.0, 30);
                fam.flourish.force_next();
                let mut spin = 0.0f32;
                for _ in 0..54 {
                    let c = frames(&mut fam, &t, w, h, 0.0, 1);
                    assert!(bbox(&c, &t).is_some(), "{id} {w}x{h}: the car vanished in the drift");
                    spin = spin.max(fam.spin.abs());
                }
                assert!(spin >= 0.95 * TAU, "{id} {w}x{h}: the drift only spun {:.0} degrees", spin.to_degrees());
                let _ = frames(&mut fam, &t, w, h, 0.0, 10);
                assert!(fam.spin == 0.0, "{id} {w}x{h}: still spinning after the drift");
            }
        }
    }

    /// `DRIFT!` in `hot`, with its dark outline, is on screen during the drift and gone after it.
    #[test]
    fn drift_text_slams_in_on_the_flourish() {
        for id in IDS {
            let t = theme(id);
            let hot = Rgba::from_hex(&t.hot, 1.0);
            for (w, h) in [(380, 60), (128, 44)] {
                let l = layout(w, h);
                let (tx, ty) = l.drift_text;
                let count = |c: &Canvas| {
                    let mut n = 0;
                    for y in ty..ty + 10 {
                        for x in tx..tx + text_w("DRIFT!", 2) {
                            if c.get(x, y) == hot {
                                n += 1;
                            }
                        }
                    }
                    n
                };
                let mut fam = Drift::default();
                let _ = frames(&mut fam, &t, w, h, 0.1, 30);
                let before = count(&frames(&mut fam, &t, w, h, 0.1, 1));
                fam.flourish.force_next();
                let _ = frames(&mut fam, &t, w, h, 0.1, 15); // ~250 ms in
                let during = count(&frames(&mut fam, &t, w, h, 0.1, 1));
                assert!(during >= before + 4 * 44, "{id} {w}x{h}: only {during} hot px of DRIFT! (before: {before})");
                let _ = frames(&mut fam, &t, w, h, 0.1, 50);
                let after = count(&frames(&mut fam, &t, w, h, 0.1, 1));
                assert!(before <= 24, "{id} {w}x{h}: DRIFT! up before the drift ({before} hot px)");
                assert!(after <= 24, "{id} {w}x{h}: DRIFT! still up after the drift ({after} hot px)");
            }
        }
    }

    /// 380x60 draws the steering gauge and its readout; 190x48 and 128x44 drop it. Counted in the
    /// gauge's own colours - its ring and the readout's `lit` digits.
    #[test]
    fn the_gauge_fits_or_drops() {
        let t = theme("drift-night");
        let lit_c = Rgba::from_hex(&t.lit, 1.0);
        let ring = Rgba::lerp_linear(Rgba::from_hex(&t.panel, 1.0), lit_c, 0.45);
        let gauge_px = |w: i32, h: i32| {
            let c = frames(&mut Drift::default(), &t, w, h, 0.15, 60);
            let mut n = 0;
            for y in 4..4 + 2 * GAUGE_R + 2 {
                for x in w - 3 - 60..w - 3 {
                    let p = c.get(x, y);
                    if p == lit_c || p == ring {
                        n += 1;
                    }
                }
            }
            n
        };
        assert!(gauge_px(380, 60) >= 20, "380x60: no gauge");
        assert_eq!(gauge_px(190, 48), 0, "190x48: the gauge did not drop");
        assert_eq!(gauge_px(128, 44), 0, "128x44: the gauge did not drop");
    }

    /// 190x48 and 128x44 with a forced drift: no panic, something drawn, nothing outside the panel.
    #[test]
    fn fits_the_narrow_panel_and_flourish_does_not_panic() {
        for id in IDS {
            let t = theme(id);
            for (w, h) in [(190, 48), (128, 44)] {
                let mut fam = Drift::default();
                let _ = frames(&mut fam, &t, w, h, 0.7, 5);
                fam.flourish.force_next();
                for n in [1usize, 10, 10, 30] {
                    let c = frames(&mut fam, &t, w, h, 0.7, n);
                    assert!(lit(&c, &t) > 0, "{id} {w}x{h}");
                    for x in 0..w {
                        for y in [0, 1, h - 2, h - 1] {
                            assert_eq!(c.get(x, y).a, 0, "{id} {w}x{h}: paint outside the panel at {x},{y}");
                        }
                    }
                    for y in 0..h {
                        assert_eq!(c.get(0, y).a, 0, "{id} {w}x{h}: paint left of the panel at 0,{y}");
                        assert_eq!(c.get(w - 1, y).a, 0, "{id} {w}x{h}: paint right of the panel at {y}");
                    }
                }
            }
        }
    }

    /// Garbage in (NaN levels, rms and dt): no panic, and the state stays finite.
    #[test]
    fn garbage_frames_do_not_break_the_motion() {
        let t = theme("drift-shibuya");
        let mut fam = Drift::default();
        let mut c = Canvas::new(380, 60);
        let mut d = FrameData::default();
        for k in 0..120 {
            d.levels = [if k % 2 == 0 { f32::NAN } else { f32::INFINITY }; NUM_BANDS];
            d.rms_l = f32::NAN;
            d.rms_r = f32::INFINITY;
            d.dt_ms = if k % 3 == 0 { f32::NAN } else { 1.0e9 };
            fam.draw(&mut c, &t, &d);
        }
        for v in [fam.s_cam, fam.cam_x, fam.lat, fam.slip, fam.amp, fam.bg_off] {
            assert!(v.is_finite(), "state went non-finite: {v}");
        }
    }

    #[test]
    fn the_four_colourways_are_visibly_different() {
        let (w, h) = (380i32, 60i32);
        let canvases: Vec<Canvas> =
            IDS.iter().map(|id| frames(&mut Drift::default(), &theme(id), w, h, 0.12, 30)).collect();
        let (ix0, iy0, ix1, iy1) = (3, 4, w - 3, h - 4);
        let interior = ((ix1 - ix0) * (iy1 - iy0)) as f32;
        let mut too_similar = Vec::new();
        for a in 0..IDS.len() {
            for b in (a + 1)..IDS.len() {
                let mut diff = 0usize;
                for y in iy0..iy1 {
                    for x in ix0..ix1 {
                        let (pa, pb) = (canvases[a].get(x, y), canvases[b].get(x, y));
                        let d = (pa.r as i32 - pb.r as i32).abs()
                            + (pa.g as i32 - pb.g as i32).abs()
                            + (pa.b as i32 - pb.b as i32).abs();
                        if d > 24 {
                            diff += 1;
                        }
                    }
                }
                let frac = diff as f32 / interior;
                if frac < 0.15 {
                    too_similar.push(format!("{} vs {}: {:.1}%", IDS[a], IDS[b], frac * 100.0));
                }
            }
        }
        assert!(too_similar.is_empty(), "colourway pairs differ in <15% of interior pixels: {too_similar:?}");
    }

    #[test]
    fn every_label_char_has_a_glyph() {
        for s in ["DRIFT!", "ANGLE ", "0123456789"] {
            for ch in s.chars() {
                assert!(font3x5::glyph(ch).is_some(), "{s:?} {ch:?}");
            }
        }
    }

    /// Dumps for the eye test, composited over `#202020`. Each file is `<name>.<w>x<h>.rgba`. The
    /// `-strip` dumps stack eight frames half a second apart, top to bottom, to show the motion.
    ///
    /// Run: cargo test --release dump_drift -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_drift() {
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
            std::fs::write(dir.join(format!("{name}.{}x{}.rgba", c.width(), c.height())), &out).unwrap();
        };
        let run = |id: &str, w: i32, h: i32, level: f32, rms: f32, n: usize| {
            let t = calm_theme(id);
            let mut fam = Drift::default();
            let mut c = Canvas::new(w, h);
            for k in 0..n {
                c.clear();
                fam.draw(&mut c, &t, &music(level, rms, k, 30));
            }
            (fam, c, t)
        };
        let flourish = |id: &str, w: i32, h: i32, at: usize| {
            let (mut fam, mut c, t) = run(id, w, h, 0.6, 0.12, 110);
            fam.flourish.force_next();
            for k in 110..110 + at {
                c.clear();
                fam.draw(&mut c, &t, &music(0.6, 0.12, k, 0));
            }
            c
        };
        let strip = |id: &str, w: i32, h: i32, level: f32, rms: f32| {
            let t = calm_theme(id);
            let mut fam = Drift::default();
            let mut c = Canvas::new(w, h);
            let mut out = Canvas::new(w, h * 8);
            for k in 0..(120 + 8 * 30) {
                c.clear();
                fam.draw(&mut c, &t, &music(level, rms, k, 30));
                if k >= 120 && (k - 120) % 30 == 29 {
                    let row = ((k - 120) / 30) as i32;
                    out.copy_region(&c, (0, 0), (0, row * h), w, h);
                }
            }
            out
        };
        for id in IDS {
            let short = &id["drift-".len()..];
            for (tag, level, rms, n) in [("calm", 0.3f32, 0.07f32, 250), ("loud", 0.85, 0.2, 400)] {
                let (_, c, _) = run(id, 380, 60, level, rms, n);
                write(format!("drift-{short}-{tag}"), &c);
            }
            write(format!("drift-{short}-flourish"), &flourish(id, 380, 60, 15));
            write(format!("drift-{short}-flourish-late"), &flourish(id, 380, 60, 30));
        }
        write("drift-shibuya-loud-strip".into(), &strip("drift-shibuya", 380, 60, 0.85, 0.2));
        write("drift-orange-calm-strip".into(), &strip("drift-orange", 380, 60, 0.3, 0.07));
        // The car alone at eight headings from the chase camera.
        for id in ["drift-shibuya", "drift-orange"] {
            let t = theme(id);
            let paint = Paint::new(&t);
            let mut c = Canvas::new(8 * 60, 40);
            c.fill_rect(0, 0, 8 * 60, 40, Palette::new(&t).road_a);
            for (k, deg) in [-60.0f32, -40.0, -20.0, 0.0, 20.0, 40.0, 60.0, 180.0].into_iter().enumerate() {
                let cam = Cam { cx: k as f32 * 60.0 + 30.0, hz: 0.0, f: FOCAL, x: 0.0, y: CAM_H };
                let pose = Pose::new(0.0, CAR_Z, deg.to_radians(), cam);
                draw_shapes(&mut c, &car_shapes(&pose, (-deg * 0.6).clamp(-COUNTER_MAX, COUNTER_MAX).to_radians(), &paint));
            }
            write(format!("{id}-turntable"), &c);
        }
        for (w, h) in [(190, 48), (128, 44)] {
            let (_, c, _) = run("drift-shibuya", w, h, 0.85, 0.2, 400);
            write(format!("drift-shibuya-{w}x{h}"), &c);
            write(format!("drift-shibuya-{w}x{h}-flourish"), &flourish("drift-shibuya", w, h, 15));
        }
        println!("wrote drift dumps to {}", dir.display());
    }

    /// Per-layer cost with `slow_vs_timing`'s frame (every band 0.5 + 0.4 sin), to find what is heavy.
    ///
    /// Run: cargo test --release probe_drift_layers -- --ignored --nocapture
    #[test]
    #[ignore]
    fn probe_drift_layers() {
        let t = theme("drift-shibuya");
        let mut d = FrameData::default();
        for (i, v) in d.levels.iter_mut().enumerate() {
            *v = 0.5 + 0.4 * ((i as f32) * 0.3).sin();
        }
        d.dt_ms = 16.7;
        for only in [Only::All, Only::Car, Only::Smoke] {
            let mut fam = Drift::default();
            fam.only_for_test(only);
            let mut c = Canvas::new(380, 60);
            for _ in 0..30 {
                fam.draw(&mut c, &t, &d);
            }
            let t0 = std::time::Instant::now();
            for _ in 0..300 {
                fam.draw(&mut c, &t, &d);
            }
            println!("{}: {:.3} ms/frame", match only { Only::All => "all", Only::Car => "car", Only::Smoke => "smoke" }, t0.elapsed().as_secs_f64() * 1000.0 / 300.0);
        }
    }

    /// Per-frame cost, steady and during the drift, at 380x60.
    ///
    /// Run: cargo test --release probe_drift_cost -- --ignored --nocapture
    #[test]
    #[ignore]
    fn probe_drift_cost() {
        for id in IDS {
            let t = theme(id);
            let mut fam = Drift::default();
            let mut c = Canvas::new(380, 60);
            for k in 0..60 {
                fam.draw(&mut c, &t, &music(0.55, 0.2, k, 24));
            }
            let n = 600;
            let t0 = std::time::Instant::now();
            for k in 0..n {
                fam.draw(&mut c, &t, &music(0.55, 0.2, k, 24));
            }
            let steady = t0.elapsed().as_secs_f64() * 1000.0 / n as f64;
            fam.flourish.force_next();
            let m = 54; // the 900 ms drift
            let t1 = std::time::Instant::now();
            for k in n..(n + m) {
                fam.draw(&mut c, &t, &music(0.55, 0.2, k, 24));
            }
            let flourish = t1.elapsed().as_secs_f64() * 1000.0 / m as f64;
            println!("{id}: steady {steady:.3} ms/frame, flourish {flourish:.3} ms/frame at 380x60");
        }
    }
}
