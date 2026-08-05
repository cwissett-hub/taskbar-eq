//! The radar PPI family: a sweep that paints the spectrum as blips and leaves a phosphor wake.
//!
//! Every other family in this project redraws the whole picture every frame. This one does
//! not, and that is the entire point of it: a plan-position indicator only knows what the
//! beam has already illuminated, so the display is built up one bearing at a time and what
//! you are looking at is a second of history rather than an instant. Nothing else here shows
//! time at all.
//!
//! Three decisions carry it, and each was made against a measured failure:
//!
//! - **The blip's RANGE is the audio cue, not its brightness.** A louder band pushes its
//!   blip further out along its own bearing. Brightness rides along, but the valve row
//!   already proved that brightness alone gives about a 1.16x spread between a driven
//!   element and its neighbour - below the visible threshold at this size - while a mark
//!   that MOVES is resolved instantly. See `RANGE_MIN`/`RANGE_MAX`.
//! - **A half-fan, elliptically squashed.** Justified at `geometry`.
//! - **The wake is tied to the sweep period.** A cell decays to `WAKE_FLOOR` over exactly
//!   one pass, so the returns immediately behind the beam are the brightest and the ones
//!   just ahead of it - painted a whole revolution ago - are the faintest. Decay that is
//!   faster than that leaves a bare bearing line with no picture; slower and every bearing
//!   sits at the same brightness and the display stops reading as a scan at all.
//!
//! Bloom, as everywhere else here: the beam and the blips are built on their OWN
//! transparent layer, bloomed, and composited over the opaque panel, because
//! `Canvas::bloom` puts its halo UNDERNEATH existing content and the panel would hide it
//! completely. The graticule is deliberately NOT on that layer - see the note where it is
//! drawn.

use super::canvas::{Canvas, Rgba};
use super::{Family, FrameData};
use crate::themes::Theme;
use std::f32::consts::PI;

/// Bearing cells the fan is divided into - the display's angular resolution.
///
/// 32 rather than 64 (one per band) on purpose. At the reference size the outer ring is
/// ~91px of half-width, so 32 cells put adjacent blips about 9px apart at maximum range and
/// they stay separable; at 64 they are 4.5px apart, which at a blip radius of up to ~3.6px
/// merges neighbouring returns into a continuous arc and throws away the one thing this
/// display is for. It also means each cell reduces a GROUP of bands, which is what makes
/// `GROUP_MAX_BIAS` do any work at all.
const CELLS: usize = 32;

/// Milliseconds for the beam to cross the whole fan.
///
/// Real surveillance sets run 2-4s per revolution. 1400ms for a half-fan is deliberately
/// faster than that: at 4s the display looked broken rather than slow, because at a 60px
/// panel there is no detail to reward waiting for, and a full picture has to be up within
/// about a second of the music starting or the family reads as unresponsive.
const SWEEP_MS: f32 = 1400.0;

/// Band level at which a bearing starts to return an echo, and the span it fills over.
///
/// Copied deliberately from `tube.rs`, and for the same measured reason: `FrameData.levels`
/// only reaches about 0.15-0.65 on real music, so anything mapping 0..1 linearly spends two
/// thirds of its travel on levels that never arrive. Mapping the window the DSP actually
/// produces onto the full range is what makes the blips move on quiet passages.
const RESP_FLOOR: f32 = 0.10;
const RESP_SPAN: f32 = 0.52;

/// Weight given to a cell's LOUDEST band rather than its mean.
///
/// Two bands feed one cell at the reference size. A plain mean of the pair halves the
/// travel of a single-band peak, which is exactly the event that should show up as one blip
/// jumping out ahead of its neighbours - the thing that makes the picture look like a
/// spectrum rather than a ring. Same value and same reasoning as the valve row, where the
/// mean measured 1.46 dL* between a driven element and its neighbour against 9.47 dL* for
/// the max-biased blend.
const GROUP_MAX_BIAS: f32 = 0.65;

/// Range a silent bearing's blip sits at, and the range a full-scale one reaches, as a
/// fraction of the fan's radius.
///
/// The audio-driven POSITION cue. Not 0.0 at the bottom: a return painted on top of the
/// centre pip is indistinguishable from the pip, and close-in clutter is what a real PPI
/// shows for a bearing with nothing on it. Not 1.0 at the top either - the outermost ring is
/// the graticule, and a blip landing exactly on it reads as a break in the ring rather than
/// as a maximum.
const RANGE_MIN: f32 = 0.20;
const RANGE_MAX: f32 = 0.96;

/// Brightness a freshly painted blip keeps regardless of level.
///
/// Applied ONCE, on paint, and only to the blip's own alpha - the lesson `HEATER_FLOOR` in
/// tube.rs records is that a floor folded into a term that is then floored again eats most of
/// the usable range before any audio arrives. Here it is affordable precisely BECAUSE
/// brightness is the secondary cue: range gets its full travel, and this only guarantees the
/// blip is visible enough for its position to be read.
const BLIP_FLOOR: f32 = 0.18;

/// What a bearing's echo has decayed to by the time the beam comes round to it again.
///
/// 0.12 rather than 0: a cell that reaches zero leaves a wedge of empty screen ahead of the
/// beam and the display reads as a single rotating line with nothing on it. Keeping the
/// oldest returns just visible is what makes one glance show a whole revolution.
const WAKE_FLOOR: f32 = 0.12;

