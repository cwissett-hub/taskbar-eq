//! The Pantone family: a column meter printed as a mis-registered four-colour page.
//!
//! Named for Felipe Pantone, and the point of it is that "ultra-saturated rainbow" is not the
//! identifiable part of that work - plenty of things are rainbows. Four devices are:
//!
//! - **RGB misregistration.** The red and blue plates offset a pixel or two from green, like a
//!   badly printed page or a display with a failing cable. This is the single most recognisable
//!   element, it is the last thing this family does to its canvas, and it is the reason
//!   `Canvas::chromatic_aberration` exists. It only works over an OPAQUE panel: a mark on a
//!   transparent ink layer has nothing either side of it to fringe against.
//! - **A halftone screen.** A clustered-dot 4x4 cell, not a dispersed dither - a dot that GROWS
//!   reads as printed tone where a scattered stipple reads as noise. Its 4px lattice against the
//!   9px bar pitch is what makes the moire, and that ratio is chosen not to be an integer.
//! - **A barcode stripe band.** Hard black-and-white vertical rules of irregular width, drawn
//!   crisp and never bloomed, because flat ink does not glow. It is also where the
//!   misregistration is most legible: a hard achromatic edge is exactly where a mis-set plate
//!   shows, in print as much as here.
//! - **A hard diagonal split and a glitch slice.** The diagonal divides solid ink from screened
//!   ink across the bar field; the glitch slice displaces a few rows sideways.
//!
//! **The audio cue is BAR HEIGHT** - the position cue, per the project's standing rule that
//! position reads far better than intensity at 190x60. The colour, the screen and the barcode are
//! all structure and say nothing about the music; the aberration widens with overall energy, which
//! is a second cue and not the primary one.
//!
//! ## The contrast problem, and what was actually chosen
//!
//! Every lit colour here must clear 3:1 against its own panel, and this look wants MAXIMUM chroma.
//! Those are in direct conflict and the conflict is not resolvable by taste:
//!
//! * Full chroma on a continuous wheel is impossible against ANY single panel colour. Blue at full
//!   chroma needs a panel at relative luminance 0.317 or above; yellow needs 0.276 or below.
//!   Sweeping every grey from black to white, the best panel is pure BLACK and even there the worst
//!   hue reaches only 2.44:1. A lighter panel is a legitimate answer to a rule about contrast
//!   rather than brightness - it is just not an answer to this one.
//! * The best any grey panel achieves is saturation 0.80 (pure black, worst hue 3.06:1), so
//!   `RAINBOW_SAT`'s 0.68 was already close to the ceiling.
//! * The way out is that Pantone's own reference is not a continuous spectrum - it is INK PLATES.
//!   Quantising the wheel to three inks lands on yellow/cyan/magenta, the chromatic process set,
//!   which clears **6.41:1 at FULL chroma** on this family's `#07070a` panel, because that set
//!   simply does not contain the one hue that cannot be made to work.
//!
//! So: a near-black panel, and four of the five colourways run at full or near-full chroma by
//! quantising. Worst-hue contrast measured on the shipped colourways: `pantone-process` (3 inks,
//! chroma 1.0) 6.41:1, `pantone-halftone` (2 inks, chroma 1.0) 3.22:1, `pantone-misregister`
//! (6 inks, chroma 0.92) 3.47:1, and the continuous `pantone-spectrum` (chroma 0.68) 3.84:1. Only
//! that last one is held below full chroma, and it is capped by arithmetic rather than preference.
//! See `themes::quantise_hue` and `themes::RAINBOW_SAT` for the full tables.

use super::canvas::{Canvas, Rgba};
use super::{Family, FrameData};
use crate::themes::Theme;

/// Band level at which a column starts to rise, and the span it rises over.
///
/// The input range is NOT 0..1: `FrameData.levels` sits around 0.15-0.65 for active bands on real
/// music. A linear 0..1 mapping therefore spends about a third of its travel and the meter looks
/// dead - the same fault already measured and fixed in the valve row, so the same window is used
/// here. Measured on this family at 190x60 with a flat spectrum, on a 42-row field: across
/// 0.15-0.65 the top of the column climbs from row 3 to row 41, i.e. 90% of the travel. A linear
/// 0..1 map over the same window moved it from row 6 to row 27 - exactly half - and then wasted the
/// whole of 0.65..1.0 on levels real music does not reach. See `probe_pantone_travel` for the table.
///
/// A FIXED window rather than a peak follower, for the same reason the valve row uses one: this is
/// a level meter, and a follower would draw a quiet passage at the same height as a loud one.
const RESP_FLOOR: f32 = 0.10;
const RESP_SPAN: f32 = 0.52;

/// Weight given to a group's LOUDEST band rather than its mean.
///
/// Twenty columns over 64 log bands is 3.2 bands each, and averaging them flattens exactly the
/// single-band peaks that make one column differ from its neighbour. Measured with one band peaking
/// at 0.65 inside a group otherwise at 0.20, on a 42-row field: biased toward the max, the driven
/// column stands at row 36 against its neighbour's row 7 - 29 rows of separation. A plain mean would
/// resolve that same group to 0.350 and draw row 20, leaving 13 rows, so more than half the visible
/// difference comes from this constant alone.
///
/// Invisible to any test that drives every band to the same level, since mean == max there - which
/// is what let the valve row ship looking static.
const GROUP_MAX_BIAS: f32 = 0.70;

/// Per-frame fall of a column's peak-hold rule, in displayed units.
///
/// Sourced from the DISPLAYED response, not `FrameData.peaks`. The shared peak-hold falls at 0.0055
/// per frame, which under continuous music leaves the rule pinned near the top of every column at
/// once - a row of identical marks that makes the field read MORE uniform, which is the opposite of
/// what a peak rule is for. Matches the valve row's marker, which was fixed the same way.
const MARKER_FALL: f32 = 0.030;

/// Rows at the top of a lit column that take the near-white `hot` ink.
///
/// Two, so the top of the column is unmistakable without the cap becoming the column. The height is
/// the cue and a cap is how you find the height at a glance.
const CAP_ROWS: i32 = 2;

/// Bar width and gap, in pixels.
///
/// 6 and 3 rather than the segmented family's 5 and 2: this look is chunky poster print, and at 5px
/// a 4px halftone lattice fits barely one dot across a bar, so the screen inside a column read as a
/// dotted line rather than as a tint. At 6 it is a real screen.
const BAR_W: i32 = 6;
const BAR_GAP: i32 = 3;
const BAR_PITCH: i32 = BAR_W + BAR_GAP;

/// Columns at a given panel width.
///
/// Scaled rather than stretched, the same call the valve row makes and for the same measured
/// reason: a fixed count at 380px would double the bar width and the halftone lattice would no
/// longer beat against the pitch at all, since the moire depends on the 4:9 ratio. Adding columns
/// keeps every bar the width it was tuned at and narrows each column's slice of the spectrum, which
/// makes neighbours differ more.
fn bar_count(w: i32) -> usize {
    ((((w - 4) / BAR_PITCH).max(3)) as usize).min(128)
}

/// Minimum bar-field height below which the barcode band is dropped entirely.
///
/// A 10-row field is already a poor meter; giving away rows to decoration below that leaves a
/// display that cannot show a level at all. The band goes first because it carries no information.
const MIN_FIELD: i32 = 10;

/// Width of barcode stripe `i`, alternating ink and paper starting with ink.
///
/// A hash of the INDEX, not a random number and not a cyclic array. Two versions were wrong in
/// opposite ways:
///
/// * Anything frame-derived reshuffles the band every frame, which is a flicker rather than printed
///   structure, and would make every test in this file unrepeatable.
/// * An 18-entry table was the first attempt and its widths summed to 36px, so across the 186px
///   reference panel it repeated 5.2 times. In the render dump that read as a wallpaper motif rather
///   than a code - the eye picks a 36px repeat out immediately at this size. Hashing the index gives
///   a sequence that never repeats at any panel width while staying byte-for-byte deterministic.
///
/// Widths are 1-5px, the range in which the band still reads as a barcode at 190px: all-1px is a
/// comb, and 6px and up is a row of blocks. 0x9E3779B1 is the usual 32-bit golden-ratio constant,
/// and the shift takes the high bits because the low bits of a multiplicative hash are the least
/// mixed - taking `h % 5` directly gives a period-5 sequence, which is exactly the repeat this is
/// meant to avoid.
fn barcode_width(i: usize) -> i32 {
    let h = (i as u32).wrapping_mul(0x9E37_79B1);
    ((h >> 17) % 5) as i32 + 1
}

/// 4x4 CLUSTERED-dot halftone screen, cell values 0..15.
///
/// Deliberately NOT `Canvas::BAYER_4X4`. Bayer is a DISPERSED pattern - it spreads its lit cells as
/// far apart as it can, which is right for hiding a gradient's quantisation and wrong for this: it
/// reads as noise. A clustered screen grows a single dot outward from one corner of each cell, so
/// coverage reads as tone while the lattice stays visible, and it is the lattice that beats against
/// the bar pitch. Lattice 4 against pitch 9 on purpose: 9/4 is not an integer, so the phase of the
/// screen relative to each bar walks across the panel and produces the interference. At a pitch of
/// 8 or 12 every bar would carry an identical dot pattern and there would be no moire at all.
const SCREEN_4X4: [[u8; 4]; 4] =
    [[12, 5, 6, 13], [4, 0, 1, 7], [11, 3, 2, 8], [15, 10, 9, 14]];

