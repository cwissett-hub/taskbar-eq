//! The chroma-field family: a Felipe Pantone stripe field whose stripe WIDTHS are the meter.
//!
//! Every other family here is a mark on a panel - a bar, a needle, a valve - and the audio
//! changes how bright or how tall that mark is. This one has no marks. The panel IS the
//! field: hard-edged vertical stripes at maximum chroma, in spectrum order left to right,
//! filling the interior edge to edge. What the audio drives is each stripe's WIDTH.
//!
//! Four things carry the idea, and each is load-bearing rather than decorative:
//!
//! - **The widths are ZERO-SUM.** They always sum to exactly the panel interior, so a
//!   swelling stripe necessarily pinches its neighbours. This is the whole design. It is
//!   also a structural answer to the failure mode this project keeps hitting, where a meter
//!   technically responds but reads as static: with a fixed total, something always has to
//!   give way, so every edge in the field moves rather than one element brightening. Width
//!   is a POSITION cue, and position is resolved far more readily than intensity at 190x60
//!   (measured on the valve row: brightness alone gave a 1.16x spread between a driven
//!   element and its neighbour, below the visible threshold).
//!
//! - **BLACK KEYLINES, 1px, hard.** Authentic to the reference work, and the reason this
//!   family can honestly run at full chroma. A full-chroma rainbow provably cannot clear
//!   this project's 3:1 contrast rule at every hue against any flat panel - measured, pure
//!   blue reaches only 2.36:1 on a near-black panel, and on a light panel yellow drops to
//!   1.00:1. The keyline delineates each stripe regardless of its hue, so legibility does
//!   not depend on hue-versus-panel contrast at all. See the retired-constant note in
//!   `themes::builtin` for the recorded per-colourway opt-in that this buys.
//!
//! - **RGB CHANNEL MISREGISTRATION.** Red and blue displaced horizontally a pixel or two
//!   from green, like a mis-printed page. Done in a STRAIGHT-colour buffer and blitted once
//!   at alpha 255, because mixing channels between premultiplied pixels of differing alpha
//!   is how you get either a broken r,g,b <= a invariant or a see-through pixel - and a
//!   see-through pixel inside the panel is a hole the Windows weather widget shows through.
//!   Applied BEFORE the keylines, so the keyline stays pure ink and keeps doing its job,
//!   with the fringe visible either side of it.
//!
//! - **A HALFTONE screen** over part of the field, ramping as a printed tone does, which
//!   beats against the moving stripe edges into moire.
//!
//! **The glow is a LIGHTBOX, and the ordering is the whole trick.** This family used to refuse
//! bloom outright, on the grounds that it models ink on paper and a halo would soften exactly the
//! hard edges everything here depends on. That reasoning is sound about compositing a halo ON TOP;
//! it is not an argument against a halo BEHIND. The field is now built in its own transparent
//! canvas, bloomed there - `Canvas::bloom` puts the halo underneath the content it came from - and
//! composited over the panel in one pass, with the field inset far enough from the panel edge that
//! the halo has somewhere to escape to.
//!
//! So the print is ink on film on a lightbox: every interior edge is exactly as hard as before,
//! every keyline is still pure ink, and colour bleeds out around the outside. Set `bloom = 0` on a
//! colourway to get the flat print back.

use super::canvas::{Canvas, Rgba};
use super::{Family, FrameData};
use crate::themes::{ChromaParams, Theme};

/// The stripe field's rect inside a panel of `w` x `h`: `(x, y, w, h)`.
///
/// Inset 4px horizontally and 5 vertically, which at 190x60 leaves 182x50 for the stripes.
///
/// The extra 2px each way over the original 2/3 is WHERE THE HALO GOES. The field used to reach
/// within 2px of the panel edge, and a bloom behind something that covers the whole interior is
/// invisible - there is nowhere for it to show. Giving up 4px of width and 4 of height buys a bleed
/// band wide enough to read as backlighting at every size the overlay actually takes.
///
/// **Shared with the tests deliberately.** Two of them hardcoded the old 2px inset and the old 186px
/// interior, and when the field moved they probed 2px off and reported a perfectly good red stripe as
/// a failed keyline. Geometry computed independently in two places is geometry that will disagree.
fn field_rect(w: i32, h: i32) -> (i32, i32, i32, i32) {
    (4, 5, w - 8, h - 10)
}

/// Band level at which a stripe starts to swell, and the span it swells over.
///
/// THE INPUT RANGE IS NOT 0..1: the DSP delivers roughly 0.15..0.65 for active bands on real
/// music, so a mapping tuned across a full 0..1 sweep spends two thirds of its travel on
/// levels that never arrive and reads as dead. Same window the valve row uses, measured the
/// same way - across 0.15..0.65 the response here travels 0.096 -> 1.0.
const RESP_FLOOR: f32 = 0.10;
const RESP_SPAN: f32 = 0.52;

/// Weight given to a group's LOUDEST band rather than its mean.
///
/// AVERAGING HIDES PEAKS. Each stripe covers about 6.4 of the 64 log bands, and a plain mean
/// flattens exactly the single-band peaks that make one stripe differ from its neighbour -
/// measured on the valve row, whose grouping is identical, a mean gave 1.46 dL* between a
/// driven element and its neighbour, below the ~2.3 dL* at which a difference is visible at
/// all. Biasing toward the max took the same case to 9.47 dL*. Invisible to any test that
/// drives every band alike, because mean == max there.
const GROUP_MAX_BIAS: f32 = 0.65;

/// A stripe narrower than this gets no keyline of its own.
///
/// At one pixel the keyline would BE the stripe, so a fully pinched stripe would vanish into
/// the ink instead of surviving as a sliver of colour. Its neighbours' keylines still
/// delineate it, so nothing is lost. Two is the smallest width at which a keyline plus a
/// pixel of colour both fit.
const MIN_KEYLINE_W: i32 = 2;

/// Bass level below which a rise is ignored, so hiss cannot fire the glitch.
const GLITCH_FLOOR: f32 = 0.06;

/// Per-nominal-frame decay of the glitch envelope.
///
/// 0.13 gives about six frames (~100ms at the loop's real rate) of displacement: three at
/// full offset and three at half. Faster than this and the slice moves for a single frame,
/// which reads as a dropped frame rather than as a glitch.
const GLITCH_DECAY: f32 = 0.13;

/// Nominal frame interval the decay above is expressed against.
const NOMINAL_DT_MS: f32 = 16.7;

/// How much a driven stripe lightens its own halftone.
///
/// Tone in print means ink coverage, so a loud stripe must carry LESS ink to read as fuller
/// chroma. Kept modest deliberately: width is this family's meter, and a tone cue strong enough
/// to compete with it would fight the zero-sum geometry rather than support it.
///
/// 0.55 was tried first and the dump killed it. On a realistically driven spectrum the mean
/// response is around 0.65, which at 0.55 relief scaled the whole ramp by 0.64 - and because
/// the dot lattice quantises coverage into a handful of steps, that dropped the entire screen
/// below its first step everywhere except the bottom fifteen rows. The halftone-dominant
/// colourway had no halftone across two thirds of its field whenever music was playing. At 0.30
/// the screen survives across the field and still gives up half its ink between an idle stripe
/// and a driven one.
const HALFTONE_RELIEF: f32 = 0.30;

/// Ceiling on halftone coverage, so the deep end of the ramp never goes solid.
///
/// The dot lattice quantises coverage into a handful of steps, and `halftone_covers` normalises
/// its threshold by the pitch so the steps land at the same fractions of `tone` whatever the
/// pitch: at pitch 3 the reachable coverages are 0%, 11%, 56% and 100%, at pitch 5 they are 0%,
/// 4%, 20%, 52%, 84% and 100%, and 1.0 is solid on both. So this is really choosing the top
/// step, and 0.85 stops one below solid at every pitch.
///
/// Stopping below solid matters: at solid the deepest rows fill completely with ink and the
/// field loses its colour along the bottom edge, which at idle is the "goes black at silence
/// looks broken rather than quiet" failure again.
const HALFTONE_MAX_TONE: f32 = 0.85;

/// Tone the screen starts at, at the shallow end of its ramp.
///
/// Not zero, and the dump is why. A ramp from zero spends its first third below the lattice's
/// first reachable step, so the screened region simply had no ink over its top half - on the
/// halftone-dominant colourway, whose entire identity is the screen, ink only appeared below
/// about row 33 of 54. Starting at 0.35 puts the shallow end on the first or second step
/// instead, so the screen is present wherever it is switched on and the ramp still climbs three
/// steps across the field. It also gives the band-limited colourways a hard top edge where the
/// screen begins, which is what a screened region on a press actually looks like.
const HALFTONE_FLOOR: f32 = 0.35;

