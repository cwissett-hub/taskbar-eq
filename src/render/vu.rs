use super::canvas::{Canvas, Rgba};
use super::{Family, FrameData};
use crate::themes::Theme;

/// ~300ms integration at 60fps. Slow on purpose: a fast needle looks broken.
pub const VU_SMOOTHING: f32 = 0.085;
const OVERLOAD_AT: f32 = 0.76;

/// Bottom of the needle's scale, in dBFS.
///
/// A VU meter is a dB instrument, and the previous code fed it LINEAR rms. Typical music
/// at an rms of 0.02-0.12 is -32 to -18 dBFS, which mapped linearly put the needle at
/// 2-12% of travel - the meter looked broken. Mapping the same range across [-45, 0] dB
/// puts normal listening around half to two-thirds of the arc, which is where a real VU
/// sits and where its swing is legible.
const VU_DB_FLOOR: f32 = -45.0;

/// Maps a linear RMS to needle travel 0..=1 through dB, scaled by the theme's
/// sensitivity. Silence maps to exactly 0 rather than -inf.
fn needle_level(rms: f32, sensitivity: f32) -> f32 {
    if !rms.is_finite() || rms <= 0.0 {
        return 0.0;
    }
    let db = 20.0 * rms.log10();
    let norm = (db - VU_DB_FLOOR) / -VU_DB_FLOOR;
    (norm * sensitivity.max(0.0)).clamp(0.0, 1.0)
}

/// Width one dial wants, in pixels.
///
/// 95 is half of the 190px reference panel, i.e. exactly the dial size that was tuned and
/// approved - so the narrow case keeps its two dials and wider panels add dials rather than
/// inflating them.
///
/// The narrow case is NOT quite pixel-identical: the new height cap trims the radius from 55 to
/// 52 at 190x60. That is a fix rather than a regression - at 55 the arc's apex landed on row 1,
/// one pixel ABOVE the panel's top edge at row 2, a containment bug this file already documented
/// and worked around by clipping afterwards. The dial is now inside the panel by construction.
const DIAL_PITCH: i32 = 95;

/// Dials to draw at a given panel width.
///
/// A dial cannot simply be scaled up: its arc apex sits at `cy - radius` with `cy` near the
/// bottom of a 60px panel, so a radius derived from width alone leaves the canvas entirely.
/// Measured at 380px wide: `dial_w * 0.60` gave a radius of 112 on a 60px panel, putting the
/// apex 56px ABOVE the top edge - the arc, ticks and scale all vanished and only two bare
/// needle lines were left. Height caps the radius, so extra width has to buy extra dials.
pub fn dial_count(w: i32) -> usize {
    ((w / DIAL_PITCH).max(2) as usize).min(8)
}

/// Silkscreen label for dial `idx` of `dials`.
///
/// Exists because unlabelled dials are unreadable: with four of them there is nothing on the
/// panel to say that two are channels and two are frequency bands, so the extra pair just looks
/// like more of the same. Real meters are silkscreened for exactly this reason.
///
/// Dials 0 and 1 are always the stereo pair. The rest split the spectrum low-to-high, so they
/// get frequency labels while there are few enough to name, and numbers past that - "MID" stops
/// meaning anything once there are five bands.
pub fn dial_label(idx: usize, dials: usize) -> &'static str {
    match idx {
        0 => "L",
        1 => "R",
        _ => {
            let band = idx - 2;
            match (dials - 2, band) {
                (1, _) => "BND",
                (2, 0) => "LO",
                (2, _) => "HI",
                (3, 0) => "LO",
                (3, 1) => "MID",
                (3, _) => "HI",
                (_, 0) => "1",
                (_, 1) => "2",
                (_, 2) => "3",
                (_, 3) => "4",
                (_, 4) => "5",
                (_, _) => "6",
            }
        }
    }
}

#[derive(Default)]
pub struct Vu {
    l: f32,
    r: f32,
    pk_l: f32,
    pk_r: f32,
    /// Smoothed (level, peak) for the frequency-band dials that exist only on a wide panel.
    ///
    /// Kept separate from `l`/`r` rather than folded into one vector so the two-dial case stays
    /// exactly what it was: dials 0 and 1 are always left and right channel RMS, which is the
    /// stereo reading the narrow layout was approved on. Anything beyond them is a band, the
    /// way a broadcast console carries a stereo pair plus band meters.
    bands: Vec<(f32, f32)>,
}