/// Whether `(x, y)` is inked by the halftone screen at the given coverage.
fn screened(x: i32, y: i32, coverage: f32) -> bool {
    if !coverage.is_finite() || coverage <= 0.0 {
        return false;
    }
    if coverage >= 1.0 {
        return true;
    }
    let v = SCREEN_4X4[y.rem_euclid(4) as usize][x.rem_euclid(4) as usize];
    (v as f32 + 0.5) / 16.0 < coverage
}

/// Slope of the hard diagonal that splits solid ink from screened ink.
///
/// 2 columns per row. At 1.0 the split is a 45 degree line that only crosses 43 of the 190 columns
/// and reads as a corner cut; at 2.0 it travels 86 columns and reads as a diagonal dividing the
/// panel into two zones, which is the Pantone device. Steeper than about 3 and it is a near-vertical
/// wall that just looks like a seam between two halves.
const DIAG_SLOPE: f32 = 2.0;

/// The flourish: a plate slips out of register and creeps back.
///
/// The family already misregisters horizontally, and widens that fringe with energy - so pushing the
/// horizontal shift further would read as the music getting louder, not as a press going wrong. The
/// slip is therefore mostly VERTICAL, which is an axis nothing else in this family uses, plus 1px of
/// horizontal so it reads as a sheet that has physically moved rather than as a scanline artefact.
///
/// 3px vertical at a 60px panel is a twentieth of the height and about half a bar's cap - big enough
/// to be unmistakable, small enough that the bars still read as bars. At 5 the panel came apart into
/// three coloured ghosts and stopped being a chart.
///
/// 900ms, and it decays rather than snapping: a plate that springs back instantly reads as one dropped
/// frame, which at 60fps most people simply do not see. The decay is what makes it read as a press
/// running out of register and being brought back in.
const MISREG_X: f32 = 1.0;
const MISREG_Y: f32 = 3.0;
const MISREG_MS: f32 = 900.0;

/// Sweeps of the glitch slice per second.
///
/// Slow: the slice is a fault, and a fault that recurs 10 times a second is a strobe. 0.55Hz gives
/// roughly one pass every two seconds, which reads as an intermittent glitch.
const GLITCH_HZ: f32 = 0.55;

#[derive(Default)]
pub struct Pantone {
    /// The flourish: the plates slip out of register. See `MISREG_Y`.
    flourish: crate::dsp::flourish::Trigger,
    slip: crate::dsp::flourish::Envelope,
    /// Fast-falling peak hold per column, in displayed-response units.
    ///
    /// A `Vec` because `bar_count` reaches 48 and `#[derive(Default)]` has no impl for arrays that
    /// long - the std impls stop at 32. Same reason the valve row uses one.
    marker: Vec<f32>,
}

impl Pantone {
    /// Level for column `i` of `n`, biased toward the group's loudest band.
    fn level_for(d: &FrameData, i: usize, n: usize) -> f32 {
        let len = d.levels.len();
        let n = n.max(1);
        let lo = i * len / n;
        let hi = (((i + 1) * len / n).max(lo + 1)).min(len);
        let (mut acc, mut cnt, mut peak) = (0.0f32, 0.0f32, 0.0f32);
        for v in &d.levels[lo..hi] {
            // is_finite BEFORE anything accumulates: f32::clamp does not sanitise NaN (every
            // comparison with NaN is false, so clamp returns it unchanged) and a single poisoned
            // band would otherwise reach the peak-hold state and stay there for the process's life.
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

    /// Maps a group level onto 0..=1 of column travel. See `RESP_FLOOR`.
    fn response(level: f32, sensitivity: f32) -> f32 {
        if !level.is_finite() {
            return 0.0;
        }
        (((level - RESP_FLOOR) / RESP_SPAN) * sensitivity.max(0.0)).clamp(0.0, 1.0)
    }
}

impl Family for Pantone {
    fn id(&self) -> &'static str {
        "pantone"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());
        c.clear();
        let panel = Rgba::from_hex(&t.panel, t.panel_alpha);

        // Too small to hold a bar field: paint the panel and stop, rather than drawing a barcode
        // and a screen on top of each other in eight pixels. The overlay is sized from the LIVE
        // Widgets-button rect, so a degenerate size is a real runtime case, not a test-only one.
        if w < 24 || h < 18 {
            c.rounded_rect(1, 2, (w - 2).max(1), (h - 4).max(1), 3, panel);
            return;
        }
        c.rounded_rect(1, 2, w - 2, h - 4, 4, panel);

        // ---- geometry ----
        let interior_top = 3;
        let interior_bot = h - 5; // exclusive; rows h-5..h-3 hold the baseline rule and the bezel
        let interior_h = interior_bot - interior_top;
        let mut bc_h = if t.pantone.barcode > 0.0 {
            (interior_h as f32 * t.pantone.barcode.clamp(0.0, 0.6)).round() as i32
        } else {
            0
        };
        if bc_h < 2 || interior_h - bc_h - 2 < MIN_FIELD {
            bc_h = 0;
        }
        let field_top = interior_top + if bc_h > 0 { bc_h + 2 } else { 0 };
        let field_bot = interior_bot;
        let field_h = (field_bot - field_top).max(1);

        let bars = bar_count(w);
        let ox = 2 + ((w - 4) - bars as i32 * BAR_PITCH + BAR_GAP) / 2;
        if self.marker.len() != bars {
            self.marker.resize(bars, 0.0);
        }

        // Responses first, because the glitch amplitude and the aberration width both need the
        // whole frame's energy before any of it is drawn.
        let mut resp = Vec::with_capacity(bars);
        let mut energy = 0.0f32;
        for b in 0..bars {
            let r = Self::response(Self::level_for(d, b, bars), t.sensitivity);
            energy += r;
            resp.push(r);
        }
        energy = if bars > 0 { energy / bars as f32 } else { 0.0 };
        if !energy.is_finite() {
            energy = 0.0;
        }

        // Glitch slice: WHERE it is comes from the clock, HOW FAR it moves comes from the audio.
        // `time_s` and not a frame counter, per the project rule - the render loop's real period
        // drifts with load, so a counter-driven sweep would speed up and slow down with CPU.
        let time = if d.time_s.is_finite() { d.time_s } else { 0.0 };
        let sweeps = (time * GLITCH_HZ).floor();
        let phase = (time * GLITCH_HZ) - sweeps;
        let slice_y = field_top + (phase * field_h as f32) as i32;
        let slice_h = (field_h / 6).max(2);
        // Alternating direction per sweep, so it does not read as a permanent step in one place.
        let sign = if (sweeps as i64).rem_euclid(2) == 0 { 1 } else { -1 };
        let slice_dx = if t.pantone.glitch > 0.0 {
            sign * (t.pantone.glitch.clamp(0.0, 24.0) * (0.20 + 0.80 * energy)).round() as i32
        } else {
            0
        };
        let glitch_at = |y: i32| -> i32 {
            if y >= slice_y && y < slice_y + slice_h {
                slice_dx
            } else {
                0
            }
        };

        let diag_at = |y: i32| -> f32 {
            // Anchored so the line crosses the panel's horizontal centre halfway down the field,
            // which is the only placement that puts a comparable amount of panel either side of it
            // at both 190px and 380px.
            w as f32 * 0.5 + ((y - field_top) as f32 - field_h as f32 * 0.5) * DIAG_SLOPE
        };

        // ---- the light: bars, caps and peak rules, on their OWN transparent layer ----
        //
        // `Canvas::bloom` composites its halo UNDER what is already on the canvas, so blooming a
        // canvas that already carries the opaque panel leaves the halo invisible. Build the light
        // separately, bloom that, and draw it over the panel. Same trap already documented in
        // segmented/scope/vu/tube.
        let mut ink = Canvas::new(w, h);
        // Screen coverage inside a lit bar, on the far side of the diagonal. 0.62 keeps the dots
        // touching enough to read as a solid tint of the same ink rather than as a separate texture.
        let inner_cov = (0.34 + 0.40 * t.pantone.halftone.clamp(0.0, 1.0)).clamp(0.0, 1.0);

        for b in 0..bars {
            let r = resp[b];
            self.marker[b] = (self.marker[b] - MARKER_FALL).max(r);
            let lit = ((r * field_h as f32).round() as i32).clamp(0, field_h);
            let bx = ox + b as i32 * BAR_PITCH;
            let x01 = b as f32 / (bars as i32 - 1).max(1) as f32;
            // ONE colour path: `tint` resolves the rainbow (and its ink quantisation) or falls
            // back to the fixed `lit`/`hot` hex, exactly as every other family does. A colourway
            // with `rainbow = 0` therefore renders this family monochrome, which is what makes the
            // black-and-white barcode colourway possible without a second code path.
            let body = super::tint(t, x01, time, false, &t.lit, 1.0);
            let cap = super::tint(t, x01, time, true, &t.hot, 1.0);

            for row in 0..lit {
                let y = field_bot - 1 - row;
                let dx = glitch_at(y);
                let col = if row >= lit - CAP_ROWS { cap } else { body };
                let edge = diag_at(y);
                for k in 0..BAR_W {
                    let x = bx + k + dx;
                    // Solid on the near side of the diagonal, screened on the far side. Tested on
                    // the DISPLACED x so the diagonal moves with the glitch slice instead of the
                    // slice sliding out from under a stationary edge.
                    let solid = !t.pantone.split || (x as f32) < edge;
                    if solid || screened(x, y, inner_cov) {
                        ink.fill_rect(x, y, 1, 1, col);
                    }
                }
            }

            // Peak rule, only where it is ABOVE the column - a mark drawn on top of the bar it is
            // measuring says nothing. Two pixels of clearance so it reads as a separate rule.
            let pk = ((self.marker[b] * field_h as f32).round() as i32).clamp(0, field_h - 1);
            if pk > lit + 1 {
                let y = field_bot - 1 - pk;
                ink.fill_rect(bx + glitch_at(y), y, BAR_W, 1, cap);
            }
        }

        // Baseline rule under the columns: printed structure, and it also gives the eye the datum
        // the bar heights are read against.
        ink.fill_rect(2, field_bot, w - 4, 1, Rgba::from_hex(&t.hot, 0.45));

        if t.bloom > 0.0 {
            let mut glow = ink.clone();
            glow.bloom(t.bloom.round().max(0.0) as i32, t.glow_strength.clamp(0.0, 1.0));
            c.draw_over(&glow);
        }
        c.draw_over(&ink);

        // ---- flat ink, drawn crisp on top and never bloomed ----
        //
        // The screen and the barcode are flat ink, and flat ink does not glow. Putting either on
        // the bloomed layer smears the 4px lattice and the 1px stripes into a wash, which destroys
        // both the moire and the hard edges the misregistration needs to show against.

        // Dormant halftone in the unlit part of each column: a printed tone ramp, densest at the
        // baseline. Coverage is a function of HEIGHT only and carries no audio - the bars are the
        // meter, and a second thing responding to level in the same place competes with them.
        if t.pantone.halftone > 0.0 {
            for b in 0..bars {
                let lit = ((resp[b] * field_h as f32).round() as i32).clamp(0, field_h);
                let bx = ox + b as i32 * BAR_PITCH;
                let x01 = b as f32 / (bars as i32 - 1).max(1) as f32;
                let dot = super::tint(
                    t,
                    x01,
                    time,
                    false,
                    &t.lit,
                    (0.30 + 0.35 * t.pantone.halftone.clamp(0.0, 1.0)).clamp(0.0, 1.0),
                );
                for row in lit..field_h {
                    let y = field_bot - 1 - row;
                    let up = row as f32 / field_h as f32;
                    let cov = t.pantone.halftone.clamp(0.0, 1.0) * 0.75 * (1.0 - up);
                    let dx = glitch_at(y);
                    for k in 0..BAR_W {
                        let x = bx + k + dx;
                        if screened(x, y, cov) {
                            c.fill_rect(x, y, 1, 1, dot);
                        }
                    }
                }
            }
        }

        // Barcode band: hard achromatic rules of irregular width. Achromatic on purpose - it is the
        // KEY plate, and it is where the misregistration is most legible, because a mis-set plate
        // shows at a hard black-and-white edge far more than inside a coloured field.
        if bc_h > 0 {
            let ink_col = Rgba::from_hex(&t.hot, 0.92);
            let mut x = 2;
            let mut i = 0usize;
            while x < w - 2 {
                let sw = barcode_width(i).min(w - 2 - x);
                if i % 2 == 0 {
                    c.fill_rect(x, interior_top, sw, bc_h, ink_col);
                }
                x += sw;
                i += 1;
            }
        }

        // Clip back to the panel, same rect `rounded_rect` used, so the halo cannot escape onto the
        // bare taskbar past the rounded corners.
        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 4);

