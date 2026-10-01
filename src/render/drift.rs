//! The drift family: Tokyo Drift. A night street seen from above at an angle - a neon skyline behind,
//! a road in front - and a car drifting down it whose tyre-smoke trail IS the meter.
//!
//! # The view
//!
//! An oblique parallel projection, the pixel-art "iso" look that still fits a 60 px panel: world `x`
//! (along the road) is screen x; depth `z` (across the road, away from the viewer) goes up and to the
//! right, `ZX` px across and `ZY` px up per unit; height `y` goes straight up, `YY` px per unit. The
//! road stays horizontal - a true isometric road would climb ~190 px across a 380 px panel - and
//! everything with depth still shows a top and a side: the buildings' roofs and side faces, the
//! sidewalk's slanted seams, the zebra crossings, and the car.
//!
//! # The scene
//!
//! - **Skyline (baked once per size and colours into `bg`).** A night sky shading from `panel` at the
//!   top toward `edge` and a wash of the neon at the horizon; a far row of hazy towers and a near row
//!   of black blocks standing on the back of the sidewalk, each an iso box (a roof and a right-hand
//!   side face as parallelograms) with hashed widths and heights, the near ones carrying a scatter of
//!   lit windows. Drawn left to right, so a block's side face shows only above a lower neighbour.
//! - **Neon signs.** Vertical blades on the near blocks' fronts and bars on their roofs in the neon
//!   (`zones[0].lit`) with a few in `hot`. The unlit tubes are baked; the lit tube, a 1 px halo
//!   (brighter on the bass) and a faint streak of reflection on the wet road are drawn each frame, so
//!   a third of them can flicker at 1-2 Hz the way a failing neon does - reflection and all.
//! - **The street.** A sidewalk with slanted seams, a bright far kerb, the asphalt, a centre line, the
//!   near kerb, and every `CROSSING` units a zebra crossing of slanted stripes (Shibuya) - all
//!   scrolling with the world.
//! - **The car.** Our own low-poly coupe - no model, no badge: an octagonal body prism, a cabin
//!   frustum with glass in the neon, a rear wing on struts, and four wheels, the front pair
//!   counter-steering. It is projected and flat-shaded every frame at its real heading (faces culled
//!   by their projected winding, parts drawn in painter's order, a 1 px dark outline under all of it),
//!   so it TURNS on the road. Head and tail lamps sit on the front and rear faces; the headlights throw
//!   a beam on the asphalt that sweeps as the car swings; a soft shadow sits under it.
//! - **The drift.** The car weaves across the road (a critically damped spring toward one side or the
//!   other, its reach growing with rms) while the body holds a slip angle into the turn - up to
//!   `SLIP_MAX` at full rms - and FLICKS to the opposite lock on a bass kick (at most once every
//!   `MIN_FLIP_MS`, and on its own after `MAX_FLIP_MS` without one). The flick is quick (`FLICK_MS`)
//!   and the path follows slowly, so the body swings first and then the car arcs the other way, which
//!   is what a drift transition looks like. Silence: the car straightens and cruises down the centre.
//! - **Skid marks.** A fixed ring of each rear wheel's last `MARKS` road positions, scrolled with the
//!   world and drawn as dark lines fading with age, laid only while the tyres slide.
//! - **The meter - the tyre smoke.** A fixed ring of the rear axle's last 64 road positions, scrolled
//!   with the world, so the trail follows the car's curving path and rises as it ages. Puff `k` (band
//!   `k`, bass at the car) is a filled circle of radius `1 + level * 7` px (scaled on a short panel) in
//!   white at `ghost` alpha with a 2-step rim: the smoke billows at the car on a bass-heavy passage and
//!   thins to a wisp on treble. `FrameData.levels` arrive already smoothed by main's `Smoother`; the
//!   puffs read them directly - no second attack/decay here.
//! - **Steering gauge.** Top right, a radius-8 semicircle with a needle at the slip angle and
//!   `ANGLE NN` (`[u8; 2]`). Dropped below 200 px wide.
//!
//! # The flourish - the drift
//!
//! 900 ms, fired only on a bass hit. The car whips across the full width in one sweep (out past the
//! right edge and back in from the left to where the wander has it), pinned at full lock and spinning
//! a full 360 on the way; the smoke goes to max radius tinted `hot`, so the sweep lays a wall of it
//! across the panel; the neon flashes; and `DRIFT!` (2x `font3x5`, `hot`, 1 px dark outline) slams in
//! centred in the sky for 400 ms (one frame at 3x, then 2x with a 1 px shake). No Japanese glyphs.
//!
//! The panel is opaque (painted first) and the clip to the rounded rect runs last on every path.
//! `draw` allocates nothing after the first frame at a size: the background canvas and the geometry
//! are rebuilt only on a size (or colour) change, the trail and the marks are fixed rings, the car's
//! polygons a fixed array on the stack, and the angle readout `[u8; 2]`.

use std::f32::consts::TAU;

use crate::dsp::bands::NUM_BANDS;
use crate::dsp::flourish::{Envelope, Trigger};
use crate::dsp::onset::Flux;
use crate::render::canvas::{Canvas, Rgba};
use crate::render::font3x5;
use crate::render::{Family, FrameData};
use crate::themes::Theme;

// ---- the projection ----

/// Screen px per unit of depth, across (right) and up.
const ZX: f32 = 0.60;
const ZY: f32 = 0.55;
/// Screen px per unit of height, up.
const YY: f32 = 0.85;
/// The light, in world (x, y, z): from above, a little to the right and toward the viewer.
const LIGHT: (f32, f32, f32) = (0.35, 0.85, -0.40);

// ---- the street ----

/// The road's share of the interior height on screen.
const ROAD_FRAC: f32 = 0.38;
/// Scrolling street features' periods, in world units at full scale, and a common multiple of them to
/// wrap the scroll at so none of them jumps.
const SEAM: f32 = 8.0;
const DASH: f32 = 14.0;
const DASH_ON: f32 = 6.0;
const CROSSING: f32 = 300.0;
const STRIPES: i32 = 6;
const STRIPE_W: f32 = 2.5;
const SCROLL_WRAP: f32 = 4200.0;

// ---- the meter and the marks ----

/// The trail: one puff per band, and the ring of past positions they sit on.
const TRAIL: usize = NUM_BANDS;
/// Puff radius `1 + level * PUFF_GAIN` px on a full-height panel.
const PUFF_GAIN: f32 = 7.0;
/// Skid marks: this many past positions per rear wheel.
const MARKS: usize = 120;
/// Marks start at this slip (degrees) and are at full strength this far past it.
const MARK_FROM: f32 = 6.0;
const MARK_SPAN: f32 = 22.0;

// ---- the car's motion ----

/// The car's centre wanders across the screen between 32 % and 68 % of the width, one swing every
/// `SWING_S` at full rms.
const SWING_FRAC: f32 = 0.18;
const SWING_S: f32 = 9.0;
/// How fast the motion's amplitude follows rms (a motion ease, not a level smoother).
const SWING_EASE_MS: f32 = 600.0;
/// The world scrolls at this fraction of the interior width per second, plus more with rms.
const SCROLL_BASE: f32 = 0.20;
const SCROLL_RMS: f32 = 0.16;
/// The slip angle at full rms, degrees, and how fast the body flicks to a new one.
const SLIP_MAX: f32 = 40.0;
const FLICK_MS: f32 = 140.0;
/// The lateral spring (rad/s) and how far either side of the road's centre it reaches at full rms,
/// as a fraction of the road's depth.
const LAT_OMEGA: f32 = 5.0;
const Z_SWING: f32 = 0.28;
/// A bass kick flicks the car to the other lock at most this often; with no kick it flicks anyway
/// after `MAX_FLIP_MS`. Below `FLIP_AMP` of motion it does not flick at all.
const MIN_FLIP_MS: f32 = 650.0;
const MAX_FLIP_MS: f32 = 2200.0;
const FLIP_AMP: f32 = 0.08;
/// A kick: an onset (the `bling` net) with the low bands over this.
const KICK_BASS: f32 = 0.45;
const ONSET_RATIO: f32 = 2.8;
const ONSET_REFRACTORY_MS: f32 = 200.0;
/// The front wheels counter-steer up to this, degrees.
const COUNTER_MAX: f32 = 30.0;

// ---- the car's model: `u` forward, `v` to its left, `y` up, world units at full scale ----

/// The body: an octagonal prism, nose and tail chamfered, counter-clockwise seen from above. Edge 0
/// is the front face, edge 4 the rear.
const BODY: [(f32, f32); 8] =
    [(11.0, -3.6), (11.0, 3.6), (9.2, 5.0), (-9.6, 5.0), (-11.0, 4.0), (-11.0, -4.0), (-9.6, -5.0), (9.2, -5.0)];
