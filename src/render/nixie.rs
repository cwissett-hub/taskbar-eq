//! The nixie-tube family: a row of glass envelopes, each reading its band as a lit DIGIT.
//!
//! A cold-cathode counterpart to `tube`. The valve row is an ANALOGUE instrument - a light that
//! swells inside the glass - and this is the digital one: inside every envelope sits a stack of
//! ten cathode glyphs, 0 at the bottom and 9 at the top, and the one matching the band's level
//! is struck in neon while the other nine sit as faint unlit wire. Exactly like a real IN-12.
//!
//! Three things do the heavy lifting:
//!
//! - **The cue is WHICH digit is lit, not how bright it is.** That is the whole reason to build
//!   this family: the position cue is free (rule 6 of the house style, and the valve row's
//!   measured 1.16x brightness spread is what motivated it). A digit climbing a ten-step stack
//!   is a cue you read in one glance, and it does not compete with the bloom for legibility.
//! - **The unlit cathodes are drawn, not omitted.** A single floating digit reads as a
//!   seven-segment LED; the dim ghost stack behind it is what makes it a nixie, and it also gives
//!   the eye a SCALE to read the lit digit's height against. `Theme.ghost` sets its alpha.
//! - **Fewer, wider tubes than the valve row.** A legible digit needs a 3px-wide glyph plus glass
//!   around it, so this family runs a 27px pitch against the valve row's 19px - 7 tubes at 190px
//!   where the valves get 10. Each tube covers more of the spectrum in exchange for a readout you
//!   can actually resolve, which is the trade this family exists to make.
//!
//! The digit stack FILLS the envelope at the reference size, with no slack: ten 5px cells is 50 of
//! the 60 available rows once the panel inset, the base and the pins are paid for. That is not a
//! coincidence to be tidied up later - it is why the envelope is a rounded rect rather than the
//! valve row's dome. A dome deep enough to read as glass eats 8-9 rows at the top, i.e. two whole
//! digits; corner rounding on a 17px-wide envelope only cuts pixels the 3px digit column never
//! occupies, so it buys the silhouette for free.

use super::canvas::{Canvas, Rgba};
use super::{Family, FrameData};
use crate::themes::Theme;

/// Band level at which the readout leaves 0, and the span it counts up over.
///
/// The same window as `tube::RESP_FLOOR`/`RESP_SPAN`, and for the same measured reason: real
/// music delivers roughly 0.15-0.65 per active band, so a mapping spread over the full 0..1
/// would spend two thirds of its range on levels that never arrive. Here the cost is not
/// dimness but SILENCE: over 0..1 a band at 0.35 lights digit 3 and the whole row would sit in
/// the bottom third of every stack, never once reaching the digits at the top. Across this
/// window the same 0.15-0.65 sweep walks the readout from digit 0 to digit 9.
///
/// Fixed, not a peak follower, for the reason `tube` spells out: this is a level meter, and a
/// follower would show the same band at a different digit depending on what came before it.
const RESP_FLOOR: f32 = 0.10;
const RESP_SPAN: f32 = 0.52;

/// Weight given to a group's LOUDEST band rather than its mean.
///
/// Carried over from `tube` (where a plain mean measured 1.46 dL* between a driven element and
/// its neighbour, below the ~2.3 dL* visible threshold), and it matters MORE here, not less:
/// seven tubes across 64 bands is about 9 bands each, so a mean has half again as many quiet
/// bands to dilute a peak with. Digits quantise on top of that - at nine steps a diluted group
/// often does not clear the next digit's threshold at all, so the tube simply never moves.
const GROUP_MAX_BIAS: f32 = 0.65;

/// Rows per digit cell: exactly the 3x5 font's glyph height, with no leading.
///
/// Ten cells at 6px would need 60 rows and there are only ~50 to spend, so the glyphs touch
/// vertically. That reads as authentic rather than as a bug - the unlit cathodes in a real nixie
/// genuinely do overlap into a confusing wire mesh when you look into the tube - and the LIT
/// digit is still unambiguous because it is many times brighter than its neighbours.
const CELL_H: i32 = 5;

/// Digits in a full stack, and the fewest a shrunken panel may show.
///
/// Below the reference height the stack loses digits rather than squeezing the glyphs: a cell
/// under 5 rows cannot render the font at all, so it would silently draw nothing. Fewer digits
/// is a coarser meter that still works; a sub-5px cell is a blank tube.
const MAX_DIGITS: i32 = 10;
const MIN_DIGITS: i32 = 3;

/// Pitch one nixie wants, in pixels.
///
/// 27 against the valve row's 19. A nixie's readable content is a 3px glyph, so the envelope has
/// to be wide enough that the glyph sits in glass rather than filling it - at the valve row's
/// pitch the 11px envelope leaves 4px of glass either side of the digit and the tube reads as a
/// character cell in a dot-matrix display, not as a bottle with something burning inside it.
const TUBE_PITCH: i32 = 27;

/// How far the peak afterglow falls per reference frame, in DIGITS.
///
/// Sourced from the displayed digit position, not from `FrameData.peaks`, for the reason `tube`
/// documents: the shared peak-hold falls at 0.0055/frame, so under continuous music it would pin
/// the afterglow near the top of every stack and add a bright feature that makes the row read
/// MORE uniform. At 0.09 digits/frame a transient's afterglow decays over about 1.7s from the top
/// of the stack, which is roughly how long a struck cathode stays visible to a dark-adapted eye.
const MARKER_FALL: f32 = 0.09;