        let e = Rgba::from_hex(&t.edge, t.edge_alpha);
        c.fill_rect(1, 2, w - 2, 1, e);
        c.fill_rect(1, h - 3, w - 2, 1, e);

        // ---- misregistration, LAST ----
        //
        // After the clip, deliberately. `chromatic_aberration` never touches alpha, so it cannot
        // punch a hole in the panel (which would show the Windows weather widget through it) and
        // cannot make an off-panel pixel visible - so running it last is safe, and last is the only
        // place it does anything, because the plates need the opaque panel underneath to fringe
        // against.
        //
        // Widened by energy on top of the colourway's own amount. A display whose registration
        // degrades as it is driven harder is the "failing panel" half of this look; the 0.55 floor
        // keeps the fringe visible at silence so the colourway's identity does not depend on the
        // music.
        //
        // THE FLOURISH adds to the same offset rather than running a second pass. Two passes would
        // sample the first pass's own output, so the second one's red plate would be fringing against
        // an already-fringed image - the plates would separate by more than either shift asked for and
        // the amount would depend on their order.
        let fired = self.flourish.update(&d.levels, d.dt_ms, t.flourish);
        let slip = self.slip.update(fired, d.dt_ms, MISREG_MS);
        let base = if t.aberration.is_finite() && t.aberration != 0.0 {
            t.aberration * (0.55 + 0.75 * energy)
        } else {
            0.0
        };
        let dx = (base + slip * MISREG_X).round() as i32;
        let dy = (slip * MISREG_Y).round() as i32;
        if dx != 0 || dy != 0 {
            c.misregister(dx, dy);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::themes::builtin;

    /// The vertical lag at which one channel of `on` best matches the same channel of `off`.
    ///
    /// This measures the plate DISPLACEMENT itself rather than any consequence of it, which matters
    /// because the consequences are all ambiguous at this size: "more pixels where red and blue
    /// disagree" is also what a wider horizontal fringe produces, and this family already has one of
    /// those that grows with the music.
    fn plate_lag(on: &Canvas, off: &Canvas, chan: usize) -> i32 {
        let (w, h) = (on.width(), on.height());
        let mut best = (u64::MAX, 0i32);
        for lag in -6..=6 {
            let mut acc = 0u64;
            for y in 8..(h - 8) {
                for x in 4..(w - 4) {
                    let p = on.get(x, y);
                    let q = off.get(x, y + lag);
                    let (a, b) = match chan {
                        0 => (p.r, q.r),
                        1 => (p.g, q.g),
                        _ => (p.b, q.b),
                    };
                    acc += (a as i32 - b as i32).unsigned_abs() as u64;
                }
            }
            if acc < best.0 {
                best = (acc, lag);
            }
        }
        best.1
    }

    /// Renders the family with and without a flourish, from byte-identical audio.
    ///
    /// Fired by forcing the trigger directly. Two reasons, and the second one cost an hour:
    ///
    /// Not the audio firing sequence, because that sequence is a loud transient and this family's
    /// fringe width and glitch amplitude both follow energy - the arms would differ in ways that have
    /// nothing to do with the flourish.
    ///
    /// Not `flourish::request()` either, because `REQUEST` is a single process-global atomic that
    /// EVERY family's `draw` consumes. In a parallel suite an unrelated drawing test eats the request,
    /// and the symptom is bizarre: the effect provably fires with the right offset when the test runs
    /// alone, and the two canvases compare byte-identical when the whole suite runs.
    fn slip_pair(after: usize, aberration: f32) -> (Canvas, Canvas) {
        let mut t = builtin::all()
            .into_iter()
            .find(|t| t.family == "pantone")
            .expect("no pantone colourway");
        // Every shipped colourway in this family carries a standing horizontal aberration, so the
        // amount is a parameter: 0 isolates the vertical slip, and the colourway's own value is the
        // production case, where the flourish has to remain legible ON TOP of a fringe that is
        // already there.
        t.aberration = aberration;
        t.flourish = crate::themes::DEFAULT_FLOURISH;
        let run = |fire: bool| -> Canvas {
            let mut p = Pantone::default();
            let mut c = Canvas::new(190, 60);
            let mut d = flat(0.45);
            d.dt_ms = 16.7;
            for _ in 0..20 {
                p.draw(&mut c, &t, &d);
            }
            if fire {
                p.flourish.force_next();
            }
            for _ in 0..after {
                p.draw(&mut c, &t, &d);
            }
            c
        };
        (run(true), run(false))
    }

    #[test]
    fn the_flourish_slips_a_plate_out_of_register_vertically() {
        // Both arms, at one frame after firing where the envelope is full and the offset is its peak.
        // The second arm is the one that matters in production: the vertical lag must still read
        // cleanly with the colourway's own horizontal fringe underneath it.
        let standing = builtin::all()
            .into_iter()
            .find(|t| t.family == "pantone")
            .map(|t| t.aberration)
            .unwrap();
        assert!(standing != 0.0, "this test assumes the family fringes by default");
        // The expected lag is a LITERAL 3, not `MISREG_Y as i32`. Written against the constant, this
        // test passed with `MISREG_Y` mutated to zero - the expectation moved with the thing it was
        // supposed to be checking, which is the most comfortable kind of vacuous test.
        assert_eq!(
            MISREG_Y, 3.0,
            "the slip depth changed; update the expected plate lag in this test deliberately"
        );
        for ab in [0.0, standing] {
            let (on, off) = slip_pair(1, ab);
            assert_ne!(on.bits(), off.bits(), "the flourish changed nothing at aberration {ab}");
            assert_eq!(
                plate_lag(&on, &off, 0),
                3,
                "the red plate did not slip down 3px at aberration {ab}"
            );
            assert_eq!(
                plate_lag(&on, &off, 2),
                -3,
                "the blue plate did not slip up 3px at aberration {ab}"
            );
            assert_eq!(plate_lag(&on, &off, 1), 0, "the green plate moved at aberration {ab}");
        }
    }

    #[test]
    fn the_slip_lasts_long_enough_to_be_seen() {
        // A separate test because it catches a separate failure. `Envelope::update` sets the level to
        // 1.0 on the firing frame whatever its decay is set to, so the peak-offset test above passes
        // with `MISREG_MS` mutated to 1ms - an effect that would exist for a single frame at 60fps and
        // be invisible. Measured a third of a second in, where a 900ms envelope still has about 63% of
        // its level and the offset still rounds to 2px.
        let (on, off) = slip_pair(20, 0.0);
        assert!(
            plate_lag(&on, &off, 0) > 0,
            "the slip was gone 334ms after firing, so nobody would see it"
        );
    }

    #[test]
    fn the_plates_come_back_into_register() {
        // 60 frames is 1.0s against a 900ms envelope, so the slip has fully expired. Byte-identical,
        // which also proves the flourish leaves no residue in the family's own state.
        let standing = builtin::all()
            .into_iter()
            .find(|t| t.family == "pantone")
            .map(|t| t.aberration)
            .unwrap();
        for ab in [0.0, standing] {
            let (on, off) = slip_pair(60, ab);
            assert_eq!(on.bits(), off.bits(), "the plates never came back into register at {ab}");
        }
    }

    fn flat(level: f32) -> FrameData {
        let mut d = FrameData::default();
        d.levels = [level; crate::dsp::bands::NUM_BANDS];
        d.peaks = d.levels;
        d
    }

    /// A spectrum with one band peaking inside an otherwise quiet group.
    ///
    /// Every test that drives EVERY band to the same level is blind to the group reducer, because a
    /// group's mean equals its max there - the exact blind spot that let the valve row ship looking
    /// static. So the per-column assertions all use this.
    fn one_loud_band(bars: usize, loud_bar: usize, loud: f32, quiet: f32) -> FrameData {
        let mut d = flat(quiet);
        let n = d.levels.len();
        let lo = loud_bar * n / bars;
        let hi = ((loud_bar + 1) * n / bars).min(n);
        d.levels[(lo + hi) / 2] = loud;
        d.peaks = d.levels;
        d
    }

    /// An uneven, non-repeating spectrum, so no two neighbouring columns match.
    fn uneven() -> FrameData {
        let mut d = FrameData::default();
        for (i, v) in d.levels.iter_mut().enumerate() {
            let x = i as f32 / 63.0;
            *v = (0.14 + 0.55 * (x * 11.0).sin().abs()) * (1.0 - x * 0.35);
        }
        d.peaks = d.levels;
        d.rms_l = 0.09;
        d.rms_r = 0.06;
        d
    }

    fn lum(p: Rgba) -> f32 {
        (0.2126 * p.r as f32 + 0.7152 * p.g as f32 + 0.0722 * p.b as f32) * (p.a as f32 / 255.0)
    }

    /// Geometry mirrored from `draw`, so a test's sample points follow the constants rather than
    /// pinning the numbers they happen to have today.
    fn geometry(w: i32, h: i32, barcode: f32) -> (i32, i32, i32, usize, i32) {
        let interior_top = 3;
        let interior_bot = h - 5;
        let interior_h = interior_bot - interior_top;
        let mut bc_h = if barcode > 0.0 {
            (interior_h as f32 * barcode.clamp(0.0, 0.6)).round() as i32
        } else {
            0
        };
        if bc_h < 2 || interior_h - bc_h - 2 < MIN_FIELD {
            bc_h = 0;
        }
        let field_top = interior_top + if bc_h > 0 { bc_h + 2 } else { 0 };
        let field_h = (interior_bot - field_top).max(1);
        let bars = bar_count(w);
        let ox = 2 + ((w - 4) - bars as i32 * BAR_PITCH + BAR_GAP) / 2;
        (field_top, interior_bot, field_h, bars, ox)
    }

    /// Rows above the baseline at which column `b` carries its near-white CAP ink, i.e. the
    /// column's height. -1 when the column is dark.
    ///
    /// Thresholded at 170 luminance, and finding the CAP rather than any ink is the whole point of
    /// the number. Three things are drawn in the bar's own columns and they overlap in brightness:
    /// the body ink is a `chroma`-saturated hue, whose luminance runs from 94 (a blue-ish hue) to
    /// 216 (a yellow one); the dormant halftone screen reaches 122 at a yellow hue; and the cap and
    /// the peak rule are `hot`, which pulls to saturation 0.20 and therefore has EVERY channel at
    /// 203 or above, so it cannot go below 203 luminance at any hue. The first version of this
    /// helper thresholded at 110 and reported the same height at levels 0.05 and 0.20, because it
    /// was finding a yellow halftone dot in the dormant field rather than the bar at all.
    ///
    /// Also not thresholded on ALPHA: `panel_alpha` is 1.0, so alpha is 255 at every pixel inside
    /// the panel whether anything is drawn there or not, and an alpha assertion would silently
    /// measure the panel.
    fn column_height(c: &Canvas, t: &Theme, b: usize) -> i32 {
        let (field_top, field_bot, field_h, _, ox) = geometry(c.width(), c.height(), t.pantone.barcode);
        let bx = ox + b as i32 * BAR_PITCH;
        for row in (0..field_h).rev() {
            let y = field_bot - 1 - row;
            if y < field_top {
                continue;
            }
            for k in 0..BAR_W {
                if lum(c.get(bx + k, y)) > 170.0 {
                    return row;
                }
            }
        }
        -1
    }

    /// Inked pixels within column `b`'s own 6px width at row `y`, out of `BAR_W`.
    ///
    /// Threshold 50: the bare panel measures 7 and the DIMMEST solid ink 94, so this separates
    /// "inked" from "paper" at any hue, which a threshold anywhere near the ink's own range would
    /// not. Bounded to the bar's own columns so the 3px inter-bar gaps cannot be counted as holes -
    /// the first version of the diagonal-split test counted them and its two sides therefore
    /// differed by the gaps rather than by the screen.
    fn inked_in_bar(c: &Canvas, w: i32, h: i32, barcode: f32, b: usize, y: i32) -> i32 {
        let (_, _, _, _, ox) = geometry(w, h, barcode);
        let bx = ox + b as i32 * BAR_PITCH;
        (0..BAR_W).filter(|k| lum(c.get(bx + k, y)) > 50.0).count() as i32
    }

    fn render(t: &Theme, d: &FrameData, w: i32, h: i32) -> Canvas {
        let mut p = Pantone::default();
        let mut c = Canvas::new(w, h);
        // Two frames: the peak-hold allocates on the first and settles on the second.
        p.draw(&mut c, t, d);
        p.draw(&mut c, t, d);
        c
    }

    // ---- the audio response ----

    #[test]
    fn the_response_window_spends_its_range_on_the_levels_the_dsp_actually_produces() {
        // The input range is not 0..1: active bands sit around 0.15-0.65. A mapping that looks
        // right over a full sweep looks DEAD on music.
        let lo = Pantone::response(0.15, 1.0);
        let hi = Pantone::response(0.65, 1.0);
        assert!(hi - lo > 0.75, "the music window must cover most of the travel: {lo} -> {hi}");
        assert_eq!(Pantone::response(0.0, 1.0), 0.0, "silence must be zero, not a pedestal");
        assert_eq!(Pantone::response(1.0, 1.0), 1.0, "full scale must reach the top");
        assert!(
            Pantone::response(0.3, 2.0) > Pantone::response(0.3, 1.0),
            "sensitivity is the user-facing knob and must actually do something"
        );
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert!(Pantone::response(bad, 1.0).is_finite(), "{bad} must not escape the response");
        }
    }

