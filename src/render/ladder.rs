//! The LED bar ladder family: a hi-fi amplifier front panel, lit from the bottom up.
//!
//! Discrete rectangular LEDs in columns, sitting in a dark slot with the UNLIT lenses still
//! visible, green while there is headroom, amber when loud, red at the top. `Theme::zones`
//! already expresses exactly that three-colour scale (it is what `classic-three-colour` was
//! authored for), so this family reads its colour entirely through `lit_at`/`hot_at` and never
//! interpolates its own ramp.
//!
//! It shares a silhouette with `segmented`, so the differences are the whole point of it
//! existing and each is deliberate:
//!
//! - **Chunky LEDs with visible unlit housings.** `segmented`'s marks are 5x3 with a 1px gap
//!   and its dormant grid is a `ghost` wash *only where a mark is not lit*, which reads as a
//!   faint continuous column. A real ladder shows you the whole scale even in silence: a black
//!   recessed slot per column with every lens tinted in its own zone colour, so you can see
//!   the four green / two amber / two red positions before a note is played. Here that is 9x4
//!   lenses at a 3px column gap and a 2px rung gap - both gaps doubled from `segmented`,
//!   because the unlit slot between LEDs has to survive the halo (see BLEED) or the column
//!   collapses into one bar and the count stops being readable.
//! - **A peak-hold dot that falls slowly.** `segmented` draws a 1px cap the full width of the
//!   bar straight from `FrameData.peaks`. That cap is the same width as the mark below it, so
//!   it reads as one more lit segment rather than as a marker. Here it is a short 5x2 dot in
//!   the middle of its lens - narrower AND shorter than the LED, so it is legible sitting on
//!   top of a lit column - and it is held in this family's own state so it keeps stepping down
//!   after the music stops, which is the behaviour that makes it read as a memory of the peak.
//! - **No glass panel.** `t.texture` is deliberately ignored: every `Texture` variant paints a
//!   full-width sheen, haze or grille across the panel, and any of them lifts the dark slot
//!   the unlit lenses are read against. The look this family is after is a matte extrusion
//!   with LEDs punched through it, and the readability of an unlit lens depends entirely on
//!   the slot staying near-black.
//!
//! Crisp rather than glowy, which is the reason it was picked as the most readable of the set:
//! the bloom radius is capped (MAX_BLOOM) no matter what a theme file asks for, and the halo
//! is only ever a fraction of `glow_strength` (BLEED).

use super::canvas::{Canvas, Rgba};
use super::{Family, FrameData};
use crate::themes::Theme;

/// One LED lens, and the dark gaps around it.
///
/// Chunkier than `segmented`'s 5x3/1px on purpose - see the module docs. The gaps are the part
/// that matters: at a 1px rung gap the halo of a lit LED reaches its neighbour and eight
/// discrete rungs render as one continuous bar, which throws away the position cue this family
/// is built on.
const LED_W: i32 = 9;
const LED_H: i32 = 4;
const COL_GAP: i32 = 3;
const RUNG_GAP: i32 = 2;

/// Band level at which a column starts to climb, and the span it climbs over.
///
/// THE INPUT IS NOT 0..1. `FrameData.levels` sits at roughly 0.15-0.65 for active bands, so a
/// straight `level * rungs` mapping spends about a third of its range on levels real music
/// never reaches: measured on an 8-rung ladder, a plain mapping moved a column from 1.2 to 5.2
/// rungs across that window - it never lit the amber pair, let alone the red one, so the whole
/// three-colour scale the family exists for was unreachable. Remapped, the same window covers
/// 0.1 to 8.0 rungs, i.e. the full scale including the red tip.
///
/// A FIXED window, not a peak follower, for the same reason `tube` uses one: this is a level
/// meter, and a follower would show the same band at a different height depending on what came
/// before it - which would make a quiet passage look loud.
const RESP_FLOOR: f32 = 0.10;
const RESP_SPAN: f32 = 0.52;

/// Weight given to a group's LOUDEST band rather than its mean.
///
/// 15 columns over 64 bands is about 4.3 bands each, and a plain mean flattens exactly the
/// single-band peaks that make one column taller than the next. Measured with one band at 0.65
/// inside a group otherwise at 0.20: the mean puts that column at 0.30 (2 rungs) against its
/// neighbour's 0.20 (1 rung), a one-rung difference that a viewer cannot reliably see as
/// "different" rather than "quantisation". Biased to the max it is 0.52 vs 0.20, five rungs
/// against one.
///
/// Invisible to any test that drives every band to the same level, since mean == max there.
const GROUP_MAX_BIAS: f32 = 0.70;