/// Angular length of the beam's own afterglow, in cells, and how many lines draw it.
///
/// 4.5 cells is ~25 degrees. The wake above is the AUDIO history; this is just the beam
/// looking like a beam.
///
/// 24 lines, and that number is the whole reason this constant is documented. It was 7,
/// which spaces consecutive samples 0.71 cells apart - and at the outer ring, 91px out,
/// 0.71 cells is 6.4px, so the trail rendered as five or six clearly separate spokes fanning
/// out behind the beam rather than as one wedge. It looked like a broken bicycle wheel and it
/// was plainly visible in the very first dump. 24 puts them 1.7px apart at the rim, which the
/// bloom closes into a continuous wedge; the per-line alpha is scaled down to compensate for
/// the extra overlap.
const BEAM_CELLS: f32 = 4.5;
const BEAM_STEPS: usize = 24;

/// Rise in the low bands that fires a close-in transient blip, and how long it lives.
///
/// Measured against a rise DETECTOR rather than an absolute level, because bass sits high
/// on almost all music - an absolute threshold fires continuously and the extra blip becomes
/// a permanent fixture instead of an event. 300ms is about two beats' gap at 120bpm, so
/// successive kicks re-fire it rather than merging.
const BASS_RISE: f32 = 0.055;
const BASS_FALL_MS: f32 = 300.0;
const HIT_RANGE: f32 = 0.14;

/// Panel width one PPI face wants.
///
/// See `geometry` for why extra width buys another face instead of a wider one.
const SCOPE_PITCH: i32 = 190;

/// PPI faces to draw at a given panel width.
fn scope_count(w: i32) -> usize {
    ((w / SCOPE_PITCH).max(1) as usize).min(4)
}

/// Centre and the two radii of face `s` of `scopes`, in pixels.
///
/// **A half-fan from the bottom centre, squashed onto an ellipse.** The panel is 190x60, so a
/// circular PPI cannot fit at all: the largest circle that clears the bezel has a radius of
/// about 26px and would leave two thirds of the width empty, which at this size means
/// throwing away most of the resolution available. A 180-degree fan (like the VU family's
/// dial arc) fixes the height problem but not the width one - a semicircle of radius 26 is
/// still 52px wide on a 186px panel. So the fan is squashed: `rx` takes the full available
/// half-width and `ry` the full available height, and range is measured in normalised
/// ellipse units so "maximum range" is the boundary at every bearing. The cost is that the
/// range rings are flattened arcs rather than circles, which is honest about the shape of
/// the space it has.
///
/// Extra WIDTH buys extra faces rather than a wider one, following the valve row and the VU
/// dials: at 380px a single face reaches 3.9:1 squash, at which the flattening stops reading
/// as a squashed circle and starts reading as a lens. Two faces at 1.9:1 keep the shape that
/// was tuned, and because each covers half the spectrum they also halve the number of bands
/// per cell - the wide mode gets DOUBLE the bearing resolution rather than a bigger picture
/// of the same thing.
fn geometry(w: i32, h: i32, scopes: usize, s: usize) -> (i32, i32, f32, f32) {
    let scope_w = ((w - 4) / scopes as i32).max(1);
    let cx = 2 + s as i32 * scope_w + scope_w / 2;
    let cy = h - 5;
    let rx = (scope_w as f32 * 0.5 - 2.0).max(2.0);
    let ry = ((cy - 4) as f32).max(2.0);
    (cx, cy, rx, ry)
}

/// Bearing of a (possibly fractional) cell position, in radians, 0 = left horizon.
///
/// The fan is swept left to right, and the spectrum is mapped low-to-high the same way, so
/// bass sits at the left horizon exactly as it does in every other family here. A radar
/// convention (north up, clockwise) would have been more authentic and would have put the
/// low bands somewhere arbitrary; consistency with the other five wins.
fn bearing(cell_pos: f32) -> f32 {
    PI * (cell_pos + 0.5) / CELLS as f32
}

/// Pixel for a (bearing, range) pair. Range is in normalised ellipse units, so 1.0 is the
/// outer ring at every bearing.
fn plot(cx: i32, cy: i32, rx: f32, ry: f32, theta: f32, range: f32) -> (i32, i32) {
    (
        cx - (theta.cos() * rx * range).round() as i32,
        cy - (theta.sin() * ry * range).round() as i32,
    )
}

#[derive(Default)]
pub struct Radar {
    /// Beam position, in cell units, 0..CELLS.
    pos: f32,
    /// Monotonic sweep phase, 0..1 per there-and-back cycle.
    ///
    /// The position is DERIVED from this rather than accumulated directly, because the beam has to
    /// reverse smoothly and an accumulator that wraps cannot: see the note in `draw`.
    phase: f32,
    /// Direction of travel, +1 outbound and -1 on the return leg. The wake trails behind the beam,
    /// so it has to know which way behind is.
    dir: f32,
    /// Per (face, cell) echo strength as painted - the value `range` and blip size derive
    /// from. Held separately from `glow` because a fading return must keep its SIZE and only
    /// lose brightness: deriving size from the decaying value instead made an old loud
    /// return shrink until it was indistinguishable from a fresh quiet one, which destroys
    /// the only reason to keep history on screen.
    echo: Vec<f32>,
    /// Per (face, cell) brightness, decaying with the phosphor.
    glow: Vec<f32>,
    /// Slew-limited low-band level, for the transient detector to measure a rise against.
    bass_avg: f32,
    /// Live transient blip: strength, and the bearing it was fired on. It stays where it was
    /// fired rather than following the beam - a return does not move because the antenna did.
    hit: f32,
    hit_pos: f32,
}