    #[test]
    fn the_group_reducer_is_biased_toward_the_peak_not_the_mean() {
        // Guards GROUP_MAX_BIAS directly. One loud band among three quiet ones has a mean far below
        // its max, and a mean would flatten exactly the peak that distinguishes this column.
        let d = one_loud_band(20, 0, 0.9, 0.1);
        let n = d.levels.len();
        let hi = (n / 20).max(1);
        let mean = d.levels[..hi].iter().sum::<f32>() / hi as f32;
        let peak = d.levels[..hi].iter().copied().fold(0.0f32, f32::max);
        let got = Pantone::level_for(&d, 0, 20);
        assert!(got > mean + (peak - mean) * 0.5, "must sit above the midpoint: {got} in [{mean}, {peak}]");
        assert!(got <= peak + 1e-6, "but never above the peak itself: {got} vs {peak}");
    }

    #[test]
    fn a_column_grows_taller_as_its_band_drives_it() {
        // The POSITION cue, which is the primary one. Height, not brightness: a 1.16x brightness
        // spread between a driven element and its neighbour was measured on the valve row to be
        // below the visible threshold, which is why this family is a bar meter at all.
        let t = builtin::pantone_spectrum();
        let mut prev = -2;
        for level in [0.05f32, 0.20, 0.35, 0.50, 0.70] {
            let c = render(&t, &flat(level), 190, 60);
            let hgt = column_height(&c, &t, 4);
            assert!(hgt > prev, "level {level} must be taller than the level below it: {hgt} vs {prev}");
            prev = hgt;
        }
    }