/// Per-frame fall of the peak-hold dot, in response units (1.0 = the top of the ladder).
///
/// The render loop runs at ~16ms, so 0.010 is one full traverse in ~1.6s and one rung of an
/// 8-rung ladder (0.125) in ~200ms - slow enough to sit visibly above a column that has
/// already dropped, fast enough that the dot is not still hanging around from the previous
/// track. Sourced from the DISPLAYED response and not from `FrameData.peaks`: the shared
/// peak-hold falls at 0.0055 per frame in INPUT units, which past this family's response
/// remap is roughly 0.011 per frame near the floor but only reaches the dot's own units after
/// the remap has already clipped, so under continuous music the dot would pin to the top of
/// every column and stop being a marker at all.
const HOLD_FALL: f32 = 0.010;

/// Alpha of the topmost (partial) LED when the level has only just reached it.
///
/// Eight rungs means one whole LED is 12.5% of the scale, and real music routinely moves less
/// than that between frames - quantised hard, a column visibly sticks and then jumps. The head
/// LED is therefore dimmed by the fractional part, which restores the in-between positions
/// WITHOUT making brightness the cue: the count of fully lit LEDs is still what you read, the
/// head is just the leading edge. Not 0.0, because a head LED that fades all the way out makes
/// the column appear a whole rung shorter than the level it is showing.
const HEAD_FLOOR: f32 = 0.45;

/// Hard cap on the bloom radius, whatever the theme asks for.
///
/// The column gap is 3px. A radius of 4 already carries a lit LED's halo across it into the
/// neighbouring slot, and the brief for this family is "crisp rather than glowy" - the
/// separation between columns is what makes fifteen of them readable at 190px wide. Capped
/// rather than trusted, because `bloom` is a shared theme field and the shipped values for
/// other families run up to 12.
const MAX_BLOOM: f32 = 3.0;

/// Fraction of `glow_strength` the halo actually gets.
///
/// An LED behind a diffuser does bleed a little into its housing, and with no bleed at all the
/// lenses read as painted rectangles rather than as light sources. But at full strength the
/// bleed fills the 2px rung gaps and the eight discrete LEDs merge into a bar.
const BLEED: f32 = 0.55;

/// Alpha of the black recess the LEDs sit in.
///
/// Painted over the panel rather than punched, because `punch_row` zeroes the FULL canvas
/// width (it would erase the panel and leave the taskbar showing through) and even
/// `punch_rect` would make the slot transparent rather than dark. Darkening the panel is also
/// what makes a `ghost`-tinted unlit lens legible: against the bare panel a 0.20 tint is a
/// smudge, against this it is a visible lens.
const SLOT_ALPHA: f32 = 0.55;

/// Where the ladder sits, derived ONCE and shared with the tests.
///
/// `segmented`'s tests re-derived its centring offset by hand and sampled a pixel in the
/// margin instead of in the first bar, which nearly passed on bloom spill. Handing the same
/// struct to `draw` and to the tests removes that whole class of mistake.
struct Geom {
    /// Left edge of column 0.
    ox: i32,
    /// Top edge of the TOP rung.
    oy: i32,
    cols: i32,
    rungs: i32,
}

impl Geom {
    /// Top-left of one lens. `rung` 0 is the BOTTOM of the ladder, so the caller counts
    /// upward the way the meter does.
    fn cell(&self, col: i32, rung: i32) -> (i32, i32) {
        (
            self.ox + col * (LED_W + COL_GAP),
            self.oy + (self.rungs - 1 - rung) * (LED_H + RUNG_GAP),
        )
    }

    /// Fraction up the ladder of a rung, which is what `Theme::lit_at` keys the zones on.
    fn frac(&self, rung: i32) -> f32 {
        (rung + 1) as f32 / self.rungs.max(1) as f32
    }
}