/// Peak alpha of the ADJACENT digit, faded in so the readout is not a hard ten-step staircase.
///
/// A real multiplexed nixie clock cannot do this; a real nixie BARGRAPH tube (an IN-9/IN-13) is
/// continuous. It is here because ten steps over the response window is a 0.058-level quantum, and
/// on sustained material a band can sit inside one digit's span for seconds at a time - the tube
/// then looks frozen while the music is plainly moving.
///
/// The pair is centred on the NEAREST digit, not on the one below: the fade runs from the nearest
/// digit toward whichever neighbour the level is leaning to, so the brightest digit is always the
/// digit the level rounds to. The first version lit `floor(pos)` and faded in `floor(pos) + 1`,
/// which spends the whole top half of every span with the reading one digit low.
const CROSSFADE: f32 = 0.34;
/// How far the nearest digit dims when the level sits midway to its neighbour.
///
/// Both constants are bounded by something easy to miss: the glyphs do not all have the same AREA.
/// A '7' lights 7 of its 15 pixels and an '8' lights 13, so a neighbour drawn at a merely lower
/// alpha can still put MORE light in its cell than the nearest digit - and then the brightest cell,
/// which is this family's only cue, points at the wrong number. Measured, that is not theoretical:
/// at CROSSFADE 0.55 / PRIMARY_DIP 0.35 the 7-to-8 boundary read 8 while the level said 7.98, and
/// even at 0.38/0.15 it still did. 0.34 against a floor of 0.85 is an alpha ratio of 0.40, which
/// survives the worst glyph-area ratio in the set (13/7 = 1.86) with margin, and only reaches that
/// ratio exactly halfway between two digits - where either reading is honest anyway.
const PRIMARY_DIP: f32 = 0.15;

/// Glyphs for the stack. Indexed by digit, so `cell_y(d)` and this agree by construction.
///
/// `Canvas`'s 3x5 font shipped with 1-6 only (the meter labels never needed the rest); 0, 7, 8
/// and 9 were added to `canvas.rs` for this family, leaving every pre-existing glyph byte for
/// byte as it was.
const DIGIT_STR: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];

/// Tubes to draw at a given panel width.
///
/// Scaled, not stretched, for the reason measured on the valve row: a fixed count at 380px grew
/// the glass to 20px and the row read as arched windows. Widening a nixie is worse than widening
/// a valve, because the glyph inside it does NOT scale - a 3px digit in a 40px envelope is a
/// speck in a jar.
fn tube_count(w: i32) -> usize {
    ((w / TUBE_PITCH).max(2) as usize).min(20)
}

/// Everything the geometry decides, computed once so `draw`, the envelope clip and the tests
/// cannot disagree about where a digit is.
///
/// That disagreement is a real bug this project has already paid for twice: `tube`'s peak rim was
/// drawn outside its own silhouette closure and flattened the dome, and `segmented`'s tests
/// sampled at `PAD_X + 2` while `draw` centred the grid, so they measured the margin. Tests here
/// ask the same `Geom` the renderer used.
struct Geom {
    tubes: usize,
    margin: i32,
    pitch: f32,
    /// Odd, so the envelope has a true centre column for the digit to sit on.
    glass_w: i32,
    top: i32,
    glass_h: i32,
    base_y: i32,
    base_h: i32,
    radius: i32,
    digits: i32,
    stack_top: i32,
}

impl Geom {
    /// `None` when the panel cannot hold a legible stack, in which case `draw` renders the bare
    /// panel. A row of 2px-tall unreadable smudges is worse than an honest blank.
    fn new(w: i32, h: i32) -> Option<Geom> {
        if w < 28 || h < 24 {
            return None;
        }
        let margin = 4;
        let tubes = tube_count(w);
        let pitch = ((w - margin * 2) as f32 / tubes as f32).max(10.0);
        let glass_w = ((pitch * 0.62) as i32).max(7) | 1;
        let top = 3;
        // 4 rows of base at the reference height, 3 when there is no room - the base carries the
        // collar row plus pins, and at 2 it degenerates to a bar with no pins visible.
        let base_h = if h >= 46 { 4 } else { 3 };
        let base_y = h - 3 - base_h;
        let glass_h = base_y - top;
        if glass_h < MIN_DIGITS * CELL_H {
            return None;
        }
        let digits = (glass_h / CELL_H).clamp(MIN_DIGITS, MAX_DIGITS);
        // Centred in the envelope, so a taller panel gets margin above and below the stack rather
        // than a stack pinned to the top with a gap under it.
        let stack_top = top + (glass_h - digits * CELL_H).max(0) / 2;
        // Must match what `rounded_rect` will actually clamp to, or the clip and the drawn
        // silhouette diverge - see `half_at`.
        let radius = 4.min(glass_w.min(glass_h) / 2).max(0);
        Some(Geom {
            tubes,
            margin,
            pitch,
            glass_w,
            top,
            glass_h,
            base_y,
            base_h,
            radius,
            digits,
            stack_top,
        })
    }

    fn cx(&self, i: usize) -> i32 {
        self.margin + (self.pitch * (i as f32 + 0.5)) as i32
    }

    /// Top row of digit `d`'s cell, `d = 0` at the bottom of the stack.
    fn cell_y(&self, d: i32) -> i32 {
        self.stack_top + (self.digits - 1 - d.clamp(0, self.digits - 1)) * CELL_H
    }

    /// Half-width of the envelope at row `y`, or 0 where there is no envelope.
    ///
    /// This is `Canvas::rounded_rect`'s own corner arithmetic, re-run here rather than
    /// approximated, so the glow clip, the rim highlight and the drawn envelope all describe
    /// exactly the same shape. Approximating it with a bounding box is how a halo ends up
    /// floating in a corner the glass does not occupy.
    fn half_at(&self, y: i32) -> i32 {
        let ly = y - self.top;
        if ly < 0 || ly >= self.glass_h {
            return 0;
        }
        let r = self.radius;
        let dy = if ly < r {
            r - ly
        } else if ly >= self.glass_h - r {
            ly - (self.glass_h - r - 1)
        } else {
            0
        };
        let inset = if dy > 0 {
            let f = (r * r - dy * dy).max(0) as f32;
            r - f.sqrt().round() as i32
        } else {
            0
        };
        (self.glass_w / 2 - inset).max(0)
    }
}

#[derive(Default)]
pub struct Nixie {
    /// Fast-falling peak hold per tube, in DIGIT units (not levels), so the afterglow lands on a
    /// cell boundary the same way the live digit does.
    ///
    /// A `Vec` because `tube_count` scales to 20 and `#[derive(Default)]` has no impl for
    /// `[f32; 20]` - std's array impls stop at 32 only for some traits, and Default is not one.
    marker: Vec<f32>,
}