impl Family for Vu {
    fn id(&self) -> &'static str {
        "vu"
    }
    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());

        // Ballistics live on the family, not the canvas, so a resize does not
        // reset the needles.
        //
        // f32::clamp does not sanitise NaN (NaN < min and NaN > max are both
        // false, so clamp returns NaN unchanged), so a single non-finite
        // rms_l/rms_r sample would poison self.l/self.r with NaN forever -
        // every later frame computes `(NaN - self.l) * k` which is still
        // NaN, and cos/sin of a NaN angle then casts to 0 when drawn,
        // collapsing the needle to a dot at the pivot until process restart.
        // Guard exactly like `dsp::ballistics::Smoother::update` already
        // does for the same pattern.
        let l_in = needle_level(d.rms_l, t.sensitivity);
        let r_in = needle_level(d.rms_r, t.sensitivity);
        self.l += (l_in - self.l) * VU_SMOOTHING;
        self.r += (r_in - self.r) * VU_SMOOTHING;
        self.pk_l = (self.pk_l - 0.004).max(self.l);
        self.pk_r = (self.pk_r - 0.004).max(self.r);

        // Band dials, on wide panels only. Same ballistics as the stereo pair, so they settle
        // together instead of one bank visibly leading the other.
        let dials = dial_count(w);
        let band_dials = dials.saturating_sub(2);
        if self.bands.len() != band_dials {
            self.bands.resize(band_dials, (0.0, 0.0));
        }
        for i in 0..band_dials {
            let lo = i * d.levels.len() / band_dials.max(1);
            let hi = (((i + 1) * d.levels.len()) / band_dials.max(1)).max(lo + 1).min(d.levels.len());
            let mut acc = 0.0;
            let mut n = 0.0;
            for v in &d.levels[lo..hi] {
                if v.is_finite() {
                    acc += *v;
                    n += 1.0;
                }
            }
            let target = if n > 0.0 { (acc / n).clamp(0.0, 1.0) } else { 0.0 };
            let (lv, pk) = self.bands[i];
            let lv = lv + (target - lv) * VU_SMOOTHING;
            self.bands[i] = (lv, (pk - 0.004).max(lv));
        }

        c.clear();
        c.rounded_rect(1, 2, w - 2, h - 4, 4, Rgba::from_hex(&t.panel, t.panel_alpha));

        // Warm backlight pooling from the bottom.
        //
        // 0.22 washed the panel to a muddy mid-tone across the whole lower two thirds -
        // which is exactly where the needle sits - so the needle had almost nothing to
        // contrast against and the dial read as flat. The glow is meant to suggest a lamp
        // behind the dial, not to be the brightest thing on it.
        for y in (h / 3)..(h - 4) {
            let f = (y - h / 3) as f32 / (h - 4 - h / 3).max(1) as f32;
            c.fill_rect(1, y, w - 2, 1, Rgba::from_hex(&t.lit, 0.09 * f));
        }

        let ink = Rgba::from_hex(&t.lit, 0.72);
        let over = Rgba::from_hex(t.overload_hex(), 0.85);
        let dial_w = (w / dials as i32 - 3).max(8);
        let readings: Vec<(f32, f32)> = [(self.l, self.pk_l), (self.r, self.pk_r)]
            .into_iter()
            .chain(self.bands.iter().copied())
            .take(dials)
            .collect();

        // Arcs, ticks and needles are drawn onto their own transparent layer,
        // not straight onto `c`. `Canvas::bloom` composites its halo UNDER
        // whatever is already on the canvas, and by this point `c` is the
        // fully opaque panel (panel_alpha 1.0 on every dial theme) - blooming
        // `c` directly, as the brief's own code did, means every dial pixel
        // already has dst.a == 255, so `blend_over`'s `sa == 255` fast path
        // (or the near-255 accumulated alpha for partial-alpha ink) leaves
        // the halo invisible everywhere inside the panel. Building the ink on
        // its own layer and `draw_over`-ing the bloomed result onto the
        // opaque panel is the same fix already applied to `segmented.rs` and
        // `scope.rs` for exactly this reason.
        let mut dial = Canvas::new(w, h);

        for (idx, (level, peak)) in readings.iter().enumerate() {
            let cx = 2 + idx as i32 * (w / dials as i32) + dial_w / 2;
            let cy = h - 4;
            // Height caps the radius. Without this the arc's apex leaves the panel entirely on
            // a wide display - see `dial_count`. The -8 keeps the apex a few pixels below the
            // top edge so the bezel and the bloom halo have somewhere to go.
            let radius = ((dial_w as f32 * 0.60) as i32).min(h - 8).max(4);
            let (a0, a1) = (-std::f32::consts::PI * 0.78, -std::f32::consts::PI * 0.22);

            // Printed arc, with the overload segment in its own colour.
            //
            // Joined with `line` rather than plotted as independent points. Stepping the
            // angle and setting one pixel per step leaves 8-connected diagonal runs, which
            // at 1px read as a dashed arc - visible in the render as a dotted scale that
            // looked like noise rather than print.
            let arc_pt = |f: f32| {
                let ang = a0 + (a1 - a0) * f;
                (
                    cx + (ang.cos() * radius as f32) as i32,
                    cy + (ang.sin() * radius as f32) as i32,
                )
            };
            let steps = 60;
            for step in 1..=steps {
                let f = step as f32 / steps as f32;
                let (x0, y0) = arc_pt((step - 1) as f32 / steps as f32);
                let (x1, y1) = arc_pt(f);
                let col = if f >= OVERLOAD_AT { over } else { ink };
                dial.line(x0, y0, x1, y1, col);
            }

            // Tick marks, longer at the ends and centre.
            for k in 0..=6 {
                let ang = a0 + (a1 - a0) * k as f32 / 6.0;
                let big = k == 0 || k == 3 || k == 6;
                let inner = radius - if big { 5 } else { 3 };
                dial.line(
                    cx + (ang.cos() * inner as f32) as i32,
                    cy + (ang.sin() * inner as f32) as i32,
                    cx + (ang.cos() * radius as f32) as i32,
                    cy + (ang.sin() * radius as f32) as i32,
                    ink,
                );
            }

            // Ghost peak needle. Deliberately faint: it is a reference mark, and at 0.32
            // it competed with the live needle it exists to annotate.
            let pang = a0 + (a1 - a0) * peak.clamp(0.0, 1.0);
            dial.line(
                cx + (pang.cos() * (radius / 3) as f32) as i32,
                cy + (pang.sin() * (radius / 3) as f32) as i32,
                cx + (pang.cos() * radius as f32) as i32,
                cy + (pang.sin() * radius as f32) as i32,
                Rgba::from_hex("#ffffff", 0.22),
            );

            // Live needle, red past the overload point.
            let ang = a0 + (a1 - a0) * level.clamp(0.0, 1.0);
            let needle = if *level > OVERLOAD_AT {
                Rgba::from_hex(t.overload_hex(), 1.0)
            } else {
                Rgba::from_hex(&t.hot, 1.0)
            };
            // The needle is the one thing on the dial that must read instantly, and as a
            // 1px hairline with a 2x2 pivot it read as a scratch. Now: a full-length core,
            // a second offset line thickening the lower two thirds, and a round hub - so
            // it tapers from a broad base to a fine tip the way a real pointer does.
            let (dx, dy) = (ang.cos(), ang.sin());
            let tip = radius as f32 * 0.95;
            let (tx, ty) = (cx + (dx * tip) as i32, cy + (dy * tip) as i32);
            dial.line(cx, cy, tx, ty, needle);

            // Perpendicular offset, stopping short of the tip so it stays pointed.
            let (ox, oy) = ((-dy).round() as i32, dx.round() as i32);
            if ox != 0 || oy != 0 {
                let broad = tip * 0.66;
                dial.line(
                    cx + ox,
                    cy + oy,
                    cx + ox + (dx * broad) as i32,
                    cy + oy + (dy * broad) as i32,
                    needle,
                );
            }

            // Bright hot tip, so the eye lands on where the needle is pointing.
            dial.line(
                cx + (dx * tip * 0.82) as i32,
                cy + (dy * tip * 0.82) as i32,
                tx,
                ty,
                Rgba::from_hex(&t.hot, 1.0),
            );

            // Pivot hub.
            dial.fill_circle(cx, cy, 2, needle);

            // Silkscreen label, placed under the arc's left-hand end. Off-centre on purpose:
            // the needle sweeps through the middle of the dial, so a centred label would be
            // struck through by it at normal listening levels.
            let label = dial_label(idx, dials);
            let lw = Canvas::text_3x5_width(label);
            let ly = cy - (radius * 3 / 5);
            let lx = cx - (radius as f32 * 0.52) as i32;
            if ly + 5 < cy && lx >= 2 && lx + lw < w - 2 {
                dial.text_3x5(lx, ly, label, Rgba::from_hex(&t.lit, 0.60));
            }
        }

        // `Canvas::bloom` keeps the original (crisp) pixel on top of the
        // blurred halo it composites underneath, so blooming `dial` in place
        // preserves the sharp arc/needle lines while adding the glow -
        // `draw_over` then blends that combined result onto the opaque panel.
        dial.bloom(t.bloom as i32, 0.7);
        c.draw_over(&dial);

        // The arc's own radius reaches y = cy - radius at its apex (angle
        // -90 deg), which on a 190x60 canvas is 1px above the panel's top
        // edge (row 2) - a real containment bug independent of bloom. Bloom
        // then spreads that leak (and any other near-edge ink) further past
        // the panel's rounded corners. Clip back to the same rect
        // `rounded_rect` drew above, exactly as `segmented.rs`/`scope.rs` do
        // after their own bloom step.
        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 4);

        let e = Rgba::from_hex(&t.edge, t.edge_alpha);
        c.fill_rect(1, 2, w - 2, 1, e);
        c.fill_rect(1, h - 3, w - 2, 1, e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::golden::canvas_to_ascii;
    use crate::themes::builtin;

    fn level(l: f32, r: f32) -> FrameData {
        FrameData { rms_l: l, rms_r: r, ..FrameData::default() }
    }

    /// Every dial theme ships `panel_alpha: 1.0`, so `Canvas::blend_over` drives
    /// `.a` to 255 at every in-bounds pixel the instant the opaque panel is
    /// painted - regardless of whether any arc/needle ink was drawn on top.
    /// The brief's own version of this test summed `.a`, which is therefore
    /// vacuous: it would pass identically even with the dial-drawing loop
    /// deleted entirely. Luminance is what actually distinguishes lit ink
    /// from bare (panel + backlight) canvas - the same fix already applied to
    /// `segmented.rs` and `scope.rs`'s own alpha-vs-luminance traps.
    fn lum(p: Rgba) -> f32 {
        0.2126 * p.r as f32 + 0.7152 * p.g as f32 + 0.0722 * p.b as f32
    }

    #[test]
    fn draws_two_dials_with_printed_arcs() {
        let mut c = Canvas::new(190, 60);
        Vu::default().draw(&mut c, &builtin::vu_cream(), &level(0.0, 0.0));
        // Both halves must contain ink clearly brighter than the bare
        // panel-plus-backlight gradient at this row (measured ~28 for
        // vu_cream; arc ink measures ~170).
        let left = (5..90).map(|x| lum(c.get(x, 30))).fold(0.0f32, f32::max);
        let right = (100..185).map(|x| lum(c.get(x, 30))).fold(0.0f32, f32::max);
        assert!(left > 80.0 && right > 80.0, "expected a dial in each half (left {left}, right {right})");
    }

    #[test]
    fn dial_count_holds_at_two_when_narrow_and_grows_with_width() {
        assert_eq!(dial_count(190), 2, "the reference panel must keep its stereo pair");
        assert_eq!(dial_count(150), 2, "and so must anything narrower");
        assert_eq!(dial_count(380), 4, "double the width buys two more dials");
        assert_eq!(dial_count(456), 4);
        assert!(dial_count(4000) <= 8, "capped, or a huge panel draws a useless smear of dials");
    }

    #[test]
    fn the_dial_arc_stays_inside_the_panel_at_every_width() {
        // The bug this guards was visible only once the display was widened: the radius came
        // from dial width alone, so at 380px it reached 112 on a 60px-tall panel and the arc,
        // the ticks and the printed scale all left the canvas, leaving two bare needle lines.
        //
        // Checks rendered pixels rather than the radius arithmetic, because the arithmetic is
        // what was wrong - asserting on it would just restate the bug.
        for w in [150, 190, 240, 380, 456, 600] {
            let mut v = Vu::default();
            let mut c = Canvas::new(w, 60);
            for _ in 0..90 {
                v.draw(&mut c, &builtin::vu_cream(), &level(0.5, 0.5));
            }
            // Rows 0..2 are outside the panel, which starts at y=2.
            for y in 0..2 {
                for x in 0..w {
                    assert_eq!(
                        c.get(x, y).a, 0,
                        "at width {w} the dial painted outside the panel at ({x},{y})"
                    );
                }
            }
            // And something must actually be drawn near the top of the panel, or the arc has
            // silently collapsed to nothing and the check above would pass vacuously.
            let ink_high = (2..14).any(|y| (0..w).any(|x| c.get(x, y).a > 40));
            assert!(ink_high, "at width {w} nothing was drawn in the upper panel - arc missing?");
        }
    }

    #[test]
    fn a_wide_panel_shows_band_dials_that_respond_to_the_spectrum() {
        // The extra dials must be driven by something. A wide panel whose band dials sat at zero
        // would look like broken hardware next to two live stereo dials.
        let mut v = Vu::default();
        let mut c = Canvas::new(380, 60);
        let mut d = level(0.09, 0.055);
        for (i, x) in d.levels.iter_mut().enumerate() {
            *x = if i < 32 { 0.8 } else { 0.1 };
        }
        for _ in 0..90 {
            v.draw(&mut c, &builtin::vu_cream(), &d);
        }
        assert_eq!(v.bands.len(), 2, "380px should add two band dials");
        assert!(
            v.bands[0].0 > v.bands[1].0 + 0.15,
            "the low-band dial must read higher than the high-band one: {:?}",
            v.bands
        );
    }

    #[test]
    fn needles_move_slowly_toward_the_target() {
        let mut v = Vu::default();
        let mut c = Canvas::new(190, 60);
        v.draw(&mut c, &builtin::vu_cream(), &level(1.0, 1.0));
        assert!(v.l < 0.2, "one frame must not swing the needle far, got {}", v.l);
        for _ in 0..80 {
            v.draw(&mut c, &builtin::vu_cream(), &level(1.0, 1.0));
        }
        assert!(v.l > 0.9, "should converge after ~1.3s, got {}", v.l);
    }

    #[test]
    fn the_two_channels_are_independent() {
        let mut v = Vu::default();
        let mut c = Canvas::new(190, 60);
        for _ in 0..80 {
            v.draw(&mut c, &builtin::vu_cream(), &level(0.9, 0.1));
        }
        // dB mapping deliberately COMPRESSES a wide linear range: rms 0.9 and 0.1 are
        // -0.9 and -20 dBFS, which land at ~0.98 and ~0.56 of travel. That compression
        // is the point of a dB scale - it is why quiet music is now visible at all - so
        // the channels must clearly differ without the old linear gap of 0.5.
        assert!(
            v.l > v.r + 0.2,
            "channels must read clearly differently: L {} vs R {}",
            v.l,
            v.r
        );
        assert!(v.l > 0.9, "a near-full-scale channel should be near the top, got {}", v.l);
        assert!(v.r < 0.7, "a quiet channel should sit mid-arc, got {}", v.r);
    }

    #[test]
    fn peak_needles_lag_behind_on_the_way_down() {
        let mut v = Vu::default();
        let mut c = Canvas::new(190, 60);
        for _ in 0..80 {
            v.draw(&mut c, &builtin::vu_cream(), &level(0.9, 0.9));
        }
        for _ in 0..10 {
            v.draw(&mut c, &builtin::vu_cream(), &level(0.0, 0.0));
        }
        assert!(v.pk_l > v.l, "peak needle must hold above the live needle");
    }

    #[test]
    fn the_red_dial_flips_its_overload_arc_to_white() {
        // Red-on-red would be invisible - the one colourway needing a behavioural change.
        assert_eq!(builtin::vu_red().overload_hex(), "#ffffff");
        assert_ne!(builtin::vu_cream().overload_hex(), "#ffffff");
    }

    #[test]
    fn state_survives_a_canvas_resize() {
        let mut v = Vu::default();
        let mut a = Canvas::new(190, 60);
        for _ in 0..80 {
            v.draw(&mut a, &builtin::vu_cream(), &level(0.8, 0.8));
        }
        let before = v.l;
        let mut b = Canvas::new(120, 60);
        v.draw(&mut b, &builtin::vu_cream(), &level(0.8, 0.8));
        assert!((v.l - before).abs() < 0.05, "needle position must not jump on resize");
    }

    #[test]
    fn bloom_halo_does_not_leak_outside_the_panel_bezel() {
        // The dial arc's own radius reaches its apex at y = cy - radius,
        // which on a 190x60 canvas lands at row 1 - one row above the
        // panel's top edge (row 2, see `rounded_rect(1, 2, w-2, h-4, 4, ...)`
        // in `draw`) - a real containment bug independent of bloom. Rows 0,
        // 1, 58 and 59 all sit outside the panel and must stay fully
        // transparent even at a driving level that pushes the needle toward
        // vertical (the steepest, highest-reaching angle).
        let mut v = Vu::default();
        let mut c = Canvas::new(190, 60);
        for _ in 0..80 {
            v.draw(&mut c, &builtin::vu_red(), &level(1.0, 1.0));
        }
        for y in [0, 1, 58, 59] {
            for x in 0..190 {
                let p = c.get(x, y);
                assert_eq!(p.a, 0, "row {y} col {x} must stay blank outside the panel bezel, got {p:?}");
            }
        }
    }

    #[test]
    fn a_single_nan_sample_does_not_poison_the_level_forever() {
        // Mirrors `dsp::ballistics::Smoother`'s own regression test for the
        // identical one-pole-filter-on-live-audio pattern: f32::clamp does
        // not sanitise NaN, so an unguarded `self.l += (target - self.l) * k`
        // would leave `self.l`/`self.r` NaN on every subsequent frame once a
        // single non-finite rms sample arrives.
        let mut v = Vu::default();
        let mut c = Canvas::new(190, 60);
        v.draw(&mut c, &builtin::vu_cream(), &level(0.5, 0.5));
        v.draw(&mut c, &builtin::vu_cream(), &level(f32::NAN, f32::NAN));
        assert!(v.l.is_finite(), "a NaN target must not leave l non-finite, got {}", v.l);
        assert!(v.r.is_finite(), "a NaN target must not leave r non-finite, got {}", v.r);

        // Confirm the poisoning doesn't linger: subsequent normal frames must
        // still converge, not stay NaN forever.
        for _ in 0..80 {
            v.draw(&mut c, &builtin::vu_cream(), &level(0.7, 0.7));
        }
        // The needle tracks needle_level(rms), not rms itself - it is a dB
        // instrument. An rms of 0.7 is -3.1 dBFS, which maps to ~0.93 of arc travel.
        // Asserting 0.7 here would be asserting the old linear behaviour.
        let want = needle_level(0.7, builtin::vu_cream().sensitivity);
        assert!((v.l - want).abs() < 0.01, "want ~{want}, got {}", v.l);
        assert!((v.r - want).abs() < 0.01, "want ~{want}, got {}", v.r);
    }

    #[test]
    fn golden_vu_cream() {
        let mut v = Vu::default();
        let mut c = Canvas::new(190, 60);
        for _ in 0..80 {
            v.draw(&mut c, &builtin::vu_cream(), &level(0.65, 0.4));
        }
        let expected = include_str!("../../tests/golden/vu-cream.txt");
        // `matches_golden` normalises line endings - a bare `assert_eq!` on the
        // string broke every golden test once already on a branch merge where
        // git checked the committed file out as CRLF while `canvas_to_ascii`
        // emits LF, with byte-identical content otherwise.
        assert!(
            crate::render::golden::matches_golden(&c, expected),
            "golden mismatch - if this change is intended, overwrite \
             tests/golden/vu-cream.txt and eyeball the diff:\n{}",
            canvas_to_ascii(&c)
        );
    }

    #[test]
    #[ignore]
    fn regenerate_golden() {
        let mut v = Vu::default();
        let mut c = Canvas::new(190, 60);
        for _ in 0..80 {
            v.draw(&mut c, &builtin::vu_cream(), &level(0.65, 0.4));
        }
        std::fs::write("tests/golden/vu-cream.txt", canvas_to_ascii(&c)).unwrap();
    }

    /// Dumps every VU colourway to raw RGBA for visual inspection.
    /// Run: cargo test --release dump_vu_frames -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_vu_frames() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/eyeball");
        std::fs::create_dir_all(&dir).unwrap();
        let mut n = 0usize;
        for t in builtin::all().into_iter().filter(|t| t.family == "vu") {
            let mut v = Vu::default();
            let mut c = Canvas::new(190, 60);
            // settle the ballistics at a realistic listening level
            for _ in 0..120 {
                v.draw(&mut c, &t, &level(0.09, 0.055));
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
            std::fs::write(dir.join(format!("vu-{}.rgba", t.id)), &out).unwrap();
            n += 1;
        }
        println!("wrote {} vu dumps to {}", n, dir.display());
    }
}
