//! The spectrogram-waterfall family: frequency up the panel, time scrolling left, level as colour.
//!
//! The only family here that shows HISTORY, which is its entire reason to exist. Every other
//! family draws the present instant - a bar, a needle, a valve - so a phrase is only ever
//! visible as motion you had to be watching. This one leaves the last ~3 seconds on the panel,
//! so a snare, a bass drop and a vocal line are all *shapes* you can still read after they
//! happened.
//!
//! Four decisions carry it, and each was forced by the 190x60 canvas rather than chosen:
//!
//! - **The ramp's dark end is the PANEL, not a colour.** Intensity maps to a multi-stop ramp
//!   whose bottom stop is fully transparent, so a quiet cell simply shows the near-black panel
//!   and a loud one is opaque. That is what makes "black-red-yellow-white" work with only three
//!   authored colours, and - the reason it matters mechanically - it leaves the quiet regions
//!   TRANSPARENT on the lit layer, which is the only place a bloom halo can be seen at all
//!   (see the layer note in `draw`).
//! - **Bands fold into rows by MAX.** 54 usable rows for 64 bands, so 10 of those rows carry two
//!   bands each and at a smaller height every row carries several. Averaging them flattens
//!   exactly the single-band peaks that make one row differ from the one above it. Measured on an
//!   18-row plot (h=24) with one band at 0.75 among a 0.14 floor: max-folding renders that row at
//!   luminance 245, the top of the ramp, where the mean of its four bands (0.29) would render it at
//!   86 - the same peak turned into background.
//! - **One column per 16.7ms of REAL time, not per frame.** 183 plot columns at the reference
//!   width is a 3.1s window: long enough to hold a phrase, and at 1px per frame the scroll is
//!   smooth. A 2-frame cadence would double the window to 6.1s but steps the whole image sideways
//!   at 30Hz, which is visible as judder on a 2px-wide event. Driving it from `dt_ms` rather than
//!   counting frames is what keeps the time axis honest: the render loop sleeps a fixed 16ms plus
//!   however long the frame took, so a frame-counted axis silently stretches history under load
//!   and the same 3.1s window would represent 4s or 5s with no way to tell.
//! - **A moving marker, because colour alone is a weak cue at this size.** The whole family
//!   encodes level as intensity, which this project has already measured to be the weaker channel
//!   (the valve row got a 1.16x spread from brightness alone, below the visible threshold). So the
//!   dominant band's POSITION is drawn explicitly: a bright block in a rail down the right-hand
//!   side that rides up and down with the loudest band, plus a 1px tick per column inside the plot
//!   that leaves the same reading behind as a fading pitch track. The rail block is the cue you
//!   actually read; the track is what makes it history.

use super::canvas::{Canvas, Rgba};
use super::{Family, FrameData};
use crate::dsp::bands::NUM_BANDS;
use crate::themes::Theme;

/// Columns of spectra kept in the ring buffer.
///
/// Sized by the widest panel this can be asked to draw, not by the reference width: the overlay
/// claims taskbar space leftward and the wide mode is 380px, with 456 already exercised by the
/// dispatch tests. 512 covers every plausible rect with room to spare and costs 512 * 64 * 4 =
/// 128KB, allocated once on the first frame. A panel wider than this would simply leave its
/// left-hand columns empty rather than misbehave.
const HIST_COLS: usize = 512;

/// Real time one column represents. See the scroll note in the module docs.
const COLUMN_MS: f32 = 16.7;

/// Columns the flourish's tear spans. See `Waterfall::tear`.
const TEAR_COLS: u8 = 3;

/// Most columns a single frame may push.
///
/// A stall - a device change, a theme reload, the machine swapping - hands us one enormous
/// `dt_ms`, and catching up faithfully would rip several hundred columns of duplicated spectrum
/// through the plot in one frame, erasing the history it exists to show. Dropping the backlog
/// instead costs a small discontinuity in the time axis, which is invisible, and keeps everything
/// on screen.
const MAX_STEPS: usize = 4;

/// Band level at which a cell starts to colour, and the span it colours over.
///
/// The input range is NOT 0..1: `FrameData.levels` sits at roughly 0.15-0.65 for active bands on
/// real music. A ramp spread over 0..1 would spend two thirds of its colours on levels that never
/// arrive, so a whole track would render in the bottom third of the ramp - a red-brown wash with
/// the yellows and whites unreachable. This window puts 0.15 just above the transparent floor and
/// 0.65 at the top of the ramp, and `Theme.sensitivity` scales it so it stays TOML-tunable.
///
/// A fixed window, like the valve row's and for the same reason: a spectrogram is read for
/// absolute level as well as shape, and an auto-ranging ramp would paint a quiet passage in the
/// same whites as a loud one.
const RESP_FLOOR: f32 = 0.12;
const RESP_SPAN: f32 = 0.50;

/// Alpha of the ramp's lowest AUTHORED stop.
///
/// The stop below it is fully transparent (the panel showing through), so this sets how abruptly a
/// cell appears out of the background. 0.45 was picked so the faintest visible cell reads as a
/// dark tint of the ramp's first colour rather than as a solid block switching on: measured on the
/// heat ramp, the first authored stop composites to luminance 49 against the panel's 5.9, and the
/// steps below it fall away to the panel in even ~2.2 increments.
const RAMP_ALPHA_LOW: f32 = 0.45;

/// Entries in the colour lookup table.
///
/// The ramp is evaluated once per cell and a 190x60 panel has ~9,900 of them per frame, so it is
/// baked into a table each frame instead of interpolated per cell. 64 steps quantises the ramp far
/// finer than a 1px cell against a dark panel can show - measured on the heat ramp the largest
/// luminance step between adjacent LUT entries is 5.5, against a full-scale range of 239.
const RAMP_STEPS: usize = 64;

/// Sentinel for "this column had no band worth marking".
const NO_MARK: u8 = u8::MAX;

/// Response below which the dominant-band marker is suppressed.
///
/// Without it, silence still has a numerically loudest band - whichever one the noise floor
/// happens to favour - so the pitch track would draw a jittering line across an empty plot and
/// read as a fault.
const MARK_MIN_RESP: f32 = 0.06;

/// Alpha of the in-plot pitch track at the newest and oldest column.
///
/// It fades with age so the reading you are meant to take - the current dominant band - is the
/// brightest point on the track, while the older part still traces where it has been. A constant
/// alpha made the track read as a static drawn curve rather than as something being written.
const TRACK_A_NEW: f32 = 0.95;
const TRACK_A_OLD: f32 = 0.20;

/// Geometry. Row 2 and row `h-3` are the bezel lines, so the plot occupies rows 3..h-3, and
/// column 1 is left free for the frequency-axis ticks.
const PLOT_LEFT: i32 = 2;
const PLOT_TOP: i32 = 3;
/// Width of the dominant-band rail, and the total space reserved for it on the right.
///
/// The reserve is one wider than gap + rail so the rail cannot land in the columns
/// `clip_to_rounded_rect` cuts for the panel's rounded corners (at r=3 the inset reaches 2px, so
/// the last two columns of the panel disappear on the top and bottom rows) - the marker's whole
/// job is to be readable at the extremes of its travel, which is exactly where that would have
/// eaten it.
const RAIL_W: i32 = 2;
const RAIL_RESERVE: i32 = 4;