impl Nixie {
    /// Level feeding one tube: the group mean blended toward the group MAX.
    ///
    /// Non-finite bands are dropped rather than clamped, because `f32::clamp` does NOT sanitise
    /// NaN - every comparison against NaN is false, so it returns the NaN untouched and it goes
    /// on to poison the digit index and the peak marker. This project has been bitten by that
    /// twice.
    fn level_for(d: &FrameData, i: usize, tubes: usize) -> f32 {
        let n = d.levels.len();
        let tubes = tubes.max(1);
        let lo = i * n / tubes;
        let hi = (((i + 1) * n / tubes).max(lo + 1)).min(n);
        let (mut acc, mut cnt, mut peak) = (0.0f32, 0.0f32, 0.0f32);
        for v in &d.levels[lo..hi] {
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

    /// Maps a group level onto 0..1 of the stack. `sensitivity` is the TOML-facing gain.
    fn response(level: f32, sensitivity: f32) -> f32 {
        if !level.is_finite() {
            return 0.0;
        }
        let s = if sensitivity.is_finite() { sensitivity.max(0.0) } else { 1.0 };
        (((level - RESP_FLOOR) / RESP_SPAN) * s).clamp(0.0, 1.0)
    }

    /// One struck cathode: a discharge cloud with the glyph burning inside it.
    ///
    /// The cloud goes down FIRST and the glyph over it, because a nixie's neon does not sit in
    /// front of the wire - it clings to it. Drawing the halo last washed the glyph out into an
    /// orange blob at these alphas, which loses the digit and with it the only cue this family
    /// has.
    fn strike(lit: &mut Canvas, g: &Geom, cx: i32, digit: i32, t: &Theme, a: f32) {
        let a = a.clamp(0.0, 1.0);
        if a <= 0.004 {
            return;
        }
        let y = g.cell_y(digit);
        // Wider than tall, and both fixed: the cloud must not become a second, competing size
        // cue. rx is tied to the envelope so the glow fills the glass (a real tube's discharge
        // does) without reaching the neighbouring tube - 0.36 of a 17px envelope is 6.1px against
        // a 9px gap between envelopes. ry is deliberately UNDER one cell: at 0.95 of a cell the
        // cloud reached a whole digit up and down, so the neighbour's cell measured almost as much
        // light as the struck one and the position cue smeared. The soft spread is the bloom's job.
        lit.elliptical_gradient(
            cx,
            y + CELL_H / 2,
            (g.glass_w as f32 * 0.36).max(2.0),
            (CELL_H as f32 * 0.62).max(2.0),
            &[
                (0.0, Rgba::from_hex(&t.lit, 0.42 * a)),
                (0.55, Rgba::from_hex(&t.lit, 0.18 * a)),
                (1.0, Rgba::from_hex(&t.lit, 0.0)),
            ],
        );
        let s = DIGIT_STR[digit.clamp(0, 9) as usize];
        // Glyph left at cx - 1 centres the 3px cell on the envelope's true centre column.
        lit.text_3x5(cx - 1, y, s, Rgba::from_hex(&t.lit, a));
        // A hotter core over the same pixels, so the struck digit is near-white in the middle and
        // neon at its edges, as a cathode actually is. Not a separate shape - at 3x5 there is no
        // room for one.
        lit.text_3x5(cx - 1, y, s, Rgba::from_hex(&t.hot, 0.45 * a));
    }
}

impl Family for Nixie {
    fn id(&self) -> &'static str {
        "nixie"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());
        c.clear();
        let panel_r = 3;
        c.rounded_rect(
            1,
            2,
            (w - 2).max(1),
            (h - 4).max(1),
            panel_r,
            Rgba::from_hex(&t.panel, t.panel_alpha),
        );

        let Some(g) = Geom::new(w, h) else {
            // Panel only. Everything below assumes a legible stack exists.
            return;
        };

        // Chassis: a top-lit plate, so the tubes read as bolted THROUGH something rather than
        // floating on a flat background.
        c.vertical_gradient(
            2,
            3,
            w - 4,
            h - 6,
            &[
                (0.0, Rgba::from_hex(&t.tube.chassis_top, 0.50)),
                (1.0, Rgba::from_hex(&t.tube.chassis_bottom, 0.50)),
            ],
            true,
        );

        if self.marker.len() != g.tubes {
            self.marker.resize(g.tubes, 0.0);
        }
        // Frame-rate independent, unlike the older families (whose ballistics were tuned
        // per-frame and are deliberately left alone). Clamped as well as checked for finiteness:
        // a stalled frame reporting 400ms would drop the afterglow two whole digits in one step,
        // which looks like a dropped frame rather than a decay.
        let dt = if d.dt_ms.is_finite() { d.dt_ms.clamp(4.0, 64.0) } else { 16.7 };
        let fall = MARKER_FALL * dt / 16.7;

        // Behind the light: the dark interior of the glass, the unlit cathode stack and the anode
        // rails. All crisp and dim, and all UNDER the discharge - they are metal, not emitters, so
        // they must not be fed to `bloom`.
        for i in 0..g.tubes {
            let cx = g.cx(i);
            for y in g.top..g.base_y {
                let half = g.half_at(y);
                if half <= 0 {
                    continue;
                }
                // Darker than the chassis, so the envelope reads as a void with wire in it. This
                // is also what gives the ghost cathodes something to be legible against - on the
                // bare chassis gradient they measured within a few luminance of their background.
                c.fill_rect(cx - half, y, half * 2 + 1, 1, Rgba::from_hex(&t.tube.internals, 0.55));
            }
            // The anode grid's side rails. Two hairlines rather than the real honeycomb mesh: a
            // mesh at 1px pitch over a 3px glyph is indistinguishable from noise at this size, and
            // it competed with the digit for the same pixels.
            let rail = (g.glass_w / 2 - 2).max(1);
            for dx in [-rail, rail] {
                c.fill_rect(
                    cx + dx,
                    g.top + 2,
                    1,
                    (g.base_y - g.top - 3).max(1),
                    Rgba::from_hex(&t.tube.collar, 0.16),
                );
            }
        }

        // The unlit cathodes. Drawn for every digit including the one about to be struck: the lit
        // glyph lands on exactly the same pixels and simply overwrites it, and skipping it would
        // mean the stack changes shape as the level moves.
        if t.ghost > 0.0 {
            for i in 0..g.tubes {
                let cx = g.cx(i);
                for dd in 0..g.digits {
                    c.text_3x5(
                        cx - 1,
                        g.cell_y(dd),
                        DIGIT_STR[(dd as usize).min(9)],
                        Rgba::from_hex(&t.lit, t.ghost.clamp(0.0, 1.0) * 0.85),
                    );
                }
            }
        }

        // Everything that EMITS goes on its own transparent layer.
        //
        // `Canvas::bloom` composites its halo UNDER the existing content, so blooming in place on
        // the opaque chassis hides it completely - the trap already documented in segmented, scope,
        // vu and tube. Bloom the emitters alone, then composite over the panel.
        let mut lit = Canvas::new(w, h);

        for i in 0..g.tubes {
            let cx = g.cx(i);
            let resp = Self::response(Self::level_for(d, i, g.tubes), t.sensitivity);
            // Position on the stack. `resp` is already finite and clamped, so this is too.
            let pos = resp * (g.digits - 1) as f32;
            // The NEAREST digit is the one struck, and `lean` (0..1) is how far the level sits
            // toward its neighbour - 1.0 means exactly halfway, where the two are as close to
            // equal as this family ever allows them to get. See CROSSFADE.
            let near = pos.round().clamp(0.0, (g.digits - 1) as f32) as i32;
            let lean = ((pos - near as f32).abs() * 2.0).clamp(0.0, 1.0);
            let toward = if pos >= near as f32 { near + 1 } else { near - 1 };

            self.marker[i] = (self.marker[i] - fall).max(pos);
            if !self.marker[i].is_finite() {
                // Belt and braces: persistent state is exactly where a NaN does lasting damage,
                // since it survives into every later frame.
                self.marker[i] = pos;
            }

            // Afterglow of the peak, at a fraction of a struck cathode and with no cloud of its
            // own, so it can never be mistaken for the live digit. Only drawn above the pair
            // currently lit - below them it would just double-expose the same cells.
            let pk = self.marker[i].round() as i32;
            if pk > near + 1 && pk < g.digits {
                let y = g.cell_y(pk);
                lit.text_3x5(
                    cx - 1,
                    y,
                    DIGIT_STR[pk.clamp(0, 9) as usize],
                    Rgba::from_hex(&t.lit, 0.34),
                );
            }

            Self::strike(&mut lit, &g, cx, near, t, 1.0 - PRIMARY_DIP * lean);
            if toward >= 0 && toward < g.digits {
                Self::strike(&mut lit, &g, cx, toward, t, CROSSFADE * lean);
            }
        }

        // Confine the light to the envelopes, in ONE pass over all of them.
        //
        // Per-tube clipping cannot work here for the reason `tube` records: each tube's glow
        // reaches wider than the gap to its neighbour, so a per-tube punch erases the tube before
        // it and the row renders dark except for stray pixels. Clipping against the UNION of the
        // envelopes is the only formulation that does not depend on draw order.
        for y in 2..(h - 2) {
            let half = g.half_at(y);
            if half <= 0 {
                lit.punch_rect(0, y, w, 1);
                continue;
            }
            let mut prev_end = 0;
            for i in 0..g.tubes {
                let lo = g.cx(i) - half;
                if lo > prev_end {
                    lit.punch_rect(prev_end, y, lo - prev_end, 1);
                }
                prev_end = g.cx(i) + half + 1;
            }
            if prev_end < w {
                lit.punch_rect(prev_end, y, w - prev_end, 1);
            }
        }

        if t.bloom > 0.0 {
            let mut glow = lit.clone();
            glow.bloom(t.bloom as i32, t.glow_strength.clamp(0.0, 1.0));
            c.draw_over(&glow);
        }
        c.draw_over(&lit);

        // Glass and hardware, over the light, so a tube reads as something burning BEHIND glass.
        for i in 0..g.tubes {
            let cx = g.cx(i);
            let gx = cx - g.glass_w / 2;

            // Base: bakelite with a collar and pins, the same construction the valve row uses.
            c.fill_rect(gx - 1, g.base_y, g.glass_w + 2, g.base_h, Rgba::from_hex(&t.tube.socket, 1.0));
            c.fill_rect(gx - 1, g.base_y, g.glass_w + 2, 1, Rgba::from_hex(&t.tube.collar, 0.85));
            for pn in 0..3 {
                let px = gx + 2 + pn * ((g.glass_w - 3).max(3) / 3);
                c.fill_rect(px, g.base_y + 2, 1, (g.base_h - 3).max(1), Rgba::from_hex(&t.tube.collar, 0.5));
            }

            // Rim: a specular edge down one side and a dimmer catch-light down the other, both
            // following `half_at` so they curve with the envelope instead of squaring it off.
            for y in g.top..g.base_y {
                let half = g.half_at(y);
                if half <= 1 {
                    continue;
                }
                c.fill_rect(cx - half, y, 1, 1, Rgba::from_hex(&t.tube.glass, 0.24));
                c.fill_rect(cx + half, y, 1, 1, Rgba::from_hex(&t.tube.glass, 0.12));
            }
            // Close the top, or the envelope reads as an open channel. Only the rounded span is
            // capped, which is what makes the silhouette a bottle rather than a box.
            //
            // Drawn either side of the 3px digit column, not straight across it. At the reference
            // size the top cell IS the cap row - ten 5px cells consume the whole envelope - and the
            // ASCII dump showed the full-width cap lighting the top row of the '9', which reads as
            // an '8'. Losing 3px from the middle of a 9px cap is invisible by comparison.
            let cap = g.half_at(g.top);
            let cap_col = Rgba::from_hex(&t.tube.glass, 0.18);
            if cap > 1 {
                c.fill_rect(cx - cap, g.top, cap - 1, 1, cap_col);
                c.fill_rect(cx + 2, g.top, cap - 1, 1, cap_col);
            }
        }

        // Clip back to the panel with the SAME rect it was drawn with, or the bloom halo escapes
        // onto the bare taskbar and reads as a bright box around the display.
        c.clip_to_rounded_rect(1, 2, (w - 2).max(1), (h - 4).max(1), panel_r);
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
        for v in d.levels.iter_mut() {
            *v = level;
        }
        d.peaks = d.levels;
        d
    }

