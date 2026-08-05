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

/// Onset detection for the lightning, calibrated against a recorded fixture of real music
/// (`tests/fixtures/real-music-bands.csv`, 8 seconds, 792 frames).
///
/// The previous trigger was a RISE IN THE BASS MEAN, and measured against that fixture it fires
/// ZERO times: the largest single-frame rise in the bass mean is 0.140 while the threshold demanded
/// 0.157. It could not fire on real music at all - which means the reports of lightning being "not
/// in time with anything" and then "going with every snare" were both about something else. The
/// second one was the white peak glow, which the loudest band trips every frame; that threshold has
/// been raised separately.
///
/// A bass-mean rise is the wrong measure regardless. Kick, snare and clap are broadband transients,
/// and averaging four bands then differencing throws away almost all of the evidence - which is why
/// the numbers came out an order of magnitude below the threshold. SPECTRAL FLUX - the sum of
/// positive change across every band - is the standard onset measure and it works: swept over the
/// fixture, a threshold of `avg * FLUX_RATIO` yields 4.12 onsets/s at ratio 1.6, 3.25 at 2.0, 2.25
/// at 2.5, 1.38 at 3.0. "Every snare" was too many, so 2.8 is used, landing near 1.6/s.
///
/// The threshold is RELATIVE to a slow-following average of the flux, not absolute, so it adapts to
/// how busy and how loud a track is instead of needing per-track tuning - the failure that made the
/// old absolute threshold unreachable on this material.
const FLUX_RATIO: f32 = 2.8;

/// How fast the flux average follows, per frame. Slow enough to represent "this track", fast enough
/// to follow an arrangement change within a couple of seconds.
const FLUX_AVG_RATE: f32 = 0.02;

/// Minimum frames between strikes. At ~99 capture frames/s this is ~120ms, shorter than any musical
/// gap worth marking but long enough to stop one hit registering as two as the flux peak decays.
const FLUX_REFRACTORY: u32 = 12;



/// RAW band level a segment must exceed before it is re-stroked as a peak glow.
///
/// Keyed on the raw level, NOT on the terrain response, because the response CLAMPS at 1.0 - so any
/// threshold below 1.0 fires on every band that saturates, and a threshold at 1.0 fires on all of
/// them at once. Measured on the real-music fixture after the window remap landed: a response
/// threshold of 0.97 caught 8.0% of all band-frames, which is about five of the sixty-four bands
/// glowing white EVERY FRAME. That is the constant flashing that was reported as the lightning
/// firing on every snare - it was not the lightning at all.
///
/// The raw level cannot be defeated that way. Measured on the same fixture: >= 0.60 catches 5.7% of
/// band-frames, >= 0.70 catches 2.3%, >= 0.78 catches about 0.7%, >= 0.85 catches none at all. 0.78
/// puts it back to a genuine occasional highlight - slightly rarer than the 1.4% the old inert
/// mapping produced, which was already described as flashing too much.
const GLOW_AT_RAW: f32 = 0.78;

/// Reference frame duration. The render loop sleeps a fixed 16ms per tick, so its real
/// period is 16ms plus however long the frame took; scroll and bolt decay are scaled by the
/// measured `dt_ms` against this so they run at the same speed when the loop slows.
const NOMINAL_DT_MS: f32 = 16.7;

/// Terrain response window, MEASURED off real audio rather than assumed.
///
/// This is the fourth attempt at making the ground react, and the first based on what the DSP
/// actually emits. `--levels` captured 8 seconds of music: per-band levels run p10 0.119, p50 0.284,
/// p90 0.575, p99 0.834 - but the FRAME's loudest band sits at p50 0.819.
///
/// That last number is what defeated the previous three attempts. The terrain auto-ranged against
/// the frame's loudest band, and since that is already 0.82 the gain settled at 0.92/0.82 = 1.12 -
/// the normaliser was inert. A median band at 0.284 therefore reached 0.32 of the displacement
/// range, which is 5px of lift on a 29px ground, while the single loudest band each frame landed at
/// 0.92 and tripped the peak-glow threshold. "Generally the waves seem very low" and "it
/// occasionally flashes white" are those two facts.
///
/// Normalising against a maximum can never fix this: a real spectrum's median is about a third of
/// its max, so the median ridge stays a third height at any gain. What is needed is to spend the
/// output range on the part of the DISTRIBUTION the eye actually sees - hence a fixed window from
/// p10 to p90, with a gamma that lifts the middle. Measured result: the median ridge goes from 5.0px
/// to 8.6px and the p90 band reaches full height.
const TERRAIN_FLOOR_LEVEL: f32 = 0.119;
const TERRAIN_SPAN_LEVEL: f32 = 0.456;
/// Below 1 to expand the low half of the distribution, where most band-frames actually live.
const TERRAIN_GAMMA: f32 = 0.6;