const BODY_Y: (f32, f32) = (1.2, 4.0);
/// The cabin: a frustum from the body's top to the roof; its faces are front, left, rear, right.
const CABIN_LO: [(f32, f32); 4] = [(4.0, -4.3), (4.0, 4.3), (-6.6, 4.3), (-6.6, -4.3)];
const CABIN_HI: [(f32, f32); 4] = [(0.4, -3.5), (0.4, 3.5), (-4.6, 3.5), (-4.6, -3.5)];
const CABIN_Y: (f32, f32) = (4.0, 6.9);
/// The rear wing: a thin plate on two struts.
const WING: [(f32, f32); 4] = [(-9.4, -5.0), (-9.4, 5.0), (-11.6, 5.0), (-11.6, -5.0)];
const WING_Y: (f32, f32) = (6.0, 6.6);
const STRUT: (f32, f32) = (-10.5, 2.8);
/// Wheels: the axles, the track's half width, and a wheel's half length, half width and height.
const AXLE_F: f32 = 7.2;
const AXLE_R: f32 = -7.0;
const TRACK: f32 = 4.4;
const WHEEL: (f32, f32, f32) = (2.3, 0.9, 3.4);
/// Lamps: each one's `v` span out from the centre line and its `y` span, on the end faces.
const LAMP_V: (f32, f32) = (1.7, 3.5);
const LAMP_Y: (f32, f32) = (2.3, 3.7);
/// A turned car's reach from its centre on screen, for the sweep's off-screen margin.
const REACH: f32 = 16.0;
/// The car is drawn this much larger than the street's scale, so it reads at 1:1 on the taskbar.
const CAR_SCALE: f32 = 1.2;

// ---- the gauge, the signs, the drift ----

/// Below this width the steering gauge is dropped.
const GAUGE_MIN_W: i32 = 200;
const GAUGE_R: i32 = 8;
/// Neon: a flickering sign is off for a stutter each cycle, at 1-2 Hz.
const SIGNS: usize = 40;
/// The drift: total length, the sweep's share, `DRIFT!`'s window, the sign flash.
const DRIFT_MS: f32 = 900.0;
const SWEEP_MS: f32 = 720.0;
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
const WARM_WINDOW: Rgba = Rgba { r: 0xff, g: 0xc8, b: 0x7a, a: 255 };
/// `DRIFT!`'s outline and the car's.
const OUTLINE: Rgba = Rgba { r: 0x05, g: 0x03, b: 0x08, a: 255 };

/// A building block in the skyline: its front face's left column, width and height.
#[derive(Clone, Copy, Default)]
struct Block {
    x: i32,
    w: i32,
    h: i32,
}

/// A neon sign: a bar, whether it flickers and at what rate/phase, and whether it is in `hot`.
#[derive(Clone, Copy, Default)]
struct Sign {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    flicker: bool,
    rate: f32,
    phase: f32,
    hot: bool,
}