impl Radar {
    /// Echo strength for one bearing cell of one face.
    ///
    /// Face `s` takes its own contiguous slice of the spectrum (the whole of it when there is
    /// only one face), and the cell takes a sub-slice of that.
    fn cell_level(d: &FrameData, s: usize, scopes: usize, ci: usize) -> f32 {
        let n = d.levels.len();
        let scopes = scopes.max(1);
        let base = s * n / scopes;
        let span = ((s + 1) * n / scopes).saturating_sub(base);
        let lo = (base + ci * span / CELLS).min(n);
        let hi = (base + (ci + 1) * span / CELLS).max(lo + 1).min(n);
        if lo >= hi {
            return 0.0;
        }
        let mut acc = 0.0;
        let mut cnt = 0.0;
        let mut peak = 0.0f32;
        for v in &d.levels[lo..hi] {
            // is_finite BEFORE anything else: f32::clamp returns NaN unchanged (every
            // comparison with NaN is false), so a single poisoned band would otherwise
            // reach `echo`/`glow` and stay there for the life of the process - the display
            // would lose that bearing permanently, with no way to recover it.
            if v.is_finite() {
                acc += *v;
                cnt += 1.0;
                peak = peak.max(*v);
            }
        }
        if cnt <= 0.0 {
            return 0.0;
        }
        let mean = acc / cnt;
        (mean * (1.0 - GROUP_MAX_BIAS) + peak * GROUP_MAX_BIAS).clamp(0.0, 1.0)
    }

    /// Maps a cell level onto the usable 0..1 echo range, scaled by the theme's sensitivity.
    fn response(level: f32, sensitivity: f32) -> f32 {
        if !level.is_finite() {
            return 0.0;
        }
        (((level - RESP_FLOOR) / RESP_SPAN) * sensitivity.max(0.0)).clamp(0.0, 1.0)
    }

    /// Peak of the lowest bands, for the transient detector.
    fn bass_of(d: &FrameData) -> f32 {
        let hi = 8.min(d.levels.len());
        let mut peak = 0.0f32;
        for v in &d.levels[..hi] {
            if v.is_finite() {
                peak = peak.max(*v);
            }
        }
        peak.clamp(0.0, 1.0)
    }
}

