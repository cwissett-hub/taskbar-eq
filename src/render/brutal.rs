//! The brutalist family: heavy concrete blocks that slam between two orientations on the beat.
//!
//! Asked for as "the brutalist bars as a separate theme". It is the design that was cut when the
//! frenchcore family turned into rave lasers, and it is different enough to earn its own family rather
//! than being a colourway of anything: the rave family is all light and no mass, and this is all mass.
//!
//! # THE ORIENTATION FLIPS ON THE BEAT
//!
//! In one state the blocks rise from the floor, in the other they hang from the ceiling, and an onset
//! toggles between them. The whole panel slams between two configurations several times a second.
//!
//! That is the idea worth keeping from the original sketch, and the reason is that it is a strobe made of
//! POSITION rather than brightness. It looks violent while leaving the house rule intact - `tube.rs:54-60`
//! measured a driven element 1.46 dL* brighter than its neighbour as invisible against a ~2.3 dL*
//! threshold, so brightness could not carry this even if it were allowed to.
//!
//! The consequence, which is intended: flipping destroys frame-to-frame comparability of the block TOPS.
//! You cannot track a block's tip across a flip, because the tip moves the full height of the panel. What
//! stays comparable is its LENGTH, which is what actually encodes the level - so the meter is unharmed and
//! the slam is free. It does mean the peak-hold marks are anchored to each block's own base rather than to
//! a fixed panel row; anchored to a row they would appear to leap the panel's height on every beat.
//!
//! # No glow, no gradient, no ornament
//!
//! `bloom` is 0 on every colourway here, the way the chroma family sets it to zero: a halo softens exactly
//! the edges this family is about. Raw concrete, hard rectangles, and the gaps between blocks doing the
//! work a keyline would do elsewhere. Half the band count at double the width, because the subject is
//! MASS and a thin bar has none.

use crate::render::canvas::{Canvas, Rgba};
use crate::render::{Family, FrameData};
use crate::themes::Theme;

/// Blocks across the panel. Half the usual band count, at double the width.
///
/// Eleven at 380px gives 28px blocks with 5px gaps - a block wide enough to read as a slab rather than as
/// a bar, which is the entire point. More blocks would be thinner and this family would just be the
/// segmented meter with the segments removed.
const BLOCKS: usize = 11;

/// The gap between blocks, in pixels. It does the job a keyline does in other families.
///
/// Five, not one or two: at 1-2px the gap closes up under any halo and the blocks weld into a single mass,
/// and this project has already measured that a 2-row waist always fills under a 1px closing. Five
/// survives, and a wide gap is itself brutalist - the concrete is the ornament.
const GAP: i32 = 5;

/// The onset detector for the flip: flux ratio and refractory.
///
/// The same values the blossom family's branch shake uses, which measured 190 / 143 / 98 fires per minute
/// over the repo's three real-music fixtures - so the panel flips between one and three times a second on
/// real material. That is the intended violence.
const FLIP_RATIO: f32 = 2.8;
const FLIP_REFRACTORY_MS: f32 = 200.0;

/// The level window: `vapor`'s MEASURED p10-p90 of real music. Not a 0..1 mapping, which renders dead,
/// and not normalised against the frame's loudest band, which is provably inert at p50 0.819.
const LEVEL_FLOOR: f32 = 0.119;
const LEVEL_SPAN: f32 = 0.456;
const LEVEL_GAMMA: f32 = 0.6;

/// The shortest a block ever gets, in pixels.
///
/// Not zero. A block that vanishes takes its gap with it and the grid's rhythm breaks; a stub still reads
/// as a block at rest. It is also what makes the flip visible on a quiet passage, where there would
/// otherwise be nothing to flip.
const STUB_PX: i32 = 3;

/// The peak-hold cap's thickness in pixels.
const CAP_PX: i32 = 2;