/// Below these dimensions there is no plot worth drawing, so only the panel is painted. Chosen so
/// the plot keeps at least ~6 rows and a handful of columns; the degenerate cases (1x1) fall out
/// of `rounded_rect`'s own guards without reaching here.
const MIN_W: i32 = 14;
const MIN_H: i32 = 12;

/// Maps a band level onto 0..=1 of the colour ramp. See RESP_FLOOR.
fn response(level: f32, sensitivity: f32) -> f32 {
    // is_finite FIRST: f32::clamp does not sanitise NaN (every comparison with NaN is false, so
    // clamp hands it straight back), and this value indexes the LUT and scales alphas.
    if !level.is_finite() {
        return 0.0;
    }
    (((level - RESP_FLOOR) / RESP_SPAN) * sensitivity.max(0.0)).clamp(0.0, 1.0)
}

/// The ramp, as ascending (position, straight-colour) stops.
///
/// `Theme.zones` is the natural home for this and needs no new field: a zone is already
/// "(position, colour)" and the family that introduced them uses them for exactly this - a
/// green/amber/red scale up a bar. Here they are the ramp's authored stops, so a colourway (or an
/// external TOML file) can declare any number of them. A theme with fewer than two zones falls
/// back to `lit -> hot`, which is what a hand-written file that only set the two obvious colours
/// will have.
///
/// The stop at 0.0 is synthesised, not authored, and is fully transparent - see the module docs.
/// It also keeps the authored stops bright enough to pass the project's 3:1 lit-vs-panel contrast
/// rule, which the dark end of a heat ramp could never satisfy if it had to be a real colour.
fn ramp_stops(t: &Theme) -> Vec<(f32, Rgba)> {
    let mut authored: Vec<(f32, &str)> =
        t.zones.iter().map(|z| (z.upto, z.lit.as_str())).collect();
    if authored.len() < 2 {
        authored = vec![(0.55, t.lit.as_str()), (1.0, t.hot.as_str())];
    }
    // Force the positions to ascend. Theme files are user-authored and `zones` has no ordering
    // guarantee at the schema level, and a non-monotonic stop list would make the interpolation
    // below pick whichever segment it hit first - i.e. a silently wrong ramp rather than an error.
    let mut prev = 0.0f32;
    let mut stops = Vec::with_capacity(authored.len() + 1);
    stops.push((0.0, Rgba::from_hex(authored[0].1, 0.0)));
    let last = authored.len() - 1;
    for (i, (pos, hex)) in authored.iter().enumerate() {
        let pos = if pos.is_finite() { pos.clamp(0.0, 1.0) } else { 1.0 };
        let pos = pos.max(prev);
        prev = pos;
        let a = RAMP_ALPHA_LOW + (1.0 - RAMP_ALPHA_LOW) * (i as f32 / last as f32);
        stops.push((pos, Rgba::from_hex(hex, a)));
    }
    stops
}

/// Interpolates `stops` in STRAIGHT colour space.
///
/// Deliberately not premultiplied, mirroring `Canvas::sample_stops`' own note: with alpha falling
/// to zero at the bottom of the ramp, interpolating premultiplied values would drag the colour
/// toward black as it faded, so a red cell would go maroon on its way out instead of simply
/// getting fainter. The canvas premultiplies on store.
fn ramp_at(stops: &[(f32, Rgba)], x: f32) -> Rgba {
    if stops.is_empty() {
        return Rgba::TRANSPARENT;
    }
    if x <= stops[0].0 {
        return stops[0].1;
    }
    let last = stops[stops.len() - 1];
    if x >= last.0 {
        return last.1;
    }
    for pair in stops.windows(2) {
        let (p0, c0) = pair[0];
        let (p1, c1) = pair[1];
        if x >= p0 && x <= p1 {
            let f = ((x - p0) / (p1 - p0).max(f32::EPSILON)).clamp(0.0, 1.0);
            let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * f).round() as u8;
            return Rgba::new(mix(c0.r, c1.r), mix(c0.g, c1.g), mix(c0.b, c1.b), mix(c0.a, c1.a));
        }
    }
    last.1
}

fn ramp_lut(t: &Theme) -> [Rgba; RAMP_STEPS] {
    let stops = ramp_stops(t);
    let mut lut = [Rgba::TRANSPARENT; RAMP_STEPS];
    for (i, e) in lut.iter_mut().enumerate() {
        *e = ramp_at(&stops, i as f32 / (RAMP_STEPS - 1) as f32);
    }
    lut
}

#[derive(Default)]
pub struct Waterfall {
    /// Ring buffer of RAW band levels, `HIST_COLS * NUM_BANDS`, newest at `head`.
    ///
    /// Raw rather than already mapped to colour, so switching theme or `sensitivity` recolours
    /// the whole visible history instead of leaving a seam where the old mapping stops. The cost
    /// is remapping ~9,900 cells per frame, which the LUT makes a table lookup.
    ///
    /// A flat `Vec` because `[f32; 64]` has no `Default` (std's array impls stop at 32) and a
    /// `Vec<Vec<f32>>` would be 512 separate allocations for a buffer that is written strictly
    /// one column at a time.
    hist: Vec<f32>,
    /// Per-column dominant band index, or `NO_MARK`.
    mark: Vec<u8>,
    /// That band's level, kept so the marker's visibility can be judged against the CURRENT
    /// theme's sensitivity rather than being frozen at push time.
    mark_lvl: Vec<f32>,
    head: usize,
    /// The flourish: a broadband tear written into the history. See `dsp::flourish`.
    ///
    /// No envelope, unlike every other family's. A spectrogram already HAS persistence - the torn
    /// column is written into the ring buffer once and then scrolls away on its own over the next few
    /// seconds, which is a longer and more legible afterlife than any decay envelope could give it. It
    /// is the one family whose flourish is a fact about the data rather than a filter over the drawing.
    flourish: crate::dsp::flourish::Trigger,
    /// Columns of tear still to be written, set when the flourish fires and consumed by
    /// `push_column`.
    ///
    /// A COUNT rather than a flag, and `TEAR_COLS` wide rather than one. A single full-scale column
    /// read as merely one brighter column among the audio's own - visible in the eyeball dump, but not
    /// as a rip. Three columns is about 50ms of history and reads as a discontinuity in the recording,
    /// which is what a dropout actually looks like on a spectrogram.
    ///
    /// Deferred rather than applied on the flourish frame because the history advances on its OWN clock
    /// - `COLUMN_MS`, not the frame interval - so a flourish frame and a column push are different
    /// events. Writing directly would have torn whichever column happened to be current, which on a
    /// slow frame can be several pushes old.
    tear: u8,
    filled: usize,
    /// Leftover real time not yet worth a column.
    acc: f32,
}

