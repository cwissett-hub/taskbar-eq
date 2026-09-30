//! The NOS family: a street-racing in-car dash from the early films / 2 Fast 2 Furious, with the
//! "DANGER TO MANIFOLD" nitrous-warning screen as its flourish.
//!
//! # The dash
//!
//! - **Panel.** A dark in-car display with a carbon-fibre weave (4x4 cells in a diagonal checker, one
//!   shade off the panel) and a backlit dial face (the colourway's `lit` glowing faintly inside the
//!   tacho arc, brighter toward the ring - the EL-backlit gauge look of the era), with 0-8 figures
//!   under the ring on a tall panel. All of it baked once per (size, colours) into `bg`.
//! - **Tacho (the meter).** 48 LED segments on a 150 degree arc (an ellipse, so it spans the panel
//!   rather than sitting in a 50 px circle), bands folded 64 -> 48, spaced evenly by ARC LENGTH. Each
//!   segment is a 2x4 px block along the arc (1 px ticks, 3 deep, when the pitch is under 4 px, so
//!   neighbours do not merge into a pinholed band) and lights when its band's level passes its threshold
//!   (0.10 at the left rising to 0.35 at the right, so a loud bass-heavy track reads as a tacho
//!   winding up left to right as well as a spectrum). Colour by position: `lit` to 70 %, amber to
//!   90 %, red above; an unlit segment is a dim ghost of its colour. A 1 px `hot` needle sweeps from
//!   the hub with rms. The segment pixels are computed once per size into a fixed array.
//! - **Shift lights.** 10 LEDs across the top - green x3, amber x3, red x3, blue - lighting with rms;
//!   at `rms_norm >= 0.9` all ten flash blue at 8 Hz.
//! - **Gear + speed.** Right side, 2x `font3x5`: the gear `N`/`1`-`6` from rms (upshifts at once,
//!   downshifts with a little hysteresis so it does not chatter; pinned with the rest in the purge),
//!   boxed; and a 3-digit speed
//!   `round(rms_norm * 199)` over `MPH`. Below 200 px wide only the gear shows.
//! - **Under-glow.** On bass onsets a neon strip in `zones[0].lit` glows up from the bottom edge
//!   (8 px, alpha 0.6 -> 0), decaying over 250 ms.
//!
//! `FrameData.levels` arrive already smoothed by main's `Smoother` (the theme's ballistics); the
//! segments read them directly - no second attack/decay here.
//!
//! # The flourish - the NOS hit
//!
//! 800 ms, fired only on a bass hit. For the first 300 ms the interior is the green-on-black warning
//! screen: black, a 1 px `#39ff5a` frame, `DANGER TO MANIFOLD` in 2x green blinking at 4 Hz (on two
//! lines, `DANGER TO` / `MANIFOLD`, when one line would not leave `WARN_MARGIN` either side), `NOS`
//! and a bar gauge draining to empty beneath. Then the PURGE: back to the dash with 12 preallocated
//! white speed lines streaking left at 400 px/s and the tacho, needle and shift lights pinned at
//! redline, all decaying with the envelope.
//!
//! No car logos, badges or film wordmarks: the words are our own font, the shapes our own.
//!
//! The panel is opaque (painted first) and the clip to the rounded rect runs last on every path.
//! `draw` allocates nothing after the first frame at a size: the background canvas and the geometry
//! are rebuilt only on a size (or colour) change, the speed lines are a fixed pool, and the gear and
//! speed are `[u8; N]`.

use std::f32::consts::PI;

use crate::dsp::bands::NUM_BANDS;
use crate::dsp::flourish::{Envelope, Trigger};
use crate::dsp::onset::Flux;
use crate::render::canvas::{Canvas, Rgba};
use crate::render::font3x5;
use crate::render::{Family, FrameData};
use crate::themes::Theme;

/// Tacho segments (bands folded 64 -> 48) and the most pixels one segment's block can hold.
const SEGS: usize = 48;
const SEG_PX: usize = 12;
/// The arc: 150 degrees, symmetric about the top.
const ARC_DEG: f32 = 150.0;
/// A segment lights when its band passes this threshold, rising left to right.
const SEG_THR_LO: f32 = 0.10;
const SEG_THR_HI: f32 = 0.35;
/// Colour by position: `lit` below this fraction of the arc, amber below the next, red above.
const AMBER_FROM: f32 = 0.7;
const RED_FROM: f32 = 0.9;
/// The arc is never flatter than this (rx / ry), so it still reads as a gauge on a wide panel.
const MAX_ASPECT: f32 = 3.4;

const AMBER: Rgba = Rgba { r: 0xff, g: 0xb0, b: 0x00, a: 255 };
const RED: Rgba = Rgba { r: 0xff, g: 0x2a, b: 0x2a, a: 255 };
/// Shift-light colours. The green is deliberately NOT the warning screen's `#39ff5a`.
const SHIFT_GREEN: Rgba = Rgba { r: 0x2c, g: 0xe0, b: 0x48, a: 255 };
const SHIFT_BLUE: Rgba = Rgba { r: 0x2a, g: 0x5c, b: 0xff, a: 255 };
/// The NOS warning screen's green.
const WARN_GREEN: Rgba = Rgba { r: 0x39, g: 0xff, b: 0x5a, a: 255 };

const WHITE: Rgba = Rgba { r: 255, g: 255, b: 255, a: 255 };
const BLACK: Rgba = Rgba { r: 0, g: 0, b: 0, a: 255 };

/// Shift lights: how many, and the rms_norm at which they all flash blue, at this rate.
const SHIFT_LEDS: usize = 10;
const REDLINE: f32 = 0.9;
const REDLINE_HZ: f32 = 8.0;

/// Below this width the speed readout is dropped and only the gear shows.
const SPEED_MIN_W: i32 = 200;
/// Gear downshift hysteresis, in rms_norm.
const GEAR_HYST: f32 = 0.04;
/// At or below this rms_norm the box shows `N`.
const NEUTRAL_BELOW: f32 = 0.03;

/// Under-glow: strip height, peak alpha, decay.
const GLOW_H: i32 = 8;
const GLOW_ALPHA: f32 = 0.6;
const GLOW_MS: f32 = 250.0;

/// The NOS hit: total length, the warning screen's share, the blink rate, the one-line margin.
const NOS_MS: f32 = 800.0;
const WARN_MS: f32 = 300.0;
const WARN_BLINK_HZ: f32 = 4.0;
/// `DANGER TO MANIFOLD` goes on one line only if it leaves this much either side; otherwise two.
const WARN_MARGIN: i32 = 24;
/// The flourish only fires on a BASS hit (the `sesh` pattern).
const FLOURISH_BASS_MIN: f32 = 0.6;

