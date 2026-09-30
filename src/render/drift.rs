//! The drift family: Tokyo Drift. A night skyline with neon signs, and a car drifting along a ground
//! line whose tyre-smoke trail IS the meter.
//!
//! # The scene
//!
//! - **Skyline (baked once per size and colours into `bg`).** A night sky shading from `panel` at the
//!   top toward `edge` and a faint wash of the neon at the horizon (the city's light pollution); a far
//!   row of hazy tower silhouettes and a near row of black blocks in front, both with hashed widths and
//!   heights, the near ones carrying a scatter of lit windows (the colourway's `edge` lifted toward
//!   `lit`, with a few warm ones). The road below the ground line carries a faint neon reflection.
//! - **Neon signs.** Short bars on the near blocks in the neon (`zones[0].lit`) with a few in `hot`:
//!   vertical blades down a building's side and rooftop bars. Their unlit tubes are baked; the lit
//!   tube and a 1 px halo (brighter on the bass) are drawn each frame, so roughly a third of them can
//!   flicker at 1-2 Hz the way a failing neon does (mostly on, with a stutter off).
//! - **The car.** Our own 14x6 side profile of a generic sports coupe - no model, no badge - in `lit`
//!   with a `hot` tail light, glass that picks up the neon and a faint headlight cone. Three bitmaps
//!   show the drift angle: level, tail swung out (the rear face shows) and nose in (the front face
//!   shows). It slides along the ground line on a slow pendulum between 25 % and 75 % of the width
//!   whose swing grows with rms (silence parks it in the centre), while the world scrolls past it.
//! - **The meter - the tyre smoke.** A fixed ring of the car's last 64 rear-wheel positions, scrolled
//!   with the world, so the trail streams out behind the car and rises as it ages. Puff `k` (band `k`,
//!   bass at the car) is a filled circle of radius `1 + level * 7` px (scaled on a short panel) in
//!   white at `ghost` alpha with a 2-step rim (a disc at half alpha under a disc one pixel smaller at
//!   half alpha): the smoke billows near the car on a bass-heavy passage and thins to a wisp on
//!   treble. `FrameData.levels` arrive already smoothed by main's `Smoother`; the puffs read them
//!   directly - no second attack/decay here.
//! - **Steering gauge.** Top right, a radius-8 semicircle with a needle at the drift angle and
//!   `ANGLE NN` (`[u8; 2]`). Dropped below 200 px wide.
//! - **Road.** Lane dashes scrolling with the world.
//!
//! # The flourish - the drift
//!
//! 900 ms, fired only on a bass hit. The car whips across the full width in one sweep (out past the
//! right edge and back in from the left to where the pendulum has it), pinned at full drift angle; the
//! smoke goes to max radius tinted `hot`, so the sweep lays a wall of it across the panel; the neon
//! signs flash; and `DRIFT!` (2x `font3x5`, `hot`, 1 px dark outline) slams in centred for 400 ms
//! (one frame at 3x, then 2x with a 1 px shake). No Japanese glyphs.
//!
//! The panel is opaque (painted first) and the clip to the rounded rect runs last on every path.
//! `draw` allocates nothing after the first frame at a size: the background canvas and the geometry
//! are rebuilt only on a size (or colour) change, the trail is a fixed ring, the radii a fixed array,
//! and the angle readout `[u8; 2]`.

use std::f32::consts::PI;

use crate::dsp::bands::NUM_BANDS;
use crate::dsp::flourish::{Envelope, Trigger};
use crate::render::canvas::{Canvas, Rgba};
use crate::render::font3x5;
use crate::render::{Family, FrameData};
use crate::themes::Theme;

/// The trail: one puff per band, and the ring of past positions they sit on.
const TRAIL: usize = NUM_BANDS;
/// Puff radius `1 + level * PUFF_GAIN` px on a full-height panel.
const PUFF_GAIN: f32 = 7.0;
/// The car bitmap.
const CAR_W: i32 = 14;
const CAR_H: i32 = 6;

