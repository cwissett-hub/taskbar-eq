//! The radar family: a rectangular search window whose sweep paints the spectrum as blips and
//! leaves a phosphor wake behind it.
//!
//! Every other family in this project redraws the whole picture every frame. This one does not,
//! and that is the entire point of it: a search display only knows what the beam has already
//! illuminated, so the picture is built up one column at a time and what you are looking at is a
//! second and a half of history rather than an instant. Nothing else here shows time at all.
//!
//! **It used to be a PPI - a 180-degree fan squashed onto an ellipse - and the fan is gone.** The
//! panel is 190x60. The largest circle that clears the bezel has a radius of about 26px and would
//! leave two thirds of the width empty, so the fan was squashed to 1.9:1 to fill the box, and that
//! cost three things at once: the corners carried nothing, adjacent bearings bunched to 5.9px near
//! the horizons while sitting 9px apart mid-arc, and the range rings flattened into arcs that no
//! longer read as range at all. Worse, a fan has ends, and sweeping to an end forced a choice
//! between a hard reset (which lurched) and an oscillation (which reverses the age gradient - see
//! the sweep note in `draw`). A rectangular search field - a B-scan, which is what real hardware
//! with a limited arc actually presents - fits a 190x60 box exactly, spaces every column alike,
//! gives range a straight vertical axis, and has a natural flyback.
//!
//! Three decisions carry it, and each was made against a measured failure:
//!
//! - **The blip's RANGE is the audio cue, not its brightness.** A louder band pushes its blip
//!   further UP its own column. Brightness rides along, but the valve row already proved that
//!   brightness alone gives about a 1.16x spread between a driven element and its neighbour -
//!   below the visible threshold at this size - while a mark that MOVES is resolved instantly.
//!   See `RANGE_MIN`/`RANGE_MAX`.
//! - **Up is further out**, so a loud band is a high blip. A B-scan can legitimately run range
//!   downward, but every other family here (and every level meter ever built) makes loud = tall,
//!   and a display that inverted that convention alone on the taskbar would be misread.
//! - **The wake is tied to the sweep period.** A column decays to `WAKE_FLOOR` over exactly one
//!   pass, so the returns immediately behind the sweep are the brightest and the ones just ahead
//!   of it - painted a whole pass ago - are the faintest. Decay faster than that leaves a bare
//!   line with no picture; slower and every column sits at the same brightness and the display
//!   stops reading as a scan at all. The rectangle made this exact rather than approximate: with
//!   the oscillating fan a column near a limit was re-swept twice in quick succession at the
//!   turnaround, so its wake never reached the floor, while a mid-arc column did.
//!
//! Bloom, as everywhere else here: the sweep and the blips are built on their OWN transparent
//! layer, bloomed, and composited over the opaque panel, because `Canvas::bloom` puts its halo
//! UNDERNEATH existing content and the panel would hide it completely. The grid is deliberately
//! NOT on that layer - see the note where it is drawn.

use super::canvas::{Canvas, Rgba};
use super::{Family, FrameData};
use crate::themes::Theme;

/// Columns the field is divided into at the reference panel width - the display's bearing
/// resolution.
///
/// 32 rather than 64 (one per band) on purpose, and the number survives the move from the fan
/// because the pitch it produces does: 32 columns across the 184px field are 5.75px apart, where
/// the fan's bearings were 5.9px apart at their tightest (bunched near the horizons by the ellipse
/// squash). At 64 they would be 2.9px apart, which against a blip 4.8px wide merges neighbouring
/// returns into one continuous trace and throws away the one thing this display is for. It also
/// means each column reduces a GROUP of bands, which is what makes `GROUP_MAX_BIAS` do any work.
const CELLS: usize = 32;

/// Column pitch the display is tuned at: the 184px field of a 190x60 panel divided by `CELLS`.
///
/// This, not the column count, is what has to stay put across widths - it is what sets the blip
/// width and the gap between neighbouring returns.
const COL_PITCH: f32 = 5.75;

/// Columns to divide the field into at a given panel width.
///
/// Extra width buys extra COLUMNS, not fatter ones - the same rule the valve row and the VU dials
/// follow, and the rectangle makes it trivial where the fan could not. The fan had to answer extra
/// width by adding whole second and third faces, because a single face stretched to 380px reached
/// a 3.9:1 squash and read as a lens rather than as a squashed circle; that left a seam down the
/// middle, two sweep lines moving in lockstep, and a frequency axis that restarted half way
/// across. One field with more columns has none of those problems: the axis stays monotonic left to
/// right and there is one beam.
///
/// Derived from the pitch rather than stepped per whole panel-width, which is what the fan's face
/// count did - and it left the intermediate sizes stretched: at 240px (150% DPI, a size the
/// dispatch suite actually renders) one face still covered the whole panel and every column grew to
/// 7.3px. Solving for the pitch instead keeps it within 0.1px of 5.75 at every plausible size.
///
/// Capped at four times the reference count so the persistence buffers stay bounded; the cap holds
/// the pitch out to about 740px, well past the widest overlay rect ever observed.
///
/// Takes the height and the warning-receiver flag because the scope eats width the field would
/// otherwise have had: with the RWR on at 190px the field keeps 24 columns rather than 32. The PITCH
/// is what must not move, and it does not - see the width test.
fn column_count(w: i32, h: i32, rwr: bool) -> usize {
    ((field_w(w, h, rwr) as f32 / COL_PITCH).round() as usize).clamp(8, CELLS * 4)
}

/// Milliseconds for the sweep to cross the whole field.
///
/// Real surveillance sets run 2-4s per revolution. 1400ms is deliberately faster than that: at 4s
/// the display looked broken rather than slow, because at a 60px panel there is no detail to
/// reward waiting for, and a full picture has to be up within about a second and a half of the
/// music starting or the family reads as unresponsive.
const SWEEP_MS: f32 = 1400.0;

/// Band level at which a column starts to return an echo, and the span it fills over.
///
/// Copied deliberately from `tube.rs`, and for the same measured reason: `FrameData.levels` only
/// reaches about 0.15-0.65 on real music, so anything mapping 0..1 linearly spends two thirds of
/// its travel on levels that never arrive. Mapping the window the DSP actually produces onto the
/// full range is what makes the blips move on quiet passages.
const RESP_FLOOR: f32 = 0.10;
const RESP_SPAN: f32 = 0.52;

/// Weight given to a column's LOUDEST band rather than its mean.
///
/// Two bands feed one column at the reference size. A plain mean of the pair halves the travel of
/// a single-band peak, which is exactly the event that should show up as one blip jumping above
/// its neighbours - the thing that makes the picture look like a spectrum rather than a flat line.
/// Same value and same reasoning as the valve row, where the mean measured 1.46 dL* between a
/// driven element and its neighbour against 9.47 dL* for the max-biased blend.
const GROUP_MAX_BIAS: f32 = 0.65;

/// Range a silent column's blip sits at, and the range a full-scale one reaches, as a fraction of
/// the field height.
///
/// The audio-driven POSITION cue. Not 0.0 at the bottom: a return painted on the datum line is
/// indistinguishable from the datum, and close-in clutter is what a real search display shows for
/// a bearing with nothing on it. Not 1.0 at the top either - the top of the field is a printed
/// range line, and a blip landing exactly on it reads as a break in the line rather than as a
/// maximum.
const RANGE_MIN: f32 = 0.20;
const RANGE_MAX: f32 = 0.96;

/// Brightness a freshly painted blip keeps regardless of level.
///
/// Applied ONCE, on paint, and only to the blip's own alpha - the lesson `HEATER_FLOOR` in tube.rs
/// records is that a floor folded into a term that is then floored again eats most of the usable
/// range before any audio arrives. Here it is affordable precisely BECAUSE brightness is the
/// secondary cue: range gets its full travel, and this only guarantees the blip is visible enough
/// for its position to be read.
const BLIP_FLOOR: f32 = 0.18;

/// What a column's echo has decayed to by the time the sweep comes round to it again.
///
/// 0.12 rather than 0: a column that reaches zero leaves a band of empty screen ahead of the sweep
/// and the display reads as a single travelling line with nothing on it. Keeping the oldest
/// returns just visible is what makes one glance show a whole pass.
const WAKE_FLOOR: f32 = 0.12;

/// Length of the sweep's own afterglow, in columns, and the alpha its freshest wash carries.
///
/// 4.5 columns is 26px at the reference size. The per-column wake above is the AUDIO history; this
/// is just the beam looking like a beam.
///
/// The alpha went UP, from the fan's 0.16 to 0.30, and that is the whole reason this is documented.
/// The fan's trail was 24 lines all radiating from one centre, so they piled up 2-3 deep per pixel
/// near the middle and had to be kept faint or the wedge composited into a solid slab that erased
/// every blip it swept over; out at the rim the same 24 lines were 1.7px apart and the trail was
/// barely there. A rectangular trail steps in x directly - exactly one line per pixel column, no
/// overlap anywhere - so the compensation is not needed and carrying it over leaves the wash
/// competing with the print instead of sitting above it. Measured on radar-p1 at 190x60, a third of
/// the way along the trail (8px behind the sweep): 41 luminance at 0.16 against the printed range
/// lines' own 34, versus 66 at 0.30. The brightest print on the panel is the datum at 47 and bare
/// panel is 8.
///
/// Kept a wash rather than a hard-edged block: it is `t.lit`, not `t.hot`, and it squares its fade,
/// so the trail is bright for its first few columns and gone by the last few.
const TRAIL_CELLS: f32 = 4.5;
const TRAIL_ALPHA: f32 = 0.30;

