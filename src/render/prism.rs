//! The prism family: the user's own image at taskbar scale. A soft, luminous spectral arc over a dark
//! rim-lit horizon on a plum-to-black sky, the arc being the meter.
//!
//! # The scene
//!
//! - **Backdrop (baked once per size and colours into `bg`).** A vertical gradient from `panel` at the
//!   top to near-black at the bottom, with two faint translucent sheets in `edge` (alpha 0.12) sweeping
//!   through the top-left and top-right corners: large soft ellipses with a brighter ridge near their
//!   rim, so they read as curtains of light rather than blobs. Under them the **horizon**: a wide
//!   ellipse whose top edge has its apex at ~70 % of the height and dips to ~90 % at the edges, filled
//!   below with near-black and anti-aliased along its top edge.
//! - **The spectral arc (the meter).** A band on a wider concentric ellipse above the horizon.
//!   Position along it is frequency: 64 samples left to right, their hue from the colourway's spectral
//!   stops (sunset: orange, pink, yellow, green, cyan, blue) mixed in linear light with
//!   `Rgba::lerp_linear`. Each x column interpolates between the two samples either side of it, so
//!   the band is one continuous shape rather than 64 steps. The core's thickness is
//!   `2 + level * 10` px (scaled on a short panel; half above and half below the centre line), its
//!   brightness `0.35 + 0.65 * level`, and it glows through three layered alpha columns: a 4 px halo
//!   at 0.15, a 2 px halo at 0.4 and the core at full, each with fractional (anti-aliased) ends so
//!   the curve has no stair-steps. No `bloom`. The horizon occludes the band: its columns stop at the
//!   horizon's edge.
//! - **Rim-light.** A 1 px line in `hot` along the horizon's top edge (split across two rows by the
//!   edge's sub-pixel position) whose alpha follows the arc's glow directly above it, with a faint
//!   sheen of the same light just under it at the colourway's `ghost` alpha.
//! - **Breathing.** The arc drifts vertically +-2 px on a slow 8 s cycle, and its hue stops rotate by
//!   up to +-4 % with rms, so a loud passage shifts the spectrum slightly - like the light moving.
//!
//! # The flourish - flare
//!
//! 700 ms, fired only on a bass hit. The arc's core brightens to white from the loudest point outward
//! (a travelling highlight with a bright front), the rim-light flares full width, and a soft lens flare
//! (three concentric filled circles at alpha 0.2 / 0.1 / 0.05 in `hot`) blooms at the arc's apex.
//!
//! `FrameData.levels` arrive already smoothed by main's `Smoother`; the arc reads them directly - no
//! second attack/decay here. The panel is opaque (painted first) and the clip to the rounded rect runs
//! last on every path. `draw` allocates nothing after the first frame at a size: the backdrop is a
//! canvas rebuilt only on a size (or colour) change and the arc's geometry lives in fixed arrays.

use std::f32::consts::PI;

use crate::dsp::bands::NUM_BANDS;
use crate::dsp::flourish::{Envelope, Trigger};
use crate::render::canvas::{Canvas, Rgba};
use crate::render::{Family, FrameData};
use crate::themes::Theme;

/// The arc's core: `CORE_MIN + level * CORE_GAIN` px thick on a full-height panel.
const CORE_MIN: f32 = 2.0;
const CORE_GAIN: f32 = 10.0;
/// Brightness `BRIGHT_MIN + (1 - BRIGHT_MIN) * level`.
const BRIGHT_MIN: f32 = 0.35;
/// The two halos: extra px each side of the core, and their alpha.
const HALO_NEAR: f32 = 2.0;
const HALO_NEAR_A: f32 = 0.4;
const HALO_FAR: f32 = 4.0;
const HALO_FAR_A: f32 = 0.15;

/// The core's hot heart: its middle `HEART_FRAC` of the core, lifted toward white by up to
/// `HEART_WHITE` at full level.
const HEART_FRAC: f32 = 0.45;
const HEART_WHITE: f32 = 0.30;

/// The corner sheets' peak alpha.
const SHEET_A: f32 = 0.12;

/// The horizon: its apex and edge heights as fractions of the interior, and how much wider than the
/// interior its ellipse is (so the edges dip rather than drop).
const HORIZON_APEX: f32 = 0.70;
const HORIZON_EDGE: f32 = 0.90;
const HORIZON_RX: f32 = 1.25;
/// Where the arc's apex and the horizon's sit across the width - a touch left of centre, as in the
/// image.
const APEX_X: f32 = 0.46;
/// The arc's centre line at its apex, as a fraction of the interior height.
const ARC_APEX: f32 = 0.45;

/// Breathing: +-`BREATHE_PX` over `BREATHE_S`, and the hue rotation at full rms.
const BREATHE_PX: f32 = 2.0;
const BREATHE_S: f32 = 8.0;
const HUE_ROTATE: f32 = 0.04;