    fn lum(p: Rgba) -> f64 {
        0.2126 * p.r as f64 + 0.7152 * p.g as f64 + 0.0722 * p.b as f64
    }

    fn render(t: &Theme, d: &FrameData, w: i32, h: i32) -> Canvas {
        let mut n = Nixie::default();
        let mut c = Canvas::new(w, h);
        n.draw(&mut c, t, d);
        c
    }

    /// Total light in one digit cell of one tube, over the 3px glyph column.
    fn cell_light(c: &Canvas, g: &Geom, tube: usize, digit: i32) -> f64 {
        let cx = g.cx(tube);
        let y0 = g.cell_y(digit);
        let mut s = 0.0;
        for y in y0..(y0 + CELL_H) {
            for x in (cx - 1)..=(cx + 1) {
                s += lum(c.get(x, y));
            }
        }
        s
    }

    /// Which digit is actually lit, measured off the pixels rather than recomputed from the
    /// level. This is the assertion primitive for the whole family: if the brightest cell is not
    /// the digit the level asks for, the readout is wrong however plausible the arithmetic looks.
    fn lit_digit(c: &Canvas, g: &Geom, tube: usize) -> i32 {
        let mut best = (f64::MIN, 0);
        for dd in 0..g.digits {
            let s = cell_light(c, g, tube, dd);
            if s > best.0 {
                best = (s, dd);
            }
        }
        best.1
    }