/// The pendulum: between 25 % and 75 % of the width, one swing every `SWING_S` at full rms.
const SWING_FRAC: f32 = 0.25;
const SWING_S: f32 = 9.0;
/// How fast the swing's amplitude follows rms (a motion ease, not a level smoother).
const SWING_EASE_MS: f32 = 600.0;
/// The world scrolls at this fraction of the interior width per second, plus more with rms.
const SCROLL_BASE: f32 = 0.20;
const SCROLL_RMS: f32 = 0.16;
/// The drift angle at full swing, in degrees, and where the bitmap changes from level.
const MAX_ANGLE: f32 = 42.0;
const ANGLE_BITMAP_FROM: f32 = 12.0;

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
const WARM_WINDOW: Rgba = Rgba { r: 0xff, g: 0xc8, b: 0x7a, a: 255 };
/// `DRIFT!`'s outline and the car's.
const OUTLINE: Rgba = Rgba { r: 0x05, g: 0x03, b: 0x08, a: 255 };

/// The car, facing right (the front on the right). `#` body, `d` a face in shadow, `g` glass,
/// `t` tail light, `h` headlight, `w` tyre, `r` wheel rim, `.` nothing. Our own shape.
const CAR_LEVEL: [&[u8; 14]; 6] = [
    b".....#####....",
    b"...##gg#gggg#.",
    b"t###########hh",
    b"##############",
    b"##wrw####wrw##",
    b"..www....www..",
];
/// The tail swung out toward the viewer: the rear face shows on the left, the side foreshortens.
const CAR_TAIL_OUT: [&[u8; 14]; 6] = [
    b"....#####.....",
    b"..##gg#ggg#...",
    b"tdt#########h.",
    b"ddd###########",
    b"ddwrw####wrw#.",
    b"..www....www..",
];
/// The nose turned in toward the viewer: the front face shows on the right.
const CAR_NOSE_IN: [&[u8; 14]; 6] = [
    b".....#####....",
    b"...##gg#ggg#d.",
    b"t##########hdh",
    b"###########ddd",
    b"##wrw####wrwdd",
    b"..www....www..",
];