/// The flare: its length, the highlight's travel time, and the lens flare's radii (fractions of the
/// interior height) and alphas.
const FLARE_MS: f32 = 700.0;
const TRAVEL_MS: f32 = 420.0;
/// The highlight's front fades in over this many px, so it has no hard end.
const TAPER: f32 = 24.0;
const FLARE_R: [f32; 3] = [0.44, 0.27, 0.13];
const FLARE_A: [f32; 3] = [0.05, 0.10, 0.20];
/// The flourish only fires on a BASS hit (the `sesh` pattern).
const FLOURISH_BASS_MIN: f32 = 0.6;

/// The arc's columns are held in fixed arrays of this many; a wider panel draws one column per
/// `ceil(iw / MAX_COLS)` px.
const MAX_COLS: usize = 1024;

const WHITE: Rgba = Rgba { r: 255, g: 255, b: 255, a: 255 };
const BLACK: Rgba = Rgba { r: 0, g: 0, b: 0, a: 255 };

const fn hex(v: u32) -> Rgba {
    Rgba { r: (v >> 16) as u8, g: (v >> 8) as u8, b: v as u8, a: 255 }
}

/// The spectral stops, left to right along the arc, per colourway. `lit` in each colourway is the
/// brightest of these (asserted).
const SUNSET: [Rgba; 6] = [hex(0xff5a1a), hex(0xff3a8a), hex(0xffd21a), hex(0x5aff7a), hex(0x3ad2ff), hex(0x5a6aff)];
const AURORA: [Rgba; 4] = [hex(0x3aff8a), hex(0x3affe0), hex(0x3aa0ff), hex(0xa05aff)];
const MONO: [Rgba; 3] = [hex(0xd0d0d8), hex(0xffffff), hex(0xd0d0d8)];
const DAWN: [Rgba; 4] = [hex(0xff7a1a), hex(0xffb03a), hex(0xffe07a), hex(0xfff2d0)];

/// Per-colourway style (the `sesh` pattern), matched on the id; anything unknown is sunset.
#[derive(Clone, Copy)]
struct Style {
    stops: &'static [Rgba],
}

fn style(t: &Theme) -> Style {
    match t.id.as_str() {
        "prism-aurora" => Style { stops: &AURORA },
        "prism-mono" => Style { stops: &MONO },
        "prism-dawn" => Style { stops: &DAWN },
        _ => Style { stops: &SUNSET },
    }
}

/// The colour at `u` (0..1) along evenly spaced stops, mixed in linear light.
fn sample_stops(stops: &[Rgba], u: f32) -> Rgba {
    let n = stops.len();
    if n == 0 {
        return WHITE;
    }
    if n == 1 {
        return stops[0];
    }
    let u = if u.is_finite() { u.clamp(0.0, 1.0) } else { 0.0 };
    let p = u * (n - 1) as f32;
    let i = (p as usize).min(n - 2);
    Rgba::lerp_linear(stops[i], stops[i + 1], p - i as f32)
}

/// One x column of the arc, precomputed per size.
#[derive(Clone, Copy, Default)]
struct Col {
    x: i32,
    /// The arc's centre line and the horizon's top edge at this column (sub-pixel).
    arc_y: f32,
    hor_y: f32,
    /// 0..1 across the arc: the hue position.
    u: f32,
    /// The sample to the left of this column and how far toward the next one it sits.
    band: u8,
    frac: f32,
}

/// Everything positional, for a panel size. Shared by `draw`, the bake and the tests.
#[derive(Clone, Copy)]
struct Layout {
    cols: [Col; MAX_COLS],
    n: usize,
    /// Width of each column, px (1 unless the panel is wider than `MAX_COLS`).
    cw: i32,
    /// Thickness scale (1 on a full-height panel).
    scale: f32,
    /// The arc's apex (x, centre-line y), for the lens flare.
    apex: (i32, f32),
    /// The horizon ellipse: centre and radii.
    hor: (f32, f32, f32, f32),
}

impl Default for Layout {
    fn default() -> Self {
        Layout { cols: [Col::default(); MAX_COLS], n: 0, cw: 1, scale: 1.0, apex: (0, 0.0), hor: (0.0, 0.0, 1.0, 1.0) }
    }
}

