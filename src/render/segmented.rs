use super::canvas::{Canvas, Rgba};
use super::{Family, FrameData};
use crate::themes::{Texture, Theme};

const BAR_W: i32 = 5;
const GAP: i32 = 2;
const SEG_H: i32 = 3;
const SEG_GAP: i32 = 1;
const PAD_X: i32 = 5;
const PAD_Y: i32 = 6;

/// How much brighter than "energy-preserving" the halo is driven. 1.0 would make
/// a wide radius look identical in intensity to a narrow one - only more spread -
/// because the box blur normalises by kernel size. Above 1.0 is what actually
/// reads as phosphor glow. Chosen by eye against the live taskbar.
const GLOW_GAIN: f32 = 3.4;

/// Couples strength to radius. `Canvas::bloom` is a box blur and normalises by
/// kernel size, so raising the radius alone makes the halo FAINTER, not stronger -
/// the opposite of the intent, and a trap for every theme that follows. Scaling by
/// radius squared cancels the normalisation; GLOW_GAIN then overdrives it.
fn bloom_strength(bloom: f32) -> f32 {
    const REF_RADIUS: f32 = 9.0;
    let r = bloom.max(1.0);
    ((r / REF_RADIUS).powi(2) * 0.85 * GLOW_GAIN).clamp(0.6, 12.0)
}

pub struct Segmented;