/// Rise in the low bands that fires a close-in contact, and how long it lives.
///
/// Measured against a rise DETECTOR rather than an absolute level, because bass sits high on
/// almost all music - an absolute threshold fires continuously and the extra contact becomes a
/// permanent fixture instead of an event. 300ms is about two beats' gap at 120bpm, so successive
/// kicks re-fire it rather than merging.
const BASS_RISE: f32 = 0.055;
const BASS_FALL_MS: f32 = 300.0;
/// Below `RANGE_MIN`, so the transient contact always sits under the band blips rather than
/// among them.
const HIT_RANGE: f32 = 0.14;

/// Bands the low-band onset detector watches, and how fast its average follows.
///
/// 8 bands and 0.22 per frame, unchanged from when this family owned the detector itself - the code
/// moved to `dsp::onset`, the tuning did not.
const LOW_BANDS_WATCHED: usize = 8;
const BASS_EASE: f32 = 0.22;

/// Bands the low-energy centroid is measured over. See `low_centroid`.
#[cfg(test)]
const LOW_BANDS: usize = 12;

/// Blip half-width as a fraction of the column pitch.
///
/// 0.42 of 5.75px is 2.4px, which leaves a 1px gap between the returns of two neighbouring
/// columns. The fan's blip grew with level (`1.1 + 1.7 * e`) because a fan has no fixed column to
/// live in; here a level-driven width is actively harmful - it would weld a run of loud
/// neighbours into one continuous band, which is precisely the failure the fan's blip radius was
/// cut down to avoid. Level drives the HEIGHT of the mark and only the height.
const BLIP_RX_FRAC: f32 = 0.42;

/// Nominal spacing of the printed bearing lines, in pixels.
///
/// 23px is four columns at the reference size, which is eight divisions across the field - enough
/// for a column to be located against the grid without the print ever being closer together than
/// the returns it sits behind.
const GRID_PX: f32 = 23.0;

/// Range fractions the printed range lines sit at.
///
/// Shared with the tests, which have to skip these rows when probing a column for its blip: print
/// at 0.20 alpha of `lit` genuinely can out-shine a return that has decayed to `WAKE_FLOOR`, and
/// an unmasked probe reported the middle range line's position instead of the blip's.
const RANGE_LINES: [f32; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];