/// Stripes to draw for a given interior width.
///
/// Ten at the 190px reference (186px of interior, 18.6px each at rest), and that count is a
/// measured trade rather than a taste call:
///   - TWELVE stripes give 15.5px at rest and 5px when fully pinched. One pixel of that is
///     the keyline and the channel misregistration eats 2px either side, so a pinched stripe
///     has no pure core left at all - it degenerates into fringe.
///   - TEN gives 18.6px at rest and about 6px pinched, which keeps a core after the same
///     shift, and 6.4 bands per stripe - the grouping already approved on the valve row.
///   - EIGHT gives 23.2px at rest and swells to about 46px, a quarter of the whole panel.
///     Eight hue steps across the visible spectrum reads as a flag, not as a print.
///
/// Scaled by width rather than fixed, for the reason the valve row scales its count: at 380px
/// a fixed ten becomes 37px stripes, which is the same flag problem. Twenty stripes at 380px
/// keep the 18.8px pitch that was tuned, and narrow each stripe's share of the spectrum so
/// neighbours differ more.
fn stripe_count(interior: i32, stripe_px: f32) -> usize {
    let target = if stripe_px.is_finite() { stripe_px.clamp(4.0, 64.0) } else { 18.0 };
    (((interior as f32 / target).floor() as i64).clamp(4, 40)) as usize
}

/// Whole-pixel stripe widths that sum to `interior` EXACTLY.
///
/// The exactness is not a nicety. A rounding residue that drifts either leaves the right edge
/// of the interior unpainted - a transparent pixel inside the panel, which the weather widget
/// shows through - or overflows the panel onto the taskbar. So the integerisation is done by
/// largest remainder (the Hare quota): floor every exact width, then hand the leftover pixels
/// one each to the stripes with the largest fractional parts, ties to the lower index. That
/// gives the extra pixel to whoever most deserved it instead of always to the left edge,
/// which would leave the leftmost stripe permanently a pixel fat, and it is deterministic for
/// a given frame.
///
/// One consequence worth stating: the tie-break is by index, so at EXACT rest - every stripe
/// weighted identically, every fractional part equal - the remainder does land on the leftmost
/// stripes (at 186px and ten stripes, stripes 0..5 are 19px and 6..9 are 18px). Under any real
/// spectrum the fractions differ and the remainder follows the largest ones instead. A 1px
/// static asymmetry at silence is not worth defeating determinism for.
///
/// `swell` is the weight a fully driven stripe carries over an idle one minus one, so the
/// width ratio between them is `1 + swell`. Zero-sum means only the SHAPE of the spectrum
/// moves the field: drive every band equally and every stripe is the same width again, which
/// is correct - a flat spectrum has no shape.
fn stripe_widths(resp: &[f32], interior: i32, swell: f32) -> Vec<i32> {
    let n = resp.len();
    if n == 0 {
        return Vec::new();
    }
    if interior <= 0 {
        return vec![0; n];
    }
    let swell = if swell.is_finite() { swell.clamp(0.0, 12.0) } else { 4.0 };
    // is_finite BEFORE clamp: f32::clamp does NOT sanitise NaN - every comparison against
    // NaN is false, so clamp falls through and returns the NaN, which would then poison the
    // weight total and hand every stripe a zero width.
    let weights: Vec<f32> = resp
        .iter()
        .map(|r| 1.0 + swell * if r.is_finite() { r.clamp(0.0, 1.0) } else { 0.0 })
        .collect();
    let total: f32 = weights.iter().sum();
    if !total.is_finite() || total <= 0.0 {
        // Cannot happen with the guard above (every weight is at least 1.0), but an equal
        // split is the right degradation if it ever could.
        let mut out = vec![interior / n as i32; n];
        let rem = interior - out.iter().sum::<i32>();
        for i in 0..(rem.max(0) as usize).min(n) {
            out[i] += 1;
        }
        return out;
    }

    let mut out = Vec::with_capacity(n);
    let mut fracs: Vec<(usize, f32)> = Vec::with_capacity(n);
    for (i, wt) in weights.iter().enumerate() {
        let exact = interior as f32 * wt / total;
        let base = exact.floor().max(0.0);
        out.push(base as i32);
        fracs.push((i, exact - base));
    }
    let mut rem = interior - out.iter().sum::<i32>();
    // Largest fractional part first; index ascending on a tie. partial_cmp cannot fail here
    // (every frac is finite by construction) but unwrap_or keeps it total regardless.
    fracs.sort_by(|a, b| {
        b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(&b.0))
    });
    let mut k = 0usize;
    while rem > 0 && !fracs.is_empty() {
        out[fracs[k % fracs.len()].0] += 1;
        rem -= 1;
        k += 1;
    }
    out
}

/// Straight-colour mix of `over` onto `base`, always opaque.
///
/// Alpha is forced to 255 rather than carried, because this composites within the field
/// buffer, and every pixel of that buffer must reach the canvas fully opaque - the overlay is
/// composited with per-pixel alpha over the weather widget, so anything less is a hole.
fn mix(base: Rgba, over: Rgba, k: f32) -> Rgba {
    let k = if k.is_finite() { k.clamp(0.0, 1.0) } else { 0.0 };
    let f = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * k).round().clamp(0.0, 255.0) as u8;
    Rgba::new(f(base.r, over.r), f(base.g, over.g), f(base.b, over.b), 255)
}

/// Whether a halftone dot covers this pixel at the given ink coverage.
///
/// Diamond dots on a `pitch` lattice. The diagonal dot geometry is the point: a screen whose
/// edges run at 45 degrees to the stripes beats against every vertical stripe edge, and it is
/// that beat - not the dots themselves - that produces the moire.
///
/// `tone` is ink coverage: 0 is bare paper with no ink at all (hence the +1 on each side of the
/// comparison) and 1.0 is a solid. The threshold is normalised by the LARGEST distance the
/// lattice can produce at this pitch, so the tone steps land at the same fractions of `tone`
/// whatever the pitch - otherwise `HALFTONE_MAX_TONE` would mean "one step below solid" at one
/// pitch and "solid" at another, and a colourway changing its pitch would silently change how
/// dark its screen goes.
fn halftone_covers(x: i32, y: i32, pitch: i32, tone: f32) -> bool {
    let p = pitch.clamp(2, 16);
    let u = x + y;
    let v = x - y;
    let du = (u.rem_euclid(p) - p / 2).abs();
    let dv = (v.rem_euclid(p) - p / 2).abs();
    let d = (du + dv) as f32;
    let max_d = 2 * (p / 2);
    let tone = if tone.is_finite() { tone.clamp(0.0, 1.0) } else { 0.0 };
    d + 1.0 <= tone * (max_d + 1) as f32
}

/// Small integer hash, for the glitch slice and the barcode's irregular ink runs.
/// A hash rather than a random source so a given frame always renders identically.
fn hash(seed: u32, k: u32) -> u32 {
    let mut x = seed.wrapping_mul(0x9E37_79B1).wrapping_add(k.wrapping_mul(0x85EB_CA6B));
    x ^= x >> 15;
    x = x.wrapping_mul(0x2545_F491);
    x ^= x >> 13;
    x
}

#[derive(Default)]
pub struct Chroma {
    /// Previous frame's bass level, for RISE detection. A level threshold fires continuously
    /// on any bassy track; a rise fires on the kick, which is what a glitch should land on.
    prev_bass: f32,
    /// Glitch envelope, 1.0 at the strike and falling. Also gates the displacement.
    glitch: f32,
    /// Advanced on every strike so consecutive glitches pick different slices.
    seed: u32,
}