fn layout(w: i32, h: i32) -> Layout {
    let (ix0, iy0, ix1, iy1) = (3, 4, w - 3, h - 4);
    let (iw, ih) = ((ix1 - ix0).max(1), (iy1 - iy0).max(1));
    let (iwf, ihf) = (iw as f32, ih as f32);
    let mut l = Layout { scale: (ihf / 52.0).clamp(0.6, 1.0), ..Layout::default() };

    // The horizon: an ellipse `HORIZON_RX` times the interior's half-width, its top edge at the apex
    // fraction under `APEX_X` and at the edge fraction where it leaves the wider side of the panel.
    let cx = ix0 as f32 + APEX_X * iwf;
    let rx = HORIZON_RX * iwf * 0.5;
    let y_apex = iy0 as f32 + HORIZON_APEX * ihf;
    let y_edge = iy0 as f32 + HORIZON_EDGE * ihf;
    let far = (cx - ix0 as f32).max(ix1 as f32 - cx);
    let dip = 1.0 - (1.0 - (far / rx).powi(2)).max(0.0).sqrt();
    let ry = (y_edge - y_apex) / dip.max(0.05);
    let cy = y_apex + ry;
    l.hor = (cx, cy, rx, ry);
    // The arc: concentric, both radii grown by the gap between the two apexes.
    let gap = y_apex - (iy0 as f32 + ARC_APEX * ihf);
    let (arx, ary) = (rx + gap, ry + gap);
    let top = |x: f32, rx: f32, ry: f32| cy - ry * (1.0 - ((x - cx) / rx).powi(2)).max(0.0).sqrt();

    l.cw = ((iw as usize).div_ceil(MAX_COLS)).max(1) as i32;
    let mut x = ix0;
    while x < ix1 && l.n < MAX_COLS {
        let xc = x as f32 + l.cw as f32 * 0.5;
        let u = ((xc - ix0 as f32) / iwf).clamp(0.0, 1.0);
        let s = (u * NUM_BANDS as f32 - 0.5).clamp(0.0, (NUM_BANDS - 1) as f32);
        let band = (s as usize).min(NUM_BANDS - 2);
        l.cols[l.n] = Col {
            x,
            arc_y: top(xc, arx, ary),
            hor_y: top(xc, rx, ry),
            u,
            band: band as u8,
            frac: (s - band as f32).clamp(0.0, 1.0),
        };
        l.n += 1;
        x += l.cw;
    }
    l.apex = (cx.round() as i32, top(cx, arx, ary));
    l
}

/// Which layers draw - a test hook, so the arc and the rim can be measured on their own.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Only {
    All,
    /// The arc alone, on the plain panel.
    Arc,
    /// The backdrop and the rim-light, no arc.
    Rim,
}

pub struct Prism {
    /// Fires the flare on a rare bass hit. `pub(crate)` so the tests can force it.
    pub(crate) flourish: Trigger,
    hit: Envelope,
    elapsed_s: f32,
    /// Where the flare's highlight started: the loudest sample's x at the hit.
    flare_x: f32,
    layout: Box<Layout>,
    layout_dim: (i32, i32),
    /// The baked backdrop. Keyed on size and colours.
    bg: Canvas,
    bg_key: u64,
    only: Only,
}