/// The flourish: THE MONOLITH. Every block slams to full height at once and the panel inverts, so the
/// blocks become dark voids in a lit field, then it releases.
///
/// The inversion is the part that makes it read. Going merely brighter or taller would not: this project
/// measured a flourish changing 38.5% of the panel and still being reported as never happening, because it
/// was not a change of KIND. Figure and ground swapping is a change of kind.
///
/// The inversion is CONTINUOUS, driven by the envelope rather than a threshold - the background lifts
/// toward `lit` while the blocks darken toward the panel colour, and the two cross over in the middle. A
/// threshold would put two hard jumps in the middle of a one-shot decay, which is the snap this project
/// has now been reported for twice.
const SLAB_MS: f32 = 1100.0;

/// The smallest panel this family will draw on.
const MIN_W: i32 = 60;
const MIN_H: i32 = 18;

#[derive(Default)]
pub struct Brutal {
    onset: crate::dsp::onset::Flux,
    /// `true` means the blocks hang from the ceiling.
    hanging: bool,
    flourish: crate::dsp::flourish::Trigger,
    slab: crate::dsp::flourish::Envelope,
}

fn lerp(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let t = t.clamp(0.0, 1.0);
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    Rgba { r: f(a.r, b.r), g: f(a.g, b.g), b: f(a.b, b.b), a: f(a.a, b.a) }
}

impl Brutal {
    /// The block grid: `(x of block 0, block width)`. `None` if the panel cannot hold the grid.
    fn grid(w: i32) -> Option<(i32, i32)> {
        let fw = w - 4;
        let total_gap = GAP * (BLOCKS as i32 + 1);
        let bw = (fw - total_gap) / BLOCKS as i32;
        if bw < 3 {
            return None;
        }
        // Centre whatever is left over, so the grid is not flush against one side.
        let used = bw * BLOCKS as i32 + total_gap;
        let x0 = 2 + GAP + (fw - used) / 2;
        Some((x0, bw))
    }
}