impl Chroma {
    /// Blend of a stripe's mean and its loudest band - see `GROUP_MAX_BIAS`.
    fn level_for(d: &FrameData, i: usize, stripes: usize) -> f32 {
        let n = d.levels.len();
        let stripes = stripes.max(1);
        let lo = i * n / stripes;
        let hi = (((i + 1) * n / stripes).max(lo + 1)).min(n);
        let mut acc = 0.0;
        let mut cnt = 0.0;
        let mut peak = 0.0f32;
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

    /// Maps a group level onto 0..1 of stripe swell.
    fn response(level: f32, sensitivity: f32) -> f32 {
        if !level.is_finite() {
            return 0.0;
        }
        (((level - RESP_FLOOR) / RESP_SPAN) * sensitivity.max(0.0)).clamp(0.0, 1.0)
    }

    /// Advances the bass-transient detector and returns the glitch envelope.
    ///
    /// Fires on a RISE in bass rather than a level, the same way the vaporwave family's
    /// lightning does: a level test fires every frame of a bassy passage, so the slice would
    /// simply sit displaced, which is not a glitch. Reads the raw levels, since the width
    /// remap would compress exactly the rise being detected.
    fn update_glitch(&mut self, d: &FrameData, p: &ChromaParams) -> f32 {
        let dt = if d.dt_ms.is_finite() {
            (d.dt_ms / NOMINAL_DT_MS).clamp(0.25, 4.0)
        } else {
            1.0
        };
        let n = d.levels.len().min(4).max(1);
        let mut acc = 0.0;
        let mut cnt = 0.0;
        for v in &d.levels[..n.min(d.levels.len())] {
            if v.is_finite() {
                acc += *v;
                cnt += 1.0;
            }
        }
        let bass = if cnt > 0.0 { acc / cnt } else { 0.0 };
        let sens = if p.glitch_sens.is_finite() { p.glitch_sens.clamp(0.0, 1.0) } else { 0.0 };
        // 0.03 at full sensitivity up to 0.25 at none. The lower bound is above the
        // frame-to-frame wobble of a steady bass line, measured on the smoothed levels the
        // renderer receives; the upper bound is a rise no music reaches, which is how a
        // colourway switches the glitch off entirely.
        let need = 0.03 + (1.0 - sens) * 0.22;
        if sens > 0.0 && bass - self.prev_bass > need && bass > GLITCH_FLOOR {
            self.glitch = 1.0;
            self.seed = self.seed.wrapping_add(1);
        }
        self.prev_bass = if bass.is_finite() { bass } else { 0.0 };
        self.glitch = (self.glitch - GLITCH_DECAY * dt).max(0.0);
        if !self.glitch.is_finite() {
            self.glitch = 0.0;
        }
        self.glitch
    }

    /// Which ink a scrambled stripe takes: BALANCED but non-periodic.
    ///
    /// Each ink appears exactly once per cycle of `len` stripes, and the order within each cycle is
    /// reshuffled by the cycle index. So the sequence never repeats and never clusters.
    ///
    /// The obvious version - hash the stripe index and take it modulo the palette size - is neither.
    /// Measured on the six Riso inks across a 190px field it put FIVE of eight stripes on the same
    /// orange, which does not read as a random palette, it reads as a broken one. A hash is uniform in
    /// the limit; eight draws is not the limit.
    ///
    /// Fisher-Yates, seeded per cycle, over at most a handful of entries - the cost is nothing and it
    /// is exact rather than approximately fair.
    fn shuffled_slot(i: usize, len: usize) -> usize {
        if len == 0 {
            return 0;
        }
        let cycle = (i / len) as u32;
        let mut order = [0usize; 16];
        let n = len.min(16);
        for (j, o) in order[..n].iter_mut().enumerate() {
            *o = j;
        }
        for j in (1..n).rev() {
            let pick = (hash(0x5EED ^ cycle, j as u32) as usize) % (j + 1);
            order.swap(j, pick);
        }
        order[i % n]
    }

    /// The ink for one stripe.
    ///
    /// `inks` empty is the spectrum ramp: hue from position, at whatever chroma the colourway
    /// asks for. A non-empty `inks` list is a fixed process palette instead - CMYK stripes
    /// cycle it in order, because a print's plates are in order; the barcode colourway
    /// scrambles it, because a barcode's runs are not periodic and a cycling palette against
    /// varying widths reads as a repeating pattern.
    fn stripe_colour(t: &Theme, i: usize, n: usize, accent: Option<usize>) -> Rgba {
        let p = &t.chroma;
        let x01 = (i as f32 + 0.5) / n.max(1) as f32;
        let hue = || {
            let span = if p.hue_span.is_finite() { p.hue_span } else { 0.85 };
            let off = if p.hue_offset.is_finite() { p.hue_offset } else { 0.0 };
            let sat = if p.sat.is_finite() { p.sat.clamp(0.0, 1.0) } else { 1.0 };
            let base = if p.lightness.is_finite() { p.lightness.clamp(0.05, 0.98) } else { 0.72 };
            let tilt = if p.lightness_tilt.is_finite() { p.lightness_tilt.clamp(0.0, 1.0) } else { 0.0 };
            let h = off + x01 * span;
            // A little of each hue's natural lightness, so the yellows do not go olive. Referenced to
            // 0.75, which is about the mean natural lightness around the hue circle, so a tilt leaves
            // the ramp's average weight where `lightness` put it instead of lifting the whole field.
            let l = (base + tilt * (Rgba::oklch_natural_l(h) - 0.75)).clamp(0.05, 0.98);
            // OKLCh, not HSV. See `ChromaParams::lightness` - an HSV sweep at full saturation and
            // value is uneven in hue spacing AND in lightness, and both were visible on the panel.
            // `sat` scales the chroma against the most the gamut holds at this lightness, so 1.0 is
            // still "as chromatic as possible" and every hue is as chromatic as it can be WITHOUT
            // changing weight relative to its neighbours.
            Rgba::from_oklch(l, sat * Rgba::oklch_max_chroma(l, h), h, 1.0)
        };
        if accent == Some(i) {
            return hue();
        }
        if p.inks.is_empty() {
            return hue();
        }
        let k = if p.scramble { Self::shuffled_slot(i, p.inks.len()) } else { i % p.inks.len() };
        let c = Rgba::from_hex(&p.inks[k], 1.0);
        // A malformed hex parses to TRANSPARENT; force it opaque so a typo in a theme file
        // cannot punch a hole through the panel.
        Rgba::new(c.r, c.g, c.b, 255)
    }
}

impl Family for Chroma {
    fn id(&self) -> &'static str {
        "chroma"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());
        // Advanced before the size guard, so a canvas too small to draw still keeps the
        // transient detector's history current - otherwise a resize would leave `prev_bass`
        // stale and fire a spurious glitch on the first frame back.
        let glitch = self.update_glitch(d, &t.chroma);
        let p = &t.chroma;

        c.clear();
        let panel = Rgba::from_hex(&t.panel, t.panel_alpha);
        c.rounded_rect(1, 2, (w - 2).max(1), (h - 4).max(1), 3, panel);

        let (fx, fy, fw, fh) = field_rect(w, h);
        if fw < 8 || fh < 4 {
            // Too small for a stripe field with keylines in it. The panel alone is the
            // graceful degradation, exactly as the valve row does below its own minimum.
            return;
        }

        let n = stripe_count(fw, p.stripe_px);
        let resp: Vec<f32> = (0..n)
            .map(|i| Self::response(Self::level_for(d, i, n), t.sensitivity))
            .collect();
        let widths = stripe_widths(&resp, fw, p.swell);

        // The accent stripe: the loudest group, used only by the colourways that withhold
        // chroma. It makes the one coloured stripe in a monochrome field a POSITION cue for
        // where the energy is, rather than decoration.
        let accent = if p.accent {
            let mut best = (0usize, f32::MIN);
            for (i, r) in resp.iter().enumerate() {
                if *r > best.1 {
                    best = (i, *r);
                }
            }
            Some(best.0)
        } else {
            None
        };

        let ink = {
            let k = Rgba::from_hex(&p.ink, 1.0);
            Rgba::new(k.r, k.g, k.b, 255)
        };

        // The field is built in a STRAIGHT-colour buffer and blitted once. Doing the channel
        // misregistration on the canvas instead would mean mixing channels between
        // premultiplied pixels, which either breaks the r,g,b <= a invariant or leaves a
        // pixel below alpha 255 inside the panel - a hole the weather widget shows through.
        let mut buf = vec![Rgba::new(0, 0, 0, 255); (fw * fh) as usize];
        let hband = if p.halftone.is_finite() { p.halftone.clamp(0.0, 1.0) } else { 0.0 };
        let hrows = (fh as f32 * hband).round() as i32;
        let htop = fh - hrows;
        let hstrength = if p.halftone_strength.is_finite() {
            p.halftone_strength.clamp(0.0, 1.0)
        } else {
            0.0
        };

        let mut x = 0i32;
        for i in 0..n {
            let sw = widths[i];
            if sw <= 0 {
                continue;
            }
            let col = Self::stripe_colour(t, i, n, accent);
            for row in 0..fh {
                // Halftone: a printed tone RAMP down the band, not a flat screen. Coverage
                // grows with depth into the band, and a driven stripe carries less ink - see
                // HALFTONE_RELIEF.
                let tone = if hrows > 0 && row >= htop && hstrength > 0.0 {
                    let depth = (row - htop + 1) as f32 / hrows.max(1) as f32;
                    let ramp = HALFTONE_FLOOR + (1.0 - HALFTONE_FLOOR) * depth;
                    (ramp * HALFTONE_MAX_TONE * hstrength
                        * (1.0 - HALFTONE_RELIEF * resp[i]))
                        .clamp(0.0, 1.0)
                } else {
                    0.0
                };
                for dx in 0..sw {
                    let px = x + dx;
                    let v = if tone > 0.0 && halftone_covers(px, row, p.halftone_pitch, tone) {
                        mix(col, ink, 1.0)
                    } else {
                        col
                    };
                    buf[(row * fw + px) as usize] = v;
                }
            }
            x += sw;
        }