    #[test]
    fn a_single_peaking_band_lifts_its_own_column_well_clear_of_its_neighbour() {
        // The case a mean reducer cannot see. Measured with a plain mean the driven column sat 4
        // rows above its neighbour on a 43-row field; biased toward the max it is over 20.
        let t = builtin::pantone_spectrum();
        let bars = bar_count(190);
        let d = one_loud_band(bars, 7, 0.65, 0.18);
        let c = render(&t, &d, 190, 60);
        let driven = column_height(&c, &t, 7);
        let neighbour = column_height(&c, &t, 8);
        assert!(
            driven > neighbour + 8,
            "a peaking band must clearly lift its own column: driven {driven} vs neighbour {neighbour}"
        );
    }

    #[test]
    fn an_uneven_spectrum_produces_a_visibly_uneven_profile() {
        // The single most mutation-sensitive assertion here: replace the response with any constant
        // and every column is the same height, so both the spread and the distinct-value count
        // collapse. An even spectrum could not see this.
        let t = builtin::pantone_spectrum();
        let c = render(&t, &uneven(), 190, 60);
        let bars = bar_count(190);
        let heights: Vec<i32> = (0..bars).map(|b| column_height(&c, &t, b)).collect();
        let lo = *heights.iter().min().unwrap();
        let hi = *heights.iter().max().unwrap();
        assert!(hi - lo > 12, "the profile is nearly flat: heights {heights:?}");
        let mut distinct = heights.clone();
        distinct.sort_unstable();
        distinct.dedup();
        assert!(distinct.len() >= 6, "expected many distinct heights, got {distinct:?}");
    }

    #[test]
    fn the_peak_rule_holds_above_a_column_after_the_level_drops() {
        // Sourced from the DISPLAYED response rather than FrameData.peaks - see MARKER_FALL. After
        // a drop the rule must still be near the old height, and there must be dark panel BETWEEN
        // it and the shrunken column: a mark drawn on the bar it is measuring says nothing.
        let t = builtin::pantone_spectrum();
        let (_, field_bot, field_h, _, _) = geometry(190, 60, t.pantone.barcode);
        let mut p = Pantone::default();
        let mut c = Canvas::new(190, 60);
        p.draw(&mut c, &t, &flat(0.70));
        let tall = column_height(&c, &t, 4);
        assert!(tall >= field_h - 2, "0.70 should drive the column to the top, got {tall}");

        p.draw(&mut c, &t, &flat(0.35));
        let held = column_height(&c, &t, 4);
        assert!(held >= tall - 3, "the rule must HOLD, not follow the level down: {held} vs {tall}");

        // The column itself is now around 20 rows; the gap in between must be bare panel.
        let mid = field_bot - 1 - (tall + 20) / 2;
        assert_eq!(
            inked_in_bar(&c, 190, 60, t.pantone.barcode, 4, mid),
            0,
            "the held rule must be separated from the column by bare panel at row y={mid}"
        );
        // And it must actually be a rule rather than a stuck column: the row below it is bare too.
        let just_below = field_bot - 1 - held + 1;
        assert_eq!(
            inked_in_bar(&c, 190, 60, t.pantone.barcode, 4, just_below),
            0,
            "the peak mark must be one row tall"
        );
    }

    // ---- the printed structure ----

    #[test]
    fn the_bars_carry_a_full_spectrum_gradient_across_the_panel() {
        // Edge to edge, not one flat colour: the hue at the left of the panel must differ from the
        // hue at the right. Compared as a HUE ORDERING rather than a luminance, because two
        // different hues can be equally bright.
        let t = builtin::pantone_spectrum();
        let c = render(&t, &flat(0.55), 190, 60);
        let (_, field_bot, _, bars, ox) = geometry(190, 60, t.pantone.barcode);
        let dominant = |b: usize| -> u8 {
            let bx = ox + b as i32 * BAR_PITCH;
            let p = c.get(bx + 1, field_bot - 3);
            if p.r >= p.g && p.r >= p.b {
                0
            } else if p.g >= p.b {
                1
            } else {
                2
            }
        };
        let mut seen: Vec<u8> = (0..bars).map(dominant).collect();
        seen.sort_unstable();
        seen.dedup();
        assert!(
            seen.len() >= 2,
            "the gradient must sweep hue across the width, saw only channel(s) {seen:?} dominant"
        );
    }

    #[test]
    fn the_barcode_band_is_hard_edged_and_irregular() {
        // Hard geometric structure, so the test is about EDGES and stripe widths - a soft or
        // uniformly striped band would pass a mere "is something drawn" check.
        let t = builtin::pantone_barcode();
        let c = render(&t, &flat(0.4), 190, 60);
        let (field_top, _, _, _, _) = geometry(190, 60, t.pantone.barcode);
        let y = 3 + (field_top - 3) / 3;
        let on: Vec<bool> = (3..187).map(|x| lum(c.get(x, y)) > 90.0).collect();
        let transitions = on.windows(2).filter(|p| p[0] != p[1]).count();
        assert!(transitions >= 12, "expected many hard stripe edges, saw {transitions}");
        // Irregular: collect the run lengths and require several different widths, or the band is
        // a comb rather than a barcode.
        let mut runs = Vec::new();
        let mut len = 1;
        for pair in on.windows(2) {
            if pair[0] == pair[1] {
                len += 1;
            } else {
                runs.push(len);
                len = 1;
            }
        }
        let mut widths = runs.clone();
        widths.sort_unstable();
        widths.dedup();
        assert!(widths.len() >= 3, "stripe widths must vary, got {widths:?}");
        // And there must be genuine paper between the rules, not a grey wash.
        let brightest = (3..187).map(|x| lum(c.get(x, y))).fold(0.0f32, f32::max);
        let darkest = (3..187).map(|x| lum(c.get(x, y))).fold(f32::MAX, f32::min);
        assert!(brightest - darkest > 100.0, "band is a wash: {darkest} to {brightest}");
    }

    #[test]
    fn the_halftone_field_is_a_lattice_and_not_a_flat_tint() {
        // A halftone that has been averaged into a tint is just a dim colour. The dormant field
        // must alternate along a row, and it must be denser at the baseline than near the top -
        // that ramp is what makes it read as printed tone.
        let t = builtin::pantone_halftone();
        let c = render(&t, &flat(0.0), 190, 60);
        let (field_top, field_bot, _, _, _) = geometry(190, 60, t.pantone.barcode);
        let count_at = |y: i32| (3..187).filter(|&x| lum(c.get(x, y)) > 30.0).count();
        let near_base = count_at(field_bot - 2);
        let near_top = count_at(field_top + 2);
        assert!(near_base > 0, "the screen must ink something at the baseline");
        assert!(
            near_base > near_top * 2,
            "coverage must ramp: {near_base} dots at the baseline vs {near_top} near the top"
        );
        // Alternating along the row, i.e. a lattice.
        let on: Vec<bool> = (3..187).map(|x| lum(c.get(x, field_bot - 2)) > 30.0).collect();
        let transitions = on.windows(2).filter(|p| p[0] != p[1]).count();
        assert!(transitions >= 20, "the screen is not a lattice, saw {transitions} transitions");
    }

    #[test]
    fn the_clustered_screen_grows_a_dot_rather_than_scattering_pixels() {
        // Guards the choice of SCREEN_4X4 over Bayer. At low coverage a clustered screen inks ONE
        // cell of the 4x4 and a dispersed one inks the corners furthest apart, so the test is
        // whether the first cells to light are adjacent.
        let lit: Vec<(i32, i32)> = (0..4)
            .flat_map(|y| (0..4).map(move |x| (x, y)))
            .filter(|&(x, y)| screened(x, y, 0.20))
            .collect();
        assert_eq!(lit.len(), 3, "coverage 0.20 of 16 cells should ink 3, got {lit:?}");
        // Every inked cell must touch another inked cell (8-connected) - a cluster, not a scatter.
        for &(x, y) in &lit {
            let touches = lit
                .iter()
                .any(|&(ox, oy)| (ox, oy) != (x, y) && (ox - x).abs() <= 1 && (oy - y).abs() <= 1);
            assert!(touches, "cell ({x},{y}) is isolated - this screen is dispersed, not clustered");
        }
        assert!(!screened(0, 0, 0.0), "zero coverage inks nothing");
        assert!(screened(3, 0, 1.0), "full coverage inks everything");
        assert!(!screened(0, 0, f32::NAN), "a poisoned coverage must ink nothing, not panic");
    }