/// Columns and rungs for a live panel size, or `None` if it cannot hold even one LED.
///
/// Column and rung SIZE is fixed and the count scales, rather than the reverse: the whole
/// point of a ladder is discrete elements of a known size, and stretching nine of them across
/// 380px gives 21px-wide slabs that read as a bar graph. This is the same conclusion `tube`
/// and `vu` reached for their own elements.
fn geometry(w: i32, h: i32) -> Option<Geom> {
    // The panel interior, matching the rounded_rect `draw` paints below.
    let (iw, ih) = (w - 2, h - 4);
    if iw <= 0 || ih <= 0 {
        return None;
    }
    // Keep the light off the bezel. Collapsed to 1px on small canvases so a 40x24 overlay
    // still gets a recognisable ladder instead of nothing - the overlay is sized from the live
    // Widgets rect and genuinely turns up at odd sizes.
    let mx = if iw >= 40 { 4 } else { 1 };
    let my = if ih >= 30 { 4 } else { 1 };
    let (aw, ah) = (iw - mx * 2, ih - my * 2);
    if aw < LED_W || ah < LED_H {
        return None;
    }
    // A ladder of n elements is n*pitch - gap wide, so the +gap is what stops the last
    // column being dropped for a trailing gap that is never drawn.
    let cols = ((aw + COL_GAP) / (LED_W + COL_GAP)).max(1);
    let rungs = ((ah + RUNG_GAP) / (LED_H + RUNG_GAP)).max(1);
    let used_w = cols * (LED_W + COL_GAP) - COL_GAP;
    let used_h = rungs * (LED_H + RUNG_GAP) - RUNG_GAP;
    Some(Geom {
        ox: 1 + mx + (aw - used_w) / 2,
        oy: 2 + my + (ah - used_h) / 2,
        cols,
        rungs,
    })
}

