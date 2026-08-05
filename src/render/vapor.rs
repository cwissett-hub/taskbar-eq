//! The vaporwave grid family: a sunset over a perspective grid, where the grid carries the
//! audio.
//!
//! Unlike `segmented`/`scope`/`vu`, which are instruments, this one is a scene. Every
//! geometric constant here came from a live browser tuner the user drove; see
//! `docs/superpowers/specs/2026-07-31-vaporwave-grid-family-design.md`, whose §7 records
//! which of my own composition guesses were wrong. The parameter names are a published
//! schema the moment a theme file sets one, so they match the spec exactly.
//!
//! Two techniques here are not optional, both found by fixing "it started to look muddy":
//!
//! - **Hidden-line removal.** Horizontal lines are drawn far-to-near, and each one fills
//!   the area between itself and the bottom of the canvas with opaque ground BEFORE it is
//!   stroked, so every ridge occludes what is behind it. Stroking all the lines and then
//!   filling produces overlapping spaghetti.
//! - **Half-pixel snapping.** A 1px stroke at a fractional y anti-aliases across two rows
//!   as grey mush. At 60px tall, snapping is the difference between a wireframe and a
//!   smear. `Canvas::line` takes integers, which snaps by construction - the thing to avoid
//!   is accumulating fractional offsets before the cast.

use super::canvas::{Canvas, Rgba};
use super::{Family, FrameData};
use crate::themes::Theme;

/// Bass level below which a lightning strike will not fire regardless of the transient.
const BOLT_FLOOR: f32 = 0.35;

/// Level a segment's mean must exceed before it is re-stroked as a peak glow.
const GLOW_AT: f32 = 0.55;

/// Reference frame duration. The render loop sleeps a fixed 16ms per tick, so its real
/// period is 16ms plus however long the frame took; scroll and bolt decay are scaled by the
/// measured `dt_ms` against this so they run at the same speed when the loop slows.
const NOMINAL_DT_MS: f32 = 16.7;

#[derive(Default)]
pub struct Vapor {
    /// Grid scroll phase, wrapping 0->1.
    scroll: f32,
    /// Previous frame's bass mean, for transient detection.
    prev_bass: f32,
    /// Decaying brightness of the current lightning strike, 0 when none.
    bolt: f32,
    /// Advanced on each strike so successive bolts take different paths.
    bolt_seed: u32,
}

/// Deterministic value hash. Used for bolt paths instead of a real RNG so a given seed
/// always draws the same bolt, which keeps the renderer reproducible for goldens.
fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

/// `hash` mapped to -1.0..=1.0.
fn hash_signed(seed: u32, n: u32) -> f32 {
    (hash(seed ^ n.wrapping_mul(0x9e37_79b9)) as f32 / u32::MAX as f32) * 2.0 - 1.0
}

impl Vapor {
    /// Moving average over the band levels, so the terrain reads as rolling hills rather
    /// than spikes. Radius comes from the theme's `smoothing`.
    fn smoothed(d: &FrameData, smoothing: f32) -> Vec<f32> {
        let n = d.levels.len();
        let r = (smoothing.clamp(0.0, 1.0) * 5.0).round() as usize;
        if r == 0 {
            return d.levels.iter().map(|v| if v.is_finite() { *v } else { 0.0 }).collect();
        }
        (0..n)
            .map(|i| {
                let lo = i.saturating_sub(r);
                let hi = (i + r + 1).min(n);
                let mut acc = 0.0;
                let mut cnt = 0.0;
                for v in &d.levels[lo..hi] {
                    if v.is_finite() {
                        acc += *v;
                        cnt += 1.0;
                    }
                }
                if cnt > 0.0 {
                    acc / cnt
                } else {
                    0.0
                }
            })
            .collect()
    }

    /// Advances the bass-transient detector and returns the current strike brightness.
    ///
    /// Triggered by a RISE in bass rather than a timer, so strikes land on kick drums.
    /// Threshold and decay both scale with the theme so a colourway can be calmer.
    fn update_bolt(&mut self, levels: &[f32], t: &crate::themes::VaporParams, dt: f32) -> f32 {
        let bass: f32 = {
            let n = levels.len().min(4).max(1);
            levels[..n].iter().filter(|v| v.is_finite()).sum::<f32>() / n as f32
        };
        let need = 0.04 + (1.0 - t.bolt_sens.clamp(0.0, 1.0)) * 0.26;
        if bass - self.prev_bass > need && bass > BOLT_FLOOR {
            self.bolt = t.bolt_bright.clamp(0.0, 1.0);
            self.bolt_seed = self.bolt_seed.wrapping_add(1);
        }
        self.prev_bass = if bass.is_finite() { bass } else { 0.0 };
        self.bolt = (self.bolt - t.bolt_decay.clamp(0.0, 1.0) * 0.09 * dt).max(0.0);
        self.bolt
    }