impl Family for Segmented {
    fn id(&self) -> &'static str {
        "segmented"
    }

    fn draw(&mut self, c: &mut Canvas, t: &Theme, d: &FrameData) {
        let (w, h) = (c.width(), c.height());
        c.clear();

        // 1. panel
        c.rounded_rect(1, 2, w - 2, h - 4, 4, Rgba::from_hex(&t.panel, t.panel_alpha));

        // 2. panel texture
        match t.texture {
            Texture::Glass => {
                for y in 2..(h / 2) {
                    let a = 0.09 * (1.0 - (y - 2) as f32 / (h / 2 - 2) as f32);
                    c.fill_rect(1, y, w - 2, 1, Rgba::from_hex("#bef0ff", a));
                }
            }
            Texture::Scanlines => {
                let mut y = 2;
                while y < h - 4 {
                    c.fill_rect(1, y, w - 2, 1, Rgba::from_hex(&t.lit, 0.045));
                    y += 2;
                }
            }
            Texture::Filament => {
                for y in (h / 2)..(h - 4) {
                    let a = 0.20 * (y - h / 2) as f32 / (h / 2 - 4) as f32;
                    c.fill_rect(1, y, w - 2, 1, Rgba::from_hex(&t.lit, a));
                }
            }
            Texture::Haze => {
                for y in 2..(h - 4) {
                    let dy = ((y - h / 2) as f32 / (h as f32 * 0.5)).abs();
                    c.fill_rect(1, y, w - 2, 1, Rgba::from_hex(&t.lit, 0.13 * (1.0 - dy).max(0.0)));
                }
            }
            Texture::Grille => {
                let mut x = 1;
                while x < w - 1 {
                    c.fill_rect(x, 2, 1, h - 4, Rgba::from_hex("#ffffff", 0.028));
                    x += 3;
                }
            }
            Texture::None_ => {}
        }

        // 3. geometry from the live rect
        let usable_w = w - PAD_X * 2;
        let usable_h = h - PAD_Y * 2;
        let pitch = BAR_W + GAP;
        let nbars = (usable_w / pitch).max(1);
        let ox = PAD_X + (usable_w - nbars * pitch + GAP) / 2;
        let seg_pitch = SEG_H + SEG_GAP;
        let nseg = (usable_h / seg_pitch).max(1);

        let sample = |arr: &[f32], b: i32| -> f32 {
            let lo = (b as usize * arr.len()) / nbars as usize;
            let hi = (((b + 1) as usize * arr.len()) / nbars as usize).max(lo + 1);
            arr[lo..hi.min(arr.len())].iter().copied().fold(0.0f32, f32::max)
        };

        // 4. lit columns FIRST, so that only they feed the bloom.
        let lit_of = |b: i32| (sample(&d.levels, b) * nseg as f32).round() as i32;
        for b in 0..nbars {
            for k in 0..lit_of(b).min(nseg) {
                let frac = (k + 1) as f32 / nseg as f32;
                let y = PAD_Y + usable_h - (k + 1) * seg_pitch;
                c.fill_rect(ox + b * pitch, y, BAR_W, SEG_H, Rgba::from_hex(t.lit_at(frac), 1.0));
            }
        }

        // 5. punch the segment gaps out of the lit marks BEFORE the bloom, not
        // after.
        //
        // The gaps must be punched before `bloom` runs, not after: `bloom` only
        // sees whatever is on the canvas at the moment it runs, so blooming the
        // *unbroken* lit columns and only then punching the gaps back out
        // erases the halo in precisely the rows between segments - the punch
        // (`punch_row`) zeroes alpha across the FULL WIDTH for each gap row,
        // which destroys any glow that landed there. That produced a
        // hard-edged, glow-less meter where the segmentation should have been
        // visible in the halo too.
        //
        // Punching first means `bloom` blurs the already-segmented shape, so
        // the halo it produces wraps each lit segment individually and bleeds
        // a little into the gaps - which is the point: a real phosphor
        // segment's glow does not stop dead at the segment's physical edge.
        for k in 1..=nseg {
            c.punch_row(PAD_Y + usable_h - k * seg_pitch + SEG_H, SEG_GAP);
        }

        // 6. bloom - the now-segmented lit marks only.
        //
        // Radius and strength must be coupled: Canvas::bloom is a box blur and
        // normalises by kernel size, so a wider radius spreads the same energy over
        // more pixels and the halo gets FAINTER. Raising `bloom` alone therefore dims
        // the glow, the opposite of the intent. See bloom_strength().
        //
        // And this must happen BEFORE the dormant grid is drawn. Blooming the ghost
        // grid too drove it to full alpha at these gain levels, so the idle grid glowed
        // as brightly as a lit segment and the meter read as permanently full.
        let radius = t.bloom.round().max(0.0) as i32;
        c.bloom(radius, bloom_strength(t.bloom));

        // 7. clip the halo to the panel interior.
        //
        // The panel (step 1) is only inset 1-2px from the canvas edge, but the
        // bloom above can spread up to `radius` pixels in every direction -
        // far past that thin margin - so without this the glow spills onto
        // the bare taskbar outside the rounded panel and reads as a bright
        // blue/white edge sitting outside the display. `Canvas::bloom` itself
        // only clips at the canvas boundary, not the panel's, so this must be
        // its own step, run immediately after `bloom` and before anything else
        // is drawn (the later layers are already confined to the panel
        // interior by construction, so nothing after this can reintroduce the
        // leak). `clip_to_rounded_rect` shares `rounded_rect`'s own corner
        // math, so this respects the rounded corners exactly rather than just
        // clipping to the bounding box.
        c.clip_to_rounded_rect(1, 2, w - 2, h - 4, 4);

        // 8. dormant ghost grid - zone-coloured when zones are present. Drawn after
        // the bloom so it stays crisp and dim, and only in the UNLIT part of each bar
        // so it never tints a lit segment.
        if t.ghost > 0.0 {
            for b in 0..nbars {
                let lit = lit_of(b).min(nseg);
                for k in lit..nseg {
                    let frac = (k + 1) as f32 / nseg as f32;
                    let col = Rgba::from_hex(t.lit_at(frac), t.ghost);
                    let y = PAD_Y + usable_h - (k + 1) * seg_pitch;
                    c.fill_rect(ox + b * pitch, y, BAR_W, SEG_H, col);
                }
            }
        }

        // 9. hot core: a narrower brighter rect, not a gradient. Drawn as one
        // continuous rect per bar (not per segment) rather than a gradient, so
        // it needs its own re-punch below to stay segmented.
        let hot_x = (BAR_W as f32 * 0.28) as i32;
        let hot_w = (BAR_W as f32 * 0.44).ceil() as i32;
        if t.zones.is_empty() {
            for b in 0..nbars {
                let lit = (sample(&d.levels, b) * nseg as f32).round() as i32;
                if lit <= 0 {
                    continue;
                }
                let hh = lit.min(nseg) * seg_pitch - SEG_GAP;
                c.fill_rect(
                    ox + b * pitch + hot_x,
                    PAD_Y + usable_h - hh,
                    hot_w,
                    hh,
                    Rgba::from_hex(&t.hot, 0.55),
                );
            }
        }

        // 10. re-cut the hot core's own gaps.
        //
        // The hot core above is drawn as ONE continuous rect per bar, so it
        // now paints straight across the segment gaps the bloom was made to
        // glow in at step 5 - the middle path promised there. A second
        // full-width punch (like step 5's, or the original single punch this
        // fix replaced) would fix the hot core but also re-erase the halo
        // that step 5 + bloom deliberately left glowing at the bar's edges
        // and in the margins between bars, undoing the fix for fault 2.
        //
        // So this punch is narrower, not just later: `punch_rect` (unlike
        // `punch_row`) is bounded in x to only the hot core's own column,
        // leaving the glow either side of it untouched. Net look per gap
        // row: the centre (where the hot core would have bridged the gap)
        // goes dark again, exactly like a real segment's gap, while the
        // edges of the bar keep glowing - which is what "a slight, authentic
        // bleed into the gaps" should mean, rather than the gap vanishing
        // into a slab of continuous hot core.
        if t.zones.is_empty() {
            for b in 0..nbars {
                for k in 1..=nseg {
                    c.punch_rect(
                        ox + b * pitch + hot_x,
                        PAD_Y + usable_h - k * seg_pitch + SEG_H,
                        hot_w,
                        SEG_GAP,
                    );
                }
            }
        }

        // 11. peak-hold caps
        for b in 0..nbars {
            let pk = (sample(&d.peaks, b) * nseg as f32).round() as i32;
            if pk <= 0 {
                continue;
            }
            let frac = pk as f32 / nseg as f32;
            c.fill_rect(
                ox + b * pitch,
                PAD_Y + usable_h - pk * seg_pitch,
                BAR_W,
                1,
                Rgba::from_hex(t.hot_at(frac), 1.0),
            );
        }

        // 12. bezel
        let e = Rgba::from_hex(&t.edge, t.edge_alpha);
        c.fill_rect(1, 2, w - 2, 1, e);
        c.fill_rect(1, h - 3, w - 2, 1, e);
        c.fill_rect(1, 2, 1, h - 4, e);
        c.fill_rect(w - 2, 2, 1, h - 4, e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::bands::NUM_BANDS;
    use crate::render::golden::canvas_to_ascii;
    use crate::themes::builtin;

    /// Perceived brightness. Use this rather than the alpha channel: the panel is
    /// ~0.96 opaque, so alpha is near-saturated everywhere on it and an alpha
    /// assertion silently measures the panel instead of the mark.
    fn lum(p: crate::render::canvas::Rgba) -> f32 {
        0.2126 * p.r as f32 + 0.7152 * p.g as f32 + 0.0722 * p.b as f32
    }

    fn frame(level: f32) -> FrameData {
        FrameData {
            levels: [level; NUM_BANDS],
            peaks: [level; NUM_BANDS],
            waveform: [0.0; 256],
            rms_l: level,
            rms_r: level,
        }
    }

    #[test]
    fn silence_still_draws_the_panel_and_ghost_grid() {
        let mut c = Canvas::new(190, 60);
        Segmented.draw(&mut c, &builtin::vfd_ice(), &frame(0.0));
        // Panel is opaque enough to be visible.
        assert!(c.get(95, 30).a > 100, "panel must be drawn even in silence");
        // Ghost grid present but dim.
        let ascii = canvas_to_ascii(&c);
        assert!(ascii.contains('.') || ascii.contains(':'), "expected a dim ghost grid");
        assert!(!ascii.contains('@'), "nothing should be fully lit in silence");
    }

    /// x of the first bar's left edge. The brief's original tests sampled at
    /// `PAD_X + 2`, which assumes the bar grid starts exactly at PAD_X. It
    /// doesn't: `draw` centres the 25-bar grid inside the usable width, and
    /// for 190x60 that centring offset is 3px (measured: ox=8, not 5), so
    /// `PAD_X + 2` (=7) landed one pixel to the *left* of the first bar - in
    /// the unlit margin, not "2px into" it. A live probe there read alpha
    /// 199 (bloom bleeding just past the edge) against 255 one column to the
    /// right: an easy-to-miss near pass/fail at the >200 threshold.
    /// Recomputing ox from the real geometry (mirroring `draw`'s own
    /// formula) keeps the sample point correct if the constants ever change.
    fn first_bar_x() -> i32 {
        let usable_w = 190 - PAD_X * 2;
        let pitch = BAR_W + GAP;
        let nbars = usable_w / pitch;
        PAD_X + (usable_w - nbars * pitch + GAP) / 2
    }

    #[test]
    fn full_level_lights_the_top_of_the_bars() {
        let mut c = Canvas::new(190, 60);
        Segmented.draw(&mut c, &builtin::vfd_ice(), &frame(1.0));
        let top = c.get(first_bar_x() + 2, PAD_Y + 1);
        assert!(lum(top) > 150.0, "top segment should be lit at full level");
    }

    #[test]
    fn half_level_lights_the_bottom_but_not_the_top() {
        let mut c = Canvas::new(190, 60);
        Segmented.draw(&mut c, &builtin::vfd_ice(), &frame(0.5));
        let bottom = c.get(first_bar_x() + 2, 60 - PAD_Y - 2);
        let top = c.get(first_bar_x() + 2, PAD_Y + 1);
        // Assert on BRIGHTNESS, not alpha. The panel is 0.96 opaque, so every pixel
        // on it has alpha ~245 whether or not a segment is lit - an alpha-based
        // assertion here passes on the panel alone and tests nothing.
        assert!(lum(bottom) > 120.0, "bottom must be lit, got lum {}", lum(bottom));
        assert!(
            lum(top) < lum(bottom) * 0.5,
            "top must be clearly dimmer than bottom at half level (top {}, bottom {})",
            lum(top),
            lum(bottom)
        );
    }

    #[test]
    fn segment_gaps_are_punched_through_to_transparent() {
        let mut c = Canvas::new(190, 60);
        Segmented.draw(&mut c, &builtin::vfd_ice(), &frame(1.0));
        // Walk a lit column and confirm the luminance is not monotonic - the
        // gaps must interrupt it. This is what distinguishes a segmented meter
        // from a solid bar.
        // Must sample inside a real bar. The geometry CENTRES the bars, so the
        // first one starts at first_bar_x() (8 at 190px wide), not at PAD_X - and
        // PAD_X + 2 lands in the left margin, where the column is uniform and this
        // test silently measures nothing.
        let x = first_bar_x() + 2;
        let mut transitions = 0;
        let mut prev = lum(c.get(x, PAD_Y)) > 90.0;
        for y in PAD_Y..(60 - PAD_Y) {
            let now = lum(c.get(x, y)) > 90.0;
            if now != prev {
                transitions += 1;
            }
            prev = now;
        }
        assert!(transitions >= 6, "expected several segment gaps, saw {transitions}");
    }

    #[test]
    fn segment_gaps_glow_instead_of_going_dark() {
        let mut c = Canvas::new(190, 60);
        Segmented.draw(&mut c, &builtin::vfd_ice(), &frame(0.5));
        // Sample the EDGE of the first bar (offset 0 of its 5px width), which
        // sits outside the hot core's narrower highlight strip - measuring
        // the halo the bloom leaves behind, not the hot core's flat colour.
        let x = first_bar_x();
        let usable_h = 60 - PAD_Y * 2;
        let seg_pitch = SEG_H + SEG_GAP;
        // k=2: a gap deep inside the lit region (this mirrors `draw`'s own
        // gap-row formula, `PAD_Y + usable_h - k * seg_pitch + SEG_H`), far
        // from both the panel edge and the lit/unlit boundary, so this is
        // unambiguously "between two lit segments", not an edge case.
        let gap_y = PAD_Y + usable_h - 2 * seg_pitch + SEG_H;
        let seg_y = gap_y - 2; // inside the solid segment band just below it
        let gap_lum = lum(c.get(x, gap_y));
        let seg_lum = lum(c.get(x, seg_y));
        // Bug: punching the gaps AFTER bloom zeroed them outright (lum 0).
        // Fix: the gap must show a real glow - clearly brighter than the
        // near-black bare panel (lum ~9 for #040a0e at 0.96 alpha).
        assert!(gap_lum > 100.0, "gap row should glow, got lum {gap_lum}");
        // And the two rows must still read as distinct bands - a real
        // segment gap, not a uniform smear that erased the segmentation.
        assert!(
            (gap_lum - seg_lum).abs() > 15.0,
            "gap and segment rows should be visibly different, got gap {gap_lum} vs segment {seg_lum}"
        );
    }

    #[test]
    fn nothing_glows_outside_the_panel_rect() {
        // The panel (step 1) occupies x in [1, w-2) and y in [2, h-4) - see
        // the rounded_rect call in `draw`. Bloom radius is up to 16px, far
        // more than that 1-2px margin, so without clipping the halo spills
        // onto the bare taskbar and reads as a bright edge outside the
        // display. Checked at full level, where the halo is strongest.
        let mut c = Canvas::new(190, 60);
        Segmented.draw(&mut c, &builtin::vfd_ice(), &frame(1.0));
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
    fn bar_count_matches_the_geometry() {
        // (190 - 10) / 7 = 25 bars at the measured widget width.
        let usable = 190 - PAD_X * 2;
        assert_eq!(usable / (BAR_W + GAP), 25);
    }

    #[test]
    fn nothing_is_drawn_outside_the_canvas() {
        // Narrow rect - must clip, not panic.
        let mut c = Canvas::new(40, 20);
        Segmented.draw(&mut c, &builtin::vfd_ice(), &frame(1.0));
        assert_eq!(c.bits().len(), 40 * 20);
    }

    #[test]
    fn golden_vfd_ice_at_half_level() {
        let mut c = Canvas::new(190, 60);
        Segmented.draw(&mut c, &builtin::vfd_ice(), &frame(0.5));
        let actual = canvas_to_ascii(&c);
        let expected = include_str!("../../tests/golden/vfd-ice.txt");
        assert_eq!(
            actual, expected,
            "golden mismatch - if this change is intended, overwrite \
             tests/golden/vfd-ice.txt and eyeball the diff"
        );
    }

    #[test]
    #[ignore]
    fn regenerate_golden() {
        let mut c = Canvas::new(190, 60);
        Segmented.draw(&mut c, &builtin::vfd_ice(), &frame(0.5));
        std::fs::write("tests/golden/vfd-ice.txt", canvas_to_ascii(&c)).unwrap();
    }

    #[test]
    fn golden_classic_three_colour_at_high_level() {
        let mut c = Canvas::new(190, 60);
        // 0.9 so all three colour zones are lit and visible in the golden.
        Segmented.draw(&mut c, &crate::themes::builtin::classic_three_colour(), &frame(0.9));
        let actual = canvas_to_ascii(&c);
        let expected = include_str!("../../tests/golden/classic-three-colour.txt");
        assert_eq!(actual, expected, "golden mismatch - regenerate and eyeball the diff");
    }

    #[test]
    fn zoned_themes_skip_the_hot_core() {
        // Real coloured LEDs are flat; a hot core would wash out the zone colour.
        let mut c = Canvas::new(190, 60);
        let t = crate::themes::builtin::classic_three_colour();
        Segmented.draw(&mut c, &t, &frame(1.0));
        let centre = c.get(first_bar_x() + 2, 20);
        let edge = c.get(first_bar_x(), 20);
        assert!(
            centre.r.abs_diff(edge.r) < 40,
            "zoned bars must be flat across their width, got centre {centre:?} edge {edge:?}"
        );
    }

    #[test]
    #[ignore]
    fn regenerate_classic_golden() {
        let mut c = Canvas::new(190, 60);
        Segmented.draw(&mut c, &crate::themes::builtin::classic_three_colour(), &frame(0.9));
        std::fs::write("tests/golden/classic-three-colour.txt", canvas_to_ascii(&c)).unwrap();
    }
}