/// Level driving one column: the group's mean blended toward its max. See GROUP_MAX_BIAS.
///
/// Every value is checked with `is_finite` before it is used. `f32::clamp` does NOT sanitise
/// NaN (every comparison against NaN is false, so clamp returns the NaN untouched), and this
/// feeds the persistent peak-hold - one poisoned frame would otherwise park a column's dot at
/// a NaN height for the rest of the session.
fn group_level(d: &FrameData, col: usize, cols: usize) -> f32 {
    let n = d.levels.len();
    let cols = cols.max(1);
    let lo = col * n / cols;
    let hi = (((col + 1) * n / cols).max(lo + 1)).min(n);
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

/// Maps a group level onto 0..1 of ladder travel. See RESP_FLOOR / RESP_SPAN.
fn response(level: f32, sensitivity: f32) -> f32 {
    if !level.is_finite() {
        return 0.0;
    }
    let s = if sensitivity.is_finite() { sensitivity.max(0.0) } else { 1.0 };
    (((level - RESP_FLOOR) / RESP_SPAN) * s).clamp(0.0, 1.0)
}

#[derive(Default)]
pub struct Ladder {
    /// Peak-hold per column, in response units. A `Vec` because the column count follows the
    /// panel width and the canvas is resized live.
    hold: Vec<f32>,
}

impl Family for Ladder {
    fn id(&self) -> &'static str {
        "ladder"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());
        c.clear();
        c.rounded_rect(1, 2, w - 2, h - 4, 4, Rgba::from_hex(&t.panel, t.panel_alpha));

        let g = match geometry(w, h) {
            Some(g) => g,
            // Too small for a single LED. The panel alone is the honest degradation - a
            // sub-LED ladder would be a couple of stray pixels that look like corruption.
            None => return,
        };

        // The unlit ladder: a black recess per column with every lens tinted in its own zone
        // colour, so the scale is readable in silence. This goes on the OPAQUE panel, not on
        // the light layer, because it must not be bloomed - a blurred housing is a smudge.
        let slot = Rgba::from_hex("#000000", SLOT_ALPHA);
        let lip = Rgba::from_hex(&t.edge, (t.edge_alpha * 0.6).clamp(0.0, 1.0));
        let slot_h = g.rungs * (LED_H + RUNG_GAP) - RUNG_GAP + 2;
        for b in 0..g.cols {
            let (x, y_top) = g.cell(b, g.rungs - 1);
            c.fill_rect(x - 1, y_top - 1, LED_W + 2, slot_h, slot);
            // 1px lip along the top of the slot - the extrusion's own edge catching the room
            // light. Cheap, but it is what stops the slot reading as a hole cut in the panel.
            c.fill_rect(x - 1, y_top - 1, LED_W + 2, 1, lip);
            if t.ghost > 0.0 {
                for k in 0..g.rungs {
                    let (lx, ly) = g.cell(b, k);
                    c.fill_rect(lx, ly, LED_W, LED_H, Rgba::from_hex(t.lit_at(g.frac(k)), t.ghost));
                }
            }
        }

        // Everything that emits light goes on its OWN transparent layer.
        //
        // `Canvas::bloom` composites its halo UNDERNEATH what is already on the canvas, so
        // blooming in place on the opaque panel above hides the halo completely - the trap
        // segmented/scope/vu/tube each document. Build here, bloom, then composite over.
        let mut lit = Canvas::new(w, h);

        if self.hold.len() != g.cols as usize {
            self.hold.resize(g.cols as usize, 0.0);
        }

        for b in 0..g.cols {
            let resp = response(group_level(d, b as usize, g.cols as usize), t.sensitivity);
            // `resp` is already finite (see `response`), but the accumulator is re-guarded
            // because it is the one value that persists across frames.
            let held = self.hold[b as usize];
            let held = if held.is_finite() { held } else { 0.0 };
            let held = (held - HOLD_FALL).max(resp).clamp(0.0, 1.0);
            self.hold[b as usize] = held;

            // POSITION is the cue: `full` whole LEDs lit, growing from the bottom. Brightness
            // only ever refines the leading edge (see HEAD_FLOOR).
            let units = resp * g.rungs as f32;
            let full = units.floor().max(0.0) as i32;
            let partial = (units - full as f32).clamp(0.0, 1.0);

            for k in 0..g.rungs {
                let a = if k < full {
                    1.0
                } else if k == full && partial > 0.02 {
                    HEAD_FLOOR + (1.0 - HEAD_FLOOR) * partial
                } else {
                    continue;
                };
                let (x, y) = g.cell(b, k);
                let frac = g.frac(k);
                lit.fill_rect(x, y, LED_W, LED_H, Rgba::from_hex(t.lit_at(frac), a));
                // A 1px hot row along the lens's top edge. A flat rectangle of one colour
                // reads as paint; one bright edge reads as a diffuser with a die behind it.
                // Scaled by `a` so the head LED does not out-highlight the LEDs below it.
                lit.fill_rect(x, y, LED_W, 1, Rgba::from_hex(t.hot_at(frac), 0.45 * a));
            }

            // Peak-hold dot. `ceil` so a peak that has climbed even slightly into a rung
            // shows that rung: when the peak is fresh the dot lands ON the head LED, and as
            // the column falls away it is left standing above it, which is what makes it read
            // as a held marker rather than as the top of the bar.
            if held > 0.02 {
                let rung = (((held * g.rungs as f32).ceil() as i32).clamp(1, g.rungs)) - 1;
                let (x, y) = g.cell(b, rung);
                // Narrower AND shorter than the lens, deliberately. A full-width cap - what
                // `segmented` draws - is indistinguishable from one more lit segment when it
                // is sitting directly on top of the column.
                let dw = (((LED_W / 2) | 1).max(1)).min(LED_W);
                let dh = (LED_H / 2).max(1);
                lit.fill_rect(
                    x + (LED_W - dw) / 2,
                    y + (LED_H - dh) / 2,
                    dw,
                    dh,
                    Rgba::from_hex(t.hot_at(g.frac(rung)), 1.0),
                );
            }
        }

        let radius = if t.bloom.is_finite() { t.bloom.round().clamp(0.0, MAX_BLOOM) as i32 } else { 0 };
        let strength = if t.glow_strength.is_finite() { t.glow_strength.clamp(0.0, 1.0) * BLEED } else { 0.0 };
        if radius > 0 && strength > 0.0 {
            let mut halo = lit.clone();
            halo.bloom(radius, strength);
            c.draw_over(&halo);
        }
        // The crisp LEDs go on last, over their own halo.
        c.draw_over(&lit);

        // Clip back to the panel, with the SAME rect the panel was drawn with. The panel is
        // inset only 1-2px but the bloom spreads up to `radius` in every direction, so without
        // this the halo escapes onto the bare taskbar and reads as a bright box around the
        // widget.
        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 4);

        let e = Rgba::from_hex(&t.edge, t.edge_alpha);
        c.fill_rect(1, 2, w - 2, 1, e);
        c.fill_rect(1, h - 3, w - 2, 1, e);
        c.fill_rect(1, 2, 1, h - 4, e);
        c.fill_rect(w - 2, 2, 1, h - 4, e);
        // Re-clipped, because this bezel is SQUARE and the panel is rounded, so its corners land
        // outside the panel's silhouette and paint onto the bare taskbar. Every family draws its
        // bezel after the clip and so leaks a few corner pixels this way; this one is the worst of
        // them because it is the only one that also draws the left and right verticals, which run
        // the full height straight through both rounded corners on each side.
        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 4);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::themes::builtin;

    fn lum(p: Rgba) -> f32 {
        0.2126 * p.r as f32 + 0.7152 * p.g as f32 + 0.0722 * p.b as f32
    }

    /// Every band at the same level. Enough for the "does it light at all" cases, and
    /// deliberately NOT enough for the per-column ones - a flat spectrum makes a group's mean
    /// equal its max, so the reducer is a no-op and a per-band bug is invisible. See
    /// `uneven`.
    fn frame(level: f32) -> FrameData {
        let mut d = FrameData::default();
        d.levels = [level; crate::dsp::bands::NUM_BANDS];
        d.peaks = d.levels;
        d
    }

    /// One band peaking inside an otherwise quiet group - the case a flat spectrum cannot see.
    fn uneven(cols: usize, loud_col: usize, loud: f32, quiet: f32) -> FrameData {
        let mut d = FrameData::default();
        let n = d.levels.len();
        d.levels = [quiet; crate::dsp::bands::NUM_BANDS];
        let lo = loud_col * n / cols;
        let hi = ((loud_col + 1) * n / cols).min(n);
        d.levels[(lo + hi) / 2] = loud;
        d.peaks = d.levels;
        d
    }

    fn render(t: &Theme, frames: &[FrameData]) -> Canvas {
        let mut l = Ladder::default();
        let mut c = Canvas::new(190, 60);
        for d in frames {
            l.draw(&mut c, t, d);
        }
        c
    }

    /// Sample point inside a lens but OUTSIDE the peak dot's 5px-wide core, so LED body and
    /// dot can be told apart. The dot spans x+2..x+7; x+1 is clear of it.
    fn body(c: &Canvas, g: &Geom, col: i32, rung: i32) -> f32 {
        let (x, y) = g.cell(col, rung);
        lum(c.get(x + 1, y + 2))
    }

    fn dot(c: &Canvas, g: &Geom, col: i32, rung: i32) -> f32 {
        let (x, y) = g.cell(col, rung);
        lum(c.get(x + LED_W / 2, y + 1))
    }

    /// How many LEDs of a column read as lit. The threshold sits well above an unlit lens
    /// (measured ~15-40 depending on colourway) and below the head LED at HEAD_FLOOR.
    fn lit_rungs(c: &Canvas, g: &Geom, col: i32) -> i32 {
        (0..g.rungs).filter(|k| body(c, g, col, *k) > 90.0).count() as i32
    }

    /// Topmost rung showing the peak dot, ignoring the LED bodies.
    fn dot_rung(c: &Canvas, g: &Geom, col: i32) -> Option<i32> {
        (0..g.rungs).rev().find(|k| dot(c, g, col, *k) > 90.0)
    }

    #[test]
    fn the_ladder_lights_from_the_bottom_up() {
        let t = builtin::ladder_classic();
        let g = geometry(190, 60).unwrap();
        let c = render(&t, &[frame(0.36)]);
        assert!(body(&c, &g, 3, 0) > 90.0, "the bottom LED must be lit at a mid level");
        assert!(
            body(&c, &g, 3, g.rungs - 1) < 60.0,
            "the top LED must stay dark at a mid level, got {}",
            body(&c, &g, 3, g.rungs - 1)
        );
        // Contiguous, not scattered: a ladder with a hole in it is a bug, not a look.
        let n = lit_rungs(&c, &g, 3);
        for k in 0..n {
            assert!(body(&c, &g, 3, k) > 90.0, "rung {k} below the head must be lit");
        }
    }

    #[test]
    fn an_uneven_spectrum_lights_columns_to_visibly_different_heights() {
        // The test a flat spectrum cannot do. A single peaking band must take its own column
        // clearly higher than the quiet ones either side - measured in RUNGS, i.e. in the
        // position cue, not in brightness.
        let t = builtin::ladder_classic();
        let g = geometry(190, 60).unwrap();
        let target = 5;
        let c = render(&t, &[uneven(g.cols as usize, target, 0.62, 0.20)]);
        let driven = lit_rungs(&c, &g, target as i32);
        let left = lit_rungs(&c, &g, target as i32 - 1);
        let right = lit_rungs(&c, &g, target as i32 + 1);
        assert!(
            driven >= left + 3 && driven >= right + 3,
            "the driven column must stand several rungs above its neighbours: \
             {left} | {driven} | {right}"
        );
    }

    #[test]
    fn the_group_reducer_is_biased_toward_the_peak_not_the_mean() {
        // Guards GROUP_MAX_BIAS directly: a mean over 4 bands with one loud one lands barely
        // above the quiet floor, which is what makes a ladder look static.
        let d = uneven(15, 0, 0.9, 0.1);
        let n = d.levels.len();
        let hi = (n / 15).max(1);
        let mean = d.levels[..hi].iter().sum::<f32>() / hi as f32;
        let peak = d.levels[..hi].iter().copied().fold(0.0f32, f32::max);
        let got = group_level(&d, 0, 15);
        assert!(got > mean + (peak - mean) * 0.5, "must sit above the midpoint: {got} in [{mean}, {peak}]");
        assert!(got <= peak + 1e-6, "but never above the peak: {got} vs {peak}");
    }

    #[test]
    fn the_response_window_spends_its_range_on_levels_the_dsp_actually_produces() {
        let lo = response(0.15, 1.0);
        let hi = response(0.65, 1.0);
        assert!(hi - lo > 0.75, "the music window must cover most of the travel: {lo} -> {hi}");
        assert_eq!(response(0.0, 1.0), 0.0, "silence must not sit on a pedestal");
        assert_eq!(response(1.0, 1.0), 1.0, "full scale must reach the top rung");
        assert!(response(0.3, 2.0) > response(0.3, 1.0), "sensitivity must be a live knob");
    }

    #[test]
    fn the_zones_run_green_then_amber_then_red_up_the_column() {
        // The reason this family reads `Theme::lit_at` instead of its own ramp. Sampled at
        // full drive, where every rung is lit, so this is about colour and not about height.
        let t = builtin::ladder_classic();
        let g = geometry(190, 60).unwrap();
        let c = render(&t, &[frame(1.0)]);
        let (x0, y0) = g.cell(4, 0);
        let bottom = c.get(x0 + 1, y0 + 2);
        let (xt, yt) = g.cell(4, g.rungs - 1);
        let top = c.get(xt + 1, yt + 2);
        assert!(
            bottom.g as f32 > bottom.r as f32 * 1.4,
            "the bottom of the ladder must read green: {bottom:?}"
        );
        assert!(
            top.r as f32 > top.g as f32 * 1.4,
            "the top of the ladder must read red: {top:?}"
        );
    }

    #[test]
    fn the_unlit_housings_are_visible_in_silence_but_far_darker_than_a_lit_led() {
        // The headline difference from `segmented`: a real ladder shows you where the unlit
        // LEDs are. Both halves matter - invisible housings lose the scale, bright ones make
        // the meter unreadable because a lit LED no longer stands out.
        let t = builtin::ladder_classic();
        let g = geometry(190, 60).unwrap();
        let silent = render(&t, &[frame(0.0)]);
        let housing = body(&silent, &g, 4, 3);
        // The bare panel, sampled between two columns where no slot is drawn.
        let (x, y) = g.cell(4, 3);
        let bare = lum(silent.get(x + LED_W + 1, y + 2));
        assert!(
            housing > bare + 4.0,
            "an unlit lens must be visible against the bare panel: lens {housing} vs panel {bare}"
        );
        let driven = render(&t, &[frame(1.0)]);
        let on = body(&driven, &g, 4, 3);
        assert!(
            on > housing * 3.0,
            "a lit LED must dominate its own unlit housing: {on} vs {housing}"
        );
    }

    #[test]
    fn the_peak_dot_is_left_standing_above_the_column_and_steps_down_slowly() {
        // The second position cue, and the one that carries information after the music stops.
        let t = builtin::ladder_classic();
        let g = geometry(190, 60).unwrap();
        let mut l = Ladder::default();
        let mut c = Canvas::new(190, 60);
        l.draw(&mut c, &t, &frame(0.62));
        let peak = dot_rung(&c, &g, 4).expect("a loud frame must place the dot");
        assert!(peak >= g.rungs - 2, "a loud frame should drive the dot near the top, got {peak}");

        // One silent frame: the column must collapse while the dot stays up.
        l.draw(&mut c, &t, &frame(0.0));
        assert_eq!(lit_rungs(&c, &g, 4), 0, "the column must fall immediately");
        let held = dot_rung(&c, &g, 4).expect("the dot must still be held after the level drops");
        assert!(held >= peak - 1, "the dot must not fall with the column: {peak} -> {held}");

        // ...and then step down. 0.010/frame over an 8-rung ladder is ~12.5 frames per rung.
        for _ in 0..40 {
            l.draw(&mut c, &t, &frame(0.0));
        }
        let later = dot_rung(&c, &g, 4).expect("still falling, not gone");
        assert!(later < held, "the dot must fall: {held} -> {later}");
        for _ in 0..200 {
            l.draw(&mut c, &t, &frame(0.0));
        }
        assert_eq!(dot_rung(&c, &g, 4), None, "the dot must eventually clear");
    }

    #[test]
    fn the_audio_response_actually_changes_the_pixels() {
        // Guards against a no-op implementation: three levels must give three different
        // images, and the count of lit LEDs must rise monotonically with level.
        let t = builtin::ladder_classic();
        let g = geometry(190, 60).unwrap();
        let mut prev_bits: Option<Vec<u32>> = None;
        let mut prev_n = -1;
        for level in [0.12f32, 0.30, 0.48, 0.62] {
            let c = render(&t, &[frame(level)]);
            let n = lit_rungs(&c, &g, 7);
            assert!(n > prev_n, "level {level} lit {n} rungs, no more than the level below ({prev_n})");
            prev_n = n;
            let bits = c.bits().to_vec();
            if let Some(p) = &prev_bits {
                assert_ne!(p, &bits, "level {level} rendered identically to the level below it");
            }
            prev_bits = Some(bits);
        }
        assert!(prev_n >= 7, "a loud frame must fill most of the ladder, got {prev_n}");
    }

    #[test]
    fn nothing_is_drawn_outside_the_panel_rect() {
        // The bloom spreads up to MAX_BLOOM px in every direction from a panel inset only
        // 1-2px, so without the clip-back the halo lands on the bare taskbar.
        let t = builtin::ladder_plasma();
        let c = render(&t, &[frame(1.0)]);
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
    fn a_wide_panel_adds_columns_instead_of_fattening_the_leds() {
        let narrow = geometry(190, 60).unwrap();
        let wide = geometry(380, 60).unwrap();
        assert_eq!(narrow.cols, 15, "the reference panel");
        assert!(wide.cols >= narrow.cols * 2 - 1, "twice the width must roughly double the columns, got {}", wide.cols);
        assert_eq!(wide.rungs, narrow.rungs, "the rung count is set by height, which has not changed");
        // And the wide render must actually differ per column, i.e. the extra columns are fed
        // their own slice of the spectrum rather than a copy.
        let t = builtin::ladder_modern();
        let mut l = Ladder::default();
        let mut c = Canvas::new(380, 60);
        let mut d = FrameData::default();
        for (i, v) in d.levels.iter_mut().enumerate() {
            let x = i as f32 / 63.0;
            *v = 0.15 + 0.5 * (x * 7.0).sin().abs();
        }
        d.peaks = d.levels;
        l.draw(&mut c, &t, &d);
        let heights: Vec<i32> = (0..wide.cols).map(|b| lit_rungs(&c, &wide, b)).collect();
        let hi = *heights.iter().max().unwrap();
        let lo = *heights.iter().min().unwrap();
        assert!(hi - lo >= 3, "a wide ladder must still show a profile: {heights:?}");
    }

    #[test]
    fn renders_at_every_plausible_size_without_panicking() {
        let t = builtin::ladder_vintage();
        for (w, h) in [
            (190, 60), (380, 60), (456, 60), (240, 72), (150, 48),
            (96, 40), (40, 24), (16, 16), (12, 12), (8, 60), (1, 1), (0, 0),
        ] {
            let mut l = Ladder::default();
            let mut c = Canvas::new(w, h);
            // Several frames: the peak-hold buffer is allocated on the first and reused after.
            for _ in 0..3 {
                l.draw(&mut c, &t, &frame(0.7));
            }
            assert_eq!(
                c.bits().len(),
                (w.max(0) * h.max(0)) as usize,
                "{w}x{h} changed the canvas size"
            );
        }
    }

    #[test]
    fn nan_and_infinity_cannot_poison_the_held_peaks() {
        // f32::clamp does NOT sanitise NaN, and `hold` persists across frames - one bad frame
        // could otherwise park every dot at a NaN height for the rest of the session. So the
        // real assertion is the RECOVERY: a normal frame after a poisoned one must render a
        // normal ladder.
        let t = builtin::ladder_amber();
        let g = geometry(190, 60).unwrap();
        let mut l = Ladder::default();
        let mut c = Canvas::new(190, 60);

        let mut bad = frame(0.5);
        bad.levels[0] = f32::NAN;
        bad.levels[7] = f32::INFINITY;
        bad.levels[8] = f32::NEG_INFINITY;
        bad.peaks[3] = f32::NAN;
        bad.dt_ms = f32::NAN;
        for _ in 0..4 {
            l.draw(&mut c, &t, &bad);
        }
        let all_nan = FrameData { levels: [f32::NAN; crate::dsp::bands::NUM_BANDS], ..FrameData::default() };
        for _ in 0..4 {
            l.draw(&mut c, &t, &all_nan);
        }
        // An all-NaN spectrum must read as silence, not as a stuck ladder.
        assert_eq!(lit_rungs(&c, &g, 5), 0, "a NaN spectrum must not light the ladder");

        l.draw(&mut c, &t, &frame(0.62));
        assert!(lit_rungs(&c, &g, 5) >= 6, "the ladder must recover after poisoned frames");
        assert!(dot_rung(&c, &g, 5).is_some(), "the held peak must recover too");

        // A NaN theme gain must not take the meter out either - `sensitivity` is user-authored.
        let mut mad = t.clone();
        mad.sensitivity = f32::NAN;
        mad.bloom = f32::NAN;
        mad.glow_strength = f32::INFINITY;
        let mut c2 = Canvas::new(190, 60);
        Ladder::default().draw(&mut c2, &mad, &frame(0.5));
        assert!(c2.bits().iter().any(|p| *p != 0), "a NaN-tuned theme must still draw something");
    }

    #[test]
    fn every_ladder_colourway_renders_and_differs() {
        let mut seen: Vec<Vec<u32>> = Vec::new();
        for t in builtin::all().into_iter().filter(|t| t.family == "ladder") {
            let mut c = Canvas::new(190, 60);
            Ladder::default().draw(&mut c, &t, &frame(0.55));
            let bits = c.bits().to_vec();
            assert!(bits.iter().any(|p| *p != 0), "{} rendered nothing", t.id);
            for prior in &seen {
                assert_ne!(prior, &bits, "{} renders identically to another colourway", t.id);
            }
            seen.push(bits);
        }
        assert_eq!(seen.len(), 5, "the family ships five colourways");
    }

    /// Run: cargo test --release dump_ladder_frames -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_ladder_frames() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();

        // An uneven spectrum, so the columns sit at different heights, plus a decayed peak on
        // top: the dumps have to show the two things the family is judged on.
        let spectrum = |gain: f32| -> FrameData {
            let mut d = FrameData::default();
            for (i, v) in d.levels.iter_mut().enumerate() {
                let x = i as f32 / 63.0;
                *v = ((0.14 + 0.72 * (x * 9.0).sin().abs()) * (1.0 - x * 0.35) * gain).min(1.0);
            }
            d.peaks = d.levels;
            d
        };
        let d = spectrum(1.0);
        let loud = spectrum(1.5);

        let write = |name: String, c: &Canvas| {
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
        for t in builtin::all().into_iter().filter(|t| t.family == "ladder") {
            let mut l = Ladder::default();
            let mut c = Canvas::new(190, 60);
            // A loud frame first, then the real one, so the held peak dots are sitting ABOVE
            // the columns exactly as they do on falling music - a single frame would put every
            // dot on its own head LED and the marker would not be visible as a marker.
            l.draw(&mut c, &t, &loud);
            for _ in 0..14 {
                l.draw(&mut c, &t, &d);
            }
            write(format!("ladder-{}.rgba", t.id), &c);
            n += 1;
        }

        // The wide mode, on the reference colourway, at the same decayed-peak moment.
        let t = builtin::ladder_classic();
        let mut l = Ladder::default();
        let mut c = Canvas::new(380, 60);
        l.draw(&mut c, &t, &loud);
        for _ in 0..14 {
            l.draw(&mut c, &t, &d);
        }
        write("ladder-classic-wide.rgba".into(), &c);
        n += 1;

        // And silence, which is where the unlit housings have to carry the whole panel.
        let mut c = Canvas::new(190, 60);
        Ladder::default().draw(&mut c, &t, &frame(0.0));
        write("ladder-classic-silent.rgba".into(), &c);
        n += 1;

        println!("wrote {n} ladder dumps (190x60, one 380x60, one silent) to {}", dir.display());
    }
}