    /// Builds the sun on its own layer: gradient body clipped to the upper semicircle, with
    /// the slots cut out and an optional rim.
    ///
    /// On a layer rather than straight onto the canvas because the body is a vertical
    /// gradient that has to be *clipped* to a circle - there is no clip-to-circle
    /// primitive, so the gradient is drawn over the bounding box and the corners are
    /// punched out per row, which would erase the sky if done in place.
    fn sun_layer(w: i32, h: i32, cx: i32, cy: i32, r: i32, t: &crate::themes::VaporParams) -> Canvas {
        let mut layer = Canvas::new(w, h);
        if r <= 0 {
            return layer;
        }
        let top = cy - r;
        layer.vertical_gradient(
            cx - r,
            top,
            r * 2,
            r,
            &[
                (0.0, Rgba::from_hex(&t.sun_crown, 1.0)),
                (0.35, Rgba::from_hex(&t.sun_upper, 1.0)),
                (0.70, Rgba::from_hex(&t.sun_lower, 1.0)),
                (1.0, Rgba::from_hex(&t.sun_base, 1.0)),
            ],
            // Dithered: a 20px-tall gradient across four stops bands visibly without it.
            true,
        );

        // Clip to the semicircle by punching each row back to the circle's half-width.
        for y in top..=cy {
            let dy = (cy - y) as f32;
            let half = ((r as f32 * r as f32) - dy * dy).max(0.0).sqrt() as i32;
            layer.punch_rect(cx - r - 1, y, r - half + 1, 1);
            layer.punch_rect(cx + half, y, r - half + 2, 1);
        }

        // Slots: horizontal gaps. At the approved slot_bias of 0 every slot is 1px and
        // uniform - the user explicitly preferred that over widening toward the horizon.
        let slots = t.slots.max(0);
        for i in 0..slots {
            let pos = t.slot_top + (1.0 - t.slot_top) * (i as f32 + 1.0) / (slots as f32 + 1.0);
            let thick =
                (1.0 + t.slot_bias.max(0.0) * pos * (r as f32 * 0.22).max(2.0)).round().max(1.0) as i32;
            let y = top + (pos * r as f32).round() as i32;
            layer.punch_rect(cx - r, y, r * 2, thick);
        }

        if t.sun_rim {
            let rim = Rgba::from_hex("#fffae6", 0.75);
            let steps = (r * 4).max(8);
            let (a0, a1) = (std::f32::consts::PI * 1.08, std::f32::consts::PI * 1.92);
            let mut prev: Option<(i32, i32)> = None;
            for s in 0..=steps {
                let ang = a0 + (a1 - a0) * s as f32 / steps as f32;
                let p = (
                    cx + (ang.cos() * r as f32).round() as i32,
                    cy + (ang.sin() * r as f32).round() as i32,
                );
                if let Some(q) = prev {
                    layer.line(q.0, q.1, p.0, p.1, rim);
                }
                prev = Some(p);
            }
        }
        layer
    }

    /// Draws one lightning strike: a wide dim pass, a tight bright core, and one fork.
    fn draw_bolt(c: &mut Canvas, w: i32, horizon: i32, seed: u32, bright: f32, t: &crate::themes::VaporParams) {
        if bright <= 0.0 {
            return;
        }
        let wide = Rgba::from_hex("#9fe8ff", (bright * 0.30).clamp(0.0, 1.0));
        let core = Rgba::from_hex("#eafcff", bright.clamp(0.0, 1.0));

        // Path from the top of the panel down to the horizon, jittered per segment.
        let segs = 7;
        let x0 = w / 2 + (hash_signed(seed, 0) * (w as f32 * 0.28)) as i32;
        let mut pts: Vec<(i32, i32)> = Vec::with_capacity(segs as usize + 1);
        for s in 0..=segs {
            let f = s as f32 / segs as f32;
            let y = 2 + (f * (horizon - 2) as f32).round() as i32;
            let x = x0 + (hash_signed(seed, s as u32 + 1) * (w as f32 * 0.06)) as i32;
            pts.push((x, y));
        }
        for pair in pts.windows(2) {
            // Wide pass first, offset either side, so the core lands on top of it.
            for dx in -1..=1 {
                c.line(pair[0].0 + dx, pair[0].1, pair[1].0 + dx, pair[1].1, wide);
            }
        }
        for pair in pts.windows(2) {
            c.line(pair[0].0, pair[0].1, pair[1].0, pair[1].1, core);
        }

        // One fork, starting 45% down.
        let start = (pts.len() as f32 * 0.45) as usize;
        if start + 1 < pts.len() {
            let mut fx = pts[start].0;
            let mut fy = pts[start].1;
            for k in 0..3 {
                let nx = fx + (hash_signed(seed, 100 + k) * (w as f32 * 0.10)) as i32;
                let ny = fy + ((horizon - fy) as f32 * 0.28).round().max(1.0) as i32;
                c.line(fx, fy, nx, ny, core);
                fx = nx;
                fy = ny;
            }
        }
        let _ = t;
    }
}