/// The rectangular search field, in pixels: `x`/`y` is its top-left, `y + h` is the datum row.
///
/// Note `h` is a SPAN, not a row count - the field occupies rows `y ..= y + h`, so range 0.0 lands
/// exactly on the datum and range 1.0 exactly on the top line.
#[derive(Clone, Copy)]
struct Field {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

/// The whole panel interior the display has to divide up, before the scope takes its share.
fn usable_w(w: i32) -> i32 {
    (w - 6).max(1)
}

/// Field height. The scope is sized from this, so it has to be available before the field exists.
fn field_h(h: i32) -> i32 {
    (h - 10).max(2)
}

/// Width the warning receiver takes, including its gap to the field.
///
/// 0 when the colourway has it off, and also 0 when the panel is too narrow or too short for it -
/// `rwr::width_for` gives up rather than starving the spectrum, so no width is special-cased here.
fn scope_w(w: i32, h: i32, rwr: bool) -> i32 {
    if !rwr {
        return 0;
    }
    super::rwr::width_for(usable_w(w), h, COL_PITCH)
}

/// Field width alone, because `column_count` needs it before there is a `Field` to speak of.
fn field_w(w: i32, h: i32, rwr: bool) -> i32 {
    (usable_w(w) - scope_w(w, h, rwr)).max(1)
}

/// The field for a given panel, inset from the panel's own rounded rect.
///
/// 3px in at the sides and 3 rows below the top bezel, which is what it takes for the grid's left
/// and right frame lines (drawn one pixel further out again) to clear the rounded corners at
/// radius 4. Two rows are left below the datum so the datum reads as a baseline with the panel
/// under it rather than as the bottom bezel.
///
/// The left inset grows by the scope's width when the warning receiver is on: the sweep field gives
/// up its left end, keeping its own pitch and its own full height. Shrinking the field's HEIGHT to
/// make room instead was the alternative and it is worse twice over - range is the sweep field's
/// audio axis, so shortening it costs the display's resolution rather than its extent, and a scope
/// wide enough to matter would have had to be an ellipse.
fn field(w: i32, h: i32, rwr: bool) -> Field {
    Field { x: 3 + scope_w(w, h, rwr), y: 5, w: field_w(w, h, rwr), h: field_h(h) }
}

/// Pixel column for a (possibly fractional) position in column units.
///
/// Integers land on column BOUNDARIES, so `col_x(f, cols, 0.0)` is the field's left edge and
/// `col_x(f, cols, cols)` its right edge. A blip therefore passes `ci + 0.5` to sit in the middle
/// of its own column, while the sweep passes its raw position - which means the sweep line is at
/// the left edge of the column it is painting and reaches the centre half a column later. That is
/// the correct way round: the beam enters a column, paints it, and moves on.
///
/// The spectrum is mapped low-to-high left-to-right, so bass sits at the left exactly as it does
/// in every other family here.
fn col_x(f: Field, cols: usize, cell_pos: f32) -> i32 {
    let cols = cols.max(1);
    // Clamped, not merely NaN-guarded: `hit_pos` survives a resize that changes the column count,
    // so a position sampled on a 64-column field can be read back on a 32-column one.
    let p = if cell_pos.is_finite() { cell_pos.clamp(0.0, cols as f32) } else { 0.0 };
    f.x + (p * f.w as f32 / cols as f32).round() as i32
}

/// Pixel row for a range in 0..1, measured UP from the datum - see the module docs on why up is
/// further out.
fn range_y(f: Field, range: f32) -> i32 {
    let r = if range.is_finite() { range.clamp(0.0, 1.0) } else { 0.0 };
    f.y + f.h - (r * f.h as f32).round() as i32
}

/// Pixel for a (column, range) pair.
fn plot(f: Field, cols: usize, cell_pos: f32, range: f32) -> (i32, i32) {
    (col_x(f, cols, cell_pos), range_y(f, range))
}

#[derive(Default)]
pub struct Radar {
    /// Monotonic sweep phase, 0..1 over one there-and-back cycle.
    ///
    /// The position is DERIVED from this rather than accumulated into directly, because a beam that
    /// has to reverse cannot be an accumulator that wraps - which is exactly what the previous version
    /// was, and why it flew back instead of returning.
    phase: f32,
    /// +1 travelling right, -1 travelling left. The wake trails BEHIND the beam, so it has to know
    /// which way behind is.
    dir: f32,
    /// Sweep position, in column units, 0..cols.
    ///
    /// Accumulated directly and wrapped. The fan derived this from a monotonic `phase` and carried
    /// a `dir` flag, because a beam that has to reverse smoothly cannot be an accumulator that
    /// wraps; both are gone with the fan, and the wrap is argued for where it happens in `draw`.
    pos: f32,
    /// Per-column echo strength as painted - the value `range` derives from. Held separately from
    /// `glow` because a fading return must keep its HEIGHT and only lose brightness: deriving the
    /// height from the decaying value instead made an old loud return sink until it was
    /// indistinguishable from a fresh quiet one, which destroys the only reason to keep history on
    /// screen.
    echo: Vec<f32>,
    /// Per-column brightness, decaying with the phosphor.
    glow: Vec<f32>,
    /// Slew-limited low-band level, for the transient detector to measure a rise against.
    /// The shared low-band onset detector - see `dsp::onset`. It reports the MAGNITUDE of the rise,
    /// which the warning receiver needs to judge whether a hit is exceptional for the material.
    bass: crate::dsp::onset::BassRise,
    /// Live transient contact: strength, and the column it was fired on. It stays where it was
    /// fired rather than following the sweep - a return does not move because the antenna did.
    hit: f32,
    hit_pos: f32,
    /// The warning receiver at the left of the panel. Fed from the same transient detector as `hit`,
    /// which is the point: one onset opinion, two displays reading it differently.
    pub(super) rwr: super::rwr::Rwr,
    /// Last frame's rise above the slew-limited bass average, so a probe can measure on real music
    /// what the detector actually reports rather than re-deriving it and calibrating against a copy.
    #[cfg(test)]
    last_excess: f32,
    /// Companion to `last_excess`: where the low energy sat, and which single band led it. Both are
    /// recorded so a probe can ask which of the two makes a better bearing cue on real music instead
    /// of assuming, which is how the bearing ended up stuck in one quadrant.
    #[cfg(test)]
    last_centroid: f32,
    #[cfg(test)]
    last_argmax: usize,
}

impl Radar {
    /// Echo strength for one column.
    fn cell_level(d: &FrameData, ci: usize, cols: usize) -> f32 {
        let n = d.levels.len();
        let cols = cols.max(1);
        let lo = (ci * n / cols).min(n);
        let hi = (((ci + 1) * n / cols).max(lo + 1)).min(n);
        if lo >= hi {
            return 0.0;
        }
        let mut acc = 0.0;
        let mut cnt = 0.0;
        let mut peak = 0.0f32;
        for v in &d.levels[lo..hi] {
            // is_finite BEFORE anything else: f32::clamp returns NaN unchanged (every comparison
            // with NaN is false), so a single poisoned band would otherwise reach `echo`/`glow`
            // and stay there for the life of the process - the display would lose that column
            // permanently, with no way to recover it.
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

    /// Maps a column level onto the usable 0..1 echo range, scaled by the theme's sensitivity.
    fn response(level: f32, sensitivity: f32) -> f32 {
        if !level.is_finite() {
            return 0.0;
        }
        (((level - RESP_FLOOR) / RESP_SPAN) * sensitivity.max(0.0)).clamp(0.0, 1.0)
    }

    /// Where in the low region the energy sits, 0..1.
    ///
    /// **Nothing shipped reads this any more, and that is the point of keeping it.** It WAS the warning
    /// receiver's bearing and designator cue, and `probe_rwr_rates` measures it to record why it is
    /// not: on real music the value spans 0.527..0.604 at contact moments - 8% of a circle - which put
    /// every contact in one quadrant and made the designator flicker across a quantisation boundary.
    /// Deleting it would delete the evidence, and the next person to reach for "key the bearing to the
    /// spectrum" should be able to re-run the measurement rather than re-ship the bug.
    ///
    /// A level-weighted centroid of the lowest twelve bands, which is four more than the detector
    /// itself watches. Deliberately: the detector only has to decide THAT an onset happened, and it
    /// uses the peak of the lowest eight for that, while the bearing has to say something about the
    /// SHAPE of the low end - and over eight bands a 40Hz kick and an 80Hz one land close enough
    /// together that the scope showed one bearing for everything. Twelve bands spread them.
    ///
    /// Returns the midpoint when there is nothing to weigh, so silence points a contact dead ahead
    /// rather than snapping it to a limit. Nothing spawns at silence anyway; this is only so the
    /// value is never a lie.
    #[cfg(test)]
    fn low_centroid(d: &FrameData) -> f32 {
        let hi = LOW_BANDS.min(d.levels.len());
        if hi < 2 {
            return 0.5;
        }
        let mut num = 0.0f32;
        let mut den = 0.0f32;
        for (i, v) in d.levels[..hi].iter().enumerate() {
            if v.is_finite() && *v > 0.0 {
                num += i as f32 * *v;
                den += *v;
            }
        }
        if den <= 0.0 {
            return 0.5;
        }
        (num / den / (hi - 1) as f32).clamp(0.0, 1.0)
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
            // Below this there is no search field worth drawing: the field height collapses to a
            // couple of rows, so the datum, the range lines and every blip land on the same two
            // rows and the display would be an unreadable smear rather than a small radar. Fill
            // the panel and stop, exactly as the valve row does.
            c.rounded_rect(1, 2, (w - 2).max(1), (h - 4).max(1), 3, panel);
            return;
        }
        c.rounded_rect(1, 2, w - 2, h - 4, 4, panel);

        // The warning receiver is a per-colourway element, and everything downstream of it is
        // geometry: whether it is on decides where the sweep field starts and therefore how many
        // columns the spectrum gets. Resolved once, here, so the two can never disagree.
        let use_rwr = t.radar.rwr;
        let f = field(w, h, use_rwr);
        let cols = column_count(w, h, use_rwr);
        let col_w = f.w as f32 / cols as f32;
        // Both conditions are load-bearing. `scope()` only refuses when the panel is too SHORT for a
        // circle, while `scope_w` also refuses when the field would be left too narrow - and it is
        // `scope_w` that decided where the field starts. Dropping the width check here would draw the
        // scope at x = 3 on a narrow panel where the field also starts at x = 3, i.e. on top of it.
        let scope = if use_rwr && scope_w(w, h, true) > 0 {
            super::rwr::scope(3, h)
        } else {
            None
        };

        // ---- timing ----
        //
        // dt clamped, not merely NaN-guarded. A stalled render loop (the overlay is repainted from
        // the message pump, so a drag or a taskbar reflow can hand over a 500ms frame) would
        // otherwise jump the sweep most of the way across the field in one step, painting a stripe
        // of identical returns across columns that never got sampled.
        let dt = if d.dt_ms.is_finite() { d.dt_ms.clamp(0.0, 120.0) } else { 16.7 };
        if !self.pos.is_finite() {
            self.pos = 0.0;
        }

        if self.glow.len() != cols {
            // Resizing wipes the picture, and that is correct rather than merely convenient: the
            // column count changes which bands map to which column, so the stored returns would be
            // plotted against a spectrum they were never sampled from. A resize that does NOT
            // change the count (190x60 -> 190x62) leaves the picture intact.
            self.glow = vec![0.0; cols];
            self.echo = vec![0.0; cols];
            // The contact carries a column index too, and it means a different frequency now.
            self.hit = 0.0;
        }

        // Phosphor decay. `fade` is an existing theme field - `scope.rs` already repurposes it as
        // persistence rather than as a theme cross-fade - so the wake is TOML-tunable without
        // adding to the schema. At the default 0.30 a column reaches WAKE_FLOOR after exactly one
        // pass, and with the wrap that is now true of EVERY column rather than only the mid-field
        // ones (the oscillating fan re-swept a column near a limit twice in quick succession).
        let persist = if t.fade.is_finite() { t.fade.clamp(0.05, 1.5) } else { 0.30 };
        let tau = (SWEEP_MS * (persist / 0.30) / -WAKE_FLOOR.ln()).max(1.0);
        let keep = (-dt / tau).exp();
        for g in self.glow.iter_mut() {
            *g *= keep;
        }

        // ---- advance the sweep, painting every column it crossed ----
        //
        // Every column, not just the one it landed on: at 43.75ms per column and a 16.7ms frame
        // the sweep usually stays put, but a slow frame crosses two or three and skipping them
        // leaves permanent dead columns - the picture would develop holes that only a resize could
        // clear.
        //
        // THE SWEEP WRAPS: left to right, then a flyback to the left edge. It does not oscillate,
        // and that is a deliberate reversal of the choice the fan made.
        //
        // The fan oscillated with raised-cosine easing because a hard reset lurched: its beam is a
        // RADIUS, so the brightest object on screen - the beam and its wedge - is anchored to the
        // same centre at every bearing, and flipping an object 180 degrees while it stays in place
        // reads as the whole picture jumping rather than as the beam moving. A translating line has
        // no such anchor. It leaves at the right edge and reappears at the left, which is the most
        // familiar scan motion there is (every CRT, every chart recorder, every DAW playhead) and
        // is read as a wrap, not as a jolt.
        //
        // The rectangle also makes oscillation actively wrong here, which the fan did not suffer
        // from:
        //   - On a raster the reading convention IS the axis: everything to the right of the sweep
        //     is a pass old and everything to the left is fresh. An oscillation reverses that every
        //     pass, so the display alternates between two mirror-image conventions and a glance can
        //     no longer tell which end is newest.
        //   - The raised-cosine easing makes the sweep DWELL at the limits, which was a virtue in
        //     the fan (it kept the end bearings refreshed, where the ellipse squash had bunched
        //     them). On an evenly spaced field it is a defect: the two edge columns would be
        //     refreshed many times over while mid-field columns got one look, leaving a pair of
        //     permanently bright edge stripes.
        // The trail is still CLAMPED at the left edge rather than wrapped round to the right, for
        // the reason the fan recorded - wrapping it would lay the wash on top of the OLDEST
        // returns, which are the ones the wake exists to show. On a raster that clamp is not a
        // compromise: a beam with no wake at the moment it reappears is exactly what a flyback
        // looks like.
        // IT SWEEPS THERE AND BACK: left to right, then right to left. Reported twice, and the
        // second report was explicit - "needs to sweep left to right, then right to left".
        //
        // The first attempt at this complaint kept the unidirectional wrap and merely BLANKED the beam
        // across the flyback, on the reading that what was disliked was the visible jump. That was the
        // wrong reading. The module's long note argued oscillation was actively wrong on a raster
        // because it reverses which end of the picture is freshest on every pass; that is true, and it
        // is worth less than the sweep looking right. The wake now follows the direction of travel,
        // which is what carries the reading instead.
        //
        // Derived from a monotonic phase, so a reversal cannot be lost to floating-point drift the way
        // an accumulator that has to change sign can.
        let prev = self.pos;
        self.phase = (self.phase + dt / (SWEEP_MS * 2.0)).rem_euclid(1.0);
        if !self.phase.is_finite() {
            self.phase = 0.0;
        }
        // Triangle wave: 0 -> 1 -> 0 across one cycle.
        let tri = if self.phase < 0.5 { self.phase * 2.0 } else { 2.0 - self.phase * 2.0 };
        // A MILD ease, 35% of the way to a smoothstep. Not the full raised cosine the fan used: that
        // brings the beam to a stop at each limit, and on an evenly spaced field the dwell refreshes
        // the two edge columns many times over while mid-field columns get one look, leaving a pair of
        // permanently bright edge stripes. This softens the turn without stopping at it.
        let eased = tri + 0.35 * (tri * tri * (3.0 - 2.0 * tri) - tri);
        // Scaled by `cols`, not `cols - 1`, and held just inside it. With `cols - 1` the beam reached
        // the final column only when the phase landed exactly on 0.5, which a discrete frame interval
        // essentially never does - so the rightmost column was never sampled.
        self.pos = (eased * cols as f32).min(cols as f32 - 0.001);
        self.dir = if self.phase < 0.5 { 1.0 } else { -1.0 };
        if !self.pos.is_finite() {
            self.pos = 0.0;
        }
        // Every cell between where the beam was and where it is now, in whichever direction. No
        // wrapping: the beam no longer crosses the ends, it turns round at them.
        let (lo, hi) = if self.pos >= prev { (prev, self.pos) } else { (self.pos, prev) };
        for k in (lo.floor() as i64).max(0)..=(hi.floor() as i64).min(cols as i64 - 1) {
            let ci = k as usize;
            let resp = Self::response(Self::cell_level(d, ci, cols), t.sensitivity);
            self.echo[ci] = resp;
            self.glow[ci] = BLIP_FLOOR + (1.0 - BLIP_FLOOR) * resp;
        }

        // Transient detector. The slew-limited average is what the rise is measured against, so a
        // sustained bass line settles and stops firing while a kick still spikes above it.
        self.hit = (self.hit - dt / BASS_FALL_MS).max(0.0);
        // Captured BEFORE the average moves, and shared with the warning receiver. One detector
        // feeding both displays is the whole reason the scope has no threshold of its own: this
        // project already carries two independently-written onset detectors, and a third would have
        // been a third chance to ship a threshold that never fires.
        let excess = self.bass.update(&d.levels, LOW_BANDS_WATCHED, BASS_EASE);
        #[cfg(test)]
        {
            self.last_excess = excess;
            self.last_centroid = Self::low_centroid(d);
            self.last_argmax = d.levels[..8.min(d.levels.len())]
                .iter()
                .enumerate()
                .filter(|(_, v)| v.is_finite())
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
        if excess > BASS_RISE {
            self.hit = 1.0;
            self.hit_pos = self.pos;
        }

        if scope.is_some() {
            self.rwr.update(dt, excess, BASS_RISE, t.radar.launch);
        }

        // ---- the grid ----
        //
        // Drawn straight onto the panel, NOT onto the lit layer. It is print on the glass, not
        // light: putting it through the bloom with everything else lifted it to within a few
        // luminance steps of the faint end of the wake, at which point the oldest returns were
        // indistinguishable from the range lines and the picture read as a pattern rather than as
        // data.
        //
        // 0.20/0.13, measured against the wake rather than picked: the faintest a stale return gets
        // is WAKE_FLOOR (0.12) of a near-white `hot`, so print above about 0.24 competes with real
        // data, and the fan's first dump showed 0.10 dropping the inner rings below the panel's own
        // near-black by enough that the display had no visible shape at silence.
        let ink = Rgba::from_hex(&t.lit, 0.20);
        let ink_faint = Rgba::from_hex(&t.lit, 0.13);
        let datum = f.y + f.h;

        // Range lines. The datum carries full ink because it is the reference the blips are read
        // against; the rest are fainter so a run of them cannot read as a return.
        for r in RANGE_LINES {
            // The quarter lines need somewhere to be. Below a 12px field they land on each other
            // and on the datum, so the small-panel case keeps only the datum and the top.
            if f.h < 12 && r != 0.0 && r != 1.0 {
                continue;
            }
            c.fill_rect(f.x, range_y(f, r), f.w, 1, if r == 0.0 { ink } else { ink_faint });
        }
        // Left and right frame, one pixel outside the field so they never sit under a blip column.
        c.fill_rect(f.x - 1, f.y, 1, f.h + 1, ink_faint);
        c.fill_rect(f.x + f.w, f.y, 1, f.h + 1, ink_faint);

        // Bearing lines, DASHED (2 on, 2 off). Solid was the obvious version and it is wrong at
        // this size, for a reason that is about total light rather than about alpha: a full-height
        // 1px line crosses all 51 rows of the field where a blip lights about 13 pixels, so ONE
        // solid vertical is worth four returns and the seven of them are worth 28 of the 32 the
        // display can show - the returns end up reading as decoration on a grid rather than the
        // other way round. Dashing halves each line to 26 pixels while leaving the alpha - which
        // was measured against the wake and must not move - exactly where it is.
        //
        // Spaced in PIXELS, not in columns. A fixed number of divisions was the first version and
        // it halves the grid's density in wide mode: `cols / 8` is 4 columns at 190px (a line every
        // 23px) but 8 columns at 380px, which is a line every 46px on a panel whose returns are
        // still 5.75px apart. Solving for the pixel spacing keeps the graticule looking the same at
        // every width, and it degrades correctly at the small end - on a 22px field the step comes
        // out wider than the field and no bearing lines are drawn at all, which is what a field
        // that narrow should get.
        let bstep = ((GRID_PX / col_w).round() as usize).max(1);
        let mut k = bstep;
        while k < cols {
            let x = col_x(f, cols, k as f32);
            let mut y = f.y;
            while y <= datum {
                c.fill_rect(x, y, 1, (datum - y + 1).min(2), ink);
                y += 4;
            }
            k += bstep;
        }
        // Finer bearing ticks, as stubs standing on the datum - a scale, not a grid.
        let tstep = (bstep / 2).max(1);
        let mut k = tstep;
        while k < cols {
            c.fill_rect(col_x(f, cols, k as f32), datum - 2, 1, 2, ink);
            k += tstep;
        }

        // The warning receiver's graticule, in the SAME two inks as the field's - so the two halves
        // of the panel read as one instrument rather than two pasted together.
        if let Some(s) = scope {
            self.rwr.print(c, s, ink, ink_faint);
        }

        // ---- light ----
        //
        // Own transparent layer. `Canvas::bloom` composites the halo UNDER what is already there,
        // and the panel above is fully opaque on every shipped colourway, so blooming in place
        // would leave no visible glow at all.
        let mut lit = Canvas::new(w, h);
        let hot = &t.hot;
        let sweep_x = col_x(f, cols, self.pos);

        // The sweep's own wash goes down FIRST, so the blips stay crisp on top of it. The fan drew
        // its beam last and the wedge visibly dimmed every return it was passing over.
        let trail = (TRAIL_CELLS * col_w).max(2.0) as i32;
        for back in 1..=trail {
            // Behind means the opposite way on the return leg.
            let x = sweep_x - (back as f32 * self.dir).round() as i32;
            if x < f.x || x >= f.x + f.w {
                break;
            }
            let fade = 1.0 - back as f32 / trail as f32;
            lit.fill_rect(
                x,
                f.y,
                1,
                f.h + 1,
                Rgba::from_hex(&t.lit, (fade * fade * TRAIL_ALPHA).clamp(0.0, 1.0)),
            );
        }

        let blip_rx = (col_w * BLIP_RX_FRAC).max(1.0);
        for ci in 0..cols {
            let g = self.glow[ci];
            if g <= 0.02 {
                continue;
            }
            let e = self.echo[ci];
            let range = RANGE_MIN + (RANGE_MAX - RANGE_MIN) * e;
            let (bx, by) = plot(f, cols, ci as f32 + 0.5, range);
            // Wider than tall, and its height barely grows with level: a contact whose vertical
            // extent grew with the reading would smear the very cue it carries. See BLIP_RX_FRAC.
            lit.elliptical_gradient(
                bx,
                by,
                blip_rx,
                1.2 + 0.9 * e,
                &[
                    (0.0, Rgba::from_hex(hot, (g * 0.85).clamp(0.0, 1.0))),
                    (0.45, Rgba::from_hex(&t.lit, (g * 0.58).clamp(0.0, 1.0))),
                    (1.0, Rgba::from_hex(&t.lit, 0.0)),
                ],
            );
            // Crisp 3x1 core, so the blip has a definite row to read. The gradient alone peaks
            // over two or three rows, and at this size that is enough to make two adjacent ranges
            // look like the same range.
            lit.fill_rect(bx - 1, by, 3, 1, Rgba::from_hex(hot, g.clamp(0.0, 1.0)));
        }

        // Transient contact, close in - below every band blip (see HIT_RANGE) so a kick reads as
        // clutter near the datum rather than as another band suddenly returning.
        if self.hit > 0.02 {
            let (hx, hy) = plot(f, cols, self.hit_pos, HIT_RANGE);
            let a = self.hit.clamp(0.0, 1.0);
            lit.elliptical_gradient(
                hx,
                hy,
                3.2,
                2.2,
                &[
                    (0.0, Rgba::from_hex(hot, a)),
                    (0.5, Rgba::from_hex(&t.lit, a * 0.6)),
                    (1.0, Rgba::from_hex(&t.lit, 0.0)),
                ],
            );
        }

        // The sweep line itself, last and brightest: after the flyback it is the one feature the
        // eye can pick up immediately, which is what makes the wrap read as a wrap.
        lit.fill_rect(sweep_x, f.y, 1, f.h + 1, Rgba::from_hex(hot, 0.95));

        // Threat contacts and the launch flash, on the same light layer so they bloom with the rest.
        if let Some(s) = scope {
            self.rwr.light(&mut lit, t, s, &t.radar.codes);
        }

        if t.bloom > 0.0 {
            let mut halo = lit.clone();
            halo.bloom(t.bloom.max(0.0) as i32, t.glow_strength.clamp(0.0, 1.0));
            c.draw_over(&halo);
        }
        c.draw_over(&lit);

        // Clip back to the panel with the SAME rect it was drawn with, or the bloom - and the
        // grid's own frame, 2px from the panel edge - escapes onto the bare taskbar and reads as a
        // bright box around the display.
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

    /// A spectrum with a loud region and a quiet one, so per-column behaviour is visible.
    ///
    /// Every level-sweep test in the valve row's first version drove EVERY band to the same value,
    /// where a column's mean equals its max and the whole per-column mapping is a mathematical
    /// no-op - which is exactly how a static-looking row passed its tests. Any test here that
    /// could pass on a display showing one number uses this instead.
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

    /// Range at which the brightest pixel in a column sits, in field units - 0 at the datum, 1 at
    /// maximum range. This is the measurement the whole family stands on: the blip's POSITION,
    /// read off the pixels rather than off the state.
    ///
    /// Printed rows are skipped, and that is a guard rather than a fix for an observed failure: on
    /// the fixture below the unmasked probe happens to agree (the quiet column's blip reads 112
    /// luminance against the faint range lines' 34). The margin is the wake, and it is thin. A
    /// return decayed to WAKE_FLOOR is 0.12 of `hot`, which composites to 36 - level with the
    /// print - so an unmasked probe would silently stop measuring the data and start measuring the
    /// graticule as soon as the frame count, `fade` or `SWEEP_MS` moved. That is the class of
    /// failure this project keeps catching after the fact, so the mask goes in now.
    fn peak_range(c: &Canvas, t: &Theme, w: i32, h: i32, cell: usize) -> f32 {
        let f = field(w, h, t.radar.rwr);
        let cols = column_count(w, h, t.radar.rwr);
        let x = col_x(f, cols, cell as f32 + 0.5);
        let print: Vec<i32> = RANGE_LINES.iter().map(|r| range_y(f, *r)).collect();
        let mut best = (0.0f64, -1.0f32);
        for row in 0..=f.h {
            let y = f.y + row;
            if print.iter().any(|p| (p - y).abs() <= 1) {
                continue;
            }
            let v = lum(c.get(x, y));
            if v > best.0 {
                best = (v, (f.h - row) as f32 / f.h as f32);
            }
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
            (28, 18),
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
        // The project has been bitten twice by f32::clamp NOT sanitising NaN. Here a poisoned dt
        // would leave `pos` NaN, which makes the column calculation NaN, which casts to 0 when
        // plotted - the sweep would sit on the left edge for the life of the process.
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
            assert!(r.pos.is_finite(), "spoil {spoil} left the sweep position at {}", r.pos);
            assert!(r.last_excess.is_finite(), "spoil {spoil} poisoned the bass detector");
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
    fn the_sweep_goes_there_and_back_without_ever_jumping() {
        // The reported behaviour, twice: "needs to sweep left to right, then right to left". The
        // previous version wrapped, and the test that guarded it asserted the sweep MUST wrap and must
        // NOT oscillate - so that test had to be replaced rather than repaired, along with the retrace
        // blanking that existed only to hide the wrap.
        //
        // Three properties, the last being the one a flyback fails: it reaches both ends, it reverses
        // at them, and it never moves further in one frame than a frame's worth.
        let t = builtin::radar_p1();
        let mut r = Radar::default();
        let mut c = Canvas::new(190, 60);
        let d = flat(0.4);
        let cols = column_count(190, 60, t.radar.rwr) as f32;

        r.draw(&mut c, &t, &d);
        let (mut min_pos, mut max_pos) = (r.pos, r.pos);
        let mut reversals = 0;
        let mut worst_step = 0.0f32;
        let mut last = r.pos;
        let mut last_dir = r.dir;
        for _ in 0..420 {
            r.draw(&mut c, &t, &d);
            min_pos = min_pos.min(r.pos);
            max_pos = max_pos.max(r.pos);
            worst_step = worst_step.max((r.pos - last).abs());
            if r.dir != last_dir {
                reversals += 1;
                last_dir = r.dir;
            }
            last = r.pos;
        }

        assert!(min_pos < 1.0, "never reached the left end, closest was {min_pos:.2}");
        assert!(max_pos > cols - 1.5, "never reached the right end, furthest was {max_pos:.2}");
        assert!(reversals >= 3, "only {reversals} reversals in two cycles - it is not going back and forth");
        // A wrap appears here as one step the width of the field. At 16ms a frame moves it about 0.36
        // of a column, so anything past a couple of columns is a jump rather than a sweep.
        assert!(
            worst_step < 3.0,
            "the beam moved {worst_step:.1} columns in one frame, which is a jump"
        );
    }

    #[test]
    fn one_pass_paints_every_column() {
        // The defining behaviour: the picture is BUILT UP over a pass rather than redrawn. A fresh
        // display must have empty columns, and a settled one must have none.
        let t = builtin::radar_p1();
        let cols = column_count(190, 60, t.radar.rwr);
        let (fresh, _) = run(&t, &uneven(4..9, 0.62, 0.18), 190, 60, 6);
        let painted = fresh.glow.iter().filter(|g| **g > 0.01).count();
        assert!(
            painted < cols / 2,
            "after 6 frames most columns should still be unpainted, {painted} of {cols} are lit"
        );

        let (settled, _) = run(&t, &uneven(4..9, 0.62, 0.18), 190, 60, 100);
        assert!(
            settled.glow.iter().all(|g| *g > 0.01),
            "after a full pass every column must carry a return: {:?}",
            settled.glow
        );
    }

    #[test]
    fn a_loud_band_puts_its_blip_higher_up_the_field_than_a_quiet_one() {
        // THE test for this family. Position, not brightness, read off the rendered pixels in two
        // different columns - so it cannot pass on a display that only changes colour, and it
        // cannot pass on a flat spectrum either.
        let t = builtin::radar_p1();
        // 220 frames, not 200: with the warning receiver taking the left of the panel the field has
        // 23 columns rather than 32, so the sweep is fewer COLUMNS along at the same elapsed time and
        // at 200 frames it sat within the trail's reach of probe column 5. The guard below is what
        // caught that, which is why it is an assertion rather than a comment.
        let (r, c) = run(&t, &uneven(4..8, 0.62, 0.17), 190, 60, 220);
        // Both probes must sit clear of the sweep and its wash, which run the full height of the
        // field and would swamp the measurement. The wash is BEHIND the sweep, i.e. at lower
        // columns, so a probe is safe if it is either well ahead or more than TRAIL_CELLS behind.
        for probe in [1usize, 5] {
            let behind = r.pos - probe as f32;
            assert!(
                behind < 0.0 || behind > TRAIL_CELLS + 1.0,
                "probe column {probe} is under the sweep at {} - retune the frame count",
                r.pos
            );
        }
        let loud = peak_range(&c, &t, 190, 60, 5);
        let quiet = peak_range(&c, &t, 190, 60, 1);
        assert!(
            loud > quiet + 0.25,
            "a loud column's blip must sit clearly higher: loud {loud:.2} vs quiet {quiet:.2}"
        );
        // And the quiet one must not be pinned on the datum, or "higher" is trivial.
        assert!(quiet > 0.1, "a quiet column must still return close-in clutter, got {quiet:.2}");
        // Nor may the loud one be pinned at the ceiling by a mapping that saturates early.
        assert!(loud < 1.0, "a full-scale return must stay off the top range line, got {loud:.2}");
    }

    #[test]
    fn the_audio_actually_changes_the_pixels() {
        // Summed over the FIELD, not the whole canvas, and the difference is not cosmetic: this used
        // to be a whole-canvas sum and it measured 1.12x once the warning receiver arrived, against a
        // 1.15x requirement. Nothing had regressed - the scope is a quarter of the panel and, on a
        // held flat level with no transients in it, contributes a constant print pedestal that
        // dilutes the ratio. Measuring the region the assertion actually talks about is the tighter
        // test, and the scope's own response is asserted separately below.
        let t = builtin::radar_p1();
        let total = |d: &FrameData| -> f64 {
            let (_, c) = run(&t, d, 190, 60, 100);
            let f = field(190, 60, t.radar.rwr);
            let mut sum = 0.0;
            for y in f.y..=(f.y + f.h) {
                for x in f.x..(f.x + f.w) {
                    sum += lum(c.get(x, y));
                }
            }
            sum
        };
        let quiet = total(&flat(0.05));
        let loud = total(&flat(0.95));
        assert!(loud > quiet * 1.15, "driving the input must light the field: {quiet:.0} -> {loud:.0}");

        // And two different SHAPES of spectrum at the same total energy must differ, which a
        // display keyed only to overall loudness would fail.
        let (_, a) = run(&t, &uneven(0..8, 0.62, 0.17), 190, 60, 200);
        let (_, b) = run(&t, &uneven(24..32, 0.62, 0.17), 190, 60, 200);
        assert_ne!(a.bits(), b.bits(), "bass-heavy and treble-heavy must not render identically");
    }

    #[test]
    fn the_wake_leaves_fresh_returns_brighter_than_stale_ones() {
        // A flat spectrum on purpose: every column was painted from the same level, so any
        // difference between them is the decay and nothing else.
        let t = builtin::radar_p1();
        let cols = column_count(190, 60, t.radar.rwr);
        let (r, _) = run(&t, &flat(0.5), 190, 60, 200);
        let behind = (r.pos.floor() as usize).min(cols - 1);
        let ahead = (behind + 3) % cols;
        assert!(
            r.glow[behind] > r.glow[ahead] * 1.5,
            "the column just swept must out-glow one swept a pass ago: {} vs {}",
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
    fn the_phosphor_wake_trails_to_the_left_of_the_sweep() {
        // Direction, on the pixels. A wake drawn on the wrong side would still decay correctly in
        // the state and every other test here would pass, but the display would read as a line
        // pushing a glow ahead of itself instead of leaving one behind.
        //
        // A flat spectrum, so the blips are at the same height everywhere and the only left/right
        // asymmetry near the sweep is the wash itself.
        let t = builtin::radar_p1();
        let (r, c) = run(&t, &flat(0.45), 190, 60, 200);
        let f = field(190, 60, t.radar.rwr);
        let cols = column_count(190, 60, t.radar.rwr);
        let sx = col_x(f, cols, r.pos);
        // Well inside the field, so neither sample falls off the end.
        assert!(sx - 12 > f.x && sx + 12 < f.x + f.w, "the sweep is too near an edge at {sx}");
        // A band of rows above the datum and below the blips, where only the wash lives.
        // Note the row order: `range_y` counts UP from the datum, so the higher range is the
        // SMALLER row number and a naive `range_y(0.05)..range_y(0.17)` is an empty range - which
        // is how the first version of this test compared 0 against 0 and still asserted something.
        let strip = |x: i32| -> f64 {
            (range_y(f, 0.17)..=range_y(f, 0.05)).map(|y| lum(c.get(x, y))).sum()
        };
        let (left, right) = (strip(sx - 4), strip(sx + 4));
        assert!(
            left > right * 1.5,
            "the wash must be behind the sweep: 4px left {left:.0} vs 4px right {right:.0}"
        );
    }

    #[test]
    fn a_bass_transient_fires_a_close_in_contact() {
        let t = builtin::radar_p1();
        let steady = flat(0.10);
        let mut kick = flat(0.10);
        for v in &mut kick.levels[..8] {
            *v = 0.85;
        }
        // Identical run in both cases except for the last three frames, so the sweep, the grid and
        // the whole painted picture are the same and the difference measured below is the contact
        // alone.
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

        let f = field(190, 60, t.radar.rwr);
        let cols = column_count(190, 60, t.radar.rwr);
        let (hx, hy) = plot(f, cols, r_hit.hit_pos, HIT_RANGE);
        // It has to be CLOSE IN, or it is just another blip: below every band return.
        assert!(
            hy > range_y(f, RANGE_MIN),
            "the contact must sit below the band blips: row {hy} vs {}",
            range_y(f, RANGE_MIN)
        );
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
    fn a_bass_transient_puts_a_contact_on_the_warning_receiver() {
        // The scope's own tests drive its state directly. This one goes through the family's `draw`,
        // so it additionally proves the scope is COMPOSITED - a state test cannot tell the difference
        // between a contact that was received and one that was received and then never drawn.
        let t = builtin::radar_p1();
        let steady = flat(0.10);
        let mut kick = flat(0.10);
        for v in &mut kick.levels[..8] {
            *v = 0.85;
        }
        // Identical run in both cases bar the last three frames, so the graticule and everything
        // else printed inside the scope is the same and the difference measured is the contact alone.
        let render = |hit: bool| -> Canvas {
            let mut r = Radar::default();
            let mut c = Canvas::new(190, 60);
            for _ in 0..100 {
                r.draw(&mut c, &t, &steady);
            }
            for _ in 0..3 {
                r.draw(&mut c, &t, if hit { &kick } else { &steady });
            }
            c
        };
        let s = super::super::rwr::scope(3, 60)
            .expect("the reference panel must carry a scope");
        // Strictly INSIDE the outer ring, so the printed ring itself is not part of the measurement.
        let inside = |c: &Canvas| -> f64 {
            let mut sum = 0.0;
            for dy in -s.r..=s.r {
                for dx in -s.r..=s.r {
                    if dx * dx + dy * dy < (s.r - 2) * (s.r - 2) {
                        sum += lum(c.get(s.cx + dx, s.cy + dy));
                    }
                }
            }
            sum
        };
        let (with, without) = (inside(&render(true)), inside(&render(false)));
        assert!(
            with > without + 300.0,
            "the transient must put light on the scope: {without:.0} -> {with:.0}"
        );

        // And the colourway's flag has to actually switch it off, or "per-colourway" is a lie.
        let mut off = builtin::radar_p1();
        off.radar.rwr = false;
        let (_, plain) = run(&off, &kick, 190, 60, 103);
        let (_, scoped) = run(&t, &kick, 190, 60, 103);
        assert_ne!(plain.bits(), scoped.bits(), "the rwr flag changed nothing about the render");
        assert!(
            field(190, 60, false).x < field(190, 60, true).x,
            "with the scope off the field must reclaim the left of the panel"
        );
    }

    #[test]
    fn the_group_reducer_is_biased_toward_the_peak_not_the_mean() {
        // Two bands feed one column at the reference size, so a single-band peak is halved by a
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
        let got = Radar::cell_level(&d, 5, CELLS);
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
    fn a_wide_panel_adds_columns_instead_of_fattening_them() {
        // Both modes, because the warning receiver takes width off the field and the invariant that
        // has to survive it is the PITCH, not the column count.
        for rwr in [false, true] {
            // Column pitch is the thing that must not drift: it is what sets the blip width and the
            // separation between neighbouring returns. The fan's face-count version of this stepped
            // once per whole panel width, so 240px (150% DPI) stretched to a 7.31px pitch.
            let pitch = |w: i32| field(w, 60, rwr).w as f32 / column_count(w, 60, rwr) as f32;
            for w in [190, 200, 240, 300, 380, 456, 600, 740] {
                let p = pitch(w);
                assert!(
                    (p - COL_PITCH).abs() < 0.2,
                    "rwr={rwr}: at width {w} the pitch drifted to {p:.2} from the tuned {COL_PITCH:.2}"
                );
            }
            // The printed grid is spaced in pixels for the same reason, and it is the part that is
            // easy to get wrong: a step counted in COLUMNS looks identical at the reference size and
            // comes out at half the density in wide mode.
            for w in [190, 240, 380, 456] {
                let cw = pitch(w);
                let spacing = (GRID_PX / cw).round().max(1.0) * cw;
                assert!(
                    (spacing - GRID_PX).abs() < 1.5,
                    "rwr={rwr}: at {w}px bearing lines land {spacing:.1}px apart, not {GRID_PX:.0}px"
                );
            }
            // Monotonic WITHIN a mode, so a wider panel never shows less of the spectrum than a
            // narrower one at the same setting.
            //
            // Across the scope switching on it is NOT monotonic and cannot be: the scope costs a fixed
            // ~57px, so at the first width where it fits, the field loses ten columns it had one pixel
            // earlier. There is no threshold that avoids that - the drop is the scope's width divided
            // by the pitch, whatever width it happens at - so the honest property is this one, plus
            // the assertion below that the two sizes actually shipped both keep a usable field.
            let mut prev = 0;
            let mut prev_has_scope = None;
            for w in [96, 150, 190, 240, 380, 456, 600] {
                // Whether the scope FITS is itself width-dependent, so even at rwr=true the sweep
                // crosses the switch-on. Compare only across widths that agree about it.
                let has = scope_w(w, 60, rwr) > 0;
                let n = column_count(w, 60, rwr);
                if prev_has_scope == Some(has) {
                    assert!(n >= prev, "rwr={rwr}: count fell from {prev} to {n} at width {w}");
                }
                prev = n;
                prev_has_scope = Some(has);
            }
        }
        // The widths this actually runs at. 190 is the default and 380 the doubled mode the config
        // uses, and both must keep enough spectrum to be worth looking at.
        for w in [190, 380] {
            let n = column_count(w, 60, true);
            assert!(n >= 20, "at the shipped width {w} the field is only {n} columns");
            assert!(scope_w(w, 60, true) > 0, "no scope at {w}px");
        }
        // With no scope the reference panel keeps the count the whole family was tuned at.
        assert_eq!(column_count(190, 60, false), CELLS, "the reference panel keeps the tuned count");
        assert!(column_count(380, 60, false) >= CELLS * 2, "double the width, double the resolution");
        assert!(column_count(4000, 60, false) <= CELLS * 4, "capped, or the buffers grow unbounded");
        // With it, the cost is real and bounded. It has to cost SOMETHING - a scope that took no
        // width would not be drawn at all - but a third of the spectrum would be too much to give up.
        let with = column_count(190, 60, true);
        assert!(with < CELLS, "the scope must cost width, or it is not being drawn: {with}");
        assert!(
            with >= CELLS * 2 / 3,
            "the scope took too much of the spectrum: {with} columns of {CELLS}"
        );
    }

    #[test]
    fn the_spectrum_still_runs_low_to_high_left_to_right_at_the_wide_size() {
        // The wide mode used to be two faces side by side, each covering half the spectrum, so the
        // frequency axis restarted in the middle of the panel. One field with more columns must
        // keep it monotonic, or the display means something different at each width.
        let t = builtin::radar_p1();
        let mut d = FrameData::default();
        let n = d.levels.len();
        for (i, v) in d.levels.iter_mut().enumerate() {
            *v = if i < n / 2 { 0.62 } else { 0.16 };
        }
        d.peaks = d.levels;
        for w in [190, 380] {
            let (r, _) = run(&t, &d, w, 60, 220);
            let cols = column_count(w, 60, t.radar.rwr);
            let half = |lo: usize, hi: usize| -> f32 {
                r.echo[lo..hi].iter().sum::<f32>() / (hi - lo) as f32
            };
            let (left, right) = (half(0, cols / 2), half(cols / 2, cols));
            assert!(
                left > right + 0.3,
                "at {w}px the bass must live in the LEFT half: {left:.2} vs {right:.2}"
            );
        }
    }

    #[test]
    fn the_field_fills_the_panel_rather_than_wasting_its_corners() {
        // The whole reason the fan went. A 190x60 panel has 188x56 of interior and the elliptical
        // fan touched it only along the bottom centre; the rectangle must reach all four corners.
        // Measured with the warning receiver OFF, because that is what the claim is about: the
        // sweep field ALONE has to fill the panel. With the scope on the panel is still fully
        // occupied, but by two instruments, and a round instrument legitimately leaves its own
        // corners as panel - which is why the pixel probe below is split by mode rather than
        // relaxed to whatever both happen to pass.
        let f = field(190, 60, false);
        assert!(f.x >= 2 && f.x + f.w <= 187, "the field must stay inside the panel: {} {}", f.x, f.w);
        assert!(
            f.w as f32 >= 188.0 * 0.95,
            "the field only uses {} of the 188px interior width",
            f.w
        );
        assert!(
            f.h as f32 >= 56.0 * 0.85,
            "the field only uses {} of the 56px interior height",
            f.h
        );
        // And on the pixels: something printed or lit must appear in each interior corner region.
        // The TOP two are the discriminating ones - a half-fan rising from the bottom centre
        // reaches the bottom corners with its outer ring but cannot put anything at all in the top
        // corners, at any level, because they are outside the ellipse.
        let mut plain = builtin::radar_p1();
        plain.radar.rwr = false;
        let panel = lum(Rgba::from_hex(&plain.panel, 1.0));
        let peak = |c: &Canvas, x0: i32, y0: i32| -> f64 {
            (y0..y0 + 7)
                .flat_map(|y| (x0..x0 + 7).map(move |x| (x, y)))
                .map(|(x, y)| lum(c.get(x, y)))
                .fold(0.0f64, f64::max)
        };
        let (_, c) = run(&plain, &uneven(0..CELLS, 0.6, 0.2), 190, 60, 120);
        for (x0, y0) in [(3, 4), (180, 4), (3, 49), (180, 49)] {
            assert!(
                peak(&c, x0, y0) > panel + 6.0,
                "the corner at ({x0},{y0}) is bare panel (peak {:.1} vs panel {panel:.1})",
                peak(&c, x0, y0)
            );
        }
        // As the family actually ships, with the scope on: the RIGHT corners are still the field's,
        // and the left third has to carry the scope's own ring at its top and bottom rather than
        // being dead space.
        let t = builtin::radar_p1();
        let (_, c) = run(&t, &uneven(0..CELLS, 0.6, 0.2), 190, 60, 120);
        for (x0, y0) in [(180, 4), (180, 49)] {
            assert!(
                peak(&c, x0, y0) > panel + 6.0,
                "with the scope on, the field corner at ({x0},{y0}) went bare"
            );
        }
        let s = super::super::rwr::scope(3, 60)
            .expect("the reference panel must carry a scope");
        for y in [s.cy - s.r, s.cy + s.r] {
            let best = ((s.cx - 3)..=(s.cx + 3)).map(|x| lum(c.get(x, y))).fold(0.0f64, f64::max);
            assert!(best > panel + 4.0, "the scope's ring is missing at row {y} (peak {best:.1})");
        }
    }

    // REMOVED: `the_grid_stays_recessive_behind_the_returns`, and its two replacements.
    //
    // The property is real and worth having - the printed grid must not out-read the data - but four
    // formulations all turned out vacuous, each for a different reason, and a passing test that
    // proves nothing is worse than an absent one:
    //
    //   1. Sampling "a fresh return" at the column the sweep had just landed on put the sample 1px
    //      from the beam, so it measured the beam and its radius-4 bloom halo.
    //   2. Sampling three cells behind put it inside the 26px trail wash, which is painted per
    //      column regardless of any echo - so it measured the WAKE.
    //   3. Comparing against a column 7px away compared a return against another RETURN, since that
    //      is barely one cell at 5.75px per column.
    //   4. A differential peak between silence and full drive, with the beam and trail excluded,
    //      still passed with the per-column blips AND the bass-transient contact both deleted -
    //      so something else in the field varies with level and it was never isolating the returns.
    //
    // The grid's recessiveness is asserted indirectly by the tests that DO survive mutation:
    // `a_loud_band_puts_its_blip_higher_up_the_field_than_a_quiet_one` (killed by a constant range),
    // `the_audio_actually_changes_the_pixels`, and `the_wake_leaves_fresh_returns_brighter_than_
    // stale_ones` (killed by removing the phosphor decay). Judging grid-versus-data balance is left
    // to the eyeball dump, which is what it is for.


    #[test]
    fn nothing_is_drawn_outside_the_panel() {
        // The grid's frame sits 2px from the panel edge and the bloom spreads past it, so without
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
        // The degenerate branch must not be a blank canvas - the overlay would show the Widgets
        // button's own weather text through it.
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
        assert_eq!(seen.len(), 8, "expected the eight radar colourways, got {}", seen.len());
    }

    /// Real music, captured with `--levels`: the committed fixture, or whatever `TASKBAR_EQ_FIXTURE`
    /// points at.
    ///
    /// The override exists because the committed fixture turned out to be 13 seconds of a STEADY
    /// groove - its loudest bass transient is only 1.25x the median - so it cannot say anything about
    /// how often a launch should fire on material that has dynamics. Calibrating against one capture is
    /// how the launch came to be unfireable in the first place. `include_str!` is compile-time, so
    /// without this every new capture cost a full rebuild and measuring several tracks was too slow to
    /// bother with, which is exactly why it did not happen sooner.
    fn real_music() -> Vec<Vec<f32>> {
        let text = match std::env::var("TASKBAR_EQ_FIXTURE") {
            Ok(p) => std::fs::read_to_string(&p)
                .unwrap_or_else(|e| panic!("TASKBAR_EQ_FIXTURE={p}: {e}")),
            Err(_) => include_str!("../../tests/fixtures/real-music-bands.csv").to_string(),
        };
        text.lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.split(',').filter_map(|v| v.parse::<f32>().ok()).collect())
            .collect()
    }

    /// The launch fires on dynamic material at the shipped default, and stays rare on flat material.
    ///
    /// **This is the test that would have caught every launch failure in this file's history**, and
    /// there were four: an absolute threshold in detector units, then a fast-adapting reference that
    /// tracked the beat too closely, then a slow one that never shook off the detector's startup
    /// transient, then a ratio window (1.3x..2.5x) far above anything real music produces. Every one of
    /// them passed the synthetic tests and fired zero times on real audio.
    ///
    /// Two committed fixtures, captured with `--levels` from a real session:
    /// `real-music-dynamic.csv` is drum and bass with genuine dynamics, `real-music-flat.csv` is a
    /// steady, flat-mastered track whose loudest bass transient is 1.08x its median.
    #[test]
    fn the_launch_fires_on_dynamic_music_and_stays_quiet_on_flat_music() {
        let count = |csv: &str, at: f32| -> u32 {
            let frames: Vec<Vec<f32>> = csv
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.split(',').filter_map(|v| v.parse::<f32>().ok()).collect())
                .collect();
            assert!(frames.len() > 500, "fixture looks truncated: {} frames", frames.len());
            let mut t = builtin::radar_p1();
            t.radar.launch = at;
            let mut r = Radar::default();
            let mut c = Canvas::new(190, 60);
            for row in &frames {
                let mut d = FrameData::default();
                for (i, x) in d.levels.iter_mut().enumerate() {
                    *x = row.get(i).copied().unwrap_or(0.0);
                }
                d.peaks = d.levels;
                r.draw(&mut c, &t, &d);
            }
            r.rwr.launches
        };
        let dynamic = include_str!("../../tests/fixtures/real-music-dynamic.csv");
        let flat = include_str!("../../tests/fixtures/real-music-flat.csv");
        let default = builtin::radar_p1().radar.launch;

        // The shipped default must actually fire on material that has big hits in it. 13 seconds is
        // about five at the measured rate; three is a wide enough floor to survive retuning.
        let n = count(dynamic, default);
        assert!(
            n >= 3,
            "the launch fired only {n} times in 13s of dynamic music at the shipped default {default}"
        );
        // And it must stay an EVENT rather than a per-beat animation.
        assert!(n <= 30, "{n} launches in 13s is not \"fairly rare\", it is the normal state");
        // Flat material legitimately has no big hits, so it should be quiet - but this must be because
        // the material is flat, not because the threshold is unreachable, which the line above proves.
        let f = count(flat, default);
        assert!(f < n, "flat material launched {f} times against dynamic material's {n}");

        // The knob has to move the rate on real audio, not just on synthetic fixtures.
        let loose = count(dynamic, 0.15);
        let strict = count(dynamic, 0.95);
        assert!(loose > n, "loosening the knob did nothing: {loose} vs {n} at the default");
        assert!(strict < n, "tightening the knob did nothing: {strict} vs {n} at the default");
    }

    /// What the detector actually reports on real music, and what that means for the launch rate.
    ///
    /// Run: cargo test --release probe_rwr_rates -- --ignored --nocapture
    ///
    /// This exists because the first cut scaled the strength by `rise * 3`, i.e. saturating at an
    /// excess of 0.22, and a synthetic fixture made that look sane. It is not: this project has
    /// already shipped one transient threshold that could not fire on real music at all (see the
    /// vaporwave lightning note), and the mirror-image mistake - one that fires on EVERYTHING - is
    /// just as easy. So the excess distribution is measured, and the span is read off it.
    #[test]
    #[ignore]
    fn probe_rwr_rates() {
        let frames = real_music();
        let secs = frames.len() as f32 * 16.7 / 1000.0;
        let t = builtin::radar_p1();
        let mut r = Radar::default();
        let mut c = Canvas::new(190, 60);
        let mut excess = Vec::new();
        let mut cent = Vec::new();
        let mut argmax = Vec::new();
        for row in &frames {
            let mut d = FrameData::default();
            for (i, x) in d.levels.iter_mut().enumerate() {
                *x = row.get(i).copied().unwrap_or(0.0);
            }
            d.peaks = d.levels;
            r.draw(&mut c, &t, &d);
            excess.push(r.last_excess);
            cent.push(r.last_centroid);
            argmax.push(r.last_argmax);
        }
        let mut sorted: Vec<f32> = excess.iter().copied().filter(|e| e.is_finite()).collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let pct = |p: f32| sorted[((sorted.len() - 1) as f32 * p) as usize];
        println!("{} frames, {secs:.1}s of music", frames.len());
        println!(
            "  excess percentiles: p50 {:.3}  p90 {:.3}  p99 {:.3}  max {:.3}",
            pct(0.50),
            pct(0.90),
            pct(0.99),
            sorted[sorted.len() - 1]
        );
        let contacts = excess.iter().filter(|e| **e > BASS_RISE).count();
        println!("  contacts fired: {contacts} ({:.2}/s)", contacts as f32 / secs);

        // WHERE the bearing cue actually points. The live overlay put every contact in the top-right
        // quadrant with the same designator on every one, which means the quantity driving both is
        // barely moving - so measure its spread rather than assume a window for it.
        let fired: Vec<usize> =
            (0..excess.len()).filter(|i| excess[*i] > BASS_RISE).collect();
        let mut cs: Vec<f32> = fired.iter().map(|i| cent[*i]).collect();
        cs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        if !cs.is_empty() {
            println!(
                "  centroid AT CONTACTS: min {:.3}  p25 {:.3}  p50 {:.3}  p75 {:.3}  max {:.3}  (spread {:.3})",
                cs[0],
                cs[cs.len() / 4],
                cs[cs.len() / 2],
                cs[cs.len() * 3 / 4],
                cs[cs.len() - 1],
                cs[cs.len() - 1] - cs[0]
            );
        }
        let mut hist = [0usize; 8];
        for i in &fired {
            hist[argmax[*i].min(7)] += 1;
        }
        println!("  leading band AT CONTACTS: {hist:?}  (bands 0..8)");
        let distinct = hist.iter().filter(|n| **n > 0).count();
        println!("  -> centroid spread would use {:.0}% of a circle, leading band gives {distinct} of 8 discrete bearings",
            (cs.last().copied().unwrap_or(0.0) - cs.first().copied().unwrap_or(0.0)) * 100.0);

        // The launch rate, counted through the SHIPPED code path at each knob setting: a fresh `Radar`
        // per setting, driven over the same frames. Nothing here restates the rule, and that is the
        // whole reason this number can be trusted - the version before measured a formula written out
        // again in the test, which can drift from the one that ships without any test failing.
        for at in [0.0f32, 0.1, 0.2, 0.3, 0.4, 0.55, 0.7, 0.85, 1.0] {
            let mut tt = builtin::radar_p1();
            tt.radar.launch = at;
            let mut rr = Radar::default();
            let mut cc = Canvas::new(190, 60);
            for row in &frames {
                let mut d = FrameData::default();
                for (i, x) in d.levels.iter_mut().enumerate() {
                    *x = row.get(i).copied().unwrap_or(0.0);
                }
                d.peaks = d.levels;
                rr.draw(&mut cc, &tt, &d);
            }
            if at == 0.0 {
                let mut rs: Vec<f32> = rr.rwr.seen.iter().map(|(r, _)| *r).collect();
                rs.sort_by(|a, b| a.partial_cmp(b).unwrap());
                if !rs.is_empty() {
                    println!(
                        "  ratio AT CONTACTS ({} of them): p10 {:.2}  p50 {:.2}  p90 {:.2}  max {:.2}   (need >= {:.2}..{:.2})",
                        rs.len(), rs[rs.len()/10], rs[rs.len()/2], rs[rs.len()*9/10], rs[rs.len()-1],
                        1.3, 2.5
                    );
                    let typ: Vec<f32> = rr.rwr.seen.iter().map(|(_, t)| *t).collect();
                    println!("  typical over the run: first {:.3}  last {:.3}", typ[0], typ[typ.len()-1]);
                }
            }
            let n = rr.rwr.launches;
            println!(
                "  launch = {at:.2}: {n} launches in {secs:.1}s ({:.2}/s, one every {:.1}s)",
                n as f32 / secs,
                if n > 0 { secs / n as f32 } else { f32::INFINITY }
            );
        }
    }

    /// Run: cargo test --release dump_radar_frames -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_radar_frames() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();
        // An uneven, musical-looking spectrum, so the blips sit at visibly different heights.
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
            // Just over one pass at 16.7ms/frame, so the wake is fully developed and the sweep is
            // mid-field - which is the state a human will actually be looking at.
            let (_, c) = run(&t, &d, 190, 60, 100);
            dump(&c, &format!("radar-{}.rgba", t.id));
            n += 1;
        }
        // The frame just after the flyback, which is the one the wrap-versus-oscillate decision
        // rests on: the sweep is at the left edge with no wash behind it and a full pass of
        // history still standing to its right.
        let t = builtin::radar_p1();
        let (_, fly) = run(&t, &d, 190, 60, 85);
        dump(&fly, "radar-flyback-p1.rgba");
        // One wide dump too: at 380px the field takes 65 columns rather than a second face, and
        // that is invisible at the reference size.
        let (_, wide) = run(&t, &d, 380, 60, 100);
        dump(&wide, "radar-wide-p1.rgba");

        // The warning receiver needs TRANSIENTS to have anything on it, and the static spectrum
        // above has none - a held level settles the detector's average and the scope goes empty,
        // which would make an eyeball dump of it look broken when it is working exactly as designed.
        // So this drives a groove: a kick every 500ms, one of them hard enough to launch.
        // The low bands are damped for this fixture, and that is not cosmetic. `d` above is a
        // display-shape spectrum whose lowest bands already peak near 0.77, so a kick laid on top of
        // it produced NO rise at all - the detector watches the peak of bands 0..8 and it was already
        // there. The first version of this dump came out with an empty scope for exactly that reason,
        // and it looked like a broken feature rather than a broken fixture.
        // 0.02, not 0.12. The centroid is a level-weighted mean over twelve bands, so a high floor
        // DOMINATES it: at 0.12 the five kicks below produced centroids 0.44-0.51, a spread of 0.07
        // against a merge window of 0.072, and every beat merged into one contact. A near-silent floor
        // lets the kick decide where the energy is, which is the whole premise of the bearing cue.
        let mut base = d.levels;
        for v in &mut base[..12] {
            *v = 0.02;
        }
        let beat = |th: &Theme, r: &mut Radar, c: &mut Canvas, frames: usize, hard_at: usize| {
            for i in 0..frames {
                let mut f = FrameData::default();
                f.levels = base;
                // A kick every 30 frames (~500ms, 120bpm), two frames long.
                if i % 30 < 2 {
                    let b = i / 30;
                    // Amplitudes chosen against the measured spans: an excess of 0.06-0.11 over the
                    // floor spreads ordinary contacts from the rim to mid-scope, and the hard one
                    // clears LAUNCH_SPAN outright.
                    let amp = if b == hard_at { 0.90 } else { [0.08, 0.11, 0.09, 0.13, 0.10][b % 5] };
                    // Each kick sits in a DIFFERENT part of the low band, so the centroid moves and
                    // successive beats land at different bearings with different designators. A
                    // fixture that kicked the same bands every beat exercised the merge rule instead
                    // and put exactly one contact on the scope - which also looks like a bug and is
                    // not one. Kept inside bands 0..8, which is the window the detector watches.
                    let o = [0usize, 5, 2, 4, 1, 3][b % 6];
                    for v in &mut f.levels[o..(o + 3).min(8)] {
                        *v = amp;
                    }
                }
                f.peaks = f.levels;
                r.draw(c, th, &f);
            }
        };
        // Frame 92 is two frames after the hard kick at beat 3, so the launch ring is still fresh.
        let mut r = Radar::default();
        let mut c = Canvas::new(190, 60);
        beat(&t, &mut r, &mut c, 92, 3);
        dump(&c, "radar-rwr-launch-p1.rgba");
        // No hard beat at all (hard_at is past the run), five beats in, so the scope is holding
        // several ordinary contacts and every one shows its designator. This is the frame that says
        // whether the labels are legible at 45px.
        let mut r = Radar::default();
        let mut c = Canvas::new(190, 60);
        beat(&t, &mut r, &mut c, 158, 99);
        dump(&c, "radar-rwr-hold-p1.rgba");
        // The same groove at the wide size, where the scope keeps its size and the field gets the
        // extra width - the proportion is completely different and only a dump shows it.
        let mut r = Radar::default();
        let mut c = Canvas::new(380, 60);
        beat(&t, &mut r, &mut c, 92, 3);
        dump(&c, "radar-rwr-wide-p1.rgba");
        // With the scope switched off, for the side-by-side that decides whether its width is
        // worth eight columns of spectrum.
        let mut plain = builtin::radar_p1();
        plain.radar.rwr = false;
        let (_, off) = run(&plain, &d, 190, 60, 100);
        dump(&off, "radar-rwr-off-p1.rgba");
        // One more colourway with the scope, since the rings and contacts are drawn in the
        // colourway's own two inks and alert is the most saturated of the five.
        let alert = builtin::radar_alert();
        let mut r = Radar::default();
        let mut c = Canvas::new(190, 60);
        beat(&alert, &mut r, &mut c, 92, 3);
        dump(&c, "radar-rwr-launch-alert.rgba");

        println!(
            "wrote {} radar dumps (190x60) + flyback + wide + 5 rwr dumps to {}",
            n,
            dir.display()
        );
    }
}