    #[test]
    fn the_lit_digit_climbs_the_stack_as_the_band_drives_it() {
        // The family's entire reason to exist (rule 6: position over intensity). Measured as the
        // brightest CELL, not as a topmost-lit-row scan - the ghost cathodes and the chassis put a
        // pedestal on every row, and `tube`'s notes record three different threshold metrics that
        // each reported a fixed row at every level because of exactly that.
        let t = builtin::nixie_orange();
        let g = Geom::new(190, 60).unwrap();
        let at = |lvl: f32| lit_digit(&render(&t, &flat(lvl), 190, 60), &g, 3);
        let (quiet, mid, loud) = (at(0.11), at(0.35), at(0.64));
        assert!(quiet < mid, "the digit must climb: {quiet} at 0.11 vs {mid} at 0.35");
        assert!(mid < loud, "the digit must keep climbing: {mid} at 0.35 vs {loud} at 0.64");
        assert_eq!(quiet, 0, "a near-silent band must read 0, not a pedestal digit");
        assert_eq!(loud, g.digits - 1, "the top of the music window must reach the top digit");
    }

    #[test]
    fn an_uneven_spectrum_lights_a_different_digit_on_each_tube() {
        // A test that drives every band to the SAME level cannot see a per-band bug at all - it is
        // how this project shipped a valve row that looked static. Here the two ends of the
        // spectrum are driven a full stack apart, so a reducer that ignored its band range, or
        // geometry that reused one tube's digit for all of them, fails.
        let t = builtin::nixie_orange();
        let g = Geom::new(190, 60).unwrap();
        let mut d = FrameData::default();
        let n = d.levels.len();
        for (i, v) in d.levels.iter_mut().enumerate() {
            // A descending ramp across the spectrum: 0.62 at the bottom band, 0.12 at the top.
            *v = 0.62 - 0.50 * (i as f32 / (n - 1) as f32);
        }
        d.peaks = d.levels;
        let c = render(&t, &d, 190, 60);
        let digits: Vec<i32> = (0..g.tubes).map(|i| lit_digit(&c, &g, i)).collect();
        assert!(
            digits[0] > digits[g.tubes - 1] + 4,
            "a descending spectrum must descend across the row, got {digits:?}"
        );
        // And it must be monotonic, i.e. tube i really does read band group i.
        for pair in digits.windows(2) {
            assert!(
                pair[1] <= pair[0],
                "the row must follow the spectrum's shape, got {digits:?}"
            );
        }
        let mut uniq = digits.clone();
        uniq.dedup();
        assert!(uniq.len() >= 5, "the tubes must not all read the same digit: {digits:?}");
    }

    #[test]
    fn a_single_peaking_band_still_moves_its_own_tube() {
        // The case GROUP_MAX_BIAS exists for: one band peaking inside an otherwise quiet group.
        // A plain mean over ~9 bands puts this at 0.19, which quantises to digit 1 - i.e. the tube
        // would not visibly move at all while a band under it was at 0.65.
        let t = builtin::nixie_orange();
        let g = Geom::new(190, 60).unwrap();
        let mut d = FrameData::default();
        let n = d.levels.len();
        for v in d.levels.iter_mut() {
            *v = 0.14;
        }
        let lo = 2 * n / g.tubes;
        let hi = 3 * n / g.tubes;
        d.levels[(lo + hi) / 2] = 0.66;
        d.peaks = d.levels;
        let c = render(&t, &d, 190, 60);
        let driven = lit_digit(&c, &g, 2);
        let neighbour = lit_digit(&c, &g, 3);
        assert!(
            driven >= neighbour + 4,
            "a single peaking band must lift its own tube well clear of its neighbour: \
             {driven} vs {neighbour}"
        );
    }

    #[test]
    fn the_group_reducer_sits_near_the_peak_not_the_mean() {
        let mut d = FrameData::default();
        let n = d.levels.len();
        for v in d.levels.iter_mut() {
            *v = 0.1;
        }
        let hi = n / 7;
        d.levels[hi / 2] = 0.9;
        let mean = d.levels[..hi].iter().sum::<f32>() / hi as f32;
        let peak = d.levels[..hi].iter().copied().fold(0.0f32, f32::max);
        let got = Nixie::level_for(&d, 0, 7);
        assert!(got > mean + (peak - mean) * 0.5, "must clear the midpoint: {got} in [{mean}, {peak}]");
        assert!(got <= peak + 1e-6, "but never exceed the peak: {got} vs {peak}");
    }