        // RGB channel misregistration. Red and blue planes displaced horizontally from green,
        // like a mis-printed page - the single most identifiable element of the reference work.
        //
        // The source x is CLAMPED to the field, not wrapped and not skipped: at the left and
        // right edges a shifted plane has nothing to read from, and edge-extending is both the
        // correct print analogue (the plate simply runs off the paper) and the only option that
        // cannot leave an under-opaque pixel there.
        let sr = p.shift_r.clamp(-6, 6);
        let sb = p.shift_b.clamp(-6, 6);
        if sr != 0 || sb != 0 {
            let src = buf.clone();
            for row in 0..fh {
                let at = |xx: i32| src[(row * fw + xx.clamp(0, fw - 1)) as usize];
                for px in 0..fw {
                    let g = at(px);
                    buf[(row * fw + px) as usize] =
                        Rgba::new(at(px - sr).r, g.g, at(px - sb).b, 255);
                }
            }
        }

        // Keylines, AFTER the misregistration so each one stays pure ink. They are what
        // delineates a stripe independently of its hue, which is the whole contrast argument
        // for running at full chroma - a fringed keyline would give that up.
        let mut x = 0i32;
        for i in 0..n {
            let sw = widths[i];
            if sw >= MIN_KEYLINE_W {
                for row in 0..fh {
                    buf[(row * fw + x) as usize] = ink;
                }
            }
            x += sw;
        }
        // Close the field on the right. Painted OVER the last stripe's final column rather
        // than taking a pixel from it, so the zero-sum total is untouched.
        for row in 0..fh {
            buf[(row * fw + (fw - 1)) as usize] = ink;
        }

        // Glitch displacement: one horizontal slice of the field shoved sideways for a few
        // frames after a bass transient.
        //
        // Quantised to two offsets rather than following the envelope continuously, because a
        // continuous slide reads as a wobble - a glitch has to snap. Rotated WITHIN the field
        // (wrapped, not shifted) so the slice cannot leave an unpainted column behind, which
        // would be a transparent hole in the panel.
        if glitch > 0.2 && p.glitch_px != 0 {
            let g = hash(self.seed, 1);
            let sh = p.glitch_px.clamp(-24, 24).unsigned_abs().min(fw as u32 / 2).max(1) as i32;
            let off = if glitch > 0.6 { sh } else { sh / 2 };
            let off = if g & 1 == 0 { off } else { -off };
            if off != 0 {
                let sy = (hash(self.seed, 2) % fh.max(1) as u32) as i32;
                // 4..=10 rows at the reference height, and never more than a third of the
                // field: a slice taller than that displaces the composition rather than
                // glitching it.
                let sh_rows = (4 + (hash(self.seed, 3) % 7) as i32).min((fh / 3).max(1));
                let src = buf.clone();
                for row in sy..(sy + sh_rows).min(fh) {
                    for px in 0..fw {
                        let from = (px - off).rem_euclid(fw);
                        buf[(row * fw + px) as usize] = src[(row * fw + from) as usize];
                    }
                }
                // A 1px tear along the top of the displaced slice, in the theme's hot colour.
                // Without it the slice reads as a mis-drawn frame; with it, as a tear.
                if sy < fh {
                    let tear = Rgba::from_hex(&t.hot, 1.0);
                    let tear = Rgba::new(tear.r, tear.g, tear.b, 255);
                    for px in 0..fw {
                        buf[(sy * fw + px) as usize] = tear;
                    }
                }
            }
        }

        // The field goes onto its OWN transparent canvas, not straight onto the panel, so it can
        // be bloomed with the halo landing behind it - see the module note on the lightbox. Alpha is
        // forced to 255 on every field pixel, which is the invariant the whole buffer approach
        // exists to guarantee: no pixel inside the field is ever see-through.
        let mut film = Canvas::new(w, h);
        for row in 0..fh {
            for px in 0..fw {
                let v = buf[(row * fw + px) as usize];
                film.fill_rect(fx + px, fy + row, 1, 1, Rgba::new(v.r, v.g, v.b, 255));
            }
        }
        if t.bloom > 0.0 && t.glow_strength > 0.0 {
            // `bloom` composites its halo UNDERNEATH the content that produced it, so this leaves
            // every field pixel bit-identical and only adds light outside the field's own footprint.
            // That is precisely the property that makes a glow compatible with a family built on
            // hard edges.
            film.bloom(t.bloom.max(0.0) as i32, t.glow_strength.clamp(0.0, 1.0));
        }
        c.draw_over(&film);