impl Family for Brutal {
    fn id(&self) -> &'static str {
        "brutal"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());
        let panel = Rgba::from_hex(&t.panel, t.panel_alpha);
        c.rounded_rect(1, 2, w - 2, h - 4, 3, panel);
        if w < MIN_W || h < MIN_H {
            return; // shed rather than smudge
        }
        let Some((x0, bw)) = Self::grid(w) else {
            return;
        };
        let (fy, fh) = (3, h - 6);
        if fh < 6 {
            return;
        }
        let dt = if d.dt_ms.is_finite() { d.dt_ms.clamp(0.0, 250.0) } else { 16.7 };

        // ---- the flip ----
        if self.onset.update(&d.levels, dt, FLIP_RATIO, FLIP_REFRACTORY_MS) {
            self.hanging = !self.hanging;
        }

        // ---- the flourish ----
        let fired = self.flourish.update(&d.levels, dt, t.flourish);
        let slab = self.slab.update(fired, dt, SLAB_MS).clamp(0.0, 1.0);
        let slab = if slab.is_finite() { slab } else { 0.0 };

        // Figure and ground cross over continuously - see SLAB_MS.
        let dark = Rgba::from_hex(&t.panel, 1.0);
        if slab > 0.0 {
            c.fill_rect(2, fy, w - 4, fh, Rgba::from_hex(&t.lit, slab));
        }
        let bands = d.levels.len().max(1);

        for i in 0..BLOCKS {
            let bx = x0 + i as i32 * (bw + GAP);
            // This block's slice of the spectrum, taken as its loudest band so a wide slice is not
            // averaged into nothing.
            let lo = i * bands / BLOCKS;
            let hi = (((i + 1) * bands) / BLOCKS).max(lo + 1).min(bands);
            let mut slice = 0.0f32;
            let mut peak = 0.0f32;
            for k in lo..hi {
                let v = d.levels[k];
                if v.is_finite() {
                    slice = slice.max(v);
                }
                let p = d.peaks[k];
                if p.is_finite() {
                    peak = peak.max(p);
                }
            }
            let norm = |v: f32| ((v - LEVEL_FLOOR) / LEVEL_SPAN).clamp(0.0, 1.0).powf(LEVEL_GAMMA);
            let usable = (fh - STUB_PX).max(1) as f32;
            let mut len = STUB_PX + (norm(slice) * usable).round() as i32;
            let cap_at = STUB_PX + (norm(peak) * usable).round() as i32;
            // The monolith takes every block to full height.
            len = len + (((fh - len) as f32) * slab).round() as i32;
            let len = len.clamp(1, fh);

            // Resolved PER BLOCK through the shared rainbow resolver, keyed on the block's position
            // across the panel. On a fixed colourway `tint` returns the hex unchanged, so those
            // colourways are bit-for-bit what they were; on a rainbow one every block gets its own hue,
            // which is what makes a bold primary-colour set possible in a family built on flat fills.
            //
            // Position rather than a per-block random, because the blocks are a fixed grid: a stable hue
            // per column reads as painted concrete, where a hue that moved would read as a light show
            // and this family is explicitly not that.
            let x01 = if BLOCKS > 1 { i as f32 / (BLOCKS - 1) as f32 } else { 0.5 };
            let body = lerp(crate::render::tint(t, x01, d.time_s, false, &t.lit, 1.0), dark, slab);
            let tip = lerp(crate::render::tint(t, x01, d.time_s, true, &t.hot, 1.0), dark, slab);
            // The two states. `hanging` grows downward from the ceiling, otherwise upward from the floor.
            let (by, cap_y) = if self.hanging {
                (fy, fy + cap_at.clamp(0, fh) - CAP_PX)
            } else {
                (fy + fh - len, fy + fh - cap_at.clamp(0, fh))
            };
            c.fill_rect(bx, by, bw, len, body);
            // The peak cap, anchored to THIS BLOCK'S BASE rather than to a panel row - see the module
            // note. Only drawn when the peak is genuinely ahead of the block, or it just thickens the tip.
            if cap_at > len + CAP_PX {
                c.fill_rect(bx, cap_y.clamp(fy, fy + fh - CAP_PX), bw, CAP_PX, tip);
            }
        }

        // No bloom. Every colourway here sets `bloom` to 0 and this family would ignore it anyway - see
        // the module note. The clip is kept because the grid is centred by integer division and any future
        // change to that arithmetic is one slip from painting on the rounded corners.
        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::themes::builtin;

    fn frame(gain: f32, t_s: f32) -> FrameData {
        let mut d = FrameData { dt_ms: 16.7, time_s: t_s, ..FrameData::default() };
        for (i, v) in d.levels.iter_mut().enumerate() {
            let f = i as f32 / crate::dsp::bands::NUM_BANDS as f32;
            let shape = (1.0 - f).powf(1.5) * 0.58 + 0.15;
            let wob = 1.0 + 0.32 * ((t_s * 2.2 + f * 7.0).sin());
            *v = ((shape * wob) * gain).clamp(0.0, 1.0);
        }
        d.peaks = d.levels;
        d
    }

    /// A frame with a hard transient on the given step, so the flip detector has something to find.
    fn beat_frame(t_s: f32, period: usize, k: usize, gain: f32) -> FrameData {
        let mut d = FrameData { dt_ms: 16.7, time_s: t_s, ..FrameData::default() };
        let hit = k % period == 0;
        for (i, v) in d.levels.iter_mut().enumerate() {
            let f = i as f32 / crate::dsp::bands::NUM_BANDS as f32;
            let shape = (1.0 - f).powf(1.4) * 0.5 + 0.14;
            let punch = if hit { 0.45 } else { 0.0 };
            *v = ((shape + punch) * gain).clamp(0.0, 1.0);
        }
        d.peaks = d.levels;
        d
    }

    /// How much ink sits in the top third of the panel against the bottom third.
    fn top_vs_bottom(c: &Canvas, t: &Theme) -> (i32, i32) {
        let dark = Rgba::from_hex(&t.panel, 1.0);
        let h = c.height();
        let (mut top, mut bot) = (0, 0);
        for y in 3..h - 3 {
            for x in 2..c.width() - 2 {
                let p = c.get(x, y);
                if p.a == 0 || (p.r, p.g, p.b) == (dark.r, dark.g, dark.b) {
                    continue;
                }
                if y < 3 + (h - 6) / 3 {
                    top += 1;
                } else if y >= 3 + 2 * (h - 6) / 3 {
                    bot += 1;
                }
            }
        }
        (top, bot)
    }

    /// THE load-bearing property: a beat flips the whole panel between hanging from the ceiling and
    /// standing on the floor. This is the family's identity and its strobe.
    ///
    /// Mutation: remove the `self.hanging = !self.hanging` toggle, or make the draw ignore it - the two
    /// measured layouts become identical and this fails.
    #[test]
    fn a_beat_flips_the_blocks_between_floor_and_ceiling() {
        let t = builtin::brutal_concrete();
        let mut fam = Brutal::default();
        let mut c = Canvas::new(380, 60);
        // Settle, then find one frame in each state and compare where the ink is.
        let mut standing: Option<(i32, i32)> = None;
        let mut hanging: Option<(i32, i32)> = None;
        for k in 0..400 {
            fam.draw(&mut c, &t, &beat_frame(k as f32 * 0.0167, 24, k, 0.75));
            if k < 40 {
                continue;
            }
            let m = top_vs_bottom(&c, &t);
            if fam.hanging {
                hanging = Some(m);
            } else {
                standing = Some(m);
            }
        }
        let (h_top, h_bot) = hanging.expect("the panel never entered the hanging state");
        let (s_top, s_bot) = standing.expect("the panel never entered the standing state");
        assert!(h_top + h_bot > 300 && s_top + s_bot > 300, "almost nothing was drawn");
        assert!(
            h_top > h_bot,
            "the hanging state is not top-heavy: {h_top} top, {h_bot} bottom"
        );
        assert!(
            s_bot > s_top,
            "the standing state is not bottom-heavy: {s_top} top, {s_bot} bottom"
        );
    }

    /// Block LENGTH tracks the level, in both states - which is what makes the flip free. The tips move
    /// the height of the panel on a flip, so length is the only thing that can carry the reading.
    ///
    /// Mutation: make `len` a constant, or drop the `norm(slice)` term.
    #[test]
    fn block_length_tracks_the_level_in_both_states() {
        let t = builtin::brutal_concrete();
        let ink = |gain: f32, force_hanging: bool| -> i32 {
            let mut fam = Brutal::default();
            let mut c = Canvas::new(380, 60);
            for k in 0..40 {
                fam.draw(&mut c, &t, &frame(gain, k as f32 * 0.0167));
            }
            fam.hanging = force_hanging;
            fam.draw(&mut c, &t, &frame(gain, 1.0));
            let (a, b) = top_vs_bottom(&c, &t);
            a + b
        };
        for hanging in [false, true] {
            let quiet = ink(0.12, hanging);
            let loud = ink(0.95, hanging);
            assert!(
                loud > quiet * 2,
                "hanging={hanging}: length did not follow level, {quiet} -> {loud} px of ink"
            );
        }
    }

    /// The flourish INVERTS figure and ground, which is the change of kind that makes it read on a panel
    /// that is already slamming several times a second.
    ///
    /// Mutation: drop the background wash, or the `lerp(lit, dark, slab)` on the body - the panel then
    /// merely gets taller and this fails.
    #[test]
    fn the_flourish_inverts_the_panel() {
        let t = builtin::brutal_concrete();
        let mut fam = Brutal::default();
        let mut c = Canvas::new(380, 60);
        for k in 0..80 {
            fam.draw(&mut c, &t, &beat_frame(k as f32 * 0.0167, 24, k, 0.7));
        }
        // Before: the gaps are dark and the blocks are lit.
        let dark = Rgba::from_hex(&t.panel, 1.0);
        let count_dark = |c: &Canvas| -> i32 {
            let mut n = 0;
            for y in 4..56 {
                for x in 4..376 {
                    let p = c.get(x, y);
                    if (p.r, p.g, p.b) == (dark.r, dark.g, dark.b) {
                        n += 1;
                    }
                }
            }
            n
        };
        let before = count_dark(&c);
        fam.flourish.force_next();
        let mut most_dark = before;
        for k in 80..110 {
            fam.draw(&mut c, &t, &beat_frame(k as f32 * 0.0167, 24, k, 0.7));
            most_dark = most_dark.max(count_dark(&c));
        }
        // At the peak the panel is a lit field with the blocks as dark voids, so the count of
        // panel-coloured pixels must have moved a long way - in EITHER direction, since which of figure
        // and ground dominates depends on the level. What must not happen is nothing.
        assert!(
            (most_dark - before).abs() > 500,
            "the flourish did not invert anything: {before} dark px before, {most_dark} at most"
        );
        for k in 110..300 {
            fam.draw(&mut c, &t, &beat_frame(k as f32 * 0.0167, 24, k, 0.7));
        }
        assert!(fam.slab.level() < 0.05, "the monolith never let go: {:.3}", fam.slab.level());
    }

    /// The peak cap is anchored to the BLOCK'S OWN BASE, not to a panel row. Anchored to a row it would
    /// appear to leap the panel's height on every flip, which is the specific fault the module note warns
    /// about - so this asserts the cap stays a bounded distance from its block in both states.
    ///
    /// Mutation: compute `cap_y` as `fy + fh - cap_at` in both branches and the hanging case fails.
    #[test]
    fn the_peak_cap_follows_its_own_block_through_a_flip() {
        let t = builtin::brutal_concrete();
        let (x0, bw) = Brutal::grid(380).unwrap();
        let hot = Rgba::from_hex(&t.hot, 1.0);
        // A frame whose peaks sit well above the current level, so a cap is actually drawn.
        let mut d = frame(0.35, 1.0);
        for p in d.peaks.iter_mut() {
            *p = 0.95;
        }
        for hanging in [false, true] {
            let mut fam = Brutal::default();
            let mut c = Canvas::new(380, 60);
            for k in 0..20 {
                fam.draw(&mut c, &t, &frame(0.35, k as f32 * 0.0167));
            }
            fam.hanging = hanging;
            fam.draw(&mut c, &t, &d);
            // Find the cap in the first block's columns.
            let mut cap_rows: Vec<i32> = Vec::new();
            for y in 3..57 {
                let p = c.get(x0 + bw / 2, y);
                if (p.r, p.g, p.b) == (hot.r, hot.g, hot.b) {
                    cap_rows.push(y);
                }
            }
            assert!(!cap_rows.is_empty(), "hanging={hanging}: no peak cap was drawn");
            let cap = cap_rows[0];
            // In the standing state the cap is above the block's top, in the hanging state below its
            // bottom - in both, it is on the block's FAR side from its own base.
            if hanging {
                assert!(cap > 3 + 10, "hanging: the cap sat at row {cap}, too near the ceiling");
            } else {
                assert!(cap < 57 - 10, "standing: the cap sat at row {cap}, too near the floor");
            }
        }
    }

    /// Small panels shed, a hostile frame cannot poison anything, and the grid never paints outside the
    /// panel.
    #[test]
    fn tiny_panels_shed_and_a_hostile_frame_is_survivable() {
        let t = builtin::brutal_concrete();
        for (w, h) in [(1, 1), (8, 8), (59, 17), (60, 10), (12, 60), (0, 0), (61, 19)] {
            let mut fam = Brutal::default();
            let mut c = Canvas::new(w, h);
            fam.flourish.force_next();
            fam.draw(&mut c, &t, &frame(0.6, 0.1));
            fam.draw(&mut c, &t, &frame(0.6, 0.2));
        }
        let mut fam = Brutal::default();
        let mut c = Canvas::new(380, 60);
        for k in 0..20 {
            fam.draw(&mut c, &t, &frame(0.6, k as f32 * 0.0167));
        }
        for bad in [f32::NAN, f32::INFINITY, -1.0e30, 1.0e30] {
            let mut d = frame(0.6, 1.0);
            d.dt_ms = bad;
            d.levels[0] = bad;
            d.peaks[1] = f32::NAN;
            fam.draw(&mut c, &t, &d);
        }
        fam.draw(&mut c, &t, &frame(0.6, 2.0));
        // Nothing outside the rounded panel.
        for x in 0..380 {
            assert_eq!(c.get(x, 0).a, 0, "painted on row 0 at x {x}");
            assert_eq!(c.get(x, 59).a, 0, "painted on the last row at x {x}");
        }
    }

    /// Every colourway draws on both panel widths, and none of them enables bloom - a halo would soften
    /// the edges this family is built on.
    #[test]
    fn every_colourway_draws_hard_edged_on_both_widths() {
        for t in builtin::all().into_iter().filter(|t| t.family == "brutal") {
            assert_eq!(t.bloom, 0.0, "{}: bloom must be 0 in this family", t.id);
            for w in [380, 190] {
                let mut fam = Brutal::default();
                let mut c = Canvas::new(w, 60);
                for k in 0..30 {
                    fam.draw(&mut c, &t, &frame(0.62, k as f32 * 0.0167));
                }
                let (a, b) = top_vs_bottom(&c, &t);
                assert!(a + b > w / 2, "{} drew almost nothing at {w}px: {}", t.id, a + b);
            }
        }
    }

    #[test]
    #[ignore]
    fn probe_brutal_cost() {
        let t = builtin::brutal_concrete();
        let mut fam = Brutal::default();
        let mut c = Canvas::new(380, 60);
        for k in 0..60 {
            fam.draw(&mut c, &t, &beat_frame(k as f32 * 0.0167, 24, k, 0.8));
        }
        let n = 300;
        let t0 = std::time::Instant::now();
        for k in 0..n {
            fam.draw(&mut c, &t, &beat_frame(k as f32 * 0.0167, 24, k, 0.8));
        }
        println!("brutal: {:.3} ms/frame at 380x60", t0.elapsed().as_secs_f64() * 1000.0 / n as f64);
    }

    #[test]
    #[ignore]
    fn dump_brutal() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();
        let write = |name: String, c: &Canvas| {
            let mut out = Vec::new();
            for y in 0..c.height() {
                for x in 0..c.width() {
                    let px = c.get(x, y);
                    out.extend_from_slice(&[px.r, px.g, px.b, px.a]);
                }
            }
            std::fs::write(dir.join(format!("{name}.rgba")), &out).unwrap();
        };
        for t in builtin::all().into_iter().filter(|t| t.family == "brutal") {
            let mut fam = Brutal::default();
            let mut c = Canvas::new(380, 60);
            for k in 0..300 {
                fam.draw(&mut c, &t, &frame(0.62, k as f32 * 0.0167));
            }
            write(format!("brutal-{}", t.id), &c);
        }
        // Both states of the flip, side by side, which is the thing to judge.
        let t = builtin::brutal_concrete();
        for hanging in [false, true] {
            let mut fam = Brutal::default();
            let mut c = Canvas::new(380, 60);
            for k in 0..120 {
                fam.draw(&mut c, &t, &frame(0.62, k as f32 * 0.0167));
            }
            fam.hanging = hanging;
            fam.draw(&mut c, &t, &frame(0.62, 2.0));
            write(format!("brutal-{}", if hanging { "hanging" } else { "standing" }), &c);
        }
        // The monolith, mid-hold.
        let mut fam = Brutal::default();
        let mut c = Canvas::new(380, 60);
        for k in 0..120 {
            fam.draw(&mut c, &t, &frame(0.62, k as f32 * 0.0167));
        }
        fam.flourish.force_next();
        for k in 120..132 {
            fam.draw(&mut c, &t, &frame(0.62, k as f32 * 0.0167));
        }
        write("brutal-monolith".into(), &c);
    }
}