    #[test]
    fn the_response_window_spends_its_range_on_levels_the_dsp_actually_produces() {
        // Rule 5: the input is NOT 0..1. Over 0..1 a band at the top of real music's range (0.65)
        // would light digit 5 and the top four digits would never be reached by anything.
        let lo = Nixie::response(0.15, 1.0);
        let hi = Nixie::response(0.65, 1.0);
        assert!(hi - lo > 0.75, "the music window must cover most of the stack: {lo} -> {hi}");
        assert_eq!(Nixie::response(0.0, 1.0), 0.0, "silence must read the bottom digit");
        assert_eq!(Nixie::response(1.0, 1.0), 1.0, "full scale must reach the top");
        assert!(
            Nixie::response(0.25, 2.0) > Nixie::response(0.25, 1.0),
            "sensitivity is the TOML knob and must actually scale it"
        );
        assert_eq!(Nixie::response(f32::NAN, 1.0), 0.0, "NaN must not survive as a digit index");
        assert_eq!(Nixie::response(0.4, f32::NAN), Nixie::response(0.4, 1.0), "NaN gain falls back");
    }

    #[test]
    fn the_unlit_cathodes_are_visible_but_never_compete_with_the_struck_one() {
        // Both halves matter. Ghosts too dim and the tube is a floating digit with no scale to
        // read it against; ghosts too bright and the brightest-cell cue - the only cue this family
        // has - stops being unambiguous.
        let t = builtin::nixie_orange();
        let g = Geom::new(190, 60).unwrap();
        let c = render(&t, &flat(0.11), 190, 60); // reads digit 0, so 9 ghosts are visible
        let struck = cell_light(&c, &g, 3, 0);
        let ghost = cell_light(&c, &g, 3, 6);
        // Against the same cell with the ghost stack switched off, so this cannot pass on the
        // chassis and the envelope interior alone.
        let mut dark = t.clone();
        dark.ghost = 0.0;
        let bare = cell_light(&render(&dark, &flat(0.11), 190, 60), &g, 3, 6);
        assert!(ghost > bare * 1.25, "unlit cathodes must be visible: {ghost:.0} vs bare {bare:.0}");
        assert!(struck > ghost * 3.0, "the struck digit must dominate: {struck:.0} vs {ghost:.0}");
    }

    #[test]
    fn light_stays_inside_its_own_envelope() {
        // The clip is this family's most likely failure: without it the discharge clouds bleed onto
        // the chassis and into the next tube, and the row reads as one lit bar.
        //
        // Measured over the TOP of each tube, not the whole column, and that distinction is the
        // whole reason this test was rewritten. The valve row's equivalent measures total light in
        // the tube because a valve gets brighter when driven - a nixie does not. Exactly one digit
        // is struck at every level, so a driven tube emits about as much light as an idle one; the
        // first version of this test measured -360, i.e. the driven tube read DARKER, purely
        // because '9' lights fewer pixels than '0'. Only the light's POSITION carries information
        // here, so that is what has to be measured.
        let t = builtin::nixie_orange();
        let g = Geom::new(190, 60).unwrap();
        let top_cells = |c: &Canvas, i: usize| -> f64 {
            let cx = g.cx(i);
            let y0 = g.cell_y(g.digits - 1);
            let mut s = 0.0;
            for y in y0..(y0 + CELL_H * 3) {
                for x in (cx - g.glass_w / 2)..=(cx + g.glass_w / 2) {
                    s += lum(c.get(x, y));
                }
            }
            s
        };
        let silent = render(&t, &flat(0.0), 190, 60);
        let mut low = FrameData::default();
        for (i, v) in low.levels.iter_mut().enumerate() {
            *v = if i < 9 { 0.9 } else { 0.0 };
        }
        low.peaks = low.levels;
        let driven = render(&t, &low, 190, 60);
        // Tube 0 reads 9 (top of its stack); every other tube reads 0 (bottom of its stack), so
        // any light appearing at the TOP of another tube got there by bleeding.
        let first = top_cells(&driven, 0) - top_cells(&silent, 0);
        let neighbour = top_cells(&driven, 1) - top_cells(&silent, 1);
        let far = top_cells(&driven, g.tubes - 1) - top_cells(&silent, g.tubes - 1);
        assert!(first > 500.0, "the driven tube must actually light its top digit: rise {first:.0}");
        assert!(
            first > neighbour.abs() * 8.0,
            "light must stay in its own envelope: driven {first:.0}, ADJACENT tube {neighbour:.0}"
        );
        assert!(
            first > far.abs() * 8.0,
            "light must stay in its own envelope: driven {first:.0}, far tube {far:.0}"
        );
    }

    #[test]
    fn nothing_is_drawn_outside_the_panel_rect() {
        // The bloom spreads several pixels past the 1-2px panel margin, so without the final clip
        // the halo lands on the bare taskbar as a bright box.
        let t = builtin::nixie_orange();
        let c = render(&t, &flat(0.7), 190, 60);
        for x in 0..190 {
            assert_eq!(c.get(x, 0), Rgba::TRANSPARENT, "row 0 is above the panel, x={x}");
            assert_eq!(c.get(x, 59), Rgba::TRANSPARENT, "row 59 is below the panel, x={x}");
        }
        for y in 0..60 {
            assert_eq!(c.get(0, y), Rgba::TRANSPARENT, "column 0 is left of the panel, y={y}");
            assert_eq!(c.get(189, y), Rgba::TRANSPARENT, "column 189 is right of the panel, y={y}");
        }
    }

    #[test]
    fn the_audio_actually_changes_the_pixels() {
        // Guards the whole no-op class: three tests in this project have passed against a stub.
        let t = builtin::nixie_orange();
        let quiet = render(&t, &flat(0.05), 190, 60).bits().to_vec();
        let loud = render(&t, &flat(0.9), 190, 60).bits().to_vec();
        assert_ne!(quiet, loud, "the readout must respond to the audio");
        let changed = quiet.iter().zip(&loud).filter(|(a, b)| a != b).count();
        assert!(changed > 200, "only {changed} pixels moved between silence and full drive");
    }