        // Insurance, not correction: the field is already flush inside the rounded panel at
        // every row, so this clips nothing today. It is here because every future change to
        // the geometry above would otherwise be one arithmetic slip away from painting on the
        // taskbar itself.
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
        for v in d.levels.iter_mut() {
            *v = level;
        }
        d.peaks = d.levels;
        d
    }

    /// An UNEVEN spectrum. A test driving every band alike cannot see a per-band bug, and it
    /// also cannot see this family's core mechanism at all: zero-sum widths only move when the
    /// spectrum has a shape.
    fn uneven() -> FrameData {
        let mut d = FrameData::default();
        for (i, v) in d.levels.iter_mut().enumerate() {
            let x = i as f32 / 63.0;
            *v = (0.16 + 0.48 * (x * 9.0).sin().abs()) * (1.0 - x * 0.35);
        }
        d.peaks = d.levels;
        d
    }

    /// One band peaking inside an otherwise quiet group.
    fn one_loud_band(stripes: usize, loud: usize, hot: f32, quiet: f32) -> FrameData {
        let mut d = FrameData::default();
        let n = d.levels.len();
        for v in d.levels.iter_mut() {
            *v = quiet;
        }
        let lo = loud * n / stripes;
        let hi = ((loud + 1) * n / stripes).min(n);
        d.levels[(lo + hi) / 2] = hot;
        d.peaks = d.levels;
        d
    }

    fn render(t: &Theme, d: &FrameData, w: i32, h: i32, frames: usize) -> Canvas {
        let mut f = Chroma::default();
        let mut c = Canvas::new(w, h);
        for _ in 0..frames {
            f.draw(&mut c, t, d);
        }
        c
    }

    // ---------- the zero-sum property ----------

    #[test]
    fn widths_always_sum_to_the_interior_exactly() {
        // The property the whole design rests on, and the one whose failure is a transparent
        // hole at the right edge of the panel. Swept over counts, interiors and shapes,
        // including the pathological ones.
        for &interior in &[186i32, 376, 176, 45, 9, 8, 1, 0, -7] {
            for n in [4usize, 7, 10, 12, 20, 40] {
                let shapes: Vec<Vec<f32>> = vec![
                    vec![0.0; n],
                    vec![1.0; n],
                    (0..n).map(|i| i as f32 / n as f32).collect(),
                    (0..n).map(|i| if i == n / 2 { 1.0 } else { 0.0 }).collect(),
                    (0..n).map(|i| ((i * 7) % 11) as f32 / 10.0).collect(),
                    // poisoned, because f32::clamp does not sanitise NaN
                    (0..n)
                        .map(|i| match i % 3 {
                            0 => f32::NAN,
                            1 => f32::INFINITY,
                            _ => 0.4,
                        })
                        .collect(),
                ];
                for shape in shapes {
                    let out = stripe_widths(&shape, interior, 4.0);
                    assert_eq!(out.len(), n);
                    let sum: i32 = out.iter().sum();
                    let want = interior.max(0);
                    assert_eq!(
                        sum, want,
                        "n={n} interior={interior} summed to {sum}, not {want}: {out:?}"
                    );
                    assert!(out.iter().all(|w| *w >= 0), "negative width: {out:?}");
                }
            }
        }
    }

    #[test]
    fn a_swelling_stripe_pinches_its_neighbours() {
        // The design claim, stated as arithmetic: because the total is fixed, driving one
        // stripe must take pixels OFF the others. A meter that merely brightened would leave
        // its neighbours untouched.
        let n = 10;
        let rest = stripe_widths(&vec![0.3f32; n], 186, 4.0);
        let mut driven = vec![0.3f32; n];
        driven[4] = 1.0;
        let hit = stripe_widths(&driven, 186, 4.0);
        assert!(
            hit[4] > rest[4] + 4,
            "the driven stripe must visibly swell: {} -> {}",
            rest[4],
            hit[4]
        );
        for i in [0usize, 3, 5, 9] {
            assert!(
                hit[i] < rest[i],
                "stripe {i} must be pinched by its neighbour's swell: {} -> {}",
                rest[i],
                hit[i]
            );
        }
        assert_eq!(hit.iter().sum::<i32>(), 186, "and the total is still the interior");
    }

    #[test]
    fn the_rounding_remainder_is_spread_and_not_dumped_on_one_stripe() {
        // Largest-remainder, not "give it all to stripe 0". With an interior that does not
        // divide evenly the leftovers must land on different stripes, or the leftmost stripe
        // is permanently fat.
        let n = 7;
        let resp: Vec<f32> = (0..n).map(|i| 0.2 + 0.1 * i as f32).collect();
        let out = stripe_widths(&resp, 186, 4.0);
        assert_eq!(out.iter().sum::<i32>(), 186);
        // The exact widths differ by at most one pixel from their unrounded value, which is
        // what "spread deliberately" means - no stripe absorbs the whole remainder.
        let total: f32 = resp.iter().map(|r| 1.0 + 4.0 * r).sum();
        for (i, r) in resp.iter().enumerate() {
            let exact = 186.0 * (1.0 + 4.0 * r) / total;
            assert!(
                (out[i] as f32 - exact).abs() < 1.0 + 1e-3,
                "stripe {i} is {} against an exact {exact:.2}",
                out[i]
            );
        }
    }

    #[test]
    fn the_rendered_field_leaves_no_unpainted_column_in_the_panel() {
        // The rendered consequence of zero-sum. A 1px residue at the right edge would be a
        // transparent pixel inside the panel - a hole the weather widget shows through - so
        // this asserts the interior is opaque edge to edge at several widths, including ones
        // that do not divide evenly by the stripe count.
        for (w, h) in [(190, 60), (380, 60), (191, 60), (177, 47), (240, 72), (40, 24)] {
            for t in builtin::all().into_iter().filter(|t| t.family == "chroma") {
                let c = render(&t, &uneven(), w, h, 8);
                for y in 3..(h - 3) {
                    for x in 2..(w - 2) {
                        assert_eq!(
                            c.get(x, y).a,
                            255,
                            "{} at {w}x{h}: ({x},{y}) is see-through inside the panel",
                            t.id
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn no_chroma_colourway_leaves_a_transparent_pixel_at_any_level() {
        // The family-local twin of render::opacity's sweep. Swept across levels because the
        // bug that shipped in the segmented family was LEVEL-DEPENDENT - it only appeared on
        // loud bars, so a single-level test missed it entirely.
        let (w, h) = (190, 60);
        for t in builtin::all().into_iter().filter(|t| t.family == "chroma") {
            let mut f = Chroma::default();
            for step in 0..=10 {
                let level = step as f32 / 10.0;
                let mut d = FrameData::default();
                for (i, v) in d.levels.iter_mut().enumerate() {
                    *v = (level * (0.7 + 0.3 * ((i % 7) as f32 / 6.0))).clamp(0.0, 1.0);
                }
                d.peaks = d.levels;
                let mut c = Canvas::new(w, h);
                for _ in 0..6 {
                    f.draw(&mut c, &t, &d);
                }
                for y in 3..(h - 3) {
                    for x in 2..(w - 2) {
                        assert_eq!(
                            c.get(x, y).a,
                            255,
                            "{} at level {level}: hole at ({x},{y})",
                            t.id
                        );
                    }
                }
            }
        }
    }

    // ---------- the surface ----------

    #[test]
    fn every_stripe_boundary_carries_a_black_keyline() {
        // The keyline is what makes full chroma legible - it delineates a stripe regardless of
        // hue, so legibility does not depend on hue-versus-panel contrast. If these columns
        // stop being ink, the contrast opt-in in themes::builtin is no longer earned.
        let t = builtin::chroma_spectrum();
        let d = uneven();
        let c = render(&t, &d, 190, 60, 8);
        let (fx, fy, fw, _) = field_rect(190, 60);
        let n = stripe_count(fw, t.chroma.stripe_px);
        let resp: Vec<f32> = (0..n)
            .map(|i| Chroma::response(Chroma::level_for(&d, i, n), t.sensitivity))
            .collect();
        let widths = stripe_widths(&resp, fw, t.chroma.swell);
        // Three rows into the field, which is above the halftone band, so a dark pixel there can
        // only be a keyline.
        let y = fy + 3;
        let mut x = fx;
        let mut checked = 0;
        for w in &widths {
            if *w >= MIN_KEYLINE_W {
                let px = c.get(x, y);
                let sum = px.r as u32 + px.g as u32 + px.b as u32;
                assert!(sum < 60, "keyline at x={x} is not ink: {px:?}");
                // and the pixel just inside it must NOT be ink, or the stripe is all keyline
                let inside = c.get(x + 1, y);
                assert!(
                    inside.r as u32 + inside.g as u32 + inside.b as u32 > sum,
                    "the stripe beside the keyline at x={x} is as dark as the keyline itself"
                );
                checked += 1;
            }
            x += *w;
        }
        assert!(checked >= 8, "expected a keyline on nearly every stripe, checked {checked}");
    }

    #[test]
    fn the_channels_are_misregistered_beside_a_stripe_boundary() {
        // The most identifiable element of the reference work, and the one most likely to be
        // silently defeated: the fringe lives at stripe boundaries, and the keylines sit on
        // those same boundaries. This asserts a fringe SURVIVES beside the keyline - a pixel
        // whose red comes from one stripe while its green and blue come from the next.
        let t = builtin::chroma_spectrum();
        let d = flat(0.4);
        let shifted = render(&t, &d, 190, 60, 8);

        let mut plain = t.clone();
        plain.chroma.shift_r = 0;
        plain.chroma.shift_b = 0;
        let registered = render(&plain, &d, 190, 60, 8);

        let y = 6;
        let mut fringe = 0;
        for x in 2..188 {
            if shifted.get(x, y) != registered.get(x, y) {
                fringe += 1;
            }
        }
        assert!(
            fringe >= 10,
            "the channel shift must leave a visible fringe, only {fringe} columns differ"
        );

        // And specifically: a column that matches NEITHER of its two neighbours in all three
        // channels, which is what a mis-registered plane looks like and a plain edge does not.
        let mut split = false;
        for x in 4..186 {
            let a = shifted.get(x - 2, y);
            let b = shifted.get(x, y);
            let cc = shifted.get(x + 2, y);
            if b != a && b != cc && (b.r == a.r || b.r == cc.r) && (b.g != a.g || b.b != cc.b) {
                split = true;
                break;
            }
        }
        assert!(split, "no pixel shows one channel taken from a different stripe than the others");
    }

    #[test]
    fn the_halftone_screen_lays_ink_and_a_driven_stripe_carries_less_of_it() {
        // Tone in print IS ink coverage, so the loud stripe must be the one with less ink -
        // which also means this test fails if the halftone stops reading the audio at all.
        let t = builtin::chroma_halftone();
        let ink_rows = |level: f32| -> u32 {
            let d = flat(level);
            let c = render(&t, &d, 190, 60, 8);
            let mut dark = 0;
            // The lower half of the field, where the tone ramp is deepest.
            for y in 40..56 {
                for x in 10..180 {
                    let px = c.get(x, y);
                    if (px.r as u32 + px.g as u32 + px.b as u32) < 90 {
                        dark += 1;
                    }
                }
            }
            dark
        };
        // Measured on the shipped colourway over that window (2720 pixels): 1188 inked at level
        // 0.12, 896 at 0.4 and 436 at 0.95 - so the screen gives up about two thirds of its ink
        // between a quiet stripe and a driven one, monotonically. The bounds below sit either
        // side of that.
        let quiet = ink_rows(0.12);
        let mid = ink_rows(0.4);
        let loud = ink_rows(0.95);
        assert!(quiet > 900, "the halftone must actually lay ink: {quiet} dark pixels at idle");
        assert!(mid < quiet, "the ramp must be monotonic: {quiet} at 0.12 vs {mid} at 0.4");
        assert!(
            (loud as f32) < quiet as f32 * 0.55,
            "a driven stripe must carry visibly less ink: {quiet} at idle vs {loud} when driven"
        );
    }

    #[test]
    fn the_halftone_screen_reaches_the_shallow_end_of_the_region_it_covers() {
        // The regression the eyeball dump caught, and which no other test here could see: with
        // the ramp starting at zero tone, the top half of the screened region fell below the dot
        // lattice's first reachable coverage step and carried no ink at all - on the one
        // colourway whose entire identity is the screen. See HALFTONE_FLOOR.
        //
        // Measured as a PER-PIXEL DIFFERENTIAL against the same colourway with the screen switched
        // off, rather than as a count of pixels below an absolute luminance.
        //
        // The absolute version was calibrated against this colourway when its ramp was darker, and it
        // broke the moment the ramp's lightness changed - reporting 376 inked against 300 where it
        // wanted a margin of 100. Nothing about the screen had regressed; the threshold was measuring
        // the palette as much as the ink. A differential cannot: it compares each pixel with what
        // that same pixel is without the screen, so it is immune to the ramp, to the keylines, and to
        // the lightbox halo alike.
        let t = builtin::chroma_halftone();
        let mut off = t.clone();
        off.chroma.halftone = 0.0;
        let (fx, fy, fw, fh) = field_rect(190, 60);
        let on_c = render(&t, &flat(0.4), 190, 60, 8);
        let off_c = render(&off, &flat(0.4), 190, 60, 8);
        let lum = |p: Rgba| p.r as i32 + p.g as i32 + p.b as i32;
        let screened = |y0: i32, y1: i32| -> u32 {
            let mut n = 0;
            for y in y0..y1 {
                for x in fx..(fx + fw) {
                    if lum(off_c.get(x, y)) - lum(on_c.get(x, y)) > 60 {
                        n += 1;
                    }
                }
            }
            n
        };
        let upper = screened(fy, fy + fh / 2);
        let whole = screened(fy, fy + fh);
        // The shallow end of the ramp is the half that used to carry NO ink at all, because it fell
        // below the dot lattice's first reachable coverage step.
        assert!(
            upper > 150,
            "the screen must lay ink over the shallow end of its region too, only {upper} pixels darkened in the upper half"
        );
        // And more over the whole field than over half of it, or the ramp is not ramping.
        assert!(
            whole > upper * 3 / 2,
            "the screen is not deepening with the ramp: {upper} in the upper half, {whole} overall"
        );
    }

    #[test]
    fn a_halftone_dot_screen_is_present_and_periodic() {
        // Guards the screen itself rather than its response: an all-ink or all-paper band
        // would satisfy the coverage test above at one level but is not a halftone.
        let mut on = 0;
        let mut off = 0;
        for y in 0..24 {
            for x in 0..24 {
                if halftone_covers(x, y, 3, 0.5) {
                    on += 1;
                } else {
                    off += 1;
                }
            }
        }
        assert!(on > 0 && off > 0, "a 50% screen must be neither solid nor blank: {on}/{off}");
        // Periodic on the lattice, which is what beats against the stripes.
        for y in 0..8 {
            for x in 0..8 {
                assert_eq!(
                    halftone_covers(x, y, 3, 0.5),
                    halftone_covers(x + 3, y + 3, 3, 0.5),
                    "the screen must repeat on its own pitch at ({x},{y})"
                );
            }
        }
        assert!(!halftone_covers(0, 0, 3, 0.0), "zero tone must be bare paper");
    }

    #[test]
    fn a_bass_transient_glitches_one_slice_and_a_steady_bass_does_not() {
        // A RISE, not a level - see update_glitch. A level threshold would leave the slice
        // permanently displaced on any bassy track, which is not a glitch.
        let t = builtin::chroma_misreg();
        let quiet = flat(0.05);
        let mut kick = flat(0.05);
        for v in kick.levels[..4].iter_mut() {
            *v = 0.8;
        }
        kick.peaks = kick.levels;

        let mut f = Chroma::default();
        let mut c = Canvas::new(190, 60);
        // Settle on the quiet frame, so the first frames' rise from silence has decayed.
        for _ in 0..12 {
            f.draw(&mut c, &t, &quiet);
        }
        let before: Vec<u32> = c.bits().to_vec();
        // Steady bass, held: the rise happens once, so after it decays the field must be
        // stable again.
        for _ in 0..12 {
            f.draw(&mut c, &t, &kick);
        }
        let settled: Vec<u32> = c.bits().to_vec();
        for _ in 0..3 {
            f.draw(&mut c, &t, &kick);
        }
        assert_eq!(
            settled,
            c.bits().to_vec(),
            "a held bass level must not keep glitching - the trigger is a rise"
        );
        assert_ne!(before, settled, "the kick must change the field at all");

        // Now the transient itself: drop back to quiet, then hit it, and the very next frame
        // must differ from the same frame rendered without the transient.
        let mut a = Chroma::default();
        let mut b = Chroma::default();
        let mut ca = Canvas::new(190, 60);
        let mut cb = Canvas::new(190, 60);
        for _ in 0..12 {
            a.draw(&mut ca, &t, &quiet);
            b.draw(&mut cb, &t, &quiet);
        }
        a.draw(&mut ca, &t, &kick);
        // b sees the same spectrum but with the glitch disabled by its colourway knob.
        let mut calm = t.clone();
        calm.chroma.glitch_sens = 0.0;
        b.draw(&mut cb, &calm, &kick);
        assert_ne!(
            ca.bits(),
            cb.bits(),
            "the frame after a bass transient must be displaced"
        );
        // And the displacement is a SLICE: most rows must be identical between the two.
        //
        // Counted twice, because the lightbox halo widens the answer. A displaced band of rows also
        // changes the bloom for a bloom-radius either side of itself, so on the shipped configuration
        // 22 rows differ where the slice itself is 8 - which failed a bound of 20 with nothing wrong.
        // The tight bound belongs on the glitch alone; the shipped configuration gets the bound that
        // actually matters, which is that it is nowhere near the whole field.
        let (fx, fy, fw, fh) = field_rect(190, 60);
        let rows_differing = |x: &Canvas, y: &Canvas| -> i32 {
            let mut n = 0;
            for row in fy..(fy + fh) {
                if (fx..(fx + fw)).any(|c| x.get(c, row) != y.get(c, row)) {
                    n += 1;
                }
            }
            n
        };
        let with_glow = rows_differing(&ca, &cb);
        assert!(
            with_glow < fh / 2,
            "the glitch must displace a slice, not the whole field: {with_glow} of {fh} rows moved"
        );
        // The same comparison with the halo held off, which isolates the slice itself.
        let mut flat_t = t.clone();
        flat_t.bloom = 0.0;
        let mut flat_calm = flat_t.clone();
        flat_calm.chroma.glitch_sens = 0.0;
        let (mut fa, mut fb) = (Chroma::default(), Chroma::default());
        let (mut fca, mut fcb) = (Canvas::new(190, 60), Canvas::new(190, 60));
        for _ in 0..12 {
            fa.draw(&mut fca, &flat_t, &quiet);
            fb.draw(&mut fcb, &flat_t, &quiet);
        }
        fa.draw(&mut fca, &flat_t, &kick);
        fb.draw(&mut fcb, &flat_calm, &kick);
        let bare = rows_differing(&fca, &fcb);
        assert!(
            (1..=20).contains(&bare),
            "the glitch slice itself must be a slice: {bare} rows moved with the halo off"
        );
    }

    // ---------- audio response ----------

    #[test]
    fn the_response_window_spends_its_range_on_levels_the_dsp_actually_produces() {
        // THE INPUT RANGE IS NOT 0..1. Real music sits at roughly 0.15..0.65 for active
        // bands, so a mapping tuned across a full sweep would use a third of its travel.
        let lo = Chroma::response(0.15, 1.0);
        let hi = Chroma::response(0.65, 1.0);
        assert!(hi - lo > 0.75, "the music window must cover most of the range: {lo} -> {hi}");
        assert_eq!(Chroma::response(0.0, 1.0), 0.0, "silence maps to zero, not a pedestal");
        assert_eq!(Chroma::response(1.0, 1.0), 1.0, "full scale reaches the top");
        assert!(
            Chroma::response(0.3, 2.0) > Chroma::response(0.3, 1.0),
            "sensitivity is the user-facing knob and must do something"
        );
        assert_eq!(Chroma::response(f32::NAN, 1.0), 0.0, "NaN must not reach the geometry");
    }

    #[test]
    fn the_group_reducer_is_biased_toward_the_peak_not_the_mean() {
        // Guards GROUP_MAX_BIAS. Averaging hides peaks: a group of one loud band among six
        // quiet ones has a mean far below its max.
        let d = one_loud_band(10, 0, 0.9, 0.1);
        let n = d.levels.len();
        let hi = (n / 10).max(1);
        let mean = d.levels[..hi].iter().sum::<f32>() / hi as f32;
        let peak = d.levels[..hi].iter().copied().fold(0.0f32, f32::max);
        let got = Chroma::level_for(&d, 0, 10);
        assert!(got > mean + (peak - mean) * 0.5, "must sit above the midpoint: {got}");
        assert!(got <= peak + 1e-6, "but never above the peak: {got} vs {peak}");
    }

    #[test]
    fn a_single_peaking_band_visibly_swells_its_own_stripe() {
        // The per-band case, which a test driving every band alike cannot see at all: with a
        // mean reducer the swell here would be a pixel or two.
        let d = one_loud_band(10, 4, 0.65, 0.20);
        let n = 10;
        let resp: Vec<f32> = (0..n)
            .map(|i| Chroma::response(Chroma::level_for(&d, i, n), 1.0))
            .collect();
        let widths = stripe_widths(&resp, 186, 4.0);
        let neighbour = widths[5].max(widths[3]);
        assert!(
            widths[4] > neighbour + 6,
            "the peaking band must swell its own stripe: {} vs neighbour {}",
            widths[4],
            neighbour
        );
    }

    #[test]
    fn a_shaped_spectrum_moves_nearly_every_edge_in_the_field_and_all_of_them_over_time() {
        // The structural claim, and the reason this family cannot read as static: with the
        // total fixed, giving one stripe pixels TAKES them from others, so a shape change moves
        // interior boundaries rather than one element's brightness.
        //
        // The field's last edge is the interior's right-hand end and never moves - that IS the
        // zero-sum property - so only n-1 edges are movable. A single shape can still leave one
        // of them where it was by coincidence of rounding, so this asserts nearly all of them
        // move for one shape, and that every movable edge moves at some point over a sweep.
        let n = 10;
        let edges = |resp: &[f32]| -> Vec<i32> {
            let mut acc = 0;
            stripe_widths(resp, 186, 4.0)
                .iter()
                .map(|w| {
                    acc += *w;
                    acc
                })
                .collect()
        };
        let flat_e = edges(&vec![0.4f32; n]);
        let resp_for = |d: &FrameData| -> Vec<f32> {
            (0..n).map(|i| Chroma::response(Chroma::level_for(d, i, n), 1.0)).collect()
        };
        let shaped = edges(&resp_for(&uneven()));
        let moved = flat_e.iter().zip(&shaped).filter(|(p, q)| p != q).count();
        assert!(
            moved >= n - 2,
            "only {moved} of the {} movable edges moved on a shaped spectrum",
            n - 1
        );
        assert_eq!(
            flat_e[n - 1], shaped[n - 1],
            "the right-hand end of the field must never move - that is the zero-sum property"
        );

        // Every movable edge, over a swept spectrum. A per-band bug that froze one stripe's
        // group would show up here and nowhere else.
        let mut ever = vec![false; n - 1];
        for step in 0..24 {
            let phase = step as f32 / 24.0;
            let mut d = FrameData::default();
            for (i, v) in d.levels.iter_mut().enumerate() {
                let x = i as f32 / 63.0;
                *v = 0.15 + 0.45 * (x * 7.0 + phase * std::f32::consts::TAU).sin().abs();
            }
            d.peaks = d.levels;
            let e = edges(&resp_for(&d));
            for k in 0..(n - 1) {
                if e[k] != flat_e[k] {
                    ever[k] = true;
                }
            }
        }
        assert!(
            ever.iter().all(|m| *m),
            "these edges never moved across a swept spectrum: {:?}",
            ever.iter().enumerate().filter(|(_, m)| !**m).map(|(i, _)| i).collect::<Vec<_>>()
        );
    }

    #[test]
    fn the_stripe_count_holds_its_tuned_pitch_as_the_panel_widens() {
        assert_eq!(stripe_count(186, 18.0), 10, "the reference panel keeps ten stripes");
        assert_eq!(stripe_count(376, 18.0), 20, "double the width doubles the stripes");
        let pitch = |interior: i32| interior as f32 / stripe_count(interior, 18.0) as f32;
        let reference = pitch(186);
        for interior in [186, 236, 376, 452, 596] {
            let p = pitch(interior);
            assert!(
                (p - reference).abs() < 4.0,
                "at interior {interior} the pitch drifted to {p:.1} from the tuned {reference:.1}"
            );
        }
        assert!(stripe_count(4000, 18.0) <= 40, "capped");
        assert!(stripe_count(8, 18.0) >= 4, "and never fewer than a handful");
        assert!(stripe_count(186, f32::NAN) >= 4, "a poisoned pitch must not empty the field");
    }

    #[test]
    fn the_barcode_colourway_withholds_chroma_except_for_the_loudest_stripe() {
        // "Chroma withheld almost entirely" is the whole point of that colourway, and the one
        // coloured stripe is a position cue for where the energy is - so it must track the
        // audio, not sit at a fixed index.
        let t = builtin::chroma_barcode();
        let n = stripe_count(186, t.chroma.stripe_px);
        let sat = |c: Rgba| {
            let hi = c.r.max(c.g).max(c.b) as i32;
            let lo = c.r.min(c.g).min(c.b) as i32;
            hi - lo
        };
        let accent_for = |loud: usize| -> usize {
            let d = one_loud_band(n, loud, 0.9, 0.05);
            let resp: Vec<f32> = (0..n)
                .map(|i| Chroma::response(Chroma::level_for(&d, i, n), t.sensitivity))
                .collect();
            let mut best = (0usize, f32::MIN);
            for (i, r) in resp.iter().enumerate() {
                if *r > best.1 {
                    best = (i, *r);
                }
            }
            best.0
        };
        for loud in [1usize, n / 2, n - 2] {
            assert_eq!(accent_for(loud), loud, "the accent must follow the loudest group");
        }
        // And every other stripe is greyscale.
        let mut coloured = 0;
        for i in 0..n {
            let c = Chroma::stripe_colour(&t, i, n, Some(3));
            if sat(c) > 24 {
                coloured += 1;
            }
        }
        assert_eq!(coloured, 1, "exactly one stripe may carry chroma, found {coloured}");
    }

    // ---------- contrast ----------

    /// WCAG relative luminance of an Rgba, matching themes::builtin's own helper.
    fn luminance(c: Rgba) -> f32 {
        let ch = |v: u8| {
            let v = v as f32 / 255.0;
            if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * ch(c.r) + 0.7152 * ch(c.g) + 0.0722 * ch(c.b)
    }

    fn contrast(a: Rgba, b: Rgba) -> f32 {
        let (la, lb) = (luminance(a), luminance(b));
        let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
        (hi + 0.05) / (lo + 0.05)
    }

    #[test]
    fn every_stripe_colour_clears_its_own_colourways_declared_contrast_floor() {
        // The honest version of the 3:1 rule for a full-chroma family. themes::builtin's
        // `every_lit_colour_clears_three_to_one_against_its_own_panel` can only see a
        // colourway's declared `lit`; these are the colours actually printed, swept densely
        // enough to hit every hue of the ramp and every entry of a fixed ink palette.
        //
        // Also asserts the floor is TIGHT. A recorded 2.30 for a measured 2.36 is a deliberate
        // decision; a blanket 1.0 would be an accident, and the tightness bound is what makes
        // the difference fail rather than pass silently.
        for t in builtin::all().into_iter().filter(|t| t.family == "chroma") {
            let panel = Rgba::from_hex(&t.panel, 1.0);
            let ink = Rgba::from_hex(&t.chroma.ink, 1.0);
            let mut worst = (f32::MAX, 0usize);
            for i in 0..360 {
                let col = Chroma::stripe_colour(&t, i, 360, None);
                // The ink is the KEY plate, not a lit colour: a black stripe is meant to read
                // as black, and it is delineated by its neighbours rather than by contrast.
                if contrast(col, ink) < 1.5 {
                    continue;
                }
                let c = contrast(col, panel);
                if c < worst.0 {
                    worst = (c, i);
                }
            }
            assert!(
                worst.0 >= t.contrast_floor,
                "{}: stripe {} reaches only {:.2}:1 against its panel {}, below its declared \
                 floor of {:.2}",
                t.id,
                worst.1,
                worst.0,
                t.panel,
                t.contrast_floor
            );
            // Tightness applies only where the floor was LOWERED. A colourway sitting on the
            // project's 3.0 has nothing to justify; one that opted down to 2.30 has to be
            // measurably near it, which is what makes a deliberate 2.3:1 pass while a slack
            // blanket floor - declared to make some accidental 1.2:1 fit - fails.
            if t.contrast_floor < 3.0 {
                assert!(
                    worst.0 < t.contrast_floor + 0.6,
                    "{}: declared floor {:.2} is slack - the worst colour actually measures \
                     {:.2}:1, so a real regression toward the floor would pass unnoticed",
                    t.id,
                    t.contrast_floor,
                    worst.0
                );
            }
        }
    }

    /// Every colourway's actual inks and their contrast, so `lit` and `contrast_floor` can be set
    /// from measurement instead of from memory.
    ///
    /// Run: cargo test --release probe_chroma_inks -- --ignored --nocapture
    #[test]
    #[ignore]
    fn probe_chroma_inks() {
        for t in builtin::all().into_iter().filter(|t| t.family == "chroma") {
            let panel = Rgba::from_hex(&t.panel, 1.0);
            let ink = Rgba::from_hex(&t.chroma.ink, 1.0);
            let mut worst = (f32::MAX, Rgba::TRANSPARENT);
            let mut hexes = Vec::new();
            for i in 0..10 {
                let col = Chroma::stripe_colour(&t, i, 10, None);
                hexes.push(format!("#{:02x}{:02x}{:02x}", col.r, col.g, col.b));
            }
            for i in 0..360 {
                let col = Chroma::stripe_colour(&t, i, 360, None);
                if contrast(col, ink) < 1.5 {
                    continue;
                }
                let c = contrast(col, panel);
                if c < worst.0 {
                    worst = (c, col);
                }
            }
            println!(
                "{:18} worst {:.2}:1 at #{:02x}{:02x}{:02x} floor {:.2} ramp {}",
                t.id,
                worst.0,
                worst.1.r,
                worst.1.g,
                worst.1.b,
                t.contrast_floor,
                hexes.join(" ")
            );
        }
    }

    #[test]
    fn the_perceptual_ramp_holds_its_lightness_and_so_clears_the_contrast_rule_at_every_hue() {
        // The whole reason the ramp moved from HSV to OKLCh, asserted rather than described.
        //
        // The old sweep's lightness ran from L* 97 at yellow to L* 32 at blue, which is what made the
        // field flicker in brightness across its width AND what forced this family to declare a
        // 2.30:1 contrast floor - pure blue cannot clear 3:1 on any panel. Hold lightness constant and
        // both problems are the same problem, solved once.
        let t = builtin::chroma_spectrum();
        let panel = Rgba::from_hex(&t.panel, 1.0);
        let mut lums = Vec::new();
        let mut worst = f32::MAX;
        for i in 0..360 {
            let col = Chroma::stripe_colour(&t, i, 360, None);
            lums.push(Rgba::oklab_l_of(col));
            worst = worst.min(contrast(col, panel));
        }
        let (lo, hi) = (
            lums.iter().copied().fold(f32::MAX, f32::min),
            lums.iter().copied().fold(f32::MIN, f32::max),
        );
        // The tilt deliberately allows SOME variation - a dead-flat ramp turns yellow to olive - but
        // it must stay far inside the 0.52 spread an HSV sweep produces.
        assert!(
            hi - lo < 0.22,
            "lightness across the ramp spans {:.3} ({lo:.3}..{hi:.3}), which is a visible flicker",
            hi - lo
        );
        assert!(hi - lo > 0.01, "a dead-flat ramp makes the yellows olive - the tilt did nothing");
        // And the payoff: the project's rule, met at every hue, with no opt-in.
        assert!(
            worst >= 3.0,
            "worst hue reaches only {worst:.2}:1 - the ramp no longer earns the 3:1 rule"
        );
        assert_eq!(t.contrast_floor, 3.0, "this colourway should be back on the standard floor");
    }

    #[test]
    fn scrambled_inks_are_balanced_rather_than_merely_random() {
        // A plain hash-per-stripe put five of the six Riso inks' eight stripes on one orange. Every
        // ink must appear about equally often over a run, and no ink may take more than a third.
        for len in [2usize, 3, 4, 6, 8] {
            let n = len * 6;
            let mut counts = vec![0usize; len];
            for i in 0..n {
                counts[Chroma::shuffled_slot(i, len)] += 1;
            }
            assert!(
                counts.iter().all(|c| *c == 6),
                "with {len} inks over {n} stripes the counts are {counts:?}, not balanced"
            );
            // Balanced is not enough on its own - walking the palette in order is perfectly balanced
            // and reads as a repeating pattern, which is what `scramble` exists to avoid.
            if len >= 3 {
                let ordered: Vec<usize> = (0..n).map(|i| i % len).collect();
                let got: Vec<usize> = (0..n).map(|i| Chroma::shuffled_slot(i, len)).collect();
                assert_ne!(ordered, got, "with {len} inks the scramble is just the plain cycle");
            }
        }
    }

    // ---------- robustness ----------

    #[test]
    fn renders_at_every_plausible_size_and_survives_nan_and_infinity() {
        let sizes = [
            (190, 60),
            (380, 60),
            (456, 60),
            (240, 72),
            (150, 48),
            (96, 40),
            (40, 24),
            (12, 12),
            (8, 6),
            (1, 1),
        ];
        for t in builtin::all().into_iter().filter(|t| t.family == "chroma") {
            for (w, h) in sizes {
                for spoil in 0..4 {
                    let mut d = uneven();
                    match spoil {
                        0 => {}
                        1 => {
                            d.levels[0] = f32::NAN;
                            d.levels[31] = f32::NAN;
                            d.peaks[5] = f32::NAN;
                            d.dt_ms = f32::NAN;
                            d.time_s = f32::NAN;
                        }
                        2 => {
                            d.levels[3] = f32::INFINITY;
                            d.levels[63] = f32::NEG_INFINITY;
                            d.dt_ms = 0.0;
                        }
                        _ => {
                            for v in d.levels.iter_mut() {
                                *v = f32::NAN;
                            }
                        }
                    }
                    let mut f = Chroma::default();
                    let mut c = Canvas::new(w, h);
                    for _ in 0..4 {
                        f.draw(&mut c, &t, &d);
                    }
                    assert_eq!(
                        c.bits().len(),
                        (w.max(0) * h.max(0)) as usize,
                        "{} at {w}x{h} changed the canvas size",
                        t.id
                    );
                    // A poisoned frame must not empty the field at the sizes that draw one.
                    if w >= 24 && h >= 16 {
                        for y in 3..(h - 3) {
                            for x in 2..(w - 2) {
                                assert_eq!(
                                    c.get(x, y).a,
                                    255,
                                    "{} at {w}x{h} spoil {spoil}: hole at ({x},{y})",
                                    t.id
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn a_poisoned_spectrum_cannot_permanently_corrupt_the_glitch_state() {
        // f32::clamp does not sanitise NaN, and this family carries exactly the kind of
        // persistent state the vaporwave scroll phase was permanently corrupted through.
        let t = builtin::chroma_spectrum();
        let mut f = Chroma::default();
        let mut c = Canvas::new(190, 60);
        let mut bad = flat(0.4);
        bad.levels[0] = f32::NAN;
        bad.levels[1] = f32::INFINITY;
        bad.dt_ms = f32::NAN;
        for _ in 0..6 {
            f.draw(&mut c, &t, &bad);
        }
        assert!(f.prev_bass.is_finite(), "prev_bass was poisoned: {}", f.prev_bass);
        assert!(f.glitch.is_finite(), "the glitch envelope was poisoned: {}", f.glitch);
        // and it still renders a full field afterwards
        let good = render(&t, &uneven(), 190, 60, 4);
        assert!(good.bits().iter().any(|p| *p != 0));
    }

    #[test]
    fn every_chroma_colourway_renders_and_differs() {
        let mut seen: Vec<Vec<u32>> = Vec::new();
        let mut ids = Vec::new();
        for t in builtin::all().into_iter().filter(|t| t.family == "chroma") {
            let c = render(&t, &uneven(), 190, 60, 8);
            let bits = c.bits().to_vec();
            assert!(bits.iter().any(|p| *p != 0), "{} rendered nothing", t.id);
            for (prior, pid) in seen.iter().zip(&ids) {
                assert_ne!(prior, &bits, "{} renders identically to {}", t.id, pid);
            }
            ids.push(t.id.clone());
            seen.push(bits);
        }
        assert_eq!(seen.len(), 7, "the family ships seven colourways, found {}", seen.len());
    }

    /// Run: cargo test --release dump_chroma_frames -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_chroma_frames() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();
        let mut n = 0usize;
        for t in builtin::all().into_iter().filter(|t| t.family == "chroma") {
            for (tag, w, h) in [("190", 190, 60), ("380", 380, 60)] {
                // A shaped spectrum plus a bass hit on the last frame, so one dump per
                // colourway shows the glitch and the widths at the same time.
                let mut f = Chroma::default();
                let mut c = Canvas::new(w, h);
                for k in 0..24 {
                    let mut d = FrameData::default();
                    let phase = k as f32 / 24.0;
                    for (i, v) in d.levels.iter_mut().enumerate() {
                        let x = i as f32 / 63.0;
                        *v = (0.16 + 0.48 * ((x * 9.0 + phase * 4.0).sin().abs()))
                            * (1.0 - x * 0.35);
                    }
                    // A kick on the final frame: prev_bass is low, so this is a RISE.
                    if k == 23 {
                        for v in d.levels[..4].iter_mut() {
                            *v = 0.85;
                        }
                    }
                    d.peaks = d.levels;
                    d.dt_ms = 16.7;
                    d.time_s = k as f32 * 0.0167;
                    f.draw(&mut c, &t, &d);
                }
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
                std::fs::write(dir.join(format!("chroma-{}-{tag}.rgba", t.id)), &out).unwrap();
                n += 1;
            }
        }
        println!("wrote {n} chroma dumps ({} colourways x 190/380) to {}", n / 2, dir.display());
    }

    /// The perceptual ramp at a spread of lightnesses, for choosing one by eye.
    ///
    /// Run: cargo test --release dump_chroma_lightness -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_chroma_lightness() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();
        for (l, tilt) in [
            (0.62f32, 0.0f32),
            (0.62, 0.35),
            (0.62, 0.60),
            (0.70, 0.0),
            (0.70, 0.35),
            (0.70, 0.60),
        ] {
            let mut t = builtin::chroma_spectrum();
            t.chroma.lightness = l;
            t.chroma.lightness_tilt = tilt;
            let mut f = Chroma::default();
            let mut c = Canvas::new(190, 60);
            for k in 0..24 {
                let mut d = FrameData::default();
                let phase = k as f32 / 24.0;
                for (i, v) in d.levels.iter_mut().enumerate() {
                    let x = i as f32 / 63.0;
                    *v = (0.16 + 0.48 * ((x * 9.0 + phase * 4.0).sin().abs())) * (1.0 - x * 0.35);
                }
                d.peaks = d.levels;
                d.dt_ms = 16.7;
                d.time_s = k as f32 * 0.0167;
                f.draw(&mut c, &t, &d);
            }
            let (w, h) = (190, 60);
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
            let tag = format!("{}-{}", (l * 100.0).round() as i32, (tilt * 100.0).round() as i32);
            std::fs::write(dir.join(format!("chromaL-{tag}.rgba")), &out).unwrap();
        }
        println!("wrote 6 lightness/tilt variants to {}", dir.display());
    }
}