    #[test]
    fn the_diagonal_split_divides_solid_ink_from_screened_ink() {
        // At the same row, a column on the near side of the diagonal must be solidly inked across
        // its full width and one on the far side must be broken by the screen. Measured WITHIN each
        // bar's own 6px width, never across the inter-bar gaps: the first version counted the gaps
        // as holes and so its two sides differed by bar pitch rather than by the screen.
        let mut solid_only = builtin::pantone_spectrum();
        solid_only.pantone.split = false;
        solid_only.pantone.glitch = 0.0;
        solid_only.aberration = 0.0;
        let mut split = solid_only.clone();
        split.pantone.split = true;

        let d = flat(0.9); // every column at full height, so both sides of the diagonal are lit
        let a = render(&solid_only, &d, 190, 60);
        let b = render(&split, &d, 190, 60);
        let (field_top, _, field_h, _, ox) = geometry(190, 60, split.pantone.barcode);
        // Mid-field, where `diag_at` sits at exactly w/2 = 95. Bar 4 spans x 42..48 and bar 15
        // spans 141..147, so they are unambiguously on opposite sides of it.
        let y = field_top + field_h / 2;
        assert!(ox + 4 * BAR_PITCH + BAR_W < 95, "bar 4 must sit left of the diagonal");
        assert!(ox + 15 * BAR_PITCH > 95, "bar 15 must sit right of it");

        let cov = |c: &Canvas, bar: usize| inked_in_bar(c, 190, 60, split.pantone.barcode, bar, y);
        assert_eq!(cov(&a, 4), BAR_W, "unsplit, a lit bar is solid across its width");
        assert_eq!(cov(&a, 15), BAR_W, "unsplit, so is the far one");
        assert_eq!(cov(&b, 4), BAR_W, "the near side of the split must stay solid");
        assert!(cov(&b, 15) < BAR_W, "the far side must be broken by the screen, got {}", cov(&b, 15));
        assert!(cov(&b, 15) > 0, "but screened, not blank, got {}", cov(&b, 15));
    }

    #[test]
    fn the_glitch_slice_displaces_a_band_of_rows_sideways_and_only_when_driven() {
        // Displacement, not brightness, and confined to a BAND of rows rather than the whole field.
        // Everything is compared against the identical frame at glitch = 0, so nothing else can
        // account for the difference.
        let mut on = builtin::pantone_misregister();
        on.pantone.split = false;
        on.pantone.halftone = 0.0; // so a lit bar is a solid block and its edge is unambiguous
        on.aberration = 0.0; // the fringes would move the very edges this measures
        on.bloom = 0.0; // and a halo would blur the edge past any sensible threshold
        let mut off = on.clone();
        off.pantone.glitch = 0.0;

        let mut d = flat(0.9);
        d.time_s = 0.5 / GLITCH_HZ; // mid-sweep, so the slice sits inside the field
        let driven = render(&on, &d, 190, 60);
        let base = render(&off, &d, 190, 60);
        let differing = (0..60).filter(|&y| (2..188).any(|x| driven.get(x, y) != base.get(x, y))).count();
        let (_, _, field_h, _, _) = geometry(190, 60, on.pantone.barcode);
        assert!(differing >= 2, "the glitch must displace a band of rows, {differing} rows differ");
        assert!(
            differing as i32 <= field_h / 3,
            "a band, not the whole field: {differing} of {field_h} rows differ"
        );

        // And the displacement's SIZE must come from the audio. Measured as how far a known bar's
        // left edge has moved, not by cross-correlation: a correlation against a 4px-periodic
        // halftone lattice matches equally well at every multiple of 4, and the first version of
        // this test duly reported a 12px shift for a frame whose slice had moved 2px.
        //
        // The slice is put near the BOTTOM of the field, because that is the only part of it that
        // is lit at both of the two levels compared.
        // Measured on the LEFTMOST column, which has nothing but bare panel to its left, so the
        // first inked pixel in the row is unambiguously that column's own left edge. Measuring an
        // interior bar does not work: the whole slice moves, so a search window wide enough to
        // contain the displacement also contains the neighbour that has moved into it, and the
        // first version of this returned the neighbour's edge instead.
        let edge_shift = |level: f32| -> i32 {
            let mut d = flat(level);
            d.time_s = 0.85 / GLITCH_HZ;
            let (_, field_bot, _, _, ox) = geometry(190, 60, on.pantone.barcode);
            let y = field_bot - 3; // inside the slice, and lit at any level above about 0.20
            let left = |c: &Canvas| (2..ox + 14).find(|&x| lum(c.get(x, y)) > 50.0);
            let a = render(&on, &d, 190, 60);
            let b = render(&off, &d, 190, 60);
            match (left(&a), left(&b)) {
                (Some(p), Some(q)) => p - q,
                _ => panic!("column 0 was not lit at level {level} - the sample row is wrong"),
            }
        };
        let low = edge_shift(0.30);
        let high = edge_shift(0.95);
        assert!(low > 0, "the slice must displace the bar at all, moved {low}px");
        assert!(
            high > low,
            "displacement must scale with energy: {low}px at level 0.30 vs {high}px at 0.95"
        );
        assert!(high >= 6, "at full drive it should reach most of its 8px, got {high}px");
    }

    // ---- misregistration ----

    #[test]
    fn the_aberration_leaves_a_red_fringe_on_one_side_and_a_blue_one_on_the_other() {
        // The identifying element. Measured as an actual channel imbalance at a bar edge, not as
        // "the output differs" - a test that only compared bits would pass on any change at all.
        let mut t = builtin::pantone_misregister();
        t.pantone.split = false;
        t.pantone.glitch = 0.0;
        t.pantone.halftone = 0.0;
        let mut clean = t.clone();
        clean.aberration = 0.0;

        let d = flat(0.9);
        let dirty_c = render(&t, &d, 190, 60);
        let clean_c = render(&clean, &d, 190, 60);
        let (_, field_bot, _, _, ox) = geometry(190, 60, t.pantone.barcode);
        let y = field_bot - 6;

        // Without aberration the gap between two bars is achromatic panel; with it, the gap on one
        // side of a bar goes red-dominant and the other blue-dominant.
        let mut red_fringe = 0;
        let mut blue_fringe = 0;
        for b in 1..bar_count(190) - 1 {
            let bx = ox + b as i32 * BAR_PITCH;
            for x in (bx - BAR_GAP)..bx {
                let p = dean(&dirty_c, x, y);
                let q = dean(&clean_c, x, y);
                if p.0 > p.2 + 40 && !(q.0 > q.2 + 40) {
                    red_fringe += 1;
                }
                if p.2 > p.0 + 40 && !(q.2 > q.0 + 40) {
                    blue_fringe += 1;
                }
            }
        }
        assert!(red_fringe > 0, "no red plate fringe appeared in any inter-bar gap");
        assert!(blue_fringe > 0, "no blue plate fringe appeared in any inter-bar gap");
    }

    /// (r, g, b) of a pixel as i32, for signed channel comparisons.
    fn dean(c: &Canvas, x: i32, y: i32) -> (i32, i32, i32) {
        let p = c.get(x, y);
        (p.r as i32, p.g as i32, p.b as i32)
    }

    #[test]
    fn the_five_colourways_are_visibly_different_from_one_another() {
        // Three of the five used to render almost identically, because `inks` and `ink_chroma` were
        // both INERT - the quantisation was expected to live in `themes::rainbow_hsv`, which predates
        // this family and honoured neither. `pantone-process` at three inks was drawing the exact
        // same continuous rainbow as `pantone-spectrum`.
        //
        // Measured as mean absolute channel difference over the whole panel. With the fields wired up
        // the closest pair is spectrum-vs-process at 24.6 and the rest span 39 to 83; before the fix
        // that pair was indistinguishable. The floor here is well under the closest real pair, so it
        // catches a colourway collapsing back onto another rather than policing taste.
        let ids = [
            "pantone-spectrum",
            "pantone-process",
            "pantone-barcode",
            "pantone-misregister",
            "pantone-halftone",
        ];
        let render = |id: &str| -> Vec<u8> {
            let t = builtin::all().into_iter().find(|t| t.id == id).expect(id);
            let mut p = Pantone::default();
            let mut c = Canvas::new(190, 60);
            for _ in 0..30 {
                p.draw(&mut c, &t, &flat(0.6));
            }
            (0..(190 * 60))
                .flat_map(|i| {
                    let px = c.get(i % 190, i / 190);
                    [px.r, px.g, px.b]
                })
                .collect()
        };
        let imgs: Vec<(&str, Vec<u8>)> = ids.iter().map(|id| (*id, render(id))).collect();
        for i in 0..imgs.len() {
            for j in (i + 1)..imgs.len() {
                let d: f64 = imgs[i]
                    .1
                    .iter()
                    .zip(imgs[j].1.iter())
                    .map(|(a, b)| (*a as i32 - *b as i32).unsigned_abs() as f64)
                    .sum::<f64>()
                    / imgs[i].1.len() as f64;
                assert!(
                    d > 12.0,
                    "{} and {} differ by only {d:.1} - one has collapsed onto the other",
                    imgs[i].0,
                    imgs[j].0
                );
            }
        }
    }