    #[test]
    fn the_crossfade_moves_the_readout_within_one_digits_span() {
        // Ten steps over the response window is a 0.058-level quantum, so without the cross-fade a
        // sustained band sitting inside one digit's span freezes the tube. Two levels that both
        // round to digit 4 must still differ on the pixels, with the neighbour the level is leaning
        // toward carrying the difference.
        let t = builtin::nixie_orange();
        let g = Geom::new(190, 60).unwrap();
        let step = RESP_SPAN / (g.digits - 1) as f32;
        let a = render(&t, &flat(RESP_FLOOR + step * 4.02), 190, 60);
        let b = render(&t, &flat(RESP_FLOOR + step * 4.45), 190, 60);
        assert_eq!(lit_digit(&a, &g, 2), 4, "both levels round to digit 4");
        assert_eq!(lit_digit(&b, &g, 2), 4, "both levels round to digit 4");
        let above = |c: &Canvas| cell_light(c, &g, 2, 5);
        assert!(
            above(&b) > above(&a) * 1.5,
            "the digit leaned toward must fade in across the span: {:.0} -> {:.0}",
            above(&a),
            above(&b)
        );
        // And it must lean the OTHER way below the digit, or the cross-fade is just a floor+1 that
        // happens to be centred - which is what it was, and which read one digit low all the way
        // up the top half of every span.
        let c = render(&t, &flat(RESP_FLOOR + step * 3.55), 190, 60);
        assert_eq!(lit_digit(&c, &g, 2), 4, "3.55 still rounds to 4");
        assert!(
            cell_light(&c, &g, 2, 3) > cell_light(&a, &g, 2, 3) * 1.5,
            "leaning down must light the digit BELOW, not the one above"
        );
    }

    #[test]
    fn the_brightest_digit_is_always_the_one_the_level_indicates() {
        // The glyph-area trap, swept across every digit in the stack rather than spot-checked.
        // '8' lights 13 of its 15 pixels and '7' only 7, so the neighbour at a LOWER alpha can
        // still put more light in its cell than the struck digit - and when it does, the readout
        // silently points at the wrong number. This is what set CROSSFADE and PRIMARY_DIP: the
        // 7-to-8 boundary failed at 0.55/0.35 AND at 0.38/0.15. Consult this before touching either.
        //
        // Swept to +-0.4 of a digit, not +-0.5: at exactly halfway the level is equidistant and
        // whichever cell wins is honest, so demanding a winner there would be asserting a
        // tie-break nobody can see.
        let t = builtin::nixie_orange();
        let g = Geom::new(190, 60).unwrap();
        let step = RESP_SPAN / (g.digits - 1) as f32;
        for d in 0..g.digits {
            for off in [-0.4f32, -0.2, 0.0, 0.2, 0.4] {
                let pos = (d as f32 + off).clamp(0.0, (g.digits - 1) as f32);
                let level = RESP_FLOOR + step * pos;
                let got = lit_digit(&render(&t, &flat(level), 190, 60), &g, 3);
                assert_eq!(
                    got,
                    pos.round() as i32,
                    "stack position {pos:.2} (level {level:.4}) read digit {got}"
                );
            }
        }
    }

    #[test]
    fn the_geometry_keeps_the_tubes_wide_and_scales_the_count_with_width() {
        assert_eq!(tube_count(190), 7, "the reference panel gets 7 wide tubes, not the valve row's 10");
        assert_eq!(tube_count(380), 14, "double the width doubles the tubes rather than the glass");
        assert!(tube_count(4000) <= 20, "capped");
        assert!(tube_count(20) >= 2, "never fewer than a pair");
        // Pitch is what must stay put across widths - a stretched nixie is a speck in a jar.
        let reference = Geom::new(190, 60).unwrap().pitch;
        for w in [190, 240, 380, 456, 600] {
            let p = Geom::new(w, 60).unwrap().pitch;
            assert!((p - reference).abs() < 4.0, "at width {w} the pitch drifted to {p:.1}");
        }
        // And a full stack of ten really does fit at the reference size, glyph height included.
        let g = Geom::new(190, 60).unwrap();
        assert_eq!(g.digits, MAX_DIGITS, "the reference panel must show all ten digits");
        assert!(g.stack_top >= g.top, "the stack must start inside the envelope");
        assert!(
            g.cell_y(0) + CELL_H <= g.base_y,
            "the bottom digit must clear the base: cell ends {} vs base {}",
            g.cell_y(0) + CELL_H,
            g.base_y
        );
    }

    #[test]
    fn a_shorter_panel_drops_digits_rather_than_squeezing_the_glyphs() {
        // A cell under 5 rows cannot render the 3x5 font at all, so it would draw a blank tube.
        for h in [24, 32, 40, 48, 60, 72] {
            let g = Geom::new(190, h).unwrap_or_else(|| panic!("h={h} should still lay out"));
            assert!(g.digits >= MIN_DIGITS && g.digits <= MAX_DIGITS, "h={h}: {} digits", g.digits);
            assert!(g.cell_y(0) + CELL_H <= g.base_y, "h={h}: the stack must fit the envelope");
            assert!(g.cell_y(g.digits - 1) >= g.top, "h={h}: the top cell must be inside the glass");
        }
    }

    #[test]
    fn every_digit_the_stack_needs_is_in_the_font() {
        // Silent failure if not: an unsupported character advances the cursor and draws NOTHING, so
        // a missing 7, 8, 9 or 0 would leave those cells blank and the tube would appear to skip
        // levels. 0, 7, 8 and 9 were added to canvas.rs for this family.
        for (i, s) in DIGIT_STR.iter().enumerate() {
            let mut c = Canvas::new(12, 12);
            c.text_3x5(1, 1, s, Rgba::new(255, 255, 255, 255));
            let n = c.bits().iter().filter(|p| **p != 0).count();
            assert!(n > 0, "digit {i} ({s}) drew nothing - it is missing from the 3x5 font");
        }
    }

    #[test]
    fn renders_at_every_plausible_size_without_panicking() {
        let t = builtin::nixie_orange();
        for (w, h) in [
            (190, 60),
            (380, 60),
            (456, 60),
            (240, 72),
            (150, 48),
            (96, 40),
            (40, 24),
            (28, 24),
            (12, 12),
            (2, 2),
            (1, 1),
            (0, 0),
        ] {
            let mut n = Nixie::default();
            let mut c = Canvas::new(w, h);
            // Several frames, because the peak marker is persistent state sized on the first one.
            for _ in 0..3 {
                n.draw(&mut c, &t, &flat(0.5));
            }
            assert_eq!(c.bits().len(), (w.max(0) * h.max(0)) as usize, "{w}x{h} changed size");
        }
    }