/// Speed lines in the purge: pool size and speed.
const SPEED_LINES: usize = 12;
const SPEED_LINE_PX_S: f32 = 400.0;

/// A bass onset (the under-glow) needs the low bands over this; the permissive flux net `sesh` uses.
const BASS_ONSET: f32 = 0.55;
const ONSET_RATIO: f32 = 2.8;
const ONSET_REFRACTORY_MS: f32 = 200.0;

/// The warning screen's layout.
#[derive(Clone, Copy, Default, PartialEq)]
struct Warn {
    two_lines: bool,
    /// Top-left of each text line (line 2 unused on one line).
    l1: (i32, i32),
    l2: (i32, i32),
    nos_scale: i32,
    nos: (i32, i32),
    /// The bar gauge: x, y, w, h (outline included).
    bar: (i32, i32, i32, i32),
}

/// Everything positional, for a panel size. Shared by `draw` and the tests.
#[derive(Clone, Copy)]
struct Layout {
    /// Each segment's pixels (the first `seg_n[i]` of row `i` are used).
    seg_px: [[(i16, i16); SEG_PX]; SEGS],
    seg_n: [u8; SEGS],
    /// The arc's centre (the needle hub) and its outer radii.
    cx: i32,
    cy: i32,
    rx: f32,
    ry: f32,
    /// Shift lights: left x of each, their y, width, height.
    led_x: [i32; SHIFT_LEDS],
    led_y: i32,
    led_w: i32,
    led_h: i32,
    show_speed: bool,
    /// The gear box (x, y, w, h) and the speed digits' top-left, `MPH` top-left.
    gear_box: (i32, i32, i32, i32),
    speed: (i32, i32),
    mph: (i32, i32),
    /// Where 0-8 go under the ring (only on a tall panel).
    figures: bool,
    warn: Warn,
}

impl Default for Layout {
    fn default() -> Self {
        Layout {
            seg_px: [[(0, 0); SEG_PX]; SEGS],
            seg_n: [0; SEGS],
            cx: 0,
            cy: 0,
            rx: 0.0,
            ry: 0.0,
            led_x: [0; SHIFT_LEDS],
            led_y: 0,
            led_w: 0,
            led_h: 0,
            show_speed: false,
            gear_box: (0, 0, 0, 0),
            speed: (0, 0),
            mph: (0, 0),
            figures: false,
            warn: Warn::default(),
        }
    }
}

/// The width of `text` in `font3x5` at `scale` (glyph pixels `scale` x `scale`, 4 * scale pitch).
fn text_w(text: &str, scale: i32) -> i32 {
    (text.chars().count() as i32 * 4 * scale - scale).max(0)
}

/// A point on the ellipse at angle `th` (radians, 0 = right, y up).
fn ell(cx: f32, cy: f32, rx: f32, ry: f32, th: f32) -> (f32, f32) {
    (cx + rx * th.cos(), cy - ry * th.sin())
}

fn layout(w: i32, h: i32) -> Layout {
    let (ix0, iy0, ix1, iy1) = (3, 4, w - 3, h - 4);
    let (iw, ih) = (ix1 - ix0, iy1 - iy0);
    let mut l = Layout::default();
    let small = h < 48;

    // ---- readouts, right-aligned ----
    l.show_speed = w >= SPEED_MIN_W;
    let (gw, gh) = (12, 14); // a 6x10 2x glyph in a box with a 2 px margin and a 1 px ring
    let speed_w = text_w("000", 2);
    let readout_w = if l.show_speed { speed_w + 5 + gw } else { gw };
    let rx0 = ix1 - 3 - readout_w;
    let gy = iy0 + (ih - gh) / 2;
    l.gear_box = (ix1 - 3 - gw, gy, gw, gh);
    let block_h = 10 + 2 + 5;
    let sy = iy0 + (ih - block_h) / 2;
    l.speed = (rx0, sy);
    l.mph = (rx0 + speed_w - text_w("MPH", 1), sy + 12);

    // ---- shift lights and the tacho arc ----
    l.led_h = if small { 3 } else { 4 };
    l.led_y = iy0 + 2;
    let (tx0, tx1) = (ix0 + 3, rx0 - 5);
    let arc_top = l.led_y + l.led_h + 3;
    let cy = iy1 - 2;
    let ry = (cy - arc_top).max(4) as f32;
    let half_span = (tx1 - tx0).max(8) as f32 / 2.0;
    let half_ang = (ARC_DEG / 2.0).to_radians();
    // The arc's widest point is at its ends (15 degrees above the centre line).
    let rx = (half_span / (PI / 2.0 - half_ang).cos() - 1.0).min(ry * MAX_ASPECT).max(4.0);
    let cx = (tx0 + tx1) as f32 / 2.0;
    l.cx = cx.round() as i32;
    l.cy = cy;
    l.rx = rx;
    l.ry = ry;

    let led_w = if small { 5 } else { 7 };
    let led_gap = if small { 3 } else { 4 };
    l.led_w = led_w;
    let leds_w = SHIFT_LEDS as i32 * (led_w + led_gap) - led_gap;
    let lx0 = l.cx - leds_w / 2;
    for (i, x) in l.led_x.iter_mut().enumerate() {
        *x = lx0 + i as i32 * (led_w + led_gap);
    }

    // ---- the 48 segments, evenly spaced by arc length ----
    let th0 = PI / 2.0 + half_ang; // the left end
    let th1 = PI / 2.0 - half_ang; // the right end
    const STEPS: usize = 720;
    let mut cum = [0.0f32; STEPS + 1];
    let mut prev = ell(cx, cy as f32, rx, ry, th0);
    for (k, cm) in cum.iter_mut().enumerate().skip(1) {
        let th = th0 + (th1 - th0) * k as f32 / STEPS as f32;
        let p = ell(cx, cy as f32, rx, ry, th);
        *cm = (p.0 - prev.0).hypot(p.1 - prev.1);
        prev = p;
    }
    for k in 1..=STEPS {
        cum[k] += cum[k - 1];
    }
    let total = cum[STEPS];
    let depth = if small { 3.0 } else { 4.0 };
    // The block is 2 px along the arc where there is room; on a small panel (pitch under 4 px) the
    // 2 px blocks would touch and leave pinholes where they overlap, so they narrow to 1 px ticks.
    let half_along = if total / (SEGS - 1) as f32 >= 4.0 { 1.0 } else { 0.62 };
    let mut k = 0usize;
    let mut seg_n = [0u8; SEGS];
    for (i, seg) in l.seg_px.iter_mut().enumerate() {
        let want = total * i as f32 / (SEGS - 1) as f32;
        while k < STEPS && cum[k] < want {
            k += 1;
        }
        let th = th0 + (th1 - th0) * k as f32 / STEPS as f32;
        let (px, py) = ell(cx, cy as f32, rx, ry, th);
        // The outward normal of the ellipse there, and the tangent.
        let (nx, ny) = (th.cos() / rx, -th.sin() / ry);
        let nl = nx.hypot(ny).max(1e-6);
        let (nx, ny) = (nx / nl, ny / nl);
        let (tx, ty) = (-ny, nx);
        // Rasterise the 2 (along) x `depth` (inward) block: every pixel whose centre falls inside the
        // rotated rectangle, so a tilted segment is a clean slanted tick, not a rounding scatter.
        let mut n = 0;
        let (bx0, by0) = ((px - 5.0).floor() as i32, (py - 5.0).floor() as i32);
        for yy in by0..by0 + 11 {
            for xx in bx0..bx0 + 11 {
                let (dx, dy) = (xx as f32 + 0.5 - px, yy as f32 + 0.5 - py);
                let along = dx * tx + dy * ty;
                let inward = -(dx * nx + dy * ny);
                if along.abs() <= half_along && (-0.5..depth - 0.5).contains(&inward) && n < SEG_PX {
                    seg[n] = (xx as i16, yy as i16);
                    n += 1;
                }
            }
        }
        seg_n[i] = n as u8;
    }
    l.seg_n = seg_n;
    l.figures = ih >= 50;

    // ---- the warning screen ----
    let one = "DANGER TO MANIFOLD";
    let (a, b) = ("DANGER TO", "MANIFOLD");
    let mut wv = Warn { two_lines: text_w(one, 2) + 2 * WARN_MARGIN > iw, ..Warn::default() };
    let text_h = if wv.two_lines { 10 + 2 + 10 } else { 10 };
    wv.nos_scale = if text_h + 3 + 10 <= ih - 6 { 2 } else { 1 };
    let nos_h = 5 * wv.nos_scale;
    let gap = if wv.two_lines { 3 } else { 5 };
    let block = text_h + gap + nos_h;
    let ty = iy0 + (ih - block) / 2;
    if wv.two_lines {
        wv.l1 = (ix0 + (iw - text_w(a, 2)) / 2, ty);
        wv.l2 = (ix0 + (iw - text_w(b, 2)) / 2, ty + 12);
    } else {
        wv.l1 = (ix0 + (iw - text_w(one, 2)) / 2, ty);
    }
    let ny = ty + text_h + gap;
    let nos_w = text_w("NOS", wv.nos_scale);
    let bar_w = (iw / 3).clamp(16, 72);
    let row_w = nos_w + 4 + bar_w;
    let nx = ix0 + (iw - row_w) / 2;
    wv.nos = (nx, ny);
    wv.bar = (nx + nos_w + 4, ny, bar_w, nos_h);
    l.warn = wv;
    l
}