    #[test]
    fn misregistration_shifts_red_and_blue_in_opposite_directions() {
        // SECOND formulation. The first asserted "some pixel left of bar 4 has r > b + 40", which the
        // orange bar body satisfies unaided; my replacement compared the leftmost red and blue edges,
        // which are also unequal without any aberration because the bars are not grey. Both passed
        // with `chromatic_aberration` replaced by an outright no-op.
        //
        // Misregistration has one definition: red moves one way, blue moves the other, green stays.
        // So this compares an aberrated render against an un-aberrated one and asserts the channels
        // moved in OPPOSITE directions. A no-op leaves both offsets at zero and fails on the spot.
        const SHIFT: i32 = 3;
        let mut base = builtin::pantone_misregister();
        base.aberration = 0.0;
        let mut shifted = base.clone();
        shifted.aberration = SHIFT as f32;

        let render = |t: &Theme| {
            let mut p = Pantone::default();
            let mut c = Canvas::new(190, 60);
            for _ in 0..12 {
                p.draw(&mut c, t, &flat(0.85));
            }
            c
        };
        let a = render(&base);
        let b = render(&shifted);

        // Best-matching horizontal offset for each channel, over a row through the bars.
        let row = 42;
        let best = |pick: fn(&Rgba) -> u8| -> i32 {
            let mut best = (i64::MAX, 0);
            for off in -6..=6 {
                let mut err = 0i64;
                for x in 20..170 {
                    let src = x - off;
                    if src < 1 || src > 188 {
                        continue;
                    }
                    let d = pick(&b.get(x, row)) as i64 - pick(&a.get(src, row)) as i64;
                    err += d * d;
                }
                if err < best.0 {
                    best = (err, off);
                }
            }
            best.1
        };
        let dr = best(|p| p.r);
        let dg = best(|p| p.g);
        let db = best(|p| p.b);
        assert!(
            dr != 0 || db != 0,
            "neither channel moved, so the aberration did nothing: r {dr}, g {dg}, b {db}"
        );
        assert!(
            dr.signum() == -db.signum() && dr != 0 && db != 0,
            "red and blue must move in OPPOSITE directions: r {dr}, g {dg}, b {db}"
        );
        assert!(
            dg.abs() <= dr.abs().min(db.abs()),
            "green should move least of the three: r {dr}, g {dg}, b {db}"
        );
    }

    #[test]
    fn misregistration_is_wired_into_the_segmented_scope_and_vu_families_too() {
        // The three Pantone colourways on the pre-existing families would look identical to their
        // rainbow siblings if the aberration call were missing from any of those files - and each
        // one is a separate edit, so one test per family's own file would not catch a miss.
        for id in ["vfd-pantone", "scope-pantone", "vu-pantone"] {
            let t = builtin::all().into_iter().find(|t| t.id == id).unwrap();
            assert!(t.aberration > 0.0, "{id} must actually ask for misregistration");
            let mut flatted = t.clone();
            flatted.aberration = 0.0;

            let mut d = uneven();
            for (i, v) in d.waveform.iter_mut().enumerate() {
                *v = 0.5 * ((i as f32 / 22.0).sin() + 0.4 * (i as f32 / 7.0).sin());
            }
            d.rms_l = 0.10;
            d.rms_r = 0.07;

            let paint = |theme: &Theme| {
                let mut fam = super::super::family_for(&theme.family);
                let mut c = Canvas::new(190, 60);
                for _ in 0..4 {
                    fam.draw(&mut c, theme, &d);
                }
                c
            };
            let dirty = paint(&t);
            let clean = paint(&flatted);

            // Count pixels whose red/blue imbalance the aberration created. Not a bare "the bits
            // differ": that would pass if the two renders merely disagreed by rounding.
            let mut created = 0;
            for y in 4..56 {
                for x in 3..187 {
                    let (r, _, b) = dean(&dirty, x, y);
                    let (r0, _, b0) = dean(&clean, x, y);
                    if (r - b).abs() > (r0 - b0).abs() + 30 {
                        created += 1;
                    }
                }
            }
            assert!(created > 20, "{id}: only {created} pixels gained a colour fringe");
        }
    }

    // ---- containment, sizes and poisoned input ----