    #[test]
    fn survives_nan_and_infinity_without_poisoning_later_frames() {
        // f32::clamp does NOT sanitise NaN, and the peak marker is persistent - a NaN reaching it
        // once would corrupt every subsequent frame, which is how the vaporwave scroll phase was
        // permanently broken.
        let t = builtin::nixie_orange();
        let mut n = Nixie::default();
        let mut c = Canvas::new(190, 60);
        let mut d = flat(0.5);
        d.levels[0] = f32::NAN;
        d.levels[20] = f32::INFINITY;
        d.levels[40] = f32::NEG_INFINITY;
        d.peaks[3] = f32::NAN;
        d.dt_ms = f32::NAN;
        for _ in 0..4 {
            n.draw(&mut c, &t, &d);
        }
        assert!(n.marker.iter().all(|v| v.is_finite()), "marker state was poisoned: {:?}", n.marker);
        // And a clean frame afterwards must render normally again.
        let g = Geom::new(190, 60).unwrap();
        n.draw(&mut c, &t, &flat(0.64));
        assert_eq!(lit_digit(&c, &g, 3), g.digits - 1, "a clean frame after NaN must read correctly");
        // Infinity in dt (and zero) must not move the marker anywhere silly either.
        let mut d2 = flat(0.2);
        d2.dt_ms = f32::INFINITY;
        n.draw(&mut c, &t, &d2);
        d2.dt_ms = 0.0;
        n.draw(&mut c, &t, &d2);
        assert!(n.marker.iter().all(|v| v.is_finite()), "dt poisoned the marker: {:?}", n.marker);
    }

    #[test]
    fn every_nixie_colourway_renders_and_differs() {
        let mut seen: Vec<Vec<u32>> = Vec::new();
        for t in builtin::all().into_iter().filter(|t| t.family == "nixie") {
            let c = render(&t, &flat(0.45), 190, 60);
            assert!(c.bits().iter().any(|p| *p != 0), "{} rendered nothing", t.id);
            for prior in &seen {
                assert_ne!(prior, c.bits(), "{} renders identically to another colourway", t.id);
            }
            seen.push(c.bits().to_vec());
        }
        assert_eq!(seen.len(), 5, "expected the five shipped nixie colourways, got {}", seen.len());
    }

    #[test]
    fn the_aged_colourway_really_is_dimmer_than_the_classic_one() {
        // The brief asked for one aged look with a dimmer envelope, and "aged" that measures
        // identical to the reference is just another orange.
        let total = |t: &Theme| -> f64 {
            let c = render(t, &flat(0.45), 190, 60);
            let mut s = 0.0;
            for y in 0..60 {
                for x in 0..190 {
                    s += lum(c.get(x, y));
                }
            }
            s
        };
        let classic = total(&builtin::nixie_orange());
        let aged = total(&builtin::nixie_aged());
        assert!(aged < classic * 0.92, "aged {aged:.0} must read dimmer than classic {classic:.0}");
    }

    /// Prints the ladder as an ASCII luminance map. Not an assertion - it is how the digits'
    /// legibility can be judged without a raw-RGBA viewer, and it is what confirmed the glyphs
    /// survive touching their neighbours vertically at a 5px cell.
    /// Run: cargo test --release print_nixie_ladder -- --ignored --nocapture
    #[test]
    #[ignore]
    fn print_nixie_ladder() {
        let t = builtin::nixie_orange();
        let g = Geom::new(190, 60).unwrap();
        let mut d = FrameData::default();
        let nb = d.levels.len();
        for (i, v) in d.levels.iter_mut().enumerate() {
            let tube = i * g.tubes / nb;
            *v = RESP_FLOOR + RESP_SPAN * (tube as f32 / (g.tubes - 1) as f32);
        }
        d.peaks = d.levels;
        let c = render(&t, &d, 190, 60);
        println!("{}", crate::render::golden::canvas_to_ascii(&c));
        println!(
            "tubes {} pitch {:.1} glass {} digits {} stack_top {} base_y {}",
            g.tubes, g.pitch, g.glass_w, g.digits, g.stack_top, g.base_y
        );
    }

    /// Run: cargo test --release dump_nixie_frames -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_nixie_frames() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();

        // An uneven spectrum, so the tubes sit at visibly different digits - a flat one would look
        // like a working row even if the band mapping were broken.
        let mut d = FrameData::default();
        for (i, v) in d.levels.iter_mut().enumerate() {
            let x = i as f32 / 63.0;
            *v = (0.14 + 0.52 * (x * 7.0).sin().abs()) * (1.0 - x * 0.35) + 0.10;
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
            std::fs::write(dir.join(format!("{name}.rgba")), &out).unwrap();
        };

        let mut n = 0usize;
        for t in builtin::all().into_iter().filter(|t| t.family == "nixie") {
            let mut fam = Nixie::default();
            let mut c = Canvas::new(190, 60);
            fam.draw(&mut c, &t, &d);
            dump(&c, &format!("nixie-{}", t.id));
            n += 1;
        }

        // The wide mode, and a staircase so every digit in the stack is exercised somewhere in
        // the row - a human can then check the glyphs are legible at every height.
        let t = builtin::nixie_orange();
        let mut wide = Canvas::new(380, 60);
        Nixie::default().draw(&mut wide, &t, &d);
        dump(&wide, "nixie-wide");

        let mut ladder = FrameData::default();
        let nb = ladder.levels.len();
        let tubes = tube_count(190);
        for (i, v) in ladder.levels.iter_mut().enumerate() {
            let tube = i * tubes / nb;
            *v = RESP_FLOOR + RESP_SPAN * (tube as f32 / (tubes - 1) as f32);
        }
        ladder.peaks = ladder.levels;
        let mut lc = Canvas::new(190, 60);
        Nixie::default().draw(&mut lc, &t, &ladder);
        dump(&lc, "nixie-ladder");

        println!(
            "wrote {} colourway dumps plus nixie-wide (380x60) and nixie-ladder to {}",
            n,
            dir.display()
        );
    }
}