/// One speed line of the purge.
#[derive(Clone, Copy, Default)]
struct SpeedLine {
    x: f32,
    y: i32,
    len: i32,
}

pub struct Nos {
    /// Fires the NOS hit on a rare bass hit. `pub(crate)` so the tests can force it.
    pub(crate) flourish: Trigger,
    hit: Envelope,
    glow: Envelope,
    onset: Flux,
    elapsed_s: f32,
    gear: u8,
    layout: Layout,
    layout_dim: (i32, i32),
    /// The baked panel: carbon weave, dial face, figures. Keyed on size and colours.
    bg: Canvas,
    bg_key: u64,
    lines: [SpeedLine; SPEED_LINES],
    rng: u64,
    /// Test hook: draw only the gear/speed readouts.
    readouts_only: bool,
}

impl Default for Nos {
    fn default() -> Self {
        Nos {
            flourish: Default::default(),
            hit: Default::default(),
            glow: Default::default(),
            onset: Default::default(),
            elapsed_s: 0.0,
            gear: 0,
            layout: Layout::default(),
            layout_dim: (0, 0),
            bg: Canvas::new(1, 1),
            bg_key: 0,
            lines: [SpeedLine::default(); SPEED_LINES],
            rng: 0x6a09_e667_f3bc_c908,
            readouts_only: false,
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

/// FNV-1a over the strings/values the baked background depends on. No allocation.
fn colour_key(t: &Theme, w: i32, h: i32) -> u64 {
    let mut k: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |b: u8| {
        k ^= b as u64;
        k = k.wrapping_mul(0x0100_0000_01b3);
    };
    for s in [t.panel.as_str(), t.lit.as_str(), t.edge.as_str()] {
        for b in s.bytes() {
            eat(b);
        }
        eat(0);
    }
    for b in t.ghost.to_bits().to_le_bytes().into_iter().chain(w.to_le_bytes()).chain(h.to_le_bytes()) {
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

/// A segment's colour by its position on the arc.
fn seg_colour(i: usize, lit: Rgba) -> Rgba {
    let f = i as f32 / (SEGS - 1) as f32;
    if f < AMBER_FROM {
        lit
    } else if f < RED_FROM {
        AMBER
    } else {
        RED
    }
}

fn shift_colour(i: usize) -> Rgba {
    match i {
        0..=2 => SHIFT_GREEN,
        3..=5 => AMBER,
        6..=8 => RED,
        _ => SHIFT_BLUE,
    }
}

impl Nos {
    /// Draws only the readouts (no dash), for the fit-or-drop test.
    #[cfg(test)]
    fn readouts_only_for_test(&mut self) {
        self.readouts_only = true;
    }

    /// One draw of splitmix64.
    fn next_rng(&mut self) -> u64 {
        self.rng = self.rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Rebuilds the geometry on a size change and the baked background on a size or colour change.
    fn resize(&mut self, w: i32, h: i32, t: &Theme) {
        if self.layout_dim != (w, h) {
            self.layout = layout(w, h);
            self.layout_dim = (w, h);
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
        let (ix0, iy0, ix1, iy1) = (3, 4, w - 3, h - 4);
        // The carbon weave: 4x4 cells in a diagonal checker; within a cell the strands run across
        // (even cells) or down (odd cells), so the checker reads as a twill. One shade off the panel.
        let hi = Rgba::lerp_linear(panel, WHITE, 0.012);
        let lo = Rgba::lerp_linear(panel, BLACK, 0.35);
        for y in iy0..iy1 {
            for x in ix0..ix1 {
                let (u, v) = (x.rem_euclid(4), y.rem_euclid(4));
                let odd = ((x.div_euclid(4) + y.div_euclid(4)) & 1) == 1;
                let along = if odd { u } else { v };
                let col = match along {
                    0 => lo,
                    1 | 2 => hi,
                    _ => panel,
                };
                bg.fill_rect(x, y, 1, 1, col);
            }
        }
        // The backlit dial face: lit, faint at the hub and brighter toward the ring.
        let (cx, cy) = (l.cx as f32, l.cy as f32);
        for y in iy0..iy1 {
            let dy = (y as f32 + 0.5 - cy) / l.ry;
            if dy > 0.0 {
                continue;
            }
            for x in ix0..ix1 {
                let dx = (x as f32 + 0.5 - cx) / l.rx;
                let q = (dx * dx + dy * dy).sqrt();
                if q >= 1.0 {
                    continue;
                }
                let a = 0.035 + 0.15 * q.powi(4);
                bg.fill_rect(x, y, 1, 1, with_alpha(lit, a));
            }
        }
        // The figures 0-8 under the ring, dim.
        if l.figures {
            let fig = Rgba::lerp_linear(panel, lit, 0.45);
            for n in 0..=8u8 {
                let i = (n as usize * (SEGS - 1) + 4) / 8;
                let th = PI / 2.0 + (ARC_DEG / 2.0).to_radians() - ARC_DEG.to_radians() * i as f32 / (SEGS - 1) as f32;
                let (fx, fy) = ell(cx, cy, l.rx - 10.0, l.ry - 10.0, th);
                let digit = [b'0' + n];
                let st = std::str::from_utf8(&digit).unwrap_or("0");
                text(bg, fx.round() as i32 - 1, fy.round() as i32 - 2, st, 1, fig);
            }
        }
    }
}

impl Family for Nos {
    fn id(&self) -> &'static str {
        "nos"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());

        // ---- the opaque panel ----
        let panel = Rgba::from_hex(&t.panel, 1.0);
        c.rounded_rect(1, 2, w - 2, h - 4, 3, panel);
        let (ix0, iy0, ix1, iy1) = (3i32, 4i32, w - 3, h - 4);
        let (iw, ih) = (ix1 - ix0, iy1 - iy0);
        if iw < 16 || ih < 12 {
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

        // ---- onsets: the under-glow ----
        let onset = self.onset.update(&d.levels, dt, ONSET_RATIO, ONSET_REFRACTORY_MS);
        let glow = self.glow.update(onset && bass > BASS_ONSET, dt, GLOW_MS);

        // ---- the NOS hit ----
        let triggered = self.flourish.update(&d.levels, dt, t.flourish);
        let bass_gate = triggered && bass >= FLOURISH_BASS_MIN;
        #[cfg(test)]
        let fired = bass_gate || (triggered && self.flourish.was_forced());
        #[cfg(not(test))]
        let fired = bass_gate;
        let env = self.hit.update(fired, dt, NOS_MS);
        let since_ms = (1.0 - env) * NOS_MS;
        if fired {
            // Arm the purge's speed lines now, spread across the panel so the first purge frame is
            // already streaking.
            for k in 0..SPEED_LINES {
                let (r0, r1, r2) = (self.next_rng(), self.next_rng(), self.next_rng());
                self.lines[k] = SpeedLine {
                    x: (ix0 + (r0 % iw.max(1) as u64) as i32) as f32,
                    y: iy0 + 1 + (r1 % (ih - 2).max(1) as u64) as i32,
                    len: 18 + (r2 % 44) as i32,
                };
            }
        }
        let warning = env > 0.0 && since_ms < WARN_MS;
        // The purge pin: 1 as the warning ends, 0 at the end of the hit.
        let pin = if env > 0.0 && !warning { (env / (1.0 - WARN_MS / NOS_MS)).clamp(0.0, 1.0) } else { 0.0 };

        // ---- gear: upshift at once, downshift with hysteresis (pinned with everything in the purge) ----
        let drive = rms_norm.max(pin);
        let target = if drive <= NEUTRAL_BELOW { 0 } else { 1 + ((drive * 6.0) as u8).min(5) };
        if target > self.gear {
            self.gear = target;
        } else if target < self.gear {
            let floor = (self.gear as f32 - 1.0) / 6.0 - GEAR_HYST;
            if drive < floor || target == 0 {
                self.gear = target;
            }
        }

        let l = &self.layout;
        if warning {
            draw_warning(c, l, since_ms, (ix0, iy0, iw, ih));
            c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);
            return;
        }

        let lit = Rgba::from_hex(&t.lit, 1.0);
        let hot = Rgba::from_hex(&t.hot, 1.0);
        let edge = Rgba::from_hex(&t.edge, 1.0);

        if !self.readouts_only {
            // ---- the baked background ----
            c.copy_region(&self.bg, (ix0, iy0), (ix0, iy0), iw, ih);

            // ---- under-glow ----
            if glow > 0.0 {
                let ug = t.zones.first().map(|z| z.lit.as_str()).unwrap_or(t.lit.as_str());
                let ug = Rgba::from_hex(ug, 1.0);
                for k in 0..GLOW_H {
                    let a = GLOW_ALPHA * (1.0 - k as f32 / GLOW_H as f32) * glow;
                    c.fill_rect(ix0, iy1 - 1 - k, iw, 1, with_alpha(ug, a));
                }
            }

            // ---- the tacho ----
            let ghost_k = (t.ghost * 1.6).clamp(0.05, 0.3);
            let pinned = (pin * SEGS as f32).round() as usize;
            let mut on = [false; SEGS];
            for (i, o) in on.iter_mut().enumerate() {
                let lo = i * NUM_BANDS / SEGS;
                let hi = ((i + 1) * NUM_BANDS / SEGS).max(lo + 1);
                let level = d.levels[lo..hi].iter().map(|&v| lvl(v)).sum::<f32>() / (hi - lo) as f32;
                let thr = SEG_THR_LO + (SEG_THR_HI - SEG_THR_LO) * i as f32 / (SEGS - 1) as f32;
                *o = level >= thr || i < pinned;
            }
            // Ghosts first, lit on top: on a small panel neighbouring blocks overlap, and a ghost
            // must never bite into a lit one.
            for pass in [false, true] {
                for i in 0..SEGS {
                    if on[i] != pass {
                        continue;
                    }
                    let base = seg_colour(i, lit);
                    let col = if pass { base } else { Rgba::lerp_linear(panel, base, ghost_k) };
                    for &(x, y) in &l.seg_px[i][..l.seg_n[i] as usize] {
                        c.fill_rect(x as i32, y as i32, 1, 1, col);
                    }
                }
            }
            // The needle, from the hub, sweeping with rms (pinned at redline in the purge).
            let rn = rms_norm.max(pin);
            let half_ang = (ARC_DEG / 2.0).to_radians();
            let th = PI / 2.0 + half_ang - ARC_DEG.to_radians() * rn;
            let reach = 0.78;
            let (nx, ny) = ell(l.cx as f32, l.cy as f32, l.rx * reach, l.ry * reach, th);
            c.line(l.cx, l.cy, nx.round() as i32, ny.round() as i32, hot);
            // The hub: a cap in `edge`, ringed in a dim `lit`, with a `hot` pivot.
            let hub_r = if h >= 48 { 4 } else { 3 };
            c.fill_semicircle_upper(l.cx, l.cy + 1, hub_r + 1, Rgba::lerp_linear(panel, lit, 0.35));
            c.fill_semicircle_upper(l.cx, l.cy + 1, hub_r, edge);
            c.fill_rect(l.cx, l.cy - 1, 1, 1, hot);

            // ---- shift lights ----
            let srn = rms_norm.max(pin);
            let redline = srn >= REDLINE;
            let flash_on = (e * REDLINE_HZ).fract() < 0.5;
            let n_lit = ((srn / REDLINE) * 9.0).floor() as usize;
            for i in 0..SHIFT_LEDS {
                let own = shift_colour(i);
                let on = if redline { flash_on } else { i < n_lit.min(9) };
                let col = if redline && on {
                    SHIFT_BLUE
                } else if on {
                    own
                } else {
                    Rgba::lerp_linear(panel, own, 0.16)
                };
                let (x, y) = (l.led_x[i], l.led_y);
                c.fill_rect(x, y, l.led_w, l.led_h, col);
                if on {
                    // A hot highlight along the top: an LED, not a flat block.
                    c.fill_rect(x + 1, y, l.led_w - 2, 1, Rgba::lerp_linear(col, WHITE, 0.5));
                }
            }
        }

        // ---- gear + speed ----
        let (bx, by, bw, bh) = l.gear_box;
        let ring = Rgba::lerp_linear(panel, lit, 0.5);
        c.fill_rect(bx, by, bw, 1, ring);
        c.fill_rect(bx, by + bh - 1, bw, 1, ring);
        c.fill_rect(bx, by, 1, bh, ring);
        c.fill_rect(bx + bw - 1, by, 1, bh, ring);
        let gears = b"N123456";
        let gch = [gears[self.gear.min(6) as usize]];
        let gs = std::str::from_utf8(&gch).unwrap_or("N");
        text(c, bx + 3, by + 2, gs, 2, hot);
        if l.show_speed {
            let mph = (rms_norm.max(pin) * 199.0).round().clamp(0.0, 199.0) as u32;
            let digits = [b'0' + (mph / 100) as u8, b'0' + (mph / 10 % 10) as u8, b'0' + (mph % 10) as u8];
            let ds = std::str::from_utf8(&digits).unwrap_or("000");
            text(c, l.speed.0, l.speed.1, ds, 2, hot);
            text(c, l.mph.0, l.mph.1, "MPH", 1, lit);
        }

        // ---- the purge's speed lines ----
        if pin > 0.0 && !self.readouts_only {
            let step = SPEED_LINE_PX_S * dt / 1000.0;
            for k in 0..SPEED_LINES {
                let mut s = self.lines[k];
                s.x -= step;
                if s.x + (s.len as f32) < ix0 as f32 {
                    let (r0, r1) = (self.next_rng(), self.next_rng());
                    s.x = (ix1 + (r0 % 40) as i32) as f32;
                    s.y = iy0 + 1 + (r1 % (ih - 2).max(1) as u64) as i32;
                }
                self.lines[k] = s;
                let x = s.x.round() as i32;
                // A bright head (leading, on the left) and a tail thinning out behind it.
                let a = pin.sqrt();
                c.fill_rect(x, s.y, 5, 1, with_alpha(WHITE, a));
                let q = s.len / 3;
                c.fill_rect(x + 5, s.y, q, 1, with_alpha(WHITE, 0.8 * a));
                c.fill_rect(x + 5 + q, s.y, s.len - 5 - q, 1, with_alpha(WHITE, 0.45 * a));
            }
        }

        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);
    }
}

/// The green-on-black warning screen: black, a 1 px green frame, `DANGER TO MANIFOLD` blinking at
/// 4 Hz, `NOS` and a bar gauge draining to empty over the warning's 300 ms.
fn draw_warning(c: &mut Canvas, l: &Layout, since_ms: f32, interior: (i32, i32, i32, i32)) {
    let (ix0, iy0, iw, ih) = interior;
    c.fill_rect(ix0, iy0, iw, ih, BLACK);
    let (fx, fy, fw, fh) = (ix0 + 1, iy0 + 1, iw - 2, ih - 2);
    c.fill_rect(fx, fy, fw, 1, WARN_GREEN);
    c.fill_rect(fx, fy + fh - 1, fw, 1, WARN_GREEN);
    c.fill_rect(fx, fy, 1, fh, WARN_GREEN);
    c.fill_rect(fx + fw - 1, fy, 1, fh, WARN_GREEN);
    let wv = &l.warn;
    let blink_on = (since_ms / 1000.0 * WARN_BLINK_HZ).fract() < 0.5;
    if blink_on {
        if wv.two_lines {
            text(c, wv.l1.0, wv.l1.1, "DANGER TO", 2, WARN_GREEN);
            text(c, wv.l2.0, wv.l2.1, "MANIFOLD", 2, WARN_GREEN);
        } else {
            text(c, wv.l1.0, wv.l1.1, "DANGER TO MANIFOLD", 2, WARN_GREEN);
        }
    }
    text(c, wv.nos.0, wv.nos.1, "NOS", wv.nos_scale, WARN_GREEN);
    let (bx, by, bw, bh) = wv.bar;
    c.fill_rect(bx, by, bw, 1, WARN_GREEN);
    c.fill_rect(bx, by + bh - 1, bw, 1, WARN_GREEN);
    c.fill_rect(bx, by, 1, bh, WARN_GREEN);
    c.fill_rect(bx + bw - 1, by, 1, bh, WARN_GREEN);
    let left = (1.0 - since_ms / WARN_MS).clamp(0.0, 1.0);
    let fill = ((bw - 4) as f32 * left).round() as i32;
    if fill > 0 && bh > 4 {
        c.fill_rect(bx + 2, by + 2, fill, bh - 4, WARN_GREEN);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::canvas::Canvas;

    const IDS: [&str; 4] = ["nos-2fast", "nos-original", "nos-quarter", "nos-miami"];

    fn theme(id: &str) -> Theme {
        crate::themes::builtin::all().into_iter().find(|t| t.id == id).unwrap()
    }
    fn frames(fam: &mut Nos, t: &Theme, w: i32, h: i32, level: f32, n: usize) -> Canvas {
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
    /// `n` frames with only `bands` at `level`, the rest silent, and the given rms.
    #[allow(clippy::too_many_arguments)]
    fn frames_bands(
        fam: &mut Nos,
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
            fam.draw(&mut c, t, &d);
        }
        c
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
    /// A tacho segment is LIT when its painted pixels are far brighter than the panel (a ghost
    /// segment is a dim tint). Read off the canvas at the segment's own pixels.
    fn seg_lit(c: &Canvas, t: &Theme, i: usize) -> bool {
        let l = layout(c.width(), c.height());
        let panel = Rgba::from_hex(&t.panel, 1.0);
        let n = l.seg_n[i] as usize;
        assert!(n >= 2, "segment {i} has only {n} pixels");
        let bright = l.seg_px[i][..n]
            .iter()
            .filter(|&&(x, y)| {
                let px = c.get(x as i32, y as i32);
                (px.r as i32 - panel.r as i32) + (px.g as i32 - panel.g as i32) + (px.b as i32 - panel.b as i32) > 300
            })
            .count();
        bright * 2 > n
    }
    fn is_warn_green(px: Rgba) -> bool {
        px.a == 255
            && (px.r as i32 - 0x39).abs() <= 4
            && (px.g as i32 - 0xff).abs() <= 4
            && (px.b as i32 - 0x5a).abs() <= 4
    }
    /// Warning-green pixels strictly inside the warning frame (the text, `NOS` and the gauge).
    fn warn_text_px(c: &Canvas) -> usize {
        let (w, h) = (c.width(), c.height());
        let mut n = 0;
        for y in 6..h - 6 {
            for x in 5..w - 5 {
                if is_warn_green(c.get(x, y)) {
                    n += 1;
                }
            }
        }
        n
    }

    #[test]
    fn registered_and_labelled() {
        assert!(crate::render::KNOWN_FAMILIES.contains(&"nos"));
        assert_eq!(crate::render::family_for("nos").id(), "nos");
        assert_eq!(crate::themes::family_label("nos"), "Fast & Furious: NOS");
        let themes: Vec<Theme> = crate::themes::builtin::all().into_iter().filter(|t| t.family == "nos").collect();
        let ids: Vec<String> = themes.iter().map(|t| t.id.clone()).collect();
        assert_eq!(ids, IDS.map(String::from).to_vec());
        // Each colourway registers exactly one zone: its under-glow.
        for t in &themes {
            assert_eq!(t.zones.len(), 1, "{}", t.id);
            assert_eq!(t.zones[0].lit, t.zones[0].hot, "{}", t.id);
        }
    }

    /// Silence still shows a dash: the backlit face, the ghost tacho, the unlit shift lights, `N`
    /// and `000`.
    #[test]
    fn rest_frame_is_not_empty() {
        for id in IDS {
            let t = theme(id);
            for (w, h) in [(380, 60), (190, 48), (128, 44)] {
                let c = frames(&mut Nos::default(), &t, w, h, 0.0, 10);
                let n = lit(&c, &t);
                assert!(n as f32 >= 0.025 * (w * h) as f32, "{id} {w}x{h}: only {n} px at rest");
            }
        }
    }

    /// Bass-only (the lowest quarter of the bands): the first 10 segments lit, the last 10 dark - the
    /// arc IS the spectrum, left to right, read off the painted pixels.
    #[test]
    fn the_tacho_fills_left_to_right_with_the_spectrum() {
        for id in IDS {
            let t = theme(id);
            for (w, h) in [(380, 60), (190, 48), (128, 44)] {
                let c = frames_bands(&mut Nos::default(), &t, w, h, 0..16, 0.9, 0.05, 10);
                for i in 0..10 {
                    assert!(seg_lit(&c, &t, i), "{id} {w}x{h}: bass segment {i} dark on a bass-only input");
                }
                for i in SEGS - 10..SEGS {
                    assert!(!seg_lit(&c, &t, i), "{id} {w}x{h}: treble segment {i} lit on a bass-only input");
                }
            }
            // And treble-only lights the right end, not the left.
            let c = frames_bands(&mut Nos::default(), &t, 380, 60, 48..64, 0.9, 0.05, 10);
            assert!(seg_lit(&c, &t, SEGS - 1) && !seg_lit(&c, &t, 0), "{id}: treble-only does not light the right end");
        }
    }

    /// At redline (rms_norm >= 0.9) the shift lights flash blue at 8 Hz: over 8 frames (133 ms) the
    /// 10th LED is blue in some and off in another, and when it is blue so is the first.
    #[test]
    fn shift_lights_flash_blue_at_redline() {
        let is_blue = |px: Rgba| px.b >= 200 && px.r < 120 && px.g < 160;
        for id in IDS {
            let t = theme(id);
            let (w, h) = (380, 60);
            let l = layout(w, h);
            let mut fam = Nos::default();
            let _ = frames(&mut fam, &t, w, h, 0.5, 5);
            let (mut blue, mut off) = (0, 0);
            for _ in 0..8 {
                let c = frames(&mut fam, &t, w, h, 0.5, 1);
                let p10 = c.get(l.led_x[9] + l.led_w / 2, l.led_y + l.led_h - 1);
                let p1 = c.get(l.led_x[0] + l.led_w / 2, l.led_y + l.led_h - 1);
                if is_blue(p10) {
                    blue += 1;
                    assert!(is_blue(p1), "{id}: LED 10 blue but LED 1 is not ({p1:?})");
                } else {
                    off += 1;
                }
            }
            assert!(blue > 0 && off > 0, "{id}: at redline the 10th LED was blue {blue}/8 and off {off}/8");
            // Below redline the 10th stays dark and the first is its own green.
            let mut fam = Nos::default();
            let c = frames(&mut fam, &t, w, h, 0.15, 12); // rms_norm 0.6
            let p10 = c.get(l.led_x[9] + l.led_w / 2, l.led_y + l.led_h - 1);
            let p1 = c.get(l.led_x[0] + l.led_w / 2, l.led_y + l.led_h - 1);
            assert!(!is_blue(p10), "{id}: 10th LED blue below redline");
            assert!(p1.g > 200 && p1.b < 120, "{id}: first LED not green below redline ({p1:?})");
        }
    }

    /// A forced NOS hit: the early frames are the green warning (warning-green text inside the
    /// frame), and 900 ms later no warning green is left anywhere.
    #[test]
    fn nos_flourish_shows_the_manifold_warning() {
        for id in IDS {
            let t = theme(id);
            for (w, h) in [(380, 60), (190, 48), (128, 44)] {
                let mut fam = Nos::default();
                let _ = frames(&mut fam, &t, w, h, 0.5, 5);
                fam.flourish.force_next();
                let c = frames(&mut fam, &t, w, h, 0.5, 1);
                let n = warn_text_px(&c);
                assert!(n >= 150, "{id} {w}x{h}: only {n} warning-green px on the fired frame");
                // It is a black screen, not green over the dash.
                let bg = c.get(w / 2, h - 7);
                assert!(bg.r < 8 && bg.g < 8 && bg.b < 8, "{id} {w}x{h}: warning background {bg:?}");
                let c = frames(&mut fam, &t, w, h, 0.5, 3);
                assert!(warn_text_px(&c) > 0, "{id} {w}x{h}: warning gone by 67 ms");
                let c = frames(&mut fam, &t, w, h, 0.5, 54); // ~ 950 ms after firing
                let mut green = 0;
                for y in 0..h {
                    for x in 0..w {
                        if is_warn_green(c.get(x, y)) {
                            green += 1;
                        }
                    }
                }
                assert_eq!(green, 0, "{id} {w}x{h}: {green} warning-green px after 900 ms");
            }
        }
    }

    /// The row bands of warning-green text inside the frame, as (first row, last row, min x, max x).
    fn text_bands(c: &Canvas) -> Vec<(i32, i32, i32, i32)> {
        let (w, h) = (c.width(), c.height());
        let mut out: Vec<(i32, i32, i32, i32)> = Vec::new();
        let mut cur: Option<(i32, i32, i32, i32)> = None;
        for y in 6..h - 6 {
            let mut span: Option<(i32, i32)> = None;
            for x in 5..w - 5 {
                if is_warn_green(c.get(x, y)) {
                    span = Some(match span {
                        None => (x, x),
                        Some((a, _)) => (a, x),
                    });
                }
            }
            match (span, cur) {
                (Some((a, b)), None) => cur = Some((y, y, a, b)),
                (Some((a, b)), Some((y0, _, x0, x1))) => cur = Some((y0, y, x0.min(a), x1.max(b))),
                (None, Some(band)) => {
                    out.push(band);
                    cur = None;
                }
                (None, None) => {}
            }
        }
        if let Some(band) = cur {
            out.push(band);
        }
        out
    }

    /// At 190x48 `DANGER TO MANIFOLD` does not fit on one line with its margin, so it falls back to
    /// `DANGER TO` over `MANIFOLD`: two text bands the widths of those words (70 and 62 px at 2x),
    /// above the `NOS` row, and no green past the interior. At 380x60 it is one 142 px line.
    #[test]
    fn manifold_warning_falls_back_to_two_lines() {
        let t = theme("nos-2fast");
        let fire = |w: i32, h: i32| {
            let mut fam = Nos::default();
            let _ = frames(&mut fam, &t, w, h, 0.5, 5);
            fam.flourish.force_next();
            frames(&mut fam, &t, w, h, 0.5, 1)
        };
        for (w, h) in [(190, 48), (128, 44)] {
            let c = fire(w, h);
            let bands = text_bands(&c);
            assert!(bands.len() >= 3, "{w}x{h}: expected DANGER TO / MANIFOLD / NOS rows, got {bands:?}");
            // Nothing past the interior: the green stops at the frame.
            for y in 0..h {
                for x in 0..w {
                    if is_warn_green(c.get(x, y)) {
                        assert!(
                            (3..w - 3).contains(&x) && (4..h - 4).contains(&y),
                            "{w}x{h}: green past the interior at {x},{y}"
                        );
                    }
                }
            }
            let (a, b) = (bands[0], bands[1]);
            assert_eq!(a.3 - a.2 + 1, text_w("DANGER TO", 2), "{w}x{h}: line 1 {a:?}");
            assert_eq!(b.3 - b.2 + 1, text_w("MANIFOLD", 2), "{w}x{h}: line 2 {b:?}");
            assert_eq!(a.1 - a.0 + 1, 10, "{w}x{h}: line 1 is not one 2x row: {a:?}");
            assert_eq!(b.1 - b.0 + 1, 10, "{w}x{h}: line 2 is not one 2x row: {b:?}");
        }
        let c = fire(380, 60);
        let bands = text_bands(&c);
        assert!(!bands.is_empty());
        assert_eq!(bands[0].3 - bands[0].2 + 1, text_w("DANGER TO MANIFOLD", 2), "380x60: not one line: {bands:?}");
    }

    /// 380x60 shows the gear box AND the speed; 128x44 only the gear. Drawn with the dash
    /// suppressed so the readouts' own extent is measured.
    #[test]
    fn speed_and_gear_fit_or_drop() {
        let t = theme("nos-2fast");
        let extent = |w: i32, h: i32| {
            let mut fam = Nos::default();
            fam.readouts_only_for_test();
            let c = frames(&mut fam, &t, w, h, 0.12, 10); // rms_norm 0.48: gear 3, 96 MPH
            let panel = Rgba::from_hex(&t.panel, 1.0);
            let (mut x0, mut x1) = (i32::MAX, i32::MIN);
            for y in 4..h - 4 {
                for x in 3..w - 3 {
                    if drew_over_panel(c.get(x, y), panel) {
                        x0 = x0.min(x);
                        x1 = x1.max(x);
                    }
                }
            }
            (x0, x1)
        };
        let (a, b) = extent(380, 60);
        assert!(b - a + 1 >= 12 + 5 + text_w("000", 2), "380x60: readouts only {a}..{b}: no speed?");
        let (a, b) = extent(128, 44);
        assert!(a <= b, "128x44: no gear drawn");
        assert!(b - a < 14, "128x44: readouts span {a}..{b}: the speed did not drop");
        assert!(b < 125, "128x44: gear past the interior at {b}");
    }

    /// 190x48 and 128x44 with a forced hit: no panic, something drawn, nothing outside the panel.
    #[test]
    fn fits_the_narrow_panel_and_flourish_does_not_panic() {
        for id in IDS {
            let t = theme(id);
            for (w, h) in [(190, 48), (128, 44)] {
                let mut fam = Nos::default();
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

    #[test]
    fn the_four_colourways_are_visibly_different() {
        let (w, h) = (380i32, 60i32);
        let canvases: Vec<Canvas> =
            IDS.iter().map(|id| frames(&mut Nos::default(), &theme(id), w, h, 0.12, 30)).collect();
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

    /// A bass onset raises the under-glow: the bottom rows tint toward `zones[0].lit` (not `lit`),
    /// strongest at the edge; 400 ms later it has decayed away.
    #[test]
    fn under_glow_rises_on_a_bass_onset() {
        for id in IDS {
            let t = theme(id);
            let ug = Rgba::from_hex(&t.zones[0].lit, 1.0);
            let (w, h) = (380, 60);
            let mut fam = Nos::default();
            let _ = frames_bands(&mut fam, &t, w, h, 0..8, 0.05, 0.01, 20);
            let quiet = frames_bands(&mut fam, &t, w, h, 0..8, 0.05, 0.01, 1);
            let kick = frames_bands(&mut fam, &t, w, h, 0..8, 0.95, 0.05, 1);
            // Far left of the arc, under nothing but the weave: how far toward the glow colour.
            let toward = |c: &Canvas, y: i32| -> i32 {
                let (p, q) = (c.get(8, y), quiet.get(8, y));
                // Distance to the glow colour, quiet minus kick: positive = moved toward it.
                let dist = |x: Rgba| (x.r as i32 - ug.r as i32).abs() + (x.g as i32 - ug.g as i32).abs() + (x.b as i32 - ug.b as i32).abs();
                dist(q) - dist(p)
            };
            let edge = toward(&kick, h - 5);
            let high = toward(&kick, h - 5 - GLOW_H);
            assert!(edge > 120, "{id}: the bottom row moved only {edge} toward the under-glow on a kick");
            assert!(high < edge / 3, "{id}: no gradient: {high} at 8 px up vs {edge} at the edge");
            let later = frames_bands(&mut fam, &t, w, h, 0..8, 0.95, 0.05, 24); // ~400 ms, held
            let gone = toward(&later, h - 5);
            assert!(gone < 10, "{id}: the under-glow still {gone} toward its colour after 400 ms");
        }
    }

    #[test]
    fn every_label_char_has_a_glyph() {
        for s in ["DANGER TO MANIFOLD", "NOS", "MPH", "N123456", "0123456789"] {
            for ch in s.chars() {
                assert!(font3x5::glyph(ch).is_some(), "{s:?} {ch:?}");
            }
        }
    }

    /// Dumps for the eye test, composited over `#202020`. Each file is `<name>.<w>x<h>.rgba`.
    ///
    /// Run: cargo test --release dump_nos -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_nos() {
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
        // A music-like frame: bass-heavy, wobbling; `rms` as main would see it (0..~0.25).
        let frame = |level: f32, rms: f32, t_s: f32, beat: bool| {
            let mut d = FrameData { dt_ms: 16.7, time_s: t_s, ..FrameData::default() };
            for (i, v) in d.levels.iter_mut().enumerate() {
                let f = i as f32 / NUM_BANDS as f32;
                let shape = (1.0 - f).powf(1.2) * 0.7 + 0.12;
                let wob = 1.0 + 0.3 * (t_s * 2.4 + f * 6.0).sin();
                let kick = if beat { 1.0 + 0.9 * (1.0 - f) } else { 1.0 };
                *v = ((shape * wob * kick) * level).clamp(0.0, 1.0);
            }
            d.peaks = d.levels;
            d.rms_l = rms;
            d.rms_r = rms;
            d
        };
        let beat = |k: usize| k % 24 < 3;
        let run = |id: &str, w: i32, h: i32, level: f32, rms: f32, n: usize| {
            let t = theme(id);
            let mut fam = Nos::default();
            let mut c = Canvas::new(w, h);
            for k in 0..n {
                c.clear();
                fam.draw(&mut c, &t, &frame(level, rms, k as f32 * 0.0167, beat(k)));
            }
            (fam, c, t)
        };
        for id in IDS {
            let short = &id["nos-".len()..];
            // calm ends between beats; loud ends two frames after a kick (the under-glow is up).
            for (tag, level, rms, n) in [("calm", 0.3f32, 0.07f32, 118), ("loud", 0.85, 0.2, 122)] {
                let (_, c, _) = run(id, 380, 60, level, rms, n);
                write(format!("nos-{short}-{tag}"), &c);
            }
            // Flourish: settle, fire; early = ~50 ms in (the warning screen, blink on); late = ~330 ms
            // in (the purge: speed lines, tacho pinned).
            let (mut fam, mut c, t) = run(id, 380, 60, 0.6, 0.12, 110);
            fam.flourish.force_next();
            for k in 110..114 {
                c.clear();
                fam.draw(&mut c, &t, &frame(0.6, 0.12, k as f32 * 0.0167, false));
            }
            write(format!("nos-{short}-flourish-early"), &c);
            for k in 114..130 {
                c.clear();
                fam.draw(&mut c, &t, &frame(0.6, 0.12, k as f32 * 0.0167, false));
            }
            write(format!("nos-{short}-flourish-late"), &c);
        }
        for (w, h) in [(190, 48), (128, 44)] {
            let (_, c, _) = run("nos-2fast", w, h, 0.85, 0.2, 122);
            write(format!("nos-2fast-{w}x{h}"), &c);
            let (mut fam, mut c, t) = run("nos-2fast", w, h, 0.6, 0.12, 110);
            fam.flourish.force_next();
            for k in 110..114 {
                c.clear();
                fam.draw(&mut c, &t, &frame(0.6, 0.12, k as f32 * 0.0167, false));
            }
            write(format!("nos-2fast-{w}x{h}-warning"), &c);
        }
        println!("wrote nos dumps to {}", dir.display());
    }

    /// Per-frame cost, steady and during the NOS hit, at 380x60.
    ///
    /// Run: cargo test --release probe_nos_cost -- --ignored --nocapture
    #[test]
    #[ignore]
    fn probe_nos_cost() {
        for id in IDS {
            let t = theme(id);
            let mut fam = Nos::default();
            let mut c = Canvas::new(380, 60);
            let mut d = FrameData { dt_ms: 16.7, ..FrameData::default() };
            let set = |d: &mut FrameData, k: usize| {
                let kick = if k % 24 < 3 { 1.6 } else { 1.0 };
                for (i, v) in d.levels.iter_mut().enumerate() {
                    *v = (0.55 * (1.0 - i as f32 / 96.0) * kick).min(1.0);
                }
                d.peaks = d.levels;
                d.rms_l = 0.2;
                d.rms_r = 0.2;
            };
            for k in 0..60 {
                set(&mut d, k);
                fam.draw(&mut c, &t, &d);
            }
            let n = 600;
            let t0 = std::time::Instant::now();
            for k in 0..n {
                set(&mut d, k);
                d.time_s = k as f32 * 0.0167;
                fam.draw(&mut c, &t, &d);
            }
            let steady = t0.elapsed().as_secs_f64() * 1000.0 / n as f64;
            fam.flourish.force_next();
            let m = 48; // the 800 ms hit
            let t1 = std::time::Instant::now();
            for k in n..(n + m) {
                set(&mut d, k);
                d.time_s = k as f32 * 0.0167;
                fam.draw(&mut c, &t, &d);
            }
            let flourish = t1.elapsed().as_secs_f64() * 1000.0 / m as f64;
            println!("{id}: steady {steady:.3} ms/frame, flourish {flourish:.3} ms/frame at 380x60");
        }
    }
}