/// A building block in the skyline.
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
    /// The ground line's row; the car's wheels sit on the row above it.
    gy: i32,
    far: [Block; 64],
    far_n: usize,
    near: [Block; 64],
    near_n: usize,
    signs: [Sign; SIGNS],
    signs_n: usize,
    /// Puff radius scale (1 on a full-height panel).
    puff_scale: f32,
    /// How far the trail rises over its length.
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
            gy: 0,
            far: [Block::default(); 64],
            far_n: 0,
            near: [Block::default(); 64],
            near_n: 0,
            signs: [Sign::default(); SIGNS],
            signs_n: 0,
            puff_scale: 1.0,
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
    l.gy = iy1 - if small { 4 } else { 5 };
    let sky_h = (l.gy - iy0).max(1);
    l.puff_scale = (ih as f32 / 52.0).clamp(0.6, 1.0);
    l.rise = sky_h as f32 * 0.28;

    // ---- the skyline: a constant seed, so the city is the same every run ----
    let mut s: u64 = 0x5eed_d71f_7000_0001;
    // Far: tall hazy towers, packed edge to edge.
    let mut x = ix0 - 3;
    while x < ix1 && l.far_n < l.far.len() {
        let bw = 5 + (splitmix(&mut s) % 9) as i32;
        let bh = (sky_h as f32 * (0.34 + 0.34 * unit(&mut s))).round() as i32;
        l.far[l.far_n] = Block { x, w: bw, h: bh };
        l.far_n += 1;
        x += bw;
    }
    // Near: lower black blocks, with the odd gap to the far row behind.
    let mut x = ix0 - 2;
    while x < ix1 && l.near_n < l.near.len() {
        let bw = 7 + (splitmix(&mut s) % 12) as i32;
        let bh = (sky_h as f32 * (0.16 + 0.30 * unit(&mut s))).round() as i32;
        l.near[l.near_n] = Block { x, w: bw, h: bh };
        l.near_n += 1;
        x += bw + if splitmix(&mut s).is_multiple_of(4) { 2 + (splitmix(&mut s) % 4) as i32 } else { 0 };
    }
    // Signs on the near blocks: a vertical blade down one side, or a rooftop bar.
    for i in 0..l.near_n {
        if l.signs_n >= SIGNS {
            break;
        }
        let b = l.near[i];
        let r = unit(&mut s);
        if r > 0.62 || b.h < 6 {
            continue;
        }
        let top = l.gy - b.h;
        let (sx, sy, sw, sh) = if r < 0.34 {
            // A blade: 2 px wide, 3-6 tall, hanging off one side just under the roof.
            let tall = 3 + (splitmix(&mut s) % 4) as i32;
            let tall = tall.min(b.h - 3).max(2);
            let side = if splitmix(&mut s).is_multiple_of(2) { b.x + 1 } else { b.x + b.w - 3 };
            (side, top + 2, 2, tall)
        } else {
            // A rooftop bar: 2 tall, 3-8 wide, on the roof.
            let bw = (3 + (splitmix(&mut s) % 6) as i32).min(b.w - 2).max(2);
            (b.x + 1 + (splitmix(&mut s) % (b.w - bw - 1).max(1) as u64) as i32, top - 3, bw, 2)
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

    // ---- DRIFT!, centred in the sky above the car ----
    let tw = text_w("DRIFT!", 2);
    l.drift_text = (ix0 + (iw - tw) / 2, iy0 + (sky_h - 10) / 2);
    l
}

/// One smoke position in the ring: x, and a small fixed jitter drawn when it was laid.
#[derive(Clone, Copy, Default)]
struct Puff {
    x: f32,
    jy: f32,
    /// A radius jitter (-1, 0, +1 px) so the plume billows rather than drawing a smooth tube.
    jr: i32,
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
    elapsed_s: f32,
    /// The pendulum's phase and eased amplitude (a fraction of the swing, 0..1).
    phase: f32,
    amp: f32,
    /// The car's x (left edge, f32) and signed drift angle, degrees.
    car_x: f32,
    angle: f32,
    /// Where the car was when the sweep began.
    sweep_from: f32,
    /// The road dashes' scroll offset.
    road: f32,
    ring: [Puff; TRAIL],
    head: usize,
    ring_live: bool,
    layout: Layout,
    layout_dim: (i32, i32),
    /// The baked scene. Keyed on size and colours.
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
            elapsed_s: 0.0,
            phase: 0.0,
            amp: 0.0,
            car_x: 0.0,
            angle: 0.0,
            sweep_from: 0.0,
            road: 0.0,
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

impl Drift {
    #[cfg(test)]
    fn only_for_test(&mut self, only: Only) {
        self.only = only;
    }

    /// The car's left edge, rounded, as drawn - for the tests.
    #[cfg(test)]
    fn car_left(&self) -> i32 {
        self.car_x.round() as i32
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
        let gy = l.gy;
        let sky_h = (gy - iy0).max(1) as f32;

        // The sky: panel at the top, toward edge and a wash of neon at the horizon.
        for y in iy0..gy {
            let f = (y - iy0) as f32 / sky_h;
            let base = Rgba::lerp_linear(panel, edge, 0.85 * f.powf(1.6));
            let col = Rgba::lerp_linear(base, neon, 0.16 * f.powi(3));
            bg.fill_rect(ix0, y, iw, 1, col);
        }
        // Far towers: hazy silhouettes a shade off the horizon sky, with a sparse dim window here and
        // there.
        let far = Rgba::lerp_linear(panel, edge, 0.62);
        let far_win = Rgba::lerp_linear(far, lit, 0.22);
        let mut s: u64 = 0x0f0f_1a2b_3c4d_5e6f;
        for b in &l.far[..l.far_n] {
            bg.fill_rect(b.x, gy - b.h, b.w, b.h, far);
            let mut y = gy - b.h + 2;
            while y < gy - 3 {
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
        // Near blocks: black, a 1 px roof edge, and a grid of windows with a scatter lit.
        let near = Rgba::lerp_linear(panel, BLACK, 0.5);
        let roof = Rgba::lerp_linear(near, edge, 0.8);
        let win = Rgba::lerp_linear(edge, lit, 0.5);
        let win_warm = Rgba::lerp_linear(edge, WARM_WINDOW, 0.6);
        let win_dark = Rgba::lerp_linear(near, edge, 0.35);
        let win_w = if h >= 48 { 2 } else { 1 };
        for b in &l.near[..l.near_n] {
            let top = gy - b.h;
            bg.fill_rect(b.x, top, b.w, b.h, near);
            bg.fill_rect(b.x, top, b.w, 1, roof);
            let mut y = top + 2;
            while y < gy - 2 {
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
            let base = if sg.hot { hot } else { neon };
            bg.fill_rect(sg.x, sg.y, sg.w, sg.h, Rgba::lerp_linear(near, base, 0.22));
        }
        // The ground line and the road, with a faint neon reflection just under the line.
        bg.fill_rect(ix0, gy, iw, 1, Rgba::lerp_linear(edge, lit, 0.3));
        let road = Rgba::lerp_linear(panel, BLACK, 0.3);
        bg.fill_rect(ix0, gy + 1, iw, iy1 - gy - 1, road);
        bg.fill_rect(ix0, gy + 1, iw, 1, Rgba::lerp_linear(road, neon, 0.14));

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

        // ---- the drift ----
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
        }
        let drifting = env > 0.0;

        // ---- the pendulum ----
        let k = (dt / SWING_EASE_MS).min(1.0);
        self.amp += (rms_norm - self.amp) * k;
        if !self.amp.is_finite() {
            self.amp = 0.0;
        }
        self.phase = (self.phase + 2.0 * PI * dt / 1000.0 / SWING_S).rem_euclid(2.0 * PI);
        let mid = ix0 as f32 + iw as f32 * 0.5 - CAR_W as f32 / 2.0;
        let pend_x = mid + self.amp * SWING_FRAC * iw as f32 * self.phase.sin();
        let pend_angle = MAX_ANGLE * self.amp * self.phase.cos();
        // Full drift the moment the sweep starts, easing back as the hit ends.
        let boost = if drifting { (env / 0.35).min(1.0) } else { 0.0 };
        if drifting && since_ms < SWEEP_MS {
            // Out past the right edge and back in from the left, on a torus the car can leave fully.
            let lap = iw as f32 + CAR_W as f32;
            let start = ix0 as f32 - CAR_W as f32;
            let from = self.sweep_from;
            let dist = lap + (pend_x - from);
            let u = from - start + dist * ease(since_ms / SWEEP_MS);
            self.car_x = start + u.rem_euclid(lap);
        } else {
            self.car_x = pend_x;
        }
        self.angle = pend_angle + (MAX_ANGLE - pend_angle) * boost;

        // ---- the world scrolls; the trail rides it ----
        let step = iw as f32 * (SCROLL_BASE + SCROLL_RMS * rms_norm.max(boost)) * dt / 1000.0;
        let rear = self.car_x + 3.0;
        if !self.ring_live {
            for (i, p) in self.ring.iter_mut().enumerate() {
                *p = Puff { x: rear - step.max(1.0) * i as f32, jy: 0.0, jr: 0 };
            }
            self.head = 0;
            self.ring_live = true;
        } else {
            for p in self.ring.iter_mut() {
                p.x -= step;
            }
            self.head = (self.head + 1) % TRAIL;
            let rnd = self.next_rng();
            let jy = (rnd % 5) as f32 - 2.0;
            let jr = ((rnd >> 8) % 3) as i32 - 1;
            self.ring[self.head] = Puff { x: rear, jy: jy * 0.6, jr };
        }
        self.road = (self.road + step).rem_euclid(12.0);

        let lit = Rgba::from_hex(&t.lit, 1.0);
        let hot = Rgba::from_hex(&t.hot, 1.0);
        let neon = neon_of(t);
        let l = &self.layout;
        let gy = l.gy;
        let only = self.only;

        if only == Only::All {
            // ---- the baked scene ----
            c.copy_region(&self.bg, (ix0, iy0), (ix0, iy0), iw, ih);

            // ---- neon ----
            let flash = drifting && since_ms < FLASH_MS;
            let flash_on = flash && (since_ms / 1000.0 * FLASH_HZ).fract() < 0.5;
            let halo_a = 0.14 + 0.22 * bass;
            for sg in &l.signs[..l.signs_n] {
                if !flash && !sign_on(sg, e) {
                    continue;
                }
                let base = if sg.hot { hot } else { neon };
                let (col, ha) = if flash_on { (Rgba::lerp_linear(base, WHITE, 0.55), 0.5) } else { (base, halo_a) };
                c.fill_rect(sg.x - 1, sg.y - 1, sg.w + 2, sg.h + 2, with_alpha(base, ha));
                c.fill_rect(sg.x, sg.y, sg.w, sg.h, col);
            }

            // ---- lane dashes ----
            let dy = gy + 2;
            if dy < iy1 {
                let dash = Rgba::lerp_linear(panel, lit, 0.22);
                let mut x = ix0 as f32 - self.road;
                while x < ix1 as f32 {
                    c.fill_rect(x.round() as i32, dy, 6, 1, dash);
                    x += 12.0;
                }
            }
        }

        // ---- the tyre smoke: the meter ----
        if only != Only::Car {
            let s = l.puff_scale;
            let r_max = 1.0 + PUFF_GAIN * s;
            let mut radius = [0i32; TRAIL];
            for (i, r) in radius.iter_mut().enumerate() {
                let base = 1.0 + lvl(d.levels[i]) * PUFF_GAIN * s;
                *r = (base + (r_max - base) * boost).round().max(1.0) as i32;
            }
            let tint = Rgba::lerp_linear(WHITE, hot, 0.7 * boost);
            let ghost = if t.ghost.is_finite() { t.ghost.clamp(0.05, 0.6) } else { 0.3 };
            let lift = l.rise / (TRAIL - 1) as f32;
            // Oldest first, so the fresh puffs at the car sit on top.
            for i in (0..TRAIL).rev() {
                let p = self.ring[(self.head + TRAIL - i) % TRAIL];
                let r = if radius[i] >= 3 { radius[i] + p.jr } else { radius[i] };
                let x = p.x.round() as i32;
                if x + r < ix0 || x - r >= ix1 {
                    continue;
                }
                let cy = (gy as f32 - 1.0 - r as f32 * 0.55 - lift * i as f32 + p.jy).round() as i32;
                // Consecutive puffs overlap a lot, so each disc pass is a fraction of `ghost`: the
                // core of a lone puff lands near `ghost * 0.55`, and the fresh overlapping ones build
                // toward `ghost` and past it where the smoke is thick. Older puffs thin out.
                let a = ghost * (1.0 - 0.65 * i as f32 / (TRAIL - 1) as f32);
                // A 2-step rim: the outer ring at one pass, the core at two.
                let col = with_alpha(tint, a * 0.32);
                c.fill_circle(x, cy, r, col);
                if r >= 2 {
                    c.fill_circle(x, cy, r - 1, col);
                }
            }
        }

        // ---- the car ----
        if only != Only::Smoke {
            let bitmap = if self.angle > ANGLE_BITMAP_FROM {
                &CAR_TAIL_OUT
            } else if self.angle < -ANGLE_BITMAP_FROM {
                &CAR_NOSE_IN
            } else {
                &CAR_LEVEL
            };
            let (cx, cy) = (self.car_x.round() as i32, gy - CAR_H);
            if only == Only::All {
                // A faint headlight cone along the road, and the tail light's glow.
                let beam = Rgba::lerp_linear(lit, WHITE, 0.6);
                for i in 0..18 {
                    let f = 1.0 - i as f32 / 18.0;
                    let a = 0.13 * f * f;
                    let spread = i / 6;
                    c.fill_rect(cx + CAR_W + i, cy + 2 - spread / 2, 1, 1 + spread, with_alpha(beam, a));
                }
                c.fill_rect(cx - 2, cy + 1, 2, 3, with_alpha(hot, 0.35));
            }
            let body_d = Rgba::lerp_linear(lit, BLACK, 0.45);
            let glass = Rgba::lerp_linear(panel, neon, 0.45);
            let head = Rgba::lerp_linear(lit, WHITE, 0.7);
            let rim = Rgba::lerp_linear(lit, BLACK, 0.35);
            // A 1 px dark outline first, so the car reads against its own smoke and the sky.
            for (ry, row) in bitmap.iter().enumerate() {
                for (rx, &ch) in row.iter().enumerate() {
                    if ch != b'.' {
                        let (px, py) = (cx + rx as i32, cy + ry as i32);
                        c.fill_rect(px - 1, py, 3, 1, OUTLINE);
                        c.fill_rect(px, py - 1, 1, 3, OUTLINE);
                    }
                }
            }
            for (ry, row) in bitmap.iter().enumerate() {
                for (rx, &ch) in row.iter().enumerate() {
                    let col = match ch {
                        b'#' => lit,
                        b'd' => body_d,
                        b'g' => glass,
                        b't' => hot,
                        b'h' => head,
                        b'w' => TYRE,
                        b'r' => rim,
                        _ => continue,
                    };
                    c.fill_rect(cx + rx as i32, cy + ry as i32, 1, 1, col);
                }
            }
        }

        if only == Only::All {
            // ---- the steering gauge ----
            if l.show_gauge {
                let (gx, gcy) = l.gauge;
                let th = (90.0 - self.angle * 2.0).to_radians();
                let reach = (GAUGE_R - 1) as f32;
                let (nx, ny) = (gx as f32 + reach * th.cos(), gcy as f32 - reach * th.sin());
                c.line(gx, gcy, nx.round() as i32, ny.round() as i32, hot);
                c.fill_rect(gx, gcy, 1, 1, lit);
                let a = self.angle.abs().round().clamp(0.0, 99.0) as u32;
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
                if tin < 20.0 && big_w + 4 <= iw && 15 + 2 <= gy - iy0 {
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
    /// The painted columns' extent (min x, max x) of whatever the hook left drawn, inside the panel.
    fn extent(c: &Canvas, t: &Theme) -> Option<(i32, i32)> {
        let panel = Rgba::from_hex(&t.panel, 1.0);
        let (mut x0, mut x1) = (i32::MAX, i32::MIN);
        for y in 4..c.height() - 4 {
            for x in 3..c.width() - 3 {
                if drew_over_panel(c.get(x, y), panel) {
                    x0 = x0.min(x);
                    x1 = x1.max(x);
                }
            }
        }
        (x0 <= x1).then_some((x0, x1))
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

    /// Silence still shows the city: sky, skyline, windows, neon, the parked car and a thin trail.
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

    /// Bass-only vs treble-only, the car parked (rms 0): the smoke close behind the car (the first
    /// puffs, which are the bass bands) is far bigger on bass; the far end of the trail (the treble
    /// bands) is bigger on treble. Read off the painted smoke.
    #[test]
    fn the_smoke_trail_billows_with_the_spectrum() {
        for id in IDS {
            let t = theme(id);
            for (w, h) in [(380, 60), (128, 44)] {
                let run = |bands: std::ops::Range<usize>| {
                    let mut fam = Drift::default();
                    fam.only_for_test(Only::Smoke);
                    let c = frames_bands(&mut fam, &t, w, h, bands, 0.9, 0.0, 90);
                    (c, fam.car_left())
                };
                let (bass, car) = run(0..16);
                let (treble, car2) = run(48..64);
                assert_eq!(car, car2);
                // The first ~16 puffs sit within this many px behind the rear wheel.
                let iw = (w - 6) as f32;
                let step = iw * SCROLL_BASE / 60.0;
                let near = (car - (step * 14.0) as i32, car + CAR_W);
                let far = (car - (step * 63.0) as i32, car - (step * 48.0) as i32);
                let (bn, tn) = (smoke_px(&bass, &t, near.0, near.1), smoke_px(&treble, &t, near.0, near.1));
                assert!(bn >= 3 * tn.max(1), "{id} {w}x{h}: near the car bass {bn} px vs treble {tn} px");
                let (bf, tf) = (smoke_px(&bass, &t, far.0, far.1), smoke_px(&treble, &t, far.0, far.1));
                assert!(tf >= 2 * bf.max(1), "{id} {w}x{h}: at the trail's end treble {tf} px vs bass {bf} px");
            }
        }
    }

    /// Loud, the car's painted extent sweeps across a wide stretch of the panel over a swing; silent,
    /// it sits parked in the centre.
    #[test]
    fn the_car_swings_with_rms() {
        for id in IDS {
            let t = theme(id);
            for (w, h) in [(380, 60), (128, 44)] {
                let sweep = |rms: f32| {
                    let mut fam = Drift::default();
                    fam.only_for_test(Only::Car);
                    let (mut lo, mut hi) = (i32::MAX, i32::MIN);
                    let _ = frames(&mut fam, &t, w, h, rms, 120);
                    for _ in 0..(SWING_S * 60.0) as usize / 4 {
                        let c = frames(&mut fam, &t, w, h, rms, 4);
                        let (a, b) = extent(&c, &t).expect("no car drawn");
                        lo = lo.min(a);
                        hi = hi.max(b);
                    }
                    (lo, hi)
                };
                let (lo, hi) = sweep(0.25);
                let span = (hi - lo) as f32 / w as f32;
                assert!(span >= 0.45, "{id} {w}x{h}: loud, the car swept only {lo}..{hi} ({:.0}%)", span * 100.0);
                assert!(lo >= (w as f32 * 0.2) as i32 && hi <= (w as f32 * 0.8) as i32, "{id} {w}x{h}: swing {lo}..{hi} past 25-75%");
                let (lo, hi) = sweep(0.0);
                assert!(hi - lo <= CAR_W + 2, "{id} {w}x{h}: silent, the car moved {lo}..{hi}");
                let centre = (lo + hi) / 2 - w / 2;
                assert!(centre.abs() <= 2, "{id} {w}x{h}: silent, the car parked off centre ({lo}..{hi})");
            }
        }
    }

    /// A forced drift: within 900 ms the car's painted extent spans >= 60 % of the width.
    #[test]
    fn drift_flourish_sweeps_the_car_across() {
        for id in IDS {
            let t = theme(id);
            for (w, h) in [(380, 60), (190, 48), (128, 44)] {
                let mut fam = Drift::default();
                fam.only_for_test(Only::Car);
                // Silence: the forced drift fires regardless, and the car parks dead centre after.
                let _ = frames(&mut fam, &t, w, h, 0.0, 30);
                fam.flourish.force_next();
                let (mut lo, mut hi) = (i32::MAX, i32::MIN);
                for _ in 0..54 {
                    let c = frames(&mut fam, &t, w, h, 0.0, 1);
                    if let Some((a, b)) = extent(&c, &t) {
                        lo = lo.min(a);
                        hi = hi.max(b);
                    }
                }
                let span = (hi - lo) as f32 / w as f32;
                assert!(span >= 0.6, "{id} {w}x{h}: the drift swept only {lo}..{hi} ({:.0}%)", span * 100.0);
                // And it lands back on its pendulum: parked centre.
                let c = frames(&mut fam, &t, w, h, 0.0, 10);
                let (a, b) = extent(&c, &t).unwrap();
                assert!(((a + b) / 2 - w / 2).abs() <= 3, "{id} {w}x{h}: after the drift the car is at {a}..{b}");
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
                            let p = c.get(x, y);
                            if p == hot {
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
                assert!(during >= 4 * 44, "{id} {w}x{h}: only {during} hot px of DRIFT! (before: {before})");
                let _ = frames(&mut fam, &t, w, h, 0.1, 50);
                let after = count(&frames(&mut fam, &t, w, h, 0.1, 1));
                // A few `hot` pixels (a tail light, the needle) can fall in the box; DRIFT! is 192.
                assert!(before <= 16, "{id} {w}x{h}: DRIFT! up before the drift ({before} hot px)");
                assert!(after <= 16, "{id} {w}x{h}: DRIFT! still up after the drift ({after} hot px)");
            }
        }
    }

    /// 380x60 draws the steering gauge and its readout in the top-right sky; 190x48 and 128x44 drop it
    /// (the top-right corner is plain sky).
    #[test]
    fn the_gauge_fits_or_drops() {
        let t = theme("drift-night");
        let lit_c = Rgba::from_hex(&t.lit, 1.0);
        let hot = Rgba::from_hex(&t.hot, 1.0);
        let gauge_px = |w: i32, h: i32| {
            let c = frames(&mut Drift::default(), &t, w, h, 0.15, 60);
            let mut n = 0;
            for y in 4..4 + 2 * GAUGE_R + 2 {
                for x in w - 3 - 60..w - 3 {
                    let p = c.get(x, y);
                    if p == lit_c || p == hot {
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

    /// Dumps for the eye test, composited over `#202020`. Each file is `<name>.<w>x<h>.rgba`.
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
            let mut fam = Drift::default();
            let mut c = Canvas::new(w, h);
            for k in 0..n {
                c.clear();
                fam.draw(&mut c, &t, &frame(level, rms, k as f32 * 0.0167, beat(k)));
            }
            (fam, c, t)
        };
        let flourish = |id: &str, w: i32, h: i32, at: usize| {
            let (mut fam, mut c, t) = run(id, w, h, 0.6, 0.12, 110);
            fam.flourish.force_next();
            for k in 110..110 + at {
                c.clear();
                fam.draw(&mut c, &t, &frame(0.6, 0.12, k as f32 * 0.0167, false));
            }
            c
        };
        for id in IDS {
            let short = &id["drift-".len()..];
            // calm ends between beats; loud ends two frames after a kick.
            for (tag, level, rms, n) in [("calm", 0.3f32, 0.07f32, 250), ("loud", 0.85, 0.2, 266)] {
                let (_, c, _) = run(id, 380, 60, level, rms, n);
                write(format!("drift-{short}-{tag}"), &c);
            }
            // The drift ~250 ms in (mid-sweep, DRIFT! up) and ~500 ms in (the smoke wall).
            write(format!("drift-{short}-flourish"), &flourish(id, 380, 60, 15));
            write(format!("drift-{short}-flourish-late"), &flourish(id, 380, 60, 30));
        }
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
            let m = 54; // the 900 ms drift
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