/// Everything positional, for a panel size. Shared by `draw`, the bake and the tests.
#[derive(Clone, Copy)]
struct Layout {
    /// World scale: 1 on a full-height panel.
    s: f32,
    /// The screen row of depth 0 (the near kerb), as f32 so the projection is sub-pixel.
    y0: f32,
    /// The road's and the far sidewalk's depth, world units.
    road_d: f32,
    walk_d: f32,
    /// The far kerb's row, and the row the buildings stand on (the back of the sidewalk).
    kerb: i32,
    base: i32,
    /// A building's depth on screen: its roof's rows and its side face's columns.
    roof_dy: f32,
    roof_dx: f32,
    far: [Block; 64],
    far_n: usize,
    near: [Block; 64],
    near_n: usize,
    signs: [Sign; SIGNS],
    signs_n: usize,
    /// How far the trail rises over its length, px.
    rise: f32,
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
            y0: 0.0,
            road_d: 1.0,
            walk_d: 1.0,
            kerb: 0,
            base: 0,
            roof_dy: 0.0,
            roof_dx: 0.0,
            far: [Block::default(); 64],
            far_n: 0,
            near: [Block::default(); 64],
            near_n: 0,
            signs: [Sign::default(); SIGNS],
            signs_n: 0,
            rise: 0.0,
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

fn layout(w: i32, h: i32) -> Layout {
    let (ix0, iy0, ix1, iy1) = (3, 4, w - 3, h - 4);
    let (iw, ih) = (ix1 - ix0, iy1 - iy0);
    let mut l = Layout::default();
    let small = h < 48;
    l.s = (ih as f32 / 52.0).clamp(0.6, 1.0);
    // The street, from the bottom up: a 2-row near kerb, the road, the sidewalk.
    l.y0 = (iy1 - 2) as f32;
    let road_px = (ih as f32 * ROAD_FRAC).round();
    let walk_px = if small { 2.0 } else { 3.0 };
    l.road_d = road_px / ZY;
    l.walk_d = walk_px / ZY;
    l.kerb = (l.y0 - road_px).round() as i32;
    l.base = (l.y0 - road_px - walk_px).round() as i32;
    l.roof_dy = if small { 2.0 } else { 3.0 };
    l.roof_dx = l.roof_dy / ZY * ZX;
    let sky_h = (l.base - iy0).max(1);
    l.rise = ih as f32 * 0.24;

    // ---- the skyline: a constant seed, so the city is the same every run ----
    let mut s: u64 = 0x5eed_d71f_7000_0001;
    // Far: tall hazy towers, packed edge to edge.
    let mut x = ix0 - 6;
    while x < ix1 && l.far_n < l.far.len() {
        let bw = 5 + (splitmix(&mut s) % 9) as i32;
        let bh = (sky_h as f32 * (0.40 + 0.38 * unit(&mut s))).round() as i32;
        l.far[l.far_n] = Block { x, w: bw, h: bh };
        l.far_n += 1;
        x += bw;
    }
    // Near: lower black blocks, with the odd gap to the far row behind.
    let mut x = ix0 - 4;
    while x < ix1 && l.near_n < l.near.len() {
        let bw = 7 + (splitmix(&mut s) % 12) as i32;
        let bh = (sky_h as f32 * (0.20 + 0.36 * unit(&mut s))).round() as i32;
        l.near[l.near_n] = Block { x, w: bw, h: bh };
        l.near_n += 1;
        x += bw + if splitmix(&mut s).is_multiple_of(4) { 2 + (splitmix(&mut s) % 4) as i32 } else { 0 };
    }
    // Signs on the near blocks: a vertical blade down one side of the front, or a bar on the roof.
    for i in 0..l.near_n {
        if l.signs_n >= SIGNS {
            break;
        }
        let b = l.near[i];
        let r = unit(&mut s);
        if r > 0.62 || b.h < 6 {
            continue;
        }
        let top = l.base - b.h;
        let (sx, sy, sw, sh) = if r < 0.34 {
            // A blade: 2 px wide, 3-6 tall, hanging off one side just under the roof.
            let tall = 3 + (splitmix(&mut s) % 4) as i32;
            let tall = tall.min(b.h - 3).max(2);
            let side = if splitmix(&mut s).is_multiple_of(2) { b.x + 1 } else { b.x + b.w - 3 };
            (side, top + 2, 2, tall)
        } else {
            // A roof bar: 2 tall, 3-8 wide, standing on the roof's middle (up and right of the front).
            let bw = (3 + (splitmix(&mut s) % 6) as i32).min(b.w - 2).max(2);
            let back = (l.roof_dy * 0.5).round() as i32;
            let shift = (l.roof_dx * 0.5).round() as i32;
            (b.x + 1 + shift + (splitmix(&mut s) % (b.w - bw - 1).max(1) as u64) as i32, top - back - 2, bw, 2)
        };
        if sy < iy0 + 1 {
            continue;
        }
        l.signs[l.signs_n] = Sign {
            x: sx,
            y: sy,
            w: sw,
            h: sh,
            flicker: splitmix(&mut s).is_multiple_of(3),
            rate: 1.0 + unit(&mut s),
            phase: unit(&mut s),
            hot: splitmix(&mut s).is_multiple_of(4),
        };
        l.signs_n += 1;
    }

    // ---- the gauge (top right) and its readout ----
    l.show_gauge = w >= GAUGE_MIN_W;
    l.gauge = (ix1 - 3 - GAUGE_R, iy0 + 2 + GAUGE_R);
    l.angle_text = (l.gauge.0 - GAUGE_R - 5 - text_w("ANGLE 00", 1), l.gauge.1 - 5);

    // ---- DRIFT!, centred in the sky ----
    let tw = text_w("DRIFT!", 2);
    l.drift_text = (ix0 + (iw - tw) / 2, iy0 + (sky_h - 10) / 2);
    l
}

/// World (x, y, z) to screen, with depth 0 on row `y0`.
fn project(x: f32, y: f32, z: f32, y0: f32) -> (f32, f32) {
    (x + z * ZX, y0 - z * ZY - y * YY)
}

/// Fills a convex polygon given in float pixel coordinates, offset by `off`: a pixel is in when its
/// centre is, so a slowly turning shape steps a pixel at a time rather than jumping with rounded
/// vertices. Rows above `min_row` are left alone.
fn fill_convex(c: &mut Canvas, pts: &[(f32, f32)], off: (f32, f32), min_row: i32, col: Rgba) {
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
    let last = ((hi - 0.5).ceil() as i32 - 1).min(c.height() - 1);
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

/// A 1 px line from `a` to `b` with `a` itself left out, so a chain of segments paints each joint
/// once. Segments longer than 64 px are skipped: on a chain that only means a jump (a wrap).
fn trace(c: &mut Canvas, a: (f32, f32), b: (f32, f32), col: Rgba) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let n = dx.abs().max(dy.abs()).round();
    if !(1.0..=64.0).contains(&n) {
        return;
    }
    let n = n as i32;
    for k in 1..=n {
        let t = k as f32 / n as f32;
        c.fill_rect((a.0 + dx * t).floor() as i32, (a.1 + dy * t).floor() as i32, 1, 1, col);
    }
}

/// Twice the signed area of a screen polygon (y down). Negative is counter-clockwise on screen, which
/// is what an outward-wound face turned toward the viewer projects to.
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
/// (`cs`, `sn` of the yaw, counter-clockwise from +x toward +z), the world scale and the projection's
/// depth-0 row.
#[derive(Clone, Copy)]
struct Pose {
    x: f32,
    z: f32,
    cs: f32,
    sn: f32,
    s: f32,
    y0: f32,
}

impl Pose {
    fn new(x: f32, z: f32, yaw: f32, s: f32, y0: f32) -> Self {
        Pose { x, z, cs: yaw.cos(), sn: yaw.sin(), s, y0 }
    }
    /// Local (u, v) on the ground to world (x, z).
    fn at(&self, u: f32, v: f32) -> (f32, f32) {
        (self.x + self.s * (u * self.cs - v * self.sn), self.z + self.s * (u * self.sn + v * self.cs))
    }
    fn screen(&self, p: (f32, f32, f32)) -> (f32, f32) {
        let (x, z) = self.at(p.0, p.1);
        project(x, self.s * p.2, z, self.y0)
    }
    /// A local direction (u, v, y) to world (x, y, z).
    fn turn(&self, n: (f32, f32, f32)) -> (f32, f32, f32) {
        (n.0 * self.cs - n.1 * self.sn, n.2, n.0 * self.sn + n.1 * self.cs)
    }
    /// The pose of a part at local (u, v), turned `dyaw` further.
    fn child(&self, u: f32, v: f32, dyaw: f32) -> Pose {
        let (x, z) = self.at(u, v);
        let (c, s) = (dyaw.cos(), dyaw.sin());
        Pose { x, z, cs: self.cs * c - self.sn * s, sn: self.sn * c + self.cs * s, s: self.s, y0: self.y0 }
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

fn wheel(out: &mut Shapes, pose: &Pose, u: f32, v: f32, steer: f32, paint: &Paint) {
    let (w, plan, _) = wheel_parts(pose, u, v, steer);
    solid(out, &w, &plan, &plan, (0.0, WHEEL.2), Mat::Tyre, &[Mat::Tyre], paint);
    // The rim, just proud of the outer face.
    let hw = WHEEL.1 + 0.02;
    let rim = if v > 0.0 {
        [(0.9, hw, 0.9), (-0.9, hw, 0.9), (-0.9, hw, 2.5), (0.9, hw, 2.5)]
    } else {
        [(-0.9, -hw, 0.9), (0.9, -hw, 0.9), (0.9, -hw, 2.5), (-0.9, -hw, 2.5)]
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

/// The car's visible polygons in painter's order: the wheels on the far side, the body, its lamps,
/// the wing and the cabin (the wing after the cabin when the tail faces the viewer), the near wheels.
fn car_shapes(pose: &Pose, steer: f32, paint: &Paint) -> Shapes {
    let mut out = Shapes::new();
    let wheels = [(AXLE_F, TRACK, steer), (AXLE_F, -TRACK, steer), (AXLE_R, TRACK, 0.0), (AXLE_R, -TRACK, 0.0)];
    let near = wheels.map(|(u, v, st)| {
        let (w, _, outer) = wheel_parts(pose, u, v, st);
        faces_viewer(&w, &outer)
    });
    for (i, &(u, v, st)) in wheels.iter().enumerate() {
        if !near[i] {
            wheel(&mut out, pose, u, v, st, paint);
        }
    }
    solid(&mut out, pose, &BODY, &BODY, BODY_Y, Mat::Body, &[Mat::Body], paint);
    let (va, vb) = LAMP_V;
    let (ya, yb) = LAMP_Y;
    let (uf, ur) = (BODY[0].0 + 0.02, BODY[4].0 - 0.02);
    for (lo, hi) in [(va, vb), (-vb, -va)] {
        face(&mut out, pose, &[(uf, lo, ya), (uf, hi, ya), (uf, hi, yb), (uf, lo, yb)], Mat::Head, paint);
        face(&mut out, pose, &[(ur, hi, ya), (ur, lo, ya), (ur, lo, yb), (ur, hi, yb)], Mat::Tail, paint);
    }
    let (r0, r1) = (BODY[4], BODY[5]);
    let tail_seen = faces_viewer(
        pose,
        &[(r0.0, r0.1, BODY_Y.0), (r1.0, r1.1, BODY_Y.0), (r1.0, r1.1, BODY_Y.1), (r0.0, r0.1, BODY_Y.1)],
    );
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
        if near[i] {
            wheel(&mut out, pose, u, v, st, paint);
        }
    }
    out
}

/// One smoke position in the ring: on the road (world x, z), and a small fixed jitter drawn when it
/// was laid.
#[derive(Clone, Copy, Default)]
struct Puff {
    x: f32,
    z: f32,
    jy: f32,
    /// A radius jitter (-1, 0, +1 px) so the plume billows rather than drawing a smooth tube.
    jr: i32,
}

/// One skid-mark position: on the road, and how hard the tyre was sliding (0 = no mark).
#[derive(Clone, Copy, Default)]
struct Mark {
    x: f32,
    z: f32,
    a: f32,
}

/// Which layers draw - a test hook, so the car, the smoke and the marks can be measured on their own.
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
    /// Finds the kicks the car flicks on.
    onset: Flux,
    elapsed_s: f32,
    /// The wander's phase and the motion's eased amplitude (0..1).
    phase: f32,
    amp: f32,
    /// The car's centre: x in camera space (the world scrolls past it), z across the road; and its
    /// lateral speed, units per second.
    car_x: f32,
    car_z: f32,
    vz: f32,
    /// Which way the car is turning (+1 toward the far kerb), and how long since it last flicked.
    side: f32,
    since_flip_ms: f32,
    /// The body's slip angle against its path (degrees, signed) and its heading (radians).
    slip: f32,
    yaw: f32,
    /// The drift's spin (radians) and its direction.
    spin: f32,
    spin_dir: f32,
    /// Where the car was when the sweep began.
    sweep_from: f32,
    /// The world's scroll, wrapped at `SCROLL_WRAP` (scaled).
    dist: f32,
    ring: [Puff; TRAIL],
    head: usize,
    marks: [[Mark; MARKS]; 2],
    mark_head: usize,
    ring_live: bool,
    layout: Layout,
    layout_dim: (i32, i32),
    /// The baked scene. Keyed on size and colours.
    bg: Canvas,
    bg_key: u64,
    rng: u64,
    only: Only,
    /// Flicks so far, and how many of them landed on a kick - for the tests.
    #[cfg(test)]
    flips: u32,
    #[cfg(test)]
    kick_flips: u32,
}

impl Default for Drift {
    fn default() -> Self {
        Drift {
            flourish: Default::default(),
            hit: Default::default(),
            onset: Default::default(),
            elapsed_s: 0.0,
            phase: 0.0,
            amp: 0.0,
            car_x: 0.0,
            car_z: f32::NAN,
            vz: 0.0,
            side: 1.0,
            since_flip_ms: 0.0,
            slip: 0.0,
            yaw: 0.0,
            spin: 0.0,
            spin_dir: 1.0,
            sweep_from: 0.0,
            dist: 0.0,
            ring: [Puff::default(); TRAIL],
            head: 0,
            marks: [[Mark::default(); MARKS]; 2],
            mark_head: 0,
            ring_live: false,
            layout: Layout::default(),
            layout_dim: (0, 0),
            bg: Canvas::new(1, 1),
            bg_key: 0,
            rng: 0xbb67_ae85_84ca_a73b,
            only: Only::All,
            #[cfg(test)]
            flips: 0,
            #[cfg(test)]
            kick_flips: 0,
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

/// FNV-1a over the strings/values the baked background depends on. No allocation.
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

/// The asphalt, the sidewalk and the near kerb's colours for a colourway.
fn street_colours(t: &Theme) -> (Rgba, Rgba) {
    let panel = Rgba::from_hex(&t.panel, 1.0);
    let edge = Rgba::from_hex(&t.edge, 1.0);
    (Rgba::lerp_linear(panel, edge, 0.62), Rgba::lerp_linear(panel, edge, 0.95))
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

/// Whether a flickering sign is lit at time `e`: mostly on, with a stutter off (off, on, off) at the
/// start of each cycle.
fn sign_on(s: &Sign, e: f32) -> bool {
    if !s.flicker {
        return true;
    }
    let f = (e * s.rate + s.phase).fract();
    !((0.0..0.12).contains(&f) || (0.17..0.25).contains(&f))
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

/// An iso box in the skyline: its side face and roof as parallelograms, then the front over them.
#[allow(clippy::too_many_arguments)]
fn iso_block(bg: &mut Canvas, b: &Block, base: i32, dx: f32, dy: f32, front: Rgba, side: Rgba, roof: Rgba) {
    let (x0, x1) = (b.x as f32, (b.x + b.w) as f32);
    let (top, bot) = ((base - b.h) as f32, base as f32);
    fill_convex(bg, &[(x1, top), (x1 + dx, top - dy), (x1 + dx, bot - dy), (x1, bot)], (0.0, 0.0), 0, side);
    fill_convex(bg, &[(x0, top), (x1, top), (x1 + dx, top - dy), (x0 + dx, top - dy)], (0.0, 0.0), 0, roof);
    bg.fill_rect(b.x, base - b.h, b.w, b.h, front);
}

impl Drift {
    #[cfg(test)]
    fn only_for_test(&mut self, only: Only) {
        self.only = only;
    }

    /// The car's centre on screen, x - for the tests.
    #[cfg(test)]
    fn car_screen_x(&self) -> f32 {
        self.car_x + self.car_z * ZX
    }

    fn next_rng(&mut self) -> u64 {
        splitmix(&mut self.rng)
    }

    /// Rebuilds the geometry on a size change and the baked scene on a size or colour change.
    fn resize(&mut self, w: i32, h: i32, t: &Theme) {
        if self.layout_dim != (w, h) {
            self.layout = layout(w, h);
            self.layout_dim = (w, h);
            self.ring_live = false;
            self.car_z = self.layout.road_d * 0.5;
            self.vz = 0.0;
            self.dist = 0.0;
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
        let panel = Rgba::from_hex(&t.panel, 1.0);
        let lit = Rgba::from_hex(&t.lit, 1.0);
        let hot = Rgba::from_hex(&t.hot, 1.0);
        let edge = Rgba::from_hex(&t.edge, 1.0);
        let neon = neon_of(t);
        let (ix0, iy0, ix1, iy1) = (3, 4, w - 3, h - 4);
        let iw = ix1 - ix0;
        let base = l.base;
        let sky_h = (base - iy0).max(1) as f32;

        // The sky: panel at the top, toward edge and a wash of neon at the horizon.
        for y in iy0..base {
            let f = (y - iy0) as f32 / sky_h;
            let col = Rgba::lerp_linear(panel, edge, 0.85 * f.powf(1.6));
            bg.fill_rect(ix0, y, iw, 1, Rgba::lerp_linear(col, neon, 0.16 * f.powi(3)));
        }
        // Far towers: hazy iso boxes a shade off the horizon sky, with a sparse dim window here and
        // there.
        let far = Rgba::lerp_linear(panel, edge, 0.62);
        let far_side = Rgba::lerp_linear(far, panel, 0.35);
        let far_roof = Rgba::lerp_linear(far, edge, 0.6);
        let far_win = Rgba::lerp_linear(far, lit, 0.22);
        let (dx, dy) = (l.roof_dx, l.roof_dy);
        let mut s: u64 = 0x0f0f_1a2b_3c4d_5e6f;
        for b in &l.far[..l.far_n] {
            iso_block(bg, b, base, dx * 0.7, dy * 0.7, far, far_side, far_roof);
            let mut y = base - b.h + 2;
            while y < base - 3 {
                let mut x = b.x + 1;
                while x < b.x + b.w - 1 {
                    if splitmix(&mut s).is_multiple_of(11) {
                        bg.fill_rect(x, y, 1, 1, far_win);
                    }
                    x += 2;
                }
                y += 3;
            }
        }
        // Near blocks: black iso boxes, a lit roof, and a grid of windows with a scatter lit.
        let near = Rgba::lerp_linear(panel, BLACK, 0.5);
        let near_side = Rgba::lerp_linear(near, edge, 0.4);
        let roof = Rgba::lerp_linear(near, edge, 0.85);
        let win = Rgba::lerp_linear(edge, lit, 0.5);
        let win_warm = Rgba::lerp_linear(edge, WARM_WINDOW, 0.6);
        let win_dark = Rgba::lerp_linear(near, edge, 0.35);
        let win_w = if h >= 48 { 2 } else { 1 };
        for b in &l.near[..l.near_n] {
            iso_block(bg, b, base, dx, dy, near, near_side, roof);
            let mut y = base - b.h + 2;
            while y < base - 2 {
                let mut x = b.x + 2;
                while x + win_w < b.x + b.w {
                    let r = splitmix(&mut s) % 100;
                    let col = if r < 16 {
                        win
                    } else if r < 22 {
                        win_warm
                    } else {
                        win_dark
                    };
                    bg.fill_rect(x, y, win_w, 1, col);
                    x += win_w + 2;
                }
                y += 3;
            }
        }
        // The signs' unlit tubes (the lit tube is drawn per frame).
        for sg in &l.signs[..l.signs_n] {
            let base_c = if sg.hot { hot } else { neon };
            bg.fill_rect(sg.x, sg.y, sg.w, sg.h, Rgba::lerp_linear(near, base_c, 0.22));
        }
        // The street: the sidewalk, the far kerb's bright edge, the asphalt (a little neon in it toward
        // the far side), the near kerb.
        let (road, walk) = street_colours(t);
        bg.fill_rect(ix0, base, iw, l.kerb - base, walk);
        bg.fill_rect(ix0, l.kerb, iw, 1, Rgba::lerp_linear(edge, lit, 0.35));
        let y0 = l.y0 as i32;
        let road_rows = (y0 - l.kerb - 1).max(1) as f32;
        for y in l.kerb + 1..y0 {
            let f = 1.0 - (y - l.kerb - 1) as f32 / road_rows;
            bg.fill_rect(ix0, y, iw, 1, Rgba::lerp_linear(road, neon, 0.07 * f * f));
        }
        bg.fill_rect(ix0, y0, iw, 1, Rgba::lerp_linear(walk, lit, 0.18));
        bg.fill_rect(ix0, y0 + 1, iw, iy1 - y0 - 1, Rgba::lerp_linear(panel, BLACK, 0.2));

        // The gauge's ring and ticks.
        if l.show_gauge {
            let (gx, gcy) = l.gauge;
            let ring = Rgba::lerp_linear(panel, lit, 0.45);
            let r = GAUGE_R as f32;
            for y in gcy - GAUGE_R - 1..=gcy {
                for x in gx - GAUGE_R - 1..=gx + GAUGE_R + 1 {
                    let d = ((x - gx) as f32).hypot((y - gcy) as f32);
                    if (d - r).abs() <= 0.5 {
                        bg.fill_rect(x, y, 1, 1, ring);
                    }
                }
            }
            bg.fill_rect(gx - GAUGE_R - 1, gcy + 1, 2 * GAUGE_R + 3, 1, Rgba::lerp_linear(panel, lit, 0.25));
            // The centre tick.
            bg.fill_rect(gx, gcy - GAUGE_R + 1, 1, 1, Rgba::lerp_linear(panel, lit, 0.7));
        }
    }

    /// Advances the drift: the wander, the flicks, the lateral spring, the slip and the heading.
    /// Returns whether the car wrapped round the panel this frame.
    #[allow(clippy::too_many_arguments)]
    fn steer(&mut self, dt: f32, rms_norm: f32, kick: bool, drifting: bool, since_ms: f32, iw: f32, ix0: f32) -> bool {
        let (s, road_d) = (self.layout.s, self.layout.road_d);
        let k = (dt / SWING_EASE_MS).min(1.0);
        self.amp += (rms_norm - self.amp) * k;
        if !self.amp.is_finite() {
            self.amp = 0.0;
        }
        let amp = self.amp.clamp(0.0, 1.0);

        // The flick: on a kick once the last one has settled, or on its own after a while.
        self.since_flip_ms = (self.since_flip_ms + dt).min(1.0e6);
        let due = (kick && self.since_flip_ms >= MIN_FLIP_MS) || self.since_flip_ms >= MAX_FLIP_MS;
        if !drifting && amp > FLIP_AMP && due {
            self.side = -self.side;
            self.since_flip_ms = 0.0;
            #[cfg(test)]
            {
                self.flips += 1;
                if kick {
                    self.kick_flips += 1;
                }
            }
        }

        // Across the road: a critically damped spring toward this side, in small steps.
        let z_mid = road_d * 0.5;
        let z_to = z_mid + self.side * amp * Z_SWING * road_d;
        let steps = ((dt / 8.0).ceil() as i32).clamp(1, 25);
        let h = dt / 1000.0 / steps as f32;
        for _ in 0..steps {
            let a = LAT_OMEGA * LAT_OMEGA * (z_to - self.car_z) - 2.0 * LAT_OMEGA * self.vz;
            self.vz += a * h;
            self.car_z += self.vz * h;
        }
        if !self.car_z.is_finite() || !self.vz.is_finite() {
            self.car_z = z_mid;
            self.vz = 0.0;
        }

        // The slip: into the turn, flicking fast to the new lock; pinned at full lock in the drift.
        let reach = if drifting { 1.0 } else { amp };
        let slip_to = self.side * SLIP_MAX * reach;
        self.slip += (slip_to - self.slip) * (1.0 - (-dt / FLICK_MS).exp());
        if !self.slip.is_finite() {
            self.slip = 0.0;
        }

        // Along the screen: the wander, or the drift's sweep round the panel.
        self.phase = (self.phase + TAU * dt / 1000.0 / SWING_S).rem_euclid(TAU);
        let mid = ix0 + iw * 0.5 - z_mid * ZX;
        let pend_x = mid + amp * SWING_FRAC * iw * self.phase.sin();
        let pend_v = amp * SWING_FRAC * iw * self.phase.cos() * TAU / SWING_S;
        let reach_px = REACH * s * CAR_SCALE;
        let lap = iw + 2.0 * reach_px;
        let old_x = self.car_x;
        let sweeping = drifting && since_ms < SWEEP_MS;
        if sweeping {
            // Out past the right edge and back in from the left, on a torus the car can leave fully.
            let start = ix0 - reach_px - z_mid * ZX;
            let from = self.sweep_from;
            let dist = lap + (pend_x - from);
            let u = from - start + dist * ease(since_ms / SWEEP_MS);
            self.car_x = start + u.rem_euclid(lap);
            self.spin = self.spin_dir * TAU * ease(since_ms / SWEEP_MS);
        } else {
            self.car_x = pend_x;
            self.spin = 0.0;
        }
        let speed = iw * (SCROLL_BASE + SCROLL_RMS * rms_norm);
        let heading = if sweeping { 0.0 } else { self.vz.atan2(speed + pend_v) };
        self.yaw = heading + self.slip.to_radians() + self.spin;
        (self.car_x - old_x).abs() > lap * 0.5
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
            self.sweep_from = self.car_x;
            self.spin_dir = self.side;
        }
        let drifting = env > 0.0;
        // Full lock the moment the sweep starts, easing back as the hit ends.
        let boost = if drifting { (env / 0.35).min(1.0) } else { 0.0 };

        // ---- the car ----
        let kick = self.onset.update(&d.levels, dt, ONSET_RATIO, ONSET_REFRACTORY_MS) && bass >= KICK_BASS;
        let wrapped = self.steer(dt, rms_norm, kick, drifting, since_ms, iw as f32, ix0 as f32);
        let l = &self.layout;
        let (s, y0) = (l.s, l.y0);
        let pose = Pose::new(self.car_x, self.car_z, self.yaw, s * CAR_SCALE, y0);

        // ---- the world scrolls; the trail and the marks ride it ----
        let step = iw as f32 * (SCROLL_BASE + SCROLL_RMS * rms_norm.max(boost)) * dt / 1000.0;
        let rear = pose.at(AXLE_R, 0.0);
        let wheels = [pose.at(AXLE_R, TRACK), pose.at(AXLE_R, -TRACK)];
        if !self.ring_live {
            for (i, p) in self.ring.iter_mut().enumerate() {
                *p = Puff { x: rear.0 - step.max(1.0) * i as f32, z: rear.1, jy: 0.0, jr: 0 };
            }
            self.head = 0;
            self.marks = [[Mark::default(); MARKS]; 2];
            self.mark_head = 0;
            self.ring_live = true;
        } else {
            for p in self.ring.iter_mut() {
                p.x -= step;
            }
            self.head = (self.head + 1) % TRAIL;
            let rnd = self.next_rng();
            let jy = (rnd % 5) as f32 - 2.0;
            let jr = ((rnd >> 8) % 3) as i32 - 1;
            self.ring[self.head] = Puff { x: rear.0, z: rear.1, jy: jy * 0.6, jr };

            for side in self.marks.iter_mut() {
                for m in side.iter_mut() {
                    m.x -= step;
                }
            }
            self.mark_head = (self.mark_head + 1) % MARKS;
            let a = if wrapped {
                0.0
            } else if drifting {
                1.0
            } else {
                ((self.slip.abs() - MARK_FROM) / MARK_SPAN).clamp(0.0, 1.0)
            };
            for (k, &(x, z)) in wheels.iter().enumerate() {
                self.marks[k][self.mark_head] = Mark { x, z, a };
            }
        }
        self.dist = (self.dist + step).rem_euclid(SCROLL_WRAP * s);

        let lit = Rgba::from_hex(&t.lit, 1.0);
        let hot = Rgba::from_hex(&t.hot, 1.0);
        let neon = neon_of(t);
        let l = &self.layout;
        let only = self.only;
        let (road_d, walk_d, kerb, base) = (l.road_d, l.walk_d, l.kerb, l.base);
        let (road, walk) = street_colours(t);

        if only == Only::All {
            // ---- the baked scene ----
            c.copy_region(&self.bg, (ix0, iy0), (ix0, iy0), iw, ih);

            // ---- neon, and its streak on the wet road ----
            let flash = drifting && since_ms < FLASH_MS;
            let flash_on = flash && (since_ms / 1000.0 * FLASH_HZ).fract() < 0.5;
            let halo_a = 0.14 + 0.22 * bass;
            let road_px = (y0 - kerb as f32).max(1.0);
            for sg in &l.signs[..l.signs_n] {
                if !flash && !sign_on(sg, e) {
                    continue;
                }
                let base_c = if sg.hot { hot } else { neon };
                let (col, ha) = if flash_on { (Rgba::lerp_linear(base_c, WHITE, 0.55), 0.5) } else { (base_c, halo_a) };
                c.fill_rect(sg.x - 1, sg.y - 1, sg.w + 2, sg.h + 2, with_alpha(base_c, ha));
                c.fill_rect(sg.x, sg.y, sg.w, sg.h, col);
                // Mirrored in the ground at the buildings' base and stretched, the way a wet street
                // smears a light; only the part on the asphalt shows.
                let (r0, r1) = (base + (1.6 * (base - sg.y - sg.h) as f32) as i32, base + (1.6 * (base - sg.y) as f32) as i32);
                for r in r0.max(kerb + 1)..r1.min(y0 as i32) {
                    let f = 1.0 - (r - kerb) as f32 / road_px;
                    let f = f.max(0.0);
                    c.fill_rect(sg.x, r, sg.w, 1, with_alpha(base_c, 0.10 * f * f));
                }
            }

            // ---- the street's scrolling features ----
            let lo_x = ix0 as f32 - (road_d + walk_d) * ZX - 8.0;
            let hi_x = ix1 as f32 + 8.0;
            // Sidewalk seams: slanted, one every SEAM.
            let seam = Rgba::lerp_linear(walk, BLACK, 0.5);
            let period = SEAM * s;
            let mut x = lo_x - (self.dist % period);
            while x < hi_x {
                let a = project(x, 0.0, road_d, y0);
                let b = project(x, 0.0, road_d + walk_d, y0);
                trace(c, (a.0, a.1 - 0.5), b, seam);
                x += period;
            }
            // The centre line.
            let dash = Rgba::lerp_linear(road, lit, 0.28);
            let zc = road_d * 0.5;
            let row = project(0.0, 0.0, zc, y0).1.floor() as i32;
            let period = DASH * s;
            let mut x = lo_x - (self.dist % period);
            while x < hi_x {
                c.fill_rect((x + zc * ZX).round() as i32, row, (DASH_ON * s).round() as i32, 1, dash);
                x += period;
            }
            // Zebra crossings: slanted stripes across the road.
            let zebra = Rgba::lerp_linear(road, lit, 0.24);
            let period = CROSSING * s;
            let (z0, z1) = (road_d * 0.10, road_d * 0.90);
            let mut x = lo_x - (self.dist % period) + period * 0.6;
            while x < hi_x + period {
                for j in 0..STRIPES {
                    let sx = x + j as f32 * 2.0 * STRIPE_W * s;
                    let sw = STRIPE_W * s;
                    let pts = [
                        project(sx, 0.0, z0, y0),
                        project(sx + sw, 0.0, z0, y0),
                        project(sx + sw, 0.0, z1, y0),
                        project(sx, 0.0, z1, y0),
                    ];
                    if pts[1].0 >= ix0 as f32 && pts[3].0 < hi_x {
                        fill_convex(c, &pts, (0.0, 0.0), kerb + 1, zebra);
                    }
                }
                x += period;
            }
        }

        // ---- skid marks ----
        if only == Only::All {
            let mark = Rgba::lerp_linear(road, BLACK, 0.85);
            for side in &self.marks {
                let mut prev: Option<((f32, f32), f32)> = None;
                for i in 0..MARKS {
                    let m = side[(self.mark_head + MARKS - i) % MARKS];
                    let p = project(m.x, 0.0, m.z, y0);
                    if let Some((q, qa)) = prev {
                        let a = m.a.min(qa);
                        if a > 0.0 {
                            let fade = 1.0 - (i as f32 / MARKS as f32).powf(1.5);
                            trace(c, q, p, with_alpha(mark, 0.9 * a * fade));
                        }
                    }
                    prev = Some((p, m.a));
                }
            }

            // ---- the headlight beam and the shadow ----
            let beam = Rgba::lerp_linear(lit, WHITE, 0.6);
            let nose = BODY[0].0;
            for (len, spread, a) in [(36.0, 11.0, 0.03), (24.0, 7.5, 0.035), (12.0, 4.8, 0.045)] {
                let pts = [
                    pose.screen((nose, -3.0, 0.0)),
                    pose.screen((nose, 3.0, 0.0)),
                    pose.screen((nose + len, spread, 0.0)),
                    pose.screen((nose + len, -spread, 0.0)),
                ];
                fill_convex(c, &pts, (0.0, 0.0), kerb, with_alpha(beam, a));
            }
            let mut shadow = [(0.0f32, 0.0f32); 8];
            for (i, p) in BODY.iter().enumerate() {
                shadow[i] = pose.screen((p.0 * 1.06, p.1 * 1.12, 0.0));
            }
            fill_convex(c, &shadow, (0.0, 0.5), 0, with_alpha(BLACK, 0.45));
        }

        // ---- the tyre smoke: the meter ----
        if only != Only::Car {
            let r_max = 1.0 + PUFF_GAIN * s;
            let mut radius = [0i32; TRAIL];
            for (i, r) in radius.iter_mut().enumerate() {
                let base_r = 1.0 + lvl(d.levels[i]) * PUFF_GAIN * s;
                *r = (base_r + (r_max - base_r) * boost).round().max(1.0) as i32;
            }
            let tint = Rgba::lerp_linear(WHITE, hot, 0.7 * boost);
            let ghost = if t.ghost.is_finite() { t.ghost.clamp(0.05, 0.6) } else { 0.3 };
            let lift = l.rise / (TRAIL - 1) as f32;
            // Oldest first, so the fresh puffs at the car sit on top.
            for i in (0..TRAIL).rev() {
                let p = self.ring[(self.head + TRAIL - i) % TRAIL];
                let r = if radius[i] >= 3 { radius[i] + p.jr } else { radius[i] };
                let (sx, gy) = project(p.x, 0.0, p.z, y0);
                let x = sx.round() as i32;
                if x + r < ix0 || x - r >= ix1 {
                    continue;
                }
                let cy = (gy - 1.0 - r as f32 * 0.55 - lift * i as f32 + p.jy).round() as i32;
                // Consecutive puffs overlap a lot, so each disc pass is a fraction of `ghost`: the core
                // of a lone puff lands near `ghost * 0.55`, and the fresh overlapping ones build toward
                // `ghost` and past it where the smoke is thick. Older puffs thin out.
                let a = ghost * (1.0 - 0.65 * i as f32 / (TRAIL - 1) as f32);
                let col = with_alpha(tint, a * 0.32);
                c.fill_circle(x, cy, r, col);
                if r >= 2 {
                    c.fill_circle(x, cy, r - 1, col);
                }
            }
        }

        // ---- the car ----
        if only != Only::Smoke {
            let paint = Paint::new(t);
            let steer = (-self.slip).clamp(-COUNTER_MAX, COUNTER_MAX).to_radians();
            let shapes = car_shapes(&pose, steer, &paint);
            let list = &shapes.s[..shapes.n];
            // A 1 px dark outline first, so the car reads against its own smoke and the street.
            for sh in list.iter().filter(|sh| sh.n >= 3) {
                for off in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
                    fill_convex(c, &sh.pts[..sh.n], off, 0, OUTLINE);
                }
            }
            for sh in list {
                if sh.n == 2 {
                    let (a, b) = (sh.pts[0], sh.pts[1]);
                    c.line(a.0.floor() as i32, a.1.floor() as i32, b.0.floor() as i32, b.1.floor() as i32, sh.col);
                } else {
                    fill_convex(c, &sh.pts[..sh.n], (0.0, 0.0), 0, sh.col);
                }
            }
            if only == Only::All {
                // The tail lamps' glow, when the tail faces the viewer.
                let (r0, r1) = (BODY[4], BODY[5]);
                let tail = [(r0.0, r0.1, BODY_Y.0), (r1.0, r1.1, BODY_Y.0), (r1.0, r1.1, BODY_Y.1), (r0.0, r0.1, BODY_Y.1)];
                if faces_viewer(&pose, &tail) {
                    let ym = (LAMP_Y.0 + LAMP_Y.1) * 0.5;
                    let vm = (LAMP_V.0 + LAMP_V.1) * 0.5;
                    for v in [vm, -vm] {
                        let (gx, gy) = pose.screen((r0.0 - 0.5, v, ym));
                        c.fill_circle(gx.round() as i32, gy.round() as i32, 2, with_alpha(hot, 0.22));
                    }
                }
            }
        }

        if only == Only::All {
            // ---- the steering gauge ----
            if l.show_gauge {
                let angle = wrap_deg(self.slip + self.spin.to_degrees());
                let (gx, gcy) = l.gauge;
                let th = (90.0 - angle.clamp(-45.0, 45.0) * 2.0).to_radians();
                let reach = (GAUGE_R - 1) as f32;
                let (nx, ny) = (gx as f32 + reach * th.cos(), gcy as f32 - reach * th.sin());
                c.line(gx, gcy, nx.round() as i32, ny.round() as i32, hot);
                c.fill_rect(gx, gcy, 1, 1, lit);
                let a = angle.abs().round().clamp(0.0, 99.0) as u32;
                let digits = [b'0' + (a / 10) as u8, b'0' + (a % 10) as u8];
                let ds = std::str::from_utf8(&digits).unwrap_or("00");
                let (tx, ty) = l.angle_text;
                text(c, tx, ty, "ANGLE", 1, Rgba::lerp_linear(panel, lit, 0.75));
                text(c, tx + text_w("ANGLE ", 1) + 1, ty, ds, 1, lit);
            }

            // ---- DRIFT! ----
            let tin = since_ms - TEXT_FROM_MS;
            if drifting && (0.0..TEXT_MS).contains(&tin) {
                let (tx, ty) = l.drift_text;
                let big_w = text_w("DRIFT!", 3);
                if tin < 20.0 && big_w + 4 <= iw && 15 + 2 <= base - iy0 {
                    // The slam: one frame at 3x.
                    let bx = ix0 + (iw - big_w) / 2;
                    let by = (ty - 2).max(iy0 + 1);
                    text_outlined(c, bx, by, "DRIFT!", 3, hot, OUTLINE);
                } else {
                    let shake = if tin < 100.0 { if (tin / 33.0) as i32 % 2 == 0 { 1 } else { -1 } } else { 0 };
                    text_outlined(c, tx + shake, ty, "DRIFT!", 2, hot, OUTLINE);
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
    /// Smoke pixels (drawn over the panel, smoke-only hook) within the columns `x0..x1`.
    fn smoke_px(c: &Canvas, t: &Theme, x0: i32, x1: i32) -> usize {
        let panel = Rgba::from_hex(&t.panel, 1.0);
        let mut n = 0;
        for y in 4..c.height() - 4 {
            for x in x0.max(3)..x1.min(c.width() - 3) {
                if drew_over_panel(c.get(x, y), panel) {
                    n += 1;
                }
            }
        }
        n
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

    /// Silence still shows the street: sky, skyline, windows, neon, the road, the car and a thin trail.
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

    /// The projection's winding test: the roof always faces the viewer and the floor never does; side
    /// on (heading +x) the viewer sees the front and the near side, not the tail or the far side.
    #[test]
    fn the_roof_faces_the_viewer_and_the_floor_does_not() {
        let roof: Vec<(f32, f32, f32)> = CABIN_HI.iter().map(|p| (p.0, p.1, CABIN_Y.1)).collect();
        let floor: Vec<(f32, f32, f32)> = BODY.iter().rev().map(|p| (p.0, p.1, BODY_Y.0)).collect();
        for k in 0..36 {
            let pose = Pose::new(100.0, 10.0, k as f32 * TAU / 36.0, 1.0, 50.0);
            assert!(faces_viewer(&pose, &roof), "heading {}: roof hidden", k * 10);
            assert!(!faces_viewer(&pose, &floor), "heading {}: floor shown", k * 10);
        }
        let pose = Pose::new(100.0, 10.0, 0.0, 1.0, 50.0);
        let side = |i: usize| {
            let (a, b) = (BODY[i], BODY[(i + 1) % 8]);
            faces_viewer(&pose, &[(a.0, a.1, 1.2), (b.0, b.1, 1.2), (b.0, b.1, 4.0), (a.0, a.1, 4.0)])
        };
        assert!(side(0), "front hidden");
        assert!(!side(4), "tail shown");
        assert!(side(6), "near side hidden");
        assert!(!side(2), "far side shown");
    }

    /// The model really turns: drawn side on it is long and low; turned 45 degrees either way it is
    /// taller on screen (its length now runs into the depth) and, nose toward the viewer, far shorter;
    /// and it shows several shades (a lit top, shaded sides, glass) rather than one flat sprite colour.
    #[test]
    fn the_car_model_turns_and_is_shaded() {
        let t = theme("drift-shibuya");
        let paint = Paint::new(&t);
        let draw_at = |deg: f32| {
            let mut c = Canvas::new(120, 70);
            let pose = Pose::new(50.0, 10.0, deg.to_radians(), 1.0, 60.0);
            let sh = car_shapes(&pose, 0.0, &paint);
            for s in &sh.s[..sh.n] {
                if s.n >= 3 {
                    fill_convex(&mut c, &s.pts[..s.n], (0.0, 0.0), 0, s.col);
                }
            }
            let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
            let mut cols: Vec<u32> = Vec::new();
            for y in 0..70 {
                for x in 0..120 {
                    let p = c.get(x, y);
                    if p.a > 0 {
                        x0 = x0.min(x);
                        x1 = x1.max(x);
                        y0 = y0.min(y);
                        y1 = y1.max(y);
                        let k = (p.r as u32) << 16 | (p.g as u32) << 8 | p.b as u32;
                        if !cols.contains(&k) {
                            cols.push(k);
                        }
                    }
                }
            }
            (x1 - x0 + 1, y1 - y0 + 1, cols.len())
        };
        let (w0, h0, n0) = draw_at(0.0);
        let (_, h45, n45) = draw_at(45.0);
        let (wm45, hm45, _) = draw_at(-45.0);
        assert!((22..=30).contains(&w0), "side on, {w0} px long");
        // Either way round, the length runs into the depth and the car stands taller.
        assert!(h45 >= h0 + 4 && hm45 >= h0 + 4, "turned 45, {h45} / {hm45} px tall vs {h0} side on");
        // Nose toward the viewer, the depth axis's lean cancels much of the length: nearly end on.
        assert!(wm45 + 6 <= w0, "turned -45, {wm45} px wide vs {w0} side on");
        assert!(n0 >= 4 && n45 >= 4, "only {n0} / {n45} distinct colours: not shaded");
    }

    /// Silent: the car straightens and cruises down the middle of the road, centred on the panel, not
    /// turning (its painted box is the same frame to frame).
    #[test]
    fn silent_the_car_cruises_straight_down_the_centre() {
        for id in IDS {
            let t = calm_theme(id);
            for (w, h) in [(380, 60), (128, 44)] {
                let mut fam = Drift::default();
                fam.only_for_test(Only::Car);
                let _ = frames(&mut fam, &t, w, h, 0.0, 60);
                let mut boxes = Vec::new();
                for _ in 0..12 {
                    let c = frames(&mut fam, &t, w, h, 0.0, 10);
                    boxes.push(bbox(&c, &t).expect("no car drawn"));
                }
                let (x0, y0, x1, y1) = boxes[0];
                assert!(((x0 + x1) / 2 - w / 2).abs() <= 3, "{id} {w}x{h}: parked off centre at {x0}..{x1}");
                for b in &boxes {
                    assert!((b.0 - x0).abs() <= 1 && (b.2 - x1).abs() <= 1, "{id} {w}x{h}: moved {b:?} vs {:?}", boxes[0]);
                    assert!((b.1 - y0).abs() <= 1 && (b.3 - y1).abs() <= 1, "{id} {w}x{h}: turned {b:?} vs {:?}", boxes[0]);
                }
                assert!(fam.slip.abs() < 0.5, "{id} {w}x{h}: slip {} at rest", fam.slip);
            }
        }
    }

    /// Loud with a beat: the body flicks from lock to lock several times, holds a big slip angle, and
    /// the painted car changes shape as it turns and moves across the road's depth - while staying in
    /// the panel.
    #[test]
    fn loud_the_car_flicks_from_lock_to_lock_across_the_road() {
        for id in IDS {
            let t = calm_theme(id);
            for (w, h) in [(380, 60), (128, 44)] {
                let mut fam = Drift::default();
                fam.only_for_test(Only::Car);
                let mut c = Canvas::new(w, h);
                let (mut sign_changes, mut last_sign, mut max_slip) = (0, 0.0f32, 0.0f32);
                let (mut h_lo, mut h_hi, mut cy_lo, mut cy_hi) = (i32::MAX, i32::MIN, i32::MAX, i32::MIN);
                for k in 0..420 {
                    c.clear();
                    fam.draw(&mut c, &t, &music(0.85, 0.22, k, 30));
                    if k < 60 {
                        continue;
                    }
                    let sg = fam.slip.signum();
                    if fam.slip.abs() > 5.0 {
                        if last_sign != 0.0 && sg != last_sign {
                            sign_changes += 1;
                        }
                        last_sign = sg;
                    }
                    max_slip = max_slip.max(fam.slip.abs());
                    let (_, y0, _, y1) = bbox(&c, &t).expect("no car drawn");
                    assert!(y0 >= 4 && y1 < h - 4, "{id} {w}x{h}: car outside the panel at rows {y0}..{y1}");
                    h_lo = h_lo.min(y1 - y0);
                    h_hi = h_hi.max(y1 - y0);
                    cy_lo = cy_lo.min(y0 + y1);
                    cy_hi = cy_hi.max(y0 + y1);
                }
                assert!(sign_changes >= 4, "{id} {w}x{h}: only {sign_changes} flicks in 6 s");
                assert!(max_slip >= 25.0, "{id} {w}x{h}: slip only reached {max_slip:.0} degrees");
                assert!(h_hi - h_lo >= 3, "{id} {w}x{h}: the car's height only varied {h_lo}..{h_hi} px");
                let road_px = (fam.layout.y0 - fam.layout.kerb as f32) as i32;
                assert!(
                    (cy_hi - cy_lo) / 2 >= road_px / 5,
                    "{id} {w}x{h}: the car only moved {} rows across a {road_px} px road",
                    (cy_hi - cy_lo) / 2
                );
            }
        }
    }

    /// The flicks are on the beat: with a kick every second (longer than the flick's minimum gap), most
    /// flicks land on a kick; with no beat at all the car still flicks, on its own timer.
    #[test]
    fn flicks_land_on_the_kicks() {
        let t = calm_theme("drift-shibuya");
        let mut fam = Drift::default();
        let mut c = Canvas::new(380, 60);
        for k in 0..480 {
            c.clear();
            fam.draw(&mut c, &t, &music(0.85, 0.22, k, 60));
        }
        assert!(fam.flips >= 5, "only {} flicks in 8 s", fam.flips);
        assert!(fam.kick_flips * 10 >= fam.flips * 8, "{} of {} flicks on a kick", fam.kick_flips, fam.flips);

        // Steady levels: no onsets, so the timer does it.
        let mut fam = Drift::default();
        let _ = frames(&mut fam, &t, 380, 60, 0.22, 400);
        assert!(fam.flips >= 2, "no beat: only {} flicks in 6.7 s", fam.flips);
        assert_eq!(fam.kick_flips, 0);
    }

    /// Skid marks: after a sliding passage the road behind the car carries dark marks (pixels darker
    /// than the baked road); after a silent one it carries none.
    #[test]
    fn skid_marks_trail_behind_a_sliding_car() {
        for id in IDS {
            let t = calm_theme(id);
            let (w, h) = (380, 60);
            let marks = |level: f32, rms: f32| {
                let mut fam = Drift::default();
                let c = frames_bands(&mut fam, &t, w, h, 0..64, level, rms, 300);
                let l = fam.layout;
                let right = (fam.car_screen_x() - REACH * l.s * CAR_SCALE) as i32;
                let mut n = 0;
                for y in l.kerb + 1..l.y0 as i32 {
                    for x in 3..right {
                        let (p, b) = (c.get(x, y), fam.bg.get(x, y));
                        let (lp, lb) = (p.r as i32 + p.g as i32 + p.b as i32, b.r as i32 + b.g as i32 + b.b as i32);
                        if lb - lp > 24 {
                            n += 1;
                        }
                    }
                }
                n
            };
            let sliding = marks(0.15, 0.22);
            assert!(sliding >= 40, "{id}: only {sliding} px of skid marks after a sliding passage");
            let silent = marks(0.0, 0.0);
            assert_eq!(silent, 0, "{id}: {silent} px of skid marks after silence");
        }
    }

    /// Bass-only vs treble-only, the car straight (rms 0): the smoke close behind the car (the first
    /// puffs, which are the bass bands) is far bigger on bass; the far end of the trail (the treble
    /// bands) is bigger on treble. Read off the painted smoke.
    #[test]
    fn the_smoke_trail_billows_with_the_spectrum() {
        for id in IDS {
            let t = calm_theme(id);
            for (w, h) in [(380, 60), (128, 44)] {
                let run = |bands: std::ops::Range<usize>| {
                    let mut fam = Drift::default();
                    fam.only_for_test(Only::Smoke);
                    let c = frames_bands(&mut fam, &t, w, h, bands, 0.9, 0.0, 90);
                    let rear = (fam.car_screen_x() + AXLE_R * fam.layout.s * CAR_SCALE).round() as i32;
                    (c, rear)
                };
                let (bass, rear) = run(0..16);
                let (treble, rear2) = run(48..64);
                assert_eq!(rear, rear2);
                // The first ~16 puffs sit within this many px behind the rear axle.
                let iw = (w - 6) as f32;
                let step = iw * SCROLL_BASE / 60.0;
                let near = (rear - (step * 14.0) as i32, rear + 9);
                let far = (rear - (step * 63.0) as i32, rear - (step * 48.0) as i32);
                let (bn, tn) = (smoke_px(&bass, &t, near.0, near.1), smoke_px(&treble, &t, near.0, near.1));
                assert!(bn >= 3 * tn.max(1), "{id} {w}x{h}: near the car bass {bn} px vs treble {tn} px");
                let (bf, tf) = (smoke_px(&bass, &t, far.0, far.1), smoke_px(&treble, &t, far.0, far.1));
                assert!(tf >= 2 * bf.max(1), "{id} {w}x{h}: at the trail's end treble {tf} px vs bass {bf} px");
            }
        }
    }

    /// A forced drift: within 900 ms the car's painted extent spans >= 60 % of the width, it spins a
    /// full turn on the way, and it lands back on its line (centred, at silence).
    #[test]
    fn drift_flourish_sweeps_and_spins_the_car_across() {
        for id in IDS {
            let t = theme(id);
            for (w, h) in [(380, 60), (190, 48), (128, 44)] {
                let mut fam = Drift::default();
                fam.only_for_test(Only::Car);
                // Silence: the forced drift fires regardless, and the car parks dead centre after.
                let _ = frames(&mut fam, &t, w, h, 0.0, 30);
                fam.flourish.force_next();
                let (mut lo, mut hi, mut spin) = (i32::MAX, i32::MIN, 0.0f32);
                for _ in 0..54 {
                    let c = frames(&mut fam, &t, w, h, 0.0, 1);
                    if let Some((a, _, b, _)) = bbox(&c, &t) {
                        lo = lo.min(a);
                        hi = hi.max(b);
                    }
                    spin = spin.max(fam.spin.abs());
                }
                let span = (hi - lo) as f32 / w as f32;
                assert!(span >= 0.6, "{id} {w}x{h}: the drift swept only {lo}..{hi} ({:.0}%)", span * 100.0);
                assert!(spin >= 0.95 * TAU, "{id} {w}x{h}: the drift only spun {:.0} degrees", spin.to_degrees());
                let c = frames(&mut fam, &t, w, h, 0.0, 10);
                let (a, _, b, _) = bbox(&c, &t).unwrap();
                assert!(((a + b) / 2 - w / 2).abs() <= 3, "{id} {w}x{h}: after the drift the car is at {a}..{b}");
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
                // DRIFT! at 2x: D R I F T ! are 48 glyph pixels, 192 canvas pixels; the car or smoke
                // may cross a few.
                assert!(during >= before + 4 * 44, "{id} {w}x{h}: only {during} hot px of DRIFT! (before: {before})");
                let _ = frames(&mut fam, &t, w, h, 0.1, 50);
                let after = count(&frames(&mut fam, &t, w, h, 0.1, 1));
                // A few `hot` pixels (a sign, the needle) can fall in the box; DRIFT! is 192.
                assert!(before <= 24, "{id} {w}x{h}: DRIFT! up before the drift ({before} hot px)");
                assert!(after <= 24, "{id} {w}x{h}: DRIFT! still up after the drift ({after} hot px)");
            }
        }
    }

    /// 380x60 draws the steering gauge and its readout in the top-right sky; 190x48 and 128x44 drop it.
    /// Counted in the gauge's own colours - its ring and the readout's `lit` digits - since a `hot`
    /// sign can stand in that corner of the skyline.
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

    /// Some neon flickers: across a second at rest, at least one sign's pixel is lit in one frame and
    /// dark in another.
    #[test]
    fn some_neon_flickers() {
        let t = theme("drift-shibuya");
        let (w, h) = (380, 60);
        let l = layout(w, h);
        let flick: Vec<Sign> = l.signs[..l.signs_n].iter().copied().filter(|s| s.flicker).collect();
        assert!(!flick.is_empty(), "no flickering sign");
        let mut fam = Drift::default();
        let mut on = vec![0usize; flick.len()];
        let mut off = vec![0usize; flick.len()];
        let panel = Rgba::from_hex(&t.panel, 1.0);
        for _ in 0..60 {
            let c = frames(&mut fam, &t, w, h, 0.0, 1);
            for (i, s) in flick.iter().enumerate() {
                let p = c.get(s.x, s.y);
                // A lit tube is the full neon; an unlit one is a dim tint of it.
                let bright = (p.r as i32 + p.g as i32 + p.b as i32) - (panel.r as i32 + panel.g as i32 + panel.b as i32);
                if bright > 250 {
                    on[i] += 1;
                } else {
                    off[i] += 1;
                }
            }
        }
        assert!(
            (0..flick.len()).any(|i| on[i] > 0 && off[i] > 0),
            "no sign flickered over a second: on {on:?} off {off:?}"
        );
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
        for v in [fam.car_x, fam.car_z, fam.vz, fam.slip, fam.yaw, fam.amp, fam.dist] {
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
    /// `-strip` dumps stack eight frames a quarter of a second apart, top to bottom, to show the
    /// motion in a still.
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
            for k in 0..(120 + 8 * 15) {
                c.clear();
                fam.draw(&mut c, &t, &music(level, rms, k, 30));
                if k >= 120 && (k - 120) % 15 == 14 {
                    let row = ((k - 120) / 15) as i32;
                    out.copy_region(&c, (0, 0), (0, row * h), w, h);
                }
            }
            out
        };
        for id in IDS {
            let short = &id["drift-".len()..];
            for (tag, level, rms, n) in [("calm", 0.3f32, 0.07f32, 250), ("loud", 0.85, 0.2, 266)] {
                let (_, c, _) = run(id, 380, 60, level, rms, n);
                write(format!("drift-{short}-{tag}"), &c);
            }
            // The drift ~250 ms in (mid-sweep, mid-spin, DRIFT! up) and ~500 ms in (the smoke wall).
            write(format!("drift-{short}-flourish"), &flourish(id, 380, 60, 15));
            write(format!("drift-{short}-flourish-late"), &flourish(id, 380, 60, 30));
        }
        write("drift-shibuya-loud-strip".into(), &strip("drift-shibuya", 380, 60, 0.85, 0.2));
        // The car alone on the asphalt at eight headings, as `draw` paints it (outline, then faces).
        for id in ["drift-shibuya", "drift-orange"] {
            let t = theme(id);
            let paint = Paint::new(&t);
            let (road, _) = street_colours(&t);
            let mut c = Canvas::new(8 * 48, 34);
            c.fill_rect(0, 0, 8 * 48, 34, road);
            for (k, deg) in [-60.0f32, -40.0, -20.0, 0.0, 20.0, 40.0, 60.0, 90.0].into_iter().enumerate() {
                let pose = Pose::new(k as f32 * 48.0 + 20.0, 6.0, deg.to_radians(), CAR_SCALE, 30.0);
                let sh = car_shapes(&pose, (-deg * 0.6).clamp(-COUNTER_MAX, COUNTER_MAX).to_radians(), &paint);
                for s in sh.s[..sh.n].iter().filter(|s| s.n >= 3) {
                    for off in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
                        fill_convex(&mut c, &s.pts[..s.n], off, 0, OUTLINE);
                    }
                }
                for s in &sh.s[..sh.n] {
                    if s.n == 2 {
                        let (a, b) = (s.pts[0], s.pts[1]);
                        c.line(a.0.floor() as i32, a.1.floor() as i32, b.0.floor() as i32, b.1.floor() as i32, s.col);
                    } else {
                        fill_convex(&mut c, &s.pts[..s.n], (0.0, 0.0), 0, s.col);
                    }
                }
            }
            write(format!("{id}-turntable"), &c);
        }
        write("drift-orange-calm-strip".into(), &strip("drift-orange", 380, 60, 0.3, 0.07));
        for (w, h) in [(190, 48), (128, 44)] {
            let (_, c, _) = run("drift-shibuya", w, h, 0.85, 0.2, 266);
            write(format!("drift-shibuya-{w}x{h}"), &c);
            write(format!("drift-shibuya-{w}x{h}-flourish"), &flourish("drift-shibuya", w, h, 15));
        }
        println!("wrote drift dumps to {}", dir.display());
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
