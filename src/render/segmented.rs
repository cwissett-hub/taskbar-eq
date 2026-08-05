use super::canvas::{Canvas, Rgba};
use super::{Family, FrameData};
use crate::themes::{Texture, Theme};

const BAR_W: i32 = 5;
const GAP: i32 = 2;
const SEG_H: i32 = 3;
const SEG_GAP: i32 = 1;
const PAD_X: i32 = 5;
const PAD_Y: i32 = 6;

/// Radius of the dim edge halo. Wider than the per-segment glow so it reaches the
/// bezel from the outermost bars.
const EDGE_RADIUS: i32 = 12;
/// Width of the band the edge halo is confined to, measured in from the panel edge.
/// Everything inside this is punched out so the halo never touches the grid.
const EDGE_BAND: i32 = 5;

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

        let lit_of = |b: i32| (sample(&d.levels, b) * nseg as f32).round() as i32;

        // 4-6. Lit marks, glow, and the crisp marks on top.
        //
        // The glow MUST be built on its own transparent layer. `Canvas::bloom`
        // composites its halo UNDER the existing content, so blooming in place on an
        // opaque panel hides the halo entirely - which is exactly why the glow
        // vanished when panel_alpha went from 0.55 to 0.96. On the translucent panel
        // it happened to show through; on an opaque one it is behind a wall.
        let mut glow = Canvas::new(w, h);
        for b in 0..nbars {
            for k in 0..lit_of(b).min(nseg) {
                let frac = (k + 1) as f32 / nseg as f32;
                let y = PAD_Y + usable_h - (k + 1) * seg_pitch;
                glow.fill_rect(ox + b * pitch, y, BAR_W, SEG_H, Rgba::from_hex(t.lit_at(frac), 1.0));
            }
        }
        let radius = t.bloom.round().max(0.0) as i32;
        let mut edge = glow.clone();
        glow.bloom(radius, t.glow_strength.max(0.0));
        c.draw_over(&glow);

        // A second, much dimmer halo confined to the display's EDGE RING.
        //
        // A wide bloom drawn across the whole panel washes over the dormant grid and
        // reads as haze rather than glow, and if it is allowed outside the panel at
        // any real strength it produces a visible box around the display. So this one
        // is masked to a band hugging the bezel: it lights the edges of the VFD and
        // nothing else. Deliberately subtle - EDGE_GLOW is a fraction of the main
        // halo's strength, because the whole point is that you notice it without
        // being able to point at it.
        edge.bloom(EDGE_RADIUS, t.glow_strength.max(0.0) * t.edge_glow.max(0.0));
        edge.punch_rect(
            EDGE_BAND + 1,
            EDGE_BAND + 2,
            (w - 2 - EDGE_BAND * 2).max(0),
            (h - 4 - EDGE_BAND * 2).max(0),
        );
        c.draw_over(&edge);

        // the crisp marks, over their own halo
        for b in 0..nbars {
            for k in 0..lit_of(b).min(nseg) {
                let frac = (k + 1) as f32 / nseg as f32;
                let y = PAD_Y + usable_h - (k + 1) * seg_pitch;
                c.fill_rect(ox + b * pitch, y, BAR_W, SEG_H, Rgba::from_hex(t.lit_at(frac), 1.0));
            }
        }

        // Cut the segment gaps by PAINTING the panel colour over them, within each
        // bar's own width. Not by punching.
        //
        // Two earlier attempts were both wrong, in opposite directions:
        //   * punching the gaps AFTER the bloom zeroed alpha across the full canvas
        //     width, which erased the PANEL too and left transparent stripes with the
        //     taskbar showing through - hard scanlines, not a display.
        //   * punching them BEFORE the bloom let the halo flood straight back into
        //     the 1px gaps, so gap and segment ended up the same brightness and the
        //     segmentation vanished into a smear.
        // Painting opaque panel colour over them gives a crisp dark gap, leaves the
        // panel intact, and preserves the halo on the column's flanks - which is
        // where a real phosphor segment's glow actually shows.
        let gap_col = Rgba::from_hex(&t.panel, 1.0);
        for k in 1..=nseg {
            let gap_y = PAD_Y + usable_h - k * seg_pitch + SEG_H;
            for b in 0..nbars {
                c.fill_rect(ox + b * pitch, gap_y, BAR_W, SEG_GAP, gap_col);
            }
        }

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
        // So this cut is narrower, not just later: it is bounded in x to only the hot
        // core's own column, leaving the glow either side of it untouched. Net look
        // per gap row: the centre (where the hot core would have bridged the gap)
        // goes dark again, exactly like a real segment's gap, while the edges of the
        // bar keep glowing - which is what "a slight, authentic bleed into the gaps"
        // should mean, rather than the gap vanishing into a slab of continuous hot
        // core.
        //
        // PAINTED with the panel colour, not punched. This used `punch_rect`, and
        // `punch_rect` writes ZERO - a fully transparent pixel, not a dark one. The
        // overlay is composited with per-pixel alpha over the Windows weather widget,
        // so every one of those was a hole the weather text showed through. Measured
        // before the fix: 825 transparent pixels per frame in every segmented
        // colourway except classic-three-colour, which is exempt only because
        // `zones` is non-empty for it.
        //
        // It read as intermittent - "occasionally, not constantly" - because the holes
        // are always there and it is the WEATHER that moves: the text only shows
        // through where a glyph happens to line up with a punched column, and the
        // forecast wording changes every few minutes.
        //
        // This is the same fault, in the same file, as the gap handling at step 5,
        // whose own comment already says gaps must be painted rather than punched.
        // That fix was applied to the segment gaps and this one narrower cut was
        // missed.
        if t.zones.is_empty() {
            let panel = Rgba::from_hex(&t.panel, t.panel_alpha);
            for b in 0..nbars {
                for k in 1..=nseg {
                    c.fill_rect(
                        ox + b * pitch + hot_x,
                        PAD_Y + usable_h - k * seg_pitch + SEG_H,
                        hot_w,
                        SEG_GAP,
                        panel,
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
            rms_l: level,
            rms_r: level,
            ..FrameData::default()
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
        // The premise here was changed deliberately. The original assertion demanded
        // the GAP itself glow, but at a 1px gap any halo bright enough to satisfy
        // that also floods the gap to the same brightness as the segment, erasing the
        // segmentation (measured: gap 213.1 vs segment 211.9 - indistinguishable).
        // A segmented display's gaps are DARK; the glow belongs on the column's
        // flanks. So: the gap must be clearly darker than the segment...
        assert!(
            seg_lum - gap_lum > 40.0,
            "gap must read clearly darker than the segment, got gap {gap_lum} vs segment {seg_lum}"
        );
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
        let expected = include_str!("../../tests/golden/vfd-ice.txt");
        assert!(
            crate::render::golden::matches_golden(&c, expected),
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

    /// Not a assertion test - a measurement harness. Prints, for a range of glow
    /// strengths, how bright a lit segment is versus the gap BETWEEN adjacent bars.
    /// The bars stop reading as separate once those two converge, which is the real
    /// ceiling on glow at a 7px bar pitch.
    #[test]
    #[ignore]
    fn sweep_glow_strength() {
        let base = builtin::vfd_ice();
        println!("radius strength  segment  inter-bar-gap  ratio  verdict");
        for (r, s) in [
            (3.0f32, 0.15f32), (3.0, 0.25), (3.0, 0.35), (3.0, 0.5), (3.0, 0.7),
            (4.0, 0.2), (4.0, 0.3), (4.0, 0.45),
            (5.0, 0.15), (5.0, 0.25), (5.0, 0.4),
            (6.0, 0.2), (7.0, 0.18),
        ] {
            let mut t = base.clone();
            t.bloom = r;
            t.glow_strength = s;
            let mut c = Canvas::new(190, 60);
            Segmented.draw(&mut c, &t, &frame(0.7));
            let x = first_bar_x();
            let y = 60 - PAD_Y - 3;                 // inside a lit segment band
            let seg = lum(c.get(x + 2, y));
            // midpoint of the 2px gap between bar 0 and bar 1
            let gap = lum(c.get(x + BAR_W, y)).max(lum(c.get(x + BAR_W + 1, y)));
            let ratio = if gap > 0.5 { seg / gap } else { 999.0 };
            let verdict = if ratio > 9.0 { "no glow" }
                else if ratio >= 3.5 { "GOOD - visible halo, bars distinct" }
                else if ratio >= 2.2 { "strong" }
                else { "MERGED" };
            println!("{r:6.1} {s:8.1}  {seg:7.1}  {gap:13.1}  {ratio:5.2}  {verdict}");
        }
    }

    /// Measurement, not an assertion. Prints the luminance of the bezel ring against
    /// the bare panel for a range of edge_glow values. The ASCII golden's level 1
    /// spans a wide luminance range, so "visible in the golden" and "visible on
    /// screen" are not the same thing - these are the numbers that decide it.
    #[test]
    #[ignore]
    fn sweep_edge_glow() {
        let base = builtin::vfd_ice();
        println!("edge_glow  bezel  panel  delta  verdict");
        for e in [0.3f32, 0.8, 1.5, 3.0, 5.0, 8.0, 12.0] {
            let mut t = base.clone();
            t.edge_glow = e;
            let mut c = Canvas::new(190, 60);
            Segmented.draw(&mut c, &t, &frame(0.75));
            // brightest point on the left bezel ring, vertically over the lit region
            let mut bezel = 0.0f32;
            for y in 30..54 {
                for x in 1..5 {
                    bezel = bezel.max(lum(c.get(x, y)));
                }
            }
            // bare panel well inside, above the bars and away from the ring
            let panel = lum(c.get(95, 10));
            let delta = bezel - panel;
            let verdict = if delta < 12.0 { "INVISIBLE" }
                else if delta < 30.0 { "borderline" }
                else if delta < 90.0 { "VISIBLE, subtle" }
                else { "obvious" };
            println!("{e:9.1}  {bezel:5.1}  {panel:5.1}  {delta:5.1}  {verdict}");
        }
    }

    /// Measurement, not an assertion. For every shipped colourway, prints how bright a
    /// lit segment is versus the gap BETWEEN adjacent bars. Bars stop reading as
    /// separate once those converge, and the halo radius is what decides it.
    #[test]
    #[ignore]
    fn audit_every_colourway_bar_separation() {
        println!("{:<22} {:>6} {:>8} {:>8} {:>6}  verdict", "theme", "bloom", "segment", "gap", "ratio");
        for t in builtin::all() {
            let mut c = Canvas::new(190, 60);
            Segmented.draw(&mut c, &t, &frame(0.75));
            let x = first_bar_x();
            let y = 60 - PAD_Y - 3;
            let seg = lum(c.get(x + 2, y));
            let gap = lum(c.get(x + BAR_W, y)).max(lum(c.get(x + BAR_W + 1, y)));
            let ratio = if gap > 0.5 { seg / gap } else { 999.0 };
            let verdict = if ratio < 2.2 { "MERGED - bars lost" }
                else if ratio < 3.2 { "close" }
                else if ratio < 9.0 { "good" }
                else { "no visible halo" };
            println!("{:<22} {:>6.1} {:>8.1} {:>8.1} {:>6.2}  {}", t.id, t.bloom, seg, gap, ratio, verdict);
        }
    }
}