#[derive(Default)]
pub struct Vapor {
    /// Grid scroll phase, wrapping 0->1.
    scroll: f32,
    /// Previous frame's band levels, for the spectral-flux onset detector.
    prev_levels: Vec<f32>,
    /// Slow-following average of the flux, which the threshold is relative to.
    flux_avg: f32,
    /// Frames since the last strike, for the refractory period.
    since_bolt: u32,
    /// Decaying brightness of the current lightning strike, 0 when none.
    bolt: f32,
    /// Advanced on each strike so successive bolts take different paths.
    bolt_seed: u32,
    /// One spectrum snapshot per grid line, newest first - index 0 is the line at the horizon.
    ///
    /// THE reason the ground read as static, and it is structural rather than a matter of
    /// amplitude. Every line used to sample the CURRENT frame's spectrum, so all twelve rendered
    /// the same curve at different scales: the entire surface flexed in unison and nothing ever
    /// travelled. Raising the displacement made that flex bigger without making it move, which is
    /// why it still looked like the ground was not doing much.
    ///
    /// A real terrain has history. Each line now keeps the spectrum from when it was born at the
    /// horizon, so a bass hit raises one ridge that then scrolls toward the viewer.
    rows: Vec<Vec<f32>>,
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

    /// Maps a band level onto terrain displacement through the measured window.
    ///
    /// A fixed window, not a follower. The follower it replaced was measurably inert - see the
    /// constants above - and the valve-row diagnosis reached the same conclusion for the same
    /// reason: an auto-ranger is a compressor in time, and a compressor in front of a transient is
    /// the wrong tool. The terrain is also the one place a compressor actively hurts, because it
    /// normalises away exactly the band-to-band contrast the hills are made of.
    fn terrain_resp(level: f32, sensitivity: f32) -> f32 {
        if !level.is_finite() || level <= TERRAIN_FLOOR_LEVEL {
            return 0.0;
        }
        let x = ((level - TERRAIN_FLOOR_LEVEL) / TERRAIN_SPAN_LEVEL).clamp(0.0, 1.0);
        (x.powf(TERRAIN_GAMMA) * sensitivity.max(0.0)).clamp(0.0, 1.0)
    }