impl Family for Radar {
    fn id(&self) -> &'static str {
        "radar"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());
        c.clear();
        let panel = Rgba::from_hex(&t.panel, t.panel_alpha);
        if w < 28 || h < 18 {
            // Below this there is no fan worth drawing: `ry` collapses to a couple of pixels
            // and the range rings, the ticks and the blips all land on the same two rows, so
            // the display would be an unreadable smear rather than a small radar. Fill the
            // panel and stop, exactly as the valve row does.
            c.rounded_rect(1, 2, (w - 2).max(1), (h - 4).max(1), 3, panel);
            return;
        }
        c.rounded_rect(1, 2, w - 2, h - 4, 4, panel);

        let scopes = scope_count(w);

        // ---- timing ----
        //
        // dt clamped, not merely NaN-guarded. A stalled render loop (the overlay is repainted
        // from the message pump, so a drag or a taskbar reflow can hand over a 500ms frame)
        // would otherwise jump the beam most of the way round the fan in one step, painting a
        // stripe of identical returns across bearings that never got sampled.
        let dt = if d.dt_ms.is_finite() { d.dt_ms.clamp(0.0, 120.0) } else { 16.7 };
        if !self.pos.is_finite() {
            self.pos = 0.0;
        }

        let need = scopes * CELLS;
        if self.glow.len() != need {
            // Resizing wipes the picture, and that is correct rather than merely convenient:
            // the number of faces changes which bands map to which bearing, so the stored
            // returns would be plotted against a spectrum they were never sampled from. A
            // resize that does NOT change the face count (190x60 -> 190x62) leaves the
            // picture intact.
            self.glow = vec![0.0; need];
            self.echo = vec![0.0; need];
        }

        // Phosphor decay. `fade` is an existing theme field - `scope.rs` already repurposes
        // it as persistence rather than as a theme cross-fade - so the wake is TOML-tunable
        // without adding to the schema. At the default 0.30 a cell reaches WAKE_FLOOR after
        // exactly one sweep.
        let persist = if t.fade.is_finite() { t.fade.clamp(0.05, 1.5) } else { 0.30 };
        let tau = (SWEEP_MS * (persist / 0.30) / -WAKE_FLOOR.ln()).max(1.0);
        let keep = (-dt / tau).exp();
        for g in self.glow.iter_mut() {
            *g *= keep;
        }

        // Advance the beam and paint every cell it crossed.
        //
        // Every cell, not just the one it landed on: at 43.75ms per cell and a 16.7ms frame
        // the beam usually stays put, but a slow frame crosses two or three and skipping them
        // leaves permanent dead bearings - the picture would develop holes that only a
        // resize could clear.
        // The beam OSCILLATES rather than resetting - a sector scan, which is what real hardware
        // with a limited arc actually does (airport surface radar, marine sector scan, sonar).
        //
        // It used to advance linearly and wrap with `rem_euclid`, which snapped the beam from the
        // far limit back to the near one in a single frame. That discontinuity is jarring on its
        // own, and it is made worse by the wake: at the instant of the reset the beam is drawn on
        // top of the OLDEST returns while its trail is clamped away, so the whole picture appears
        // to lurch once per sweep.
        //
        // Derived from a raised cosine rather than a triangle wave, so angular velocity falls to
        // zero at each limit instead of reversing instantaneously - a mechanical scanner has to
        // decelerate and accelerate at the ends of its arc, and easing it is both smoother and
        // more faithful. It also makes the beam DWELL at the limits, which keeps the end bearings
        // refreshed rather than letting them fade while the beam spends its time mid-arc.
        let prev = self.pos;
        // A full there-and-back cycle is two sweeps, so one sweep still takes SWEEP_MS.
        self.phase = (self.phase + dt / (SWEEP_MS * 2.0)).rem_euclid(1.0);
        if !self.phase.is_finite() {
            self.phase = 0.0;
        }
        let ang = self.phase * std::f32::consts::TAU;
        // Scaled by CELLS, not CELLS-1, and held just inside the top. With CELLS-1 the far cell
        // was only reachable when the phase landed EXACTLY on 0.5, which discrete frames never do,
        // so `floor` capped at CELLS-2 and the outermost bearing was never painted at all - the
        // sweep test caught it as a permanent 0.0 at the last cell.
        self.pos = ((0.5 - 0.5 * ang.cos()) * CELLS as f32).min(CELLS as f32 - 0.001);
        self.dir = if ang.sin() >= 0.0 { 1.0 } else { -1.0 };
        if !self.pos.is_finite() {
            self.pos = 0.0;
        }
        // Every cell between where the beam was and where it is now, in whichever direction. No
        // wrapping: the beam no longer crosses the ends, it turns around at them.
        let (lo, hi) = if self.pos >= prev { (prev, self.pos) } else { (self.pos, prev) };
        for k in (lo.floor() as i64).max(0)..=(hi.floor() as i64).min(CELLS as i64 - 1) {
            let ci = k as usize;
            for s in 0..scopes {
                let resp = Self::response(Self::cell_level(d, s, scopes, ci), t.sensitivity);
                let i = s * CELLS + ci;
                self.echo[i] = resp;
                self.glow[i] = BLIP_FLOOR + (1.0 - BLIP_FLOOR) * resp;
            }
        }

        // Transient detector. The slew-limited average is what the rise is measured against,
        // so a sustained bass line settles and stops firing while a kick still spikes above
        // it.
        let bass = Self::bass_of(d);
        self.hit = (self.hit - dt / BASS_FALL_MS).max(0.0);
        if !self.bass_avg.is_finite() {
            self.bass_avg = 0.0;
        }
        if bass - self.bass_avg > BASS_RISE {
            self.hit = 1.0;
            self.hit_pos = self.pos;
        }
        self.bass_avg += (bass - self.bass_avg) * 0.22;

        // ---- graticule ----
        //
        // Drawn straight onto the panel, NOT onto the lit layer. It is print on the glass,
        // not light: putting it through the bloom with everything else lifted it to within a
        // few luminance steps of the faint end of the wake, at which point the oldest returns
        // were indistinguishable from the range rings and the picture read as a pattern
        // rather than as data.
        // 0.20/0.13, measured against the wake rather than picked: the faintest a stale return
        // gets is WAKE_FLOOR (0.12) of a near-white `hot`, so print above about 0.24 competes
        // with real data, and the first dump showed 0.10 dropping the inner rings below the
        // panel's own near-black by enough that the fan had no visible shape at silence.
        let ink = Rgba::from_hex(&t.lit, 0.20);
        let ink_faint = Rgba::from_hex(&t.lit, 0.13);
        for s in 0..scopes {
            let (cx, cy, rx, ry) = geometry(w, h, scopes, s);

            // Range rings, as joined segments rather than plotted points: stepping the angle
            // and setting one pixel per step leaves 8-connected diagonal runs that read as a
            // dashed ring, the same trap the VU family's printed arc documents.
            for ring in [0.34f32, 0.67, 1.0] {
                let steps = 44;
                let mut p0 = plot(cx, cy, rx, ry, bearing(-0.5), ring);
                for step in 1..=steps {
                    let th = PI * step as f32 / steps as f32;
                    let p1 = plot(cx, cy, rx, ry, th, ring);
                    c.line(p0.0, p0.1, p1.0, p1.1, if ring >= 1.0 { ink } else { ink_faint });
                    p0 = p1;
                }
            }

            // Bearing ticks every 30 degrees, cut in from the outer ring, plus the horizon
            // baseline and the centre pip.
            for k in 0..=6 {
                let th = PI * k as f32 / 6.0;
                let a = plot(cx, cy, rx, ry, th, 0.86);
                let b = plot(cx, cy, rx, ry, th, 1.0);
                c.line(a.0, a.1, b.0, b.1, ink);
            }
            c.fill_rect(cx - rx as i32, cy, (rx as i32) * 2 + 1, 1, ink_faint);
            c.fill_rect(cx, cy - 1, 1, 2, ink);

            // Divider between faces, so a wide panel reads as two displays sharing an
            // antenna rather than as one confusing picture with a seam.
            if s > 0 {
                let x = cx - (rx as i32) - 2;
                c.fill_rect(x, 4, 1, h - 10, Rgba::from_hex(&t.edge, t.edge_alpha * 0.9));
            }
        }

        // ---- light ----
        //
        // Own transparent layer. `Canvas::bloom` composites the halo UNDER what is already
        // there, and the panel above is fully opaque on every shipped colourway, so blooming
        // in place would leave no visible glow at all.
        let mut lit = Canvas::new(w, h);
        let hot = &t.hot;
        for s in 0..scopes {
            let (cx, cy, rx, ry) = geometry(w, h, scopes, s);

            for ci in 0..CELLS {
                let i = s * CELLS + ci;
                let g = self.glow[i];
                if g <= 0.02 {
                    continue;
                }
                let e = self.echo[i];
                let range = RANGE_MIN + (RANGE_MAX - RANGE_MIN) * e;
                let (bx, by) = plot(cx, cy, rx, ry, bearing(ci as f32), range);
                // Blob kept ROUND even though the fan is squashed. A return squashed with the
                // geometry would be an ellipse whose aspect changed with bearing, which reads
                // as a smear rather than as a contact.
                //
                // Small, and smaller than it was. Adjacent cells are only 5.9px apart near the
                // horizons (the ellipse squash bunches them there), so at the first attempt's
                // `1.3 + 2.3 * e` a run of loud neighbouring bearings composited into one
                // saturated blotch about 24px across - individual contacts were gone, which is
                // the one thing this display exists to show.
                let blob = 1.1 + 1.7 * e;
                lit.elliptical_gradient(
                    bx,
                    by,
                    blob,
                    blob,
                    &[
                        (0.0, Rgba::from_hex(hot, (g * 0.85).clamp(0.0, 1.0))),
                        (0.45, Rgba::from_hex(&t.lit, (g * 0.58).clamp(0.0, 1.0))),
                        (1.0, Rgba::from_hex(&t.lit, 0.0)),
                    ],
                );
                // Crisp 1px core, so the blip has a definite position to read. The gradient
                // alone peaks over 2-3 pixels and at this size that is enough to make two
                // adjacent ranges look like the same range.
                lit.fill_rect(bx, by, 1, 1, Rgba::from_hex(hot, g.clamp(0.0, 1.0)));
            }

            // Transient return, close in. Shared across every face on purpose: this is the
            // clutter a hard transmit pulse leaves near the centre, and every display fed by
            // the same antenna shows it.
            if self.hit > 0.02 {
                let (hx, hy) = plot(cx, cy, rx, ry, bearing(self.hit_pos), HIT_RANGE);
                let a = self.hit.clamp(0.0, 1.0);
                lit.elliptical_gradient(
                    hx,
                    hy,
                    2.6,
                    2.6,
                    &[
                        (0.0, Rgba::from_hex(hot, a)),
                        (0.5, Rgba::from_hex(&t.lit, a * 0.6)),
                        (1.0, Rgba::from_hex(&t.lit, 0.0)),
                    ],
                );
            }

            // The beam, leading edge first.
            //
            // The trail is CLAMPED at the start of the fan rather than wrapped round to the
            // far end. Wrapping was the obvious version and it is wrong: it draws the trail at
            // the opposite bearing, where it reads as a second antenna sweeping the other way
            // - and it lands exactly on top of the oldest returns, which are the ones the
            // wake is there to show.
            for step in 0..BEAM_STEPS {
                let back = step as f32 * (BEAM_CELLS / BEAM_STEPS as f32);
                // Behind the beam means the opposite way on the return leg.
                let at = self.pos - back * self.dir;
                if at < -0.5 || at > CELLS as f32 - 0.5 {
                    break;
                }
                let th = bearing(at);
                let (ex, ey) = plot(cx, cy, rx, ry, th, 1.0);
                let f = 1.0 - back / BEAM_CELLS;
                let col = if step == 0 {
                    Rgba::from_hex(hot, 0.95)
                } else {
                    // Low per-line alpha because 24 overlapping lines accumulate: at the 0.55
                    // that suited seven spokes the wedge composited to a solid slab that
                    // erased every blip it swept over.
                    Rgba::from_hex(&t.lit, (f * f * 0.16).clamp(0.0, 1.0))
                };
                lit.line(cx, cy, ex, ey, col);
            }
        }

        if t.bloom > 0.0 {
            let mut halo = lit.clone();
            halo.bloom(t.bloom.max(0.0) as i32, t.glow_strength.clamp(0.0, 1.0));
            c.draw_over(&halo);
        }
        c.draw_over(&lit);

        // Clip back to the panel with the SAME rect it was drawn with, or the bloom - and the
        // outer ring itself, which reaches x = cx +/- rx - escapes onto the bare taskbar and
        // reads as a bright box around the display.
        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 4);
        let e = Rgba::from_hex(&t.edge, t.edge_alpha);
        c.fill_rect(1, 2, w - 2, 1, e);
        c.fill_rect(1, h - 3, w - 2, 1, e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::themes::builtin;

    fn lum(p: Rgba) -> f64 {
        0.2126 * p.r as f64 + 0.7152 * p.g as f64 + 0.0722 * p.b as f64
    }

    fn flat(level: f32) -> FrameData {
        let mut d = FrameData::default();
        for v in d.levels.iter_mut() {
            *v = level;
        }
        d.peaks = d.levels;
        d
    }

    /// A spectrum with a loud region and a quiet one, so per-bearing behaviour is visible.
    ///
    /// Every level-sweep test in the valve row's first version drove EVERY band to the same
    /// value, where a cell's mean equals its max and the whole per-bearing mapping is a
    /// mathematical no-op - which is exactly how a static-looking row passed its tests. Any
    /// test here that could pass on a display showing one number uses this instead.
    fn uneven(loud_cells: std::ops::Range<usize>, loud: f32, quiet: f32) -> FrameData {
        let mut d = FrameData::default();
        let n = d.levels.len();
        for v in d.levels.iter_mut() {
            *v = quiet;
        }
        for ci in loud_cells {
            let lo = ci * n / CELLS;
            let hi = ((ci + 1) * n / CELLS).max(lo + 1).min(n);
            for v in &mut d.levels[lo..hi] {
                *v = loud;
            }
        }
        d.peaks = d.levels;
        d
    }

    fn run(t: &Theme, d: &FrameData, w: i32, h: i32, frames: usize) -> (Radar, Canvas) {
        let mut r = Radar::default();
        let mut c = Canvas::new(w, h);
        for _ in 0..frames {
            r.draw(&mut c, t, d);
        }
        (r, c)
    }

    /// Range at which the brightest pixel along a bearing sits, in normalised ellipse units.
    /// This is the measurement the whole family stands on: the blip's POSITION, read off the
    /// pixels rather than off the state.
    fn peak_range(c: &Canvas, w: i32, h: i32, scopes: usize, s: usize, cell: usize) -> f32 {
        let (cx, cy, rx, ry) = geometry(w, h, scopes, s);
        let th = bearing(cell as f32);
        let mut best = (0.0f64, 0.0f32);
        let mut r = 0.05f32;
        while r <= 1.0 {
            let (x, y) = plot(cx, cy, rx, ry, th, r);
            let v = lum(c.get(x, y));
            if v > best.0 {
                best = (v, r);
            }
            r += 0.01;
        }
        best.1
    }

    #[test]
    fn renders_at_every_plausible_size_without_panicking() {
        let t = builtin::radar_p1();
        let d = uneven(4..9, 0.62, 0.18);
        for (w, h) in [
            (190, 60),
            (380, 60),
            (456, 60),
            (240, 72),
            (150, 48),
            (96, 40),
            (40, 24),
            (12, 12),
            (1, 1),
            (0, 0),
        ] {
            let (_, c) = run(&t, &d, w, h, 4);
            assert_eq!(
                c.bits().len(),
                (w.max(0) * h.max(0)) as usize,
                "{w}x{h} changed the canvas size"
            );
        }
    }

    #[test]
    fn nan_and_infinity_cannot_poison_the_sweep_or_the_picture() {
        // The project has been bitten twice by f32::clamp NOT sanitising NaN. Here a poisoned
        // dt would leave `pos` NaN, which makes every bearing calculation NaN, which casts to
        // 0 when plotted - the beam would collapse onto the centre pip for the life of the
        // process.
        let t = builtin::radar_p1();
        let mut r = Radar::default();
        let mut c = Canvas::new(190, 60);
        for spoil in 0..3 {
            let mut d = uneven(2..6, 0.6, 0.2);
            match spoil {
                0 => {
                    d.levels[0] = f32::NAN;
                    d.levels[31] = f32::NAN;
                    d.dt_ms = f32::NAN;
                }
                1 => {
                    d.levels[7] = f32::INFINITY;
                    d.levels[8] = f32::NEG_INFINITY;
                    d.dt_ms = f32::INFINITY;
                }
                _ => d.dt_ms = 0.0,
            }
            for _ in 0..12 {
                r.draw(&mut c, &t, &d);
            }
            assert!(r.pos.is_finite(), "spoil {spoil} left the beam position at {}", r.pos);
            assert!(r.bass_avg.is_finite(), "spoil {spoil} poisoned the bass average");
            assert!(
                r.glow.iter().all(|g| g.is_finite()) && r.echo.iter().all(|e| e.is_finite()),
                "spoil {spoil} poisoned the stored picture"
            );
        }
        // And it must still recover: normal frames after the poison have to draw a picture.
        for _ in 0..120 {
            r.draw(&mut c, &t, &uneven(2..6, 0.6, 0.2));
        }
        assert!(r.glow.iter().any(|g| *g > 0.1), "the display never recovered after a NaN");
    }

    #[test]
    fn the_beam_advances_with_dt_and_wraps_round_the_fan() {
        let t = builtin::radar_p1();
        let mut r = Radar::default();
        let mut c = Canvas::new(190, 60);
        let d = flat(0.4);
        r.draw(&mut c, &t, &d);
        let a = r.pos;
        for _ in 0..10 {
            r.draw(&mut c, &t, &d);
        }
        let b = r.pos;
        assert!(b > a, "the beam must advance with dt: {a} -> {b}");
        // One full sweep plus a bit, so the wrap has to have happened.
        for _ in 0..100 {
            r.draw(&mut c, &t, &d);
        }
        assert!(
            r.pos >= 0.0 && r.pos < CELLS as f32,
            "the beam must wrap into the fan, got {}",
            r.pos
        );
        // A zero-dt frame must not advance it - the render loop can deliver one.
        let held = r.pos;
        let mut still = flat(0.4);
        still.dt_ms = 0.0;
        r.draw(&mut c, &t, &still);
        assert_eq!(r.pos, held, "a zero-length frame must not move the beam");
    }

    #[test]
    fn one_revolution_paints_every_bearing() {
        // The defining behaviour: the picture is BUILT UP over a sweep rather than redrawn.
        // A fresh display must have empty bearings, and a settled one must have none.
        let t = builtin::radar_p1();
        let (fresh, _) = run(&t, &uneven(4..9, 0.62, 0.18), 190, 60, 6);
        let painted = fresh.glow.iter().filter(|g| **g > 0.01).count();
        assert!(
            painted < CELLS / 2,
            "after 6 frames most bearings should still be unpainted, {painted} of {CELLS} are lit"
        );

        let (settled, _) = run(&t, &uneven(4..9, 0.62, 0.18), 190, 60, 100);
        assert!(
            settled.glow.iter().all(|g| *g > 0.01),
            "after a full sweep every bearing must carry a return: {:?}",
            settled.glow
        );
    }

    #[test]
    fn a_loud_band_pushes_its_blip_further_out_than_a_quiet_one() {
        // THE test for this family. Position, not brightness, read off the rendered pixels
        // along two different bearings - so it cannot pass on a display that only changes
        // colour, and it cannot pass on a flat spectrum either.
        let t = builtin::radar_p1();
        let (r, c) = run(&t, &uneven(4..8, 0.62, 0.17), 190, 60, 200);
        // Neither probe may sit under the beam, whose own line runs the full radius and would
        // swamp the measurement. Deterministic at a fixed dt, but asserted rather than assumed.
        for probe in [6usize, 22] {
            assert!(
                (r.pos - probe as f32).abs() > 3.0,
                "probe cell {probe} is under the beam at {} - retune the frame count",
                r.pos
            );
        }
        let loud = peak_range(&c, 190, 60, 1, 0, 6);
        let quiet = peak_range(&c, 190, 60, 1, 0, 22);
        assert!(
            loud > quiet + 0.25,
            "a loud bearing's blip must sit clearly further out: loud {loud:.2} vs quiet {quiet:.2}"
        );
        // And the quiet one must not be pinned at the centre, or "further out" is trivial.
        assert!(quiet > 0.1, "a quiet bearing must still return close-in clutter, got {quiet:.2}");
    }

    #[test]
    fn the_audio_actually_changes_the_pixels() {
        // Whole-canvas, so it catches a family that draws its graticule and beam and ignores
        // the spectrum entirely.
        let t = builtin::radar_p1();
        let total = |d: &FrameData| -> f64 {
            let (_, c) = run(&t, d, 190, 60, 100);
            let mut sum = 0.0;
            for y in 0..60 {
                for x in 0..190 {
                    sum += lum(c.get(x, y));
                }
            }
            sum
        };
        let quiet = total(&flat(0.05));
        let loud = total(&flat(0.95));
        assert!(loud > quiet * 1.15, "driving the input must light the face: {quiet:.0} -> {loud:.0}");

        // And two different SHAPES of spectrum at the same total energy must differ, which a
        // display keyed only to overall loudness would fail.
        let (_, a) = run(&t, &uneven(0..8, 0.62, 0.17), 190, 60, 200);
        let (_, b) = run(&t, &uneven(24..32, 0.62, 0.17), 190, 60, 200);
        assert_ne!(a.bits(), b.bits(), "bass-heavy and treble-heavy must not render identically");
    }

    #[test]
    fn the_wake_leaves_fresh_returns_brighter_than_stale_ones() {
        // A flat spectrum on purpose: every cell was painted from the same level, so any
        // difference between them is the decay and nothing else.
        let t = builtin::radar_p1();
        let (r, _) = run(&t, &flat(0.5), 190, 60, 200);
        let behind = (r.pos.floor() as usize).min(CELLS - 1);
        let ahead = (behind + 3) % CELLS;
        assert!(
            r.glow[behind] > r.glow[ahead] * 1.5,
            "the bearing just swept must out-glow one swept a revolution ago: {} vs {}",
            r.glow[behind],
            r.glow[ahead]
        );
        assert!(
            r.glow[ahead] > 0.02,
            "but the stale one must still be visible, or the display is one bare line: {}",
            r.glow[ahead]
        );
    }

    #[test]
    fn a_bass_transient_fires_a_close_in_blip() {
        let t = builtin::radar_p1();
        let steady = flat(0.10);
        let mut kick = flat(0.10);
        for v in &mut kick.levels[..8] {
            *v = 0.85;
        }
        // Identical run in both cases except for the last three frames, so the beam, the
        // graticule and the whole painted picture are the same and the difference measured
        // below is the transient blip alone.
        let render = |hit: bool| -> (Radar, Canvas) {
            let mut r = Radar::default();
            let mut c = Canvas::new(190, 60);
            for _ in 0..100 {
                r.draw(&mut c, &t, &steady);
            }
            for _ in 0..3 {
                r.draw(&mut c, &t, if hit { &kick } else { &steady });
            }
            (r, c)
        };
        let (r_hit, c_hit) = render(true);
        let (r_no, c_no) = render(false);
        assert!(r_hit.hit > 0.5, "a bass jump must fire the transient, got {}", r_hit.hit);
        assert!(r_no.hit < 0.05, "a steady bass must not fire it, got {}", r_no.hit);

        let (cx, cy, rx, ry) = geometry(190, 60, 1, 0);
        let (hx, hy) = plot(cx, cy, rx, ry, bearing(r_hit.hit_pos), HIT_RANGE);
        let window = |c: &Canvas| -> f64 {
            let mut s = 0.0;
            for y in (hy - 3)..=(hy + 3) {
                for x in (hx - 4)..=(hx + 4) {
                    s += lum(c.get(x, y));
                }
            }
            s
        };
        let (with, without) = (window(&c_hit), window(&c_no));
        assert!(
            with > without + 400.0,
            "the transient must put real light close in: {without:.0} -> {with:.0}"
        );
    }

    #[test]
    fn the_group_reducer_is_biased_toward_the_peak_not_the_mean() {
        // Two bands feed one cell at the reference size, so a single-band peak is halved by a
        // plain mean. Invisible to any test driving every band alike.
        let mut d = FrameData::default();
        let n = d.levels.len();
        for v in d.levels.iter_mut() {
            *v = 0.1;
        }
        let lo = 5 * n / CELLS;
        d.levels[lo] = 0.9;
        let hi = ((5 + 1) * n / CELLS).min(n);
        let slice = &d.levels[lo..hi];
        let mean = slice.iter().sum::<f32>() / slice.len() as f32;
        let peak = slice.iter().copied().fold(0.0f32, f32::max);
        assert!(peak > mean + 0.2, "the fixture must actually have a peak to hide");
        let got = Radar::cell_level(&d, 0, 1, 5);
        assert!(got > mean + (peak - mean) * 0.5, "must sit above the midpoint: {got} in [{mean}, {peak}]");
        assert!(got <= peak + 1e-6, "but never above the peak: {got} vs {peak}");
    }

    #[test]
    fn the_response_window_spends_its_range_on_levels_the_dsp_actually_produces() {
        let lo = Radar::response(0.15, 1.0);
        let hi = Radar::response(0.65, 1.0);
        assert!(hi - lo > 0.75, "the music window must cover most of the range: {lo} -> {hi}");
        assert_eq!(Radar::response(0.0, 1.0), 0.0, "silence must not sit on a pedestal");
        assert_eq!(Radar::response(1.0, 1.0), 1.0, "full scale must reach maximum range");
        assert!(
            Radar::response(0.3, 2.0) > Radar::response(0.3, 1.0),
            "sensitivity is the user-facing knob and must do something"
        );
    }

    #[test]
    fn a_wide_panel_adds_a_second_face_instead_of_stretching_the_first() {
        assert_eq!(scope_count(190), 1, "the reference panel keeps the single tuned face");
        assert_eq!(scope_count(380), 2, "double the width buys a second face");
        assert_eq!(scope_count(456), 2);
        assert!(scope_count(4000) <= 4, "capped");
        // Squash ratio is the thing that must not drift - a stretched single face at 380px
        // reaches 3.9:1 and stops reading as a squashed circle.
        for w in [190, 240, 380, 456, 600] {
            let (_, _, rx, ry) = geometry(w, 60, scope_count(w), 0);
            let squash = rx / ry;
            assert!(
                squash < 2.6,
                "at width {w} the fan squashed to {squash:.2}:1, well past the tuned 1.9:1"
            );
        }
        // Both faces must actually carry light at 380px.
        let t = builtin::radar_p1();
        let (_, c) = run(&t, &uneven(0..CELLS, 0.6, 0.2), 380, 60, 120);
        for (lo, hi) in [(4, 186), (196, 376)] {
            let best = (4..56)
                .flat_map(|y| (lo..hi).map(move |x| (x, y)))
                .map(|(x, y)| lum(c.get(x, y)))
                .fold(0.0f64, f64::max);
            assert!(best > 60.0, "the face spanning x {lo}..{hi} drew nothing (peak {best:.0})");
        }
    }

    #[test]
    fn the_two_faces_of_a_wide_panel_show_different_halves_of_the_spectrum() {
        // Otherwise the wide mode is just the same picture twice, and the extra bearing
        // resolution it exists for is not there.
        let t = builtin::radar_p1();
        let mut d = FrameData::default();
        let n = d.levels.len();
        for (i, v) in d.levels.iter_mut().enumerate() {
            *v = if i < n / 2 { 0.62 } else { 0.16 };
        }
        d.peaks = d.levels;
        let (r, _) = run(&t, &d, 380, 60, 120);
        let face = |s: usize| -> f32 {
            r.echo[s * CELLS..(s + 1) * CELLS].iter().sum::<f32>() / CELLS as f32
        };
        assert!(
            face(0) > face(1) + 0.3,
            "the left face takes the low half: {} vs {}",
            face(0),
            face(1)
        );
    }

    #[test]
    fn nothing_is_drawn_outside_the_panel() {
        // The outer range ring reaches x = cx +/- rx and the bloom spreads past it, so without
        // the clip the halo lands on the bare taskbar as a bright box around the display.
        let t = builtin::radar_alert();
        let (_, c) = run(&t, &flat(0.95), 190, 60, 120);
        for x in 0..190 {
            for y in [0, 1, 58, 59] {
                assert_eq!(c.get(x, y).a, 0, "({x},{y}) is outside the panel");
            }
        }
        for y in 0..60 {
            assert_eq!(c.get(0, y).a, 0, "column 0 is outside the panel");
            assert_eq!(c.get(189, y).a, 0, "column 189 is outside the panel");
        }
    }

    #[test]
    fn a_tiny_panel_still_draws_something_and_stays_inside_it() {
        // The degenerate branch must not be a blank canvas - the overlay would show the
        // Widgets button's own weather text through it.
        let t = builtin::radar_ice();
        let (_, c) = run(&t, &flat(0.6), 40, 24, 4);
        assert!(c.bits().iter().any(|p| *p != 0), "the small-panel fallback drew nothing");
        let (_, tiny) = run(&t, &flat(0.6), 1, 1, 4);
        assert_eq!(tiny.bits().len(), 1);
    }

    #[test]
    fn every_radar_colourway_renders_and_differs() {
        let mut seen: Vec<Vec<u32>> = Vec::new();
        for t in builtin::all().into_iter().filter(|t| t.family == "radar") {
            let (_, c) = run(&t, &uneven(4..9, 0.62, 0.18), 190, 60, 100);
            assert!(c.bits().iter().any(|p| *p != 0), "{} rendered nothing", t.id);
            for prior in &seen {
                assert_ne!(prior, c.bits(), "{} renders identically to another colourway", t.id);
            }
            seen.push(c.bits().to_vec());
        }
        assert_eq!(seen.len(), 5, "expected the five radar colourways, got {}", seen.len());
    }

    /// Run: cargo test --release dump_radar_frames -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_radar_frames() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();
        // An uneven, musical-looking spectrum, so the blips sit at visibly different ranges.
        let mut d = FrameData::default();
        for (i, v) in d.levels.iter_mut().enumerate() {
            let x = i as f32 / 63.0;
            *v = (0.15 + 0.85 * (x * 7.0).sin().abs()) * (1.0 - x * 0.42);
        }
        d.peaks = d.levels;

        let dump = |c: &Canvas, name: &str| {
            let (w, h) = (c.width(), c.height());
            let mut out = Vec::with_capacity((w * h * 4) as usize);
            for y in 0..h {
                for x in 0..w {
                    let px = c.get(x, y);
                    let a = px.a as f32 / 255.0;
                    for ch in [px.r, px.g, px.b] {
                        out.push((ch as f32 + 22.0 * (1.0 - a)).min(255.0) as u8);
                    }
                    out.push(255);
                }
            }
            std::fs::write(dir.join(name), &out).unwrap();
        };

        let mut n = 0usize;
        for t in builtin::all().into_iter().filter(|t| t.family == "radar") {
            // Just over one sweep at 16.7ms/frame, so the wake is fully developed and the
            // beam is mid-fan - which is the state a human will actually be looking at.
            let (_, c) = run(&t, &d, 190, 60, 100);
            dump(&c, &format!("radar-{}.rgba", t.id));
            n += 1;
        }
        // One wide dump too: the two-face layout is the part most likely to be wrong and it
        // is invisible at the reference size.
        let t = builtin::radar_p1();
        let (_, wide) = run(&t, &d, 380, 60, 100);
        dump(&wide, "radar-wide-p1.rgba");
        println!("wrote {} radar dumps (190x60) + 1 wide (380x60) to {}", n, dir.display());
    }
}