    #[test]
    fn no_pantone_colourway_leaves_a_transparent_pixel_inside_its_panel() {
        // Belt and braces beside the crate-wide sweep in `render::opacity`. A pixel below alpha 255
        // inside the panel is a hole the Windows weather widget shows through - which shipped once,
        // at 825 holes per frame, and was level-dependent, hence the sweep over levels here.
        for t in builtin::all().into_iter().filter(|t| t.family == "pantone") {
            let mut p = Pantone::default();
            for step in 0..=10 {
                let level = step as f32 / 10.0;
                let mut d = flat(0.0);
                for (i, v) in d.levels.iter_mut().enumerate() {
                    *v = level * (0.7 + 0.3 * ((i % 5) as f32 / 4.0));
                }
                d.peaks = d.levels;
                d.time_s = step as f32 * 0.37;
                let mut c = Canvas::new(190, 60);
                for _ in 0..6 {
                    p.draw(&mut c, &t, &d);
                }
                for y in 6..52 {
                    for x in 6..184 {
                        assert_eq!(
                            c.get(x, y).a,
                            255,
                            "{} at level {level} left a hole at ({x},{y})",
                            t.id
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn nothing_is_drawn_outside_the_rounded_panel() {
        let t = builtin::pantone_misregister();
        let c = render(&t, &flat(1.0), 190, 60);
        for x in 0..190 {
            assert_eq!(c.get(x, 0), Rgba::TRANSPARENT, "row 0 is above the panel, x={x}");
            assert_eq!(c.get(x, 59), Rgba::TRANSPARENT, "row 59 is below the panel, x={x}");
        }
        for y in 0..60 {
            assert_eq!(c.get(0, y), Rgba::TRANSPARENT, "column 0 is left of the panel, y={y}");
            assert_eq!(c.get(189, y), Rgba::TRANSPARENT, "column 189 is right of it, y={y}");
        }
    }

    #[test]
    fn renders_at_the_reference_wide_and_degenerate_sizes() {
        let d = uneven();
        for t in builtin::all().into_iter().filter(|t| t.family == "pantone") {
            for (w, h) in [(190, 60), (380, 60), (456, 60), (150, 48), (96, 40), (40, 24), (24, 18), (12, 12), (1, 1), (190, 3), (3, 60)] {
                let mut p = Pantone::default();
                let mut c = Canvas::new(w, h);
                for _ in 0..3 {
                    p.draw(&mut c, &t, &d);
                }
                assert_eq!(
                    c.bits().len(),
                    (w.max(0) * h.max(0)) as usize,
                    "{} at {w}x{h} changed the canvas size",
                    t.id
                );
            }
        }
    }

    #[test]
    fn a_wide_panel_adds_columns_instead_of_fattening_them() {
        assert_eq!(bar_count(190), 20, "the reference panel keeps its tuned 20 columns");
        assert_eq!(bar_count(380), 41, "double the width roughly doubles the columns");
        assert!(bar_count(40_000) <= 128, "capped");
        assert!(bar_count(20) >= 3, "and never fewer than a handful");
        // The moire depends on the 4px screen beating against a 9px pitch, so the pitch is what
        // must not drift with width.
        for w in [190, 240, 380, 456, 600] {
            let pitch = (w - 4) as f32 / bar_count(w) as f32;
            assert!(
                (pitch - BAR_PITCH as f32).abs() < 1.0,
                "at width {w} the bar pitch drifted to {pitch:.2} from {BAR_PITCH}"
            );
        }
    }

    #[test]
    fn survives_nan_and_infinity_everywhere_they_can_arrive() {
        // f32::clamp does NOT sanitise NaN, and this family carries a per-column peak-hold that a
        // single poisoned sample would corrupt for the rest of the process's life.
        let t = builtin::pantone_spectrum();
        for spoil in 0..4 {
            let mut d = uneven();
            match spoil {
                0 => {
                    d.levels[0] = f32::NAN;
                    d.levels[31] = f32::NAN;
                    d.peaks[5] = f32::NAN;
                    d.time_s = f32::NAN;
                    d.dt_ms = f32::NAN;
                }
                1 => {
                    d.levels[3] = f32::INFINITY;
                    d.levels[63] = f32::NEG_INFINITY;
                    d.time_s = f32::INFINITY;
                }
                2 => d.levels = [f32::NAN; crate::dsp::bands::NUM_BANDS],
                _ => {
                    d.rms_l = f32::NAN;
                    d.rms_r = f32::INFINITY;
                    d.dt_ms = 0.0;
                }
            }
            let mut p = Pantone::default();
            let mut c = Canvas::new(190, 60);
            for _ in 0..4 {
                p.draw(&mut c, &t, &d);
            }
            // And it must RECOVER: a clean frame after a poisoned one has to render normally,
            // which is the part a mere "did not panic" test misses.
            p.draw(&mut c, &t, &flat(0.7));
            p.draw(&mut c, &t, &flat(0.7));
            assert!(column_height(&c, &t, 4) > 5, "spoil {spoil} left the column permanently dead");
        }
    }

    #[test]
    fn a_poisoned_theme_does_not_break_the_family() {
        // Theme files are user-authored, so every one of these is reachable from a TOML typo.
        let mut t = builtin::pantone_spectrum();
        t.aberration = f32::NAN;
        t.pantone.glitch = f32::NAN;
        t.pantone.halftone = f32::NAN;
        t.pantone.barcode = f32::NAN;
        t.ink_chroma = f32::NAN;
        t.sensitivity = f32::NAN;
        let mut p = Pantone::default();
        let mut c = Canvas::new(190, 60);
        for _ in 0..3 {
            p.draw(&mut c, &t, &uneven());
        }
        let mut wild = builtin::pantone_spectrum();
        wild.aberration = 1e9;
        wild.pantone.glitch = 1e9;
        wild.pantone.barcode = 9.0;
        wild.inks = u32::MAX;
        for _ in 0..3 {
            p.draw(&mut c, &wild, &uneven());
        }
        assert_eq!(c.bits().len(), 190 * 60);
    }

    #[test]
    fn every_pantone_colourway_renders_and_differs() {
        let mut seen: Vec<Vec<u32>> = Vec::new();
        let ids: Vec<String> = builtin::all()
            .iter()
            .filter(|t| t.family == "pantone")
            .map(|t| t.id.clone())
            .collect();
        for t in builtin::all().into_iter().filter(|t| t.family == "pantone") {
            let c = render(&t, &uneven(), 190, 60);
            let bits = c.bits().to_vec();
            assert!(bits.iter().any(|p| *p != 0), "{} rendered nothing", t.id);
            for prior in &seen {
                assert_ne!(prior, &bits, "{} renders identically to another colourway", t.id);
            }
            seen.push(bits);
        }
        assert_eq!(seen.len(), 5, "expected five pantone colourways, got {ids:?}");
    }

    #[test]
    fn a_colourway_with_the_rainbow_off_renders_achromatic() {
        // The barcode-dominant colourway is monochrome by having `rainbow = 0`, which makes `tint`
        // fall through to the fixed `lit` hex - one colour path, not two. If the family ever grew
        // its own hue computation this would go coloured.
        let t = builtin::pantone_barcode();
        assert_eq!(t.rainbow, 0.0, "this colourway is meant to be achromatic");
        // Measured with the misregistration OFF. With it on, this colourway is the most colourful
        // of the five at the pixel level and that is not a contradiction: white ink against a
        // near-black panel is the strongest possible edge for a mis-set plate, so a 3px offset
        // there produces saturated red and blue fringes out of purely grey ink. Those fringes are
        // the aberration's own test's business; this one is about the COLOUR PATH.
        let mut grey = t.clone();
        grey.aberration = 0.0;
        let c = render(&grey, &uneven(), 190, 60);
        let mut worst = 0i32;
        for y in 4..56 {
            for x in 3..187 {
                let (r, g, b) = dean(&c, x, y);
                worst = worst.max((r - g).abs().max((g - b).abs()).max((r - b).abs()));
            }
        }
        assert!(worst < 24, "expected grey ink, worst channel spread {worst}");

        // ...and with it on, the same grey ink must genuinely produce colour, which is the point.
        let fringed = render(&t, &uneven(), 190, 60);
        let coloured = (4..56)
            .flat_map(|y| (3..187).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let (r, _, b) = dean(&fringed, x, y);
                (r - b).abs() > 60
            })
            .count();
        assert!(coloured > 40, "achromatic ink must still fringe, only {coloured} pixels did");
    }

    /// One dump pixel, composited over background grey 22 as the other dump harnesses do.
    ///
    /// The multiply by alpha is the one departure from them, and it is a fix. `Canvas::get` returns
    /// STRAIGHT colour, so a bezel row at alpha 41 comes back as pure white - and
    /// `ch + 22 * (1 - a)`, which every existing harness in this crate uses, therefore writes 255
    /// for it instead of 41. The effect is confined to translucent pixels, which inside these
    /// panels means only the 1px bezel rows and the clipped corners, but it makes the bezel the
    /// brightest thing in the image and this family's whole point is judging edges by eye.
    fn over_grey_22(c: &Canvas, x: i32, y: i32) -> [u8; 4] {
        let px = c.get(x, y);
        let a = px.a as f32 / 255.0;
        let mix = |ch: u8| (ch as f32 * a + 22.0 * (1.0 - a)).clamp(0.0, 255.0) as u8;
        [mix(px.r), mix(px.g), mix(px.b), 255]
    }

    /// Run: cargo test --release dump_pantone_frames -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_pantone_frames() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();
        let mut n = 0usize;
        for t in builtin::all().into_iter().filter(|t| t.family == "pantone") {
            for (tag, w, h) in [("190x60", 190, 60), ("380x60", 380, 60)] {
                let mut p = Pantone::default();
                let mut c = Canvas::new(w, h);
                // 40 frames of a moving spectrum, so the peak rules are held and the glitch slice
                // has swept into the field.
                for k in 0..40 {
                    let mut d = FrameData::default();
                    let tt = k as f32 / 40.0;
                    for (i, v) in d.levels.iter_mut().enumerate() {
                        let x = i as f32 / 63.0;
                        *v = (0.14 + 0.55 * ((x * 9.0 + tt * 6.0).sin().abs())) * (1.0 - x * 0.30);
                    }
                    d.peaks = d.levels;
                    d.rms_l = 0.10;
                    d.rms_r = 0.07;
                    d.dt_ms = 16.7;
                    d.time_s = 1.1 + k as f32 * 0.0167;
                    p.draw(&mut c, &t, &d);
                }
                let mut out = Vec::with_capacity((w * h * 4) as usize);
                for y in 0..h {
                    for x in 0..w {
                        out.extend_from_slice(&over_grey_22(&c, x, y));
                    }
                }
                std::fs::write(dir.join(format!("pantone-{}-{tag}.rgba", t.id)), &out).unwrap();
                n += 1;
            }
        }
        // The three Pantone colourways on the pre-existing families, so the wired-in
        // misregistration can be eyeballed against its rainbow sibling.
        for id in ["vfd-pantone", "scope-pantone", "vu-pantone", "rgb-wave"] {
            let t = builtin::all().into_iter().find(|t| t.id == id).unwrap();
            let mut fam = super::super::family_for(&t.family);
            let mut c = Canvas::new(190, 60);
            for k in 0..40 {
                let mut d = uneven();
                d.time_s = 1.1 + k as f32 * 0.0167;
                for (i, v) in d.waveform.iter_mut().enumerate() {
                    *v = 0.5 * ((i as f32 / 22.0).sin() + 0.35 * (i as f32 / 7.0).sin());
                }
                d.rms_l = 0.10;
                d.rms_r = 0.07;
                fam.draw(&mut c, &t, &d);
            }
            let mut out = Vec::new();
            for y in 0..60 {
                for x in 0..190 {
                    out.extend_from_slice(&over_grey_22(&c, x, y));
                }
            }
            std::fs::write(dir.join(format!("pantone-on-{id}.rgba")), &out).unwrap();
            n += 1;
        }
        println!("wrote {n} pantone dumps to {}", dir.display());
    }

    /// Measurement, not an assertion. Prints the column height across the DSP's real window, for
    /// this family's mapping and for a naive linear 0..1 one - the numbers quoted on RESP_FLOOR
    /// came from here. Run:
    /// cargo test --release probe_pantone_travel -- --ignored --nocapture
    #[test]
    #[ignore]
    fn probe_pantone_travel() {
        let t = builtin::pantone_spectrum();
        let (_, _, field_h, _, _) = geometry(190, 60, t.pantone.barcode);
        println!("field is {field_h} rows tall");
        println!("level  windowed-rows  linear-rows");
        for level in [0.0f32, 0.10, 0.15, 0.25, 0.35, 0.45, 0.55, 0.65, 0.80, 1.0] {
            let c = render(&t, &flat(level), 190, 60);
            let windowed = column_height(&c, &t, 4);
            let linear = (level * field_h as f32).round() as i32;
            println!("{level:5.2}  {windowed:13}  {linear:11}");
        }
        // And the group reducer, on the case that matters.
        let bars = bar_count(190);
        let d = one_loud_band(bars, 7, 0.65, 0.20);
        let c = render(&t, &d, 190, 60);
        println!(
            "one loud band: driven column {} rows, neighbour {} rows",
            column_height(&c, &t, 7),
            column_height(&c, &t, 8)
        );
        let mean_only = {
            let mut acc = 0.0;
            let n = d.levels.len();
            let hi = (n / bars).max(1);
            for v in &d.levels[7 * n / bars..(7 * n / bars + hi)] {
                acc += *v;
            }
            acc / hi as f32
        };
        println!(
            "  (a mean-only reducer would have given level {:.3} -> {} rows)",
            mean_only,
            (Pantone::response(mean_only, 1.0) * field_h as f32).round() as i32
        );
    }
}