    /// Advances the onset detector and returns the current strike brightness.
    ///
    /// Reads `d.levels` - the DSP-smoothed bands - rather than the family's own further-smoothed
    /// copy. Onset detection wants the least smoothed signal available, and the fixture calibration
    /// above was done on exactly this one.
    fn update_bolt(&mut self, d: &FrameData, t: &crate::themes::VaporParams, dt: f32) -> f32 {
        let n = d.levels.len();
        if self.prev_levels.len() != n {
            self.prev_levels = d.levels.to_vec();
        }
        // Spectral flux: the sum of POSITIVE change across every band. Positive only - a note
        // ending is not an onset, and counting decays doubles the event rate.
        let mut flux = 0.0f32;
        for (i, &v) in d.levels.iter().enumerate() {
            if v.is_finite() {
                flux += (v - self.prev_levels[i]).max(0.0);
                self.prev_levels[i] = v;
            }
        }
        if !flux.is_finite() {
            flux = 0.0;
        }
        self.flux_avg += (flux - self.flux_avg) * (FLUX_AVG_RATE * dt).clamp(0.0, 1.0);
        if !self.flux_avg.is_finite() {
            self.flux_avg = 0.0;
        }

        self.since_bolt = self.since_bolt.saturating_add(1);
        // `bolt_sens` still scales it, so a colourway can be calmer or busier and it stays
        // TOML-tunable. 0.55 is the shipped value and maps to the calibrated ratio exactly.
        let ratio = FLUX_RATIO * (1.0 + (0.55 - t.bolt_sens.clamp(0.0, 1.0)));
        if flux > self.flux_avg * ratio && self.since_bolt > FLUX_REFRACTORY {
            self.bolt = t.bolt_bright.clamp(0.0, 1.0);
            self.bolt_seed = self.bolt_seed.wrapping_add(1);
            self.since_bolt = 0;
        }
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
        // NOTE: `band_gain` below scales the band levels the terrain and the peak glow both
        // read, so the GLOW_AT threshold is applied to the normalised value - a glow threshold
        // left on the raw level would almost never trigger once the terrain was normalised.
        let dt = if d.dt_ms.is_finite() {
            (d.dt_ms / NOMINAL_DT_MS).clamp(0.25, 4.0)
        } else {
            1.0
        };

        let levels = Self::smoothed(d, t.smoothing);
        // The onset detector reads FrameData directly, not the family's smoothed copy - see
        // `update_bolt`.
        let bolt = self.update_bolt(d, t, dt);

        let before_scroll = self.scroll;
        let step = t.scroll * 0.010 * dt;
        // `rem_euclid` rather than `fract`: fract of a negative value is negative, which would send
        // every line's depth negative and collapse the grid onto the horizon.
        self.scroll = if t.recede {
            (self.scroll - step).rem_euclid(1.0)
        } else {
            (self.scroll + step).rem_euclid(1.0)
        };

        // Advance the terrain history. `f = (k + scroll) / lines`, so when the phase wraps every
        // line inherits the position of the one behind it - which means the shapes must rotate by
        // one at the same moment, or they would visibly jump.
        // Exactly one row per VISIBLE line, with no row for the overhang.
        //
        // The line loop runs to lines+1 so the last ridge covers the bottom edge, but `row` below
        // clamps its index - so the overhang line reuses the nearest visible line's row. That is
        // what puts the newest, live, peak-held spectrum on a line that is actually on screen.
        // Sized to lines+2 it sat on the overhang at depth 74.9px over a 58px ground and was clipped
        // away every frame, which is why the terrain looked inert no matter what the mapping did.
        let want_rows = t.lines.max(1) as usize;
        let stale_rows = self.rows.len() != want_rows
            || self.rows.first().map(|r| r.len() != levels.len()).unwrap_or(true);
        if stale_rows {
            // A theme reload can change `lines`; start every row from the live spectrum rather
            // than from zeros, or the grid flattens for a full scroll cycle after a reload.
            self.rows = vec![levels.clone(); want_rows];
        }
        // Which END new spectra enter at is what makes the terrain feel responsive - see
        // `VaporParams::recede`. Receding: lines travel toward the horizon, so they are born at the
        // near edge and the newest audio lands on the biggest, closest ridges at once. Advancing:
        // lines travel toward the viewer, so they are born at the horizon.
        //
        // The rotation has to happen on the same frame as the wrap. `f = (k + scroll) / lines`, so
        // when the phase wraps every line inherits a neighbour's position; if the snapshots did not
        // shift with them the shapes would visibly jump once per cycle.
        let wrapped = if t.recede {
            self.scroll > before_scroll
        } else {
            self.scroll < before_scroll
        };
        if wrapped {
            if t.recede {
                self.rows.remove(0);
                self.rows.push(levels.clone());
            } else {
                self.rows.pop();
                self.rows.insert(0, levels.clone());
            }
        } else if let Some(newest) = if t.recede {
            self.rows.last_mut()
        } else {
            self.rows.first_mut()
        } {
            // PEAK-HOLD, not the instantaneous level, for the whole time this line is the newest.
            //
            // A line is only born once per scroll-cycle-over-lines: measured at the shipped scroll
            // and 12 lines that is every 6.7 frames, so the terrain samples the spectrum at just
            // 8.9Hz. Sampling instantaneously at that rate means a kick lasting ~50ms can fall
            // entirely between two births and never be recorded anywhere - the terrain would
            // silently skip the transient the user is watching for. Holding the maximum seen during
            // the line's life captures every hit at full height, on the line that was newest when
            // it happened.
            for (dst, src) in newest.iter_mut().zip(levels.iter()) {
                *dst = dst.max(*src);
            }
        }

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
        // ONE extra line past `lines`, not three.
        //
        // The extras exist so the nearest ridges run off the bottom edge rather than the grid
        // visibly ending before the frame does. Three was right when persp was 2.07, where f = 1.0
        // fell well short of the bottom; at persp 1.40 f = 1.0 already lands on the last row, so
        // two of the three were pure overhang.
        //
        // And in RECEDING mode that overhang was hiding the whole point of the family. New rows are
        // born at the near edge, which is the HIGHEST k, so the newest row - the one carrying the
        // live peak-held spectrum - sat at k = 12, depth 74.9px on a 58px ground. The live data was
        // drawn off the canvas every single frame and never seen; only rows several births old were
        // ever visible. That is the measured cause of the terrain reading as unresponsive through
        // four separate attempts to fix it, and no amount of gain or amplitude could have helped.
        for k in 0..(lines + 1) {
            let f = ((k as f32 + self.scroll) / lines as f32).max(0.001);
            let depth_y = horizon as f32 + f.powf(persp) * ground_h as f32;
            let half_w = (w as f32 / 2.0) * t.spread * f;
            if half_w < 1.0 {
                continue;
            }
            let x0 = vpx as f32 - half_w;
            let x1 = vpx as f32 + half_w;
            // This line's own snapshot, not the live spectrum - see `rows`.
            let row = &self.rows[(k as usize).min(self.rows.len() - 1)];

            // Sample the spectrum across THIS line's own width, so the same frequency sits
            // at the same fraction of every line and the ridges line up into hills.
            // 1px, not 2. At 2px each sample landed on its own row and the ridge rendered
            // as a run of short dashes; sampling every column lets `line` join them.
            let step = 1;
            let mut pts: Vec<(i32, i32)> = Vec::new();
            let mut vals: Vec<f32> = Vec::new();
            // Raw levels kept alongside the responses, for the peak glow - see GLOW_AT_RAW.
            let mut raws: Vec<f32> = Vec::new();
            let mut x = x0.round() as i32;
            let xe = x1.round() as i32;
            while x <= xe {
                let x01 = ((x as f32 - x0) / (x1 - x0).max(1.0)).clamp(0.0, 1.0);
                // Linear interpolation between neighbouring bands, not the nearest band.
                // Nearest-neighbour made each 2px column jump to a new band level, so the
                // ridge stepped up and down by a pixel or two between samples and rendered
                // as a stipple rather than a line.
                let fb = x01 * (row.len() - 1) as f32;
                let i0 = (fb.floor() as usize).min(row.len() - 1);
                let i1 = (i0 + 1).min(row.len() - 1);
                let frac = fb - i0 as f32;
                let raw = row[i0] * (1.0 - frac) + row[i1] * frac;
                let v = Self::terrain_resp(raw, theme.sensitivity);
                raws.push(raw);
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
                let mean = (raws[i - 1] + raws[i]) * 0.5;
                if mean > GLOW_AT_RAW {
                    let a = ((mean - GLOW_AT_RAW) / (1.0 - GLOW_AT_RAW)) * t.glow * f;
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

    /// 8 seconds of real music, captured with `--levels` and committed as a fixture.
    ///
    /// The reason this exists: every audio mapping in this project was calibrated against a
    /// synthetic spectrum, and the terrain needed fixing FOUR times before anyone measured what the
    /// DSP actually emits. The recorded frames make "does this respond to music" an assertion
    /// instead of an eyeball judgement.
    fn real_music() -> Vec<Vec<f32>> {
        include_str!("../../tests/fixtures/real-music-bands.csv")
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.split(',').filter_map(|v| v.parse::<f32>().ok()).collect())
            .collect()
    }

    #[test]
    fn the_lightning_fires_at_a_musical_rate_on_real_music() {
        // The trigger this replaced was a rise in the bass mean, and on this same fixture it fires
        // ZERO times in 8 seconds: the largest single-frame bass rise is 0.140 against a threshold
        // of 0.157. It could not fire on real music at all, which is why the strikes read as "not in
        // time with anything".
        //
        // Swept over the fixture, the flux detector yields 4.12 onsets/s at ratio 1.6, 3.25 at 2.0,
        // 2.25 at 2.5 and 1.38 at 3.0. "Every snare" was too many, so the shipped ratio aims near
        // 1.6/s. The band asserted here is deliberately wide - the point is that it fires at a rate
        // a listener would call musical, not that it hits one exact number.
        let frames = real_music();
        assert!(frames.len() > 500, "fixture looks truncated: {} frames", frames.len());

        let t = builtin::vapor_sunset();
        let mut v = Vapor::default();
        let mut c = Canvas::new(190, 60);
        let mut strikes = 0u32;
        let mut prev_seed = v.bolt_seed;
        for row in &frames {
            let mut d = FrameData::default();
            for (i, x) in d.levels.iter_mut().enumerate() {
                *x = row.get(i).copied().unwrap_or(0.0);
            }
            d.peaks = d.levels;
            v.draw(&mut c, &t, &d);
            if v.bolt_seed != prev_seed {
                strikes += 1;
                prev_seed = v.bolt_seed;
            }
        }
        // The capture ran at about 99 frames per second.
        let per_sec = strikes as f32 / (frames.len() as f32 / 99.0);
        assert!(
            (0.6..=3.5).contains(&per_sec),
            "lightning fired {strikes} times over {:.1}s = {per_sec:.2}/s, which is outside the              range a listener would call musical",
            frames.len() as f32 / 99.0
        );
    }

    #[test]
    fn the_peak_glow_stays_rare_on_real_music() {
        // The regression this guards was reported as "the lightning is going with every snare", and
        // it was not the lightning: the window remap clamps the terrain response at 1.0, so a glow
        // threshold of 0.97 caught 8.0% of all band-frames on this fixture - about five of the
        // sixty-four bands glowing white every single frame.
        //
        // Asserted on the RAW level because that is what the glow now keys on, and because a
        // response-based assertion is exactly the one that could not see the problem.
        let frames = real_music();
        let total: usize = frames.iter().map(|r| r.len()).sum();
        let hot = frames
            .iter()
            .flat_map(|r| r.iter())
            .filter(|&&v| v > GLOW_AT_RAW)
            .count();
        let pct = hot as f32 / total as f32 * 100.0;
        assert!(
            pct < 2.0,
            "the peak glow fires on {pct:.2}% of band-frames, which reads as constant flashing              rather than as a highlight"
        );
        assert!(
            pct > 0.05,
            "the peak glow fires on {pct:.2}% of band-frames, so it never appears at all and the              feature is dead weight"
        );
    }

    #[test]
    fn the_terrain_uses_most_of_its_range_on_real_music() {
        // The complaint this guards, four times over: "the waves seem very low". Measured cause was
        // that the terrain auto-ranged against the frame's LOUDEST band, which on real music already
        // sits at 0.82 - so the gain settled at 1.12, the normaliser was inert, and a median band at
        // 0.27 reached only about a third of the displacement range.
        //
        // Asserts on the response directly rather than on pixels, because it is the mapping that was
        // wrong and pixels would let a geometry change mask a regression here.
        let frames = real_music();
        let t = builtin::vapor_sunset();
        let mut resp: Vec<f32> = frames
            .iter()
            .flat_map(|r| r.iter().map(|&v| Vapor::terrain_resp(v, t.sensitivity)))
            .collect();
        resp.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let q = |p: f32| resp[((resp.len() - 1) as f32 * p) as usize];
        assert!(
            q(0.5) > 0.40,
            "the median band must drive most of a ridge, got {:.2} - it was 0.32 when the terrain              read as flat",
            q(0.5)
        );
        assert!(q(0.9) > 0.90, "the loud tenth must reach near full height, got {:.2}", q(0.9));
        assert!(q(0.05) < 0.25, "quiet bands must still be quiet, got {:.2}", q(0.05));
    }

    #[test]
    fn a_transient_lands_on_the_nearest_lines_not_at_the_horizon() {
        // The complaint this guards: "I need to see the closer lines spiking up as the audio hits.
        // it's too slow and calm."
        //
        // Displacement scales with depth, so the nearest lines move most - and a line only ever
        // shows the spectrum from when it was born. With the grid flowing toward the viewer, new
        // audio was born at the HORIZON, rendered at the smallest displacement, and needed a full
        // scroll cycle to reach the front. Both penalties at once.
        //
        // Measures where a transient's effect actually lands, as the vertical centroid of the pixels
        // that changed. It must sit in the near half of the ground.
        let t = builtin::vapor_sunset();
        assert!(t.vapor.recede, "test premise: the shipped scene puts new audio at the near edge");

        let quiet = spectrum(0.06);
        let mut v = Vapor::default();
        let mut c = Canvas::new(190, 60);
        for _ in 0..30 {
            v.draw(&mut c, &t, &quiet);
        }
        let before = c.bits().to_vec();
        v.draw(&mut c, &t, &spectrum(0.95));
        let after = c.bits().to_vec();

        let horizon = (60.0 * t.vapor.horizon).round() as i32;
        let bottom = 58;
        let midway = horizon + (bottom - horizon) / 2;
        let (mut wsum, mut w) = (0.0f64, 0.0f64);
        for y in horizon..bottom {
            for x in 0..190 {
                let i = (y * 190 + x) as usize;
                if before[i] != after[i] {
                    wsum += y as f64;
                    w += 1.0;
                }
            }
        }
        assert!(w > 40.0, "the transient must change the terrain at all; {w} pixels changed");
        let centroid = wsum / w;
        assert!(
            centroid > midway as f64,
            "a transient must land on the NEAR lines: centroid row {centroid:.1}, near half starts              at {midway} (ground {horizon}..{bottom})"
        );
    }

    #[test]
    fn the_terrain_remembers_a_transient_after_the_sound_has_gone() {
        // The defect this guards is why the ground read as static: every line sampled the CURRENT
        // spectrum, so all of them drew the same curve and the whole surface flexed in unison -
        // nothing ever travelled toward the viewer, at any amplitude.
        //
        // A terrain with memory must still show the ridge some frames AFTER the sound stops.
        let t = builtin::vapor_sunset();
        let quiet = spectrum(0.05);

        let mut hit = Vapor::default();
        let mut c_hit = Canvas::new(190, 60);
        for _ in 0..6 {
            hit.draw(&mut c_hit, &t, &quiet);
        }
        hit.draw(&mut c_hit, &t, &spectrum(0.95));
        for _ in 0..4 {
            hit.draw(&mut c_hit, &t, &quiet);
        }

        let mut calm = Vapor::default();
        let mut c_calm = Canvas::new(190, 60);
        for _ in 0..11 {
            calm.draw(&mut c_calm, &t, &quiet);
        }

        let differing = c_hit
            .bits()
            .iter()
            .zip(c_calm.bits().iter())
            .filter(|(a, b)| a != b)
            .count();
        assert!(
            differing > 150,
            "four frames after the transient the ridge must still be on screen, but only \
             {differing} pixels differ from a run that never saw it"
        );
    }

    #[test]
    fn each_grid_line_holds_its_own_snapshot_not_the_live_spectrum() {
        // Checks the mechanism directly: after a loud frame followed by quiet ones the rows must
        // hold DIFFERENT spectra. Identical rows mean the ring is not rotating and the surface is
        // back to flexing in unison.
        let t = builtin::vapor_sunset();
        let mut v = Vapor::default();
        let mut c = Canvas::new(190, 60);
        v.draw(&mut c, &t, &spectrum(0.9));
        for _ in 0..140 {
            v.draw(&mut c, &t, &spectrum(0.08));
        }
        let front: f32 = v.rows[0].iter().sum();
        let distinct = v.rows.iter().any(|r| (r.iter().sum::<f32>() - front).abs() > 0.5);
        assert!(
            distinct,
            "rows must hold different moments, but all {} carry the same spectrum",
            v.rows.len()
        );
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

#[cfg(test)]
mod sweep {
    use super::*;
    use crate::themes::builtin;

    /// Realistic music: band levels in the 0.15-0.65 band the DSP actually produces, with a
    /// spectral shape rather than a flat line.
    fn realistic(loudness: f32) -> FrameData {
        let mut d = FrameData::default();
        for (i, v) in d.levels.iter_mut().enumerate() {
            let x = i as f32 / 63.0;
            // bass-heavy with a couple of mid peaks, rolling off at the top
            let shape = 0.55 * (1.0 - x).powf(1.6) + 0.30 * (-(((x - 0.32) / 0.09).powi(2))).exp()
                + 0.22 * (-(((x - 0.62) / 0.07).powi(2))).exp();
            *v = (shape * loudness).clamp(0.0, 1.0);
        }
        d.peaks = d.levels;
        d
    }

    /// Renders the terrain at a range of `amp` values so the right one can be chosen by eye.
    /// Run: cargo test --release sweep_terrain_amp -- --ignored --nocapture
    #[test]
    #[ignore]
    fn sweep_terrain_amp() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();
        println!("{:>5} {:>14} {:>16}", "amp", "peak ridge px", "as % of ground");
        for amp in [0.55f32, 0.85, 1.15, 1.5] {
            let mut t = builtin::vapor_sunset();
            t.vapor.amp = amp;
            let mut v = Vapor::default();
            let mut c = Canvas::new(380, 60);
            let d = realistic(0.62);
            for _ in 0..60 {
                v.draw(&mut c, &t, &d);
            }
            let horizon = (60.0 * t.vapor.horizon).round() as i32;
            let ground = 60 - horizon - 2;
            // topmost grid ink above the horizon line tells us how far a ridge actually lifted
            let ground_col = Rgba::from_hex(&t.vapor.ground, 1.0);
            let mut highest = horizon;
            for y in 4..(horizon + ground) {
                for x in 6..374 {
                    let p = c.get(x, y);
                    let off = p.r.abs_diff(ground_col.r) as u32
                        + p.g.abs_diff(ground_col.g) as u32
                        + p.b.abs_diff(ground_col.b) as u32;
                    if y > horizon && off > 90 {
                        highest = highest.min(y);
                    }
                }
            }
            let lift = horizon - highest;
            println!(
                "{amp:>5.2} {:>14} {:>15.0}%",
                (horizon + ground) - highest,
                ((horizon + ground - highest) as f32 / ground as f32) * 100.0
            );
            let _ = lift;
            let mut out = Vec::with_capacity(380 * 60 * 4);
            for y in 0..60 {
                for x in 0..380 {
                    let px = c.get(x, y);
                    let a = px.a as f32 / 255.0;
                    for ch in [px.r, px.g, px.b] {
                        out.push((ch as f32 + 22.0 * (1.0 - a)).min(255.0) as u8);
                    }
                    out.push(255);
                }
            }
            std::fs::write(dir.join(format!("amp-{:.2}.rgba", amp)), &out).unwrap();
        }
    }
}

#[cfg(test)]
mod strip {
    use super::*;
    use crate::themes::builtin;
    /// Run: cargo test --release dump_vapor_strip -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_vapor_strip() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();
        let t = builtin::vapor_sunset();
        let mut v = Vapor::default();
        let mut c = Canvas::new(190, 60);
        let quiet = {
            let mut d = FrameData::default();
            for (i, x) in d.levels.iter_mut().enumerate() {
                *x = 0.10 + 0.06 * ((i as f32 / 9.0).sin().abs());
            }
            d
        };
        let hit = {
            let mut d = FrameData::default();
            for (i, x) in d.levels.iter_mut().enumerate() {
                let f = i as f32 / 63.0;
                *x = (0.20 + 0.75 * (1.0 - f).powf(1.4)).min(1.0);
            }
            d
        };
        for _ in 0..40 {
            v.draw(&mut c, &t, &quiet);
        }
        // frame 0 = just before, then the hit, then its aftermath
        for (n, d) in [(0, &quiet), (1, &hit), (2, &quiet), (3, &quiet), (4, &quiet), (5, &quiet)] {
            v.draw(&mut c, &t, d);
            let mut out = Vec::new();
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
            std::fs::write(dir.join(format!("strip-{n}.rgba")), &out).unwrap();
        }
        println!("wrote strip-0..5 (0 before, 1 = the hit, 2..5 aftermath)");
    }
}

#[cfg(test)]
mod flow {
    use super::*;
    use crate::themes::builtin;

    /// How much of the terrain actually changes between consecutive frames - a direct proxy for
    /// whether the grid reads as moving. Run: cargo test --release probe_flow -- --ignored --nocapture
    #[test]
    #[ignore]
    fn probe_flow() {
        println!("{:>7} {:>22} {:>22}", "scroll", "px changed / frame", "of the ground");
        for scroll in [1.24f32, 2.0, 3.0, 4.0, 5.0] {
            let mut t = builtin::vapor_sunset();
            t.vapor.scroll = scroll;
            let mut v = Vapor::default();
            let mut c = Canvas::new(190, 60);
            let mut d = FrameData::default();
            for (i, x) in d.levels.iter_mut().enumerate() {
                let f = i as f32 / 63.0;
                *x = 0.18 + 0.42 * (f * 7.0).sin().abs();
            }
            for _ in 0..90 {
                v.draw(&mut c, &t, &d);
            }
            let horizon = (60.0 * t.vapor.horizon).round() as i32;
            let before = c.bits().to_vec();
            v.draw(&mut c, &t, &d);
            let after = c.bits().to_vec();
            let mut changed = 0;
            let mut total = 0;
            for y in horizon..58 {
                for x in 2..188 {
                    let i = (y * 190 + x) as usize;
                    total += 1;
                    if before[i] != after[i] {
                        changed += 1;
                    }
                }
            }
            println!(
                "{scroll:>7.2} {changed:>18} px {:>20.1}%",
                changed as f32 / total as f32 * 100.0
            );
        }
    }
}