impl Waterfall {
    /// Pushes as many columns as real time has earned since the last frame.
    fn advance(&mut self, d: &FrameData) {
        if self.hist.len() != HIST_COLS * NUM_BANDS {
            self.hist = vec![0.0; HIST_COLS * NUM_BANDS];
            self.mark = vec![NO_MARK; HIST_COLS];
            self.mark_lvl = vec![0.0; HIST_COLS];
            self.head = 0;
            self.filled = 0;
            self.acc = 0.0;
        }
        // A NaN dt would poison `acc` permanently - every later frame adds to NaN, `NaN /
        // COLUMN_MS` floors to NaN, and `as usize` saturates it to 0, so the waterfall would
        // freeze until the process restarted. Same class of bug as the vaporwave scroll phase.
        let dt = if d.dt_ms.is_finite() { d.dt_ms.clamp(0.0, 1000.0) } else { COLUMN_MS };
        self.acc += dt;
        let mut steps = (self.acc / COLUMN_MS).floor().max(0.0) as usize;
        // The first frame must draw something even though no time has passed yet, or a freshly
        // selected theme shows an empty panel until the second frame.
        if self.filled == 0 {
            steps = steps.max(1);
        }
        if steps > MAX_STEPS {
            steps = MAX_STEPS;
            self.acc = 0.0;
        } else {
            self.acc = (self.acc - steps as f32 * COLUMN_MS).max(0.0);
        }
        for _ in 0..steps {
            self.push_column(d);
        }
    }

    fn push_column(&mut self, d: &FrameData) {
        self.head = (self.head + 1) % HIST_COLS;
        let base = self.head * NUM_BANDS;
        let mut best = 0.0f32;
        let mut best_i = NO_MARK;
        // THE FLOURISH: a broadband tear. One column written at full scale across every band, which
        // is what a spectrogram shows when the signal chain drops out for an instant - a hard vertical
        // rip through the whole spectrum. Consumed here, so it lands on a real column boundary.
        let tear = self.tear > 0;
        self.tear = self.tear.saturating_sub(1);
        for i in 0..NUM_BANDS {
            // Sanitised on the way IN, so nothing non-finite can ever be held in the history and
            // every later frame that redraws these columns is safe by construction.
            let mut v = if d.levels[i].is_finite() { d.levels[i].clamp(0.0, 1.0) } else { 0.0 };
            if tear {
                v = 1.0;
            }
            self.hist[base + i] = v;
            if v > best {
                best = v;
                best_i = i as u8;
            }
        }
        self.mark[self.head] = best_i;
        self.mark_lvl[self.head] = best;
        self.filled = (self.filled + 1).min(HIST_COLS);
    }

    /// Ring slot for a column `age` columns behind the newest.
    fn slot(&self, age: usize) -> usize {
        (self.head + HIST_COLS - age % HIST_COLS) % HIST_COLS
    }
}