impl Default for Prism {
    fn default() -> Self {
        Prism {
            flourish: Default::default(),
            hit: Default::default(),
            elapsed_s: 0.0,
            flare_x: 0.0,
            layout: Box::default(),
            layout_dim: (0, 0),
            bg: Canvas::new(1, 1),
            bg_key: 0,
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
    let a = if a.is_finite() { a.clamp(0.0, 1.0) } else { 0.0 };
    Rgba { a: (a * 255.0).round() as u8, ..c }
}

/// Smoothstep.
fn ease(p: f32) -> f32 {
    let p = p.clamp(0.0, 1.0);
    p * p * (3.0 - 2.0 * p)
}

/// FNV-1a over what the baked backdrop depends on. No allocation.
fn colour_key(t: &Theme, w: i32, h: i32) -> u64 {
    let mut k: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |b: u8| {
        k ^= b as u64;
        k = k.wrapping_mul(0x0100_0000_01b3);
    };
    for s in [t.id.as_str(), t.panel.as_str(), t.edge.as_str()] {
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

/// Fills the column `x..x+cw` over the sub-pixel span `y0..y1` (clipped to `lo..hi`) in `col` at
/// alpha `a`, the two end rows at their fractional coverage so a curve has no stair-steps.
#[allow(clippy::too_many_arguments)]
fn vspan(c: &mut Canvas, x: i32, cw: i32, y0: f32, y1: f32, lo: f32, hi: f32, col: Rgba, a: f32) {
    let (y0, y1) = (y0.max(lo), y1.min(hi));
    if y1 <= y0 || a <= 0.0 {
        return;
    }
    let (r0, r1) = (y0.floor() as i32, (y1.ceil() as i32) - 1);
    if r0 == r1 {
        c.fill_rect(x, r0, cw, 1, with_alpha(col, a * (y1 - y0)));
        return;
    }
    c.fill_rect(x, r0, cw, 1, with_alpha(col, a * ((r0 + 1) as f32 - y0)));
    if r1 > r0 + 1 {
        c.fill_rect(x, r0 + 1, cw, r1 - r0 - 1, with_alpha(col, a));
    }
    c.fill_rect(x, r1, cw, 1, with_alpha(col, a * (y1 - r1 as f32)));
}

/// A filled circle of radius `r` at alpha `a` whose edge is anti-aliased over ~2 px, so the lens
/// flare's three nested discs step softly instead of drawing a target.
fn soft_disc(c: &mut Canvas, cx: f32, cy: f32, r: f32, col: Rgba, a: f32) {
    if r <= 0.0 || a <= 0.0 || !cx.is_finite() || !cy.is_finite() {
        return;
    }
    let (x0, x1) = ((cx - r - 1.0).floor() as i32, (cx + r + 1.0).ceil() as i32);
    let (y0, y1) = ((cy - r - 1.0).floor() as i32, (cy + r + 1.0).ceil() as i32);
    for y in y0.max(0)..=y1.min(c.height() - 1) {
        for x in x0.max(0)..=x1.min(c.width() - 1) {
            let d = (x as f32 + 0.5 - cx).hypot(y as f32 + 0.5 - cy);
            let cov = ((r - d) * 0.5 + 0.5).clamp(0.0, 1.0);
            if cov > 0.0 {
                c.fill_rect(x, y, 1, 1, with_alpha(col, a * cov));
            }
        }
    }
}

impl Prism {
    #[cfg(test)]
    fn only_for_test(&mut self, only: Only) {
        self.only = only;
    }

    /// Rebuilds the geometry on a size change and the baked backdrop on a size or colour change.
    fn resize(&mut self, w: i32, h: i32, t: &Theme) {
        if self.layout_dim != (w, h) {
            *self.layout = layout(w, h);
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
        let l = &*self.layout;
        let bg = &mut self.bg;
        bg.clear();
        let panel = Rgba::from_hex(&t.panel, 1.0);
        let edge = Rgba::from_hex(&t.edge, 1.0);
        let (ix0, iy0, ix1, iy1) = (3, 4, w - 3, h - 4);
        let (iw, ih) = (ix1 - ix0, iy1 - iy0);

        // The sky: panel at the top to near-black at the bottom.
        let deep = Rgba::lerp_linear(panel, BLACK, 0.85);
        for y in iy0..iy1 {
            let f = (y - iy0) as f32 / (ih - 1).max(1) as f32;
            bg.fill_rect(ix0, y, iw, 1, Rgba::lerp_linear(panel, deep, f.powf(1.2)));
        }
        // The sheets: two large soft ellipses whose centres sit above the corners, brighter on a
        // ridge just inside their rim and fading to nothing past it.
        let (iwf, ihf) = (iw as f32, ih as f32);
        let sheets = [
            (ix0 as f32 - 0.02 * iwf, iy0 as f32 - 0.55 * ihf, 0.36 * iwf, 1.25 * ihf),
            (ix1 as f32 + 0.04 * iwf, iy0 as f32 - 0.70 * ihf, 0.30 * iwf, 1.30 * ihf),
        ];
        for y in iy0..iy1 {
            for x in ix0..ix1 {
                let mut a = 0.0f32;
                for &(sx, sy, rx, ry) in &sheets {
                    let d = (((x as f32 - sx) / rx).powi(2) + ((y as f32 - sy) / ry).powi(2)).sqrt();
                    let ridge = (-((d - 0.88) / 0.09).powi(2)).exp();
                    let body = 0.45 * ease((1.0 - d) / 0.5);
                    a = a.max((ridge + body).min(1.0));
                }
                if a > 0.01 {
                    bg.fill_rect(x, y, 1, 1, with_alpha(edge, SHEET_A * a));
                }
            }
        }
        // The horizon: near-black below its top edge, the edge row at its sub-pixel coverage.
        let ground = Rgba::lerp_linear(panel, BLACK, 0.92);
        for col in &l.cols[..l.n] {
            vspan(bg, col.x, l.cw, col.hor_y, iy1 as f32, iy0 as f32, iy1 as f32, ground, 1.0);
        }
    }
}

impl Family for Prism {
    fn id(&self) -> &'static str {
        "prism"
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

        // ---- the flare ----
        let triggered = self.flourish.update(&d.levels, dt, t.flourish);
        let bass_gate = triggered && bass >= FLOURISH_BASS_MIN;
        #[cfg(test)]
        let fired = bass_gate || (triggered && self.flourish.was_forced());
        #[cfg(not(test))]
        let fired = bass_gate;
        let env = self.hit.update(fired, dt, FLARE_MS);
        let since_ms = (1.0 - env) * FLARE_MS;
        if fired {
            // The highlight starts at the loudest sample.
            let mut best = 0;
            for (i, &v) in d.levels.iter().enumerate() {
                if lvl(v) > lvl(d.levels[best]) {
                    best = i;
                }
            }
            self.flare_x = ix0 as f32 + (best as f32 + 0.5) / NUM_BANDS as f32 * iw as f32;
        }
        let flaring = env > 0.0;
        // The highlight's front, travelling outward from the loudest point, and the fade-out over
        // the flare's last 40 %.
        let reach = (self.flare_x - ix0 as f32).max(ix1 as f32 - self.flare_x) + 8.0;
        let front = reach * ease(since_ms / TRAVEL_MS);
        let fade = (env / 0.4).min(1.0);

        // ---- breathing ----
        let phase = 2.0 * PI * e / BREATHE_S;
        let breathe = BREATHE_PX * phase.sin();
        let shift = HUE_ROTATE * rms_norm * phase.cos();

        let st = style(t);
        let hot = Rgba::from_hex(&t.hot, 1.0);
        let ghost = if t.ghost.is_finite() { t.ghost.clamp(0.0, 0.5) } else { 0.1 };
        let l = &*self.layout;
        let only = self.only;
        let (lo, hi_clip) = (iy0 as f32, iy1 as f32);

        if only != Only::Arc {
            // ---- the baked backdrop ----
            c.copy_region(&self.bg, (ix0, iy0), (ix0, iy0), iw, ih);
        }

        for col in &l.cols[..l.n] {
            let b = col.band as usize;
            let v = lvl(d.levels[b]) + (lvl(d.levels[b + 1]) - lvl(d.levels[b])) * col.frac;
            let bright = BRIGHT_MIN + (1.0 - BRIGHT_MIN) * v;

            // The flare's highlight at this column: white behind the front, brightest at it.
            let white = if flaring {
                let dd = (col.x as f32 - self.flare_x).abs();
                // A soft front: a bright crest at the front with a tail that tapers in over
                // `TAPER` px, over a steady white behind it - no hard-ended bar.
                let crest = (-((front - dd) / 9.0).powi(2)).exp();
                let behind = if dd <= front { 0.65 * ease((front - dd) / TAPER) } else { 0.0 };
                (behind + 0.35 * crest).min(1.0) * fade
            } else {
                0.0
            };

            if only != Only::Rim {
                // ---- the spectral arc: three layered alpha columns ----
                let hue = sample_stops(st.stops, col.u + shift);
                let half = 0.5 * (CORE_MIN + v * CORE_GAIN * l.scale);
                let cy = col.arc_y + breathe;
                // The horizon stands in front of the light.
                let bottom = col.hor_y.min(hi_clip);
                let far = HALO_FAR * l.scale.max(0.75);
                let near = HALO_NEAR;
                vspan(c, col.x, l.cw, cy - half - far, cy + half + far, lo, bottom, hue, HALO_FAR_A * bright);
                vspan(c, col.x, l.cw, cy - half - near, cy + half + near, lo, bottom, hue, HALO_NEAR_A * bright);
                let core = Rgba::lerp_linear(hue, WHITE, 0.9 * white);
                let core_a = bright + (1.0 - bright) * white;
                vspan(c, col.x, l.cw, cy - half, cy + half, lo, bottom, core, core_a);
                // The light's hot heart: the middle of the core, lifted toward white with the level.
                let heart = Rgba::lerp_linear(core, WHITE, HEART_WHITE * v);
                let hh = half * HEART_FRAC;
                vspan(c, col.x, l.cw, cy - hh, cy + hh, lo, bottom, heart, core_a);
            }

            if only != Only::Arc {
                // ---- the rim-light: its alpha follows the glow above; the flare lights it all ----
                let a = (0.15 + 0.85 * v).max(if flaring { fade } else { 0.0 });
                // A 1 px line centred on the edge, anti-aliased across the rows it straddles.
                let hy = col.hor_y;
                vspan(c, col.x, l.cw, hy - 0.5, hy + 0.5, lo, hi_clip, hot, a);
                // A faint sheen of the same light on the dark ground just under it.
                vspan(c, col.x, l.cw, hy + 0.5, hy + 2.5, lo, hi_clip, hot, ghost * a);
            }
        }

        // ---- the lens flare at the apex ----
        if only == Only::All && flaring {
            let (ax, ay) = l.apex;
            let bloom = ease(since_ms / 180.0) * fade;
            for (r, a) in FLARE_R.iter().zip(FLARE_A) {
                let rr = r * ih as f32 * (0.7 + 0.3 * bloom);
                soft_disc(c, ax as f32, ay + breathe, rr, hot, a * bloom);
            }
        }

        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::canvas::Canvas;

    const IDS: [&str; 4] = ["prism-sunset", "prism-aurora", "prism-mono", "prism-dawn"];

    fn theme(id: &str) -> Theme {
        crate::themes::builtin::all().into_iter().find(|t| t.id == id).unwrap()
    }
    /// `n` frames with only `bands` at `level`, the rest silent, and the given rms.
    #[allow(clippy::too_many_arguments)]
    fn frames_bands(
        fam: &mut Prism,
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
    fn frames(fam: &mut Prism, t: &Theme, w: i32, h: i32, level: f32, n: usize) -> Canvas {
        frames_bands(fam, t, w, h, 0..NUM_BANDS, level, level * 0.25, n)
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
    /// Painted arc pixels (arc-only hook) in the columns `x0..x1`.
    fn arc_px(c: &Canvas, t: &Theme, x0: i32, x1: i32) -> usize {
        let panel = Rgba::from_hex(&t.panel, 1.0);
        let mut n = 0;
        for y in 4..c.height() - 4 {
            for x in x0..x1 {
                if drew_over_panel(c.get(x, y), panel) {
                    n += 1;
                }
            }
        }
        n
    }
    fn lum(p: Rgba) -> i32 {
        p.r as i32 + p.g as i32 + p.b as i32
    }
    fn dist(a: Rgba, b: Rgba) -> i32 {
        (a.r as i32 - b.r as i32).abs() + (a.g as i32 - b.g as i32).abs() + (a.b as i32 - b.b as i32).abs()
    }
    /// WCAG relative luminance.
    fn rel_lum(c: Rgba) -> f32 {
        let ch = |v: u8| {
            let v = v as f32 / 255.0;
            if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * ch(c.r) + 0.7152 * ch(c.g) + 0.0722 * ch(c.b)
    }

    #[test]
    fn registered_and_labelled() {
        assert!(crate::render::KNOWN_FAMILIES.contains(&"prism"));
        assert_eq!(crate::render::family_for("prism").id(), "prism");
        assert_eq!(crate::themes::family_label("prism"), "Prism: light bloom");
        let ids: Vec<String> =
            crate::themes::builtin::all().into_iter().filter(|t| t.family == "prism").map(|t| t.id).collect();
        assert_eq!(ids, IDS.map(String::from).to_vec());
    }

    /// Each colourway's `lit` (the contrast rule's colour) is the brightest of its spectral stops, and
    /// its stops are the spec's table.
    #[test]
    fn lit_is_the_brightest_stop() {
        let want: [&[Rgba]; 4] = [&SUNSET, &AURORA, &MONO, &DAWN];
        for (id, stops) in IDS.iter().zip(want) {
            let t = theme(id);
            assert_eq!(style(&t).stops, stops, "{id}: wrong stops");
            let brightest = stops.iter().copied().fold(stops[0], |a, b| if rel_lum(b) > rel_lum(a) { b } else { a });
            assert_eq!(Rgba::from_hex(&t.lit, 1.0), brightest, "{id}: lit {} is not the brightest stop", t.lit);
        }
        // An unknown id falls back to sunset.
        let other = Theme { id: "prism-custom".into(), ..theme("prism-mono") };
        assert_eq!(style(&other).stops, &SUNSET[..]);
    }

    /// Silence still shows the scene: the sky, the sheets, the horizon, a thin arc and its rim.
    #[test]
    fn rest_frame_is_not_empty() {
        for id in IDS {
            let t = theme(id);
            for (w, h) in [(380, 60), (190, 48), (128, 44)] {
                let c = frames(&mut Prism::default(), &t, w, h, 0.0, 10);
                let n = lit(&c, &t);
                // The backdrop alone (gradient, sheets, horizon) is most of it; a thin arc and rim
                // without it would be ~20 %.
                assert!(n as f32 >= 0.35 * (w * h) as f32, "{id} {w}x{h}: only {n} px at rest");
                // And the arc itself is there, thin, across the width.
                let mut fam = Prism::default();
                fam.only_for_test(Only::Arc);
                let a = frames(&mut fam, &t, w, h, 0.0, 10);
                for third in 0..3 {
                    let x0 = 3 + third * (w - 6) / 3;
                    let px = arc_px(&a, &t, x0, x0 + (w - 6) / 3);
                    assert!(px >= ((w - 6) / 3) as usize, "{id} {w}x{h}: the silent arc is missing in third {third} ({px} px)");
                }
            }
        }
    }

    /// The arc's visible thickness summed over the columns `x0..x1` (arc-only hook): per column, the
    /// pixels at least a quarter as bright (in linear light, over the panel) as that column's core.
    /// The halo's faint outer tail is glow, not thickness - with it counted even a silent arc is
    /// ~10 px "thick", since a 5 % alpha of a bright colour is plainly visible in linear light.
    fn arc_thickness(c: &Canvas, t: &Theme, x0: i32, x1: i32) -> usize {
        let panel = Rgba::from_hex(&t.panel, 1.0);
        let lin = |p: Rgba| {
            Rgba::srgb_to_linear(p.r) + Rgba::srgb_to_linear(p.g) + Rgba::srgb_to_linear(p.b)
        };
        let base = lin(panel);
        let mut n = 0;
        let mut ex = [0.0f32; 64];
        for x in x0..x1 {
            let rows = (c.height() - 8).min(64) as usize;
            let mut peak = 0.0f32;
            for (i, e) in ex[..rows].iter_mut().enumerate() {
                *e = lin(c.get(x, 4 + i as i32)) - base;
                peak = peak.max(*e);
            }
            if peak > 0.02 {
                n += ex[..rows].iter().filter(|&&e| e >= 0.25 * peak).count();
            }
        }
        n
    }

    /// Bass-only: the arc's visible thickness over the left third is at least twice the right third's.
    #[test]
    fn the_arc_swells_where_the_music_is() {
        for id in IDS {
            let t = theme(id);
            for (w, h) in [(380, 60), (128, 44)] {
                let mut fam = Prism::default();
                fam.only_for_test(Only::Arc);
                let c = frames_bands(&mut fam, &t, w, h, 0..20, 0.9, 0.1, 30);
                let third = (w - 6) / 3;
                let left = arc_thickness(&c, &t, 3, 3 + third);
                let right = arc_thickness(&c, &t, w - 3 - third, w - 3);
                assert!(left >= 2 * right.max(1), "{id} {w}x{h}: bass-only, left third {left} px vs right third {right} px");
                // And the other way round on treble.
                let mut fam = Prism::default();
                fam.only_for_test(Only::Arc);
                let c = frames_bands(&mut fam, &t, w, h, 44..64, 0.9, 0.1, 30);
                let (left, right) = (arc_thickness(&c, &t, 3, 3 + third), arc_thickness(&c, &t, w - 3 - third, w - 3));
                assert!(right >= 2 * left.max(1), "{id} {w}x{h}: treble-only, right third {right} px vs left third {left} px");
            }
        }
    }

    /// The brightest pixel in column `x` of the arc (arc-only hook).
    fn column_peak(c: &Canvas, x: i32) -> Rgba {
        let mut best = Rgba::TRANSPARENT;
        for y in 4..c.height() - 4 {
            let p = c.get(x, y);
            if lum(p) > lum(best) {
                best = p;
            }
        }
        best
    }

    /// At a uniform level the arc's leftmost pixel is closer to the first stop than to the last, and
    /// the rightmost the other way round.
    #[test]
    fn hue_runs_along_the_arc() {
        // Mono's ends are the same silver, so it is exempt from the ends test (its middle is white).
        for id in ["prism-sunset", "prism-aurora", "prism-dawn"] {
            let t = theme(id);
            let stops = style(&t).stops;
            let (first, last) = (stops[0], stops[stops.len() - 1]);
            for (w, h) in [(380, 60), (128, 44)] {
                let mut fam = Prism::default();
                fam.only_for_test(Only::Arc);
                // Mid level: the core is at 0.68 brightness and its heart only a little whitened.
                let c = frames(&mut fam, &t, w, h, 0.5, 30);
                let (l, r) = (column_peak(&c, 4), column_peak(&c, w - 5));
                assert!(dist(l, first) < dist(l, last), "{id} {w}x{h}: leftmost {l:?} not nearer {first:?} than {last:?}");
                assert!(dist(r, last) < dist(r, first), "{id} {w}x{h}: rightmost {r:?} not nearer {last:?} than {first:?}");
            }
        }
        // Mono: the middle is whiter than the ends.
        let t = theme("prism-mono");
        let mut fam = Prism::default();
        fam.only_for_test(Only::Arc);
        let c = frames(&mut fam, &t, 380, 60, 1.0, 30);
        assert!(lum(column_peak(&c, 190)) > lum(column_peak(&c, 4)), "mono: the middle is not the white stop");
    }

    /// The rim-light's summed brightness along the horizon edge (rim-only hook), in columns `x0..x1`.
    fn rim_sum(c: &Canvas, l: &Layout, x0: i32, x1: i32) -> i64 {
        let mut s = 0i64;
        for col in &l.cols[..l.n] {
            if col.x < x0 || col.x >= x1 {
                continue;
            }
            let ry = col.hor_y.floor() as i32;
            s += (lum(c.get(col.x, ry - 1)) + lum(c.get(col.x, ry)) + lum(c.get(col.x, ry + 1))) as i64;
        }
        s
    }

    /// Bass-only, the rim under the loud left end is brighter than under the quiet right end, and
    /// treble-only the other way round; the flare lights it full width.
    #[test]
    fn the_rim_light_follows_the_glow() {
        for id in IDS {
            let t = theme(id);
            for (w, h) in [(380, 60), (128, 44)] {
                let l = layout(w, h);
                let third = (w - 6) / 3;
                let run = |bands: std::ops::Range<usize>| {
                    let mut fam = Prism::default();
                    fam.only_for_test(Only::Rim);
                    frames_bands(&mut fam, &t, w, h, bands, 0.9, 0.0, 20)
                };
                let bass = run(0..20);
                let (bl, br) = (rim_sum(&bass, &l, 3, 3 + third), rim_sum(&bass, &l, w - 3 - third, w - 3));
                assert!(bl as f32 > 1.3 * br as f32, "{id} {w}x{h}: bass-only rim left {bl} vs right {br}");
                let treble = run(44..64);
                let (tl, tr) = (rim_sum(&treble, &l, 3, 3 + third), rim_sum(&treble, &l, w - 3 - third, w - 3));
                assert!(tr as f32 > 1.3 * tl as f32, "{id} {w}x{h}: treble-only rim right {tr} vs left {tl}");
                // Silence, then a forced flare: the quiet rim lights up across the width.
                let mut fam = Prism::default();
                fam.only_for_test(Only::Rim);
                let quiet = frames_bands(&mut fam, &t, w, h, 0..0, 0.0, 0.0, 20);
                fam.flourish.force_next();
                let flare = frames_bands(&mut fam, &t, w, h, 0..0, 0.0, 0.0, 5);
                for k in 0..3 {
                    let x0 = 3 + k * third;
                    let (q, f) = (rim_sum(&quiet, &l, x0, x0 + third), rim_sum(&flare, &l, x0, x0 + third));
                    assert!(f as f32 > 1.3 * q as f32, "{id} {w}x{h}: the flare did not light rim third {k}: {q} -> {f}");
                }
            }
        }
    }

    /// The core's whiteness: the mean of each column's peak pixel's weakest channel (a saturated hue
    /// has one low channel; white has none).
    fn core_whiteness(c: &Canvas, w: i32) -> f32 {
        let mut s = 0.0;
        for x in 4..w - 4 {
            let p = column_peak(c, x);
            s += p.r.min(p.g).min(p.b) as f32;
        }
        s / (w - 8) as f32
    }

    /// A forced flare whitens the arc's core along its length and it recovers after.
    #[test]
    fn flare_flourish_brightens_the_core() {
        for id in IDS {
            let t = theme(id);
            for (w, h) in [(380, 60), (128, 44)] {
                let mut fam = Prism::default();
                fam.only_for_test(Only::Arc);
                let before = core_whiteness(&frames(&mut fam, &t, w, h, 0.5, 30), w);
                fam.flourish.force_next();
                // ~350 ms in: the highlight has travelled most of the way out.
                let during = core_whiteness(&frames(&mut fam, &t, w, h, 0.5, 21), w);
                let _ = frames(&mut fam, &t, w, h, 0.5, 40);
                let after = core_whiteness(&frames(&mut fam, &t, w, h, 0.5, 1), w);
                // At least half the way from where it was to pure white (mono's silver has little headroom).
                assert!(during - before >= 0.5 * (255.0 - before), "{id} {w}x{h}: core whiteness {before:.0} -> {during:.0}");
                assert!((after - before).abs() < 8.0, "{id} {w}x{h}: after the flare {after:.0} vs {before:.0}");
            }
        }
    }

    /// 190x48 and 128x44 with a forced flare: no panic, something drawn, nothing outside the panel.
    #[test]
    fn fits_the_narrow_panel_and_flourish_does_not_panic() {
        for id in IDS {
            let t = theme(id);
            for (w, h) in [(190, 48), (128, 44)] {
                let mut fam = Prism::default();
                let _ = frames(&mut fam, &t, w, h, 1.0, 5);
                fam.flourish.force_next();
                for n in [1usize, 5, 10, 30] {
                    let c = frames(&mut fam, &t, w, h, 1.0, n);
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
                // The arc and the horizon scale: the loud arc's top halo and the horizon's edge are
                // inside the interior at every column.
                let l = layout(w, h);
                for col in &l.cols[..l.n] {
                    let top = col.arc_y - BREATHE_PX - 0.5 * (CORE_MIN + CORE_GAIN * l.scale) - HALO_FAR;
                    assert!(top >= 4.0, "{id} {w}x{h}: the arc leaves the top at x {} ({top:.1})", col.x);
                    assert!(col.hor_y < (h - 4) as f32 && col.hor_y > col.arc_y, "{id} {w}x{h}: horizon at x {}", col.x);
                }
            }
        }
    }

    #[test]
    fn the_four_colourways_are_visibly_different() {
        let (w, h) = (380i32, 60i32);
        let canvases: Vec<Canvas> =
            IDS.iter().map(|id| frames(&mut Prism::default(), &theme(id), w, h, 0.5, 30)).collect();
        let (ix0, iy0, ix1, iy1) = (3, 4, w - 3, h - 4);
        let interior = ((ix1 - ix0) * (iy1 - iy0)) as f32;
        let mut too_similar = Vec::new();
        for a in 0..IDS.len() {
            for b in (a + 1)..IDS.len() {
                let mut diff = 0usize;
                for y in iy0..iy1 {
                    for x in ix0..ix1 {
                        if dist(canvases[a].get(x, y), canvases[b].get(x, y)) > 24 {
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

    /// Dumps for the eye test, composited over `#202020`. Each file is `<name>.<w>x<h>.rgba`.
    ///
    /// Run: cargo test --release dump_prism -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_prism() {
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
        // A music-like frame: bass-heavy with a mid bump, wobbling; `rms` as main would see it.
        let frame = |level: f32, rms: f32, t_s: f32, beat: bool| {
            let mut d = FrameData { dt_ms: 16.7, time_s: t_s, ..FrameData::default() };
            for (i, v) in d.levels.iter_mut().enumerate() {
                let f = i as f32 / NUM_BANDS as f32;
                let shape = (1.0 - f).powf(1.2) * 0.6 + 0.35 * (-((f - 0.45) / 0.12).powi(2)).exp() + 0.1;
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
            let mut fam = Prism::default();
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
            let short = &id["prism-".len()..];
            for (tag, level, rms, n) in [("calm", 0.3f32, 0.07f32, 250), ("loud", 0.85, 0.2, 266)] {
                let (_, c, _) = run(id, 380, 60, level, rms, n);
                write(format!("prism-{short}-{tag}"), &c);
            }
            write(format!("prism-{short}-flourish"), &flourish(id, 380, 60, 12));
        }
        write("prism-sunset-flourish-late".into(), &flourish("prism-sunset", 380, 60, 26));
        let (_, c, _) = run("prism-sunset", 380, 60, 0.0, 0.0, 60);
        write("prism-sunset-silent".into(), &c);
        for (w, h) in [(190, 48), (128, 44)] {
            let (_, c, _) = run("prism-sunset", w, h, 0.85, 0.2, 266);
            write(format!("prism-sunset-{w}x{h}"), &c);
            write(format!("prism-sunset-{w}x{h}-flourish"), &flourish("prism-sunset", w, h, 12));
        }
        println!("wrote prism dumps to {}", dir.display());
    }

    /// Per-frame cost, steady and during the flare, at 380x60.
    ///
    /// Run: cargo test --release probe_prism_cost -- --ignored --nocapture
    #[test]
    #[ignore]
    fn probe_prism_cost() {
        for id in IDS {
            let t = theme(id);
            let mut fam = Prism::default();
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
            let m = 42; // the 700 ms flare
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