impl Family for Vapor {
    fn id(&self) -> &'static str {
        "vapor"
    }

    fn draw(&mut self, c: &mut Canvas, theme: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());
        let t = &theme.vapor;
        // `clamp` does NOT sanitise NaN - every comparison against NaN is false, so it
        // falls through and returns NaN, which then poisons the scroll phase permanently.
        // Caught by this family's own NaN test.
        let dt = if d.dt_ms.is_finite() {
            (d.dt_ms / NOMINAL_DT_MS).clamp(0.25, 4.0)
        } else {
            1.0
        };

        let levels = Self::smoothed(d, t.smoothing);
        let bolt = self.update_bolt(&levels, t, dt);

        self.scroll = (self.scroll + t.scroll * 0.010 * dt).fract();

        let horizon = (h as f32 * t.horizon).round() as i32;
        let ground_h = (h - horizon - 2).max(1);
        let vpx = w / 2;

        c.clear();
        c.rounded_rect(1, 2, w - 2, h - 4, 4, Rgba::from_hex(&theme.panel, theme.panel_alpha));

        // Sky. Warmer (pinker) at the horizon as `warmth` rises.
        let flash = (bolt * t.sky_flash).clamp(0.0, 1.0);
        let lift = |hex: &str| {
            let base = Rgba::from_hex(hex, 1.0);
            if flash <= 0.0 {
                return base;
            }
            let mix = |v: u8| (v as f32 + (255.0 - v as f32) * flash * 0.55).min(255.0) as u8;
            Rgba::new(mix(base.r), mix(base.g), mix(base.b), 255)
        };
        c.vertical_gradient(
            1,
            2,
            w - 2,
            horizon - 2,
            &[(0.0, lift(&t.sky_top)), (1.0, lift(&t.sky_horizon))],
            true,
        );

        // Sun, after the sky and before the ground, so the horizon cuts it off naturally.
        let r = (h as f32 * 0.34 * t.sun).round() as i32;
        let sun = Self::sun_layer(w, h, vpx, horizon, r, t);
        c.draw_over(&sun);

        if bolt > 0.0 {
            Self::draw_bolt(c, w, horizon, self.bolt_seed, bolt * t.bolt_bright, t);
        }

        // Ground plane below the horizon, so the grid has something to occlude against.
        let ground = Rgba::from_hex(&t.ground, 1.0);
        c.fill_rect(1, horizon, w - 2, h - horizon - 2, ground);

        let grid_lift = (bolt * t.grid_flash).clamp(0.0, 1.0);
        let grid_alpha = (0.55 + 0.45 * grid_lift).clamp(0.0, 1.0);
        let grid = Rgba::from_hex(&theme.lit, grid_alpha);
        let hot = Rgba::from_hex(&theme.hot, 1.0);

        let persp = (t.persp).max(0.1);
        let amp_max = (horizon - 4).max(1) as f32 * t.amp * 0.55;

        // Receding horizontal lines, FAR TO NEAR. Each one fills down to the bottom with
        // opaque ground before stroking, so it occludes everything behind it.
        let lines = t.lines.max(1);
        // Iterates PAST `lines` and lets `f` exceed 1.0, so the nearest ridges run off the
        // bottom edge and get clipped. Stopping at f = 1.0 left a bare band of ground along
        // the bottom of the panel - the grid visibly ended before the frame did. The extra
        // ridges are also where hidden-line removal earns its keep, since a ridge that large
        // genuinely does occlude the ones behind it.
        for k in 0..(lines + 3) {
            let f = ((k as f32 + self.scroll) / lines as f32).max(0.001);
            let depth_y = horizon as f32 + f.powf(persp) * ground_h as f32;
            let half_w = (w as f32 / 2.0) * t.spread * f;
            if half_w < 1.0 {
                continue;
            }
            let x0 = vpx as f32 - half_w;
            let x1 = vpx as f32 + half_w;

            // Sample the spectrum across THIS line's own width, so the same frequency sits
            // at the same fraction of every line and the ridges line up into hills.
            // 1px, not 2. At 2px each sample landed on its own row and the ridge rendered
            // as a run of short dashes; sampling every column lets `line` join them.
            let step = 1;
            let mut pts: Vec<(i32, i32)> = Vec::new();
            let mut vals: Vec<f32> = Vec::new();
            let mut x = x0.round() as i32;
            let xe = x1.round() as i32;
            while x <= xe {
                let x01 = ((x as f32 - x0) / (x1 - x0).max(1.0)).clamp(0.0, 1.0);
                // Linear interpolation between neighbouring bands, not the nearest band.
                // Nearest-neighbour made each 2px column jump to a new band level, so the
                // ridge stepped up and down by a pixel or two between samples and rendered
                // as a stipple rather than a line.
                let fb = x01 * (levels.len() - 1) as f32;
                let i0 = (fb.floor() as usize).min(levels.len() - 1);
                let i1 = (i0 + 1).min(levels.len() - 1);
                let frac = fb - i0 as f32;
                let v = (levels[i0] * (1.0 - frac) + levels[i1] * frac).clamp(0.0, 1.0);
                let y = depth_y - v * amp_max * f;
                pts.push((x, y.round() as i32));
                vals.push(v);
                x += step;
            }
            if pts.len() < 2 {
                continue;
            }

            if t.occlusion {
                let mut poly = pts.clone();
                poly.push((xe, h));
                poly.push((pts[0].0, h));
                c.fill_poly(&poly, ground);
            }

            for i in 1..pts.len() {
                c.line(pts[i - 1].0, pts[i - 1].1, pts[i].0, pts[i].1, grid);
                // Peak glow: loud sections re-stroke brighter, nearer lines more so.
                let mean = (vals[i - 1] + vals[i]) * 0.5;
                if mean > GLOW_AT {
                    let a = ((mean - GLOW_AT) / (1.0 - GLOW_AT)) * t.glow * f;
                    if a > 0.02 {
                        let g = Rgba::from_hex(&theme.hot, a.clamp(0.0, 1.0));
                        c.line(pts[i - 1].0, pts[i - 1].1, pts[i].0, pts[i].1, g);
                    }
                }
            }
        }

        // Converging verticals, drawn after the ridges so they read through the terrain the
        // way a synthwave grid does. They must reach the canvas corners: an earlier version
        // fanned to half_w(1.0) alone and stopped short of the edges at some spread values.
        let fan_span = (w as f32 / 2.0).max((w as f32 / 2.0) * t.spread);
        let verts = t.verts.max(1);
        for k in 0..=verts {
            let u = (k as f32 / verts as f32) * 2.0 - 1.0;
            let ex = vpx + (u * fan_span).round() as i32;
            c.line(vpx, horizon, ex, h, grid);
        }

        let _ = hot;

        // Bloom the whole scene, then clip back - the sun halo and the bolt both reach past
        // the panel's rounded corners otherwise, the same containment bug already fixed in
        // segmented/scope/vu.
        if theme.bloom > 0.0 {
            let mut glow = c.clone();
            glow.bloom(theme.bloom as i32, 0.5);
            c.draw_over(&glow);
        }
        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 4);

        let e = Rgba::from_hex(&theme.edge, theme.edge_alpha);
        c.fill_rect(1, 2, w - 2, 1, e);
        c.fill_rect(1, h - 3, w - 2, 1, e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::themes::builtin;

    fn spectrum(level: f32) -> FrameData {
        let mut d = FrameData::default();
        for (i, v) in d.levels.iter_mut().enumerate() {
            *v = level * (0.4 + 0.6 * ((i as f32 / 8.0).sin() * 0.5 + 0.5));
        }
        d
    }

    fn bassy(level: f32) -> FrameData {
        let mut d = FrameData::default();
        for (i, v) in d.levels.iter_mut().enumerate() {
            *v = if i < 4 { level } else { level * 0.2 };
        }
        d
    }

    #[test]
    fn draws_something_over_the_whole_panel() {
        let mut v = Vapor::default();
        let mut c = Canvas::new(190, 60);
        v.draw(&mut c, &builtin::vapor_sunset(), &spectrum(0.6));
        let lit = c.bits().iter().filter(|p| **p != 0).count();
        assert!(lit > 190 * 40, "the scene should cover the panel, got {lit} px");
    }

    #[test]
    fn the_grid_scrolls_between_frames() {
        let mut v = Vapor::default();
        let mut c = Canvas::new(190, 60);
        let t = builtin::vapor_sunset();
        v.draw(&mut c, &t, &spectrum(0.5));
        let first = c.bits().to_vec();
        for _ in 0..8 {
            v.draw(&mut c, &t, &spectrum(0.5));
        }
        assert_ne!(first, c.bits().to_vec(), "the grid must scroll on a static spectrum");
    }

    #[test]
    fn scroll_is_frame_rate_independent() {
        // The loop sleeps a fixed 16ms but its real period is that plus the frame's work,
        // so a per-frame scroll would drift with load. Two frames at half dt must advance
        // the same distance as one at full dt.
        let t = builtin::vapor_sunset();
        let mut a = Vapor::default();
        let mut ca = Canvas::new(190, 60);
        let mut d = spectrum(0.4);
        d.dt_ms = NOMINAL_DT_MS;
        a.draw(&mut ca, &t, &d);

        let mut b = Vapor::default();
        let mut cb = Canvas::new(190, 60);
        let mut half = spectrum(0.4);
        half.dt_ms = NOMINAL_DT_MS / 2.0;
        b.draw(&mut cb, &t, &half);
        b.draw(&mut cb, &t, &half);

        assert!(
            (a.scroll - b.scroll).abs() < 1e-4,
            "one full-dt frame ({}) must scroll as far as two half-dt frames ({})",
            a.scroll,
            b.scroll
        );
    }

    #[test]
    fn a_bass_transient_fires_a_bolt_and_it_decays() {
        let mut v = Vapor::default();
        let mut c = Canvas::new(190, 60);
        let t = builtin::vapor_sunset();
        // Quiet, then a jump in bass: a rise, not a level, is what fires.
        v.draw(&mut c, &t, &bassy(0.05));
        assert_eq!(v.bolt, 0.0, "quiet bass must not fire");
        v.draw(&mut c, &t, &bassy(0.9));
        let fired = v.bolt;
        assert!(fired > 0.0, "a bass transient must fire a bolt");
        // Sustained loud bass is not a transient, so it must decay rather than re-fire.
        for _ in 0..3 {
            v.draw(&mut c, &t, &bassy(0.9));
        }
        assert!(v.bolt < fired, "a sustained level must decay, not re-trigger: {} vs {fired}", v.bolt);
    }

    #[test]
    fn sustained_loud_bass_alone_never_fires() {
        // Guards the transient detector specifically: without the rise condition this would
        // fire on every frame of a loud passage and the sky would strobe continuously.
        let mut v = Vapor::default();
        let mut c = Canvas::new(190, 60);
        let t = builtin::vapor_sunset();
        v.draw(&mut c, &t, &bassy(0.9)); // first frame is a rise from 0, so it does fire
        v.bolt = 0.0;
        for _ in 0..30 {
            v.draw(&mut c, &t, &bassy(0.9));
            assert_eq!(v.bolt, 0.0, "a flat loud level must not keep firing");
        }
    }

    #[test]
    fn occlusion_removes_hidden_ridges_when_perspective_bunches_them() {
        // Hidden-line removal only does anything when the perspective term packs the
        // baselines closer together than the audio lift can separate them - only then does a
        // nearer ridge rise ABOVE a farther one and cover it. Measured, at 190x60 with a
        // jagged spectrum: persp 1.4 changes 0 pixels, 1.8 changes 48, the tuner's 2.07
        // changes 83.
        //
        // So this asserts it at a persp where it demonstrably matters. The shipped default is
        // 1.40, chosen because 2.07 collapsed seven of sixteen lines onto two pixel rows at
        // this height - which means the shipped scene does not need occlusion at all, and an
        // assertion made against the default would have been vacuous. It stays in the code
        // because a theme file can raise persp, and because the planned 456px-wide variant
        // has the room for a steeper curve.
        let jagged = {
            let mut d = FrameData::default();
            for (i, v) in d.levels.iter_mut().enumerate() {
                *v = if i % 5 < 2 { 0.6 } else { 0.09 };
            }
            d
        };
        let render = |occlusion: bool| {
            let mut t = builtin::vapor_sunset();
            t.vapor.persp = 2.07;
            t.vapor.amp = 1.01;
            t.vapor.lines = 16;
            t.vapor.occlusion = occlusion;
            let mut v = Vapor::default();
            let mut c = Canvas::new(190, 60);
            for _ in 0..6 {
                v.draw(&mut c, &t, &jagged);
            }
            c.bits().to_vec()
        };
        let on = render(true);
        let off = render(false);
        let differing = on.iter().zip(off.iter()).filter(|(a, b)| a != b).count();
        assert!(
            differing > 20,
            "occlusion must visibly remove hidden ridges at a bunching perspective,              but only {differing} pixels changed"
        );
    }

    #[test]
    fn the_shipped_perspective_keeps_one_pixel_row_per_grid_line() {
        // The defect this guards is the one that made the tuner's values unusable here: at
        // persp 2.07 seven of sixteen lines rounded onto rows 28 and 29, so the far half of
        // the grid rendered as a solid band. The check is on the geometry rather than on
        // pixels, because that is where the collapse happens.
        let t = builtin::vapor_sunset();
        let (h, gh) = (60.0f32, 60.0 - (60.0 * t.vapor.horizon).round() - 2.0);
        let horizon = (h * t.vapor.horizon).round();
        for phase in [0.0f32, 0.2, 0.4, 0.6, 0.8] {
            let rows: std::collections::HashSet<i32> = (0..t.vapor.lines)
                .map(|k| {
                    let f = ((k as f32 + phase) / t.vapor.lines as f32).max(0.001);
                    (horizon + f.powf(t.vapor.persp) * gh).round() as i32
                })
                .collect();
            assert_eq!(
                rows.len(),
                t.vapor.lines as usize,
                "at scroll phase {phase} only {} of {} lines get their own pixel row",
                rows.len(),
                t.vapor.lines
            );
        }
    }

    #[test]
    fn survives_nan_and_a_zero_size_canvas() {
        let mut v = Vapor::default();
        let t = builtin::vapor_sunset();
        let mut d = spectrum(0.5);
        d.levels[0] = f32::NAN;
        d.levels[7] = f32::INFINITY;
        d.dt_ms = f32::NAN;
        let mut c = Canvas::new(190, 60);
        v.draw(&mut c, &t, &d);
        assert!(v.scroll.is_finite(), "NaN input must not poison the scroll phase");

        let mut tiny = Canvas::new(8, 8);
        v.draw(&mut tiny, &t, &spectrum(0.5));
    }

    #[test]
    fn every_vapor_colourway_renders_and_differs() {
        let mut seen: Vec<Vec<u32>> = Vec::new();
        for t in builtin::all().into_iter().filter(|t| t.family == "vapor") {
            let mut v = Vapor::default();
            let mut c = Canvas::new(190, 60);
            v.draw(&mut c, &t, &spectrum(0.6));
            let bits = c.bits().to_vec();
            assert!(bits.iter().any(|p| *p != 0), "{} rendered nothing", t.id);
            for prior in &seen {
                assert_ne!(prior, &bits, "{} renders identically to another colourway", t.id);
            }
            seen.push(bits);
        }
        assert!(seen.len() >= 4, "expected several vaporwave colourways, got {}", seen.len());
    }

    /// Dumps every vaporwave colourway to raw RGBA for visual inspection.
    /// Run: cargo test --release dump_vapor_frames -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_vapor_frames() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();
        let mut n = 0usize;
        for t in builtin::all().into_iter().filter(|t| t.family == "vapor") {
            let mut v = Vapor::default();
            let mut c = Canvas::new(190, 60);
            for f in 0..40 {
                let mut d = spectrum(0.55);
                // a rising kick partway through, so one frame carries a bolt
                if f == 30 {
                    d = bassy(0.9);
                }
                v.draw(&mut c, &t, &d);
            }
            let mut out = Vec::with_capacity(190 * 60 * 4);
            for y in 0..60 {
                for x in 0..190 {
                    let px = c.get(x, y);
                    let a = px.a as f32 / 255.0;
                    for ch in [px.r, px.g, px.b] {
                        out.push((ch as f32 + 22.0 * (1.0 - a)).min(255.0) as u8);
                    }
                    out.push(255);
                }
            }
            std::fs::write(dir.join(format!("vapor-{}.rgba", t.id)), &out).unwrap();
            n += 1;
        }
        println!("wrote {} vapor dumps to {}", n, dir.display());
    }
}