impl Family for Waterfall {
    fn id(&self) -> &'static str {
        "waterfall"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());
        c.clear();

        // The flourish is armed before the size guard so a canvas too small to draw still keeps the
        // trigger's history current, and `tear` is latched rather than acted on - see the field.
        let dt = if d.dt_ms.is_finite() { d.dt_ms.clamp(0.0, 200.0) } else { 16.7 };
        if self.flourish.update(&d.levels, dt, t.flourish) {
            self.tear = TEAR_COLS;
        }
        // `rounded_rect` guards non-positive dimensions itself, so the degenerate sizes (1x1,
        // 12x12) draw nothing here and return below rather than reaching any geometry.
        c.rounded_rect(1, 2, w - 2, h - 4, 3, Rgba::from_hex(&t.panel, t.panel_alpha));
        if w < MIN_W || h < MIN_H {
            return;
        }

        // The rail is dropped on a narrow panel rather than shrunk: below ~40px it would cost a
        // tenth of the time axis to show one marker, and the plot is the point.
        let has_rail = w >= 40;
        let plot_w = if has_rail { w - 4 - RAIL_RESERVE } else { w - 4 }.max(1);
        // One column of gap after the plot, so the marker cannot be mistaken for plot data.
        let rail_x = if has_rail { Some(PLOT_LEFT + plot_w + 1) } else { None };
        let plot_h = (h - 6).max(1);
        let plot_bottom = PLOT_TOP + plot_h - 1;

        self.advance(d);

        // Row -> band fold, computed once rather than per column. Row 0 is the BOTTOM of the plot
        // and carries the lowest bands, because bass at the bottom is what every other audio tool
        // does and a flipped frequency axis reads as a different signal entirely.
        //
        // MAX over the range, never the mean - see the fold note in the module docs.
        let rows = plot_h as usize;
        let fold: Vec<(usize, usize)> = (0..rows)
            .map(|r| {
                let lo = r * NUM_BANDS / rows;
                let hi = ((r + 1) * NUM_BANDS / rows).max(lo + 1).min(NUM_BANDS);
                (lo, hi)
            })
            .collect();

        // The rail's own track, on the PANEL layer so it stays a crisp dim scale for the marker
        // to be read against instead of being smeared by the bloom below.
        if let Some(rx) = rail_x {
            c.fill_rect(rx, PLOT_TOP, RAIL_W, plot_h, Rgba::from_hex(&t.edge, 0.30));
        }

        // Everything that emits light goes on its own transparent layer.
        //
        // `Canvas::bloom` composites its halo UNDER the existing content, so blooming a canvas
        // that already carries the opaque panel hides the halo completely - the trap documented in
        // segmented, scope, vu and tube. It matters more here than elsewhere: the halo's only
        // visible home is the TRANSPARENT quiet cells around a loud one, which is exactly where an
        // opaque panel would have swallowed it.
        let lut = ramp_lut(t);
        let mut lit = Canvas::new(w, h);
        let cols = (plot_w as usize).min(self.filled);
        for age in 0..cols {
            let x = PLOT_LEFT + plot_w - 1 - age as i32;
            let base = self.slot(age) * NUM_BANDS;
            for (r, &(lo, hi)) in fold.iter().enumerate() {
                let mut v = 0.0f32;
                for k in lo..hi {
                    v = v.max(self.hist[base + k]);
                }
                let resp = response(v, t.sensitivity);
                if resp <= 0.0 {
                    continue;
                }
                let col = lut[((resp * (RAMP_STEPS - 1) as f32).round() as usize).min(RAMP_STEPS - 1)];
                if col.a == 0 {
                    continue;
                }
                lit.fill_rect(x, plot_bottom - r as i32, 1, 1, col);
            }

            // The pitch track: one pixel per column at that column's dominant band.
            let m = self.mark[self.slot(age)];
            if m == NO_MARK {
                continue;
            }
            let resp = response(self.mark_lvl[self.slot(age)], t.sensitivity);
            if resp <= MARK_MIN_RESP {
                continue;
            }
            let newness = 1.0 - age as f32 / cols.max(1) as f32;
            let a = TRACK_A_OLD + (TRACK_A_NEW - TRACK_A_OLD) * newness;
            let row = m as usize * rows / NUM_BANDS;
            lit.fill_rect(x, plot_bottom - row as i32, 1, 1, Rgba::from_hex(&t.hot, a));
        }

        // The rail marker: the one element whose POSITION, not brightness, carries the reading.
        // 2px wide and 2px tall so it survives at a glance; on the lit layer so it blooms and is
        // the brightest thing on the panel.
        // `mark` is initialised to NO_MARK, so the check below also covers the case where no column
        // has been pushed yet - `advance` guarantees at least one on the first frame anyway.
        if let Some(rx) = rail_x {
            let m = self.mark[self.head];
            let lvl = response(self.mark_lvl[self.head], t.sensitivity);
            if m != NO_MARK && lvl > MARK_MIN_RESP {
                let row = m as usize * rows / NUM_BANDS;
                let y = (plot_bottom - row as i32).clamp(PLOT_TOP, plot_bottom - 1);
                lit.fill_rect(rx, y, RAIL_W, 2, Rgba::from_hex(&t.hot, (0.45 + 0.55 * lvl).min(1.0)));
            }
        }

        if t.bloom > 0.0 {
            let mut glow = lit.clone();
            glow.bloom(t.bloom.round().max(0.0) as i32, t.glow_strength.clamp(0.0, 1.0));
            c.draw_over(&glow);
        }
        c.draw_over(&lit);

        // Frequency-axis ticks, in the 1px column the plot leaves free at the panel's left edge.
        // Without them the vertical axis has no reference at all and the image reads as an
        // abstract texture rather than as a spectrum; with three of them the eye has a low/mid/high
        // anchor. Quarter positions rather than halves so the middle tick is not mistaken for a
        // centre line of a waveform.
        for k in 1..4 {
            c.fill_rect(1, PLOT_TOP + plot_h * k / 4, 1, 1, Rgba::from_hex(&t.edge, 0.55));
        }

        // Clip AFTER the bloom, with the panel's own rect: the bloom spreads up to its radius in
        // every direction, which is far more than the 1-2px margin the panel leaves, so without
        // this the halo escapes onto the bare taskbar as a bright box around the display.
        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);
        let e = Rgba::from_hex(&t.edge, t.edge_alpha);
        c.fill_rect(1, 2, w - 2, 1, e);
        c.fill_rect(1, h - 3, w - 2, 1, e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::themes::builtin;

    fn flat(level: f32) -> FrameData {
        let mut d = FrameData::default();
        d.levels = [level; NUM_BANDS];
        d.peaks = d.levels;
        d
    }

    /// An uneven spectrum: `loud` in `band_lo..band_hi`, `quiet` everywhere else.
    ///
    /// Every test that drives all 64 bands to the SAME level is blind to the entire band->row
    /// fold and to the frequency axis's orientation - mean and max agree there, and every row
    /// gets the same colour, so a renderer that ignored the band index completely would pass.
    /// That blind spot has already shipped a static-looking family in this project once.
    fn uneven(band_lo: usize, band_hi: usize, loud: f32, quiet: f32) -> FrameData {
        let mut d = FrameData::default();
        for (i, v) in d.levels.iter_mut().enumerate() {
            *v = if i >= band_lo && i < band_hi { loud } else { quiet };
        }
        d.peaks = d.levels;
        d
    }

    fn lum(p: Rgba) -> f64 {
        0.2126 * p.r as f64 + 0.7152 * p.g as f64 + 0.0722 * p.b as f64
    }

    /// Renders `frames` frames of `d` at `w`x`h`.
    ///
    /// Note the frame counts the callers pass: this family only draws columns it has actually
    /// lived through, so a test that renders 20 frames leaves all but the rightmost 20 columns of
    /// the plot EMPTY. Four of the tests below were written that way first and passed/failed on
    /// bare panel - `the_frequency_axis_puts_bass_at_the_bottom` reported the top and bottom of
    /// the plot as byte-identical (9635 vs 9635), which was the panel, not the spectrogram.
    /// Anything sampling a column away from the right edge must run 183+ frames to fill the plot.
    fn render(t: &Theme, frames: usize, w: i32, h: i32, d: &FrameData) -> Canvas {
        let mut f = Waterfall::default();
        let mut c = Canvas::new(w, h);
        for _ in 0..frames {
            f.draw(&mut c, t, d);
        }
        c
    }

    /// Total luminance of a column of the plot, so a scrolling event can be located.
    fn col_lum(c: &Canvas, x: i32) -> f64 {
        (PLOT_TOP..(c.height() - 3)).map(|y| lum(c.get(x, y))).sum()
    }

    fn brightest_plot_col(c: &Canvas) -> i32 {
        let plot_w = c.width() - 4 - RAIL_RESERVE;
        let mut best = (0.0f64, PLOT_LEFT);
        for x in PLOT_LEFT..(PLOT_LEFT + plot_w) {
            let v = col_lum(c, x);
            if v > best.0 {
                best = (v, x);
            }
        }
        best.1
    }

    #[test]
    fn history_scrolls_left_one_column_per_frame() {
        // The property that distinguishes this family from every other one: a transient must stay
        // on screen and MOVE. A renderer that redrew only the present instant would leave the
        // brightest column pinned to the right edge forever, which is what this measures.
        let t = builtin::waterfall_heat();
        let mut f = Waterfall::default();
        let mut c = Canvas::new(190, 60);
        f.draw(&mut c, &t, &uneven(0, 10, 0.9, 0.0));
        let at_birth = brightest_plot_col(&c);
        let silence = flat(0.0);
        for _ in 0..12 {
            f.draw(&mut c, &t, &silence);
        }
        let after = brightest_plot_col(&c);
        assert_eq!(
            at_birth - after, 12,
            "a transient must scroll one column per frame: born at x={at_birth}, 12 frames later x={after}"
        );
        assert!(col_lum(&c, after) > col_lum(&c, at_birth) * 4.0, "and it must still be the brightest thing on the plot");
    }

    #[test]
    fn the_time_axis_follows_real_time_not_the_frame_count() {
        // Guards the dt_ms drive. With a frame counter, doubling the frame period would leave the
        // transient in the same place and the visible window would silently represent twice as
        // long - see the scroll note in the module docs.
        let t = builtin::waterfall_ice();
        let travel = |dt: f32| -> i32 {
            let mut f = Waterfall::default();
            let mut c = Canvas::new(190, 60);
            let mut pulse = uneven(0, 10, 0.9, 0.0);
            pulse.dt_ms = dt;
            f.draw(&mut c, &t, &pulse);
            let born = brightest_plot_col(&c);
            let mut silence = flat(0.0);
            silence.dt_ms = dt;
            for _ in 0..10 {
                f.draw(&mut c, &t, &silence);
            }
            born - brightest_plot_col(&c)
        };
        let slow = travel(COLUMN_MS);
        let fast = travel(COLUMN_MS * 2.0);
        assert_eq!(slow, 10, "one column per frame at the nominal period");
        assert!(
            (fast - 20).abs() <= 1,
            "a doubled frame period must scroll twice as far in the same number of frames: {fast} vs {slow}"
        );
    }

    #[test]
    fn a_long_stall_drops_the_backlog_instead_of_erasing_the_history() {
        // MAX_STEPS. A 2-second stall earns 120 columns; replaying them all would flush two
        // thirds of the plot with one repeated spectrum.
        let t = builtin::waterfall_heat();
        let mut f = Waterfall::default();
        let mut c = Canvas::new(190, 60);
        for _ in 0..40 {
            f.draw(&mut c, &t, &uneven(0, 10, 0.9, 0.0));
        }
        let before = f.filled;
        let mut stalled = flat(0.0);
        stalled.dt_ms = 2000.0;
        f.draw(&mut c, &t, &stalled);
        assert_eq!(f.filled - before, MAX_STEPS, "a stall must be capped, not replayed");
        assert!(f.acc.is_finite() && f.acc == 0.0, "and the backlog must be dropped, not carried");
    }

    #[test]
    fn the_frequency_axis_puts_bass_at_the_bottom() {
        // Reads the axis directly, which no all-bands-equal test can do. Both halves are driven,
        // so this cannot pass on "one half happens to be brighter".
        let t = builtin::waterfall_heat();
        let bassy = render(&t, 200, 190, 60, &uneven(0, 16, 0.8, 0.14));
        let trebly = render(&t, 200, 190, 60, &uneven(48, 64, 0.8, 0.14));
        let band = |c: &Canvas, y0: i32, y1: i32| -> f64 {
            (y0..y1).map(|y| (PLOT_LEFT..150).map(|x| lum(c.get(x, y))).sum::<f64>()).sum()
        };
        let bass_low = band(&bassy, 45, 56);
        let bass_high = band(&bassy, 4, 15);
        let treble_low = band(&trebly, 45, 56);
        let treble_high = band(&trebly, 4, 15);
        assert!(
            bass_low > bass_high * 3.0,
            "low bands must light the BOTTOM rows: bottom {bass_low:.0} vs top {bass_high:.0}"
        );
        assert!(
            treble_high > treble_low * 3.0,
            "high bands must light the TOP rows: top {treble_high:.0} vs bottom {treble_low:.0}"
        );
    }

    #[test]
    fn folding_bands_into_fewer_rows_keeps_a_single_band_peak() {
        // Fold by MAX, not mean. At h=24 the plot is 18 rows for 64 bands, so each row carries 3-4
        // bands, and a mean would dilute a lone peak to about a quarter of its level - the peak would
        // read as background, which is the one thing this family exists to show.
        //
        // THE PITCH TRACK IS WHY THIS TEST WAS VACUOUS, and it is worth spelling out because nothing
        // about the failure pointed at it. `draw` marks each column's DOMINANT band in `t.hot` at high
        // alpha, at `m * rows / NUM_BANDS` - and for a spectrum with one loud band, that is the same row
        // the loud band folds into. The original test rendered exactly that spectrum and sampled exactly
        // that row, so it was measuring the marker, which sits at full brightness whatever the fold
        // does. It read 244.8 with a max fold and 244.8 with the max replaced by a mean.
        //
        // Two intermediate versions are worth recording as dead ends, because both looked reasonable:
        //   - lowering the levels to stay inside `response`'s clamp did not help; the marker is drawn
        //     from `t.hot` and does not care about the level at all past `MARK_MIN_RESP`.
        //   - comparing the row against separate renders predicting each fold did not help either. It
        //     measured 252.8 where a max fold predicts 138.8 - HIGHER than the value it was meant to
        //     match - because a single pixel compared across two renders carries its neighbours' bloom
        //     and, here, the marker.
        //
        // So the loud band under test is NOT the dominant one. A much louder band sits far down the
        // spectrum, which parks the marker seven rows away, and the row under test then carries nothing
        // but the fold. The comparison is within one render: make ONE of the row's bands loud, or ALL of
        // them. A max returns the loudest either way, so the row must read the same; a mean returns a
        // quarter of the level in the first case and all of it in the second.
        const LOUD: f32 = 0.35;
        const QUIET: f32 = 0.13;
        // Louder than LOUD, and low in the spectrum, so it owns the pitch track and takes the marker
        // out of the row being measured.
        const DECOY_BAND: usize = 5;
        const DECOY: f32 = 0.60;

        let t = builtin::waterfall_mono();
        let x = PLOT_LEFT + (190 - 4 - RAIL_RESERVE) - 1;
        let rows: usize = 24 - 6;
        // The row band 30 folds into, and the band range it covers - the same arithmetic `draw` uses.
        let r = 30 * rows / NUM_BANDS;
        let y = PLOT_TOP + rows as i32 - 1 - r as i32;
        let (lo, hi) = (r * NUM_BANDS / rows, ((r + 1) * NUM_BANDS / rows).max(r * NUM_BANDS / rows + 1));
        assert!(hi - lo >= 3, "the fold is too shallow here to tell max from mean: {} bands", hi - lo);
        let decoy_row = DECOY_BAND * rows / NUM_BANDS;
        assert!(
            decoy_row + 2 < r,
            "the decoy's marker at row {decoy_row} is too close to the row under test at {r}"
        );

        // `loud` bands sit at LOUD, the decoy at DECOY, everything else at QUIET.
        let spectrum = |loud: std::ops::Range<usize>| -> FrameData {
            let mut d = FrameData::default();
            for (i, v) in d.levels.iter_mut().enumerate() {
                *v = if i == DECOY_BAND {
                    DECOY
                } else if loud.contains(&i) {
                    LOUD
                } else {
                    QUIET
                };
            }
            d.peaks = d.levels;
            d
        };
        let row = |d: &FrameData| lum(render(&t, 6, 190, 24, d).get(x, y));

        let one_loud = row(&spectrum(30..31));
        let all_loud = row(&spectrum(lo..hi));
        let none_loud = row(&spectrum(0..0));

        // The fixture has to be able to see the difference at all: if a loud row and a quiet row render
        // the same, the levels are outside the usable part of the ramp and nothing below means anything.
        // This is the guard whose absence let the earlier versions pass on the bug.
        assert!(
            all_loud > none_loud + 25.0,
            "loud and quiet render the same here ({all_loud:.0} against {none_loud:.0}), so this test \
             cannot tell a max fold from a mean one"
        );
        assert!(
            (one_loud - all_loud).abs() < 6.0,
            "the row folded like a MEAN: one loud band in it reads {one_loud:.0} and all {} read \
             {all_loud:.0}. Under a max fold those are the same reading",
            hi - lo
        );
    }

    #[test]
    fn neighbouring_rows_differ_under_an_uneven_spectrum() {
        // The complaint this pre-empts is the one the valve row shipped with: a family that looks
        // the same everywhere because one reduction flattened the spectrum. A comb - alternating
        // loud and quiet groups - must produce visibly banded rows.
        let t = builtin::waterfall_viridis();
        let mut d = FrameData::default();
        for (i, v) in d.levels.iter_mut().enumerate() {
            *v = if (i / 8) % 2 == 0 { 0.70 } else { 0.15 };
        }
        d.peaks = d.levels;
        let c = render(&t, 200, 190, 60, &d);
        let x = PLOT_LEFT + 40;
        let rows: Vec<f64> = (PLOT_TOP..(60 - 3)).map(|y| lum(c.get(x, y))).collect();
        let hi = rows.iter().copied().fold(0.0f64, f64::max);
        let lo = rows.iter().copied().fold(f64::MAX, f64::min);
        assert!(hi > lo + 60.0, "a comb spectrum must band the rows: {lo:.0}..{hi:.0}");
        // And the banding must repeat, not just be one bright edge: count the crossings of the
        // midpoint up the column.
        let mid = (hi + lo) / 2.0;
        let crossings = rows.windows(2).filter(|p| (p[0] > mid) != (p[1] > mid)).count();
        assert!(crossings >= 6, "expected several alternating bands, saw {crossings}");
    }

    #[test]
    fn the_rail_marker_moves_with_the_dominant_band() {
        // The position cue. Measured on the rail columns only, so it cannot pass on the plot.
        let t = builtin::waterfall_inferno();
        let rx = 190 - 2 - RAIL_RESERVE + 1;
        let marker_y = |d: &FrameData| -> i32 {
            let c = render(&t, 8, 190, 60, d);
            let mut best = (0.0f64, -1);
            for y in PLOT_TOP..(60 - 3) {
                let v = (rx..(rx + RAIL_W)).map(|x| lum(c.get(x, y))).sum::<f64>();
                if v > best.0 {
                    best = (v, y);
                }
            }
            best.1
        };
        let bass = marker_y(&uneven(1, 3, 0.85, 0.14));
        let treble = marker_y(&uneven(60, 62, 0.85, 0.14));
        assert!(bass > treble + 25, "the marker must travel with the dominant band: bass at y={bass}, treble at y={treble}");
    }

    #[test]
    fn the_pitch_track_lights_a_faint_tone_the_ramp_alone_would_barely_show() {
        // Isolates the in-plot track from the ramp, which is otherwise impossible: the dominant
        // band's cell is also the brightest cell, so measuring it proves nothing. Two renders of
        // the SAME faint tone, differing only in whether a louder bass band steals the mark away
        // from it - the difference on the tone's own row is the track and nothing else. The bass
        // sits 26 rows away, far outside the 2px bloom radius, so it cannot contribute there.
        let t = builtin::waterfall_heat();
        let faint = uneven(32, 33, 0.26, 0.0);
        let tracked = render(&t, 200, 190, 60, &faint);
        let mut stolen = faint;
        stolen.levels[2] = 0.9;
        stolen.peaks = stolen.levels;
        let untracked = render(&t, 200, 190, 60, &stolen);
        let rows = 54usize;
        let y = PLOT_TOP + rows as i32 - 1 - (32 * rows / NUM_BANDS) as i32;
        let with = lum(tracked.get(60, y));
        let without = lum(untracked.get(60, y));
        assert!(
            with > without + 60.0,
            "the track must make a faint dominant tone legible: {with:.0} with it, {without:.0} without"
        );
    }

    #[test]
    fn silence_suppresses_the_marker_instead_of_drawing_a_phantom_track() {
        // Silence still has a numerically loudest band, so without MARK_MIN_RESP the track would
        // jitter across an empty plot.
        let t = builtin::waterfall_mono();
        let rx = 190 - 2 - RAIL_RESERVE + 1;
        let brightest_rail = |c: &Canvas| -> f64 {
            (PLOT_TOP..(60 - 3))
                .flat_map(|y| (rx..(rx + RAIL_W)).map(move |x| (x, y)))
                .map(|(x, y)| lum(c.get(x, y)))
                .fold(0.0, f64::max)
        };
        let silent = brightest_rail(&render(&t, 200, 190, 60, &flat(0.0)));
        assert!(silent < 60.0, "nothing on the rail should be bright at silence, got {silent:.0}");
        // The positive control, without which this test also passes on a rail that never draws at
        // all: the same rail must light when there IS a dominant band.
        let driven = brightest_rail(&render(&t, 200, 190, 60, &uneven(20, 23, 0.8, 0.14)));
        assert!(driven > 150.0, "the rail must light when driven, got {driven:.0}");
    }

    #[test]
    fn driving_the_audio_changes_the_pixels() {
        let t = builtin::waterfall_heat();
        let quiet = render(&t, 200, 190, 60, &flat(0.0));
        let loud = render(&t, 200, 190, 60, &uneven(8, 40, 0.8, 0.2));
        let changed = quiet
            .bits()
            .iter()
            .zip(loud.bits().iter())
            .filter(|(a, b)| a != b)
            .count();
        assert!(changed > 8000, "audio must repaint most of the plot, only {changed} pixels differ");
    }

    #[test]
    fn the_response_window_spends_its_range_on_levels_the_dsp_actually_produces() {
        let lo = response(0.15, 1.0);
        let hi = response(0.65, 1.0);
        assert!(hi - lo > 0.85, "the music window must cover most of the ramp: {lo} -> {hi}");
        assert_eq!(response(0.0, 1.0), 0.0, "silence must be transparent, not a pedestal");
        assert_eq!(response(1.0, 1.0), 1.0, "full scale must reach the top of the ramp");
        assert!(response(0.3, 2.0) > response(0.3, 1.0), "sensitivity must scale it");
        assert_eq!(response(f32::NAN, 1.0), 0.0, "clamp does not sanitise NaN; this must");
    }

    #[test]
    fn the_ramp_starts_transparent_and_climbs_in_luminance() {
        // Guards both the synthesised transparent floor (without it the plot would paint an opaque
        // block over the panel and the bloom halo would have nowhere to show) and the ordering of
        // the authored stops.
        for t in builtin::all().into_iter().filter(|t| t.family == "waterfall") {
            let lut = ramp_lut(&t);
            assert_eq!(lut[0].a, 0, "{}: the bottom of the ramp must be the panel", t.id);
            let mut prev = -1.0f64;
            for (i, e) in lut.iter().enumerate() {
                // Composite over the theme's own panel, since a translucent stop's visible
                // luminance depends on it.
                let panel = Rgba::from_hex(&t.panel, 1.0);
                let a = e.a as f64 / 255.0;
                let over = |c: u8, p: u8| c as f64 * a + p as f64 * (1.0 - a);
                let l = 0.2126 * over(e.r, panel.r)
                    + 0.7152 * over(e.g, panel.g)
                    + 0.0722 * over(e.b, panel.b);
                assert!(l >= prev - 2.0, "{}: the ramp dips at step {i} ({l:.1} after {prev:.1})", t.id);
                prev = prev.max(l);
            }
            let top = lut[RAMP_STEPS - 1];
            assert!(
                lum(top) > 180.0,
                "{}: the top of the ramp must be near-white, got {:.0}",
                t.id,
                lum(top)
            );
        }
    }

    #[test]
    fn a_theme_without_zones_falls_back_to_lit_then_hot() {
        // External TOML files will not all declare zones, and a ramp that collapsed to nothing
        // would render an invisible spectrogram rather than an error.
        let mut t = builtin::waterfall_heat();
        t.zones.clear();
        let lut = ramp_lut(&t);
        assert_eq!(lut[0].a, 0);
        assert!(lut[RAMP_STEPS - 1].a > 200, "the top stop must be solid");
        let c = render(&t, 200, 190, 60, &uneven(0, 32, 0.7, 0.2));
        assert!(
            (PLOT_TOP..57).any(|y| lum(c.get(60, y)) > 80.0),
            "a zone-less theme must still paint a visible plot"
        );
    }

    #[test]
    fn renders_at_every_plausible_size_without_panicking() {
        let sizes = [
            (190, 60),
            (380, 60),
            (456, 60),
            (240, 72),
            (150, 48),
            (96, 40),
            (40, 24),
            (14, 12),
            (12, 12),
            (3, 3),
            (1, 1),
        ];
        let t = builtin::waterfall_ice();
        for (w, h) in sizes {
            let mut f = Waterfall::default();
            let mut c = Canvas::new(w, h);
            // Several frames: the ring buffer is allocated on the first one and reused after.
            for _ in 0..4 {
                f.draw(&mut c, &t, &uneven(4, 28, 0.7, 0.2));
            }
            assert_eq!(c.bits().len(), (w * h) as usize, "{w}x{h} changed the canvas size");
        }
    }

    #[test]
    fn a_resize_keeps_the_history_rather_than_dropping_it() {
        // The history is stored in BAND space, not pixel space, precisely so the overlay's rect
        // moving (it resizes as the weather text changes) does not wipe three seconds of plot.
        let t = builtin::waterfall_heat();
        let mut f = Waterfall::default();
        let mut narrow = Canvas::new(190, 60);
        for _ in 0..30 {
            f.draw(&mut narrow, &t, &uneven(0, 12, 0.85, 0.1));
        }
        let filled = f.filled;
        let mut wide = Canvas::new(380, 60);
        f.draw(&mut wide, &t, &flat(0.0));
        assert_eq!(f.filled, filled + 1, "a resize must not reset the ring buffer");
        // And the old columns must be visible on the wider canvas.
        let lit = (PLOT_LEFT..(380 - 6)).filter(|&x| col_lum(&wide, x) > 200.0).count();
        assert!(lit > 20, "the retained history should still be drawn, only {lit} lit columns");
    }

    #[test]
    fn non_finite_input_never_reaches_the_history_or_the_clock() {
        let t = builtin::waterfall_heat();
        let mut f = Waterfall::default();
        let mut c = Canvas::new(190, 60);
        let mut bad = uneven(0, 32, 0.7, 0.2);
        bad.levels[0] = f32::NAN;
        bad.levels[9] = f32::INFINITY;
        bad.levels[63] = f32::NEG_INFINITY;
        bad.peaks[3] = f32::NAN;
        bad.dt_ms = f32::NAN;
        for _ in 0..5 {
            f.draw(&mut c, &t, &bad);
        }
        assert!(f.acc.is_finite(), "a NaN dt must not poison the column clock, got {}", f.acc);
        assert!(f.hist.iter().all(|v| v.is_finite()), "a NaN level reached the history");
        assert!(f.mark_lvl.iter().all(|v| v.is_finite()), "a NaN level reached the marker");
        // A NaN dt must still advance the plot - freezing is the failure mode this guards.
        assert!(f.filled >= 5, "the waterfall stalled on a NaN dt: only {} columns", f.filled);

        // And a recovery: normal frames after the poisoned ones must still paint.
        let good = uneven(0, 32, 0.7, 0.2);
        for _ in 0..200 {
            f.draw(&mut c, &t, &good);
        }
        assert!((PLOT_TOP..57).any(|y| lum(c.get(100, y)) > 80.0), "the plot never recovered");

        // Infinity in dt too, plus a zero-length frame.
        let mut spoiled = flat(0.3);
        spoiled.dt_ms = f32::INFINITY;
        f.draw(&mut c, &t, &spoiled);
        spoiled.dt_ms = 0.0;
        f.draw(&mut c, &t, &spoiled);
        assert!(f.acc.is_finite());
    }

    #[test]
    fn nothing_is_drawn_outside_the_panel_rect() {
        // The bloom spreads past the panel's 1-2px margin, so without the clip the halo lands on
        // the bare taskbar as a bright box. Checked with the plot fully lit, where it is worst.
        let c = render(&builtin::waterfall_heat(), 200, 190, 60, &flat(0.95));
        for x in 0..190 {
            for y in [0, 1, 58, 59] {
                assert_eq!(c.get(x, y), Rgba::TRANSPARENT, "({x},{y}) is outside the panel");
            }
        }
        for y in 0..60 {
            assert_eq!(c.get(0, y), Rgba::TRANSPARENT, "column 0 is left of the panel");
            assert_eq!(c.get(189, y), Rgba::TRANSPARENT, "column 189 is right of the panel");
        }
    }

    #[test]
    fn every_waterfall_colourway_renders_and_differs() {
        let mut seen: Vec<Vec<u32>> = Vec::new();
        let mut n = 0;
        for t in builtin::all().into_iter().filter(|t| t.family == "waterfall") {
            let c = render(&t, 200, 190, 60, &uneven(6, 30, 0.75, 0.18));
            let bits = c.bits().to_vec();
            assert!(bits.iter().any(|p| *p != 0), "{} rendered nothing", t.id);
            for prior in &seen {
                assert_ne!(prior, &bits, "{} renders identically to another colourway", t.id);
            }
            seen.push(bits);
            n += 1;
        }
        assert_eq!(n, 5, "expected the five shipped waterfall colourways, got {n}");
    }

    /// Measurement, not an assertion - the numbers quoted in the constants' doc comments come from
    /// here. Run: cargo test --release measure_waterfall -- --ignored --nocapture
    #[test]
    #[ignore]
    fn measure_waterfall() {
        let t = builtin::waterfall_heat();
        let panel = Rgba::from_hex(&t.panel, 1.0);
        println!("panel luminance {:.1}", lum(panel));
        let lut = ramp_lut(&t);
        println!("step  a lum(over panel)  delta");
        let mut prev = 0.0;
        for i in 0..RAMP_STEPS {
            let e = lut[i];
            let a = e.a as f64 / 255.0;
            let over = |c: u8, p: u8| c as f64 * a + p as f64 * (1.0 - a);
            let l = 0.2126 * over(e.r, panel.r) + 0.7152 * over(e.g, panel.g) + 0.0722 * over(e.b, panel.b);
            if i % 4 == 0 || i == RAMP_STEPS - 1 {
                println!("{i:4}  {:3}  {l:15.1}  {:5.1}", e.a, l - prev);
            }
            prev = l;
        }
        // max vs mean fold, at the small height where it bites hardest
        let x = PLOT_LEFT + (190 - 4 - RAIL_RESERVE) - 1;
        let rows: usize = 18;
        let y = PLOT_TOP + rows as i32 - 1 - (30 * rows / NUM_BANDS) as i32;
        let peak = render(&t, 6, 190, 24, &uneven(30, 31, 0.75, 0.14));
        let full = render(&t, 6, 190, 24, &flat(0.75));
        let mean = (0.75 + 3.0 * 0.14) / 4.0;
        println!(
            "fold at h=24: single band max -> {:.0}, all-loud -> {:.0}, the mean a 4-band row \
             would see is {mean:.2} (response {:.2})",
            lum(peak.get(x, y)),
            lum(full.get(x, y)),
            response(mean, 1.0)
        );
        let mean_equiv = render(&t, 6, 190, 24, &flat(mean));
        println!("  a mean fold would render that row at {:.0}", lum(mean_equiv.get(x, y)));
    }

    /// Run: cargo test --release dump_waterfall_frames -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_waterfall_frames() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();

        // A synthetic minute of music: a formant sweeping up and down, a bass thump every 24
        // frames, over a moving noise floor. A static spectrum would dump 183 identical columns
        // and show nothing about the family that a single bar graph would not.
        let feed = |frame: usize| -> FrameData {
            let mut d = FrameData::default();
            let phase = frame as f32 / 46.0 * std::f32::consts::TAU;
            let centre = 0.45 + 0.32 * phase.sin();
            for (i, v) in d.levels.iter_mut().enumerate() {
                let x = i as f32 / (NUM_BANDS - 1) as f32;
                let tone = (-((x - centre) * (x - centre)) / (2.0 * 0.055 * 0.055)).exp();
                let thump = if frame % 24 < 3 { (1.0 - x * 7.0).max(0.0) } else { 0.0 };
                let floor = 0.13 + 0.05 * ((i as f32 * 1.7 + frame as f32 * 0.31).sin() * 0.5 + 0.5);
                *v = (floor + 0.55 * tone + 0.45 * thump).min(1.0);
            }
            d.peaks = d.levels;
            d
        };

        let dump = |c: &Canvas, name: String| {
            let (w, h) = (c.width(), c.height());
            let mut out = Vec::with_capacity((w * h * 4) as usize);
            for y in 0..h {
                for x in 0..w {
                    let px = c.get(x, y);
                    let a = px.a as f32 / 255.0;
                    // Composited over taskbar grey 22, exactly as the other dump harnesses do.
                    for ch in [px.r, px.g, px.b] {
                        out.push((ch as f32 + 22.0 * (1.0 - a)).min(255.0) as u8);
                    }
                    out.push(255);
                }
            }
            std::fs::write(dir.join(name), &out).unwrap();
        };

        let mut n = 0usize;
        for t in builtin::all().into_iter().filter(|t| t.family == "waterfall") {
            let mut f = Waterfall::default();
            let mut c = Canvas::new(190, 60);
            for frame in 0..220 {
                f.draw(&mut c, &t, &feed(frame));
            }
            dump(&c, format!("waterfall-{}.rgba", t.id));
            n += 1;
        }
        // One wide dump too: 372 columns need 372 frames to fill, and the wide mode is the case
        // most likely to expose a geometry assumption baked in at 190px.
        for id in ["waterfall-heat", "waterfall-inferno"] {
            let t = builtin::all().into_iter().find(|t| t.id == id).unwrap();
            let mut f = Waterfall::default();
            let mut c = Canvas::new(380, 60);
            for frame in 0..420 {
                f.draw(&mut c, &t, &feed(frame));
            }
            dump(&c, format!("waterfall-wide-{id}.rgba"));
            n += 1;
        }
        println!("wrote {n} waterfall dumps to {}", dir.display());
    }
    #[test]
    fn the_flourish_tears_one_full_height_column_and_it_then_scrolls_away() {
        // A tear is a hard vertical rip through the whole spectrum: one column at full scale across
        // every band. Two properties, and the second is what makes it this family's flourish rather
        // than a generic flash - it is written into the HISTORY, so it survives as data and scrolls.
        let seq = crate::dsp::flourish::firing_sequence(NUM_BANDS);
        let run = |flourish: f32, after: usize| -> Canvas {
            let mut t = builtin::waterfall_heat();
            t.flourish = flourish;
            let mut f = Waterfall::default();
            let mut c = Canvas::new(190, 60);
            for row in &seq {
                let mut d = FrameData { dt_ms: 16.7, ..FrameData::default() };
                for (i, v) in d.levels.iter_mut().enumerate() {
                    *v = row.get(i).copied().unwrap_or(0.0);
                }
                d.peaks = d.levels;
                f.draw(&mut c, &t, &d);
            }
            // Quiet afterwards, so a bright column can only be the tear.
            for _ in 0..after {
                f.draw(&mut c, &t, &FrameData { dt_ms: 16.7, ..FrameData::default() });
            }
            c
        };
        // The BRIGHTEST column in the plot, summed down its height.
        //
        // Counting lit ROWS was the first attempt and it cannot discriminate: the fixture's audio spans
        // every band too, so it lights every row of its own column and the measurement read 54 against
        // 54. A tear is written at FULL scale where the audio is two thirds of the way up the response
        // curve, so what separates them is brightness per column, not extent.
        let brightest = |c: &Canvas| -> u32 {
            let mut best = 0u32;
            for x in 0..190 {
                let sum: u32 = (0..60)
                    .map(|y| {
                        let p = c.get(x, y);
                        p.r as u32 + p.g as u32 + p.b as u32
                    })
                    .sum();
                best = best.max(sum);
            }
            best
        };
        let on = run(crate::themes::DEFAULT_FLOURISH, 6);
        let off = run(0.0, 6);
        assert_ne!(on.bits(), off.bits(), "the tear changed nothing");
        let (a, b) = (brightest(&on), brightest(&off));
        assert!(
            a > b + b / 4,
            "the tear should be a much brighter column than the audio: {a} against {b}"
        );

        // AND IT SCROLLS. The column is in the ring buffer, so a second later it is still on screen -
        // just further left. This is the property that distinguishes writing history from drawing a
        // flash, and a flash would pass every assertion above.
        let later = run(crate::themes::DEFAULT_FLOURISH, 90);
        assert!(
            brightest(&later) > b + b / 4,
            "the tear vanished instead of scrolling: brightest column {} a second and a half later",
            brightest(&later)
        );
        // Eventually it does leave, or it would be a permanent stripe.
        let much_later = run(crate::themes::DEFAULT_FLOURISH, 1200);
        assert!(
            brightest(&much_later) <= b + b / 4,
            "the tear never scrolled off: brightest column still {}",
            brightest(&much_later)
        );
    }

}
